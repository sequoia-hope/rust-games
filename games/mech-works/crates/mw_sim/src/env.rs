//! The training environment: one mech, its command, observations, actions,
//! rewards and episode ends, and the randomization the curriculum widens.
//! The game runs the same [`Env`], so the policy sees in the browser exactly
//! what it saw in training.

use rapier3d::prelude::*;

use crate::mech::{self, BODY_PARAMS, BodyParams, JOINTS, LEGS, joint_design};
use crate::rng::Rng;
use crate::sim::{DT, PHYSICS_HZ, Sim};
use crate::terrain::Terrain;

/// Policy rate, Hz, and physics steps per policy step.
pub const POLICY_HZ: u32 = 50;
pub const DECIMATION: usize = (PHYSICS_HZ / POLICY_HZ) as usize;
pub const POLICY_DT: f32 = DT * DECIMATION as f32;

/// Joint targets are the standing pose plus this many radians per unit of action.
pub const ACTION_SCALE: f32 = 0.4;

/// The gait clock's period, s (a full cycle: two steps).
pub const GAIT_PERIOD: f32 = 1.3;

/// Episode length, s.
pub const EPISODE_S: f32 = 20.0;
pub const EPISODE_STEPS: u32 = (EPISODE_S * POLICY_HZ as f32) as u32;

/// The commands are resampled this often, s.
pub const COMMAND_S: f32 = 5.0;

/// Torso height the reward asks for, m (the standing pose's).
pub const TARGET_HEIGHT: f32 = 2.0;

/// Observation layout: proprioception, then the body parameters.
pub const PROPRIO: usize = 3 + 3 + 3 + 2 + 2 + JOINTS * 4;
pub const OBS: usize = PROPRIO + BODY_PARAMS;

/// Reward terms, named for the trainer's logs.
pub const REWARD_TERMS: [&str; 16] = [
    "track_lin",
    "track_yaw",
    "lin_z",
    "ang_xy",
    "orientation",
    "height",
    "torque",
    "action_rate",
    "action_jerk",
    "joint_acc",
    "gait",
    "slip",
    "thermal",
    "posture",
    "alive",
    "fall",
];

/// The command the mech follows: forward speed (m/s) and yaw rate (rad/s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Command {
    pub forward: f32,
    pub yaw: f32,
}

/// How wide the randomization is: 0 nominal, 1 the full ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curriculum {
    /// Body parameters (mass, centre of mass, leg lengths, friction, latency).
    pub body: f32,
    /// Actuator damage at reset and mid-episode.
    pub damage: f32,
    /// Pushes: their size and how often.
    pub pushes: f32,
    /// The commanded speed's range.
    pub speed: f32,
}

impl Default for Curriculum {
    fn default() -> Self {
        Self {
            body: 0.0,
            damage: 0.0,
            pushes: 0.0,
            speed: 0.3,
        }
    }
}

/// What a step produced.
#[derive(Clone, Debug)]
pub struct StepResult {
    pub reward: f32,
    /// The episode ended in a fall.
    pub terminated: bool,
    /// The episode ran out of time.
    pub truncated: bool,
    /// Each reward term, as in [`REWARD_TERMS`].
    pub terms: [f32; REWARD_TERMS.len()],
}

/// One mech in training (or in the game).
pub struct Env {
    pub sim: Sim,
    pub terrain: Terrain,
    pub command: Command,
    pub curriculum: Curriculum,
    /// Keep the command as set (the demos steer); training resamples it.
    pub hold_command: bool,
    /// The body the policy is told it has (normally the true one; the
    /// calibration demo feeds an old one).
    pub believed: BodyParams,
    rng: Rng,
    pub steps: u32,
    pub action: [f32; JOINTS],
    pub last_action: [f32; JOINTS],
    /// The action before `last_action`, for the smoothness reward.
    prev_last_action: [f32; JOINTS],
    /// Joint velocities a policy step ago, rad/s.
    prev_qd: [f32; JOINTS],
    /// Physics steps of control latency.
    latency_steps: usize,
    /// Each foot's collider position a policy step ago (Rapier reports a
    /// multibody link's velocity before its last correction).
    prev_feet: [Vector; LEGS],
    /// Seconds until the next command and the next push.
    next_command: f32,
    next_push: f32,
}

impl Env {
    pub fn new(seed: u64) -> Self {
        let terrain = Terrain::flat(400.0);
        let sim = Sim::new(BodyParams::default(), &terrain, 0.0, 0.0);
        let mut env = Self {
            sim,
            terrain,
            command: Command::default(),
            curriculum: Curriculum::default(),
            hold_command: false,
            believed: BodyParams::default(),
            rng: Rng::new(seed),
            steps: 0,
            action: [0.0; JOINTS],
            last_action: [0.0; JOINTS],
            prev_last_action: [0.0; JOINTS],
            prev_qd: [0.0; JOINTS],
            latency_steps: 0,
            prev_feet: [Vector::ZERO; LEGS],
            next_command: COMMAND_S,
            next_push: 0.0,
        };
        env.reset();
        env
    }

    /// A new episode: a random body within the curriculum, standing.
    pub fn reset(&mut self) {
        let params = self.sample_body();
        self.reset_with(params);
    }

    /// A new episode with this body.
    pub fn reset_with(&mut self, params: BodyParams) {
        self.latency_steps = ((params.latency_s() / DT).round() as usize).min(DECIMATION);
        self.believed = params.clone();
        self.sim = Sim::new(params, &self.terrain, 0.0, 0.0);
        self.steps = 0;
        self.action = [0.0; JOINTS];
        self.last_action = [0.0; JOINTS];
        self.prev_last_action = [0.0; JOINTS];
        self.prev_qd = [0.0; JOINTS];
        self.prev_feet = self.feet();
        if !self.hold_command {
            self.command = self.sample_command();
        }
        self.next_command = COMMAND_S;
        self.next_push = self.rng.range(1.0, 4.0);
    }

    fn feet(&self) -> [Vector; LEGS] {
        std::array::from_fn(|l| self.sim.foot_collider_position(l))
    }

    fn sample_body(&mut self) -> BodyParams {
        let c = self.curriculum;
        let r = &mut self.rng;
        // Fields are drawn in the order written.
        let mut p = BodyParams {
            torso_mass: c.body * r.signed(),
            com: [
                c.body * r.signed(),
                c.body * r.signed(),
                c.body * r.signed(),
            ],
            leg_scale: [c.body * r.signed(), c.body * r.signed()],
            friction: c.body * r.signed(),
            latency: -1.0 + 2.0 * c.body * r.f32(),
            ..BodyParams::default()
        };
        // A tool on the hull more often than not, anywhere on it, up to the
        // full mass at the full curriculum.
        if r.f32() < 0.6 * c.body {
            p.tool = [-1.0 + 2.0 * c.body * r.f32(), r.signed(), r.signed()];
        }
        // Damage: each actuator has a chance of being worn, by a random amount.
        for j in 0..JOINTS {
            if r.f32() < 0.25 * c.damage {
                p.health[j] = 1.0 - 2.0 * c.damage * r.f32();
            }
            if r.f32() < 0.15 * c.damage {
                p.driver[j] = 1.0 - 2.0 * c.damage * r.f32();
            }
        }
        p
    }

    fn sample_command(&mut self) -> Command {
        let s = self.curriculum.speed;
        let r = &mut self.rng;
        if r.f32() < 0.1 {
            return Command::default();
        }
        Command {
            forward: r.range(-0.3 * s, 1.5 * s),
            yaw: r.range(-0.4, 0.4) * s.min(1.0),
        }
    }

    /// The gait clock's phase, 0 to 1.
    pub fn phase(&self) -> f32 {
        (self.steps as f32 * POLICY_DT / GAIT_PERIOD).fract()
    }

    /// Whether each foot should be on the ground now.
    pub fn desired_stance(&self) -> [bool; LEGS] {
        if self.standing_command() {
            return [true; LEGS];
        }
        let phase = self.phase();
        std::array::from_fn(|leg| ((phase + 0.5 * leg as f32).fract()) < 0.55)
    }

    fn standing_command(&self) -> bool {
        self.command.forward.abs() < 0.1 && self.command.yaw.abs() < 0.1
    }

    /// The observation, [`OBS`] long.
    pub fn observe(&self) -> [f32; OBS] {
        let mut o = [0.0; OBS];
        let mut i = 0;
        let mut put = |x: f32| {
            o[i] = x;
            i += 1;
        };
        let torso = self.sim.torso();
        let inv = torso.rotation().inverse();
        let g = self.sim.gravity_in_body();
        let w = inv * torso.angvel();
        let v = inv * torso.linvel();
        [g.x, g.y, g.z].iter().for_each(|&x| put(x));
        [w.x, w.y, w.z].iter().for_each(|&x| put(0.25 * x));
        [v.x, v.y, v.z].iter().for_each(|&x| put(0.5 * x));
        put(self.command.forward);
        put(self.command.yaw);
        let phase = std::f32::consts::TAU * self.phase();
        let clock = if self.standing_command() { 0.0 } else { 1.0 };
        put(clock * phase.sin());
        put(clock * phase.cos());
        let q = self.sim.joint_angles();
        let qd = self.sim.joint_velocities();
        for (j, &a) in q.iter().enumerate() {
            put(a - joint_design(j).default);
        }
        qd.iter().for_each(|&x| put(0.1 * x));
        self.action.iter().for_each(|&x| put(x));
        self.sim
            .actuators
            .iter()
            .for_each(|a| put(a.temperature_obs()));
        self.believed.to_vec().iter().for_each(|&x| put(x));
        debug_assert_eq!(i, OBS);
        o
    }

    /// One policy step with `action` (each in [-1, 1]).
    pub fn step(&mut self, action: &[f32; JOINTS]) -> StepResult {
        self.prev_last_action = self.last_action;
        self.last_action = self.action;
        self.action = action.map(|a| a.clamp(-1.0, 1.0));
        let targets: [f32; JOINTS] = std::array::from_fn(|j| {
            let d = joint_design(j);
            (d.default + ACTION_SCALE * self.action[j]).clamp(d.limits[0], d.limits[1])
        });

        let mut torque_cost = 0.0;
        for sub in 0..DECIMATION {
            if sub == self.latency_steps {
                self.sim.targets = targets;
            }
            self.sim.step();
            for (j, a) in self.sim.actuators.iter().enumerate() {
                let t = a.torque.abs().min(self.sim.commanded[j].abs()) / a.spec.peak_torque;
                torque_cost += t * t;
            }
        }
        torque_cost /= DECIMATION as f32;
        self.steps += 1;

        self.events();
        let (terms, terminated) = self.rewards(torque_cost);
        let reward = terms.iter().sum::<f32>();
        let truncated = !terminated && self.steps >= EPISODE_STEPS;
        StepResult {
            reward,
            terminated,
            truncated,
            terms,
        }
    }

    /// Commands, pushes and damage that happen during an episode.
    fn events(&mut self) {
        self.next_command -= POLICY_DT;
        if self.next_command <= 0.0 && !self.hold_command {
            self.command = self.sample_command();
            self.next_command = COMMAND_S;
        }
        let c = self.curriculum;
        if c.pushes > 0.0 {
            self.next_push -= POLICY_DT;
            if self.next_push <= 0.0 {
                // Up to a 1.5 m/s change of the torso's speed, on any link.
                let link = self.rng.below(self.sim.link_count());
                let size = 1.5 * c.pushes * self.rng.f32() * self.sim.total_mass();
                let angle = std::f32::consts::TAU * self.rng.f32();
                let up = 0.2 * self.rng.signed();
                let impulse = Vector::new(angle.cos(), up, angle.sin()) * size;
                self.sim.push(link, impulse);
                self.next_push = self.rng.range(2.0, 6.0) / c.pushes.max(0.2);
            }
        }
        // A part breaks mid-episode now and then.
        if c.damage > 0.0 && self.rng.f32() < 0.002 * c.damage {
            let j = self.rng.below(JOINTS);
            let health = 1.0 - 2.0 * c.damage * self.rng.f32();
            self.damage(j, health);
        }
    }

    /// Damages actuator `joint` to `health` ([-1, 1], as in [`BodyParams`]);
    /// the policy is told, as its body parameters would be.
    pub fn damage(&mut self, joint: usize, health: f32) {
        let h = health.min(self.sim.params.health[joint]);
        self.sim.params.health[joint] = h;
        self.believed.health[joint] = h;
        self.sim.actuators[joint].health = self.sim.params.health_factor(joint);
    }

    fn rewards(&mut self, torque_cost: f32) -> ([f32; REWARD_TERMS.len()], bool) {
        let torso = self.sim.torso();
        let inv = torso.rotation().inverse();
        let v_world = torso.linvel();
        let w = inv * torso.angvel();
        // Speeds in the heading frame: the torso's yaw only.
        let fwd = torso.rotation() * Vector::X;
        let heading = Vector::new(fwd.x, 0.0, fwd.z).normalize_or(Vector::X);
        let side = Vector::new(-heading.z, 0.0, heading.x);
        let vf = v_world.dot(heading);
        let vs = v_world.dot(side);
        let yaw_rate = torso.angvel().y;
        let g = self.sim.gravity_in_body();
        let height = torso.translation().y
            - self
                .terrain
                .height(torso.translation().x, torso.translation().z);

        let cmd = self.command;
        let lin_err = (cmd.forward - vf).powi(2) + vs * vs;
        let feet = self.feet();
        let contacts = self.sim.foot_contacts();
        let desired = self.desired_stance();
        let mut gait = 0.0;
        let mut slip = 0.0;
        for leg in 0..LEGS {
            let on = contacts[leg].0;
            gait += if on == desired[leg] { 1.0 } else { -0.3 };
            if on {
                let d = (feet[leg] - self.prev_feet[leg]) / POLICY_DT;
                slip += d.x * d.x + d.z * d.z;
            }
        }
        self.prev_feet = feet;
        let action_rate: f32 = self
            .action
            .iter()
            .zip(&self.last_action)
            .map(|(a, b)| (a - b) * (a - b))
            .sum();
        // Smooth movement: the action's second difference (a target that
        // flips back and forth each step is the foot wiggle) and the joints'
        // acceleration, rad/s².
        let action_jerk: f32 = (0..JOINTS)
            .map(|j| {
                (self.action[j] - 2.0 * self.last_action[j] + self.prev_last_action[j]).powi(2)
            })
            .sum();
        let qd = self.sim.joint_velocities();
        let joint_acc: f32 = (0..JOINTS)
            .map(|j| ((qd[j] - self.prev_qd[j]) / POLICY_DT).powi(2))
            .sum();
        self.prev_qd = std::array::from_fn(|j| qd[j]);
        let thermal: f32 = self
            .sim
            .actuators
            .iter()
            .map(|a| (a.temperature_obs() - 0.6).max(0.0).powi(2))
            .sum();
        let q = self.sim.joint_angles();
        let mut posture = 0.0;
        for leg in 0..LEGS {
            for j in [0, 1] {
                let i = mech::joint_index(leg, j);
                posture += (q[i] - joint_design(i).default).powi(2);
            }
        }

        let fallen = height < 1.2 || g.y > -0.5 || self.torso_or_legs_touch_ground();

        let dt = POLICY_DT;
        let terms = [
            2.0 * (-lin_err / 0.25).exp(),
            0.6 * (-(cmd.yaw - yaw_rate).powi(2) / 0.25).exp(),
            -(v_world.y * v_world.y),
            -0.05 * (w.x * w.x + w.z * w.z),
            -5.0 * (g.x * g.x + g.z * g.z),
            -10.0 * (height - TARGET_HEIGHT).min(0.0).powi(2),
            -0.5 * torque_cost,
            -0.15 * action_rate,
            -0.05 * action_jerk,
            -3e-5 * joint_acc,
            0.3 * gait,
            -0.2 * slip,
            -2.0 * thermal,
            -0.5 * posture,
            0.5,
            if fallen { -50.0 } else { 0.0 },
        ]
        .map(|x| x * dt);
        (terms, fallen)
    }

    fn torso_or_legs_touch_ground(&self) -> bool {
        // Anything but a foot low enough to touch: the torso's bottom or a knee.
        let poses = self.sim.link_poses();
        let ground = |p: [f32; 3]| self.terrain.height(p[0], p[2]);
        let torso = poses[0].position;
        if torso[1] - ground(torso) < 0.4 {
            return true;
        }
        for leg in 0..LEGS {
            let knee = poses[1 + leg * mech::JOINTS_PER_LEG + 3].position;
            if knee[1] - ground(knee) < 0.2 {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_action_stands_through_an_episode_start() {
        let mut env = Env::new(1);
        env.command = Command::default();
        for _ in 0..100 {
            let r = env.step(&[0.0; JOINTS]);
            assert!(!r.terminated, "fell at step {}", env.steps);
        }
        assert!(env.sim.torso().translation().y > 1.8);
    }

    #[test]
    fn a_side_tool_adds_its_mass_and_pulls_the_centre_over() {
        let bare = Env::new(3);
        let mut env = Env::new(3);
        let p = BodyParams {
            tool: [1.0, 1.0, 0.0],
            ..BodyParams::default()
        };
        env.reset_with(p);
        let added = env.sim.total_mass() - bare.sim.total_mass();
        assert!((added - mech::TOOL_MAX_KG).abs() < 1.0, "added {added} kg");
        // The whole mech's centre of mass moves right (+Z), toward the tool.
        let com = |e: &Env| e.sim.torso().center_of_mass().z;
        assert!(
            com(&env) > com(&bare) + 0.2,
            "{} vs {}",
            com(&env),
            com(&bare)
        );
        assert!(env.sim.tool_shape().is_some() && bare.sim.tool_shape().is_none());
    }

    #[test]
    fn same_seed_same_episode() {
        let run = || {
            let mut env = Env::new(42);
            env.curriculum = Curriculum {
                body: 1.0,
                damage: 1.0,
                pushes: 1.0,
                speed: 1.0,
            };
            env.reset();
            let mut out = Vec::new();
            for i in 0..60 {
                let a: [f32; JOINTS] =
                    std::array::from_fn(|j| ((i * 7 + j) as f32 * 0.37).sin() * 0.3);
                let r = env.step(&a);
                out.push(r.reward);
                out.extend(env.observe());
            }
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn observation_has_the_body_parameters_last() {
        let mut env = Env::new(3);
        let p = BodyParams {
            torso_mass: 0.5,
            ..BodyParams::default()
        };
        env.reset_with(p.clone());
        let o = env.observe();
        assert_eq!(&o[PROPRIO..], &p.to_vec()[..]);
    }

    #[test]
    fn a_hard_push_ends_the_episode_in_a_fall() {
        let mut env = Env::new(5);
        let m = env.sim.total_mass();
        env.sim.push(0, Vector::new(6.0 * m, 0.0, 0.0));
        let mut fell = false;
        for _ in 0..200 {
            if env.step(&[0.0; JOINTS]).terminated {
                fell = true;
                break;
            }
        }
        assert!(fell);
    }
}
