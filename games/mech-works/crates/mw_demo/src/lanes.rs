//! The lanes: each a mech on the same ground, a few metres apart, stepped
//! at the policy's rate with its own policy, or standing still without one.
//! Commands from the page set the walk, push, damage, swap parts and reset.

use bevy::prelude::*;
use mw_policy::{Estimator, Policy};
use mw_sim::env::{Command, Env, POLICY_DT};
use mw_sim::mech::{BodyParams, JOINTS, LEGS, joint_design};
use mw_sim::rapier3d::prelude::Vector;
use mw_sim::sim::BodyPose;
use mw_sim::terrain::Terrain;
use serde_json::Value;

use crate::level::Level;
use crate::{COMMANDS, Mode, POLICIES, Status};

/// Lanes are this far apart, m, across the walk.
pub const LANE_GAP: f32 = 7.0;
/// A fallen mech stands up again after this long, s.
pub const RESET_AFTER: f32 = 2.5;
/// A lane walking this far from its start is brought back, m (unless on the endless path).
pub const WRAP: f32 = 150.0;

/// Settings for Demo 2: the endless dirt path with obstacles (M5).
#[derive(Clone, Debug)]
pub struct PathConfig {
    pub enabled: bool,
    pub roughness: f32,
    pub density: f32,
    pub obs_scale: f32,
    pub mid_walk_damage: bool,
    pub seed: u64,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            roughness: 0.45,
            density: 0.65,
            obs_scale: 0.95,
            mid_walk_damage: false,
            seed: 23,
        }
    }
}

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
    /// Previous foot world positions `[x, y, z]` for computing tangential slip speed.
    prev_foot_pos: [[f32; 3]; LEGS],
    /// Foot tangential slip speed `m/s` on the last policy step.
    pub foot_slip: [f32; LEGS],
    /// Counts the mech's worlds: each reset builds a new one, which a
    /// level then fills with its walls and rubble again.
    pub generation: u32,
}

#[derive(Clone, Debug)]
pub struct Calibration {
    pub from: BodyParams,
    pub secs: f32,
    pub elapsed: f32,
}

impl Lane {
    fn new(index: usize, path_cfg: &PathConfig) -> Self {
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
            prev_foot_pos: [[0.0; 3]; LEGS],
            foot_slip: [0.0; LEGS],
            generation: 0,
        };
        lane.place(index, path_cfg);
        lane
    }

    /// Restarts this lane's mech standing at its place, with its body.
    fn place(&mut self, index: usize, path_cfg: &PathConfig) {
        let params = self.env.sim.params.clone();
        self.reset_body(index, params, path_cfg);
    }

    fn reset_body(&mut self, index: usize, params: BodyParams, path_cfg: &PathConfig) {
        // Each lane on its own line across the walk.
        self.reset_at(index, params, 0.0, index as f32 * LANE_GAP, 0.0, path_cfg);
    }

    /// Restarts lane `index`'s mech standing at (x, z), facing `heading`
    /// (rad about the vertical, 0 along +X), with this body.
    pub fn reset_at(
        &mut self,
        index: usize,
        params: BodyParams,
        x: f32,
        z: f32,
        heading: f32,
        path_cfg: &PathConfig,
    ) {
        let command = self.env.command;
        self.env.terrain = if path_cfg.enabled {
            Terrain::path(
                400.0,
                path_cfg.roughness,
                path_cfg.density,
                path_cfg.obs_scale,
                path_cfg.seed.wrapping_add(index as u64),
            )
        } else {
            Terrain::flat(400.0)
        };
        self.env.reset_with(params);
        self.env.command = command;
        self.generation = self.generation.wrapping_add(1);
        let t = self.env.sim.torso_mut();
        let mut pose = *t.position();
        pose.translation.x += x;
        pose.translation.z += z;
        if heading != 0.0 {
            pose.rotation =
                mw_sim::rapier3d::prelude::Rotation::from_rotation_y(heading) * pose.rotation;
        }
        t.set_position(pose, true);
        self.env.sim.refresh_kinematics();
        self.cur = self.env.sim.link_poses();
        self.prev = self.cur.clone();
        self.prev_foot_pos = std::array::from_fn(|leg| {
            let p = self.env.sim.foot_collider_position(leg);
            [p.x, p.y, p.z]
        });
        self.foot_slip = [0.0; LEGS];
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

    /// One policy step; true if the mech just fell.
    pub fn step(&mut self, mid_walk_damage: bool) -> bool {
        if let Some(c) = &mut self.calibrating {
            c.elapsed += POLICY_DT;
            let t = (c.elapsed / c.secs.max(1e-3)).min(1.0);
            self.env.believed = c.from.lerp(&self.env.sim.params, t);
            if t >= 1.0 {
                self.calibrating = None;
            }
        }
        if mid_walk_damage && self.env.steps > 50 && self.env.steps.is_multiple_of(350) {
            let j = ((self.env.steps / 350) as usize * 5 + 3) % JOINTS;
            let cur = self.env.sim.params.health[j];
            self.env.damage(j, (cur - 0.55).max(-0.65));
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
        let contacts = self.env.sim.foot_contacts();
        for (leg, &(touching, _)) in contacts.iter().enumerate() {
            let p = self.env.sim.foot_collider_position(leg);
            let prev = self.prev_foot_pos[leg];
            let dx = p.x - prev[0];
            let dz = p.z - prev[2];
            self.foot_slip[leg] = if touching {
                (dx * dx + dz * dz).sqrt() / POLICY_DT
            } else {
                0.0
            };
            self.prev_foot_pos[leg] = [p.x, p.y, p.z];
        }
        let just_fell = r.terminated && self.fallen_for.is_none();
        if just_fell {
            self.fallen_for = Some(0.0);
            self.falls += 1;
        }
        just_fell
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
    pub path_cfg: PathConfig,
    /// Environmental wind / dust storm intensity (`0.0..=1.0`).
    pub dust_storm: f32,
    /// Projectile impacts `[impulse_ns, link_kind, pan]` since the last audio step.
    pub pending_impacts: Vec<[f32; 3]>,
    /// The level being played (`Mode::Level`): its walls, rubble and objective.
    pub level: Option<Level>,
    acc: f32,
    /// Fraction of the way from the previous policy step to the current one.
    pub blend: f32,
    telemetry_in: f32,
    audio_seq: u64,
    audio_steps: Vec<Value>,
}

impl Default for Lanes {
    fn default() -> Self {
        let path_cfg = PathConfig::default();
        Self {
            lanes: vec![Lane::new(0, &path_cfg)],
            command: Command::default(),
            selected: 0,
            paused: false,
            speed: 1.0,
            path_cfg,
            dust_storm: 0.0,
            pending_impacts: Vec::new(),
            level: None,
            acc: 0.0,
            blend: 1.0,
            telemetry_in: 0.0,
            audio_seq: 0,
            audio_steps: Vec::new(),
        }
    }
}

impl Lanes {
    pub fn set_count(&mut self, n: usize) {
        let n = n.clamp(1, 8);
        while self.lanes.len() < n {
            let mut lane = Lane::new(self.lanes.len(), &self.path_cfg);
            lane.env.command = self.command;
            self.lanes.push(lane);
        }
        self.lanes.truncate(n);
        self.selected = self.selected.min(n - 1);
    }

    pub fn reset_all(&mut self) {
        if self.level.is_some() {
            self.start_level();
            return;
        }
        let cfg = self.path_cfg.clone();
        for (i, l) in self.lanes.iter_mut().enumerate() {
            l.place(i, &cfg);
        }
    }

    /// Starts the level over: one mech at the base, fresh rubble.
    pub fn start_level(&mut self) {
        self.set_count(1);
        self.selected = 0;
        self.path_cfg.enabled = false;
        // A light desert wind.
        self.dust_storm = crate::level::WIND;
        let serial = self.level.as_ref().map_or(1, |l| l.serial + 1);
        self.level = Some(Level::new(serial));
        let cfg = self.path_cfg.clone();
        let (x, z, heading) = crate::level::START;
        let lane = &mut self.lanes[0];
        let params = lane.env.sim.params.clone();
        lane.reset_at(0, params, x, z, heading, &cfg);
        lane.env.command = Command::default();
        self.command = Command::default();
        lane.falls = 0;
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
        .add_systems(Startup, init_mode)
        .add_systems(PreUpdate, (take_policies, take_commands))
        .add_systems(Update, (keys, step_lanes).chain());
}

fn init_mode(mode: Res<Mode>, mut lanes: ResMut<Lanes>) {
    if *mode == Mode::Level {
        lanes.start_level();
    }
    if *mode == Mode::Path {
        lanes.path_cfg.enabled = true;
        lanes.command = Command {
            forward: 0.75,
            yaw: 0.0,
        };
        lanes.reset_all();
    }
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
                let cfg = lanes.path_cfg.clone();
                let level = lanes.level.is_some();
                let l = &mut lanes.lanes[lane];
                l.policy = Some(p);
                l.label = label;
                if !level {
                    l.place(lane, &cfg);
                }
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
/// the estimator's guess: autonomous calibration), `path {roughness?,
/// density?, scale?, damage?, rebuild?}`, `weather {dust}`.
pub fn apply(lanes: &mut Lanes, v: &Value) {
    let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("");
    let which = v
        .get("lane")
        .and_then(Value::as_u64)
        .map_or(lanes.selected, |l| l as usize)
        .min(lanes.lanes.len() - 1);
    match cmd {
        "mode" => {
            let m = Mode::parse(v.get("mode").and_then(Value::as_str).unwrap_or(""));
            if m == Mode::Level {
                if lanes.level.is_none() {
                    lanes.start_level();
                }
                return;
            }
            if lanes.level.take().is_some() {
                lanes.dust_storm = 0.0;
                lanes.path_cfg.enabled = m == Mode::Path;
                lanes.reset_all();
            }
            let want_path = m == Mode::Path;
            if lanes.path_cfg.enabled != want_path {
                lanes.path_cfg.enabled = want_path;
                if want_path && lanes.command.forward.abs() < 0.05 {
                    lanes.command.forward = 0.75;
                    let c = lanes.command;
                    for l in &mut lanes.lanes {
                        l.env.command = c;
                    }
                }
                lanes.reset_all();
            }
        }
        "path" => {
            if let Some(r) = num(v, "roughness") {
                lanes.path_cfg.roughness = r.clamp(0.0, 1.0);
            }
            if let Some(d) = num(v, "density") {
                lanes.path_cfg.density = d.clamp(0.0, 1.0);
            }
            if let Some(s) = num(v, "scale") {
                lanes.path_cfg.obs_scale = s.clamp(0.4, 2.0);
            }
            if let Some(dmg) = v.get("damage").and_then(Value::as_bool) {
                lanes.path_cfg.mid_walk_damage = dmg;
            }
            if v.get("rebuild").and_then(Value::as_bool).unwrap_or(false) {
                lanes.path_cfg.seed = lanes.path_cfg.seed.wrapping_add(7);
                lanes.reset_all();
            }
        }
        "weather" => {
            if let Some(d) = num(v, "dust") {
                lanes.dust_storm = d.clamp(0.0, 1.0);
            }
        }
        "lanes" if lanes.level.is_some() => {}
        "lanes" => lanes.set_count(v.get("n").and_then(Value::as_u64).unwrap_or(1) as usize),
        "shove" => {
            if let Some(level) = &mut lanes.level {
                level.want_shove = true;
            }
        }
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
            let cfg = lanes.path_cfg.clone();
            let l = &mut lanes.lanes[which];
            let mut p = l.env.sim.params.clone();
            p.health = [1.0; JOINTS];
            p.driver = [1.0; JOINTS];
            l.reset_body(which, p, &cfg);
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
            let cfg = lanes.path_cfg.clone();
            let l = &mut lanes.lanes[which];
            if cmd == "swap" {
                // The new body, but the policy is still told the old one.
                let old = l.env.believed.clone();
                l.reset_body(which, p, &cfg);
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

/// The demos' keys (the level has its own, `player::keys`).
fn keys(keys: Res<ButtonInput<KeyCode>>, mode: Res<Mode>, mut lanes: ResMut<Lanes>) {
    if *mode == Mode::Level {
        return;
    }
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
    let sel = lanes.selected;
    let dust = lanes.dust_storm;
    let cfg = lanes.path_cfg.clone();
    let wrap_dist = if cfg.enabled { 185.0 } else { WRAP };
    // A level's mech walks where it likes: the level's walls keep it in.
    let wrap = lanes.level.is_none();
    if !lanes.paused {
        lanes.acc += dt * lanes.speed;
        // At most a few steps a frame: a slow frame slows the mech down
        // rather than piling up work.
        let mut n = 0;
        while lanes.acc >= POLICY_DT && n < 6 {
            lanes.acc -= POLICY_DT;
            n += 1;
            let mut sel_step = None;
            let Lanes {
                lanes: list, level, ..
            } = &mut *lanes;
            for (i, l) in list.iter_mut().enumerate() {
                if i == 0
                    && let Some(level) = level.as_mut()
                {
                    level.attach(l);
                    level.before_step(l);
                }
                let just_fell = l.step(cfg.enabled && cfg.mid_walk_damage);
                if i == sel {
                    sel_step = Some(just_fell);
                }
                let fallen = l.fallen_for.map(|t| t + POLICY_DT);
                l.fallen_for = fallen;
                if i == 0
                    && let Some(level) = level.as_mut()
                {
                    level.after_step(l);
                    if fallen.is_some_and(|t| t > RESET_AFTER) {
                        level.stand_up(l, &cfg);
                    }
                    continue;
                }
                let far = wrap && l.env.sim.torso().translation().x.abs() > wrap_dist;
                if fallen.is_some_and(|t| t > RESET_AFTER) || far {
                    l.place(i, &cfg);
                }
            }
            if let Some(just_fell) = sel_step {
                let impacts = std::mem::take(&mut lanes.pending_impacts);
                let mut step_val = audio_step(&lanes.lanes[sel], dust, just_fell, &impacts);
                if let Some(level) = &mut lanes.level {
                    level.audio_into(&mut step_val);
                }
                lanes.audio_steps.push(step_val);
            }
        }
        if n == 6 {
            lanes.acc = 0.0;
        }
    } else if !lanes.pending_impacts.is_empty() {
        let impacts = std::mem::take(&mut lanes.pending_impacts);
        let step_val = audio_step(&lanes.lanes[sel], dust, false, &impacts);
        lanes.audio_steps.push(step_val);
    }
    lanes.blend = (lanes.acc / POLICY_DT).clamp(0.0, 1.0);

    if !lanes.audio_steps.is_empty() {
        lanes.audio_seq += 1;
        let steps = std::mem::take(&mut lanes.audio_steps);
        status.audio = serde_json::json!({
            "seq": lanes.audio_seq,
            "steps": steps,
        })
        .to_string();
    }

    lanes.telemetry_in -= dt;
    if lanes.telemetry_in <= 0.0 {
        lanes.telemetry_in = 0.1;
        status.telemetry = telemetry(&lanes).to_string();
        status.level = lanes
            .level
            .as_ref()
            .map(|l| l.status(&lanes.lanes[0]).to_string())
            .unwrap_or_default();
    }
}

/// Compact per-step audio telemetry for `mw_audio` (`mw-audio-worklet.js`).
fn audio_step(lane: &Lane, dust: f32, just_fell: bool, impacts: &[[f32; 3]]) -> Value {
    let sim = &lane.env.sim;
    let t = sim.torso();
    let v = t.linvel();
    let w = t.angvel();
    let q = sim.joint_angles();
    let qd = sim.joint_velocities();
    let contacts = sim.foot_contacts();
    let joints: Vec<[f32; 10]> = (0..JOINTS)
        .map(|j| {
            let a = &sim.actuators[j];
            [
                a.torque,
                a.current,
                a.spec.max_current,
                a.torque_limit(),
                a.temperature,
                a.health,
                q[j],
                qd[j],
                if a.tripped { 1.0 } else { 0.0 },
                a.fault_left,
            ]
        })
        .collect();
    let feet = [
        [
            if contacts[0].0 { 1.0 } else { 0.0 },
            contacts[0].1,
            lane.foot_slip[0],
            -0.32,
        ],
        [
            if contacts[1].0 { 1.0 } else { 0.0 },
            contacts[1].1,
            lane.foot_slip[1],
            0.32,
        ],
    ];
    serde_json::json!({
        "mass": sim.total_mass(),
        "dust": dust,
        "term": just_fell,
        "vx": v.x, "vy": v.y, "vz": v.z,
        "wx": w.x, "wy": w.y, "wz": w.z,
        "feet": feet,
        "joints": joints,
        "impacts": impacts,
    })
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
    let scan = l.env.height_scan();
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
        "speedScale": lanes.speed,
        "paused": lanes.paused,
        "dust": lanes.dust_storm,
        "height": t.translation().y - l.env.terrain.base_height(t.translation().x, t.translation().z),
        "joints": joints,
        "feet": contacts.iter().map(|(on, f)| serde_json::json!({"on": on, "force": f})).collect::<Vec<_>>(),
        "params": sim.params.to_vec().to_vec(),
        "believed": l.env.believed.to_vec().to_vec(),
        "calibrating": l.calibrating.as_ref().map(|c| c.elapsed / c.secs),
        "latent": l.policy.as_ref().map(|p| p.latent().to_vec()),
        "estimated": l.estimator.as_ref().map(|e| e.params().to_vec()),
        "adaptive": l.adaptive && l.estimator.is_some(),
        "scan": scan.to_vec(),
        "obstacles": l.env.terrain.obstacles.len(),
    })
}
