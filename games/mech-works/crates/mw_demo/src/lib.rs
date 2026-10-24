//! The Mech Works client: the tech demos and the training viewer. Each
//! *lane* is one mech, an [`Env`] of the same simulation the trainer steps,
//! driven live by a policy loaded from its weight blob. Nothing about how it
//! walks is animated: the picture is the physics.
//!
//! The page (`web/index.html`) owns the controls, the curves and the
//! inspector's bars; it hands the client policies and commands
//! ([`Inbox`]es, filled by `web.rs` natively by `main.rs`) and reads back
//! the selected lane's telemetry each tenth of a second.

pub mod camera;
pub mod impact;
pub mod lanes;
pub mod scene;
pub mod view;

#[cfg(target_arch = "wasm32")]
pub mod web;

use bevy::light::DirectionalLightShadowMap;
use bevy::prelude::*;
use pk_shell::Inbox;
use pk_shell::plugins::ClientPlugins;

/// The policies the page or the command line hands in: (lane, label, blob).
pub static POLICIES: Inbox<Vec<(usize, String, Vec<u8>)>> = Inbox::new();
/// Commands for the client, as JSON lines (the page's `stage`).
pub static COMMANDS: Inbox<Vec<String>> = Inbox::new();

/// Queues a policy for a lane.
pub fn hand_policy(lane: usize, label: String, bytes: Vec<u8>) {
    let mut v = POLICIES.take().unwrap_or_default();
    v.push((lane, label, bytes));
    POLICIES.put(v);
}

/// Queues a command (JSON: `{"cmd": "...", ...}`; `lanes::apply` lists them).
pub fn hand_command(json: String) {
    let mut v = COMMANDS.take().unwrap_or_default();
    v.push(json);
    COMMANDS.put(v);
}

/// Which demo the page runs.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Checkpoints side by side: the training viewer.
    #[default]
    Viewer,
    /// Demo 1: projectiles hit the mech.
    Impact,
    /// Demo 2: endless dirt path with boulders, branches, and car wrecks (M5).
    Path,
    /// Demo 3: damage an actuator, watch the limp.
    Damage,
    /// Demo 4: swap a part, watch it calibrate.
    Calibration,
}

impl Mode {
    pub fn parse(s: &str) -> Self {
        match s {
            "impact" => Self::Impact,
            "path" | "endless" => Self::Path,
            "damage" => Self::Damage,
            "calibration" => Self::Calibration,
            _ => Self::Viewer,
        }
    }
}

/// The client's state the page and the window title read.
#[derive(Resource, Debug, Default)]
pub struct Status {
    pub frames: u64,
    pub telemetry: String,
    /// Batched per-step audio frames since the last display frame, as JSON.
    pub audio: String,
}

pub fn app(mode: Mode) -> App {
    let mut app = App::new();
    #[cfg(target_arch = "wasm32")]
    let window = Window {
        title: "Mech Works".into(),
        canvas: Some("#game".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: false,
        ..default()
    };
    #[cfg(not(target_arch = "wasm32"))]
    let window = Window {
        title: "Mech Works".into(),
        resolution: bevy::window::WindowResolution::new(1280, 800),
        ..default()
    };
    app.add_plugins(ClientPlugins.build().set(WindowPlugin {
        primary_window: Some(window),
        ..default()
    }));
    game(&mut app, mode);
    app
}

pub fn game(app: &mut App, mode: Mode) {
    app.insert_resource(DirectionalLightShadowMap { size: 2048 })
        .insert_resource(ClearColor(scene::SKY))
        .insert_resource(mode)
        .init_resource::<Status>()
        .add_systems(Update, count_frames)
        .add_systems(PreUpdate, page_commands.before(lanes::take_commands));
    scene::plugin(app);
    lanes::plugin(app);
    view::plugin(app);
    camera::plugin(app);
    impact::plugin(app);
}

/// Commands the lanes do not handle, peeled off before they see them.
fn page_commands(
    mut inspector: ResMut<view::Inspector>,
    mut range: ResMut<impact::Range>,
    mut orbit: ResMut<camera::Orbit>,
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
            Some("camera") => orbit.apply(&v),
            Some("mode") => {
                *mode = Mode::parse(v.get("mode").and_then(|m| m.as_str()).unwrap_or(""));
                // Also pass to lanes so it switches between flat ground and the endless path.
                rest.push(line);
            }
            _ => rest.push(line),
        }
    }
    if !rest.is_empty() {
        crate::COMMANDS.put(rest);
    }
}

/// Natively: saves a screenshot to `path` after `secs` of running, then
/// exits (`--shot`, for looking at a change without serving a build).
#[cfg(not(target_arch = "wasm32"))]
pub fn shot_after(app: &mut App, path: String, secs: f32) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let mut state = 0u8;
    app.add_systems(
        Last,
        move |mut commands: Commands, time: Res<Time>, mut exit: MessageWriter<AppExit>| {
            let t = time.elapsed_secs();
            if state == 0 && t > secs {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path.clone()));
                state = 1;
            } else if state == 1 && t > secs + 1.0 {
                exit.write(AppExit::Success);
            }
        },
    );
}

fn count_frames(mut st: ResMut<Status>) {
    st.frames += 1;
}
