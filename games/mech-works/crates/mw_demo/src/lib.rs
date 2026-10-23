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
    /// Demo 3: damage an actuator, watch the limp.
    Damage,
    /// Demo 4: swap a part, watch it calibrate.
    Calibration,
}

impl Mode {
    pub fn parse(s: &str) -> Self {
        match s {
            "impact" => Self::Impact,
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
        .add_systems(Update, count_frames);
    scene::plugin(app);
    lanes::plugin(app);
    view::plugin(app);
    camera::plugin(app);
    impact::plugin(app);
}

fn count_frames(mut st: ResMut<Status>) {
    st.frames += 1;
}
