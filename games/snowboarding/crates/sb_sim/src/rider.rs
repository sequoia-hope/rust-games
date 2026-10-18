//! A rider on a board: carving on the snow, flying, spinning and grabbing in
//! the air, landing (or not), crashing and getting up again.
//!
//! On the snow the board's edge holds: velocity across the board decays and
//! most of it turns into speed along it, which is what makes a carve. In
//! the air the stick spins (left, right) and flips (up, down) the rider,
//! and the grab button grabs the board where the stick points. A landing is
//! judged by how far the board is from the direction of travel (either way
//! round: landing backwards is riding switch) and from level.

use crate::course::{Course, Ground, PIPE_FLAT};
use crate::noise::wrap_angle;
use crate::tricks::{AirLog, Grab, Landing, Trick, evaluate};
use glam::Vec3;
use std::f32::consts::PI;

pub const G: f32 = 9.81;

/// One tick's controls.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// -1 left .. 1 right: carve on the snow, spin in the air.
    pub steer: f32,
    /// -1 back .. 1 forward: tuck (forward) or brake (back) on the snow;
    /// flips in the air.
    pub pitch: f32,
    /// Held to crouch and charge, released to jump.
    pub jump: bool,
    pub grab: bool,
}

/// A rider's strengths; 1 is average.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub speed: f32,
    pub turn: f32,
    pub jump: f32,
    pub spin: f32,
    pub balance: f32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            speed: 1.0,
            turn: 1.0,
            jump: 1.0,
            spin: 1.0,
            balance: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Ground,
    Air,
    /// Tumbling, for this long so far.
    Crashed(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Jump { ollie: bool },
    Land { landing: Landing, impact: f32 },
    Trick(Trick),
    Crash { tree: bool },
    Respawn,
}

/// How long a crash lasts before the rider is up again.
pub const CRASH_TIME: f32 = 1.8;

#[derive(Clone, Debug)]
pub struct Rider {
    pub stats: Stats,
    /// The board's base on the snow.
    pub pos: Vec3,
    pub vel: Vec3,
    /// Where the board points (the direction of travel when riding), as an
    /// angle: 0 is -z, increasing to the right.
    pub yaw: f32,
    /// The flip so far (radians); 0 on the snow.
    pub pitch: f32,
    pub phase: Phase,
    pub s: f32,
    pub u: f32,
    pub normal: Vec3,
    pub switch: bool,
    /// 0 standing .. 1 deep crouch.
    pub crouch: f32,
    /// -1 .. 1, leaning into a carve.
    pub lean: f32,
    /// 0 .. 1: snow thrown up by the edge.
    pub spray: f32,
    /// 0 .. 1: braking sideways.
    pub skid: f32,
    pub tuck: f32,
    pub grab: Option<Grab>,
    pub air: AirLog,
    spin_rate: f32,
    flip_rate: f32,
    auto_turn: f32,
    auto_rate: f32,
    grab_since: f32,
    grab_held_prev: bool,
    jump_held_prev: bool,
    unstick: f32,
    takeoff_y: f32,
    pub events: Vec<Event>,
    /// 0 .. 1: how much the rider rights itself just before landing
    /// (squaring the board to the direction of travel, levelling a flip,
    /// letting go of a grab) and how forgiving the landing is. The
    /// player's option; the CPU riders ride without it.
    pub assist: f32,
}

fn dir(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, -yaw.cos())
}

fn heading_of(v: Vec3) -> f32 {
    v.x.atan2(-v.z)
}

impl Rider {
    pub fn new(course: &Course, s: f32, u: f32, stats: Stats) -> Rider {
        let g = course.ground_su(s, u);
        let f = course.frame(s);
        Rider {
            stats,
            pos: Vec3::new(f.center.x + f.right.x * u, g.height, f.center.z + f.right.z * u),
            vel: Vec3::ZERO,
            yaw: f.theta,
            pitch: 0.0,
            phase: Phase::Ground,
            s,
            u,
            normal: g.normal,
            switch: false,
            crouch: 0.0,
            lean: 0.0,
            spray: 0.0,
            skid: 0.0,
            tuck: 0.0,
            grab: None,
            air: AirLog::default(),
            spin_rate: 0.0,
            flip_rate: 0.0,
            auto_turn: 0.0,
            auto_rate: 0.0,
            grab_since: 0.0,
            grab_held_prev: false,
            jump_held_prev: false,
            unstick: 0.0,
            takeoff_y: 0.0,
            events: Vec::new(),
            assist: 0.0,
        }
    }

    pub fn speed(&self) -> f32 {
        self.vel.length()
    }

    pub fn airborne(&self) -> bool {
        self.phase == Phase::Air
    }

    /// Where the rider points, as a unit vector.
    pub fn forward(&self) -> Vec3 {
        dir(self.yaw)
    }

    /// Holds the rider still at the start gate.
    pub fn hold(&mut self, course: &Course) {
        let g = course.ground_su(self.s, self.u);
        self.pos.y = g.height;
        self.normal = g.normal;
        self.vel = Vec3::ZERO;
    }

    pub fn step(&mut self, course: &Course, input: Input, dt: f32) {
        match self.phase {
            Phase::Ground => self.ground_step(course, input, dt),
            Phase::Air => self.air_step(course, input, dt),
            Phase::Crashed(t) => self.crash_step(course, t, dt),
        }
        self.jump_held_prev = input.jump;
        self.grab_held_prev = input.grab;
        if self.phase != Phase::Air {
            self.collide(course);
        }
        let hw = course.half_width(self.s);
        if self.u.abs() > hw + crate::course::WALL - 6.0 {
            self.respawn(course);
        }
    }

    fn drag(&self) -> f32 {
        0.0056 * (1.0 - 0.2 * self.tuck) / (self.stats.speed * self.stats.speed)
    }

    fn ground_step(&mut self, course: &Course, input: Input, dt: f32) {
        let n = self.normal;
        let speed = self.speed();

        // Steering turns the board; the edge turns the rider.
        let rate = 2.7 * self.stats.turn / (1.0 + speed * 0.045);
        self.yaw = wrap_angle(self.yaw + input.steer * rate * dt);
        let brake = (-input.pitch).clamp(0.0, 1.0);
        let tuck_target = input.pitch.clamp(0.0, 1.0) * (1.0 - input.steer.abs() * 0.5);
        self.tuck += (tuck_target - self.tuck) * (6.0 * dt).min(1.0);
        self.skid += (brake - self.skid) * (8.0 * dt).min(1.0);

        let flat = dir(self.yaw);
        let fwd = (flat - n * flat.dot(n)).normalize_or(flat);
        let side = fwd.cross(n);

        // Full gravity; the snow takes the part into it below.
        self.vel += Vec3::new(0.0, -G, 0.0) * dt;
        let mut vl = self.vel.dot(fwd);
        let vs = self.vel.dot(side);
        let vn = self.vel.dot(n);

        // Riding backwards: the rider is now riding switch.
        if vl < -0.8 && speed > 1.5 {
            self.yaw = wrap_angle(self.yaw + PI);
            self.switch = !self.switch;
            return self.ground_step_tail(course, input, dt, -vl, -vs, vn, -fwd, -side);
        }

        let grip = (9.0 - 6.0 * self.skid) * self.stats.balance.sqrt();
        let vs_new = vs * (-grip * dt).exp();
        let carved = (vs * vs - vs_new * vs_new) * (0.82 - 0.6 * self.skid);
        if vl >= 0.0 {
            vl = (vl * vl + carved.max(0.0)).sqrt();
        }
        let mu = 0.045 + 0.42 * self.skid;
        let decel = mu * G * n.y.max(0.0) + self.drag() * speed * speed;
        vl = if vl > 0.0 { (vl - decel * dt).max(0.0) } else { vl * (1.0 - 3.0 * dt) };
        self.ground_step_tail(course, input, dt, vl, vs_new, vn, fwd, side);
    }

    #[allow(clippy::too_many_arguments)]
    fn ground_step_tail(
        &mut self,
        course: &Course,
        input: Input,
        dt: f32,
        vl: f32,
        vs: f32,
        vn: f32,
        fwd: Vec3,
        side: Vec3,
    ) {
        let n = self.normal;
        self.vel = fwd * vl + side * vs + n * vn;
        let speed = self.speed();
        self.spray = ((vs.abs() / 5.0) + self.skid * (speed / 8.0)).clamp(0.0, 1.0);
        let lean_target = (input.steer * (speed / 10.0).min(1.0)) * (1.0 - self.skid * 0.5);
        self.lean += (lean_target - self.lean) * (7.0 * dt).min(1.0);

        // Crouch while the button is held; jump when it is let go.
        if input.jump {
            self.crouch = (self.crouch + dt / 0.55).min(1.0);
        } else if self.jump_held_prev && self.crouch > 0.05 {
            let charge = self.crouch;
            self.vel += n * (self.stats.jump * (3.0 + 3.6 * charge));
            self.crouch = 0.3;
            self.pos += self.vel * dt;
            self.take_off(course, true);
            return;
        } else {
            self.crouch += (0.15 * self.tuck - self.crouch) * (5.0 * dt).min(1.0);
        }

        self.pos += self.vel * dt;
        let g = course.ground(self.pos, self.s);
        self.s = g.s;
        self.u = g.u;
        let gap = self.pos.y - g.height;
        let away = self.vel.dot(g.normal);
        // Leaving the snow: it falls away faster than gravity can follow
        // (a kicker's lip at once, a roller's crest over a few ticks).
        if gap > 0.15 || (away > 1.4 && gap > -0.05) {
            self.take_off(course, false);
            return;
        }
        if away > 0.0 {
            self.unstick = self.unstick * (-dt / 0.12).exp() + away;
            if self.unstick > 1.0 {
                // Give back what the snow held down over the last ticks.
                self.vel += g.normal * (self.unstick * 0.6);
                self.take_off(course, false);
                return;
            }
        } else {
            self.unstick *= (-dt / 0.12).exp();
        }
        self.pos.y = g.height;
        self.vel -= g.normal * away;
        self.normal = g.normal;
    }

    fn take_off(&mut self, course: &Course, ollie: bool) {
        self.phase = Phase::Air;
        self.unstick = 0.0;
        self.air = AirLog {
            switch_takeoff: self.switch,
            ..AirLog::default()
        };
        self.spin_rate = 0.0;
        self.flip_rate = 0.0;
        self.pitch = 0.0;
        self.grab = None;
        self.takeoff_y = self.pos.y;
        self.auto_turn = 0.0;
        self.auto_rate = 0.0;
        // Out of a pipe wall: straight up, a little inward, and the board
        // turned round by the time it is back on the wall.
        if course.pipe_weight(self.s) > 0.5 && self.u.abs() > PIPE_FLAT + 2.0 {
            let f = course.frame(self.s);
            let inward = -self.u.signum();
            let along = self.vel.dot(f.dir);
            let up = self.vel.y.max(2.0);
            self.vel = f.dir * along + f.right * (inward * 1.2) + Vec3::Y * up;
            let land_dir = f.dir * along.max(4.0) + f.right * (inward * 3.0);
            let heading = heading_of(land_dir);
            let a = wrap_angle(heading - self.yaw);
            let b = wrap_angle(heading + PI - self.yaw);
            self.auto_turn = if a.abs() <= b.abs() { a } else { b };
            let t = 2.0 * up / G;
            self.auto_rate = self.auto_turn / (t * 0.7).max(0.3);
        }
        self.events.push(Event::Jump { ollie });
    }

    fn air_step(&mut self, course: &Course, input: Input, dt: f32) {
        let speed = self.speed();
        self.vel += Vec3::new(0.0, -G, 0.0) * dt;
        self.vel -= self.vel * (self.drag() * speed * dt);

        let max_spin = 10.0 * self.stats.spin;
        let max_flip = 6.6 * self.stats.spin;
        self.spin_rate += (input.steer * max_spin - self.spin_rate) * (14.0 * dt).min(1.0);
        self.flip_rate += (input.pitch * max_flip - self.flip_rate) * (10.0 * dt).min(1.0);
        let spin = self.spin_rate * dt;
        self.air.spin += spin;
        let mut turn = spin;
        if self.auto_turn.abs() > 1e-4 {
            let step = (self.auto_rate * dt).clamp(-self.auto_turn.abs(), self.auto_turn.abs());
            self.auto_turn -= step;
            turn += step;
        }
        self.yaw = wrap_angle(self.yaw + turn);
        self.pitch += self.flip_rate * dt;
        self.air.flip += self.flip_rate * dt;
        self.air.time += dt;
        self.air.height = self.air.height.max(self.pos.y - self.takeoff_y);

        // Grabs.
        // A fresh press grabs; so does a button already down as the rider
        // leaves the snow (jump and grab pressed together).
        let fresh = !self.grab_held_prev || self.air.time < 0.06;
        if input.grab && fresh && self.grab.is_none() {
            self.grab = Some(Grab::from_stick(input.steer, input.pitch));
            self.grab_since = 0.0;
        }
        if let Some(g) = self.grab {
            if input.grab {
                self.grab_since += dt;
                if self.air.grab.is_none_or(|(_, t)| self.grab_since > t) {
                    self.air.grab = Some((g, self.grab_since));
                }
            } else {
                self.grab = None;
            }
        }
        let crouch_target = if self.grab.is_some() { 1.0 } else { 0.45 };
        self.crouch += (crouch_target - self.crouch) * (10.0 * dt).min(1.0);
        self.lean *= 1.0 - (3.0 * dt).min(1.0);
        self.spray = 0.0;
        self.skid = 0.0;

        self.pos += self.vel * dt;
        let g = course.ground(self.pos, self.s);
        self.s = g.s;
        self.u = g.u;
        if self.assist > 0.0 {
            self.right_self(&g, input, dt);
        }
        if self.pos.y <= g.height {
            self.land(g);
        }
    }

    /// The landing assist: in the last moments before the snow, with the
    /// stick let go, turn the board toward the nearer of forwards and
    /// switch, level out a flip, and drop a grab.
    fn right_self(&mut self, g: &Ground, input: Input, dt: f32) {
        let gap = (self.pos.y - g.height).max(0.0) * g.normal.y;
        let closing = -self.vel.dot(g.normal);
        if closing <= 0.5 {
            return;
        }
        let t = gap / closing;
        if t > 0.4 || self.air.time < 0.2 {
            return;
        }
        let a = self.assist;
        if t < 0.18 && self.grab.is_some() {
            self.grab = None;
        }
        let hv = Vec3::new(self.vel.x, 0.0, self.vel.z);
        if input.steer.abs() < 0.3 && hv.length() > 1.0 {
            let travel = heading_of(hv);
            let fwd = wrap_angle(travel - self.yaw);
            let back = wrap_angle(travel + PI - self.yaw);
            let err = if fwd.abs() <= back.abs() { fwd } else { back };
            let step = (err.signum() * 7.0 * a * dt).clamp(-err.abs(), err.abs());
            self.yaw = wrap_angle(self.yaw + step);
            self.air.spin += step;
            self.spin_rate *= 1.0 - (8.0 * a * dt).min(1.0);
        }
        if input.pitch.abs() < 0.3 {
            let err = -wrap_angle(self.pitch);
            let step = (err.signum() * 6.0 * a * dt).clamp(-err.abs(), err.abs());
            self.pitch += step;
            self.air.flip += step;
            self.flip_rate *= 1.0 - (8.0 * a * dt).min(1.0);
        }
    }

    fn land(&mut self, g: Ground) {
        let n = g.normal;
        let into = -self.vel.dot(n);
        let tangential = self.vel + n * into.max(0.0);
        let travel = if tangential.length() > 1.0 {
            heading_of(tangential - n * tangential.dot(n))
        } else {
            self.yaw
        };
        let d = wrap_angle(self.yaw - travel);
        let (yaw_err, backwards) = if d.abs() <= PI / 2.0 {
            (d.abs(), false)
        } else {
            (PI - d.abs(), true)
        };
        let flip_err = wrap_angle(self.pitch).abs();
        let bal = self.stats.balance;
        let short = self.air.time < 0.35;
        let forgive = 1.0 + 0.35 * self.assist;
        let tol = if short { 1.2 } else { 0.42 * bal * forgive };
        let sloppy = if short { 1.5 } else { 0.85 * bal * forgive };
        let landing = if into > 25.0 {
            Landing::Crash
        } else if yaw_err < tol && flip_err < 0.5 * forgive && self.grab.is_none() {
            Landing::Clean
        } else if yaw_err < sloppy && flip_err < 0.95 * forgive {
            Landing::Sloppy
        } else {
            Landing::Crash
        };

        self.pos.y = g.height;
        self.normal = n;
        self.pitch = 0.0;
        self.grab = None;
        self.spin_rate = 0.0;
        self.flip_rate = 0.0;
        self.events.push(Event::Land {
            landing,
            impact: into,
        });
        if landing == Landing::Crash {
            self.vel = tangential * 0.45;
            self.crash(false);
            return;
        }
        if backwards {
            self.switch = !self.switch;
        }
        self.yaw = travel;
        let keep = if landing == Landing::Clean { 0.97 } else { 0.72 };
        self.vel = tangential * keep;
        self.crouch = (0.35 + into * 0.06).min(1.0);
        self.phase = Phase::Ground;
        if let Some(t) = evaluate(&self.air, landing) {
            self.events.push(Event::Trick(t));
        }
    }

    fn crash(&mut self, tree: bool) {
        self.phase = Phase::Crashed(0.0);
        self.grab = None;
        self.crouch = 0.0;
        self.events.push(Event::Crash { tree });
    }

    fn crash_step(&mut self, course: &Course, t: f32, dt: f32) {
        let t = t + dt;
        self.vel += Vec3::new(0.0, -G, 0.0) * dt;
        let g = course.ground(self.pos + self.vel * dt, self.s);
        let n = g.normal;
        let into = self.vel.dot(n).min(0.0);
        self.vel -= n * into;
        self.vel *= (-1.6 * dt).exp();
        self.pos += self.vel * dt;
        let g = course.ground(self.pos, self.s);
        self.s = g.s;
        self.u = g.u;
        self.pos.y = self.pos.y.max(g.height);
        if self.pos.y - g.height < 0.3 {
            self.pos.y = g.height;
        }
        self.normal = g.normal;
        self.spray = (self.speed() / 10.0).min(1.0);
        self.phase = Phase::Crashed(t);
        if t >= CRASH_TIME {
            self.get_up(course);
        }
    }

    /// Up again after a crash, on the piste where the rider came to rest.
    fn get_up(&mut self, course: &Course) {
        let hw = course.half_width(self.s);
        let u = self.u.clamp(-(hw - 3.0).max(0.0), (hw - 3.0).max(0.0));
        let u = if course.pipe_weight(self.s) > 0.5 { 0.0 } else { u };
        self.place(course, self.s, u, 4.0);
    }

    /// Back onto the piste, out of bounds or stuck.
    pub fn respawn(&mut self, course: &Course) {
        let line = course.racing_line(self.s);
        self.place(course, self.s, line, 6.0);
    }

    fn place(&mut self, course: &Course, s: f32, u: f32, speed: f32) {
        let f = course.frame(s);
        let g = course.ground_su(s, u);
        self.s = s;
        self.u = u;
        self.pos = Vec3::new(f.center.x + f.right.x * u, g.height, f.center.z + f.right.z * u);
        self.normal = g.normal;
        self.yaw = f.theta;
        self.switch = false;
        self.vel = f.dir * speed;
        self.phase = Phase::Ground;
        self.pitch = 0.0;
        self.events.push(Event::Respawn);
    }

    /// Trees and rocks: a crash at speed, else a shove aside.
    fn collide(&mut self, course: &Course) {
        if matches!(self.phase, Phase::Crashed(_)) {
            return;
        }
        let mut hit = None;
        for o in course.obstacles_between(self.s - 6.0, self.s + 6.0) {
            let d = Vec3::new(self.pos.x - o.pos.x, 0.0, self.pos.z - o.pos.z);
            let r = o.radius + 0.35;
            if d.length_squared() < r * r && self.pos.y < o.pos.y + o.height {
                hit = Some((*o, d));
                break;
            }
        }
        if let Some((o, d)) = hit {
            let out = d.normalize_or(Vec3::X);
            let r = o.radius + 0.36;
            self.pos.x = o.pos.x + out.x * r;
            self.pos.z = o.pos.z + out.z * r;
            let into = self.vel.dot(out).min(0.0);
            if self.speed() > 6.0 && into < -2.5 {
                self.vel = out * 2.0 + Vec3::Y * 0.5;
                self.crash(o.kind == crate::course::ObstacleKind::Tree);
            } else {
                self.vel -= out * into;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::CourseId;

    const DT: f32 = 1.0 / 120.0;

    #[test]
    fn rides_downhill_and_gathers_speed() {
        let c = Course::new(CourseId::Crystal);
        let s0 = c.start_s + 100.0;
        let mut r = Rider::new(&c, s0, 0.0, Stats::default());
        r.vel = c.frame(s0).dir * 2.0;
        for _ in 0..(120 * 8) {
            r.step(&c, Input::default(), DT);
        }
        assert!(r.s > s0 + 50.0, "s {}", r.s);
        assert!(r.speed() > 8.0, "speed {}", r.speed());
    }

    #[test]
    fn ollie_goes_up_and_lands() {
        let c = Course::new(CourseId::Crystal);
        let mut r = Rider::new(&c, c.start_s + 100.0, 0.0, Stats::default());
        r.vel = c.frame(r.s).dir * 12.0;
        let held = Input {
            jump: true,
            ..Input::default()
        };
        for _ in 0..60 {
            r.step(&c, held, DT);
        }
        r.step(&c, Input::default(), DT);
        assert_eq!(r.phase, Phase::Air);
        let mut landed = false;
        for _ in 0..400 {
            r.step(&c, Input::default(), DT);
            if r.phase == Phase::Ground {
                landed = true;
                break;
            }
        }
        assert!(landed);
        assert!(r.events.iter().any(|e| matches!(
            e,
            Event::Land {
                landing: Landing::Clean,
                ..
            }
        )));
    }

    #[test]
    fn the_assist_saves_a_quarter_turn() {
        let c = Course::new(CourseId::Crystal);
        let mut outcomes = Vec::new();
        for assist in [0.0, 1.0] {
            let mut r = Rider::new(&c, c.start_s + 100.0, 0.0, Stats::default());
            r.assist = assist;
            r.vel = c.frame(r.s).dir * 12.0 + Vec3::Y * 6.0;
            r.pos.y += 0.5;
            r.phase = Phase::Air;
            // About 70° of spin, then the stick let go.
            let spin = Input { steer: 1.0, ..Input::default() };
            for _ in 0..14 {
                r.step(&c, spin, DT);
            }
            for _ in 0..400 {
                r.step(&c, Input::default(), DT);
                if r.phase != Phase::Air {
                    break;
                }
            }
            outcomes.push(matches!(r.phase, Phase::Crashed(_)));
        }
        assert_eq!(outcomes, vec![true, false], "crashes without the assist, lands with it");
    }

    #[test]
    fn landing_sideways_crashes() {
        let c = Course::new(CourseId::Crystal);
        let mut r = Rider::new(&c, c.start_s + 100.0, 0.0, Stats::default());
        r.vel = c.frame(r.s).dir * 12.0 + Vec3::Y * 6.0;
        r.pos.y += 0.5;
        r.phase = Phase::Air;
        let spin = Input {
            steer: 1.0,
            ..Input::default()
        };
        for _ in 0..24 {
            r.step(&c, spin, DT);
        }
        for _ in 0..400 {
            r.step(&c, Input::default(), DT);
            if r.phase != Phase::Air {
                break;
            }
        }
        assert!(matches!(r.phase, Phase::Crashed(_)), "{:?} yaw {}", r.phase, r.yaw);
    }
}
