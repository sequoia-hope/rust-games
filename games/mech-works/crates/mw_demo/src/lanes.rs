//! The lanes: each a mech on the same ground, a few metres apart, stepped
//! at the policy's rate with its own policy, or standing still without one.
//! Commands from the page set the walk, push, damage, swap parts and reset.

use bevy::prelude::*;
use mw_policy::{Estimator, Policy};
use mw_sim::env::{Command, Env, POLICY_DT};
use mw_sim::mech::{BodyParams, JOINTS, joint_design};
use mw_sim::rapier3d::prelude::Vector;
use mw_sim::sim::BodyPose;
use serde_json::Value;

use crate::{COMMANDS, POLICIES, Status};

/// Lanes are this far apart, m, across the walk.
pub const LANE_GAP: f32 = 7.0;
/// A fallen mech stands up again after this long, s.
pub const RESET_AFTER: f32 = 2.5;
/// A lane walking this far from its start is brought back, m.
pub const WRAP: f32 = 150.0;

pub struct Lane {
    pub env: Env,
    pub policy: Option<Policy>,
    pub label: String,
    /// Link poses before and after the last policy step, for drawing
    /// between them.
    pub prev: Vec<BodyPose>,
    pub cur: Vec<BodyPose>,
    pub fallen_for: Option<f32>,
    pub falls: u32,
    /// The calibration demo: the body the policy is told it has moves from
    /// `from` to the true one over `secs` (explicit calibration).
    pub calibrating: Option<Calibration>,
    /// The estimator paired with the policy: it guesses the body from the
    /// last second of motion, every step.
    pub estimator: Option<Estimator>,
    /// Walk on the estimator's guess rather than on what the policy is told
    /// (autonomous calibration).
    pub adaptive: bool,
    /// The estimator's history was just restarted on the current state.
    fresh: bool,
    obs: Vec<f32>,
    action: [f32; JOINTS],
}

#[derive(Clone, Debug)]
pub struct Calibration {
    pub from: BodyParams,
    pub secs: f32,
    pub elapsed: f32,
}

impl Lane {
    fn new(index: usize) -> Self {
        let mut env = Env::new(1000 + index as u64);
        env.hold_command = true;
        env.command = Command::default();
        let mut lane = Self {
            prev: Vec::new(),
            cur: Vec::new(),
            env,
            policy: None,
            label: String::new(),
            fallen_for: None,
            falls: 0,
            calibrating: None,
            estimator: None,
            adaptive: false,
            fresh: false,
            obs: Vec::new(),
            action: [0.0; JOINTS],
        };
        lane.place(index);
        lane
    }

    /// Restarts this lane's mech standing at its place, with its body.
    fn place(&mut self, index: usize) {
        let params = self.env.sim.params.clone();
        self.reset_body(index, params);
    }

    fn reset_body(&mut self, index: usize, params: BodyParams) {
        let command = self.env.command;
        self.env.reset_with(params);
        self.env.command = command;
        // Each lane on its own line across the walk.
        let z = index as f32 * LANE_GAP;
        let t = self.env.sim.torso_mut();
        let mut pose = *t.position();
        pose.translation.z += z;
        t.set_position(pose, true);
        self.env.sim.refresh_kinematics();
        self.cur = self.env.sim.link_poses();
        self.prev = self.cur.clone();
        self.fallen_for = None;
        self.calibrating = None;
        self.action = [0.0; JOINTS];
        self.restart_estimator();
    }

    /// Starts the estimator's history over from the mech as it stands.
    fn restart_estimator(&mut self) {
        if let Some(e) = &mut self.estimator {
            e.reset(&self.env.observe());
            e.estimate();
            self.fresh = true;
        }
    }

    fn step(&mut self) {
        if let Some(c) = &mut self.calibrating {
            c.elapsed += POLICY_DT;
            let t = (c.elapsed / c.secs.max(1e-3)).min(1.0);
            self.env.believed = c.from.lerp(&self.env.sim.params, t);
            if t >= 1.0 {
                self.calibrating = None;
            }
        }
        self.obs.clear();
        self.obs.extend(self.env.observe());
        if let Some(e) = &mut self.estimator {
            if !std::mem::take(&mut self.fresh) {
                e.push(&self.obs);
            }
            e.estimate();
        }
        if let Some(p) = &mut self.policy {
            if self.obs.len() == p.header.obs_dim {
                match &self.estimator {
                    Some(e) if self.adaptive && e.header.latent == p.header.latent => {
                        p.act_with_latent(&self.obs, e.latent(), &mut self.action)
                    }
                    _ => p.act(&self.obs, &mut self.action),
                }
            }
        } else {
            self.action = [0.0; JOINTS];
        }
        self.prev = std::mem::take(&mut self.cur);
        let r = self.env.step(&self.action);
        self.cur = self.env.sim.link_poses();
        if r.terminated && self.fallen_for.is_none() {
            self.fallen_for = Some(0.0);
            self.falls += 1;
        }
    }
}

/// Every lane, the shared command, and the clock.
#[derive(Resource)]
pub struct Lanes {
    pub lanes: Vec<Lane>,
    pub command: Command,
    pub selected: usize,
    pub paused: bool,
    /// Simulated seconds per real second.
    pub speed: f32,
    acc: f32,
    /// Fraction of the way from the previous policy step to the current one.
    pub blend: f32,
    telemetry_in: f32,
}

impl Default for Lanes {
    fn default() -> Self {
        Self {
            lanes: vec![Lane::new(0)],
            command: Command::default(),
            selected: 0,
            paused: false,
            speed: 1.0,
            acc: 0.0,
            blend: 1.0,
            telemetry_in: 0.0,
        }
    }
}

impl Lanes {
    pub fn set_count(&mut self, n: usize) {
        let n = n.clamp(1, 8);
        while self.lanes.len() < n {
            let mut lane = Lane::new(self.lanes.len());
            lane.env.command = self.command;
            self.lanes.push(lane);
        }
        self.lanes.truncate(n);
        self.selected = self.selected.min(n - 1);
    }

    pub fn reset_all(&mut self) {
        for (i, l) in self.lanes.iter_mut().enumerate() {
            l.place(i);
        }
    }

    /// Where the lanes are, on average (the camera follows it).
    pub fn centre(&self) -> Vec3 {
        let mut c = Vec3::ZERO;
        for l in &self.lanes {
            let p = l.env.sim.torso().translation();
            c += Vec3::new(p.x, p.y, p.z);
        }
        c / self.lanes.len().max(1) as f32
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Lanes>()
        .add_systems(PreUpdate, (take_policies, take_commands))
        .add_systems(Update, (keys, step_lanes).chain());
}

fn take_policies(mut lanes: ResMut<Lanes>) {
    let Some(list) = POLICIES.take() else { return };
    for (lane, label, bytes) in list {
        if lane >= lanes.lanes.len() {
            lanes.set_count(lane + 1);
        }
        if mw_policy::arch(&bytes).is_ok_and(|a| a == "estimator_v1") {
            match Estimator::from_bytes(&bytes) {
                Ok(e) => {
                    info!("lane {lane}: estimator {label}");
                    let l = &mut lanes.lanes[lane];
                    l.estimator = Some(e);
                    l.restart_estimator();
                }
                Err(e) => warn!("lane {lane}: estimator {label}: {e}"),
            }
            continue;
        }
        match Policy::from_bytes(&bytes) {
            Ok(p) => {
                info!(
                    "lane {lane}: policy {label} ({} weights)",
                    p.parameter_count()
                );
                let l = &mut lanes.lanes[lane];
                l.policy = Some(p);
                l.label = label;
                l.place(lane);
            }
            Err(e) => warn!("lane {lane}: policy {label}: {e}"),
        }
    }
}

fn num(v: &Value, k: &str) -> Option<f32> {
    v.get(k).and_then(Value::as_f64).map(|x| x as f32)
}

pub fn take_commands(mut lanes: ResMut<Lanes>) {
    let Some(list) = COMMANDS.take() else { return };
    for line in list {
        match serde_json::from_str::<Value>(&line) {
            Ok(v) => apply(&mut lanes, &v),
            Err(e) => warn!("command {line}: {e}"),
        }
    }
}

/// The commands: `lanes {n}`, `walk {forward, yaw}`, `reset`, `pause
/// {paused}`, `speed {speed}`, `select {lane}`, `push {lane?, link?, x, y,
/// z}` (N·s per kg of mech), `damage {lane?, joint, health}`, `swap {lane?,
/// params: [..]}` (a new body, the policy still told the old one),
/// `calibrate {lane?, secs}` (move what it is told to the truth),
/// `believe {lane?, params}`, `repair {lane?}`, `adapt {lane?, on}` (walk on
/// the estimator's guess: autonomous calibration).
pub fn apply(lanes: &mut Lanes, v: &Value) {
    let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("");
    let which = v
        .get("lane")
        .and_then(Value::as_u64)
        .map_or(lanes.selected, |l| l as usize)
        .min(lanes.lanes.len() - 1);
    match cmd {
        "lanes" => lanes.set_count(v.get("n").and_then(Value::as_u64).unwrap_or(1) as usize),
        "walk" => {
            lanes.command = Command {
                forward: num(v, "forward").unwrap_or(lanes.command.forward),
                yaw: num(v, "yaw").unwrap_or(lanes.command.yaw),
            };
            let c = lanes.command;
            for l in &mut lanes.lanes {
                l.env.command = c;
            }
        }
        "reset" => lanes.reset_all(),
        "pause" => {
            lanes.paused = v
                .get("paused")
                .and_then(Value::as_bool)
                .unwrap_or(!lanes.paused)
        }
        "speed" => lanes.speed = num(v, "speed").unwrap_or(1.0).clamp(0.05, 4.0),
        "select" => lanes.selected = which,
        "push" => {
            let l = &mut lanes.lanes[which];
            let m = l.env.sim.total_mass();
            let link = v.get("link").and_then(Value::as_u64).unwrap_or(0) as usize;
            let dv = Vector::new(
                num(v, "x").unwrap_or(0.0),
                num(v, "y").unwrap_or(0.0),
                num(v, "z").unwrap_or(0.0),
            );
            l.env.sim.push(link, dv * m);
        }
        "damage" => {
            let joint = v.get("joint").and_then(Value::as_u64).unwrap_or(0) as usize % JOINTS;
            let health = num(v, "health").unwrap_or(-1.0);
            let l = &mut lanes.lanes[which];
            l.env.damage(joint, health);
            if let Some(d) = num(v, "driver") {
                l.env.sim.params.driver[joint] = d;
                l.env.believed.driver[joint] = d;
                l.env.sim.actuators[joint].driver_strength = l.env.sim.params.driver_factor(joint);
            }
            if let Some(t) = num(v, "heat") {
                l.env.sim.actuators[joint].temperature = t;
            }
        }
        "repair" => {
            let l = &mut lanes.lanes[which];
            let mut p = l.env.sim.params.clone();
            p.health = [1.0; JOINTS];
            p.driver = [1.0; JOINTS];
            l.reset_body(which, p);
        }
        "swap" | "believe" => {
            let Some(arr) = v.get("params").and_then(Value::as_array) else {
                warn!("{cmd}: no params");
                return;
            };
            let vals: Vec<f32> = arr
                .iter()
                .filter_map(Value::as_f64)
                .map(|x| x as f32)
                .collect();
            if vals.len() != mw_sim::mech::BODY_PARAMS {
                warn!(
                    "{cmd}: {} params, want {}",
                    vals.len(),
                    mw_sim::mech::BODY_PARAMS
                );
                return;
            }
            let p = BodyParams::from_slice(&vals);
            let l = &mut lanes.lanes[which];
            if cmd == "swap" {
                // The new body, but the policy is still told the old one.
                let old = l.env.believed.clone();
                l.reset_body(which, p);
                l.env.believed = old;
            } else {
                l.env.believed = p;
            }
        }
        "adapt" => {
            let l = &mut lanes.lanes[which];
            l.adaptive = v.get("on").and_then(Value::as_bool).unwrap_or(!l.adaptive);
        }
        "calibrate" => {
            let l = &mut lanes.lanes[which];
            l.calibrating = Some(Calibration {
                from: l.env.believed.clone(),
                secs: num(v, "secs").unwrap_or(3.0),
                elapsed: 0.0,
            });
        }
        other => warn!("unknown command {other:?}"),
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut lanes: ResMut<Lanes>) {
    let mut c = lanes.command;
    let mut changed = false;
    if keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::ArrowUp) {
        c.forward = (c.forward + 0.25).min(1.5);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
        c.forward = (c.forward - 0.25).max(-0.3);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::ArrowLeft) {
        c.yaw = (c.yaw + 0.2).min(0.4);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyD) || keys.just_pressed(KeyCode::ArrowRight) {
        c.yaw = (c.yaw - 0.2).max(-0.4);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyX) {
        c = Command::default();
        changed = true;
    }
    if changed {
        let v = serde_json::json!({"cmd": "walk", "forward": c.forward, "yaw": c.yaw});
        apply(&mut lanes, &v);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        lanes.reset_all();
    }
    if keys.just_pressed(KeyCode::KeyP) {
        lanes.paused = !lanes.paused;
    }
    if keys.just_pressed(KeyCode::Space) {
        // A shove from the side, 0.6 m/s worth.
        let sel = lanes.selected;
        apply(
            &mut lanes,
            &serde_json::json!({"cmd": "push", "lane": sel, "z": 0.6}),
        );
    }
}

pub fn step_lanes(time: Res<Time>, mut lanes: ResMut<Lanes>, mut status: ResMut<Status>) {
    let dt = time.delta_secs().min(0.1);
    if !lanes.paused {
        lanes.acc += dt * lanes.speed;
        // At most a few steps a frame: a slow frame slows the mech down
        // rather than piling up work.
        let mut n = 0;
        while lanes.acc >= POLICY_DT && n < 6 {
            lanes.acc -= POLICY_DT;
            n += 1;
            for (i, l) in lanes.lanes.iter_mut().enumerate() {
                l.step();
                let fallen = l.fallen_for.map(|t| t + POLICY_DT);
                l.fallen_for = fallen;
                let far = l.env.sim.torso().translation().x.abs() > WRAP;
                if fallen.is_some_and(|t| t > RESET_AFTER) || far {
                    l.place(i);
                }
            }
        }
        if n == 6 {
            lanes.acc = 0.0;
        }
    }
    lanes.blend = (lanes.acc / POLICY_DT).clamp(0.0, 1.0);

    lanes.telemetry_in -= dt;
    if lanes.telemetry_in <= 0.0 {
        lanes.telemetry_in = 0.1;
        status.telemetry = telemetry(&lanes).to_string();
    }
}

/// The selected lane, for the inspector and the page's readouts.
pub fn telemetry(lanes: &Lanes) -> Value {
    let l = &lanes.lanes[lanes.selected];
    let sim = &l.env.sim;
    let t = sim.torso();
    let v = t.linvel();
    let q = sim.joint_angles();
    let joints: Vec<Value> = (0..JOINTS)
        .map(|j| {
            let a = &sim.actuators[j];
            serde_json::json!({
                "name": joint_design(j).name,
                "q": q[j],
                "target": sim.targets[j],
                "torque": a.torque,
                "peak": a.spec.peak_torque,
                "limit": a.torque_limit(),
                "current": a.current,
                "maxCurrent": a.spec.max_current,
                "temp": a.temperature,
                "tripped": a.tripped,
                "fault": a.fault_left > 0.0,
                "health": a.health,
            })
        })
        .collect();
    let contacts = sim.foot_contacts();
    serde_json::json!({
        "lane": lanes.selected,
        "lanes": lanes.lanes.iter().map(|l| serde_json::json!({
            "label": l.label, "policy": l.policy.is_some(), "falls": l.falls,
            "fallen": l.fallen_for.is_some(),
            "x": l.env.sim.torso().translation().x,
        })).collect::<Vec<_>>(),
        "time": sim.time,
        "command": {"forward": l.env.command.forward, "yaw": l.env.command.yaw},
        "speed": (v.x * v.x + v.z * v.z).sqrt(),
        "height": t.translation().y,
        "joints": joints,
        "feet": contacts.iter().map(|(on, f)| serde_json::json!({"on": on, "force": f})).collect::<Vec<_>>(),
        "params": sim.params.to_vec().to_vec(),
        "believed": l.env.believed.to_vec().to_vec(),
        "calibrating": l.calibrating.as_ref().map(|c| c.elapsed / c.secs),
        "latent": l.policy.as_ref().map(|p| p.latent().to_vec()),
        "estimated": l.estimator.as_ref().map(|e| e.params().to_vec()),
        "adaptive": l.adaptive && l.estimator.is_some(),
    })
}
