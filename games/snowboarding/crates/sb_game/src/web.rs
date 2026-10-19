//! Web glue: the page (`web/index.html`) picks WebGPU or WebGL2, loads
//! this module and calls [`run`]. Status goes out on `window.__sb` each
//! frame; [`gesture`] is called inside the page's input handlers, where
//! the browser lets sound start; [`screenshot`] saves the next frame as a
//! download (a test hook: headless Chrome does not composite a WebGPU
//! canvas into its own screenshots).

use crate::options::Options;
use crate::status::Status;
use bevy::prelude::*;
use js_sys::{Object, Reflect};
use std::sync::Mutex;
use wasm_bindgen::prelude::*;

static SCREENSHOT: Mutex<Option<String>> = Mutex::new(None);

#[wasm_bindgen]
pub fn run() {
    let window = web_sys::window().expect("a window");
    let search = window.location().search().unwrap_or_default();
    let o = Options::from_query(&search);
    let mut app = crate::app(o);
    app.add_systems(Last, (publish, take_screenshot));
    app.run();
}

#[cfg(sb_webgl2)]
const BACKEND: &str = "webgl2";
#[cfg(not(sb_webgl2))]
const BACKEND: &str = "webgpu";

#[wasm_bindgen]
pub fn backend() -> String {
    BACKEND.into()
}

/// Called by the page inside a user gesture.
#[wasm_bindgen]
pub fn gesture() {
    crate::audio::gesture();
}

/// The sound's state, as JSON (a test hook: `window.__sb.audio()`).
#[wasm_bindgen]
pub fn audio_status() -> String {
    crate::audio::status()
}

#[wasm_bindgen]
pub fn screenshot(name: String) {
    *SCREENSHOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(name);
}

fn take_screenshot(mut commands: Commands) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let Some(name) = SCREENSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let done = name.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(name))
        .observe(move |_: On<ScreenshotCaptured>| {
            if let Some(w) = web_sys::window()
                && let Ok(sb) = Reflect::get(&w, &JsValue::from_str("__sb"))
                && let Some(sb) = sb.dyn_ref::<Object>()
            {
                set(sb, "shot", done.as_str());
            }
        });
}

fn set(o: &Object, k: &str, v: impl Into<JsValue>) {
    let _ = Reflect::set(o, &JsValue::from_str(k), &v.into());
}

fn publish(st: Res<Status>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(sb) = Reflect::get(&window, &JsValue::from_str("__sb")) else {
        return;
    };
    let Some(sb) = sb.dyn_ref::<Object>() else {
        return;
    };
    set(sb, "frames", st.frames as f64);
    set(sb, "backend", BACKEND);
    set(sb, "screen", st.screen);
    set(sb, "state", st.state);
    set(sb, "course", st.course);
    set(sb, "mode", st.mode);
    set(sb, "time", st.time);
    set(sb, "s", st.s);
    set(sb, "speed", st.speed);
    set(sb, "phase", st.phase);
    set(sb, "score", st.score);
    set(sb, "tricks", st.tricks as f64);
    set(sb, "crashes", st.crashes);
    set(sb, "place", st.place as f64);
    set(
        sb,
        "finish",
        st.finish
            .map_or(JsValue::NULL, |f| JsValue::from_f64(f as f64)),
    );
    set(sb, "lastTrick", st.last_trick.as_str());
}
