//! The frame-to-tick arithmetic of a fixed-tick loop, with no engine and no
//! clock: the caller measures the frame and this says how many ticks to
//! run. It is the loop of Midnight Playground's `play/session.rs` with the
//! simulation taken out, so the kit's session, a headless runner and a test
//! share one definition of "how many ticks does this frame get".
//!
//! The rules (Midnight's SPEC 4.1, which snowboarding's loop matches):
//!
//! - A frame's time is clamped to `max_frame` before it is added to the
//!   accumulator; a longer frame (a stall, a backgrounded tab) does not
//!   produce a burst of ticks, the game slows down instead.
//! - At most `max_ticks` run per frame.
//! - A negative frame time (a clock that went back) and a NaN add nothing.
//! - A hair of tolerance (`1e-9` s) on the comparison, so that a frame of
//!   exactly `max_ticks * dt` runs `max_ticks` ticks whatever the rounding.
//! - What is left in the accumulator, as a fraction of a tick, is how far
//!   the drawn frame sits between the previous tick and the current one.

/// The tolerance on "a whole tick has accumulated".
pub const EPS: f64 = 1e-9;

/// The loop's three constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pacing {
    /// The tick, s.
    pub dt: f64,
    /// Most ticks in one frame.
    pub max_ticks: u32,
    /// The longest frame time credited to the accumulator, s.
    pub max_frame: f64,
}

impl Pacing {
    pub const fn new(dt: f64, max_ticks: u32, max_frame: f64) -> Pacing {
        Pacing {
            dt,
            max_ticks,
            max_frame,
        }
    }

    /// `max_ticks` ticks of a simulation's `DT`, and a frame clamp of
    /// exactly that long (Midnight's six ticks and 1/20 s).
    pub fn of<S: crate::Simulation>(max_ticks: u32) -> Pacing {
        Pacing::new(S::DT, max_ticks, S::DT * f64::from(max_ticks))
    }

    /// Credits a frame and returns the ticks to run; `acc` is the time not
    /// yet stepped, which the caller keeps between frames.
    pub fn advance(&self, acc: &mut f64, frame_dt: f64) -> u32 {
        advance(acc, frame_dt, self.dt, self.max_ticks, self.max_frame)
    }

    /// How far the frame is from the previous tick (0) to the current (1).
    pub fn alpha(&self, acc: f64) -> f64 {
        alpha(acc, self.dt)
    }
}

/// Credits `frame_dt` (clamped to `0..=max_frame`, NaN as 0) to `acc`,
/// takes as many whole ticks of `dt` out of it as it holds, up to
/// `max_ticks`, and returns how many.
pub fn advance(acc: &mut f64, frame_dt: f64, dt: f64, max_ticks: u32, max_frame: f64) -> u32 {
    let credit = if frame_dt.is_nan() {
        0.0
    } else {
        frame_dt.clamp(0.0, max_frame)
    };
    *acc += credit;
    let mut n = 0;
    while *acc >= dt - EPS && n < max_ticks {
        *acc -= dt;
        n += 1;
    }
    n
}

/// The unstepped part of a tick, 0..=1.
pub fn alpha(acc: f64, dt: f64) -> f64 {
    if acc.is_nan() {
        0.0
    } else {
        (acc / dt).clamp(0.0, 1.0)
    }
}
