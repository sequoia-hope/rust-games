//! Web glue: the page (`web/index.html`) picks WebGPU or WebGL2, loads
//! this module and calls [`run`]. It hands in policies ([`load_policy`])
//! and commands ([`stage`]); the client publishes its state on the page's
//! status global (`window.__mech`) each frame: the loader's contract
//! (`state`, `frames`, `ready`) and the selected lane's `telemetry` (JSON).

use bevy::prelude::*;
use pk_shell::{Inbox, bridge};
use wasm_bindgen::prelude::*;

use crate::impact::Range;
use crate::{Mode, Status};

#[wasm_bindgen]
pub fn run() {
    bridge::install("__mech", &[]);
    let search = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    let q = pk_shell::params::Params::from_query(&search);
    let mode = Mode::parse(q.get("mode").unwrap_or(""));
    let mut app = crate::app(mode);
    app.add_systems(Last, (publish, take_screenshot));
    app.run();
}

/// The page's user-gesture hook (nothing needs one yet: no sound).
#[wasm_bindgen]
pub fn gesture(_kind: &str) {}

static SCREENSHOT: Inbox<String> = Inbox::new();

/// Saves the next frame as a download named `name` (a test hook: headless
/// Chrome does not composite a WebGPU canvas into its own screenshots);
/// sets the global's `shot` to the name when done.
#[wasm_bindgen]
pub fn screenshot(name: String) {
    SCREENSHOT.put(name);
}

fn take_screenshot(mut commands: Commands) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let Some(name) = SCREENSHOT.take() else {
        return;
    };
    let done = name.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(name))
        .observe(move |_: On<ScreenshotCaptured>| {
            bridge::set("shot", done.as_str());
        });
}

/// A policy blob for a lane, with a label for the readouts.
#[wasm_bindgen]
pub fn load_policy(lane: usize, label: String, bytes: Vec<u8>) {
    crate::hand_policy(lane, label, bytes);
}

/// A command as JSON (`lanes::apply` lists them; also `inspector`, `range`,
/// `fire`, `mode`).
#[wasm_bindgen]
pub fn stage(json: String) {
    crate::hand_command(json);
}

fn publish(st: Res<Status>, range: Res<Range>) {
    bridge::set("frames", st.frames as f64);
    bridge::set("backend", pk_shell::backend());
    if st.frames > 2 {
        bridge::set("state", "running");
        bridge::set("ready", true);
    }
    bridge::set("telemetry", st.telemetry.as_str());
    bridge::set("audio", st.audio.as_str());
    bridge::set("level", st.level.as_str());
    bridge::set("hits", range.hits as f64);
}
