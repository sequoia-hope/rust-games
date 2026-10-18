//! The CPU riders: they look ahead along the racing line, steer round the
//! trees, charge up for the kickers, and in the air pick a trick they have
//! the time for. They drive the same [`Rider`] with the same [`Input`] the
//! player has.

use crate::course::{Course, ObstacleKind, PIPE_COPING, PIPE_FLAT};
use crate::noise::{Rng, wrap_angle};
use crate::rider::{G, Input, Phase, Rider};
use glam::Vec3;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Debug)]
struct AirPlan {
    spin: f32,
    flip: f32,
    grab_stick: Option<(f32, f32)>,
    grab_from: f32,
    grab_to: f32,
}

#[derive(Clone, Debug)]
pub struct Brain {
    /// 0 .. 1: how much it dares.
    pub skill: f32,
    rng: Rng,
    plan: Option<AirPlan>,
    lane: f32,
    lane_timer: f32,
    hit_kicker: bool,
    last_kicker: f32,
    pipe_side: f32,
}

fn heading_of(v: Vec3) -> f32 {
    v.x.atan2(-v.z)
}

impl Brain {
    pub fn new(skill: f32, seed: u32) -> Brain {
        Brain {
            skill,
            rng: Rng::new(seed),
            plan: None,
            lane: 0.0,
            lane_timer: 0.0,
            hit_kicker: true,
            last_kicker: -1.0,
            pipe_side: 1.0,
        }
    }

    pub fn think(&mut self, me: &Rider, course: &Course, dt: f32) -> Input {
        match me.phase {
            Phase::Air => self.fly(me, course),
            Phase::Crashed(_) => {
                self.plan = None;
                Input::default()
            }
            Phase::Ground => {
                self.plan = None;
                self.ride(me, course, dt)
            }
        }
    }

    fn ride(&mut self, me: &Rider, course: &Course, dt: f32) -> Input {
        let speed = me.speed();
        let in_pipe = course.pipe_weight(me.s) > 0.5;
        self.lane_timer -= dt;
        if self.lane_timer <= 0.0 {
            self.lane_timer = self.rng.range(2.0, 5.0);
            self.lane = self.rng.range(-5.0, 5.0);
        }
        let look = if in_pipe {
            5.0 + speed * 0.35
        } else {
            9.0 + speed * 0.7
        };
        let st = me.s + look;
        let hw = course.half_width(st);
        let mut target_u = course.racing_line(st) + self.lane;
        let mut jump = false;
        if in_pipe {
            if me.u * self.pipe_side > PIPE_FLAT + 1.0 {
                // On the wall it was heading for: aim for the other one next.
                self.pipe_side = -self.pipe_side;
            }
            target_u = -self.pipe_side * (PIPE_COPING + 2.0);
        } else if let Some(k) = course.kicker_ahead(me.s, 80.0) {
            if (k.s - self.last_kicker).abs() > 1.0 {
                self.last_kicker = k.s;
                self.hit_kicker = self.rng.chance(0.55 + 0.45 * self.skill);
            }
            if self.hit_kicker {
                target_u = k.u;
                let d = k.s - me.s;
                if d > 0.7 && d < 10.0 && (me.u - k.u).abs() < k.half_width {
                    jump = true;
                }
            }
        }
        if !in_pipe {
            // Round the trees.
            for o in course.obstacles_between(me.s + 1.0, st + 10.0) {
                if o.kind == ObstacleKind::Tree && (o.u - target_u).abs() < 3.2 {
                    target_u = o.u + if target_u >= o.u { 3.6 } else { -3.6 };
                }
            }
            target_u = target_u.clamp(-(hw - 2.0), hw - 2.0);
        }
        let tp = course.world(st, target_u);
        let desired = heading_of(tp - me.pos);
        let err = wrap_angle(desired - me.yaw);
        let steer = (err * 3.0).clamp(-1.0, 1.0);
        let pitch = if err.abs() < 0.12 { 1.0 } else { 0.0 };
        Input {
            steer,
            pitch,
            jump,
            grab: false,
        }
    }

    /// Seconds until the rider is back on the snow, by stepping its fall.
    fn air_time_left(me: &Rider, course: &Course) -> f32 {
        let (mut p, mut v) = (me.pos, me.vel);
        let dt = 1.0 / 30.0;
        let mut t = 0.0;
        let mut hint = me.s;
        while t < 6.0 {
            v.y -= G * dt;
            p += v * dt;
            t += dt;
            let g = course.ground(p, hint);
            hint = g.s;
            if p.y <= g.height {
                break;
            }
        }
        t
    }

    fn fly(&mut self, me: &Rider, course: &Course) -> Input {
        if self.plan.is_none() {
            let t = Self::air_time_left(me, course);
            let usable = (t - 0.45).max(0.0);
            let max_halves = (usable * 9.0 * me.stats.spin / PI).floor() as i32;
            let mut halves = 0;
            if max_halves >= 1 {
                let dare = self.skill * self.rng.range(0.55, 1.0);
                halves = ((max_halves as f32) * dare).round() as i32;
                if halves % 2 == 1 && course.pipe_weight(me.s) < 0.5 {
                    halves -= 1; // land forward off kickers
                }
            }
            let mut flip = 0.0;
            if t > 1.5 && self.rng.chance(0.3 * self.skill) {
                flip = -TAU;
                halves = halves.min(2);
            }
            let dir = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
            let grab_stick = (t > 0.8).then(|| {
                let pick = self.rng.next_u32() % 5;
                [
                    (0.0, 0.0),
                    (-1.0, 0.0),
                    (0.0, 1.0),
                    (0.0, -1.0),
                    (-1.0, -1.0),
                ][pick as usize]
            });
            self.plan = Some(AirPlan {
                spin: dir * halves as f32 * PI,
                flip,
                grab_stick,
                grab_from: 0.15 * t,
                grab_to: (t - 0.4).max(0.0),
            });
        }
        let plan = self.plan.as_ref().unwrap();
        let toward = |target: f32, done: f32| {
            let rem = target - done;
            if rem.abs() > 0.4 {
                rem.signum()
            } else if rem.abs() > 0.06 {
                rem / 0.4
            } else {
                0.0
            }
        };
        let mut steer = toward(plan.spin, me.air.spin);
        let mut pitch = toward(plan.flip, me.air.flip);
        let t = me.air.time;
        let grab = plan.grab_stick.is_some() && t > plan.grab_from && t < plan.grab_to;
        // The grab is the stick's direction as the button goes down.
        if grab
            && me.grab.is_none()
            && let Some((x, y)) = plan.grab_stick
        {
            if steer == 0.0 {
                steer = x;
            }
            if pitch == 0.0 {
                pitch = y;
            }
        }
        Input {
            steer,
            pitch,
            jump: false,
            grab,
        }
    }
}
