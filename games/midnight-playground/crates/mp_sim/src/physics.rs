//! Arcade driving model for the player car (port of
//! `src/vehicles/CarPhysics.js`).
//!
//! Heading and velocity are separate: steering turns the heading (bicycle
//! model, capped by tyre grip), and tyre grip then pulls the velocity round
//! to match. Normal grip is strong enough that the car goes where it points;
//! the handbrake drops rear grip so the heading outruns the velocity and the
//! car drifts. A drift-angle assist keeps slides catchable on a keyboard.
//!
//! The car is always kept inside the road corridor (track.wallL/wallR), which
//! the scenery dresses as rock faces, guardrails, fences or barriers.
//!
//! The JS object holds its vehicle, track and spec; here the state is the
//! struct and the vehicle and track are passed to each call.

use core::f64::consts::PI;

use mp_track::Track;
use pk_math::{clamp, damp, js, kernel, lerp, smoothstep, wrap_angle};

use crate::input::Input;
use crate::vehicle::Vehicle;

/// power: m²/s³ per kg (accel = power / speed); launch: the traction limit on
/// that accel (m/s²); vmax: a speed limiter (m/s). electric: single-speed
/// motor, regenerative braking charges the boost tank. turbo: boost builds
/// with revs and throttle (lag), and scales the power. gearScale shortens
/// (< 1) or lengthens the gear ratios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarSpec {
    pub label: &'static str,
    pub blurb: &'static str,
    pub power: f64,
    pub launch: Option<f64>,
    pub vmax: Option<f64>,
    pub gear_scale: Option<f64>,
    pub turbo: bool,
    pub electric: bool,
    pub grip: f64,
    pub drift_grip: f64,
    pub mass: f64,
    pub color: u32,
}

const fn spec(
    label: &'static str,
    blurb: &'static str,
    power: f64,
    grip: f64,
    drift_grip: f64,
    mass: f64,
    color: u32,
) -> CarSpec {
    CarSpec {
        label,
        blurb,
        power,
        launch: None,
        vmax: None,
        gear_scale: None,
        turbo: false,
        electric: false,
        grip,
        drift_grip,
        mass,
        color,
    }
}

/// `CAR_SPECS`, in the JS order.
pub const CAR_SPECS: [(&str, CarSpec); 5] = [
    (
        "sports",
        spec(
            "Vento GT",
            "Balanced all-rounder",
            480.0,
            13.2,
            8.5,
            1350.0,
            0xd81e36,
        ),
    ),
    (
        "muscle",
        spec(
            "Brawler 69",
            "Big power, loose tail",
            545.0,
            12.0,
            6.8,
            1600.0,
            0x1f4fd8,
        ),
    ),
    (
        "super",
        spec(
            "Stiletto R",
            "Grip and precision",
            515.0,
            14.4,
            9.6,
            1300.0,
            0xf2b705,
        ),
    ),
    (
        "rally",
        CarSpec {
            launch: Some(10.8),
            vmax: Some(71.0),
            gear_scale: Some(0.84),
            turbo: true,
            ..spec(
                "Kestrel RS",
                "Turbo 4WD: launches hard, slides on demand",
                470.0,
                14.0,
                10.6,
                1250.0,
                0xee5a12,
            )
        },
    ),
    (
        "electric",
        CarSpec {
            launch: Some(11.8),
            vmax: Some(71.0),
            electric: true,
            ..spec(
                "Ion Arc",
                "Electric: instant torque, braking charges boost",
                500.0,
                13.6,
                8.8,
                1850.0,
                0xdfe7ee,
            )
        },
    ),
];

pub fn car_spec(kind: &str) -> Option<CarSpec> {
    CAR_SPECS.iter().find(|(k, _)| *k == kind).map(|(_, s)| *s)
}

const G: f64 = 9.81;
/// The tyres' grip on an open level's land, of the road's (D1217): the
/// circuits' run-off keeps its 0.7.
pub const OPEN_LAND_GRIP: f64 = 0.9;
const GEAR_TOP: [f64; 7] = [0.0, 15.0, 26.0, 38.0, 51.0, 64.0, 84.0];
const IDLE: f64 = 900.0;
const REDLINE: f64 = 7800.0;
/// Analogue steering: full travel asks for this much of the grip-limited turn.
pub const ANALOG_LOCK: f64 = 1.25;
/// The electric motor: rpm at the speed limiter (single reduction gear).
pub const MOTOR_MAX: f64 = 16000.0;

/// What physics reports to the rest of the game (`phys.events`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysEvent {
    Shift {
        up: bool,
    },
    /// A landing hard enough to report (impact over 2.5 m/s).
    Land {
        strength: f64,
        air: f64,
    },
    /// A wall hit at more than 1.5 m/s into it.
    Impact {
        strength: f64,
        x: f64,
        y: f64,
        z: f64,
        side: i32,
    },
    /// Every landing: the kick the JS gives the body's pitch spring
    /// (`v.pitchV -= impact * 0.02`), for the client's springs.
    Touchdown {
        impact: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CarPhysics {
    pub spec: CarSpec,
    /// The spec on shredded tyres, made at the first spiked step.
    pub spiked_spec: Option<CarSpec>,
    pub gear: i32,
    pub rpm: f64,
    pub shift_timer: f64,
    pub nitro: f64,
    pub nitro_active: bool,
    pub drifting: bool,
    pub drift_time: f64,
    /// Slip angle (rad).
    pub slip: f64,
    /// 0..1 for audio/fx.
    pub skid: f64,
    /// 0..1 wall contact.
    pub scrape: f64,
    pub air_time: f64,
    pub events: Vec<PhysEvent>,
    /// During countdown.
    pub locked: bool,
    pub wheel_base: f64,
    pub electric: bool,
    pub gear_top: [f64; 7],
    /// Turbo boost 0..1.
    pub boost: f64,
    /// Electric: drive power (+) or regen (−), kW.
    pub power_out: f64,
    /// Electric: 0..1 regen braking strength.
    pub regen: f64,
    /// Hot Pursuit. damage 0..1: past half, the engine loses power (a wreck
    /// at 1 is the race's call). spiked: seconds left on shredded tyres
    /// after a spike strip: less grip and a lower top speed. Both are 0
    /// outside Hot Pursuit, which leaves the car exactly as it was.
    pub damage: f64,
    pub spiked: f64,
    /// Missing until the first step on the ground.
    pub off_track: Option<f64>,
    /// Missing until the first scrape.
    pub scrape_side: Option<i32>,
    pub on_ground: bool,
    /// An open level's suspension (stage 7a, D1246): the body rides a
    /// spring over the ground under its four wheels. None on a closed
    /// level, whose car follows the road as the JS's does.
    pub suspension: Option<Suspension>,
}

/// The suspension's state between ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Suspension {
    /// The ground under the wheels last tick: their mean height.
    pub contact: f64,
}

/// The suspension's spring (1/s²: force per kg per metre), its damping
/// (1/s), how far it compresses past its sag before the body takes the
/// ground's speed whole (bottoming out, m), and how far the wheels hang
/// below their rest before the car is in the air (m). The sag (G / K, 3 cm)
/// is taken up so that a car at rest sits on the ground.
const SUSP_K: f64 = 320.0;
const SUSP_D: f64 = 22.0;
const SUSP_TRAVEL: f64 = 0.16;
const SUSP_HANG: f64 = 0.22;
/// The most of the ground's vertical speed the damper passes on (m/s).
const SUSP_WHEEL_VY: f64 = 2.5;
/// Where the wheels are, as fractions of the half length and half width.
const WHEEL_L: f64 = 0.72;
const WHEEL_W: f64 = 0.82;

/// A roadside barrier's face toward a place near it (stage 7c): a point on
/// it, its normal toward the place, and its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarrierFace {
    pub x: f64,
    pub z: f64,
    pub nx: f64,
    pub nz: f64,
    pub top: f64,
}

/// How thick a barrier stands, out from the road's wall line (m), and how
/// high over the road a car must be to clear it.
const BARRIER_THICK: f64 = 0.45;
const BARRIER_TOP: f64 = 1.2;

/// The faces of the barriers within `reach` of (x, z) on an open level,
/// each facing the side the place is on: from the road the face at the
/// wall line toward the road, from behind it the far face toward the land.
pub fn barrier_planes(t: &Track, x: f64, z: f64, reach: f64) -> Vec<BarrierFace> {
    let mut out = Vec::new();
    if t.edges.is_empty() {
        return out;
    }
    let r_near = reach + 12.0;
    for r in t.net.roads_near(x, z, r_near) {
        let rt = t.road(r);
        let d = rt.distance_to_road(x, z, r_near);
        let Some(s) = d.s else { continue };
        if !rt.is_loop && !(0.0..=rt.length).contains(&s) {
            continue;
        }
        let side = if d.lat < 0.0 { -1.0 } else { 1.0 };
        if !t.barrier_at(r, s, side) {
            continue;
        }
        let f = rt.frame(if rt.is_loop { rt.wrap(s) } else { s });
        let w = if side < 0.0 { f.wall_l } else { f.wall_r };
        let a = d.lat.abs();
        if (a - w - BARRIER_THICK / 2.0).abs() > reach + BARRIER_THICK {
            continue;
        }
        // Inside (the road's side of the barrier's middle) or behind it.
        let inside = a < w + BARRIER_THICK / 2.0;
        let (lat, n) = if inside {
            (w, -side)
        } else {
            (w + BARRIER_THICK, side)
        };
        out.push(BarrierFace {
            x: f.x + f.rx * side * lat,
            z: f.z + f.rz * side * lat,
            nx: f.rx * n,
            nz: f.rz * n,
            top: rt.surface_y(s, side * w) + BARRIER_TOP,
        });
    }
    out
}

/// The ground under a car's four wheels at (x, z) heading `yaw`: their
/// mean height and the surface's up normal through them.
pub fn wheel_contact(t: &Track, v: &Vehicle, x: f64, z: f64) -> (f64, [f64; 3]) {
    let (c, s) = (kernel::cos(v.yaw), kernel::sin(v.yaw));
    let (al, aw) = (v.half_l * WHEEL_L, v.half_w * WHEEL_W);
    let at = |l: f64, w: f64| {
        let (px, pz) = (x + c * l - s * w, z + s * l + c * w);
        let (k, p) = t.project_road(px, pz, v.road, v.road_s);
        t.ground_at(k, px, pz, p.s, p.lat)
    };
    let (fl, fr, bl, br) = (at(al, -aw), at(al, aw), at(-al, -aw), at(-al, aw));
    // Along: front less back over the wheelbase; across: right less left.
    let (dl, dw) = ((fl + fr - bl - br) / 2.0, (fr + br - fl - bl) / 2.0);
    let along = [c * 2.0 * al, dl, s * 2.0 * al];
    let across = [-s * 2.0 * aw, dw, c * 2.0 * aw];
    // across × along points up (x right, z forward in the car's frame).
    let n = [
        across[1] * along[2] - across[2] * along[1],
        across[2] * along[0] - across[0] * along[2],
        across[0] * along[1] - across[1] * along[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
    (
        (fl + fr + bl + br) / 4.0,
        [n[0] / len, n[1] / len, n[2] / len],
    )
}

impl CarPhysics {
    pub fn new(vehicle: &Vehicle, spec: CarSpec) -> CarPhysics {
        let electric = spec.electric;
        let scale = spec.gear_scale.unwrap_or(1.0);
        CarPhysics {
            spec,
            spiked_spec: None,
            gear: 1,
            rpm: if electric { 0.0 } else { IDLE },
            shift_timer: 0.0,
            nitro: 0.5,
            nitro_active: false,
            drifting: false,
            drift_time: 0.0,
            slip: 0.0,
            skid: 0.0,
            scrape: 0.0,
            air_time: 0.0,
            events: Vec::new(),
            locked: false,
            wheel_base: js::or(vehicle.dims.wheel_base, 2.6),
            electric,
            gear_top: GEAR_TOP.map(|v| v * scale),
            boost: 0.0,
            power_out: 0.0,
            regen: 0.0,
            damage: 0.0,
            spiked: 0.0,
            off_track: None,
            scrape_side: None,
            on_ground: false,
            suspension: None,
        }
    }

    /// [`CarPhysics::reset`] on road `road` of the network (stage 6f).
    pub fn reset_on(&mut self, v: &mut Vehicle, track: &Track, road: usize, s: f64, lat: f64) {
        self.reset(v, track, s, lat);
        if road != 0 {
            v.place_on(track, road, s, lat, 0.0);
        }
    }

    pub fn reset(&mut self, v: &mut Vehicle, track: &Track, s: f64, lat: f64) {
        v.place(track, s, lat, 0.0);
        self.gear = 1;
        self.rpm = if self.electric { 0.0 } else { IDLE };
        self.drifting = false;
        self.on_ground = true;
        v.on_ground = true;
        self.suspension = None;
        v.ground_normal = None;
    }

    /// Equal steps of about 1/120 s that add up to exactly the frame's time. A
    /// fixed step with a carried-over remainder took 1, 2 or 3 steps per 60 Hz
    /// frame (and 0 or 1 at 144 Hz) with nothing drawn in between, so at speed
    /// the car lurched against the camera, the traffic and the rivals, which all
    /// move by the frame's own time. A step may run 10 % long, so a 60 Hz frame
    /// a hair over 1/60 s is still two steps, not three.
    pub fn update(&mut self, v: &mut Vehicle, t: &Track, frame_dt: f64, inp: &Input) {
        let h = 1.0 / 120.0;
        // `!(frameDt > 0)`: NaN steps nothing, as in the JS.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(frame_dt > 0.0) {
            return;
        }
        let n = js::min(12.0, (frame_dt / (h * 1.1)).ceil());
        let hh = js::min(h * 1.1, frame_dt / n);
        for _ in 0..n as usize {
            self.step(v, t, hh, inp);
        }
    }

    pub fn engine_accel(&self, v: f64) -> f64 {
        let spec = &self.spec;
        let mut a = js::min(spec.launch.unwrap_or(9.5), spec.power / js::max(v, 5.0));
        if spec.turbo {
            a *= 0.8 + 0.2 * self.boost;
        }
        if let Some(vmax) = spec.vmax.filter(|&m| m != 0.0) {
            a *= 1.0 - smoothstep(vmax - 5.0, vmax, v);
        }
        if self.damage > 0.5 {
            a *= 1.0 - 0.35 * smoothstep(0.5, 1.0, self.damage);
        }
        if self.spiked > 0.0 {
            let vm = spec.vmax.unwrap_or(76.0) * 0.75;
            a *= 1.0 - smoothstep(vm - 5.0, vm, v);
        }
        a
    }

    pub fn step(&mut self, v: &mut Vehicle, t: &Track, dt: f64, inp: &Input) {
        let mut spec = self.spec;
        if self.spiked > 0.0 {
            self.spiked = js::max(0.0, self.spiked - dt);
            let base = self.spec;
            spec = *self.spiked_spec.get_or_insert(CarSpec {
                grip: base.grip * 0.7,
                drift_grip: base.drift_grip * 0.7,
                ..base
            });
        }
        // The road under the car (a branch of the level's road, or the
        // road itself): its frame and surface are what the car drives.
        let rt = v.project_onto(t);
        let f = rt.frame(v.road_s);

        let mut fx = kernel::cos(v.yaw);
        let mut fz = kernel::sin(v.yaw);
        let mut rx = -fz;
        let mut rz = fx;
        let mut v_long = v.vx * fx + v.vz * fz;
        let mut v_lat = v.vx * rx + v.vz * rz;
        let speed = kernel::hypot(v.vx, v.vz);

        // Steering: less lock at speed. Analogue steering (a thumb stick, tilt,
        // a gamepad) is scaled to the grip instead: full travel asks for a
        // quarter more turn than the tyres hold at this speed, so the whole
        // stick steers. With a fixed lock, past about 100 km/h the first fifth
        // of a phone's stick (a centimetre of thumb) already turned as hard as
        // the tyres allow, and the rest did nothing. Not in a slide, which
        // needs the lock to catch.
        let mut steer_max = lerp(0.6, 0.15, smoothstep(0.0, 65.0, speed));
        if inp.analog && !self.drifting && !(inp.handbrake && speed > 9.0) {
            let yaw_max = ANALOG_LOCK * spec.grip * 1.5 / js::max(speed, 4.0);
            steer_max = js::min(
                steer_max,
                kernel::atan(yaw_max * self.wheel_base / js::max(speed, 1.0)),
            );
        }
        let target = inp.steer * steer_max;
        v.steer_angle += clamp(target - v.steer_angle, -3.2 * dt, 3.2 * dt);

        // Gearbox (automatic). Mostly for sound and the shift cut. inp.cruise
        // short-shifts and holds taller gears, like an automatic at part throttle.
        self.shift_timer = js::max(0.0, self.shift_timer - dt);
        let up_rpm = if inp.cruise { 3800.0 } else { 7350.0 };
        let down_k = if inp.cruise { 0.45 } else { 0.7 };
        let gt = self.gear_top;
        if v_long < -0.5 && inp.brake > 0.0 {
            self.gear = -1;
        } else if self.gear == -1 && v_long > -0.2 {
            self.gear = 1;
        }
        if self.electric {
            // One fixed reduction: motor speed follows the wheels, no shifts.
            if self.gear != -1 {
                self.gear = 1;
            }
            let want = if self.locked {
                0.0
            } else {
                js::min(
                    MOTOR_MAX * 1.02,
                    (v_long.abs() / spec.vmax.unwrap_or(f64::NAN)) * MOTOR_MAX,
                )
            };
            self.rpm = damp(self.rpm, want, 30.0, dt);
        } else if self.gear >= 1 {
            let g = self.gear as usize;
            let ratio = v_long.abs() / gt[g];
            let mut rpm = IDLE + ratio * (REDLINE - IDLE);
            rpm = js::max(
                rpm,
                IDLE + inp.throttle * 3800.0 * (1.0 - smoothstep(4.0, 14.0, speed)),
            );
            if self.locked {
                rpm = IDLE + inp.throttle * 6200.0;
            }
            self.rpm = damp(self.rpm, js::min(rpm, REDLINE + 150.0), 18.0, dt);
            if !self.locked
                && self.rpm > up_rpm
                && self.gear < 6
                && self.shift_timer == 0.0
                && v_long.abs() > gt[g] * (down_k + 0.08)
            {
                self.gear += 1;
                self.shift_timer = 0.2;
                self.events.push(PhysEvent::Shift { up: true });
            } else if self.gear > 1 && v_long.abs() < gt[g - 1] * down_k && self.shift_timer == 0.0
            {
                self.gear -= 1;
                self.shift_timer = 0.12;
                self.events.push(PhysEvent::Shift { up: false });
            }
        } else {
            self.rpm = damp(self.rpm, IDLE + v_long.abs() / 12.0 * 4000.0, 10.0, dt);
        }

        // Turbo: boost follows revs under throttle, with lag; lifting dumps it
        // (the audio plays the blow-off from the drop).
        if spec.turbo {
            let want = inp.throttle
                * smoothstep(2600.0, 5200.0, self.rpm)
                * (if self.shift_timer > 0.08 { 0.3 } else { 1.0 });
            self.boost = damp(
                self.boost,
                want,
                if want > self.boost { 2.2 } else { 9.0 },
                dt,
            );
        }

        if self.locked {
            v.vx = 0.0;
            v.vz = 0.0;
            v.yaw_rate = 0.0;
            self.nitro_active = false;
            self.skid = 0.0;
            self.power_out = 0.0;
            return;
        }

        let on_ground = v.on_ground;
        self.nitro_active = false;
        if on_ground {
            // ── Longitudinal ─────────────────────────────────────────
            let mut a = 0.0;
            let throttle = if self.shift_timer > 0.08 {
                inp.throttle * 0.2
            } else {
                inp.throttle
            };
            if v_long > -0.5 {
                if inp.brake > 0.0 && v_long > 0.8 {
                    a -= 15.0 * inp.brake;
                } else if inp.brake > 0.0 && v_long > -12.0 {
                    a -= 7.0 * inp.brake;
                }
                a += throttle * self.engine_accel(v_long.abs());
            } else if inp.throttle > 0.0 {
                a += 14.0 * inp.throttle;
            } else if inp.brake > 0.0 && v_long > -13.0 {
                a -= 6.0 * inp.brake;
            }
            if inp.nitro && self.nitro > 0.0 && v_long > 4.0 && inp.throttle > 0.1 {
                a += 6.0;
                self.nitro = js::max(0.0, self.nitro - dt / 6.0);
                self.nitro_active = true;
            }
            if self.electric {
                // Regenerative braking: the motor does some of the slowing and
                // tops up the boost; lifting off regens gently too.
                let moving = smoothstep(3.0, 18.0, v_long);
                let coast = if inp.throttle < 0.05 && inp.brake < 0.05 {
                    0.25
                } else {
                    0.0
                };
                let want = if v_long > 3.0 {
                    js::max(inp.brake, coast) * moving
                } else {
                    0.0
                };
                self.regen = damp(self.regen, want, 12.0, dt);
                self.nitro = js::min(1.0, self.nitro + dt * 0.075 * self.regen);
                let drive = if v_long > -0.5 {
                    throttle * self.engine_accel(v_long.abs())
                        + (if self.nitro_active { 6.0 } else { 0.0 })
                } else {
                    0.0
                };
                self.power_out = damp(
                    self.power_out,
                    (spec.mass * (drive - self.regen * 7.0) * js::max(0.0, v_long)) / 1000.0,
                    14.0,
                    dt,
                );
            }
            a -= 0.00115 * v_long * v_long.abs() + 0.01 * v_long;
            // Circuit run-off (surveyed tracks carry run-off grades): off the
            // tarmac the tyres plough through dirt and dry grass. Paved run-off
            // drives like the road.
            // An open level: past the shoulder the car is on the land.
            let mut off = if rt.run_l.is_some() {
                clamp((v.road_lat.abs() - f.hw - 0.4) / 1.5, 0.0, 1.0)
            } else if rt.open_ground.is_some() {
                clamp((v.road_lat.abs() - f.hw - f.margin) / 1.5, 0.0, 1.0)
            } else {
                0.0
            };
            if off > 0.0
                && let Some(loose_at) = &rt.loose_at
            {
                off *= loose_at(v.x, v.z);
            }
            self.off_track = Some(off);
            if off > 0.0 {
                a -= off
                    * js::sign(v_long)
                    * js::min(v_long.abs() * 4.0, 1.2 + 0.0025 * v_long * v_long);
            }
            if inp.handbrake {
                a -= js::sign(v_long) * 3.0;
            }
            if inp.throttle < 0.05 && inp.brake < 0.05 {
                a -= js::sign(v_long) * js::min(0.9, v_long.abs());
            }
            let grade_along = f.grade * (fx * f.fx + fz * f.fz);
            a -= G * grade_along / (1.0 + grade_along * grade_along).sqrt();
            let before = v_long;
            v_long += a * dt;
            // Braking or coasting through zero stops the car rather than rolling
            // it the other way; holding the brake from a standstill reverses.
            let reversing = inp.brake > 0.05 && before.abs() < 0.05;
            if inp.throttle < 0.05
                && !reversing
                && js::sign(before) != js::sign(v_long)
                && before.abs() < 1.0
            {
                v_long = 0.0;
            }
            v.accel_long = a;
            v.brake_light = if inp.brake > 0.0 && v_long > 0.5 {
                1.0
            } else {
                0.0
            };

            // ── Yaw ──────────────────────────────────────────────────
            let slip = kernel::atan2(v_lat, js::max(v_long.abs(), 0.5));
            // Nearly stopped (e.g. nose in a wall): wheelspin still lets you pivot.
            let v_yaw = if v_long.abs() < 2.5 && (inp.throttle > 0.3 || inp.brake > 0.3) {
                js::sign(js::or(v_long, 1.0))
                    * 2.5
                    * (if inp.brake > 0.3 && v_long <= 0.5 {
                        -1.0
                    } else {
                        1.0
                    })
            } else {
                v_long
            };
            let mut yaw_target = v_yaw * kernel::tan(v.steer_angle) / self.wheel_base;
            let hb = inp.handbrake && speed > 9.0;
            if hb {
                yaw_target = yaw_target * 1.6
                    + js::sign(v.steer_angle) * 0.35 * smoothstep(9.0, 20.0, speed);
            }
            // Arcade grip: ~2 g of cornering before the tyres give up.
            let grip_lim = (if self.drifting || hb {
                spec.grip * 1.8
            } else {
                spec.grip * 1.5
            }) / js::max(speed, 4.0);
            yaw_target = clamp(yaw_target, -grip_lim, grip_lim);
            // Drift assist: stop the slide angle running away past ~35°.
            if self.drifting || hb {
                let max_slip = 0.62;
                let room = max_slip - slip.abs();
                if js::sign(yaw_target) == -js::sign(slip) && slip != 0.0 {
                    let omega_v = spec.drift_grip / js::max(speed, 5.0);
                    let cap = js::max(0.0, omega_v + 3.0 * room);
                    yaw_target = js::sign(yaw_target) * js::min(yaw_target.abs(), cap);
                }
            }
            let resp = if self.drifting { 5.0 } else { 10.0 };
            v.yaw_rate = damp(v.yaw_rate, yaw_target, resp, dt);
            // Rotate heading, keep the world velocity, re-express it.
            let wvx = v_long * fx + v_lat * rx;
            let wvz = v_long * fz + v_lat * rz;
            v.yaw += v.yaw_rate * dt;
            fx = kernel::cos(v.yaw);
            fz = kernel::sin(v.yaw);
            rx = -fz;
            rz = fx;
            v_long = wvx * fx + wvz * fz;
            v_lat = wvx * rx + wvz * rz;

            // ── Lateral grip ─────────────────────────────────────────
            let grip_a = (if hb {
                4.5
            } else if self.drifting {
                spec.drift_grip * 1.15
            } else {
                spec.grip * 2.2
            }) * (1.0 - 0.3 * off);
            // An open level's land is not a circuit's gravel trap: the
            // tyres bite on it nearly as on the road, and a slide that
            // leaves the tarmac scrubs out instead of running on (D1217).
            let grip_a = if rt.open_ground.is_some() && rt.run_l.is_none() && off > 0.0 {
                let land = spec.grip * 2.2 * OPEN_LAND_GRIP;
                grip_a + (js::max(grip_a, land) - grip_a) * off
            } else {
                grip_a
            };
            let dv = js::min(v_lat.abs(), grip_a * dt);
            v_lat -= js::sign(v_lat) * dv;
            if hb || self.drifting {
                v_long += js::sign(v_long) * dv * 0.3; // keep some momentum
            }
            v.accel_lat = v.yaw_rate * speed;

            self.slip = kernel::atan2(v_lat, js::max(v_long.abs(), 0.5));
            if !self.drifting && speed > 12.0 && self.slip.abs() > 0.18 {
                self.drifting = true;
            }
            if self.drifting && (self.slip.abs() < 0.06 || speed < 7.0) {
                self.drifting = false;
            }
            if self.drifting {
                self.drift_time += dt;
                // (The electric car tops up mostly from regen, so drifting pays half.)
                self.nitro = js::min(
                    1.0,
                    self.nitro
                        + dt * 0.1
                            * (if self.electric { 0.5 } else { 1.0 })
                            * clamp(self.slip.abs() / 0.4, 0.3, 1.2)
                            * smoothstep(12.0, 30.0, speed),
                );
            } else {
                self.drift_time = 0.0;
            }
            self.skid = clamp(
                js::max_n(&[
                    v_lat.abs() / 5.0,
                    if hb { speed / 25.0 } else { 0.0 },
                    if inp.brake > 0.5 && speed > 18.0 {
                        0.35
                    } else {
                        0.0
                    },
                ]),
                0.0,
                1.0,
            ) * smoothstep(3.0, 8.0, speed);

            v.vx = v_long * fx + v_lat * rx;
            v.vz = v_long * fz + v_lat * rz;
        } else {
            // Airborne: ballistic, keep spin, a hint of air control.
            v.yaw_rate *= 1.0 - 0.8 * dt;
            v.yaw += (v.yaw_rate + inp.steer * 0.25) * dt;
            v.vx *= 1.0 - 0.02 * dt;
            v.vz *= 1.0 - 0.02 * dt;
            self.skid = 0.0;
            self.air_time += dt;
            v.accel_long = 0.0;
            v.accel_lat = 0.0;
        }
        v.speed = v.vx * kernel::cos(v.yaw) + v.vz * kernel::sin(v.yaw);

        // ── Integrate position ─────────────────────────────────────
        let prev_ground = rt.ground_y(v.x, v.z, v.road_s, v.road_lat);
        v.x += v.vx * dt;
        v.z += v.vz * dt;
        let mut rt = v.project_onto(t);
        // An open level has no walls: the car may leave the road, but not
        // drive through its buildings.
        if t.open_ground.is_none() {
            self.collide_walls(v, t, dt);
        } else {
            if !t.buildings.is_empty() && self.collide_buildings(v, t, dt) {
                rt = v.project_onto(t);
            }
            if self.collide_barriers(v, t, dt) {
                rt = v.project_onto(t);
            }
            if !t.props.is_empty() && self.collide_props(v, t, dt) {
                rt = v.project_onto(t);
            }
        }

        if t.open_ground.is_some() {
            self.suspend(v, t, dt);
            return;
        }

        // Vertical: follow the road unless it falls away faster than gravity.
        let ground = rt.ground_y(v.x, v.z, v.road_s, v.road_lat);
        let ground_vy = (ground - prev_ground) / dt;
        v.vy -= G * dt;
        let y_new = v.y + v.vy * dt;
        if y_new <= ground + 0.001 {
            if !v.on_ground {
                let impact = ground_vy - v.vy;
                if impact > 2.5 {
                    self.events.push(PhysEvent::Land {
                        strength: clamp(impact / 12.0, 0.0, 1.0),
                        air: self.air_time,
                    });
                }
                self.events.push(PhysEvent::Touchdown { impact });
                self.air_time = 0.0;
            }
            v.y = ground;
            v.vy = ground_vy;
            v.on_ground = true;
        } else {
            // Road falling away faster than gravity. Suspension travel keeps the
            // tyres on the road for the first 30 cm; past that we're flying.
            v.y = y_new;
            if v.on_ground && y_new - ground > 0.3 {
                v.on_ground = false;
            }
        }
        v.vis_y = Some(if v.on_ground { ground } else { v.y });
    }

    /// An open level's vertical (stage 7a, D1246): the body on a spring
    /// over the ground under its four wheels, which it meets as a car does.
    /// The road's own step (the JS's) gives the body the ground's vertical
    /// speed whole each tick, so every crest and kerb threw the car in the
    /// air; here the spring takes a rise up and the wheels follow a fall,
    /// and only a bump past the suspension's travel kicks the body, only a
    /// drop past its hang leaves the car in the air.
    fn suspend(&mut self, v: &mut Vehicle, t: &Track, dt: f64) {
        let (c, normal) = wheel_contact(t, v, v.x, v.z);
        let prev = self.suspension.map_or(c, |s| s.contact);
        self.suspension = Some(Suspension { contact: c });
        v.ground_normal = Some(normal);
        // The ground's own vertical speed under the car.
        let cvy = (c - prev) / dt;
        let sag = G / SUSP_K;
        let was = v.on_ground;
        // Compression past the wheels' rest (the sag at rest).
        let x = c + sag - v.y;
        let mut a = -G;
        if x > 0.0 {
            // The damper meets the ground's speed through the tyre and the
            // wheel's own give: a square step's speed (a kerb's 15 cm in a
            // tick is 18 m/s) is not a kick through it.
            let wheel = clamp(cvy, -SUSP_WHEEL_VY, SUSP_WHEEL_VY);
            a += SUSP_K * x - SUSP_D * (v.vy - wheel);
            // A spring pushes; it does not pull the body down.
            a = a.max(-G);
        }
        v.vy += a * dt;
        v.y += v.vy * dt;
        // Bottomed out: the body rides the ground's rise.
        let floor = c - SUSP_TRAVEL;
        let mut impact = 0.0;
        if v.y < floor {
            impact = js::max(0.0, cvy - v.vy);
            v.y = floor;
            v.vy = js::max(v.vy, cvy);
        }
        let reach = c + sag + SUSP_HANG;
        v.on_ground = v.y <= reach;
        if v.on_ground && !was {
            let hit = js::max(impact, cvy - v.vy);
            if hit > 2.5 {
                self.events.push(PhysEvent::Land {
                    strength: clamp(hit / 12.0, 0.0, 1.0),
                    air: self.air_time,
                });
            }
            self.events.push(PhysEvent::Touchdown { impact: hit });
            self.air_time = 0.0;
        } else if v.on_ground && impact > 2.5 {
            self.events.push(PhysEvent::Touchdown { impact });
        }
        v.vis_y = Some(v.y);
    }

    /// Pushes the car out of the building it has run into, the way a
    /// wall does (docs/vision/peninsula.md, stage 6e): out along the
    /// shortest way, the speed into the face bounced a little and the
    /// rest scrubbed, the nose swung along the face, an impact when it is
    /// hard and the scrape while it rubs. Returns whether it moved the car
    /// (the caller projects it again).
    pub fn collide_buildings(&mut self, v: &mut Vehicle, t: &Track, dt: f64) -> bool {
        self.scrape = js::max(0.0, self.scrape - dt * 4.0);
        let (c, s) = (kernel::cos(v.yaw), kernel::sin(v.yaw));
        let Some(p) = t.buildings.push_out(v.x, v.z, c, s, v.half_w, v.half_l) else {
            return false;
        };
        let b = &t.buildings.list[p.id];
        // Over the roof (a jump), nothing.
        if v.y > b.y + b.height {
            return false;
        }
        self.meet_face(v, p.nx, p.nz, p.depth, dt, false);
        true
    }

    /// The open world's solid scenery (stage 7d, D1248): the car stops
    /// against every trunk, fence and house off the road the way it does
    /// against a building, unless it is over it. Twice, for a car wedged
    /// between two. Returns whether it moved the car.
    pub fn collide_props(&mut self, v: &mut Vehicle, t: &Track, dt: f64) -> bool {
        let mut moved = false;
        for _ in 0..2 {
            let (c, s) = (kernel::cos(v.yaw), kernel::sin(v.yaw));
            let Some(p) = t.props.push_out(t, v.x, v.z, v.y, c, s, v.half_w, v.half_l) else {
                break;
            };
            // A trunk turns the car only when it glances off it: square on,
            // swinging the nose along its tangent would steer the car round.
            let round = matches!(p.solid.shape, mp_track::Shape::Round { .. });
            self.meet_face(v, p.nx, p.nz, p.depth, dt, round);
            moved = true;
        }
        moved
    }

    /// The car against a face whose outward normal is (nx, nz), `depth`
    /// into it: out along the normal, the speed into the face bounced a
    /// little and the rest scrubbed, the nose swung along the face, an
    /// impact when it is hard and the scrape while it rubs. `glance`: the
    /// nose swings only when the car meets the face at a glance (under 45
    /// degrees off it).
    fn meet_face(&mut self, v: &mut Vehicle, nx: f64, nz: f64, depth: f64, dt: f64, glance: bool) {
        let (c, s) = (kernel::cos(v.yaw), kernel::sin(v.yaw));
        v.x += nx * depth;
        v.z += nz * depth;
        // Which side of the car the face is on: +1 right, -1 left.
        let side = if -nx * -s + -nz * c >= 0.0 { 1 } else { -1 };
        let vn = -(v.vx * nx + v.vz * nz); // into the face
        if vn > 0.0 {
            let e = 0.25;
            v.vx += nx * vn * (1.0 + e);
            v.vz += nz * vn * (1.0 + e);
            let scrub = 1.0 - clamp(0.05 + vn * 0.025, 0.0, 0.35);
            v.vx *= scrub;
            v.vz *= scrub;
            // Swing the nose along the face, toward whichever way along
            // it the car already points.
            let face = kernel::atan2(nx, -nz);
            let mut rel = wrap_angle(v.yaw - face);
            if rel.abs() > PI / 2.0 {
                rel = wrap_angle(rel - PI);
            }
            if !glance || rel.abs() < PI / 4.0 {
                v.yaw -= rel * clamp(vn * 0.06, 0.05, 0.5);
                if v.yaw_rate * rel > 0.0 {
                    v.yaw_rate *= 0.5;
                }
            }
            if vn > 1.5 {
                self.events.push(PhysEvent::Impact {
                    strength: clamp(vn / 18.0, 0.0, 1.0),
                    x: v.x - nx * v.half_w,
                    y: v.y + 0.5,
                    z: v.z - nz * v.half_w,
                    side,
                });
            }
        }
        let sp = kernel::hypot(v.vx, v.vz);
        if sp > 1.0 {
            let k = js::max(0.0, 1.0 - 4.0 * dt / sp);
            v.vx *= k;
            v.vz *= k;
            self.scrape = js::min(1.0, 0.4 + sp / 60.0);
            self.scrape_side = Some(side);
        }
    }

    /// The open world's roadside barriers (stage 7c, D1247): the car stops
    /// against every fence, concrete barrier, railing and rock face or
    /// guardrail the road kit draws, from the road or from the land behind
    /// it (a car that left by a gap meets the fence's back). Returns
    /// whether it moved the car.
    pub fn collide_barriers(&mut self, v: &mut Vehicle, t: &Track, dt: f64) -> bool {
        if t.buildings.is_empty() {
            // `collide_buildings` lets the scrape fade where there are any.
            self.scrape = js::max(0.0, self.scrape - dt * 4.0);
        }
        let (c, s) = (kernel::cos(v.yaw), kernel::sin(v.yaw));
        let mut moved = false;
        for b in barrier_planes(t, v.x, v.z, v.half_l + 1.0) {
            // The car's reach toward the face, and how far it is into it.
            let ext =
                v.half_l * (b.nx * c + b.nz * s).abs() + v.half_w * (b.nx * -s + b.nz * c).abs();
            let d = (v.x - b.x) * b.nx + (v.z - b.z) * b.nz;
            let depth = ext - d;
            if depth > 0.0 && v.y < b.top {
                self.meet_face(v, b.nx, b.nz, depth, dt, false);
                moved = true;
            }
        }
        moved
    }

    pub fn collide_walls(&mut self, v: &mut Vehicle, t: &Track, dt: f64) {
        let f = t.frame(v.s);
        let track_yaw = kernel::atan2(f.fz, f.fx);
        let rel = wrap_angle(v.yaw - track_yaw);
        let ext = v.half_w * kernel::cos(rel).abs() + v.half_l * kernel::sin(rel).abs();
        let lim_r = f.wall_r - ext;
        let lim_l = -(f.wall_l - ext);
        let mut side = 0i32;
        let mut pen = 0.0;
        if v.lat > lim_r {
            side = 1;
            pen = v.lat - lim_r;
        } else if v.lat < lim_l {
            side = -1;
            pen = v.lat - lim_l;
        }
        self.scrape = js::max(0.0, self.scrape - dt * 4.0);
        if side != 0 {
            let sd = side as f64;
            v.x -= f.rx * pen;
            v.z -= f.rz * pen;
            v.lat -= pen;
            // Walls stand only on a closed level, whose one road this is.
            v.road_lat = v.lat;
            let vn = (v.vx * f.rx + v.vz * f.rz) * sd; // into the wall
            if vn > 0.0 {
                let e = 0.25;
                v.vx -= f.rx * sd * vn * (1.0 + e);
                v.vz -= f.rz * sd * vn * (1.0 + e);
                // Scrub speed along the wall and swing the nose parallel.
                let scrub = 1.0 - clamp(0.05 + vn * 0.025, 0.0, 0.35);
                v.vx *= scrub;
                v.vz *= scrub;
                let points_in = js::sign(rel) == sd && rel.abs() < PI / 2.0;
                if points_in {
                    v.yaw -= rel * clamp(vn * 0.06, 0.05, 0.5);
                }
                // The wall stops the touching corner swinging further into it (a
                // tail slap kills the spin), but never the rotation back toward
                // parallel: steering a nose-in car off the wall.
                if v.yaw_rate * rel > 0.0 {
                    v.yaw_rate *= 0.5;
                }
                if vn > 1.5 {
                    let px = v.x + f.rx * sd * ext;
                    let pz = v.z + f.rz * sd * ext;
                    self.events.push(PhysEvent::Impact {
                        strength: clamp(vn / 18.0, 0.0, 1.0),
                        x: px,
                        y: v.y + 0.5,
                        z: pz,
                        side,
                    });
                }
            }
            // Continuous scraping friction.
            let sp = kernel::hypot(v.vx, v.vz);
            if sp > 1.0 {
                let k = js::max(0.0, 1.0 - 4.0 * dt / sp);
                v.vx *= k;
                v.vz *= k;
                self.scrape = js::min(1.0, 0.4 + sp / 60.0);
                self.scrape_side = Some(side);
            }
        }
        // Ends of the road (loops have none).
        if t.is_loop {
            return;
        }
        if v.s < 1.0 {
            v.x += f.fx * (1.0 - v.s);
            v.z += f.fz * (1.0 - v.s);
            let vf = v.vx * f.fx + v.vz * f.fz;
            if vf < 0.0 {
                v.vx -= f.fx * vf;
                v.vz -= f.fz * vf;
            }
        }
        if v.s > t.road_end() - 3.0 {
            let vf = v.vx * f.fx + v.vz * f.fz;
            if vf > 0.0 {
                v.vx -= f.fx * vf;
                v.vz -= f.fz * vf;
            }
        }
    }

    pub fn speed_kmh(&self, v: &Vehicle) -> f64 {
        v.speed.abs() * 3.6
    }
}
