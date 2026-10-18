//! What the page (or a test) can read about the game: the screen, frames,
//! the run's state and the player's rider. On the web it is published on
//! `window.__sb` (`web.rs`); natively `--screenshot` saves a frame and
//! exits.

use crate::ride::Run;
use crate::{Screen, Setup};
use bevy::prelude::*;
use sb_sim::race::State;
use sb_sim::rider::Phase;

pub fn plugin(app: &mut App) {
    app.init_resource::<Status>().add_systems(Last, update);
}

#[derive(Resource, Default, Clone, Debug)]
pub struct Status {
    pub frames: u64,
    pub screen: &'static str,
    pub state: &'static str,
    pub course: &'static str,
    pub mode: &'static str,
    pub time: f32,
    pub s: f32,
    pub speed: f32,
    pub phase: &'static str,
    pub score: u32,
    pub tricks: usize,
    pub crashes: u32,
    pub place: usize,
    pub finish: Option<f32>,
    pub last_trick: String,
}

fn update(
    mut st: ResMut<Status>,
    screen: Res<bevy::state::state::State<Screen>>,
    setup: Res<Setup>,
    run: Option<Res<Run>>,
) {
    st.frames += 1;
    st.screen = match screen.get() {
        Screen::Menu => "menu",
        Screen::Ride => "ride",
    };
    st.course = setup.course.key();
    st.mode = setup.mode.key();
    if let Some(run) = run {
        let s = &run.session;
        let me = s.player();
        st.state = match s.state {
            State::Countdown(_) => "countdown",
            State::Running if run.paused => "paused",
            State::Running => "running",
            State::Done => "done",
        };
        st.time = s.time;
        st.s = me.rider.s;
        st.speed = me.rider.speed();
        st.phase = match me.rider.phase {
            Phase::Ground => "ground",
            Phase::Air => "air",
            Phase::Crashed(_) => "crashed",
        };
        st.score = me.score;
        st.tricks = me.tricks.len();
        st.crashes = me.crashes;
        st.place = s.place(0);
        st.finish = me.finish;
        st.last_trick = me.tricks.last().map(|t| t.name.clone()).unwrap_or_default();
    } else {
        st.state = "menu";
    }
}

/// Natively: save the window after `frames` frames to `path`, then exit.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_screenshot(app: &mut App, path: String, frames: u32) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let mut taken = false;
    app.add_systems(Update, move |mut commands: Commands, st: Res<Status>| {
        if taken || st.frames < frames as u64 {
            return;
        }
        taken = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()))
            .observe(
                |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                },
            );
    });
}
