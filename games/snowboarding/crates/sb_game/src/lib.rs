//! The Snowboarding client. Only this crate depends on the engine; the
//! game itself (the mountain, the riders, tricks, races) is `sb_sim`.
//!
//! - [`options`]: the query string (web) or command line (native).
//! - [`world`]: the course drawn: snow, sky, trees, flags, gates, light.
//! - [`rider_view`]: the riders' bodies, posed from their state each frame.
//! - [`camera`]: the intro, drop-in and chase camera, and the menu's flyover.
//! - [`ride`]: a run: the session stepped at its fixed tick, its events.
//! - [`fx`]: snow spray, falling snow and shader wind streaks.
//! - [`input`]: keyboard, gamepads and touch into one set of controls.
//! - [`hud`], [`menu`]: the screens.
//! - [`radio`]: the announcers (`sb_radio`), their captions, and the mix.
//! - [`audio`]: Web Audio sound on the web: effects, the station's music
//!   (`music` in an AudioWorklet) and the announcers' clips.
//! - [`plugins`]: Bevy's default plugins less its 2D sprites.
//!
//! The web build comes in two backends, WebGPU and WebGL2 (the `sb_webgl2`
//! cfg), one wasm file each; the page picks.

pub mod audio;
pub mod camera;
pub mod fx;
pub mod hud;
pub mod input;
pub mod menu;
pub mod meshes;
pub mod options;
pub mod plugins;
pub mod radio;
pub mod ride;
pub mod rider_view;
pub mod status;
pub mod ui;
pub mod world;

#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
mod web;

use bevy::light::DirectionalLightShadowMap;
use bevy::prelude::*;
use options::Options;
use sb_sim::course::CourseId;
use sb_sim::race::Mode;

/// Where the player is.
#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum Screen {
    #[default]
    Menu,
    Ride,
}

/// What the next run is.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Setup {
    pub mode: Mode,
    pub course: CourseId,
    pub character: usize,
    pub autopilot: bool,
    /// The landing assist (see `sb_sim::rider::Rider::assist`).
    pub assist: bool,
    /// The player chose the assist themselves: touch no longer turns it on.
    pub assist_chosen: bool,
    /// What the radio plays: music, the announcers, both or neither.
    pub radio: radio::Radio,
}

/// How strong the landing assist is when on: a nudge, not autopilot.
pub const ASSIST: f32 = 0.6;

/// The options, resolved.
#[derive(Resource, Clone, Debug)]
pub struct Opts(pub Options);

pub fn app(o: Options) -> App {
    let mut app = App::new();
    #[cfg(target_arch = "wasm32")]
    let window = Window {
        title: "Snowboarding".into(),
        canvas: Some("#game".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    };
    #[cfg(not(target_arch = "wasm32"))]
    let window = Window {
        title: "Snowboarding".into(),
        resolution: bevy::window::WindowResolution::new(o.width, o.height),
        ..default()
    };
    app.add_plugins(plugins::ClientPlugins.build().set(WindowPlugin {
        primary_window: Some(window),
        ..default()
    }));
    game(&mut app, o);
    app
}

/// The game itself on top of Bevy's plugins: its resources, its state and
/// every one of its plugins. [`app`] puts it in a window; [`headless`] runs
/// it with neither window nor GPU, for the tests.
pub fn game(app: &mut App, o: Options) {
    app.insert_resource(DirectionalLightShadowMap { size: 2048 })
        .insert_resource(ClearColor(Color::srgb(0.55, 0.7, 0.9)))
        .insert_resource(Setup {
            mode: o.mode,
            course: o.course,
            character: o.character,
            autopilot: o.auto,
            assist: o.assist.unwrap_or(false),
            assist_chosen: o.assist.is_some(),
            radio: o.radio.unwrap_or_else(audio::saved_radio),
        })
        .insert_resource(Opts(o.clone()))
        .init_state::<Screen>();
    status::plugin(app);
    ui::plugin(app);
    input::plugin(app);
    world::plugin(app);
    camera::plugin(app);
    rider_view::plugin(app);
    fx::plugin(app);
    ride::plugin(app);
    hud::plugin(app);
    menu::plugin(app);
    radio::plugin(app);
    audio::plugin(app);
    if o.go {
        app.insert_state(Screen::Ride);
    }
}

/// The whole game without a window or a GPU: the client's plugins less
/// winit, the renderer started with no backend (so no render world), and
/// nothing process-wide (logging, Ctrl-C, gamepads), so a test can make
/// many. Every system of the game runs; drive it with [`App::update`]
/// after `finish` and `cleanup` (see `tests::Harness`).
#[cfg(not(target_arch = "wasm32"))]
pub fn headless(o: Options) -> App {
    use bevy::render::RenderPlugin;
    use bevy::render::settings::WgpuSettings;
    let mut app = App::new();
    app.add_plugins(
        plugins::ClientPlugins
            .build()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: WgpuSettings {
                    backends: None,
                    ..default()
                }
                .into(),
                ..default()
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::app::TerminalCtrlCHandlerPlugin>()
            .disable::<bevy::gilrs::GilrsPlugin>(),
    );
    // Without a render world nothing adds this, yet despawning anything
    // drawable still reports to it.
    app.add_plugins(bevy::render::sync_world::SyncWorldPlugin);
    game(&mut app, o);
    app
}

/// Entry point natively.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> AppExit {
    let o = Options::from_args(std::env::args().skip(1));
    let shot = o.screenshot.clone();
    let mut app = app(o);
    if let Some((path, frames)) = shot {
        status::native_screenshot(&mut app, path, frames);
    }
    app.run()
}
