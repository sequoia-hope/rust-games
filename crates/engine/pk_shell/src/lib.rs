//! The Bevy client shell of Playground Kit (PLAN.md section 4, layer 1;
//! DECISIONS D8): what every game's client has that a second game would
//! otherwise copy, carved from Midnight Playground's `mp_game` with the
//! game's names replaced by configuration and small traits.
//!
//! - [`plugins`]: `DefaultPlugins` without Bevy's 2D sprites.
//! - [`params`]: the query string (web) or command line (native), ordered.
//! - [`status`]: the load state, readiness (pipelines, GPU fence), late
//!   frames; what the page and the window title show.
//! - [`store`]: settings and records, JS-exact, over `localStorage` or a
//!   file.
//! - [`warmup`]: every pipeline a scene needs, compiled behind the loading
//!   screen.
//! - [`debug`]: the frame-time overlay.
//! - [`recorder`]: the JSON-lines session log and the input log.
//! - [`ui`]: a UI node's box in CSS pixels, clipped to what shows.
//! - [`native`] (not wasm): the window's saved state, F11, Ctrl+Q, the
//!   icon, the log tally, `--smoke-test` and `--screenshot`, the title.
//! - [`stations`]: screenshot stations, flown and saved (a native run).
//! - [`bridge`], [`web`], [`worker`] (wasm): the status global, the page
//!   glue (gestures, pixel ratio, split-screen camera, stage commands)
//!   and the Web Worker executor.
//!
//! The web build comes in two backends, WebGPU and WebGL2, one wasm file
//! each; the `webgpu` and `webgl2` cargo features name which this is, and
//! a game's wasm target tables turn one on.

pub mod debug;
pub mod params;
pub mod plugins;
pub mod recorder;
pub mod stations;
pub mod status;
pub mod store;
pub mod ui;
pub mod warmup;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod bridge;
#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(target_arch = "wasm32")]
pub mod worker;

use bevy::prelude::*;
use std::sync::{Mutex, MutexGuard};

/// The player left: the window lost the focus natively, the page went
/// hidden on the web (`visibilitychange`; a hidden page gets no frames, so
/// the event is latched and the message comes in the first frame back).
/// Written in `PreUpdate`; a game pauses its run on it before the frame's
/// ticks (Midnight Playground SPEC 8.4, DECISIONS D1002).
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct FocusLost;

/// The player is back: the window has the focus again, the page is shown.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct FocusGained;

/// [`FocusLost`] and [`FocusGained`] from the platform.
pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FocusLost>().add_message::<FocusGained>();
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(PreUpdate, native::focus_messages);
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, web::install_visibility)
            .add_systems(PreUpdate, web::focus_messages);
    }
}

/// What the game calls itself: the window's title and the X11 class or
/// Wayland app id (the desktop entry's `StartupWMClass`), and the line
/// both builds print so the native and web pipelines are seen to run the
/// same crate.
#[derive(Resource, Clone, Debug)]
pub struct AppInfo {
    pub name: &'static str,
    pub app_id: &'static str,
    pub banner: String,
}

/// A mailbox: a static `Mutex<Option<T>>` for what arrives from outside
/// the app (a file read by a thread, bytes handed in by the page, a
/// command from a test hook), taken by a system in the next frame.
///
/// ```
/// use pk_shell::Inbox;
/// static SCENE: Inbox<Vec<u8>> = Inbox::new();
/// SCENE.put(vec![1, 2, 3]);
/// assert_eq!(SCENE.take(), Some(vec![1, 2, 3]));
/// assert_eq!(SCENE.take(), None);
/// ```
pub struct Inbox<T>(Mutex<Option<T>>);

impl<T> Inbox<T> {
    pub const fn new() -> Inbox<T> {
        Inbox(Mutex::new(None))
    }

    /// Puts a value in, replacing one not yet taken.
    pub fn put(&self, v: T) {
        *self.lock() = Some(v);
    }

    /// Takes the value out, if any.
    pub fn take(&self) -> Option<T> {
        self.lock().take()
    }

    /// Whether something waits.
    pub fn is_some(&self) -> bool {
        self.lock().is_some()
    }

    /// The slot itself (a poisoned lock is still a lock).
    pub fn lock(&self) -> MutexGuard<'_, Option<T>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl<T> Default for Inbox<T> {
    fn default() -> Self {
        Inbox::new()
    }
}

/// The backend this build draws with, as the status contract names it:
/// `webgpu` or `webgl2` on the web (the cargo features), `native` elsewhere.
pub const fn backend() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        if cfg!(feature = "webgl2") {
            "webgl2"
        } else {
            "webgpu"
        }
    } else {
        "native"
    }
}

/// The platform, as the recorder's `start` line names it: `web-webgpu`,
/// `web-webgl2`, or the operating system.
pub fn platform() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        if cfg!(feature = "webgl2") {
            "web-webgl2"
        } else {
            "web-webgpu"
        }
    } else {
        std::env::consts::OS
    }
}
