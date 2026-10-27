//! The first playable level (`?mode=level`): the mech starts at the home
//! base on the gravel lot, walks out along the marker posts, and finds the
//! route blocked where a landslide off the hill has filled a narrow pass
//! with rubble. Clearing the pass opens the route.
//!
//! The rubble is loose rock in the mech's own Rapier world
//! (`mw_sim::rubble`): the mech's feet and hull push it for real, and its
//! blade (the shove) throws whatever is in front of it. The pass counts as
//! clear once no rock's centre is left in the blocking volume: the route's
//! lane through the pass (rock shoved against the pass's sides is out of
//! the way). All of the
//! clearing lives behind [`Level::before_step`] (the shove) and
//! [`Level::blocked`] (the test), so another mechanic can replace it.
//!
//! The layout, along the walk (+X), the hill to the north (+Z):
//!
//! ```text
//!   z  21 ════════════ north edge (the hill's foot) ════════════
//!         base         posts …         ▓▓ north spur ▓▓
//!   z   0  ◉ start ─────────────────── ░ rubble ░ ───── beyond
//!                                      ▓▓ south spur ▓▓
//!   z -44 ════════════ south edge (the far ridge) ══════════════
//!        x 0                          x 84 … 98
//! ```

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use mw_sim::env::POLICY_DT;
use mw_sim::rapier3d::prelude::{
    Array2, ColliderBuilder, Group, InteractionGroups, InteractionTestMode, PhysicsWorld, Pose,
    RigidBodyBuilder, Rotation, Vector,
};
use mw_sim::rng::Rng;
use mw_sim::rubble::{Rubble, Volume, Wall};
use mw_sim::sim::{self, BodyPose};
use mw_sim::terrain::Terrain;
use serde_json::{Value, json};

use crate::Mode;
use crate::lanes::{Lane, Lanes, PathConfig};

/// Where the mech starts: (x, z, heading).
pub const START: (f32, f32, f32) = (0.0, 0.0, 0.0);
/// The level's light desert wind (the scene's dust and wind level).
pub const WIND: f32 = 0.12;
/// The pass the slide filled: its middle along the walk, and its sides.
pub const SITE_X: f32 = 91.0;
pub const PASS_Z: [f32; 2] = [-5.0, 2.0];
/// The pass's spurs run this far either side of `SITE_X`.
const SPUR_HALF_X: f32 = 7.0;
/// Walking this far from the start leaves the base.
const LEAVE_RADIUS: f32 = 10.0;
/// Coming this close to the pass reaches the site.
const SITE_RADIUS: f32 = 16.0;

/// The rubble: how many rocks, how big (m across, a range), where they
/// fall from.
const ROCKS: usize = 48;
const ROCK_RADIUS: [f32; 2] = [0.35, 0.6];
const PILE: Volume = Volume {
    min: [SITE_X - 3.0, 0.0, PASS_Z[0] + 0.4],
    max: [SITE_X + 3.0, 2.0, PASS_Z[1] - 0.4],
};
/// The route's line through the pass (the marker posts' side of it), and
/// the lane either side of it the mech needs.
pub const ROUTE_Z: f32 = -1.5;
const LANE_HALF: f32 = 2.0;
/// No rock's centre may be left in here for the route to be open: the
/// route's lane through the pass. Rock pushed against the pass's sides is
/// out of the way, as a road crew would leave it.
pub const BLOCKING: Volume = Volume {
    min: [SITE_X - 5.5, -1.0, ROUTE_Z - LANE_HALF],
    max: [SITE_X + 5.5, 6.0, ROUTE_Z + LANE_HALF],
};

/// The blade: how far ahead of the torso and to each side it reaches, how
/// high, how hard it throws (the speed a 300 kg rock gets), and how often.
const SHOVE_REACH: f32 = 4.2;
const SHOVE_HALF_WIDTH: f32 = 1.9;
const SHOVE_HEIGHT: f32 = 2.6;
const SHOVE_SPEED: f32 = 3.8;
const SHOVE_REF_MASS: f32 = 300.0;
pub const SHOVE_EVERY: f32 = 0.6;

/// A place counts as safe to stand the mech up again when no rock's centre
/// is nearer than this, m.
const SAFE_CLEARANCE: f32 = 3.0;

/// The level's walls: the pass's two spurs and the play area's edges.
pub fn walls() -> Vec<Wall> {
    let (x0, x1) = (-62.0, 172.0);
    let (z0, z1) = (-46.0, 23.0);
    let wall = |xa: f32, xb: f32, za: f32, zb: f32| Wall {
        centre: [0.5 * (xa + xb), 3.0, 0.5 * (za + zb)],
        half: [0.5 * (xb - xa), 3.0, 0.5 * (zb - za)],
        yaw: 0.0,
    };
    vec![
        // The spurs either side of the pass, from the edges in.
        wall(
            SITE_X - SPUR_HALF_X,
            SITE_X + SPUR_HALF_X,
            PASS_Z[1],
            z1 - 2.0,
        ),
        wall(
            SITE_X - SPUR_HALF_X,
            SITE_X + SPUR_HALF_X,
            z0 + 2.0,
            PASS_Z[0],
        ),
        // The edges: the hill's foot, the far ridge, either end.
        wall(x0, x1, z1 - 2.0, z1),
        wall(x0, x1, z0, z0 + 2.0),
        wall(x0, x0 + 2.0, z0, z1),
        wall(x1 - 2.0, x1, z0, z1),
    ]
}

/// What the player is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// At the base.
    Leave,
    /// On the way to the site.
    Travel,
    /// At the site, the pass blocked.
    Clear,
    /// The pass clear: the route is open.
    Done,
}

impl Stage {
    fn key(self) -> &'static str {
        match self {
            Stage::Leave => "leave",
            Stage::Travel => "travel",
            Stage::Clear => "clear",
            Stage::Done => "done",
        }
    }
}

/// The level's state, beside the mech's lane.
pub struct Level {
    /// Counts levels started, so the drawing knows a new one.
    pub serial: u64,
    pub rubble: Rubble,
    /// Each rock's pose at the policy step before the last, to draw between.
    pub prev: Vec<BodyPose>,
    pub stage: Stage,
    /// Rocks in the blocking volume at the start, and now.
    pub total: usize,
    pub left: usize,
    /// The player asked for a shove (the key, the page's button).
    pub want_shove: bool,
    pub shoves: u32,
    cooldown: f32,
    /// The lane world (its `generation`) the walls and rubble are in.
    attached: Option<u32>,
    /// The last place the mech stood upright clear of the rubble, and its
    /// heading: where it stands up again after a fall.
    safe: (f32, f32, f32),
    /// Each rock's velocity a step ago, for its knocks.
    vel: Vec<[f32; 3]>,
    /// Sound for the next audio step.
    rocks_audio: Vec<[f32; 2]>,
    slide_audio: f32,
    ui_audio: Vec<u32>,
    impacts_audio: Vec<[f32; 3]>,
}

impl Level {
    pub fn new(serial: u64) -> Self {
        let mut rubble = Rubble::pile(serial.wrapping_mul(7919), PILE, ROCKS, ROCK_RADIUS);
        settle(&mut rubble, 3.0);
        let total = rubble.inside(&BLOCKING).0;
        Self {
            serial,
            prev: rubble.rocks.iter().map(|r| r.pose).collect(),
            vel: vec![[0.0; 3]; rubble.rocks.len()],
            rubble,
            stage: Stage::Leave,
            total,
            left: total,
            want_shove: false,
            shoves: 0,
            cooldown: 0.0,
            attached: None,
            safe: START,
            rocks_audio: Vec::new(),
            slide_audio: 0.0,
            ui_audio: Vec::new(),
            impacts_audio: Vec::new(),
        }
    }

    /// Whether the route is still blocked: any rock's centre in the
    /// blocking volume.
    pub fn blocked(&self) -> bool {
        self.left > 0
    }

    /// Puts the walls, the hills, the boulders at their toe and the rubble
    /// into the lane's world, once per world.
    pub fn attach(&mut self, lane: &mut Lane) {
        if self.attached == Some(lane.generation) {
            return;
        }
        let friction = lane.env.sim.params.friction_coefficient();
        let world = &mut lane.env.sim.world;
        for w in walls() {
            w.insert(world, friction);
        }
        insert_scenery(world, friction);
        self.rubble.insert(world, friction);
        self.attached = Some(lane.generation);
    }

    /// Before a policy step: the shove, if asked for and the blade is ready.
    pub fn before_step(&mut self, lane: &mut Lane) {
        self.cooldown = (self.cooldown - POLICY_DT).max(0.0);
        if !std::mem::take(&mut self.want_shove) || self.cooldown > 0.0 {
            return;
        }
        self.cooldown = SHOVE_EVERY;
        self.shoves += 1;
        let (p, heading) = torso_ground(lane);
        let done = self.rubble.shove(
            &mut lane.env.sim.world,
            [p[0], 0.0, p[2]],
            heading,
            SHOVE_REACH,
            SHOVE_HALF_WIDTH,
            SHOVE_HEIGHT,
            SHOVE_SPEED,
            SHOVE_REF_MASS,
        );
        // The blade striking rock rings it; swinging at nothing clicks.
        if done.rocks > 0 {
            self.impacts_audio
                .push([150.0 + 0.1 * done.mass.min(2500.0), 3.0, 0.0]);
        } else {
            self.ui_audio.push(0);
        }
    }

    /// After a policy step: the rocks read back, their sound, the objective.
    pub fn after_step(&mut self, lane: &Lane) {
        for (prev, rock) in self.prev.iter_mut().zip(&self.rubble.rocks) {
            *prev = rock.pose;
        }
        self.rubble.sync(&lane.env.sim.world);
        self.sound(lane);
        self.left = self.rubble.inside(&BLOCKING).0;

        let (pg, [fx, fz]) = torso_ground(lane);
        let upright = (lane.env.sim.torso().rotation() * Vector::Y).y > 0.9;
        let clear = self.rubble.rocks.iter().all(|r| {
            let [x, _, z] = r.position();
            (x - pg[0]).hypot(z - pg[2]) > SAFE_CLEARANCE
        });
        if upright && clear && lane.fallen_for.is_none() {
            self.safe = (pg[0], pg[2], (-fz).atan2(fx));
        }

        let p = lane.env.sim.torso().translation();
        let from_start = (p.x - START.0).hypot(p.z - START.1);
        let to_site = (SITE_X - SITE_RADIUS - p.x).max(0.0);
        let next = match self.stage {
            Stage::Leave if from_start > LEAVE_RADIUS => Stage::Travel,
            Stage::Travel | Stage::Leave if to_site <= 0.0 => Stage::Clear,
            Stage::Leave | Stage::Travel | Stage::Clear if !self.blocked() => Stage::Done,
            s => s,
        };
        if next != self.stage {
            if matches!(next, Stage::Clear | Stage::Done) {
                self.ui_audio.push(3);
            }
            info!("level: {:?} -> {:?}", self.stage, next);
            self.stage = next;
        }
    }

    /// The rocks' knocks and grinding, heard from the cockpit.
    fn sound(&mut self, lane: &Lane) {
        let (p, [fx, fz]) = torso_ground(lane);
        let right = [-fz, fx];
        self.slide_audio = 0.0;
        for (rock, prev_v) in self.rubble.rocks.iter().zip(&mut self.vel) {
            let v = rock.linvel;
            let dv = ((v[0] - prev_v[0]).powi(2)
                + (v[1] - prev_v[1]).powi(2)
                + (v[2] - prev_v[2]).powi(2))
            .sqrt();
            *prev_v = v;
            let [x, _, z] = rock.position();
            let (dx, dz) = (x - p[0], z - p[2]);
            let d = dx.hypot(dz).max(0.5);
            let near = 1.0 / (1.0 + d / 6.0);
            if dv > 1.0 {
                let pan = ((dx * right[0] + dz * right[1]) / d).clamp(-1.0, 1.0);
                self.rocks_audio.push([rock.mass * dv * near, pan]);
            }
            let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if speed > 0.2 {
                self.slide_audio += 0.001 * rock.mass * speed * near;
            }
        }
        // The loudest knocks first: the synthesizer takes a few a step.
        self.rocks_audio.sort_by(|a, b| b[0].total_cmp(&a[0]));
        self.rocks_audio.truncate(4);
    }

    /// Adds the level's sound to an audio step (`lanes::audio_step`'s JSON).
    pub fn audio_into(&mut self, step: &mut Value) {
        let Some(o) = step.as_object_mut() else {
            return;
        };
        o.insert("rocks".into(), json!(std::mem::take(&mut self.rocks_audio)));
        o.insert("slide".into(), json!(self.slide_audio));
        o.insert("ui".into(), json!(std::mem::take(&mut self.ui_audio)));
        if !self.impacts_audio.is_empty()
            && let Some(Value::Array(list)) = o.get_mut("impacts")
        {
            list.extend(self.impacts_audio.drain(..).map(|i| json!(i)));
        }
    }

    /// Stands a fallen mech up again at the last place it stood upright
    /// clear of the rubble, facing the way it faced there; the rocks stay
    /// as they lie. (Not where it fell: a mech stood up inside the heap
    /// would be thrown out of it.)
    pub fn stand_up(&mut self, lane: &mut Lane, cfg: &PathConfig) {
        let (x, z, heading) = self.safe;
        let params = lane.env.sim.params.clone();
        let command = lane.env.command;
        lane.reset_at(0, params, x, z, heading, cfg);
        lane.env.command = command;
    }

    /// The objective and progress for the page.
    pub fn status(&self, lane: &Lane) -> Value {
        let (_, [fx, fz]) = torso_ground(lane);
        let p = lane.env.sim.torso().translation();
        let to_site = (SITE_X - SITE_RADIUS - p.x).max(0.0);
        let (title, hint) = match self.stage {
            Stage::Leave => (
                "Leave the base".to_string(),
                "Walk out along the orange posts. W to walk, A/D to turn, V to change the view (on a gamepad: the left stick walks and turns, the right stick looks, Y changes the view)."
                    .to_string(),
            ),
            Stage::Travel => (
                format!("Reach the landslide: {to_site:.0} m"),
                "Follow the posts east to the pass under the hill.".to_string(),
            ),
            Stage::Clear => (
                format!(
                    "Clear the rubble: {} {} in the route's lane",
                    self.left,
                    if self.left == 1 { "rock" } else { "rocks" }
                ),
                "Shove the rocks out of the route's lane (E or Space, A on a gamepad), through the pass or against its sides."
                    .to_string(),
            ),
            Stage::Done => (
                "Route open".to_string(),
                "The pass is clear: the way east is open again. R starts over.".to_string(),
            ),
        };
        json!({
            "stage": self.stage.key(),
            "title": title,
            "hint": hint,
            "left": self.left,
            "total": self.total,
            "toSite": to_site,
            "shoves": self.shoves,
            "falls": lane.falls,
            "ready": self.cooldown <= 0.0,
            "x": p.x,
            "z": p.z,
            "heading": (-fz).atan2(fx),
        })
    }
}

/// The torso's position, and its heading on the ground (unit, x and z).
fn torso_ground(lane: &Lane) -> ([f32; 3], [f32; 2]) {
    let t = lane.env.sim.torso();
    let p = t.translation();
    let f = t.rotation() * Vector::X;
    let l = f.x.hypot(f.z).max(1e-4);
    ([p.x, p.y, p.z], [f.x / l, f.z / l])
}

/// The scenery the mech can walk into: the hillsides either side of the lot
/// (their toes reach into the play area well before the walls), as height
/// fields of the very heights they are drawn with, and the boulders washed
/// out at their fans. Fixed, like the walls.
fn insert_scenery(world: &mut PhysicsWorld, friction: f32) {
    let groups = InteractionGroups::new(Group::GROUP_2, Group::GROUP_1, InteractionTestMode::And);
    for hill in crate::scene::hills() {
        // Parry's height field: rows along z from its -z edge, columns
        // along x, centred on its body.
        let mut zx = Array2::zeros(hill.nz, hill.nx);
        for row in 0..hill.nz {
            let iz = if hill.dir_z > 0.0 {
                row
            } else {
                hill.nz - 1 - row
            };
            for ix in 0..hill.nx {
                zx[(row, ix)] = hill.h[iz * hill.nx + ix];
            }
        }
        let cz = hill.start_z + 0.5 * hill.dir_z * hill.depth_z;
        let body =
            world.insert_body(RigidBodyBuilder::fixed().translation(Vector::new(0.0, 0.0, cz)));
        world.insert_collider(
            ColliderBuilder::heightfield(zx, Vector::new(hill.width_x, 1.0, hill.depth_z))
                .friction(friction)
                .collision_groups(groups),
            Some(body),
        );
    }
    // A boulder's mesh is a chipped unit icosahedron: its hull, scaled.
    let phi = 0.5 * (1.0 + 5.0f32.sqrt());
    let ico: Vec<Vec3> = [
        [-1.0, phi, 0.0],
        [1.0, phi, 0.0],
        [-1.0, -phi, 0.0],
        [1.0, -phi, 0.0],
        [0.0, -1.0, phi],
        [0.0, 1.0, phi],
        [0.0, -1.0, -phi],
        [0.0, 1.0, -phi],
        [phi, 0.0, -1.0],
        [phi, 0.0, 1.0],
        [-phi, 0.0, -1.0],
        [-phi, 0.0, 1.0],
    ]
    .map(|v| Vec3::from(v).normalize())
    .to_vec();
    for b in crate::scene::boulders() {
        let pts: Vec<Vector> = ico
            .iter()
            .map(|v| Vector::from((*v * b.scale).to_array()))
            .collect();
        let Some(collider) = ColliderBuilder::convex_hull(&pts) else {
            continue;
        };
        let [x, y, z, w] = b.rotation.to_array();
        let body = world.insert_body(RigidBodyBuilder::fixed().pose(Pose::from_parts(
            Vector::from(b.position.to_array()),
            Rotation::from_xyzw(x, y, z, w),
        )));
        world.insert_collider(
            collider.friction(friction).collision_groups(groups),
            Some(body),
        );
    }
}

/// Lets a new pile fall and come to rest in a world of its own (the ground
/// and the walls), so the level starts with a heap rather than a rain of
/// rocks.
fn settle(rubble: &mut Rubble, secs: f32) {
    let mut world = PhysicsWorld::new();
    world.gravity = Vector::new(0.0, sim::GRAVITY, 0.0);
    world.integration_parameters.dt = sim::DT;
    world.integration_parameters.num_solver_iterations = sim::SOLVER_ITERATIONS;
    world
        .integration_parameters
        .num_internal_stabilization_iterations = 0;
    let friction = 0.9;
    Terrain::flat(400.0).insert(&mut world, friction);
    for w in walls() {
        w.insert(&mut world, friction);
    }
    rubble.insert(&mut world, friction);
    for _ in 0..(secs / sim::DT) as usize {
        world.step();
    }
    rubble.sync(&world);
    // At rest: what is left of the fall's speed is gone.
    for r in &mut rubble.rocks {
        r.linvel = [0.0; 3];
        r.angvel = [0.0; 3];
    }
}

// ── Drawing ─────────────────────────────────────────────────────────────

pub fn plugin(app: &mut App) {
    app.init_resource::<DrawnLevel>()
        .add_systems(Startup, spawn_scenery)
        .add_systems(
            Update,
            (show_scenery, sync_rocks)
                .chain()
                .after(crate::lanes::step_lanes),
        );
}

/// Everything the level adds to the scene: shown only in a level.
#[derive(Component)]
struct LevelScenery;

/// One drawn rock: its index in the rubble.
#[derive(Component)]
struct RockView(usize);

/// The beacon over the pass: amber while blocked, green once open.
#[derive(Component)]
struct Beacon;

#[derive(Resource, Default)]
struct DrawnLevel {
    serial: Option<u64>,
    rock_mat: Option<Handle<StandardMaterial>>,
    beacon_blocked: Option<Handle<StandardMaterial>>,
    beacon_open: Option<Handle<StandardMaterial>>,
}

fn spawn_scenery(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut drawn: ResMut<DrawnLevel>,
) {
    let mat = |m: &mut Assets<StandardMaterial>, c: Color, rough: f32, metal: f32| {
        m.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: rough,
            metallic: metal,
            ..default()
        })
    };
    let concrete = mat(&mut materials, Color::srgb(0.58, 0.56, 0.52), 0.9, 0.0);
    let rust = mat(&mut materials, Color::srgb(0.5, 0.27, 0.16), 0.8, 0.2);
    let ochre = mat(&mut materials, Color::srgb(0.62, 0.48, 0.24), 0.75, 0.15);
    let steel = mat(&mut materials, Color::srgb(0.42, 0.44, 0.45), 0.5, 0.4);
    let canvas = mat(&mut materials, Color::srgb(0.72, 0.68, 0.58), 0.95, 0.0);
    let green = mat(&mut materials, Color::srgb(0.24, 0.42, 0.16), 0.9, 0.0);
    let leaf = mat(&mut materials, Color::srgb(0.3, 0.52, 0.2), 0.85, 0.0);
    let soil = mat(&mut materials, Color::srgb(0.3, 0.22, 0.15), 0.95, 0.0);
    let paint = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.52, 0.12),
        perceptual_roughness: 0.6,
        ..default()
    });
    let mut put = |mesh: Mesh, m: &Handle<StandardMaterial>, t: Transform| {
        commands.spawn((
            LevelScenery,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(m.clone()),
            t,
            Visibility::Hidden,
        ));
    };

    // The home base, south of the start: a pad, the mech's bay marked out,
    // two containers, a workshop, a water tank, a mast, and the garden.
    put(
        Cuboid::new(16.0, 0.06, 11.0).into(),
        &concrete,
        Transform::from_xyz(-6.0, 0.03, -8.5),
    );
    for (dx, dz, w, d) in [
        (0.0, -2.6, 5.2, 0.15),
        (0.0, 2.6, 5.2, 0.15),
        (-2.6, 0.0, 0.15, 5.2),
    ] {
        put(
            Cuboid::new(w, 0.02, d).into(),
            &paint,
            Transform::from_xyz(START.0 + dx, 0.012, START.1 + dz),
        );
    }
    put(
        Cuboid::new(6.0, 2.6, 2.4).into(),
        &rust,
        Transform::from_xyz(-11.0, 1.36, -11.5).with_rotation(Quat::from_rotation_y(0.06)),
    );
    put(
        Cuboid::new(6.0, 2.6, 2.4).into(),
        &ochre,
        Transform::from_xyz(-11.3, 1.36, -8.6).with_rotation(Quat::from_rotation_y(-0.04)),
    );
    put(
        Cuboid::new(6.0, 2.6, 2.4).into(),
        &rust,
        Transform::from_xyz(-11.1, 3.96, -10.0).with_rotation(Quat::from_rotation_y(0.5)),
    );
    // The workshop: walls, a lean-to roof, an open door.
    put(
        Cuboid::new(5.5, 3.4, 4.5).into(),
        &steel,
        Transform::from_xyz(-3.5, 1.76, -11.0),
    );
    put(
        Cuboid::new(6.2, 0.12, 5.4).into(),
        &rust,
        Transform::from_xyz(-3.5, 3.6, -11.0).with_rotation(Quat::from_rotation_x(0.12)),
    );
    put(
        Cuboid::new(2.4, 2.6, 0.05).into(),
        &soil,
        Transform::from_xyz(-3.0, 1.36, -8.73),
    );
    put(
        Cylinder::new(1.4, 3.2).into(),
        &canvas,
        Transform::from_xyz(2.5, 1.66, -12.0),
    );
    put(
        Cylinder::new(1.5, 0.15).into(),
        &steel,
        Transform::from_xyz(2.5, 3.3, -12.0),
    );
    put(
        Cylinder::new(0.08, 10.0).into(),
        &steel,
        Transform::from_xyz(-14.5, 5.0, -5.0),
    );
    put(
        Cuboid::new(2.0, 0.08, 0.08).into(),
        &steel,
        Transform::from_xyz(-14.5, 9.2, -5.0),
    );
    // The green spot: tilled soil, rows of plants, a little tree.
    put(
        Cuboid::new(9.0, 0.08, 5.0).into(),
        &soil,
        Transform::from_xyz(-5.0, 0.04, -17.0),
    );
    let mut r = Rng::new(51);
    for row in 0..4 {
        for k in 0..9 {
            let s = r.range(0.25, 0.45);
            put(
                Sphere::new(1.0).into(),
                &leaf,
                Transform::from_xyz(
                    -9.0 + k as f32 + r.range(-0.15, 0.15),
                    0.1 + 0.4 * s,
                    -18.8 + row as f32 * 1.2,
                )
                .with_scale(Vec3::new(s, 0.7 * s, s)),
            );
        }
    }
    put(
        Cylinder::new(0.12, 2.2).into(),
        &soil,
        Transform::from_xyz(0.5, 1.1, -17.5),
    );
    put(
        Sphere::new(1.3).into(),
        &green,
        Transform::from_xyz(0.5, 2.7, -17.5).with_scale(Vec3::new(1.0, 0.85, 1.0)),
    );

    // The pass: two spurs of hill, the north one slumped where it let go.
    let hill_tex = images.add(crate::scene::hillside_detail_texture(256));
    let hill = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(hill_tex),
        perceptual_roughness: 0.92,
        reflectance: 0.16,
        uv_transform: Affine2::from_scale(Vec2::splat(8.0)),
        ..default()
    });
    let w = walls();
    for (k, wall) in w.iter().take(2).enumerate() {
        let north = k == 0;
        let mesh = spur_mesh(
            wall.centre[0],
            wall.centre[2],
            wall.half[0] + 2.5,
            wall.half[2] + 2.5,
            if north { 9.0 } else { 7.5 },
            if north { Some(PASS_Z[1]) } else { None },
            101 + k as u64,
        );
        put(mesh, &hill, Transform::IDENTITY);
    }

    // The beacon at the pass's mouth: where to go.
    let blocked = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.6, 0.15),
        emissive: LinearRgba::rgb(9.0, 3.6, 0.5),
        unlit: true,
        ..default()
    });
    let open = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 1.0, 0.5),
        emissive: LinearRgba::rgb(1.0, 8.0, 2.0),
        unlit: true,
        ..default()
    });
    let bx = SITE_X - SPUR_HALF_X - 1.5;
    commands.spawn((
        LevelScenery,
        Mesh3d(meshes.add(Cylinder::new(0.09, 6.0))),
        MeshMaterial3d(steel.clone()),
        Transform::from_xyz(bx, 3.0, PASS_Z[0] - 0.8),
        Visibility::Hidden,
    ));
    commands.spawn((
        LevelScenery,
        Beacon,
        Mesh3d(meshes.add(Sphere::new(0.32))),
        MeshMaterial3d(blocked.clone()),
        Transform::from_xyz(bx, 6.2, PASS_Z[0] - 0.8),
        Visibility::Hidden,
        NotShadowCaster,
    ));

    drawn.rock_mat = Some(materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.55, 0.47),
        perceptual_roughness: 0.9,
        reflectance: 0.2,
        ..default()
    }));
    drawn.beacon_blocked = Some(blocked);
    drawn.beacon_open = Some(open);
}

fn show_scenery(
    mode: Res<Mode>,
    lanes: Res<Lanes>,
    drawn: Res<DrawnLevel>,
    mut q: Query<&mut Visibility, With<LevelScenery>>,
    mut beacon: Query<&mut MeshMaterial3d<StandardMaterial>, With<Beacon>>,
) {
    let on = *mode == Mode::Level && lanes.level.is_some();
    let want = if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut q {
        if *v != want {
            *v = want;
        }
    }
    let open = lanes.level.as_ref().is_some_and(|l| !l.blocked());
    let m = if open {
        &drawn.beacon_open
    } else {
        &drawn.beacon_blocked
    };
    if let Some(m) = m {
        for mut b in &mut beacon {
            if b.0 != *m {
                b.0 = m.clone();
            }
        }
    }
}

/// Draws the rubble: a mesh per rock from the hull the physics uses, placed
/// between its last two policy steps.
fn sync_rocks(
    mut commands: Commands,
    lanes: Res<Lanes>,
    mut drawn: ResMut<DrawnLevel>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rocks: Query<(Entity, &RockView, &mut Transform)>,
) {
    let serial = lanes.level.as_ref().map(|l| l.serial);
    if serial != drawn.serial {
        for (e, _, _) in &rocks {
            commands.entity(e).despawn();
        }
        drawn.serial = serial;
        let (Some(level), Some(mat)) = (&lanes.level, &drawn.rock_mat) else {
            return;
        };
        let mut r = Rng::new(level.serial);
        for (i, rock) in level.rubble.rocks.iter().enumerate() {
            commands.spawn((
                RockView(i),
                Mesh3d(meshes.add(hull_mesh(&rock.hull(), &mut r))),
                MeshMaterial3d(mat.clone()),
                pose_transform(&rock.pose, &rock.pose, 1.0),
            ));
        }
        return;
    }
    let Some(level) = &lanes.level else { return };
    for (_, view, mut tf) in &mut rocks {
        if let (Some(a), Some(rock)) = (level.prev.get(view.0), level.rubble.rocks.get(view.0)) {
            *tf = pose_transform(a, &rock.pose, lanes.blend);
        }
    }
}

fn pose_transform(a: &BodyPose, b: &BodyPose, t: f32) -> Transform {
    let ra = Quat::from_array(a.rotation);
    let rb = Quat::from_array(b.rotation);
    Transform {
        translation: Vec3::from(a.position).lerp(Vec3::from(b.position), t),
        rotation: ra.slerp(rb, t),
        scale: Vec3::ONE,
    }
}

/// A rock's hull as a flat-shaded mesh, each face a slightly different
/// shade of stone.
fn hull_mesh((verts, tris): &(Vec<[f32; 3]>, Vec<[u32; 3]>), r: &mut Rng) -> Mesh {
    let tint = r.range(-0.05, 0.05);
    let mut pos = Vec::with_capacity(tris.len() * 3);
    let mut nor = Vec::with_capacity(tris.len() * 3);
    let mut col = Vec::with_capacity(tris.len() * 3);
    for t in tris {
        let [a, b, c] = t.map(|i| Vec3::from(verts[i as usize]));
        let n = (b - a).cross(c - a).normalize_or(Vec3::Y);
        let shade = 0.82 + 0.3 * r.f32() + tint;
        let rgba = [shade, shade * 0.97, shade * 0.93, 1.0];
        for p in [a, b, c] {
            pos.push(p.to_array());
            nor.push(n.to_array());
            col.push(rgba);
        }
    }
    let n = pos.len() as u32;
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32((0..n).collect()));
    m
}

/// A spur of hill over a wall: a steep-sided rounded mound `half_x` by
/// `half_z` m about (cx, cz), `height` m at its crest, rough with noise.
/// `scar_z`: the side facing that z let go in the slide, and is fresh,
/// paler ground.
fn spur_mesh(
    cx: f32,
    cz: f32,
    half_x: f32,
    half_z: f32,
    height: f32,
    scar_z: Option<f32>,
    seed: u64,
) -> Mesh {
    let (nx, nz) = (40usize, 60usize);
    let s = seed as f32 * 0.77;
    let h = |x: f32, z: f32| -> f32 {
        let u = (x - cx) / half_x;
        let v = (z - cz) / half_z;
        // A superellipse: flat-topped, falling steeply at its edge.
        let d = (u.abs().powf(4.0) + v.abs().powf(4.0)).powf(0.25);
        let edge = ((1.0 - d) / 0.28).clamp(0.0, 1.0);
        let shape = edge * edge * (3.0 - 2.0 * edge);
        let n = (0.35 * x + s).sin() * (0.27 * z - s).cos()
            + 0.5 * (0.9 * x - 0.6 * z + 2.0 * s).sin()
            + 0.25 * (2.1 * x + 1.7 * z).sin();
        (height * shape * (1.0 + 0.12 * n)).max(0.0) - 0.05
    };
    let mut pos = Vec::with_capacity(nx * nz);
    let mut nor = Vec::with_capacity(nx * nz);
    let mut uv = Vec::with_capacity(nx * nz);
    let mut col = Vec::with_capacity(nx * nz);
    let rock = Color::srgb(0.36, 0.33, 0.3).to_linear();
    let earth = Color::srgb(0.5, 0.42, 0.33).to_linear();
    let scar = Color::srgb(0.66, 0.58, 0.47).to_linear();
    let e = 0.25;
    for iz in 0..nz {
        for ix in 0..nx {
            let x = cx - half_x + 2.0 * half_x * ix as f32 / (nx - 1) as f32;
            let z = cz - half_z + 2.0 * half_z * iz as f32 / (nz - 1) as f32;
            let y = h(x, z);
            pos.push([x, y, z]);
            let n = Vec3::new(
                h(x - e, z) - h(x + e, z),
                2.0 * e,
                h(x, z - e) - h(x, z + e),
            )
            .normalize_or(Vec3::Y);
            nor.push(n.to_array());
            uv.push([x / 8.0, z / 8.0]);
            let steep = (1.0 - n.y).clamp(0.0, 1.0);
            let t = (steep * 2.2).clamp(0.0, 1.0);
            let mut c = [
                earth.red + (rock.red - earth.red) * t,
                earth.green + (rock.green - earth.green) * t,
                earth.blue + (rock.blue - earth.blue) * t,
                1.0,
            ];
            if let Some(sz) = scar_z {
                // The slide's scar: the face toward the pass, near the
                // pass's middle, freshly bared.
                let near = (1.0 - (z - sz).abs() / 9.0).clamp(0.0, 1.0)
                    * (1.0 - ((x - cx) / (0.7 * half_x)).abs()).clamp(0.0, 1.0);
                let k = (near * 1.6).min(1.0);
                c[0] += (scar.red - c[0]) * k;
                c[1] += (scar.green - c[1]) * k;
                c[2] += (scar.blue - c[2]) * k;
            }
            col.push(c);
        }
    }
    let mut idx = Vec::with_capacity((nx - 1) * (nz - 1) * 6);
    for iz in 0..(nz - 1) {
        for ix in 0..(nx - 1) {
            let a = (iz * nx + ix) as u32;
            let (b, c) = (a + 1, a + nx as u32);
            let d = c + 1;
            idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level_lane() -> (Lanes, PathConfig) {
        let mut lanes = Lanes::default();
        lanes.start_level();
        let cfg = lanes.path_cfg.clone();
        (lanes, cfg)
    }

    #[test]
    fn the_pile_blocks_the_pass_at_the_start() {
        let level = Level::new(1);
        assert!(level.blocked());
        // Most of the heap came to rest in the pass, a good part of it in
        // the route's lane.
        let gap = Volume {
            min: [SITE_X - SPUR_HALF_X, -1.0, PASS_Z[0]],
            max: [SITE_X + SPUR_HALF_X, 6.0, PASS_Z[1]],
        };
        let in_gap = level.rubble.inside(&gap).0;
        assert!(
            in_gap >= ROCKS * 3 / 4,
            "only {in_gap} of {ROCKS} rocks in the pass"
        );
        assert!(
            level.total >= ROCKS / 2,
            "only {} of {ROCKS} rocks in the route's lane",
            level.total
        );
        for r in &level.rubble.rocks {
            let [x, y, z] = r.position();
            assert!(x.is_finite() && y > -0.5 && y < 4.0 && z.is_finite());
        }
    }

    #[test]
    fn the_walls_close_the_pass_but_for_its_gap() {
        let w = walls();
        let north = &w[0];
        let south = &w[1];
        assert!((north.centre[2] - north.half[2] - PASS_Z[1]).abs() < 1e-4);
        assert!((south.centre[2] + south.half[2] - PASS_Z[0]).abs() < 1e-4);
        // The spurs reach the play area's edges: no way round.
        assert!(north.centre[2] + north.half[2] >= w[2].centre[2] - w[2].half[2] - 1e-4);
        assert!(south.centre[2] - south.half[2] <= w[3].centre[2] + w[3].half[2] + 1e-4);
    }

    /// The hills' toes and the boulders at them are solid: a ball dropped on
    /// the highest ground the near hill reaches inside the play area, and
    /// one on a boulder, land on them, not on the lot under them.
    #[test]
    fn the_hills_and_their_boulders_are_solid() {
        let hill = &crate::scene::hills()[0];
        let north_wall = walls()[2].centre[2] - walls()[2].half[2];
        let mut best = (0.0, 0.0, 0.0);
        let mut z = hill.start_z;
        while z < north_wall {
            for i in 0..200 {
                let x = -60.0 + i as f32;
                let h = hill.height(x, z);
                if h > best.2 {
                    best = (x, z, h);
                }
            }
            z += 0.5;
        }
        // The dune the mech used to walk through.
        assert!(best.2 > 1.0, "the toe only reaches {:.2} m", best.2);
        let b = crate::scene::boulders()
            .iter()
            .max_by(|a, b| a.scale.y.total_cmp(&b.scale.y))
            .unwrap();
        let top = b.position.y + b.scale.y * 0.8;

        let mut world = PhysicsWorld::new();
        world.gravity = Vector::new(0.0, sim::GRAVITY, 0.0);
        world.integration_parameters.dt = sim::DT;
        Terrain::flat(400.0).insert(&mut world, 0.9);
        insert_scenery(&mut world, 0.9);
        let r = 0.2;
        let ball = |world: &mut PhysicsWorld, x: f32, y: f32, z: f32| {
            let h =
                world.insert_body(RigidBodyBuilder::dynamic().translation(Vector::new(x, y, z)));
            world.insert_collider(ColliderBuilder::cuboid(r, r, r).friction(1.0), Some(h));
            h
        };
        let on_hill = ball(&mut world, best.0, best.2 + 1.0, best.1);
        let on_rock = ball(&mut world, b.position.x, top + 1.0, b.position.z);
        // Long enough to land (1 m takes 0.45 s), too short to slide far.
        for _ in 0..(0.8 / sim::DT) as usize {
            world.step();
        }
        let y = |h: mw_sim::rapier3d::prelude::RigidBodyHandle| world.bodies[h].translation().y;
        assert!(
            y(on_hill) > best.2 - 0.5,
            "the ball fell through the hill to {:.2} (ground {:.2})",
            y(on_hill),
            best.2
        );
        assert!(
            y(on_rock) > r + 0.1,
            "the ball fell through the boulder to {:.2}",
            y(on_rock)
        );
    }

    /// The objective flow, driven without a policy: the mech is carried
    /// along the route (standing up again at each place), then parked while
    /// a blade of the mech's shape is swept across the pass, advancing as
    /// the rocks go, until the pass is clear. (Standing the mech up inside
    /// the heap would throw the rocks clear by itself.)
    #[test]
    fn leave_reach_clear_and_the_route_opens() {
        let (mut lanes, cfg) = level_lane();
        let Lanes {
            lanes: list, level, ..
        } = &mut lanes;
        let lane = &mut list[0];
        let level = level.as_mut().unwrap();
        let step = |level: &mut Level, lane: &mut Lane| {
            level.attach(lane);
            level.before_step(lane);
            lane.env.step(&[0.0; mw_sim::mech::JOINTS]);
            level.after_step(lane);
        };
        let params = lane.env.sim.params.clone();
        let at = |level: &mut Level, lane: &mut Lane, x: f32, z: f32| {
            lane.reset_at(0, params.clone(), x, z, 0.0, &cfg);
            step(level, lane);
        };
        assert_eq!(level.stage, Stage::Leave);
        at(level, lane, 20.0, 0.0);
        assert_eq!(level.stage, Stage::Travel);
        at(level, lane, SITE_X - SITE_RADIUS + 1.0, -1.5);
        assert_eq!(level.stage, Stage::Clear);
        assert!(level.blocked());

        // The blade works from the mech's own place: one shove from there.
        level.want_shove = true;
        step(level, lane);
        assert_eq!(level.shoves, 1);

        // Sweep: three lines across the pass, advancing as the rocks go.
        at(level, lane, SITE_X - 20.0, -1.5);
        let mut x = SITE_X - 8.5;
        let mut shoves = 0;
        'sweep: while x < SITE_X + 8.0 {
            for z in [-3.8, -1.5, 0.8] {
                for _ in 0..3 {
                    level.rubble.shove(
                        &mut lane.env.sim.world,
                        [x, 0.0, z],
                        [1.0, 0.0],
                        SHOVE_REACH,
                        SHOVE_HALF_WIDTH,
                        SHOVE_HEIGHT,
                        SHOVE_SPEED,
                        SHOVE_REF_MASS,
                    );
                    for _ in 0..(SHOVE_EVERY / POLICY_DT) as usize + 1 {
                        step(level, lane);
                    }
                    shoves += 1;
                }
                if !level.blocked() {
                    break 'sweep;
                }
            }
            // Up to the nearest rock still in the way, less the blade's reach.
            let nearest = level
                .rubble
                .rocks
                .iter()
                .map(|r| r.position())
                .filter(|p| BLOCKING.contains(*p))
                .map(|p| p[0])
                .fold(f32::INFINITY, f32::min);
            x = (x + 0.5).max(nearest - 0.5 * SHOVE_REACH);
        }
        eprintln!(
            "cleared with {shoves} shoves; {} of {} left",
            level.left, level.total
        );
        assert!(!level.blocked(), "{} rocks still in the pass", level.left);
        assert_eq!(level.stage, Stage::Done);
        let s = level.status(lane);
        assert_eq!(s["stage"], "done");
    }
}
