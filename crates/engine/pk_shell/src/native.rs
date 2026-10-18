//! Native glue (Midnight Playground SPEC 8.4 and 8.5; its `native.rs` and
//! `native/window_state.rs`): the player's window as an Electron shell kept
//! it (DECISIONS D1000: it opens where it was closed, at that size,
//! maximised or fullscreen as it was, never smaller than a minimum; F11
//! toggles fullscreen, Ctrl+Q or Cmd+Q quits; the icon), `FocusLost` when
//! the window loses the focus (D1002), the window title as a status line,
//! the log's tally of warnings and errors, and the `--smoke-test` and
//! `--screenshot` modes (D1003).
//!
//! The window's state is `window-state.json` beside the settings store,
//! `{x, y, width, height, fullscreen, maximized}` as the JS shell wrote
//! it, saved on close with the last normal bounds when the window is
//! maximised or fullscreen. A run that makes pictures or checks the build
//! keeps its own window and neither reads nor writes it
//! ([`WindowStatePlugin::persist`]).

use crate::status::{LoadState, LoadStatus};
use crate::{FocusGained, FocusLost};
use bevy::app::AppExit;
use bevy::ecs::system::NonSendMarker;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::window::{
    Monitor, MonitorSelection, PrimaryWindow, WindowCloseRequested, WindowCreated, WindowFocused,
    WindowMode, WindowMoved, WindowPosition, WindowResizeConstraints, WindowResized,
    WindowResolution,
};
use bevy::winit::WINIT_WINDOWS;
use serde_json::Value;
use std::path::PathBuf;

/// The player's window.
#[derive(Resource, Clone, Debug)]
pub struct WindowConfig {
    /// The X11 class and Wayland app id: the desktop entry's
    /// `StartupWMClass`, so the desktop claims the window with the entry's
    /// icon.
    pub app_id: &'static str,
    /// The window's title: the game's name.
    pub title: &'static str,
    /// The icon's PNG, built in (256 × 256 or so; it is resized to 64 px).
    /// On Wayland the window takes its icon from the desktop entry its app
    /// id names instead.
    pub icon_png: Option<&'static [u8]>,
    /// The size the first time (`st.width || 1600`).
    pub default_size: (u32, u32),
    /// `minWidth`, `minHeight`.
    pub min_size: (f32, f32),
    /// `window-state.json`; none keeps no state.
    pub state_path: Option<PathBuf>,
}

/// What `window-state.json` holds. The bounds are the window's pixels (the
/// client draws at a scale factor of 1 natively): `x`, `y` the outer
/// position (none where the platform does not tell it, as on Wayland),
/// `width`, `height` the inner size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub maximized: bool,
    pub fullscreen: bool,
}

impl WindowState {
    /// `JSON.parse` of the file, `{}` when it is missing or broken
    /// (`loadState`'s `catch`). A zero or missing size is the default
    /// (`st.width || 1600`).
    pub fn parse(text: &str) -> WindowState {
        let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
            return WindowState::default();
        };
        let num = |k: &str| m.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
        let int = |k: &str| num(k).map(|v| v.round() as i32);
        let size = |k: &str| {
            num(k)
                .filter(|v| *v >= 1.0 && *v < 65536.0)
                .map(|v| v.round() as u32)
        };
        let flag = |k: &str| m.get(k).and_then(Value::as_bool).unwrap_or(false);
        WindowState {
            x: int("x"),
            y: int("y"),
            width: size("width"),
            height: size("height"),
            maximized: flag("maximized"),
            fullscreen: flag("fullscreen"),
        }
    }

    /// `JSON.stringify({ ...bounds, fullscreen, maximized })`.
    pub fn to_json(&self) -> String {
        let mut m = serde_json::Map::new();
        if let (Some(x), Some(y)) = (self.x, self.y) {
            m.insert("x".into(), x.into());
            m.insert("y".into(), y.into());
        }
        if let (Some(w), Some(h)) = (self.width, self.height) {
            m.insert("width".into(), w.into());
            m.insert("height".into(), h.into());
        }
        m.insert("fullscreen".into(), self.fullscreen.into());
        m.insert("maximized".into(), self.maximized.into());
        Value::Object(m).to_string()
    }
}

impl WindowConfig {
    fn load(&self) -> WindowState {
        self.state_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(WindowState::default, |t| WindowState::parse(&t))
    }

    fn save(&self, st: &WindowState) {
        // "not worth failing over"
        let Some(p) = &self.state_path else { return };
        let r = p
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(p, st.to_json()));
        match r {
            Ok(()) => info!("window state: {} {}", p.display(), st.to_json()),
            Err(e) => warn!("window state: {}: {e}", p.display()),
        }
    }

    /// The player's window at its saved bounds (its title is the game's
    /// name; the status line may replace it, [`StatusTitle`]).
    pub fn window(&self) -> Window {
        let st = self.load();
        let w = st.width.unwrap_or(self.default_size.0);
        let h = st.height.unwrap_or(self.default_size.1);
        Window {
            title: self.title.into(),
            name: Some(self.app_id.into()),
            resolution: WindowResolution::new(w, h).with_scale_factor_override(1.0),
            position: match (st.x, st.y) {
                (Some(x), Some(y)) => WindowPosition::At(IVec2::new(x, y)),
                _ => WindowPosition::Automatic,
            },
            resize_constraints: WindowResizeConstraints {
                min_width: self.min_size.0,
                min_height: self.min_size.1,
                ..default()
            },
            ..default()
        }
    }

    /// A run's own window (a picture, a size asked for): `size` pixels at a
    /// scale factor of 1, shown or not (a screenshot run draws without
    /// showing a window, where the platform allows it).
    pub fn plain(&self, title: String, size: (u32, u32), visible: bool) -> Window {
        Window {
            title,
            name: Some(self.app_id.into()),
            resolution: WindowResolution::new(size.0, size.1).with_scale_factor_override(1.0),
            visible,
            ..default()
        }
    }
}

/// The state being kept: what was loaded (maximised, fullscreen to apply
/// once the window exists), and the last bounds seen while the window was
/// neither (`win._lastBounds`).
#[derive(Resource, Default)]
struct Kept {
    loaded: WindowState,
    last: Option<WindowState>,
    saved: bool,
}

/// The icon, once the window is made (every interactive window).
fn icon(
    mut created: MessageReader<WindowCreated>,
    cfg: Res<WindowConfig>,
    _main_thread: NonSendMarker,
) {
    if created.read().count() > 0
        && let Some(png) = cfg.icon_png
    {
        set_icon(png);
    }
}

/// The window was made: `if (st.maximized) win.maximize(); if
/// (st.fullscreen) win.setFullScreen(true)`.
fn created(
    mut created: MessageReader<WindowCreated>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    monitors: Query<(Entity, &Monitor)>,
    cfg: Res<WindowConfig>,
    mut kept: ResMut<Kept>,
    _main_thread: NonSendMarker,
) {
    if created.read().count() == 0 {
        return;
    }
    let Ok(mut w) = windows.single_mut() else {
        return;
    };
    // The bounds asked for, until the window reports its own.
    let st = kept.loaded;
    kept.last = Some(WindowState {
        width: Some(st.width.unwrap_or(cfg.default_size.0)),
        height: Some(st.height.unwrap_or(cfg.default_size.1)),
        maximized: false,
        fullscreen: false,
        ..st
    });
    if st.maximized {
        w.set_maximized(true);
    }
    if st.fullscreen {
        let at = st.x.zip(st.y).map(|(x, y)| IVec2::new(x, y));
        w.mode = WindowMode::BorderlessFullscreen(monitor_at(&monitors, at));
    }
}

/// The monitor a point of the desktop is on, else the one winit guesses
/// (`Current`, from where the window was first placed).
fn monitor_at(monitors: &Query<(Entity, &Monitor)>, p: Option<IVec2>) -> MonitorSelection {
    p.and_then(|p| {
        monitors.iter().find(|(_, m)| {
            let d = p - m.physical_position;
            d.x >= 0
                && d.y >= 0
                && (d.x as u32) < m.physical_width
                && (d.y as u32) < m.physical_height
        })
    })
    .map_or(MonitorSelection::Current, |(e, _)| {
        MonitorSelection::Entity(e)
    })
}

const ICON_SIZE: u32 = 64;

/// The icon from its PNG, on every window.
fn set_icon(png: &[u8]) {
    let icon = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())
        .and_then(|img| {
            // 64 × 64: X11 got an empty `_NET_WM_ICON` from a 256 × 256
            // one (a quarter of a megabyte in one property request), and
            // the desktops draw window icons at 48 px or less.
            let img = image::imageops::resize(
                &img.to_rgba8(),
                ICON_SIZE,
                ICON_SIZE,
                image::imageops::FilterType::Triangle,
            );
            let (w, h) = img.dimensions();
            winit::window::Icon::from_rgba(img.into_raw(), w, h).map_err(|e| e.to_string())
        });
    match icon {
        Ok(icon) => WINIT_WINDOWS.with_borrow(|ww| {
            for w in ww.windows.values() {
                w.set_window_icon(Some(icon.clone()));
            }
        }),
        Err(e) => warn!("window icon: {e}"),
    }
}

/// The window as it is now (`getBounds`, `isMaximized`, `isFullScreen`).
fn current(entity: Entity) -> Option<WindowState> {
    WINIT_WINDOWS.with_borrow(|ww| {
        let w = ww.get_window(entity)?;
        let pos = w.outer_position().ok();
        let size = w.inner_size();
        // Where the platform tells the position (X11), a fullscreen window
        // is one over the whole of a monitor (any monitor: `current_monitor`
        // is a cached guess): winit's `fullscreen()` is the last it asked
        // for, and misses the window manager's own changes (its key,
        // `wmctrl`) both ways. Elsewhere (Wayland) it is what the
        // compositor said.
        let covers = w
            .available_monitors()
            .any(|m| m.size() == size && pos == Some(m.position()));
        Some(WindowState {
            x: pos.map(|p| p.x),
            y: pos.map(|p| p.y),
            width: Some(size.width).filter(|v| *v > 0),
            height: Some(size.height).filter(|v| *v > 0),
            maximized: w.is_maximized(),
            fullscreen: if pos.is_some() {
                covers
            } else {
                w.fullscreen().is_some()
            },
        })
    })
}

/// `win.on('resize' | 'move', …)`: the bounds while the window is neither
/// maximised nor fullscreen.
fn track(
    mut resized: MessageReader<WindowResized>,
    mut moved: MessageReader<WindowMoved>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut kept: ResMut<Kept>,
    _main_thread: NonSendMarker,
) {
    let changed = resized.read().count() + moved.read().count() > 0;
    let Ok(e) = windows.single() else { return };
    if !changed {
        return;
    }
    if let Some(now) = current(e)
        && !now.maximized
        && !now.fullscreen
    {
        let last = kept.last.get_or_insert(now);
        // Wayland does not tell the position; keep the one loaded.
        if now.x.is_some() {
            (last.x, last.y) = (now.x, now.y);
        }
        (last.width, last.height) = (now.width, now.height);
    }
}

/// `saveState(win)` once, before the window goes: the last normal bounds
/// if it is maximised or fullscreen, else the bounds it has.
fn save_now(e: Entity, cfg: &WindowConfig, kept: &mut Kept) {
    if kept.saved {
        return;
    }
    let Some(now) = current(e) else { return };
    kept.saved = true;
    let b = if now.maximized || now.fullscreen {
        kept.last.unwrap_or(now)
    } else {
        now
    };
    cfg.save(&WindowState {
        x: b.x.or(kept.loaded.x),
        y: b.y.or(kept.loaded.y),
        width: b.width,
        height: b.height,
        maximized: now.maximized,
        fullscreen: now.fullscreen,
    });
}

/// `win.on('close', () => saveState(win))`, before Bevy closes it.
fn on_close(
    mut close: MessageReader<WindowCloseRequested>,
    windows: Query<Entity, With<PrimaryWindow>>,
    cfg: Res<WindowConfig>,
    mut kept: ResMut<Kept>,
    _main_thread: NonSendMarker,
) {
    for c in close.read() {
        if windows.contains(c.window) {
            save_now(c.window, &cfg, &mut kept);
        }
    }
}

/// Any other way out (Ctrl+Q, the app exiting) saves too, while the window
/// is still there.
fn on_exit(
    mut exit: MessageReader<AppExit>,
    windows: Query<Entity, With<PrimaryWindow>>,
    cfg: Res<WindowConfig>,
    mut kept: ResMut<Kept>,
    _main_thread: NonSendMarker,
) {
    if exit.read().count() > 0
        && let Ok(e) = windows.single()
    {
        save_now(e, &cfg, &mut kept);
    }
}

/// The shell's keys (`before-input-event`): F11 toggles fullscreen, Ctrl+Q
/// or Cmd+Q quits. Neither should be a key the game binds (a Ctrl+Q
/// leaves the game before any binding sees a Q).
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut events: MessageReader<KeyboardInput>,
    mut focus: MessageReader<WindowFocused>,
    mut modifier: Local<bool>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    monitors: Query<(Entity, &Monitor)>,
    mut exit: MessageWriter<AppExit>,
    _main_thread: NonSendMarker,
) {
    if keys.just_pressed(KeyCode::F11)
        && let Ok((e, mut w)) = windows.single_mut()
    {
        // `win.setFullScreen(!win.isFullScreen())`, on the monitor the
        // window's middle is on.
        let now = current(e);
        let full = now.map_or(w.mode != WindowMode::Windowed, |n| n.fullscreen);
        let middle = now.and_then(|n| {
            let (x, y) = n.x.zip(n.y)?;
            let (w, h) = n.width.zip(n.height)?;
            Some(IVec2::new(x + w as i32 / 2, y + h as i32 / 2))
        });
        info!("F11: fullscreen {}", if full { "off" } else { "on" });
        let mode = if full {
            WindowMode::Windowed
        } else {
            WindowMode::BorderlessFullscreen(monitor_at(&monitors, middle))
        };
        if (w.mode == WindowMode::Windowed) == (mode == WindowMode::Windowed) {
            // Bevy has it so already (the window manager changed it behind
            // its back): tell winit straight, as Bevy would.
            let target = (!full).then_some(winit::window::Fullscreen::Borderless(None));
            WINIT_WINDOWS.with_borrow(|ww| {
                if let Some(win) = ww.get_window(e) {
                    win.set_fullscreen(target);
                }
            });
        } else {
            w.mode = mode;
        }
    }
    // In the order the keys came: a frame can hold the Ctrl's press and
    // release both (a slow frame, a quick chord).
    for k in events.read() {
        let down = k.state == ButtonState::Pressed;
        match k.key_code {
            KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight => {
                *modifier = down;
            }
            KeyCode::KeyQ if down && *modifier => {
                info!("Ctrl+Q: quit");
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
    if focus.read().filter(|f| !f.focused).count() > 0 {
        // Its release goes to the other window.
        *modifier = false;
    }
}

/// The shell's keys and icon for an interactive window (`keys`); the saved
/// state for the player's window (`persist`).
pub struct WindowStatePlugin {
    pub config: WindowConfig,
    /// F11, Ctrl+Q and the icon: every window a player sits at.
    pub keys: bool,
    /// Read the state at start and save it on close: the player's own
    /// window, not a run that makes pictures or was given a size.
    pub persist: bool,
}

impl Plugin for WindowStatePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());
        if self.keys {
            app.add_systems(PreUpdate, (icon, keys));
        }
        if !self.persist {
            return;
        }
        app.insert_resource(Kept {
            loaded: self.config.load(),
            ..default()
        })
        .add_systems(PreUpdate, (created, track, on_close).chain())
        .add_systems(Last, on_exit);
    }
}

/// [`FocusLost`] and [`FocusGained`] from the window's focus.
pub fn focus_messages(
    mut focus: MessageReader<WindowFocused>,
    mut lost: MessageWriter<FocusLost>,
    mut gained: MessageWriter<FocusGained>,
) {
    for f in focus.read() {
        if f.focused {
            gained.write(FocusGained);
        } else {
            lost.write(FocusLost);
        }
    }
}

/// What the log said at warning and error level, for the smoke test's
/// report (an Electron smoke test lists the page's console warnings and
/// errors and fails on an error). A tracing layer beside Bevy's, under the
/// same filter (`wgpu=error,naga=warn`, `RUST_LOG`), which also hands the
/// recorder the lines it keeps (`crate::recorder::wants_log`).
pub mod log_tally {
    use bevy::app::App;
    use bevy::log::BoxedLayer;
    use bevy::log::tracing::field::{Field, Visit};
    use bevy::log::tracing::{Event, Level};
    use bevy::log::tracing_subscriber::Layer;
    use bevy::log::tracing_subscriber::layer::Context;
    use bevy::log::tracing_subscriber::registry::Registry;
    use std::sync::{Mutex, RwLock};

    /// Errors, warnings, and the first [`KEEP`] messages of either.
    pub static TALLY: Mutex<(usize, usize, Vec<String>)> = Mutex::new((0, 0, Vec::new()));
    pub const KEEP: usize = 20;

    struct Tally;

    /// The message and its fields; a `log` crate record's own target in
    /// place of `log` (the bridge's `log.target`, its other `log.*` fields
    /// left out).
    #[derive(Default)]
    struct Message(String, Option<String>);

    impl Visit for Message {
        fn record_str(&mut self, field: &Field, value: &str) {
            if field.name() == "log.target" {
                self.1 = Some(value.to_owned());
            } else {
                self.record_debug(field, &value);
            }
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            use std::fmt::Write;
            match field.name() {
                "message" => {
                    let _ = write!(self.0, "{value:?}");
                }
                n if n.starts_with("log.") => {}
                n => {
                    let _ = write!(self.0, " {n}={value:?}");
                }
            }
        }
    }

    /// Errors that say nothing about the build, counted as warnings (as an
    /// Electron shell filters the favicon's 404): `(target prefix, message
    /// part)`. A game adds its own ([`benign`]); the sound's output stream
    /// running dry while a software renderer holds the frame up is here
    /// for every game on web-audio-api.
    const DEFAULT_BENIGN: &[(&str, &str)] = &[("web_audio_api::io", "buffer underrun or overrun")];
    static BENIGN: RwLock<Vec<(&'static str, &'static str)>> = RwLock::new(Vec::new());

    /// Adds errors to count as warnings.
    pub fn benign(list: &[(&'static str, &'static str)]) {
        BENIGN
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(list);
    }

    /// Counts and keeps one record.
    pub fn note(level: Level, target: &str, message: &str) {
        let is_benign = |(t, m): &(&str, &str)| target.starts_with(t) && message.contains(m);
        let error = level == Level::ERROR
            && !DEFAULT_BENIGN.iter().any(is_benign)
            && !BENIGN
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .any(is_benign);
        if !error && level > Level::WARN {
            return;
        }
        let mut t = TALLY.lock().unwrap_or_else(|e| e.into_inner());
        if error {
            t.0 += 1;
        } else {
            t.1 += 1;
        }
        if t.2.len() < KEEP {
            let kind = if error { "error" } else { "warn" };
            t.2.push(format!("[{kind}] {target}: {}", message.trim_end()));
        }
    }

    impl Layer<Registry> for Tally {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, Registry>) {
            let meta = event.metadata();
            // The run recording keeps the game's own info lines too.
            let keep = crate::recorder::wants_log(*meta.level(), meta.target());
            if *meta.level() > Level::WARN && !keep {
                return;
            }
            let mut m = Message::default();
            event.record(&mut m);
            let target = m.1.as_deref().unwrap_or(meta.target());
            if keep {
                crate::recorder::log(*meta.level(), target, &m.0);
            }
            if *meta.level() <= Level::WARN {
                note(*meta.level(), target, &m.0);
            }
        }
    }

    /// Takes the tally: `(errors, warnings, messages)`.
    pub fn take() -> (usize, usize, Vec<String>) {
        std::mem::take(&mut *TALLY.lock().unwrap_or_else(|e| e.into_inner()))
    }

    #[cfg(test)]
    #[test]
    fn errors_fail_and_the_benign_ones_do_not() {
        use bevy::log::tracing_subscriber::layer::SubscriberExt;
        let sub = bevy::log::tracing_subscriber::registry().with(Tally);
        bevy::log::tracing::subscriber::with_default(sub, || {
            bevy::log::info!("not counted");
            bevy::log::warn!("a warning");
            bevy::log::error!(target: "web_audio_api::io::cpal", "an error occurred on the output audio stream: A buffer underrun or overrun occurred.");
            bevy::log::error!(n = 3, "a real one");
        });
        let t = take();
        assert_eq!((t.0, t.1), (1, 2), "{:?}", t.2);
        assert!(
            t.2[2].starts_with("[error] pk_shell::native::log_tally: a real one n=3"),
            "{:?}",
            t.2
        );
    }

    /// `LogPlugin::custom_layer`.
    pub fn layer(_: &mut App) -> Option<BoxedLayer> {
        Some(Box::new(Tally))
    }
}

/// `--smoke-test` gives up when the scene is not up in this long (an
/// Electron shell's waits 60 s for `__ready`; a software renderer in CI
/// compiles the pipelines far slower than a GPU).
pub const SMOKE_TIMEOUT_S: f64 = 600.0;

/// `--screenshot` and `--smoke-test`: once the scene has drawn `after`
/// frames past ready, capture the window to `screenshot` (or just report)
/// and exit. The smoke test prints what it saw (the load time, the
/// adapter, the game's `extra` line, the log's warnings and errors) and
/// fails on a logged error, a failed load, or no scene after `timeout_s`.
#[derive(Resource)]
pub struct SmokeTest {
    /// Named in the report (the scene).
    pub label: String,
    /// Frames to render after ready before the capture or the report.
    pub after: u32,
    pub timeout_s: f64,
    /// `--screenshot <png>`.
    pub screenshot: Option<String>,
    /// `--smoke-test`.
    pub smoke: bool,
    /// What else the report names (the scene's counts).
    pub extra: Option<fn(&World) -> String>,
}

impl SmokeTest {
    /// Whether this run finishes by itself.
    pub fn active(&self) -> bool {
        self.smoke || self.screenshot.is_some()
    }
}

/// Adds [`SmokeTest`] and its `finish` system.
pub struct SmokeTestPlugin(pub SmokeTest);

impl Plugin for SmokeTestPlugin {
    fn build(&self, app: &mut App) {
        if !self.0.active() {
            return;
        }
        app.insert_resource(SmokeTest {
            label: self.0.label.clone(),
            after: self.0.after,
            timeout_s: self.0.timeout_s,
            screenshot: self.0.screenshot.clone(),
            smoke: self.0.smoke,
            extra: self.0.extra,
        })
        .add_systems(Update, finish);
    }
}

fn finish(world: &mut World, mut ready_at: Local<Option<f64>>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let status = world.resource::<LoadStatus>().clone();
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let test = world.resource::<SmokeTest>();
    let (smoke, after, timeout, shot, label, extra) = (
        test.smoke,
        test.after,
        test.timeout_s,
        test.screenshot.clone(),
        test.label.clone(),
        test.extra,
    );
    if status.state == LoadState::Failed {
        *done = true;
        if smoke {
            println!(
                "smoke test: failed: {}",
                status.error.as_deref().unwrap_or("?")
            );
        }
        world.write_message(AppExit::error());
        return;
    }
    if status.ready && ready_at.is_none() {
        *ready_at = Some(now);
    }
    if smoke && !status.ready && now > timeout {
        *done = true;
        println!(
            "smoke test: failed: not ready after {timeout:.0} s ({} {:.0} %)",
            status.state.name(),
            status.progress * 100.0
        );
        world.write_message(AppExit::error());
        return;
    }
    if status.ready_frames < u64::from(after.max(1)) {
        return;
    }
    if let Some(path) = shot {
        *done = true;
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path))
            .observe(
                |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                },
            );
    } else if smoke {
        *done = true;
        let (errors, warnings, messages) = log_tally::take();
        let ok = errors == 0;
        let adapter = world.get_resource::<RenderAdapterInfo>().map_or_else(
            || "?".to_string(),
            |a| format!("{} ({:?}, {:?})", a.name, a.backend, a.device_type),
        );
        let extra = extra.map(|f| f(world)).unwrap_or_default();
        println!(
            "smoke test: {}: {label}, ready in {:.1} s, {} frames after; adapter {adapter}; \
             {errors} errors, {warnings} warnings; {extra}",
            if ok { "ok" } else { "failed" },
            ready_at.unwrap_or(now),
            status.ready_frames,
        );
        for m in &messages {
            println!("  {m}");
        }
        world.write_message(if ok {
            AppExit::Success
        } else {
            AppExit::error()
        });
    }
}

/// The window title as a status line, twice a second: the game formats it
/// from the world (its name, scene, position, the frame rate and the worst
/// frame, which the title resets). Insert it and add [`title`] to `Update`.
#[derive(Resource)]
pub struct StatusTitle(pub TitleFn);

/// What formats the title line.
pub type TitleFn = Box<dyn Fn(&World, &LoadStatus) -> String + Send + Sync>;

/// The window title as a status line, twice a second.
pub fn title(world: &mut World, mut last: Local<f32>) {
    let now = world.resource::<Time>().elapsed_secs();
    if now - *last < 0.5 {
        return;
    }
    *last = now;
    let Some(title) = world.remove_resource::<StatusTitle>() else {
        return;
    };
    let status = world.resource::<LoadStatus>().clone();
    let text = (title.0)(world, &status);
    world.insert_resource(title);
    let mut q = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    if let Ok(mut w) = q.single_mut(world) {
        w.title = text;
    }
    world.resource_mut::<LoadStatus>().worst_ms = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_the_electron_file() {
        // As `saveState` writes it.
        let js =
            r#"{"x":120,"y":64,"width":1700,"height":950,"fullscreen":false,"maximized":true}"#;
        let st = WindowState::parse(js);
        assert_eq!(
            st,
            WindowState {
                x: Some(120),
                y: Some(64),
                width: Some(1700),
                height: Some(950),
                maximized: true,
                fullscreen: false,
            }
        );
        assert_eq!(st.to_json(), js);
    }

    #[test]
    fn missing_broken_and_zero_are_the_defaults() {
        assert_eq!(WindowState::parse(""), WindowState::default());
        assert_eq!(WindowState::parse("[1,2]"), WindowState::default());
        let st = WindowState::parse(r#"{"width":0,"height":"900","fullscreen":1}"#);
        assert_eq!((st.width, st.height, st.fullscreen), (None, None, false));
        // No position (Wayland): none written.
        let st = WindowState {
            width: Some(1600),
            height: Some(900),
            ..default()
        };
        assert_eq!(
            st.to_json(),
            r#"{"width":1600,"height":900,"fullscreen":false,"maximized":false}"#
        );
    }

    #[test]
    fn windows_from_the_config() {
        let cfg = WindowConfig {
            app_id: "a-game",
            title: "A Game",
            icon_png: None,
            default_size: (1600, 900),
            min_size: (800.0, 450.0),
            state_path: Some(PathBuf::from("/nonexistent/window-state.json")),
        };
        let w = cfg.window();
        assert_eq!((w.width(), w.height()), (1600.0, 900.0));
        assert_eq!(w.title, "A Game");
        assert_eq!(w.name.as_deref(), Some("a-game"));
        let w = cfg.plain("t".into(), (512, 512), false);
        assert_eq!((w.width(), w.height(), w.visible), (512.0, 512.0, false));
    }
}
