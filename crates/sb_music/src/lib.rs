//! The music: midnight-playground's `mp_music` borrowed whole (its Music
//! Lab ported to Rust): the DSP blocks, the instruments and kits, the
//! sequencer, the composer, the genre grammars and the engine that mixes a
//! song, and the radio's station player. Snowboarding's one station is
//! Ten-Eighty FM ([`radio::STATIONS`]): a deterministic schedule of
//! generated songs on the wall clock, drum & bass, techno, eurobeat,
//! trance, garage and house, with the announcers of `sb_radio` over it.
//!
//! The sister's port rules still describe the code: the same structure,
//! constants and order of random draws as the JS lab it came from, JS
//! rounding (`Math.round` is `(x + 0.5).floor()`), `f32` where the lab
//! stored into a `Float32Array`. Its goldens stay in the sister project;
//! the tests here check the engine and the schedule on their own terms.
//!
//! On the web the engine runs as its own small wasm inside an AudioWorklet
//! ([`wasm`] and `crates/sb_game/web/music-worklet.js`, built by `cargo
//! xtask web`), so the game's frame never waits on it. Natively there is
//! no sound yet.

// Only the worklet wasm's C-ABI exports (`wasm`) touch raw pointers.
#![cfg_attr(not(feature = "worklet"), forbid(unsafe_code))]
// A JS index loop stays an index loop, so the port reads beside the JS line
// for line (DECISIONS D52).
#![allow(clippy::needless_range_loop)]

pub mod compose;
pub mod dsp;
pub mod engine;
pub mod genres;
pub mod instruments;
pub mod json;
pub mod kits;
pub mod patches;
pub mod player;
pub mod radio;
pub mod seq;
pub mod track;
#[cfg(all(target_arch = "wasm32", feature = "worklet"))]
pub mod wasm;

/// JS `Math.round`: `floor(x + 0.5)` (half rounds up, also for negatives).
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// JS `ToUint32` of a double: modulo 2^32 of its truncation.
pub fn to_u32(x: f64) -> u32 {
    (x.trunc().rem_euclid(4294967296.0)) as u32
}

/// JS `ToInt32` of a double.
pub fn to_i32(x: f64) -> i32 {
    to_u32(x) as i32
}

/// JS `Math.imul` on two int32 values given as `u32` bit patterns.
pub fn imul(a: u32, b: u32) -> u32 {
    a.wrapping_mul(b)
}
