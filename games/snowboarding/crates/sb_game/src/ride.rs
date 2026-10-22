//! A run: the [`Session`] stepped at its fixed tick from the frame's
//! controls, the riders' bodies posed from it (interpolated between ticks),
//! the camera told where to look, snow thrown, and the tick's events handed
//! on to the HUD and the sound through [`Feed`].

use crate::camera::{CamFocus, CamMode};
use crate::fx::Particles;
use crate::input::Controls;
use crate::rider_view::{RiderPose, spawn_rider};
use crate::{Opts, Screen, Setup};
use bevy::prelude::*;
use sb_sim::race::{COUNTDOWN, Session, SessionEvent, State, TICK};
use sb_sim::rider::{Event, Input, Phase};

pub fn plugin(app: &mut App) {
    app.init_resource::<Feed>()
        .add_systems(OnEnter(Screen::Ride), start_run)
        .add_systems(OnExit(Screen::Ride), end_run)
        .add_systems(
            Update,
            (step_run, pose)
                .chain()
                .in_set(RideSet)
                .run_if(in_state(Screen::Ride)),
        );
}

/// The run's frame: stepped and posed. The HUD and sound read after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RideSet;

/// The run in progress.
#[derive(Resource)]
pub struct Run {
    pub session: Session,
    acc: f32,
    prev: Vec<Vec3>,
    pub paused: bool,
    /// Seconds since the results came up.
    pub done_for: f32,
    clock: f32,
    /// Counts the runs since the game started (a restart is a new run).
    pub serial: u64,
}

/// This frame's events from the run.
#[derive(Resource, Default)]
pub struct Feed(pub Vec<SessionEvent>);

#[derive(Component)]
pub struct RiderSlot(pub usize);

fn new_run(setup: &Setup, opts: &Opts) -> Run {
    let mut session = Session::new(setup.course, setup.mode, setup.character, setup.autopilot);
    if setup.assist {
        session.entrants[0].rider.assist = crate::ASSIST;
    }
    // Fast-forward (tests and pictures).
    let skip_ticks = (opts.0.skip / TICK) as usize;
    if skip_ticks > 0 {
        while matches!(session.state, State::Countdown(_)) {
            session.step(Input::default());
        }
        for _ in 0..skip_ticks {
            if session.state == State::Done {
                break;
            }
            session.step(Input::default());
        }
        session.events.clear();
    }
    let prev = session.entrants.iter().map(|e| e.rider.pos).collect();
    Run {
        session,
        acc: 0.0,
        prev,
        paused: false,
        done_for: 0.0,
        clock: 0.0,
        serial: 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_run(
    mut commands: Commands,
    setup: Res<Setup>,
    mut opts: ResMut<Opts>,
    old: Query<Entity, With<RiderSlot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut focus: ResMut<CamFocus>,
    mut particles: ResMut<Particles>,
    mut serial: Local<u64>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let mut run = new_run(&setup, &opts);
    *serial += 1;
    run.serial = *serial;
    // The fast-forward is for the first run only: a restart starts over.
    opts.0.skip = 0.0;
    for (i, e) in run.session.entrants.iter().enumerate() {
        spawn_rider(
            &mut commands,
            &mut meshes,
            &mut materials,
            e.character,
            RiderSlot(i),
        );
    }
    let p = &run.session.entrants[0].rider;
    focus.pos = p.pos;
    focus.yaw = p.yaw;
    focus.cut = true;
    particles.clear();
    commands.insert_resource(run);
}

fn end_run(mut commands: Commands, old: Query<Entity, With<RiderSlot>>) {
    for e in &old {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<Run>();
}

fn step_run(
    time: Res<Time>,
    controls: Res<Controls>,
    run: Option<ResMut<Run>>,
    mut feed: ResMut<Feed>,
) {
    feed.0.clear();
    let Some(mut run) = run else { return };
    let run = &mut *run;
    let dt = time.delta_secs().min(0.25);
    if run.session.state == State::Done {
        run.done_for += dt;
    }
    if run.paused {
        return;
    }
    run.clock += dt;
    run.acc += dt;
    let input = Input {
        steer: controls.steer,
        pitch: controls.pitch,
        jump: controls.jump,
        grab: controls.grab,
    };
    let mut n = 0;
    while run.acc >= TICK && n < 30 {
        run.acc -= TICK;
        n += 1;
        for (p, e) in run.prev.iter_mut().zip(&run.session.entrants) {
            *p = e.rider.pos;
        }
        run.session.step(input);
        feed.0.append(&mut run.session.events);
    }
    if n == 30 {
        run.acc = 0.0;
    }
}

fn pose(
    time: Res<Time>,
    run: Option<Res<Run>>,
    feed: Res<Feed>,
    mut riders: Query<(&RiderSlot, &mut RiderPose)>,
    mut focus: ResMut<CamFocus>,
    mut particles: ResMut<Particles>,
) {
    let Some(run) = run else { return };
    let alpha = (run.acc / TICK).clamp(0.0, 1.0);
    let s = &run.session;
    for (slot, mut pose) in &mut riders {
        let Some(e) = s.entrants.get(slot.0) else {
            continue;
        };
        let r = &e.rider;
        let pos = if run.paused {
            r.pos
        } else {
            run.prev[slot.0].lerp(r.pos, alpha)
        };
        *pose = RiderPose {
            pos,
            vel: r.vel,
            yaw: r.yaw,
            normal: r.normal,
            pitch: r.pitch,
            phase: r.phase,
            crouch: r.crouch,
            lean: r.lean,
            tuck: r.tuck,
            skid: r.skid,
            grab: r.grab,
            switch: r.switch,
            airborne: r.airborne(),
            air_time: r.air.time,
            air_spin: r.air.spin,
            t: run.clock,
            idle: matches!(s.state, State::Countdown(_)),
            finished: e.finish.is_some(),
        };
        // Snow off the edge.
        if !run.paused && r.phase != Phase::Air {
            let speed = r.speed();
            let rate = r.spray * speed * 9.0
                + if matches!(r.phase, Phase::Crashed(_)) {
                    speed * 6.0
                } else {
                    0.0
                };
            let n = (rate * time.delta_secs() + fastrand_frac(run.clock + slot.0 as f32)) as usize;
            if n > 0 {
                let fwd = r.forward();
                let side = fwd.cross(r.normal).normalize_or(Vec3::X);
                let lean = if r.lean.abs() > 0.05 {
                    -r.lean.signum()
                } else {
                    1.0
                };
                let at = r.pos + side * 0.2 * lean + fwd * 0.2;
                let vel = r.vel * 0.35
                    + side * lean * (1.5 + speed * 0.12)
                    + Vec3::Y * (1.2 + speed * 0.08);
                particles.emit(at, vel, 1.3, n.min(12), 0.11);
            }
        }
    }
    for ev in &feed.0 {
        if let SessionEvent::Rider(i, ev) = ev {
            let r = &s.entrants[*i].rider;
            match ev {
                Event::Land { impact, .. } if *impact > 3.0 => {
                    particles.emit(r.pos, r.vel * 0.2, 2.5, (*impact * 3.0) as usize + 6, 0.14);
                }
                Event::Crash { .. } => particles.emit(r.pos, r.vel * 0.3, 3.5, 50, 0.16),
                Event::Jump { .. } => particles.emit(r.pos, r.vel * 0.1, 1.2, 10, 0.1),
                _ => {}
            }
        }
    }

    let p = &s.entrants[0].rider;
    focus.pos = run.prev[0].lerp(p.pos, alpha);
    focus.s = p.s;
    focus.pipe = s.course.pipe_weight(p.s) > 0.5;
    focus.vel = p.vel;
    focus.yaw = s.course.frame(p.s).theta;
    focus.airborne = p.airborne();
    focus.crashed = matches!(p.phase, Phase::Crashed(_));
    focus.mode = match s.state {
        State::Countdown(t) => CamMode::Intro(((COUNTDOWN - t) / COUNTDOWN).clamp(0.0, 1.0)),
        State::Running => CamMode::Chase,
        State::Done => CamMode::Orbit {
            radius: 5.5,
            height: 1.8,
            rate: 0.35,
        },
    };
}

/// A deterministic fraction in [0, 1) that changes every frame, so a low
/// emission rate still emits on average.
fn fastrand_frac(t: f32) -> f32 {
    sb_sim::noise::unit(sb_sim::noise::hash((t * 1000.0) as u32))
}
