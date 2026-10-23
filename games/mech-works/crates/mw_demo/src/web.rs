//! Web glue: the page (`web/index.html`) picks WebGPU or WebGL2, loads
//! this module and calls [`run`]. It hands in policies ([`load_policy`])
//! and commands ([`stage`]); the client publishes its state on the page's
//! status global (`window.__mech`) each frame: the loader's contract
//! (`state`, `frames`, `ready`) and the selected lane's `telemetry` (JSON).

use bevy::prelude::*;
use pk_shell::{Inbox, bridge};
use wasm_bindgen::prelude::*;

use crate::impact::Range;
use crate::view::Inspector;
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
    app.add_systems(PreUpdate, page_commands)
        .add_systems(Last, (publish, take_screenshot));
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

/// Commands the lanes do not handle, peeled off before they see them.
fn page_commands(
    mut inspector: ResMut<Inspector>,
    mut range: ResMut<Range>,
    mut mode: ResMut<Mode>,
) {
    let Some(list) = crate::COMMANDS.take() else {
        return;
    };
    let mut rest = Vec::new();
    for line in list {
        let v: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        match v.get("cmd").and_then(|c| c.as_str()) {
            Some("inspector") => {
                inspector.0 = v
                    .get("on")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(!inspector.0)
            }
            Some("range") | Some("fire") => range.apply(&v),
            Some("mode") => {
                *mode = Mode::parse(v.get("mode").and_then(|m| m.as_str()).unwrap_or(""))
            }
            _ => rest.push(line),
        }
    }
    if !rest.is_empty() {
        crate::COMMANDS.put(rest);
    }
}

fn publish(st: Res<Status>, range: Res<Range>) {
    bridge::set("frames", st.frames as f64);
    bridge::set("backend", pk_shell::backend());
    if st.frames > 2 {
        bridge::set("state", "running");
        bridge::set("ready", true);
    }
    bridge::set("telemetry", st.telemetry.as_str());
    bridge::set("hits", range.hits as f64);
}
