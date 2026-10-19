//! The exports a game's worklet shim calls (Midnight Playground's
//! `crates/mp_audio/web/music-worklet.js`, Snowboarding's
//! `crates/sb_game/web/music-worklet.js`): the station player as its own
//! wasm on the audio thread (no wasm-bindgen; plain numbers and pointers).
//!
//! The page gives the wasm the game's station table first: it copies the
//! JSON ([`Station::parse_list`]) into memory from [`music_alloc`] and
//! calls [`music_set_stations`]. Then the worklet creates one player per
//! node with [`music_new`], forwards the node's parameters with
//! [`music_params`] once per block, and calls [`music_process`], which
//! renders 128 frames into the player's own buffer (left then right) and
//! returns its address in linear memory.

// The exports take the pointers their own calls handed out; JS can't call
// an `unsafe fn` any differently, so each documents its contract instead.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::json::{Val, canon};
use crate::player::Player;
use crate::radio::{Station, block, block_at, cue_in, wall_from};
use std::cell::RefCell;

/// The worklet's block.
pub const BLOCK: usize = 128;

thread_local! {
    /// The last string an export made, for the page to read (one thread).
    static TEXT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// The game's stations, from [`music_set_stations`].
    static STATIONS: RefCell<Vec<Station>> = const { RefCell::new(Vec::new()) };
}

fn give(text: String) -> *const u8 {
    TEXT.with(|t| {
        *t.borrow_mut() = text.into_bytes();
        t.borrow().as_ptr()
    })
}

/// The length of the string the last [`music_stations`],
/// [`music_schedule`] or failed [`music_set_stations`] made.
#[unsafe(no_mangle)]
pub extern "C" fn music_text_len() -> usize {
    TEXT.with(|t| t.borrow().len())
}

/// The address of that string.
#[unsafe(no_mangle)]
pub extern "C" fn music_text() -> *const u8 {
    TEXT.with(|t| t.borrow().as_ptr())
}

/// `len` bytes for the page to write into, handed back to
/// [`music_set_stations`], which frees them.
#[unsafe(no_mangle)]
pub extern "C" fn music_alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// Sets the game's station table from the JSON at `ptr` (`len` bytes from
/// [`music_alloc`], freed here). Returns the number of stations, or -1 when
/// the table is bad (the error is then the text, [`music_text`]). Players
/// made afterwards play these stations.
#[unsafe(no_mangle)]
pub extern "C" fn music_set_stations(ptr: *mut u8, len: usize) -> i32 {
    // SAFETY: `ptr` and `len` came from `music_alloc` and are freed once.
    let bytes = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) };
    let parsed = std::str::from_utf8(&bytes)
        .map_err(|e| e.to_string())
        .and_then(Station::parse_list);
    match parsed {
        Ok(list) => {
            let n = list.len() as i32;
            STATIONS.with(|s| *s.borrow_mut() = list);
            n
        }
        Err(e) => {
            give(e);
            -1
        }
    }
}

/// The stations as JSON (the form [`music_set_stations`] takes), UTF-8 at
/// the returned address, [`music_text_len`] bytes. For pages that run this
/// wasm on the main thread too (Midnight Playground's `tools/radio.html`).
#[unsafe(no_mangle)]
pub extern "C" fn music_stations() -> *const u8 {
    give(STATIONS.with(|s| Station::list_json(&s.borrow())))
}

/// A station's programme at a wall time, as JSON: `{block, into, slot,
/// step, frac, left, slots: [{genre, seed, start, secs, bpm, bars, title,
/// style}]}` (`into`: seconds into the block; `slot`: the song playing).
#[unsafe(no_mangle)]
pub extern "C" fn music_schedule(station: i32, wall_day: f32, wall_sec: f32) -> *const u8 {
    let st = STATIONS.with(|s| {
        usize::try_from(station)
            .ok()
            .and_then(|i| s.borrow().get(i).cloned())
    });
    let Some(st) = st else {
        return give("null".to_owned());
    };
    let st = &st;
    let wall = wall_from(wall_day as f64, wall_sec as f64);
    let (b, into) = block_at(wall);
    let slots = block(st, b);
    let c = cue_in(b, &slots, into);
    let v = Val::obj(vec![
        ("block", Val::num(b as f64)),
        ("into", Val::num(into)),
        ("slot", Val::num(c.slot as f64)),
        ("step", Val::num(c.step as f64)),
        ("frac", Val::num(c.frac)),
        ("left", Val::num(c.left)),
        (
            "slots",
            Some(Val::Arr(
                slots
                    .iter()
                    .map(|s| {
                        Val::obj(vec![
                            ("genre", Val::str(s.genre)),
                            ("seed", Val::num(s.seed as f64)),
                            ("start", Val::num(s.start)),
                            ("secs", Val::num(s.secs)),
                            ("bpm", Val::num(s.track.bpm)),
                            ("bars", Val::num(s.track.bars() as f64)),
                            ("title", Val::ostr(&s.track.title)),
                            ("style", Val::ostr(&s.track.style)),
                        ])
                    })
                    .collect(),
            )),
        ),
    ]);
    give(canon(&v))
}

pub struct Worklet {
    player: Player,
    buf: [f32; 2 * BLOCK],
}

/// A new player at `sample_rate` for the stations set last, off (silent)
/// until tuned.
#[unsafe(no_mangle)]
pub extern "C" fn music_new(sample_rate: f64, seed: u32) -> *mut Worklet {
    let stations = STATIONS.with(|s| s.borrow().clone());
    Box::into_raw(Box::new(Worklet {
        player: Player::new(seed, sample_rate, stations),
        buf: [0.0; 2 * BLOCK],
    }))
}

/// The node's params for this block (see the worklet's descriptors).
#[unsafe(no_mangle)]
pub extern "C" fn music_params(
    w: *mut Worklet,
    station: f32,
    wall_day: f32,
    wall_sec: f32,
    tune: f32,
    energy: f32,
) {
    // SAFETY: `w` came from `music_new` and has not been freed; the worklet
    // calls these from one thread, one at a time.
    let w = unsafe { &mut *w };
    w.player
        .set_params(station, wall_day, wall_sec, tune, energy);
}

/// Renders one block; returns the address of 2 × [`BLOCK`] `f32`s.
#[unsafe(no_mangle)]
pub extern "C" fn music_process(w: *mut Worklet) -> *const f32 {
    // SAFETY: as above.
    let w = unsafe { &mut *w };
    let (l, r) = w.buf.split_at_mut(BLOCK);
    w.player.process(l, r);
    w.buf.as_ptr()
}

/// Frees a player made by [`music_new`].
#[unsafe(no_mangle)]
pub extern "C" fn music_free(w: *mut Worklet) {
    if !w.is_null() {
        // SAFETY: `w` came from `music_new` and is freed once.
        drop(unsafe { Box::from_raw(w) });
    }
}
