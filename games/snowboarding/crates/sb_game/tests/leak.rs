//! Memory across runs: starting a run again and again, and going back and
//! forth to the menu, must not keep any of the runs before. Measured by
//! the bytes the allocator has out, in a test binary of its own (one test,
//! so nothing else allocates beside it). A leak here grows the wasm heap,
//! which never shrinks, until a phone's browser kills the tab.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use sb_game::Screen;
use sb_game::options::Options;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Duration;

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        LIVE.fetch_add(new as isize - l.size() as isize, Ordering::Relaxed);
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn mb() -> f64 {
    LIVE.load(Ordering::Relaxed) as f64 / 1048576.0
}

/// Frames of uneven length, as a browser gives them: anything the game
/// derives from time then differs from run to run, as it does for a player.
fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        let x = app
            .world()
            .resource::<Jitter>()
            .0
            .wrapping_mul(1_103_515_245)
            .wrapping_add(12345);
        app.world_mut().resource_mut::<Jitter>().0 = x;
        let dt = 1.0 / 60.0 * (0.6 + 0.8 * ((x >> 8) % 1000) as f64 / 1000.0);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            dt,
        )));
        app.update();
    }
}

#[derive(Resource)]
struct Jitter(u32);

/// Glyph atlases: one set per font and size, never freed.
fn atlases(app: &App) -> usize {
    app.world()
        .resource::<bevy::text::FontAtlasSet>()
        .iter()
        .count()
}

/// To a screen, and a while on it: a run through its countdown and GO!.
fn go(app: &mut App, s: Screen) {
    app.world_mut().resource_mut::<NextState<Screen>>().set(s);
    frames(app, if s == Screen::Ride { 300 } else { 30 });
}

#[test]
fn runs_leave_nothing_behind() {
    let mut app = sb_game::headless(Options::from_query("course=timber"));
    app.insert_resource(Jitter(7));
    // A window, so the UI lays out and its text is set in glyphs.
    app.world_mut()
        .spawn((Window::default(), bevy::window::PrimaryWindow));
    app.finish();
    app.cleanup();
    frames(&mut app, 10);
    // Once round everything, so caches and pools are as big as they get.
    for _ in 0..3 {
        go(&mut app, Screen::Ride);
        go(&mut app, Screen::Ride);
        go(&mut app, Screen::Menu);
    }
    let before = mb();
    let atlases_before = atlases(&app);
    for _ in 0..10 {
        go(&mut app, Screen::Ride);
        go(&mut app, Screen::Ride);
        go(&mut app, Screen::Menu);
    }
    let after = mb();
    let atlases_after = atlases(&app);
    println!("live heap: {before:.1} MB after warming up, {after:.1} MB after 20 more runs");
    println!("glyph atlases: {atlases_before} after warming up, {atlases_after} after");
    assert!(
        atlases_after <= atlases_before + 2,
        "text drawn at ever new sizes: {atlases_before} glyph atlases -> {atlases_after}"
    );
    assert!(
        after - before < 2.0,
        "20 runs kept {:.1} MB",
        after - before
    );
}
