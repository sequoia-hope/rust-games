//! Screenshot stations (Midnight Playground DECISIONS D17; its
//! `stations.rs`): natively, a list of named poses is visited in turn; at
//! each the game applies the pose, the run waits until no pipeline is
//! compiling, the game's own `settled` predicate holds (its sky and
//! environment map built) and three quiet frames have passed, saves
//! `<out>/<name>.png`, and exits after the last. A game's parity tooling
//! compares the pictures with its reference set.

use crate::status::{LoadState, LoadStatus, PIPELINES_WAITING};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use std::sync::atomic::{AtomicBool, Ordering};

/// The run: the poses, where the pictures go, and the game's two hooks.
#[derive(Resource)]
pub struct Stations<P: Send + Sync + 'static> {
    pub list: Vec<(String, P)>,
    pub out: std::path::PathBuf,
    /// Puts the camera at the pose.
    pub apply: fn(&mut World, &P),
    /// Whether the picture is complete besides the pipelines (the
    /// environment map built, the sky still).
    pub settled: fn(&World) -> bool,
    /// Quiet frames before the capture.
    pub quiet_frames: u32,
    index: usize,
    frames: u32,
    quiet: u32,
    capturing: bool,
}

static CAPTURED: AtomicBool = AtomicBool::new(false);

impl<P: Send + Sync + 'static> Stations<P> {
    pub fn new(
        list: Vec<(String, P)>,
        out: std::path::PathBuf,
        apply: fn(&mut World, &P),
        settled: fn(&World) -> bool,
    ) -> Stations<P> {
        Stations {
            list,
            out,
            apply,
            settled,
            quiet_frames: 3,
            index: 0,
            frames: 0,
            quiet: 0,
            capturing: false,
        }
    }
}

/// Adds the run; the system runs in `Update` (order it after the game's
/// camera with `pk_shell::stations::run::<P>`).
pub struct StationsPlugin<P: Send + Sync + 'static>(pub Stations<P>);

impl<P: Clone + Send + Sync + 'static> Plugin for StationsPlugin<P> {
    fn build(&self, app: &mut App) {
        app.insert_resource(Stations {
            list: self.0.list.clone(),
            out: self.0.out.clone(),
            apply: self.0.apply,
            settled: self.0.settled,
            quiet_frames: self.0.quiet_frames,
            index: 0,
            frames: 0,
            quiet: 0,
            capturing: false,
        })
        .add_systems(Update, run::<P>);
    }
}

/// One station a frame: the pose on its first frame, the capture once
/// settled, the next after the capture lands.
pub fn run<P: Clone + Send + Sync + 'static>(world: &mut World) {
    let status = world.resource::<LoadStatus>();
    if !status.ready {
        if status.state == LoadState::Failed {
            world.write_message(AppExit::error());
        }
        return;
    }
    let (name, pose, apply, settled, out, quiet_frames) = {
        let mut sr = world.resource_mut::<Stations<P>>();
        if sr.capturing {
            if CAPTURED.load(Ordering::Relaxed) {
                sr.capturing = false;
                sr.index += 1;
                sr.frames = 0;
                sr.quiet = 0;
            }
            return;
        }
        let Some((name, pose)) = sr.list.get(sr.index).cloned() else {
            info!("stations: done");
            world.write_message(AppExit::Success);
            return;
        };
        (
            name,
            pose,
            sr.apply,
            sr.settled,
            sr.out.clone(),
            sr.quiet_frames,
        )
    };
    let first = world.resource::<Stations<P>>().frames == 0;
    if first {
        apply(world, &pose);
    }
    let settled = PIPELINES_WAITING.load(Ordering::Relaxed) == 0 && settled(world);
    let mut sr = world.resource_mut::<Stations<P>>();
    sr.frames += 1;
    if settled && sr.frames > 3 {
        sr.quiet += 1;
    } else {
        sr.quiet = 0;
    }
    if sr.quiet >= quiet_frames {
        sr.capturing = true;
        let _ = std::fs::create_dir_all(&out);
        CAPTURED.store(false, Ordering::Relaxed);
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(out.join(format!("{name}.png"))))
            .observe(|_: On<ScreenshotCaptured>| CAPTURED.store(true, Ordering::Relaxed));
    }
}
