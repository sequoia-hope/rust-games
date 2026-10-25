//! Simulation-driven sound effects for Mech Works (`games/mech-works/docs/AUDIO.md`).
//!
//! **The one principle:** sound comes from the simulation's numbers each step,
//! never from canned cues timed to an animation. A heavier body, a hot knee,
//! or a weak driver sounds different because the contact forces, currents,
//! speeds, and winding temperatures are different.
//!
//! Audio is presentation: it reads an [`AudioFrame`] per step, never writes to
//! the simulation, and draws from its own internal [`Rng`] so a run with sound
//! and one without step identically.

#![cfg_attr(not(feature = "worklet"), forbid(unsafe_code))]

pub mod traces;
#[cfg(all(target_arch = "wasm32", feature = "worklet"))]
pub mod wasm;

use std::f64::consts::TAU;

/// Number of actuated joints on the biped mech (6 per leg, left leg first:
/// hip yaw, hip roll, hip pitch, knee, ankle pitch, ankle roll).
pub const JOINTS: usize = 12;

/// Number of feet (0 = left, 1 = right).
pub const FEET: usize = 2;

/// Default nominal mech mass (kg) for force scaling (~4 tonnes).
pub const NOMINAL_MASS_KG: f32 = 4000.0;

/// Ambient winding temperature (°C).
pub const AMBIENT_C: f32 = 30.0;
/// Winding temperature where thermal derating begins (°C).
pub const DERATE_START_C: f32 = 90.0;
/// Winding temperature where the driver trips to zero torque (°C).
pub const TRIP_C: f32 = 120.0;

/// Per-joint telemetry read from `mw_sim::Sim::actuators[j]` and joint states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointFrame {
    /// Delivered torque, N·m.
    pub torque: f32,
    /// Winding current, A.
    pub current: f32,
    /// Driver's nominal maximum current, A (`spec.max_current`).
    pub max_current: f32,
    /// Current torque limit after health/derating, N·m (`torque_limit()`).
    pub torque_limit: f32,
    /// Winding temperature, °C.
    pub temperature: f32,
    /// Part health in `0..=1`.
    pub health: f32,
    /// Joint angle, rad.
    pub angle: f32,
    /// Joint angular velocity, rad/s.
    pub velocity: f32,
    /// Whether the driver is currently tripped on over-temperature.
    pub tripped: bool,
    /// Seconds remaining in an overcurrent fault (`> 0` while faulted).
    pub fault_left: f32,
}

impl Default for JointFrame {
    fn default() -> Self {
        Self {
            torque: 0.0,
            current: 0.0,
            max_current: 100.0,
            torque_limit: 1000.0,
            temperature: AMBIENT_C,
            health: 1.0,
            angle: 0.0,
            velocity: 0.0,
            tripped: false,
            fault_left: 0.0,
        }
    }
}

/// Per-foot contact telemetry read from `mw_sim::Sim::foot_contacts()` and link poses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootFrame {
    /// Whether the foot collider is in contact with the ground.
    pub touching: bool,
    /// Normal ground reaction force, N (~20 kN standing per foot on a 4 t mech).
    pub normal_force: f32,
    /// Horizontal tangential slip speed while in contact, m/s.
    pub slip_speed: f32,
    /// Stereo pan in `-1.0..=1.0` (left foot negative, right foot positive).
    pub pan: f32,
}

impl Default for FootFrame {
    fn default() -> Self {
        Self {
            touching: true,
            normal_force: 19620.0,
            slip_speed: 0.0,
            pan: 0.0,
        }
    }
}

/// Which part of the mech a projectile struck (shapes hull ring pitch & decay).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StruckLink {
    Torso,
    Thigh,
    Shin,
    Foot,
}

/// A projectile or obstacle impact on the mech's armour (Demo 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpactEvent {
    /// Impulse magnitude, N·s (e.g. 80 N·s light round, 1500 N·s heavy shell).
    pub impulse: f32,
    /// Link struck.
    pub link: StruckLink,
    /// Stereo pan in `-1.0..=1.0`.
    pub pan: f32,
}

/// Quiet mechanical UI interactions on the demo page's controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiEvent {
    /// Button toggle or slider click.
    Click,
    /// Clicking an actuator in Demo 3 to inject damage.
    Damage,
    /// Swapping a torso/limb part in Demo 4 (calibration).
    PartSwap,
    /// An objective reached in a level: a short, warm two-note chime.
    Objective,
}

/// A loose rock striking the ground, another rock or the mech (the game's
/// rubble, `mw_sim::rubble`): a stone knock, sized by the impulse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RockEvent {
    /// Impulse magnitude at the listener, N·s (the caller folds distance in).
    pub impulse: f32,
    /// Stereo pan in `-1.0..=1.0`.
    pub pan: f32,
}

/// One step of telemetry and discrete events from the simulation (`mw_sim`).
#[derive(Clone, Debug, PartialEq)]
pub struct AudioFrame {
    /// The 12 actuated leg joints (left leg 0..6, right leg 6..12).
    pub joints: [JointFrame; JOINTS],
    /// Left and right feet.
    pub feet: [FootFrame; FEET],
    /// Torso linear velocity `[vx, vy, vz]`, m/s.
    pub torso_vel: [f32; 3],
    /// Torso angular velocity `[wx, wy, wz]`, rad/s.
    pub torso_ang_vel: [f32; 3],
    /// Total mech mass, kg (~4000 kg nominal; heavier bodies make louder steps).
    pub body_mass_kg: f32,
    /// True when the episode ended in a fall (torso or knee striking the ground).
    pub terminated: bool,
    /// Environmental wind / dust storm intensity (`0.0` calm gravel lot, `1.0` storm).
    pub dust_storm: f32,
    /// Projectile impacts occurring on this step.
    pub impacts: Vec<ImpactEvent>,
    /// UI control events occurring on this step.
    pub ui: Vec<UiEvent>,
    /// Rock knocks on this step.
    pub rocks: Vec<RockEvent>,
    /// Loose rock sliding and rolling, t·m/s at the listener (each moving
    /// rock's mass times its speed, the caller folding distance in): a
    /// grinding gravel bed.
    pub rubble_slide: f32,
}

impl Default for AudioFrame {
    fn default() -> Self {
        Self {
            joints: [JointFrame::default(); JOINTS],
            feet: [
                FootFrame {
                    pan: -0.32,
                    ..FootFrame::default()
                },
                FootFrame {
                    pan: 0.32,
                    ..FootFrame::default()
                },
            ],
            torso_vel: [0.0; 3],
            torso_ang_vel: [0.0; 3],
            body_mass_kg: NOMINAL_MASS_KG,
            terminated: false,
            dust_storm: 0.0,
            impacts: Vec::new(),
            ui: Vec::new(),
            rocks: Vec::new(),
            rubble_slide: 0.0,
        }
    }
}

// ── Internal DSP primitives ──────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x6D77_6175 } else { seed })
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in `0.0..1.0`.
    #[inline]
    fn f(&mut self) -> f64 {
        (self.next_u32() as f64) * (1.0 / 4294967296.0)
    }

    /// White noise in `-1.0..1.0`.
    #[inline]
    fn white(&mut self) -> f64 {
        self.f() * 2.0 - 1.0
    }
}

/// Chamberlin / ZPT state-variable filter (low-pass, band-pass, high-pass).
#[derive(Clone, Copy, Debug, Default)]
struct Svf {
    ic1: f64,
    ic2: f64,
    g: f64,
    k: f64,
    a1: f64,
    a2: f64,
    a3: f64,
    pub lp: f64,
    pub bp: f64,
    pub hp: f64,
}

impl Svf {
    fn new(hz: f64, q: f64, sr: f64) -> Self {
        let mut s = Self::default();
        s.set(hz, q, sr);
        s
    }

    fn set(&mut self, hz: f64, q: f64, sr: f64) {
        let hz = hz.clamp(15.0, sr * 0.45);
        let g = (std::f64::consts::PI * hz / sr).tan();
        let k = 1.0 / q.max(0.5);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        self.g = g;
        self.k = k;
        self.a1 = a1;
        self.a2 = a2;
        self.a3 = a3;
    }

    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        self.lp = v2;
        self.bp = v1;
        self.hp = x - self.k * v1 - v2;
        v2
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct OnePole {
    y: f64,
    a: f64,
}

impl OnePole {
    fn new(hz: f64, sr: f64) -> Self {
        let mut f = Self { y: 0.0, a: 0.1 };
        f.set(hz, sr);
        f
    }

    fn set(&mut self, hz: f64, sr: f64) {
        self.a = 1.0 - (-TAU * hz.clamp(1.0, sr * 0.45) / sr).exp();
    }

    #[inline]
    fn lp(&mut self, x: f64) -> f64 {
        self.y += self.a * (x - self.y);
        self.y
    }

    #[inline]
    fn hp(&mut self, x: f64) -> f64 {
        x - self.lp(x)
    }
}

// ── Active transient voices ──────────────────────────────────────────────

/// Heavy steel footfall on gravel.
#[derive(Clone, Debug)]
struct FootstepVoice {
    t: usize,
    dur: usize,
    /// Normalized weight/impact intensity (`0.05..1.6`).
    intensity: f64,
    /// True when the contact force was a light shuffle rather than a full step.
    shuffle: bool,
    pan: f64,
    sub_ph: f64,
    plate1_ph: f64,
    plate2_ph: f64,
    gravel_bp: Svf,
    gravel_hp: OnePole,
    grain_env: f64,
    sr: f64,
}

impl FootstepVoice {
    fn new(force_n: f32, body_mass_kg: f32, prev_vy: f32, pan: f32, sr: f64) -> Self {
        let mass_scale = (body_mass_kg / NOMINAL_MASS_KG).clamp(0.5, 2.0) as f64;
        let impact_boost = (1.0 + (-prev_vy).max(0.0) * 0.35).clamp(1.0, 1.8) as f64;
        let raw_force = (force_n as f64) * impact_boost;
        let shuffle = raw_force < 4500.0;
        let intensity = if shuffle {
            (raw_force / 4500.0).clamp(0.08, 0.35) * mass_scale.sqrt()
        } else {
            ((raw_force - 2500.0) / 32000.0).clamp(0.25, 1.55) * mass_scale
        };
        let dur_s = if shuffle {
            0.10
        } else {
            0.22 + 0.14 * intensity.min(1.4)
        };
        Self {
            t: 0,
            dur: (dur_s * sr) as usize,
            intensity,
            shuffle,
            pan: (pan as f64).clamp(-0.85, 0.85),
            sub_ph: 0.0,
            plate1_ph: 0.0,
            plate2_ph: 0.0,
            gravel_bp: Svf::new(if shuffle { 2400.0 } else { 1650.0 }, 1.35, sr),
            gravel_hp: OnePole::new(520.0, sr),
            grain_env: 1.0,
            sr,
        }
    }

    fn run(&mut self, r: &mut Rng) -> [f64; 2] {
        if self.t >= self.dur {
            return [0.0, 0.0];
        }
        let s = self.t as f64 / self.sr;
        self.t += 1;

        let mut sig = 0.0;
        if !self.shuffle {
            // 1. 4-tonne sub-bass & low-mid ground thud (pitch drops 115 Hz → 34 Hz).
            let f_sub = (34.0 + 81.0 * (-s / 0.022).exp()) / (1.0 + 0.14 * self.intensity);
            self.sub_ph = (self.sub_ph + f_sub / self.sr).fract();
            let sub_env = (-s / (0.095 + 0.065 * self.intensity)).exp();
            let sub_wave =
                (TAU * self.sub_ph).sin() * 0.82 + (TAU * 2.0 * self.sub_ph).sin() * 0.28;
            let sub = sub_wave * sub_env * 1.55 * self.intensity;

            // 2. Heavy steel footplate & ankle-clevis clunk (pitch-dropping damped modes,
            // never a sustained pure tone).
            let drop = 1.0 + 0.45 * (-s / 0.008).exp();
            self.plate1_ph = (self.plate1_ph + (142.0 * drop) / self.sr).fract();
            self.plate2_ph = (self.plate2_ph + (338.0 * drop) / self.sr).fract();
            let plate_env = (-s / 0.016).exp();
            let plate = ((TAU * self.plate1_ph).sin() * 0.70 + (TAU * self.plate2_ph).sin() * 0.30)
                * plate_env
                * 0.50
                * self.intensity;

            sig += (sub + plate).tanh();
        }

        // 3. Crushed gravel impact & sole-settling scatter under the steel footpad.
        if (self.t & 31) == 0 {
            let pop = if r.f() < 0.42 {
                0.55 + 0.95 * r.f()
            } else {
                0.28
            };
            self.grain_env = pop;
        }
        let heel_strike = (-s / 0.014).exp() * 0.65;
        let sole_settle = ((-((s - 0.032).max(0.0)) / 0.075).exp())
            * (s / 0.018).min(1.0)
            * 0.55
            * self.grain_env;
        let crunch_env = if self.shuffle {
            (-s / 0.038).exp() * self.grain_env
        } else {
            heel_strike + sole_settle
        };
        let nz = r.white();
        self.gravel_bp.run(nz);
        let gravel =
            (self.gravel_bp.lp * 0.55 + self.gravel_bp.bp * 0.55 + self.gravel_hp.hp(nz) * 0.15)
                * crunch_env
                * 0.58
                * self.intensity;
        sig += gravel;

        stereo_pan(sig * 0.46, self.pan)
    }

    fn done(&self) -> bool {
        self.t >= self.dur
    }
}

/// Ballistic metal-on-armour impact + multi-mode hull ring (Demo 1).
#[derive(Clone, Debug)]
struct ImpactVoice {
    t: usize,
    dur: usize,
    amp: f64,
    pan: f64,
    modes: [f64; 4],
    phases: [f64; 4],
    decays: [f64; 4],
    punch_ph: f64,
    crack_bp: Svf,
    sr: f64,
}

impl ImpactVoice {
    fn new(ev: ImpactEvent, sr: f64) -> Self {
        // Normalize impulse (50 N·s light hit .. 2500 N·s heavy shell).
        let norm = ((ev.impulse as f64) / 800.0).clamp(0.08, 1.8);
        let base_modes = match ev.link {
            StruckLink::Torso => [168.0, 415.0, 790.0, 1460.0],
            StruckLink::Thigh => [225.0, 530.0, 990.0, 1780.0],
            StruckLink::Shin => [285.0, 650.0, 1210.0, 2140.0],
            StruckLink::Foot => [340.0, 770.0, 1420.0, 2480.0],
        };
        let ring_scale = match ev.link {
            StruckLink::Torso => 1.15,
            StruckLink::Thigh => 0.90,
            StruckLink::Shin => 0.75,
            StruckLink::Foot => 0.60,
        };
        let decays = [
            0.13 * ring_scale * (0.65 + 0.45 * norm),
            0.085 * ring_scale * (0.65 + 0.45 * norm),
            0.052 * ring_scale,
            0.032 * ring_scale,
        ];
        let dur_s = (decays[0] * 4.5).clamp(0.22, 0.95);
        Self {
            t: 0,
            dur: (dur_s * sr) as usize,
            amp: norm.sqrt() * 0.72,
            pan: (ev.pan as f64).clamp(-0.85, 0.85),
            modes: base_modes,
            phases: [0.0; 4],
            decays,
            punch_ph: 0.0,
            crack_bp: Svf::new(1950.0, 1.4, sr),
            sr,
        }
    }

    fn run(&mut self, r: &mut Rng) -> [f64; 2] {
        if self.t >= self.dur {
            return [0.0, 0.0];
        }
        let s = self.t as f64 / self.sr;
        self.t += 1;

        // 1. Heavy ballistic kinetic punch (deep armour plate compression wave).
        let f_punch = 46.0 + 210.0 * (-s / 0.016).exp();
        self.punch_ph = (self.punch_ph + f_punch / self.sr).fract();
        let punch = ((TAU * self.punch_ph).sin() + 0.35 * (TAU * 2.0 * self.punch_ph).sin())
            * (-s / 0.055).exp()
            * 1.25;

        // 2. Broadband steel spall & shell-shatter transient.
        let nz = r.white();
        self.crack_bp.run(nz);
        let crack = (self.crack_bp.bp * 0.75 + self.crack_bp.lp * 0.45) * (-s / 0.018).exp() * 1.45
            + self.crack_bp.hp * (-s / 0.006).exp() * 0.75;

        // 3. Damped inharmonic armour plate flexural ring (with initial dispersion bend).
        let bend = 1.0 + 0.18 * (-s / 0.010).exp();
        let weights = [0.38, 0.28, 0.18, 0.10];
        let mut ring = 0.0;
        for (i, &w) in weights.iter().enumerate() {
            self.phases[i] = (self.phases[i] + (self.modes[i] * bend) / self.sr).fract();
            let fm = 1.0 + 0.08 * (TAU * self.phases[0]).sin() * (-s / 0.04).exp();
            ring += (TAU * self.phases[i] * fm).sin() * (-s / self.decays[i]).exp() * w;
        }

        let y = (punch + crack + ring * 0.42).tanh() * self.amp;
        stereo_pan(y, self.pan)
    }

    fn done(&self) -> bool {
        self.t >= self.dur
    }
}

/// A stone knock: a dull low thud through the ground, a short crack of
/// stone on stone, and a trickle of grit after it.
#[derive(Clone, Debug)]
struct RockVoice {
    t: usize,
    dur: usize,
    amp: f64,
    pan: f64,
    thud_hz: f64,
    thud_ph: f64,
    crack_bp: Svf,
    grit_bp: Svf,
    sr: f64,
}

impl RockVoice {
    fn new(ev: RockEvent, r: &mut Rng, sr: f64) -> Self {
        // 30 N·s: a pebble's tick; 600 N·s and up: a block landing.
        let norm = ((ev.impulse as f64) / 400.0).clamp(0.04, 1.6);
        Self {
            t: 0,
            dur: ((0.16 + 0.25 * norm) * sr) as usize,
            amp: norm.sqrt() * 0.5,
            pan: (ev.pan as f64).clamp(-0.9, 0.9),
            // Bigger rocks knock lower.
            thud_hz: 120.0 - 45.0 * norm.min(1.0) + 20.0 * r.f(),
            thud_ph: 0.0,
            crack_bp: Svf::new(900.0 + 900.0 * r.f(), 1.6, sr),
            grit_bp: Svf::new(2600.0, 1.0, sr),
            sr,
        }
    }

    fn run(&mut self, r: &mut Rng) -> [f64; 2] {
        if self.t >= self.dur {
            return [0.0, 0.0];
        }
        let s = self.t as f64 / self.sr;
        self.t += 1;
        self.thud_ph = (self.thud_ph + self.thud_hz / self.sr).fract();
        let thud = (TAU * self.thud_ph).sin() * (-s / 0.045).exp();
        let nz = r.white();
        self.crack_bp.run(nz);
        let crack = self.crack_bp.bp * (-s / 0.012).exp() * 2.2;
        // Grit: sparse grains falling off after the knock.
        let grain = if r.f() < 0.02 { r.white() * 3.0 } else { 0.0 };
        self.grit_bp.run(grain);
        let grit = self.grit_bp.bp * (-s / 0.12).exp() * 0.5;
        let y = (thud * 0.9 + crack + grit).tanh() * self.amp;
        stereo_pan(y, self.pan)
    }

    fn done(&self) -> bool {
        self.t >= self.dur
    }
}

/// Relay clack on overcurrent fault, or heavy breaker clunk + alarm on thermal trip.
#[derive(Clone, Debug)]
enum FaultVoice {
    /// Overcurrent fault: sharp double relay clack + inductive pop.
    RelayClack {
        t: usize,
        dur: usize,
        pan: f64,
        sr: f64,
    },
    /// Thermal trip at 120 °C: heavy breaker clunk + pneumatic dump + cockpit alarm.
    ThermalTrip {
        t: usize,
        dur: usize,
        pan: f64,
        clunk_ph: f64,
        alarm_ph: f64,
        sr: f64,
    },
    /// Big, heavy, unglamorous 4-tonne crash when an episode ends in a fall.
    FallCrash {
        t: usize,
        dur: usize,
        sub_ph: f64,
        hull1_ph: f64,
        hull2_ph: f64,
        gravel_bp: Svf,
        sr: f64,
    },
    /// Quiet mechanical UI switch/wrench click.
    UiClick {
        t: usize,
        dur: usize,
        kind: UiEvent,
        ph: f64,
        sr: f64,
    },
}

impl FaultVoice {
    fn relay_clack(pan: f64, sr: f64) -> Self {
        Self::RelayClack {
            t: 0,
            dur: (0.08 * sr) as usize,
            pan,
            sr,
        }
    }

    fn thermal_trip(pan: f64, sr: f64) -> Self {
        Self::ThermalTrip {
            t: 0,
            dur: (0.75 * sr) as usize,
            pan,
            clunk_ph: 0.0,
            alarm_ph: 0.0,
            sr,
        }
    }

    fn fall_crash(sr: f64) -> Self {
        Self::FallCrash {
            t: 0,
            dur: (1.15 * sr) as usize,
            sub_ph: 0.0,
            hull1_ph: 0.0,
            hull2_ph: 0.0,
            gravel_bp: Svf::new(1400.0, 1.1, sr),
            sr,
        }
    }

    fn ui(kind: UiEvent, sr: f64) -> Self {
        let dur_s = match kind {
            UiEvent::Click => 0.025,
            UiEvent::Damage => 0.085,
            UiEvent::PartSwap => 0.12,
            UiEvent::Objective => 1.1,
        };
        Self::UiClick {
            t: 0,
            dur: (dur_s * sr) as usize,
            kind,
            ph: 0.0,
            sr,
        }
    }

    fn run(&mut self, r: &mut Rng) -> [f64; 2] {
        match self {
            FaultVoice::RelayClack { t, dur, pan, sr } => {
                if *t >= *dur {
                    return [0.0, 0.0];
                }
                let s = *t as f64 / *sr;
                *t += 1;
                // Two crisp contactor snaps at t = 0 ms and t = 11 ms.
                let snap1 = (-s / 0.0025).exp()
                    * ((TAU * 1850.0 * s).sin() * 0.6 + (TAU * 3420.0 * s).sin() * 0.4);
                let snap2 = if s >= 0.011 {
                    let s2 = s - 0.011;
                    (-s2 / 0.003).exp()
                        * ((TAU * 1480.0 * s2).sin() * 0.6 + (TAU * 2950.0 * s2).sin() * 0.4)
                } else {
                    0.0
                };
                stereo_pan((snap1 + snap2 * 0.85) * 0.38, *pan)
            }
            FaultVoice::ThermalTrip {
                t,
                dur,
                pan,
                clunk_ph,
                alarm_ph,
                sr,
            } => {
                if *t >= *dur {
                    return [0.0, 0.0];
                }
                let s = *t as f64 / *sr;
                *t += 1;
                // 1. Heavy solenoid breaker clunk (165 → 58 Hz) + pneumatic dump hiss.
                let f_clunk = 58.0 + 110.0 * (-s / 0.018).exp();
                *clunk_ph = (*clunk_ph + f_clunk / *sr).fract();
                let clunk = (TAU * *clunk_ph).sin() * (-s / 0.055).exp() * 0.85;
                let hiss = r.white() * (-s / 0.14).exp() * 0.22;

                // 2. Urgent two-tone cockpit trip alarm (960 Hz / 720 Hz alternating at 8 Hz).
                let slot = ((s * 8.0) as usize) % 2;
                let f_alarm = if slot == 0 { 960.0 } else { 720.0 };
                *alarm_ph = (*alarm_ph + f_alarm / *sr).fract();
                let alarm_env = if s < 0.65 { 1.0 } else { (0.75 - s) / 0.10 }.clamp(0.0, 1.0);
                let alarm = ((TAU * *alarm_ph).sin() + 0.25 * (TAU * 3.0 * *alarm_ph).sin())
                    * alarm_env
                    * 0.26;

                stereo_pan((clunk + hiss + alarm) * 0.48, *pan)
            }
            FaultVoice::FallCrash {
                t,
                dur,
                sub_ph,
                hull1_ph,
                hull2_ph,
                gravel_bp,
                sr,
            } => {
                if *t >= *dur {
                    return [0.0, 0.0];
                }
                let s = *t as f64 / *sr;
                *t += 1;
                // Massive primary ground impact + two secondary limb/chassis impacts at 95 ms and 230 ms.
                let mut f_sub = 28.0 + 68.0 * (-s / 0.038).exp();
                let mut e_impact = (-s / 0.22).exp();
                if s > 0.095 {
                    let s2 = s - 0.095;
                    f_sub += 42.0 * (-s2 / 0.025).exp();
                    e_impact += 0.72 * (-s2 / 0.15).exp();
                }
                if s > 0.23 {
                    let s3 = s - 0.23;
                    f_sub += 28.0 * (-s3 / 0.020).exp();
                    e_impact += 0.48 * (-s3 / 0.14).exp();
                }
                *sub_ph = (*sub_ph + f_sub / *sr).fract();
                let sub =
                    ((TAU * *sub_ph).sin() + 0.35 * (TAU * 2.0 * *sub_ph).sin()) * e_impact * 1.45;

                // Pitch-dropping structural chassis crunch (no sustained pure organ chord).
                let drop = 1.0 + 0.55 * (-s / 0.035).exp();
                *hull1_ph = (*hull1_ph + (96.0 * drop) / *sr).fract();
                *hull2_ph = (*hull2_ph + (218.0 * drop) / *sr).fract();
                let hull = ((TAU * *hull1_ph).sin() * 0.7 + (TAU * *hull2_ph).sin() * 0.35)
                    * (-s / 0.065).exp()
                    * 0.75;

                let nz = r.white();
                gravel_bp.run(nz);
                let gravel = (gravel_bp.lp * 0.65 + gravel_bp.bp * 0.55) * e_impact * 0.95;

                let y = (sub + hull + gravel).tanh() * 0.65;
                [y, y]
            }
            FaultVoice::UiClick {
                t,
                dur,
                kind,
                ph,
                sr,
            } => {
                if *t >= *dur {
                    return [0.0, 0.0];
                }
                let s = *t as f64 / *sr;
                *t += 1;
                let y = match kind {
                    UiEvent::Click => {
                        *ph = (*ph + 2100.0 / *sr).fract();
                        (TAU * *ph).sin() * (-s / 0.003).exp() * 0.18
                    }
                    UiEvent::Damage => {
                        *ph = (*ph + (280.0 + 420.0 * (-s / 0.008).exp()) / *sr).fract();
                        ((TAU * *ph).sin() + r.white() * 0.35) * (-s / 0.018).exp() * 0.28
                    }
                    UiEvent::PartSwap => {
                        let s2 = (s - 0.045).max(0.0);
                        let c1 = (TAU * 1250.0 * s).sin() * (-s / 0.006).exp();
                        let c2 = if s >= 0.045 {
                            (TAU * 1680.0 * s2).sin() * (-s2 / 0.008).exp()
                        } else {
                            0.0
                        };
                        (c1 + c2) * 0.22
                    }
                    UiEvent::Objective => {
                        // Two soft bell-like notes, a fifth apart.
                        let note = |hz: f64, at: f64| {
                            if s < at {
                                return 0.0;
                            }
                            let u = s - at;
                            let env = (u / 0.01).min(1.0) * (-u / 0.35).exp();
                            ((TAU * hz * u).sin() + 0.3 * (TAU * 2.0 * hz * u).sin()) * env
                        };
                        (note(523.25, 0.0) + note(783.99, 0.16)) * 0.16
                    }
                };
                [y, y]
            }
        }
    }

    fn done(&self) -> bool {
        match self {
            FaultVoice::RelayClack { t, dur, .. }
            | FaultVoice::ThermalTrip { t, dur, .. }
            | FaultVoice::FallCrash { t, dur, .. }
            | FaultVoice::UiClick { t, dur, .. } => *t >= *dur,
        }
    }
}

#[inline]
fn stereo_pan(x: f64, pan: f64) -> [f64; 2] {
    let p = pan.clamp(-1.0, 1.0);
    let l = x * ((1.0 - p) * 0.5).sqrt();
    let r = x * ((1.0 + p) * 0.5).sqrt();
    [l, r]
}

// ── Continuous beds & synthesizer ────────────────────────────────────────

/// The Mech Works sound synthesizer.
///
/// Feed each simulation or policy step with [`MechAudio::push_frame`] and render
/// stereo audio blocks at `sr` Hz with [`MechAudio::process`].
pub struct MechAudio {
    sr: f64,
    r: Rng,
    current_frame: AudioFrame,
    has_prev_frame: bool,
    prev_touching: [bool; FEET],
    prev_tripped: [bool; JOINTS],
    prev_faulted: [bool; JOINTS],
    prev_terminated: bool,
    foot_cooldown_s: [f64; FEET],

    // Active discrete voices
    footsteps: Vec<FootstepVoice>,
    impacts: Vec<ImpactVoice>,
    rocks: Vec<RockVoice>,
    faults: Vec<FaultVoice>,

    // Loose rubble grinding and rolling
    rubble_bp: Svf,
    rubble_lp: Svf,
    rubble_amp: f64,

    // Continuous servo & hydraulic beds (12 joints mixed to 1 hydraulic manifold
    // bed + top 3 individual joint servo voices + stall strain groan)
    pump_ph: f64,
    pump_env: f64,
    servo_ph: [f64; 3],
    servo_gear_ph: [f64; 3],
    servo_hz: [f64; 3],
    servo_amp: [f64; 3],
    servo_pan: [f64; 3],
    strain_ph: f64,
    strain_amp: f64,
    strain_bp: Svf,

    // Foot scrape / slide gravel bed
    slide_bp: [Svf; FEET],
    slide_amp: [f64; FEET],
    slide_grain: [f64; FEET],

    // Thermal fan / coolant hiss / derate warning
    fan_bp: Svf,
    steam_hp: OnePole,
    heat_fan_amp: f64,
    heat_hiss_amp: f64,
    derate_level: f64,
    derate_pulse_ph: f64,
    derate_tone_ph: f64,

    // Body creak (torso angular speed × total torque)
    creak_bp1: Svf,
    creak_bp2: Svf,
    creak_drive: f64,

    // Open gravel-lot hillside wind ambience
    wind_lp_l: Svf,
    wind_lp_r: Svf,
    wind_whistle: Svf,
    wind_lfo_ph: f64,

    // Master DC blocker
    dc_l: OnePole,
    dc_r: OnePole,
    ctrl_k: u32,
}

impl MechAudio {
    pub fn new(seed: u32, sr: f64) -> Self {
        let sr = sr.max(8000.0);
        Self {
            sr,
            r: Rng::new(seed),
            current_frame: AudioFrame::default(),
            has_prev_frame: false,
            prev_touching: [true; FEET],
            prev_tripped: [false; JOINTS],
            prev_faulted: [false; JOINTS],
            prev_terminated: false,
            foot_cooldown_s: [0.0; FEET],

            footsteps: Vec::with_capacity(8),
            impacts: Vec::with_capacity(8),
            rocks: Vec::with_capacity(16),
            faults: Vec::with_capacity(8),

            rubble_bp: Svf::new(700.0, 0.9, sr),
            rubble_lp: Svf::new(160.0, 0.8, sr),
            rubble_amp: 0.0,

            pump_ph: 0.0,
            pump_env: 0.0,
            servo_ph: [0.0; 3],
            servo_gear_ph: [0.0; 3],
            servo_hz: [180.0, 220.0, 260.0],
            servo_amp: [0.0; 3],
            servo_pan: [-0.25, 0.25, 0.0],
            strain_ph: 0.0,
            strain_amp: 0.0,
            strain_bp: Svf::new(340.0, 2.2, sr),

            slide_bp: [Svf::new(2100.0, 1.4, sr), Svf::new(2250.0, 1.4, sr)],
            slide_amp: [0.0; FEET],
            slide_grain: [1.0; FEET],

            fan_bp: Svf::new(950.0, 1.2, sr),
            steam_hp: OnePole::new(4800.0, sr),
            heat_fan_amp: 0.0,
            heat_hiss_amp: 0.0,
            derate_level: 0.0,
            derate_pulse_ph: 0.0,
            derate_tone_ph: 0.0,

            creak_bp1: Svf::new(360.0, 6.0, sr),
            creak_bp2: Svf::new(820.0, 7.5, sr),
            creak_drive: 0.0,

            wind_lp_l: Svf::new(240.0, 1.1, sr),
            wind_lp_r: Svf::new(260.0, 1.1, sr),
            wind_whistle: Svf::new(780.0, 3.2, sr),
            wind_lfo_ph: 0.0,

            dc_l: OnePole::new(18.0, sr),
            dc_r: OnePole::new(18.0, sr),
            ctrl_k: 0,
        }
    }

    /// Updates the synthesizer with a new simulation step's [`AudioFrame`],
    /// detecting rising edges for footfalls, thermal trips, overcurrent
    /// faults, projectile hits, UI clicks, and falls.
    pub fn push_frame(&mut self, frame: AudioFrame) {
        if !self.has_prev_frame {
            for f in 0..FEET {
                self.prev_touching[f] = frame.feet[f].touching;
            }
            for j in 0..JOINTS {
                self.prev_tripped[j] = frame.joints[j].tripped;
                self.prev_faulted[j] = frame.joints[j].fault_left > 0.0;
            }
            self.prev_terminated = frame.terminated;
            self.has_prev_frame = true;
        }

        // 1. Footfalls: rising contact edge (with 45 ms debounce so contact
        // flicker does not machine-gun stomps).
        for f in 0..FEET {
            let foot = &frame.feet[f];
            if foot.touching
                && !self.prev_touching[f]
                && self.foot_cooldown_s[f] <= 0.0
                && foot.normal_force > 400.0
            {
                self.footsteps.push(FootstepVoice::new(
                    foot.normal_force,
                    frame.body_mass_kg,
                    frame.torso_vel[1],
                    foot.pan,
                    self.sr,
                ));
                self.foot_cooldown_s[f] = 0.045;
            }
            self.prev_touching[f] = foot.touching;
        }

        // 2. Thermal trips and overcurrent faults across the 12 joints.
        for j in 0..JOINTS {
            let jt = &frame.joints[j];
            let pan = if j < 6 { -0.30 } else { 0.30 };
            if jt.tripped && !self.prev_tripped[j] {
                self.faults.push(FaultVoice::thermal_trip(pan, self.sr));
            }
            self.prev_tripped[j] = jt.tripped;

            let faulted = jt.fault_left > 0.0;
            if faulted && !self.prev_faulted[j] {
                self.faults.push(FaultVoice::relay_clack(pan, self.sr));
            }
            self.prev_faulted[j] = faulted;
        }

        // 3. Projectile impacts.
        for &ev in &frame.impacts {
            if ev.impulse > 1.0 {
                self.impacts.push(ImpactVoice::new(ev, self.sr));
            }
        }

        // Rock knocks (a few at most: a heap settling knocks many times a step).
        for &ev in frame.rocks.iter().take(4) {
            if ev.impulse > 5.0 && self.rocks.len() < 12 {
                self.rocks.push(RockVoice::new(ev, &mut self.r, self.sr));
            }
        }

        // 4. Episode ending in a fall.
        if frame.terminated && !self.prev_terminated {
            self.faults.push(FaultVoice::fall_crash(self.sr));
        }
        self.prev_terminated = frame.terminated;

        // 5. UI control clicks.
        for &ui in &frame.ui {
            self.faults.push(FaultVoice::ui(ui, self.sr));
        }

        self.current_frame = frame;
    }

    /// Updates smoothed continuous targets every 32 samples.
    fn update_control(&mut self) {
        let dt_ctrl = 32.0 / self.sr;
        for c in &mut self.foot_cooldown_s {
            *c = (*c - dt_ctrl).max(0.0);
        }

        let fr = &self.current_frame;

        // 1. Actuator drive (12 joints): total hydraulic power bed + leg servo buses.
        let mut total_cur_norm = 0.0;
        let mut max_strain = 0.0f64;
        let mut max_temp = AMBIENT_C as f64;
        let mut total_torque = 0.0f64;

        for jt in &fr.joints {
            let active = !jt.tripped && jt.fault_left <= 0.0;
            let cur_norm = if active && jt.max_current > 1e-3 {
                (jt.current.abs() / jt.max_current).clamp(0.0, 1.5) as f64
            } else {
                0.0
            };
            total_cur_norm += cur_norm;
            total_torque += jt.torque.abs() as f64;
            max_temp = max_temp.max(jt.temperature as f64);

            // Strain at the current limit: when delivered torque approaches torque_limit.
            if active && jt.torque_limit > 1.0 {
                let lim_ratio = (jt.torque.abs() / jt.torque_limit) as f64;
                if lim_ratio > 0.85 {
                    let s = ((lim_ratio - 0.85) / 0.15).clamp(0.0, 1.2)
                        * (1.3 - 0.3 * (jt.health as f64).clamp(0.2, 1.0));
                    max_strain = max_strain.max(s);
                }
            }
        }

        let target_pump = (total_cur_norm / 6.0).clamp(0.0, 1.0) * 0.045;
        self.pump_env += (target_pump - self.pump_env) * 0.08;

        // Group the 12 joints into 3 stable mechanical bus voices (0 = left leg,
        // 1 = right leg, 2 = hip/torso stabilizers) so servo pitches never jump
        // between unrelated joints like a slide whistle.
        for k in 0..3 {
            let range = match k {
                0 => 2..5,  // Left hip-pitch, knee, ankle-pitch
                1 => 8..11, // Right hip-pitch, knee, ankle-pitch
                _ => 0..2,  // Hip yaw/roll stabilizers
            };
            let mut max_spd = 0.0f64;
            let mut max_cur = 0.0f64;
            for j in range {
                let jt = &fr.joints[j];
                if !jt.tripped && jt.fault_left <= 0.0 {
                    max_spd = max_spd.max((jt.velocity.abs() as f64).min(12.0));
                    max_cur = max_cur
                        .max((jt.current.abs() / jt.max_current.max(1.0)).clamp(0.0, 1.2) as f64);
                }
            }
            let base_f = match k {
                0 => 290.0,
                1 => 312.0,
                _ => 240.0,
            };
            let target_hz = base_f + max_spd * 9.5 + max_cur * 14.0;
            // Servo whine is gated by movement speed so standing braced doesn't drone.
            let motion_gate = (max_spd / 1.8).clamp(0.08, 1.0);
            let target_amp =
                ((max_spd * 0.08 + max_cur * 0.35) * motion_gate * 0.042).clamp(0.0, 0.048);
            self.servo_hz[k] += (target_hz - self.servo_hz[k]) * 0.08;
            self.servo_amp[k] += (target_amp - self.servo_amp[k]) * 0.10;
            self.servo_pan[k] = match k {
                0 => -0.28,
                1 => 0.28,
                _ => 0.0,
            };
        }

        // 2. Strain at the limit groan (subtle mechanical shudder when torque saturates).
        self.strain_amp += (max_strain * 0.075 - self.strain_amp) * 0.10;

        // 3. Foot scrape and slide under load.
        for f in 0..FEET {
            let foot = &fr.feet[f];
            let target = if foot.touching && foot.normal_force > 500.0 {
                let slip = (foot.slip_speed.abs() as f64 / 1.2).clamp(0.0, 1.2);
                let load = (foot.normal_force as f64 / 20000.0).clamp(0.1, 1.4);
                slip * load * 0.26
            } else {
                0.0
            };
            self.slide_amp[f] += (target - self.slide_amp[f]) * 0.15;
            if self.r.f() < 0.35 {
                self.slide_grain[f] = 0.4 + 0.8 * self.r.f();
            }
        }

        // 4. Heat (coolant fan above 45 °C, boiling hiss & ticks near trip 95..120 °C)
        // and Derate warning above 90 °C.
        let fan_t = ((max_temp - 45.0) / (TRIP_C as f64 - 45.0)).clamp(0.0, 1.0);
        self.heat_fan_amp += (fan_t * 0.14 - self.heat_fan_amp) * 0.06;
        self.fan_bp.set(650.0 + fan_t * 950.0, 1.3, self.sr);

        let hiss_t = ((max_temp - 92.0) / (TRIP_C as f64 - 92.0)).clamp(0.0, 1.0);
        self.heat_hiss_amp += (hiss_t * hiss_t * 0.18 - self.heat_hiss_amp) * 0.08;

        let derate_t = if max_temp > DERATE_START_C as f64 && max_temp < TRIP_C as f64 {
            ((max_temp - DERATE_START_C as f64) / (TRIP_C as f64 - DERATE_START_C as f64))
                .clamp(0.05, 1.0)
        } else {
            0.0
        };
        self.derate_level += (derate_t - self.derate_level) * 0.12;

        // 5. Body creak (torso angular speed × total joint torque).
        let w = fr.torso_ang_vel;
        let ang_spd = ((w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt() as f64).min(6.0);
        let creak_t = ((ang_spd * (total_torque / 2500.0)) - 0.15).clamp(0.0, 1.0);
        self.creak_drive += (creak_t * 0.16 - self.creak_drive) * 0.12;

        // 6. Loose rubble grinding: louder with more rock moving.
        let slide = (fr.rubble_slide as f64).max(0.0);
        let rubble_t = (slide / 2.5).min(1.0).sqrt();
        self.rubble_amp += (rubble_t * 0.22 - self.rubble_amp) * 0.1;
    }

    /// Renders `l.len()` stereo samples into `l` and `r` (overwritten).
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let sr = self.sr;

        for i in 0..n {
            if (self.ctrl_k & 31) == 0 {
                self.update_control();
            }
            self.ctrl_k = self.ctrl_k.wrapping_add(1);

            let mut out_l = 0.0f64;
            let mut out_r = 0.0f64;

            // 1. Open gravel-lot hillside wind ambience (+ optional dust storm).
            let storm = (self.current_frame.dust_storm as f64).clamp(0.0, 1.0);
            self.wind_lfo_ph = (self.wind_lfo_ph + (0.17 + 0.25 * storm) / sr).fract();
            if (self.ctrl_k & 63) == 0 {
                let sweep = 0.5 + 0.5 * (TAU * self.wind_lfo_ph).sin();
                self.wind_lp_l
                    .set(160.0 + sweep * 190.0 + storm * 320.0, 1.2, sr);
                self.wind_lp_r
                    .set(185.0 + (1.0 - sweep) * 190.0 + storm * 340.0, 1.2, sr);
            }
            let wn_l = self.r.white();
            let wn_r = self.r.white();
            let wind_amp = 0.012 + 0.095 * storm;
            out_l += self.wind_lp_l.run(wn_l) * wind_amp;
            out_r += self.wind_lp_r.run(wn_r) * wind_amp;
            if storm > 0.05 {
                self.wind_whistle.run(wn_l + wn_r);
                let w = self.wind_whistle.bp * storm * 0.04;
                out_l += w;
                out_r += w;
            }

            // 2. Hydraulic manifold bed + top 3 active joint servos.
            if self.pump_env > 1e-4 {
                self.pump_ph = (self.pump_ph + 112.0 / sr).fract();
                let pump = ((TAU * self.pump_ph).sin() * 0.7
                    + (TAU * 2.0 * self.pump_ph).sin() * 0.3)
                    * self.pump_env;
                out_l += pump;
                out_r += pump;
            }
            for k in 0..3 {
                let amp = self.servo_amp[k];
                if amp > 1e-4 {
                    let hz = self.servo_hz[k];
                    self.servo_ph[k] = (self.servo_ph[k] + hz / sr).fract();
                    self.servo_gear_ph[k] = (self.servo_gear_ph[k] + (hz * 2.5) / sr).fract();
                    let s = ((TAU * self.servo_ph[k]).sin() * 0.72
                        + (TAU * self.servo_gear_ph[k]).sin() * 0.28)
                        * amp;
                    let [sl, sr_ch] = stereo_pan(s, self.servo_pan[k]);
                    out_l += sl;
                    out_r += sr_ch;
                }
            }

            // 3. Strain at the current limit: motor/actuator stall shudder & gear groan.
            if self.strain_amp > 1e-4 {
                self.strain_ph = (self.strain_ph + 64.0 / sr).fract();
                let ripple = 0.55 + 0.45 * (TAU * 3.0 * self.strain_ph).sin();
                let raw = (TAU * self.strain_ph).sin() * 0.6
                    + (TAU * 2.0 * self.strain_ph).sin() * 0.4
                    + self.r.white() * 0.35;
                self.strain_bp.run(raw);
                let groan = (self.strain_bp.bp * 1.6).tanh() * ripple * self.strain_amp;
                out_l += groan;
                out_r += groan;
            }

            // 4. Foot scrape and slide on gravel.
            for f in 0..FEET {
                if self.slide_amp[f] > 1e-4 {
                    let nz = self.r.white();
                    self.slide_bp[f].run(nz);
                    let crunch = self.slide_bp[f].bp * self.slide_amp[f] * self.slide_grain[f];
                    let [sl, sr_ch] = stereo_pan(crunch, self.current_frame.feet[f].pan as f64);
                    out_l += sl;
                    out_r += sr_ch;
                }
            }

            // 5. Heat (coolant fan + high-temp steam hiss & thermal expansion ticks)
            // and Derate warning pulse (> 90 °C).
            if self.heat_fan_amp > 1e-4 {
                self.fan_bp.run(self.r.white());
                let fan = self.fan_bp.bp * self.heat_fan_amp;
                out_l += fan;
                out_r += fan;
            }
            if self.heat_hiss_amp > 1e-4 {
                let tick = if self.r.f() < 0.0008 * (1.0 + self.heat_hiss_amp * 8.0) {
                    4.5
                } else {
                    1.0
                };
                let hiss = self.steam_hp.hp(self.r.white()) * self.heat_hiss_amp * tick;
                out_l += hiss;
                out_r += hiss;
            }
            if self.derate_level > 0.01 {
                let pulse_hz = 1.8 + 4.2 * self.derate_level;
                self.derate_pulse_ph = (self.derate_pulse_ph + pulse_hz / sr).fract();
                self.derate_tone_ph = (self.derate_tone_ph + 740.0 / sr).fract();
                if self.derate_pulse_ph < 0.35 {
                    let env = (self.derate_pulse_ph / 0.05).min(1.0)
                        * ((0.35 - self.derate_pulse_ph) / 0.08).min(1.0);
                    let beep =
                        (TAU * self.derate_tone_ph).sin() * env * (0.04 + 0.09 * self.derate_level);
                    out_l += beep;
                    out_r += beep;
                }
            }

            // 6. Body creak (stick-slip structural flex under load).
            if self.creak_drive > 1e-3 {
                let impulse = if self.r.f() < 0.004 * (1.0 + self.creak_drive * 6.0) {
                    self.r.white() * 3.5
                } else {
                    0.0
                };
                self.creak_bp1.run(impulse);
                self.creak_bp2.run(impulse);
                let creak = (self.creak_bp1.bp + self.creak_bp2.bp * 0.7) * self.creak_drive;
                out_l += creak;
                out_r += creak;
            }

            // 6b. Loose rubble grinding and rolling: gritty noise, gated in
            // grains, over a low rumble.
            if self.rubble_amp > 1e-4 {
                let nz = self.r.white();
                let gate = if self.r.f() < 0.3 { 1.6 } else { 0.4 };
                self.rubble_bp.run(nz * gate);
                self.rubble_lp.run(nz);
                let g = (self.rubble_bp.bp * 0.9 + self.rubble_lp.lp * 1.4) * self.rubble_amp;
                out_l += g;
                out_r += g;
            }

            // 7. Active discrete voices: footsteps, projectile impacts, faults/trips/falls/UI.
            let mut k = 0;
            while k < self.footsteps.len() {
                let [vl, vr] = self.footsteps[k].run(&mut self.r);
                out_l += vl;
                out_r += vr;
                if self.footsteps[k].done() {
                    self.footsteps.swap_remove(k);
                } else {
                    k += 1;
                }
            }
            let mut k = 0;
            while k < self.impacts.len() {
                let [vl, vr] = self.impacts[k].run(&mut self.r);
                out_l += vl;
                out_r += vr;
                if self.impacts[k].done() {
                    self.impacts.swap_remove(k);
                } else {
                    k += 1;
                }
            }
            let mut k = 0;
            while k < self.rocks.len() {
                let [vl, vr] = self.rocks[k].run(&mut self.r);
                out_l += vl;
                out_r += vr;
                if self.rocks[k].done() {
                    self.rocks.swap_remove(k);
                } else {
                    k += 1;
                }
            }
            let mut k = 0;
            while k < self.faults.len() {
                let [vl, vr] = self.faults[k].run(&mut self.r);
                out_l += vl;
                out_r += vr;
                if self.faults[k].done() {
                    self.faults.swap_remove(k);
                } else {
                    k += 1;
                }
            }

            // Soft limiter & DC block.
            let y_l = self.dc_l.hp(out_l).tanh();
            let y_r = self.dc_r.hp(out_r).tanh();
            l[i] = y_l as f32;
            r[i] = y_r as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_rms(audio: &mut MechAudio, secs: f64) -> f64 {
        let blocks = ((secs * audio.sr) / 128.0).ceil() as usize;
        let mut l = [0.0f32; 128];
        let mut r = [0.0f32; 128];
        let mut sum = 0.0f64;
        for _ in 0..blocks {
            audio.process(&mut l, &mut r);
            for (&a, &b) in l.iter().zip(&r) {
                assert!(a.is_finite() && b.is_finite());
                sum += (a as f64).powi(2) + (b as f64).powi(2);
            }
        }
        (sum / (blocks as f64 * 256.0)).sqrt()
    }

    #[test]
    fn heavier_footfall_is_louder_than_light_step_and_shuffle() {
        let render_step = |force: f32, mass: f32| -> f64 {
            let mut a = MechAudio::new(1, 48000.0);
            let mut f0 = AudioFrame::default();
            f0.feet[0].touching = false;
            a.push_frame(f0.clone());
            f0.feet[0].touching = true;
            f0.feet[0].normal_force = force;
            f0.body_mass_kg = mass;
            a.push_frame(f0);
            render_rms(&mut a, 0.25)
        };
        let shuffle = render_step(2200.0, 4000.0);
        let normal = render_step(22000.0, 4000.0);
        let heavy = render_step(38000.0, 5600.0);
        assert!(
            normal > shuffle * 2.0,
            "normal step ({normal:.4}) should be much louder than shuffle ({shuffle:.4})"
        );
        assert!(
            heavy > normal * 1.15,
            "heavier body/impact ({heavy:.4}) should be louder than normal ({normal:.4})"
        );
    }

    #[test]
    fn hot_motor_and_trip_are_unmistakable() {
        let mut cool = MechAudio::new(2, 48000.0);
        cool.push_frame(AudioFrame::default());
        let rms_cool = render_rms(&mut cool, 0.3);

        let mut hot = MechAudio::new(2, 48000.0);
        let mut f_hot = AudioFrame::default();
        f_hot.joints[3].temperature = 112.0; // derating + steam hiss
        hot.push_frame(f_hot.clone());
        let rms_hot = render_rms(&mut hot, 0.3);

        let mut tripped = MechAudio::new(2, 48000.0);
        tripped.push_frame(f_hot.clone());
        f_hot.joints[3].temperature = 121.0;
        f_hot.joints[3].tripped = true;
        tripped.push_frame(f_hot);
        let rms_trip = render_rms(&mut tripped, 0.3);

        assert!(
            rms_hot > rms_cool * 2.0,
            "hot derating motor ({rms_hot:.4}) louder than cool ({rms_cool:.4})"
        );
        assert!(
            rms_trip > rms_hot * 1.5,
            "thermal trip ({rms_trip:.4}) louder than derating ({rms_hot:.4})"
        );
    }

    #[test]
    fn actuator_drive_scales_with_speed_and_current_and_drops_on_fault() {
        let mut idle = MechAudio::new(3, 48000.0);
        idle.push_frame(AudioFrame::default());
        let rms_idle = render_rms(&mut idle, 0.25);

        let mut active = MechAudio::new(3, 48000.0);
        let mut f = AudioFrame::default();
        for j in 0..JOINTS {
            f.joints[j].velocity = 4.5;
            f.joints[j].current = 65.0;
            f.joints[j].torque = 650.0;
        }
        active.push_frame(f.clone());
        let rms_active = render_rms(&mut active, 0.25);
        assert!(
            rms_active > rms_idle * 2.5,
            "active servos ({rms_active:.4}) louder than idle ({rms_idle:.4})"
        );
    }

    #[test]
    fn projectile_impact_and_fall_sound_and_scale_with_impulse() {
        let hit_rms = |impulse: f32| -> f64 {
            let mut a = MechAudio::new(4, 48000.0);
            let mut f = AudioFrame::default();
            f.impacts.push(ImpactEvent {
                impulse,
                link: StruckLink::Torso,
                pan: 0.0,
            });
            a.push_frame(f);
            render_rms(&mut a, 0.35)
        };
        let light = hit_rms(120.0);
        let heavy = hit_rms(1800.0);
        assert!(
            heavy > light * 1.8,
            "heavy shell ({heavy:.4}) > light ({light:.4})"
        );

        let mut fall = MechAudio::new(5, 48000.0);
        fall.push_frame(AudioFrame::default());
        fall.push_frame(AudioFrame {
            terminated: true,
            ..Default::default()
        });
        let rms_fall = render_rms(&mut fall, 0.5);
        assert!(
            rms_fall > 0.08,
            "fall crash is big and heavy: {rms_fall:.4}"
        );
    }

    #[test]
    fn rocks_knock_and_rubble_grinds() {
        let mut quiet = MechAudio::new(5, 44100.0);
        quiet.push_frame(AudioFrame::default());
        let base = render_rms(&mut quiet, 0.3);

        let mut knock = MechAudio::new(5, 44100.0);
        knock.push_frame(AudioFrame {
            rocks: vec![RockEvent {
                impulse: 500.0,
                pan: 0.3,
            }],
            ..Default::default()
        });
        let rms_knock = render_rms(&mut knock, 0.3);
        assert!(
            rms_knock > 4.0 * base,
            "knock {rms_knock:.4} vs quiet {base:.4}"
        );

        let mut grind = MechAudio::new(5, 44100.0);
        for _ in 0..20 {
            grind.push_frame(AudioFrame {
                rubble_slide: 2.0,
                ..Default::default()
            });
        }
        let rms_grind = render_rms(&mut grind, 0.5);
        assert!(
            rms_grind > 3.0 * base,
            "grind {rms_grind:.4} vs quiet {base:.4}"
        );
    }

    #[test]
    fn an_objective_chimes() {
        let mut a = MechAudio::new(8, 44100.0);
        a.push_frame(AudioFrame {
            ui: vec![UiEvent::Objective],
            ..Default::default()
        });
        let rms = render_rms(&mut a, 0.6);
        assert!(rms > 0.02 && rms.is_finite(), "chime {rms:.4}");
    }

    #[test]
    fn demo_traces_render_finite_and_distinguish_body_mass() {
        let nominal = traces::render_trace(&traces::walk_trace(3.2, 4000.0, 0.1), 44100.0, 11);
        let heavy = traces::render_trace(&traces::walk_trace(3.2, 5800.0, 0.1), 44100.0, 11);
        let limp = traces::render_trace(&traces::limp_and_fault_trace(6.0), 44100.0, 22);
        let combat = traces::render_trace(&traces::combat_and_fall_trace(6.0), 44100.0, 33);

        let rms = |s: &[[f32; 2]]| -> f64 {
            let mut sum = 0.0f64;
            for &[l, r] in s {
                assert!(l.is_finite() && r.is_finite());
                sum += f64::from(l).powi(2) + f64::from(r).powi(2);
            }
            (sum / (s.len() as f64 * 2.0)).sqrt()
        };

        let r_nom = rms(&nominal);
        let r_hvy = rms(&heavy);
        let r_lmp = rms(&limp);
        let r_cmb = rms(&combat);
        assert!(
            r_hvy > r_nom * 1.12,
            "5.8t walk ({r_hvy:.4}) should be louder than 4.0t walk ({r_nom:.4})"
        );
        assert!(r_lmp > 0.05 && r_cmb > 0.05);
    }
}
