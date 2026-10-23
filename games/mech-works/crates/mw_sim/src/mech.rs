//! The mech's body: its links, joints and actuators, and the body
//! parameters that the workshop and damage change.
//!
//! Frame: X forward, Y up, Z to the mech's right. The torso is the root of
//! one Rapier multibody; each leg is a chain of six revolute joints (hip
//! yaw, roll and pitch, knee, ankle pitch and roll). A positive pitch swings
//! a leg forward; the knee bends with a negative angle.

use crate::actuator::ActuatorSpec;

/// Number of legs and joints per leg.
pub const LEGS: usize = 2;
pub const JOINTS_PER_LEG: usize = 6;
/// Number of actuated joints.
pub const JOINTS: usize = LEGS * JOINTS_PER_LEG;

/// The axis a joint turns about, in its parent's frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn vector(self) -> [f32; 3] {
        match self {
            Axis::X => [1.0, 0.0, 0.0],
            Axis::Y => [0.0, 1.0, 0.0],
            Axis::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// One joint of a leg, as designed (before body parameters).
#[derive(Clone, Copy, Debug)]
pub struct JointDesign {
    pub name: &'static str,
    pub axis: Axis,
    /// Lower and upper limits, rad (for the left leg; the right leg's roll
    /// and yaw limits are mirrored).
    pub limits: [f32; 2],
    /// The standing pose, rad (left leg; mirrored likewise).
    pub default: f32,
    pub peak_torque: f32,
    /// PD gains of the joint's position controller.
    pub kp: f32,
    pub kd: f32,
    /// Rotor inertia reflected through the gearbox, kg·m².
    pub armature: f32,
}

/// The leg's joints from the hip down.
pub const LEG_JOINTS: [JointDesign; JOINTS_PER_LEG] = [
    JointDesign {
        name: "hip_yaw",
        axis: Axis::Y,
        limits: [-0.5, 0.5],
        default: 0.0,
        peak_torque: 12_000.0,
        kp: 30_000.0,
        kd: 1_500.0,
        armature: 30.0,
    },
    JointDesign {
        name: "hip_roll",
        axis: Axis::X,
        limits: [-0.35, 0.45],
        default: 0.0,
        peak_torque: 40_000.0,
        kp: 80_000.0,
        kd: 3_000.0,
        armature: 60.0,
    },
    JointDesign {
        name: "hip_pitch",
        axis: Axis::Z,
        limits: [-0.9, 1.6],
        default: 0.35,
        peak_torque: 45_000.0,
        kp: 80_000.0,
        kd: 3_000.0,
        armature: 60.0,
    },
    JointDesign {
        name: "knee",
        axis: Axis::Z,
        limits: [-2.3, 0.0],
        default: -0.7,
        peak_torque: 55_000.0,
        kp: 100_000.0,
        kd: 3_500.0,
        armature: 60.0,
    },
    JointDesign {
        name: "ankle_pitch",
        axis: Axis::Z,
        limits: [-0.9, 0.8],
        default: 0.35,
        // Stiffer than the gravity stiffness m·g·h of the mech over one
        // foot (about 100 kN·m/rad), so a held pose stands.
        peak_torque: 35_000.0,
        kp: 150_000.0,
        kd: 4_000.0,
        armature: 25.0,
    },
    JointDesign {
        name: "ankle_roll",
        axis: Axis::X,
        limits: [-0.35, 0.35],
        default: 0.0,
        peak_torque: 25_000.0,
        kp: 100_000.0,
        kd: 3_000.0,
        armature: 15.0,
    },
];

/// Index of a joint: `leg * JOINTS_PER_LEG + joint`, leg 0 the left.
pub fn joint_index(leg: usize, joint: usize) -> usize {
    leg * JOINTS_PER_LEG + joint
}

/// Joints whose sign mirrors between the legs (rotations about X and Y).
fn mirrored(axis: Axis) -> bool {
    axis != Axis::Z
}

/// A joint of a particular leg, with its limits and default mirrored.
pub fn joint_design(index: usize) -> JointDesign {
    let leg = index / JOINTS_PER_LEG;
    let mut d = LEG_JOINTS[index % JOINTS_PER_LEG];
    if leg == 1 && mirrored(d.axis) {
        d.limits = [-d.limits[1], -d.limits[0]];
        d.default = -d.default;
    }
    d
}

pub fn actuator_spec(index: usize) -> ActuatorSpec {
    ActuatorSpec::with_peak_torque(joint_design(index).peak_torque)
}

/// The mech's nominal dimensions, m and kg.
pub mod dims {
    /// Torso box half extents, and the height of its centre above the hips.
    pub const TORSO_HALF: [f32; 3] = [0.65, 0.6, 0.75];
    pub const TORSO_CENTRE_Y: f32 = 0.55;
    pub const TORSO_MASS: f32 = 2400.0;
    /// The hips, below the torso origin and to each side.
    pub const HIP_Y: f32 = -0.1;
    pub const HIP_Z: f32 = 0.45;
    /// The small links inside the hip and ankle.
    pub const HIP_LINK_MASS: f32 = 60.0;
    pub const ANKLE_LINK_MASS: f32 = 40.0;
    pub const SMALL_LINK_RADIUS: f32 = 0.14;
    pub const THIGH_LENGTH: f32 = 0.9;
    pub const THIGH_RADIUS: f32 = 0.16;
    pub const THIGH_MASS: f32 = 320.0;
    pub const SHIN_LENGTH: f32 = 0.9;
    pub const SHIN_RADIUS: f32 = 0.13;
    pub const SHIN_MASS: f32 = 200.0;
    /// Foot box half extents and the offset of its centre below the ankle.
    pub const FOOT_HALF: [f32; 3] = [0.38, 0.07, 0.2];
    pub const FOOT_OFFSET: [f32; 3] = [0.1, -0.16, 0.0];
    pub const FOOT_MASS: f32 = 120.0;
}

/// The body parameters, each normalized to [-1, 1] with 0 nominal. The
/// policy sees this vector (or the estimator's guess at it).
#[derive(Clone, Debug, PartialEq)]
pub struct BodyParams {
    /// Torso mass, ±30%.
    pub torso_mass: f32,
    /// The torso's centre of mass offset, ±0.2 m forward, ±0.2 m up, ±0.15 m right.
    pub com: [f32; 3],
    /// Each leg's thigh and shin length, ±10%.
    pub leg_scale: [f32; LEGS],
    /// Each actuator's health: 1 is sound, -1 is 20% of its torque.
    pub health: [f32; JOINTS],
    /// Each driver's strength: 1 never faults, -1 faults at 0.6× its limit.
    pub driver: [f32; JOINTS],
    /// Ground friction, 0.4 to 1.2 (randomized only).
    pub friction: f32,
    /// Control latency, 0 to 20 ms (randomized only).
    pub latency: f32,
}

/// Length of [`BodyParams::to_vec`].
pub const BODY_PARAMS: usize = 1 + 3 + LEGS + JOINTS + JOINTS + 2;

impl Default for BodyParams {
    fn default() -> Self {
        Self {
            torso_mass: 0.0,
            com: [0.0; 3],
            leg_scale: [0.0; LEGS],
            health: [1.0; JOINTS],
            driver: [1.0; JOINTS],
            friction: 0.0,
            latency: -1.0,
        }
    }
}

impl BodyParams {
    pub fn to_vec(&self) -> [f32; BODY_PARAMS] {
        let mut v = [0.0; BODY_PARAMS];
        let mut i = 0;
        let mut put = |x: f32| {
            v[i] = x;
            i += 1;
        };
        put(self.torso_mass);
        self.com.iter().for_each(|&x| put(x));
        self.leg_scale.iter().for_each(|&x| put(x));
        self.health.iter().for_each(|&x| put(x));
        self.driver.iter().for_each(|&x| put(x));
        put(self.friction);
        put(self.latency);
        v
    }

    pub fn from_slice(v: &[f32]) -> Self {
        assert_eq!(v.len(), BODY_PARAMS, "body parameter vector length");
        let mut it = v.iter().map(|x| x.clamp(-1.0, 1.0));
        let mut next = || it.next().unwrap();
        let torso_mass = next();
        let com = [next(), next(), next()];
        let leg_scale = [next(), next()];
        let health = std::array::from_fn(|_| next());
        let driver = std::array::from_fn(|_| next());
        Self {
            torso_mass,
            com,
            leg_scale,
            health,
            driver,
            friction: next(),
            latency: next(),
        }
    }

    /// Linear interpolation between two bodies (the calibration demo).
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let (a, b) = (self.to_vec(), other.to_vec());
        let v: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + (y - x) * t).collect();
        Self::from_slice(&v)
    }

    pub fn torso_mass_kg(&self) -> f32 {
        dims::TORSO_MASS * (1.0 + 0.3 * self.torso_mass)
    }

    pub fn com_offset_m(&self) -> [f32; 3] {
        [0.2 * self.com[0], 0.2 * self.com[1], 0.15 * self.com[2]]
    }

    pub fn leg_scale_factor(&self, leg: usize) -> f32 {
        1.0 + 0.1 * self.leg_scale[leg]
    }

    /// Health as the fraction of torque left, 0.2 to 1.
    pub fn health_factor(&self, joint: usize) -> f32 {
        0.6 + 0.4 * self.health[joint]
    }

    /// Driver strength as a fraction, 0.4 to 1 (1 is a sound driver).
    pub fn driver_factor(&self, joint: usize) -> f32 {
        if self.driver[joint] >= 1.0 {
            1.0
        } else {
            0.7 + 0.3 * self.driver[joint]
        }
    }

    pub fn friction_coefficient(&self) -> f32 {
        0.8 + 0.4 * self.friction
    }

    /// Control latency in seconds.
    pub fn latency_s(&self) -> f32 {
        0.01 * (1.0 + self.latency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_round_trip() {
        let mut p = BodyParams {
            com: [0.5, -0.25, 0.1],
            ..BodyParams::default()
        };
        p.health[3] = -0.5;
        assert_eq!(BodyParams::from_slice(&p.to_vec()), p);
    }

    #[test]
    fn nominal_is_sound() {
        let p = BodyParams::default();
        for j in 0..JOINTS {
            assert_eq!(p.health_factor(j), 1.0);
            assert_eq!(p.driver_factor(j), 1.0);
        }
        assert_eq!(p.torso_mass_kg(), dims::TORSO_MASS);
    }

    #[test]
    fn right_leg_mirrors_roll_and_yaw() {
        let l = joint_design(joint_index(0, 1));
        let r = joint_design(joint_index(1, 1));
        assert_eq!(r.limits, [-l.limits[1], -l.limits[0]]);
        let lp = joint_design(joint_index(0, 2));
        let rp = joint_design(joint_index(1, 2));
        assert_eq!(lp.limits, rp.limits);
        assert_eq!(lp.default, rp.default);
    }

    #[test]
    fn defaults_are_within_limits() {
        for j in 0..JOINTS {
            let d = joint_design(j);
            assert!(
                d.limits[0] <= d.default && d.default <= d.limits[1],
                "{}",
                d.name
            );
        }
    }
}
