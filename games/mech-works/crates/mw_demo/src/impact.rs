//! Demo 1, the impact range: projectiles fly at the selected mech and hit a
//! random part of it (or the one chosen). A hit is an impulse on that link
//! at the point struck; the stagger and the catch are the policy's.

use bevy::prelude::*;
use mw_sim::rapier3d::prelude::Vector;
use mw_sim::rng::Rng;
use serde_json::Value;

use crate::Mode;
use crate::lanes::Lanes;

/// What the page sets: how heavy and fast, how often, at what.
#[derive(Resource, Debug, Clone)]
pub struct Range {
    /// Projectile mass, kg.
    pub mass: f32,
    /// Projectile speed, m/s.
    pub speed: f32,
    /// Shots a second (0: only when fired).
    pub rate: f32,
    /// The link to aim at, or any.
    pub target: Option<usize>,
    pub fire_now: bool,
    next: f32,
    rng: Rng,
    pub hits: u32,
}

impl Default for Range {
    fn default() -> Self {
        Self {
            mass: 40.0,
            speed: 60.0,
            rate: 0.5,
            target: None,
            fire_now: false,
            next: 1.0,
            rng: Rng::new(99),
            hits: 0,
        }
    }
}

impl Range {
    /// `range {mass?, speed?, rate?, target?}`, `fire`.
    pub fn apply(&mut self, v: &Value) {
        let num = |k: &str| v.get(k).and_then(Value::as_f64).map(|x| x as f32);
        match v.get("cmd").and_then(Value::as_str) {
            Some("range") => {
                self.mass = num("mass").unwrap_or(self.mass).clamp(1.0, 2000.0);
                self.speed = num("speed").unwrap_or(self.speed).clamp(5.0, 400.0);
                self.rate = num("rate").unwrap_or(self.rate).clamp(0.0, 10.0);
                if let Some(t) = v.get("target") {
                    self.target = t.as_u64().map(|x| x as usize);
                }
            }
            Some("fire") => self.fire_now = true,
            _ => {}
        }
    }
}

#[derive(Component)]
struct Shot {
    lane: usize,
    link: usize,
    velocity: Vec3,
    /// The point on the link, in the link's frame, it will strike.
    aim_local: Vec3,
    life: f32,
    mass: f32,
}

#[derive(Component)]
struct Spark {
    life: f32,
}

#[derive(Resource)]
struct ShotAssets {
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Range>()
        .add_systems(Startup, assets)
        .add_systems(
            Update,
            (fire, fly, fade).chain().after(crate::lanes::step_lanes),
        );
}

fn assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ShotAssets {
        mesh: meshes.add(Sphere::new(1.0)),
        mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.2, 0.22),
            emissive: LinearRgba::rgb(2.0, 1.2, 0.4),
            ..default()
        }),
        spark: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.4),
            emissive: LinearRgba::rgb(30.0, 15.0, 4.0),
            unlit: true,
            ..default()
        }),
    });
}

fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mode: Res<Mode>,
    lanes: Res<Lanes>,
    mut range: ResMut<Range>,
    assets: Res<ShotAssets>,
) {
    if *mode != Mode::Impact && !range.fire_now {
        return;
    }
    let mut go = std::mem::take(&mut range.fire_now);
    if *mode == Mode::Impact && range.rate > 0.0 && !lanes.paused {
        range.next -= time.delta_secs() * lanes.speed;
        if range.next <= 0.0 {
            range.next = 1.0 / range.rate * range.rng.range(0.6, 1.4);
            go = true;
        }
    }
    if !go {
        return;
    }
    let lane = lanes.selected;
    let Some(l) = lanes.lanes.get(lane) else {
        return;
    };
    let poses = &l.cur;
    let n = poses.len();
    let link = range.target.unwrap_or_else(|| range.rng.below(n)) % n;
    let p = &poses[link];
    let centre = Vec3::from(p.position);
    // From a random direction around, a little above.
    let angle = range.rng.range(0.0, std::f32::consts::TAU);
    let from = centre
        + Vec3::new(
            angle.cos() * 25.0,
            range.rng.range(0.5, 4.0),
            angle.sin() * 25.0,
        );
    let aim_local = Vec3::new(range.rng.signed(), range.rng.signed(), range.rng.signed()) * 0.15;
    let velocity = (centre - from).normalize() * range.speed;
    let radius = 0.04 * range.mass.cbrt();
    commands.spawn((
        Shot {
            lane,
            link,
            velocity,
            aim_local,
            life: 3.0,
            mass: range.mass,
        },
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(assets.mat.clone()),
        Transform::from_translation(from).with_scale(Vec3::splat(radius)),
    ));
}

/// Moves each shot toward its link (homing on where the link is now: the
/// mech moves while it flies) and strikes when it arrives.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut lanes: ResMut<Lanes>,
    mut range: ResMut<Range>,
    assets: Res<ShotAssets>,
    mut shots: Query<(Entity, &mut Shot, &mut Transform)>,
) {
    let dt = time.delta_secs() * lanes.speed;
    for (e, mut s, mut tf) in &mut shots {
        s.life -= dt;
        let Some(l) = lanes.lanes.get(s.lane) else {
            commands.entity(e).despawn();
            continue;
        };
        let Some(p) = l.cur.get(s.link) else {
            commands.entity(e).despawn();
            continue;
        };
        let rot = Quat::from_array(p.rotation);
        let aim = Vec3::from(p.position) + rot * s.aim_local;
        let to = aim - tf.translation;
        let step = s.velocity.length() * dt;
        if to.length() <= step.max(0.05) || s.life <= 0.0 {
            if s.life > 0.0 {
                // Strike: the projectile's momentum into the link at the point.
                let dir = s.velocity.normalize();
                let impulse = dir * s.mass * s.velocity.length();
                let lane = &mut lanes.lanes[s.lane];
                lane.env.sim.push_at(
                    s.link,
                    Vector::new(impulse.x, impulse.y, impulse.z),
                    Vector::new(aim.x, aim.y, aim.z),
                );
                range.hits += 1;
                commands.spawn((
                    Spark { life: 0.25 },
                    Mesh3d(assets.mesh.clone()),
                    MeshMaterial3d(assets.spark.clone()),
                    Transform::from_translation(aim).with_scale(Vec3::splat(0.35)),
                ));
            }
            commands.entity(e).despawn();
            continue;
        }
        s.velocity = to.normalize() * s.velocity.length();
        tf.translation += s.velocity * dt;
    }
}

fn fade(
    mut commands: Commands,
    time: Res<Time>,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform)>,
) {
    for (e, mut s, mut tf) in &mut sparks {
        s.life -= time.delta_secs();
        tf.scale *= 1.0 + 6.0 * time.delta_secs();
        if s.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
