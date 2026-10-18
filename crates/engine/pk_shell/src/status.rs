//! What the client reports about itself: the load state the page's loading
//! bar and the status global show (the `state`, `progress`, `frames` and
//! `ready` fields of the `__game` contract, PLAN.md section 5 item 7), the
//! window title natively, and when the scene counts as ready (Midnight
//! Playground DECISIONS D390, D392, D1005; its `status.rs`).
//!
//! **Ready** means the scene is up, the first frames have queued their
//! pipelines (the warm-up's among them, `crate::warmup`), none is still
//! compiling, every blocker the game registered has cleared, and the GPU
//! has finished the work submitted since: a browser creates a pipeline in
//! its GPU process after the call returns, and the first draw with it
//! waits there (a one-second frame just after "ready" in Chrome before
//! the fence; D390). Render pipelines only are counted as waiting: on
//! WebGL2 Bevy 0.19 queues its "sparse buffer update" compute pipeline
//! although the device has no compute, and that pipeline waits for ever
//! for a shader that is never loaded (D392). Pipelines compiled after
//! ready are counted as late frames (a missed warm-up combination shows),
//! unless the game says they are expected ([`late_is_expected_while`]: a
//! background view's own warm-up, D1005).

use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::renderer::RenderQueue;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// The client's state, as the status contract names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LoadState {
    /// Waiting for a scene.
    #[default]
    Waiting,
    /// Building the scene, a slice per frame (`progress`).
    Building,
    /// The scene is up, its pipelines compiling behind the loading screen.
    Warming,
    /// Ready; drawing.
    Running,
    /// Loading failed (`error`).
    Failed,
}

impl LoadState {
    /// The contract's spelling: `waiting`, `building`, `warming`,
    /// `running`, `failed`.
    pub fn name(self) -> &'static str {
        match self {
            LoadState::Waiting => "waiting",
            LoadState::Building => "building",
            LoadState::Warming => "warming",
            LoadState::Running => "running",
            LoadState::Failed => "failed",
        }
    }

    /// Whether a scene is up (warming or running).
    pub fn scene_up(self) -> bool {
        matches!(self, LoadState::Warming | LoadState::Running)
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct LoadStatus {
    pub state: LoadState,
    /// Build progress, 0 to 1.
    pub progress: f32,
    pub error: Option<String>,
    /// Frames rendered since start.
    pub frames: u64,
    /// Frames rendered since the scene came up.
    pub scene_frames: u64,
    /// Smoothed frame time, ms.
    pub frame_ms: f32,
    /// Worst frame since the last report, ms (the title line resets it).
    pub worst_ms: f32,
    /// Render pipelines still compiling (natively they compile in the
    /// background, and a mesh draws only once its pipeline is ready).
    pub pipelines_waiting: usize,
    /// The scene is up and every pipeline it needs is compiled.
    pub ready: bool,
    /// Frames since `ready`.
    pub ready_frames: u64,
    /// Warm-up stand-ins spawned for the scene (material × mesh-layout
    /// combinations).
    pub warm_up: usize,
    /// Scenes loaded since start (a reload adds one).
    pub scenes: u32,
    /// The GPU fence the warm-up waits on (`gpu_fence`).
    pub fence: Option<u32>,
    /// Frames since `ready` with a render pipeline still compiling (the
    /// warm-up missed one; zero when it works).
    pub late_frames: u64,
}

impl LoadStatus {
    /// Loading failed: logged, kept for the page and the smoke test.
    pub fn fail(&mut self, e: impl Into<String>) {
        let e = e.into();
        error!("{e}");
        self.error = Some(e);
        self.state = LoadState::Failed;
    }

    /// Back to waiting for a scene (a reload): the frame counts and the
    /// smoothed frame time carry over, the scene's own fields reset.
    pub fn reset_for_reload(&mut self) {
        *self = LoadStatus {
            state: LoadState::Waiting,
            frames: self.frames,
            frame_ms: self.frame_ms,
            scenes: self.scenes,
            ..default()
        };
    }
}

/// What must clear before the scene counts as ready, besides the
/// pipelines: a game registers functions that answer "still busy" (a
/// world build for its animators, a section still warming).
#[derive(Resource, Default)]
pub struct ReadyBlockers(Vec<fn() -> bool>);

impl ReadyBlockers {
    pub fn add(&mut self, blocker: fn() -> bool) {
        self.0.push(blocker);
    }

    /// Whether any blocker is still busy.
    pub fn busy(&self) -> bool {
        self.0.iter().any(|b| b())
    }
}

/// Pipelines waiting in the render world's cache, published each frame.
pub static PIPELINES_WAITING: AtomicUsize = AtomicUsize::new(0);

/// The warm-up is done (`LoadStatus::ready`), for the render world: a
/// render pipeline queued after it is logged by its label, unless expected.
static READY: AtomicBool = AtomicBool::new(false);

/// Pipelines queued after ready are expected while any of these says so
/// (a background view's own stand-ins, D1005); they do not count as late.
static EXPECTED_LATE: RwLock<Vec<fn() -> bool>> = RwLock::new(Vec::new());

/// Registers a condition under which pipelines compiling after ready are
/// expected and not counted as late frames.
pub fn late_is_expected_while(f: fn() -> bool) {
    EXPECTED_LATE
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .push(f);
}

fn late_expected() -> bool {
    EXPECTED_LATE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|f| f())
}

/// The GPU fence of the warm-up: the main world asks (a new number), the
/// render world hands the number to the queue after this frame's
/// submission, and the queue sets `FENCE_DONE` to it once that work is done.
static FENCE_NEXT: AtomicU32 = AtomicU32::new(1);
static FENCE_ASKED: AtomicU32 = AtomicU32::new(0);
static FENCE_SENT: AtomicU32 = AtomicU32::new(0);
static FENCE_DONE: AtomicU32 = AtomicU32::new(0);

/// Render world: passes the fence asked for to the GPU queue.
pub fn gpu_fence(queue: Res<RenderQueue>) {
    let asked = FENCE_ASKED.load(Ordering::Relaxed);
    if asked != FENCE_SENT.swap(asked, Ordering::Relaxed) {
        queue.on_submitted_work_done(move || FENCE_DONE.store(asked, Ordering::Relaxed));
    }
}

/// Render world: how many pipelines are still compiling.
///
/// Render pipelines only (see the module's notes). A pipeline still waiting
/// after 600 frames is logged once, with its state, and one queued after
/// the warm-up once, with its label (its shader defs at debug level:
/// `RUST_LOG=pk_shell::status=debug`).
pub fn count_pipelines(
    cache: Res<PipelineCache>,
    mut stuck: Local<u32>,
    mut late: Local<Vec<usize>>,
) {
    use bevy::render::render_resource::PipelineDescriptor;
    let waiting: Vec<usize> = cache.waiting_pipelines().collect();
    if waiting.is_empty() {
        PIPELINES_WAITING.store(0, Ordering::Relaxed);
        *stuck = 0;
        return;
    }
    let mut render = 0;
    for (i, p) in cache.pipelines().enumerate() {
        if !waiting.contains(&i) {
            continue;
        }
        let label = match &p.descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(d) => {
                render += 1;
                if READY.load(Ordering::Relaxed) && !late_expected() && !late.contains(&i) {
                    late.push(i);
                    let label = d.label.as_deref().unwrap_or("?");
                    info!("pipeline {i} queued after the warm-up: {label}");
                    debug!(
                        "pipeline {i}: vertex {:?} {:?}, fragment {:?}",
                        d.vertex.shader_defs,
                        d.vertex
                            .buffers
                            .iter()
                            .map(|b| b.attributes.len())
                            .collect::<Vec<_>>(),
                        d.fragment.as_ref().map(|f| &f.shader_defs),
                    );
                }
                &d.label
            }
            PipelineDescriptor::ComputePipelineDescriptor(d) => &d.label,
        };
        if *stuck == 600 {
            warn!("pipeline {i} still waiting: {label:?} {:?}", p.state);
        }
    }
    PIPELINES_WAITING.store(render, Ordering::Relaxed);
    *stuck = if waiting.is_empty() { 0 } else { *stuck + 1 };
}

/// Counts frames and keeps a smoothed frame time, by the wall clock's
/// frame interval (`Time<Real>`): the generic clock is the virtual one,
/// which Bevy caps at 250 ms a frame, so it hid the length of a stall.
/// Decides `ready` (see the module's notes) and counts the late frames.
pub fn tick(time: Res<Time<Real>>, mut status: ResMut<LoadStatus>, blockers: Res<ReadyBlockers>) {
    status.frames += 1;
    status.pipelines_waiting = PIPELINES_WAITING.load(Ordering::Relaxed);
    if status.state.scene_up() {
        status.scene_frames += 1;
        if !status.ready && status.scene_frames > 10 && !blockers.busy() {
            if status.pipelines_waiting > 0 {
                status.fence = None;
            } else {
                match status.fence {
                    None => {
                        let n = FENCE_NEXT.fetch_add(1, Ordering::Relaxed);
                        FENCE_ASKED.store(n, Ordering::Relaxed);
                        status.fence = Some(n);
                    }
                    Some(n) if FENCE_DONE.load(Ordering::Relaxed) == n => {
                        status.ready = true;
                        status.state = LoadState::Running;
                        info!("ready: scene up, every pipeline compiled and the GPU done");
                    }
                    Some(_) => {}
                }
            }
        }
    }
    READY.store(status.ready, Ordering::Relaxed);
    if status.ready {
        status.ready_frames += 1;
        // A pipeline queued after the warm-up is one it missed (each costs
        // a hitch on the web); an expected one is not.
        if status.pipelines_waiting > 0 && !late_expected() {
            status.late_frames += 1;
            if status.late_frames == 1 {
                warn!(
                    "{} pipeline(s) compiling after the warm-up",
                    status.pipelines_waiting
                );
            }
        }
    }
    let ms = time.delta_secs() * 1000.0;
    // `RUST_LOG=pk_shell::status=debug`: the frames a player would see as a
    // hitch.
    if ms > 50.0 && status.frames > 2 {
        debug!(
            "slow frame {ms:.0} ms (frame {}, {}, {} pipeline(s) waiting)",
            status.frames,
            status.state.name(),
            status.pipelines_waiting
        );
    }
    status.frame_ms = if status.frame_ms == 0.0 {
        ms
    } else {
        status.frame_ms * 0.95 + ms * 0.05
    };
    status.worst_ms = status.worst_ms.max(ms);
}

/// [`LoadStatus`] (waiting) and [`ReadyBlockers`], `tick` first in every
/// frame, and the render world's pipeline count and GPU fence.
pub struct StatusPlugin;

impl Plugin for StatusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadStatus>()
            .init_resource::<ReadyBlockers>()
            .add_systems(First, tick.after(bevy::time::TimeSystems));
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.add_systems(
                bevy::render::Render,
                (count_pipelines, gpu_fence).in_set(bevy::render::RenderSystems::Cleanup),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_reload() {
        assert_eq!(LoadState::Warming.name(), "warming");
        assert!(LoadState::Running.scene_up() && !LoadState::Failed.scene_up());
        let mut s = LoadStatus {
            state: LoadState::Running,
            frames: 10,
            scene_frames: 5,
            frame_ms: 16.0,
            ready: true,
            scenes: 1,
            ..default()
        };
        s.reset_for_reload();
        assert_eq!(s.state, LoadState::Waiting);
        assert_eq!((s.frames, s.scene_frames, s.scenes), (10, 0, 1));
        assert!(!s.ready && s.frame_ms == 16.0);
        let mut b = ReadyBlockers::default();
        assert!(!b.busy());
        b.add(|| true);
        assert!(b.busy());
    }
}
