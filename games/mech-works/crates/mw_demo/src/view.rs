//! The mech as drawn: each link at its pose between the last two policy
//! steps, dressed in parts built from primitives (armour plates, actuator
//! housings, pistons) over its collision shape, which the parts follow in
//! size (a longer leg gets longer plates). In the inspector each actuator's
//! link is tinted by its winding temperature.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use mw_sim::actuator::{AMBIENT, TRIP};
use mw_sim::sim::{BodyPose, LinkShape};

use crate::lanes::Lanes;

/// Draw the inspector's heat tint.
#[derive(Resource, Debug, Default)]
pub struct Inspector(pub bool);

#[derive(Component)]
struct LinkView {
    lane: usize,
    link: usize,
}

/// A lane's drawn links, kept to rebuild when a body (leg lengths) changes.
#[derive(Resource, Default)]
struct Drawn {
    lanes: Vec<(Vec<LinkShape>, Vec<Entity>)>,
}

#[derive(Resource)]
struct Palette {
    hull: Handle<StandardMaterial>,
    accent: Handle<StandardMaterial>,
    joint: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    chrome: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
}

/// A part's finish.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Finish {
    Hull,
    Accent,
    Joint,
    Dark,
    Chrome,
    Glow,
}

impl Palette {
    fn get(&self, f: Finish) -> &Handle<StandardMaterial> {
        match f {
            Finish::Hull => &self.hull,
            Finish::Accent => &self.accent,
            Finish::Joint => &self.joint,
            Finish::Dark => &self.dark,
            Finish::Chrome => &self.chrome,
            Finish::Glow => &self.glow,
        }
    }
}

/// One drawn part of a link, in the link's frame.
struct Part {
    mesh: Mesh,
    finish: Finish,
    at: Transform,
}

fn part(mesh: impl Into<Mesh>, finish: Finish, at: Vec3) -> Part {
    Part {
        mesh: mesh.into(),
        finish,
        at: Transform::from_translation(at),
    }
}

fn plate(size: [f32; 3], finish: Finish, at: Vec3) -> Part {
    part(Cuboid::new(size[0], size[1], size[2]), finish, at)
}

impl Part {
    fn rot(mut self, q: Quat) -> Self {
        self.at.rotation = q;
        self
    }
}

/// A cylinder about the link's Z axis (the pitch joints' axis).
fn drum_z(radius: f32, width: f32, finish: Finish, at: Vec3) -> Part {
    part(Cylinder::new(radius, width), finish, at).rot(Quat::from_rotation_x(FRAC_PI_2))
}

/// A cylinder about X (the roll joints' axis).
fn drum_x(radius: f32, width: f32, finish: Finish, at: Vec3) -> Part {
    part(Cylinder::new(radius, width), finish, at).rot(Quat::from_rotation_z(FRAC_PI_2))
}

/// Link `link`'s parts, from its collision shape. Links per leg: hip yaw,
/// hip roll, thigh (from the hip pitch joint, down -Y), shin (from the knee),
/// ankle, foot. X is forward, Y up, Z right.
fn parts(link: usize, shape: &LinkShape) -> Vec<Part> {
    use Finish::*;
    if link == 0 {
        return torso();
    }
    let side = if link <= 6 { -1.0 } else { 1.0 };
    let length = |s: &LinkShape| match *s {
        LinkShape::Capsule { offset, .. } => -2.0 * offset[1],
        _ => 0.9,
    };
    match (link - 1) % 6 {
        // Hip yaw: a vertical turret under the torso.
        0 => vec![
            part(Cylinder::new(0.2, 0.26), Joint, Vec3::new(0.0, 0.05, 0.0)),
            part(Cylinder::new(0.23, 0.05), Chrome, Vec3::new(0.0, 0.17, 0.0)),
        ],
        // Hip roll: a drum fore and aft.
        1 => vec![
            drum_x(0.17, 0.42, Joint, Vec3::ZERO),
            drum_x(0.19, 0.06, Chrome, Vec3::new(0.18, 0.0, 0.0)),
        ],
        2 => {
            let l = length(shape);
            vec![
                // The hip pitch actuator: the big drum, with a bright cap outboard.
                drum_z(0.27, 0.46, Joint, Vec3::ZERO),
                drum_z(0.2, 0.04, Accent, Vec3::new(0.0, 0.0, side * 0.25)),
                // Thigh armour, wide at the hip and narrowing to the knee.
                plate(
                    [0.48, 0.55 * l, 0.46],
                    Hull,
                    Vec3::new(0.03, -0.38 * l, 0.0),
                ),
                plate([0.49, 0.06, 0.47], Accent, Vec3::new(0.03, -0.2 * l, 0.0)),
                plate(
                    [0.34, 0.3 * l, 0.32],
                    Joint,
                    Vec3::new(0.02, -0.76 * l, 0.0),
                ),
                plate([0.08, 0.5 * l, 0.26], Dark, Vec3::new(0.25, -0.42 * l, 0.0))
                    .rot(Quat::from_rotation_z(0.06)),
                // The knee actuator's piston up the back of the thigh.
                part(
                    Cylinder::new(0.065, 0.55 * l),
                    Chrome,
                    Vec3::new(-0.24, -0.5 * l, 0.0),
                ),
                part(
                    Cylinder::new(0.09, 0.3 * l),
                    Dark,
                    Vec3::new(-0.24, -0.25 * l, 0.0),
                ),
                drum_z(0.2, 0.38, Joint, Vec3::new(0.0, -l, 0.0)),
            ]
        }
        3 => {
            let l = length(shape);
            vec![
                // Knee cap and shin guard in front, the frame behind.
                plate([0.24, 0.3, 0.3], Accent, Vec3::new(0.16, -0.05, 0.0))
                    .rot(Quat::from_rotation_z(-0.3)),
                plate(
                    [0.12, 0.7 * l, 0.28],
                    Accent,
                    Vec3::new(0.15, -0.5 * l, 0.0),
                )
                .rot(Quat::from_rotation_z(-0.04)),
                plate([0.26, 0.85 * l, 0.26], Hull, Vec3::new(0.0, -0.5 * l, 0.0)),
                part(
                    Cylinder::new(0.055, 0.5 * l),
                    Chrome,
                    Vec3::new(-0.17, -0.55 * l, 0.0),
                ),
                part(
                    Cylinder::new(0.08, 0.25 * l),
                    Dark,
                    Vec3::new(-0.17, -0.3 * l, 0.0),
                ),
            ]
        }
        // Ankle: a pitch drum.
        4 => vec![drum_z(0.15, 0.34, Joint, Vec3::ZERO)],
        _ => {
            let LinkShape::Cuboid { half, offset } = *shape else {
                return vec![];
            };
            let o = Vec3::from(offset);
            vec![
                // The sole as collided, then a toe cap and heel over it.
                plate([2.0 * half[0], 2.0 * half[1], 2.0 * half[2]], Dark, o),
                plate([0.42, 0.12, 0.38], Hull, o + Vec3::new(0.14, 0.1, 0.0))
                    .rot(Quat::from_rotation_z(-0.12)),
                plate([0.2, 0.16, 0.34], Accent, o + Vec3::new(-0.24, 0.1, 0.0)),
                plate([0.1, 0.1, 0.42], Joint, o + Vec3::new(0.36, 0.0, 0.0)),
            ]
        }
    }
}

/// The torso: a hull around the collision box (centre 0.55 m above the
/// hips, 1.3 × 1.2 × 1.5 m), a sloped glacis with the visor, shoulder
/// pods, a reactor pack behind, and a skirt over the hips.
fn torso() -> Vec<Part> {
    use Finish::*;
    let c = 0.55;
    let mut v = vec![
        plate([1.2, 0.95, 1.4], Hull, Vec3::new(0.0, c, 0.0)),
        plate([1.05, 0.3, 1.5], Hull, Vec3::new(-0.05, c + 0.55, 0.0)),
        // The glacis, leaning back, and the visor slit across it.
        plate([0.16, 0.85, 1.25], Hull, Vec3::new(0.63, c + 0.05, 0.0))
            .rot(Quat::from_rotation_z(-0.3)),
        plate([0.17, 0.08, 1.26], Accent, Vec3::new(0.56, c - 0.22, 0.0))
            .rot(Quat::from_rotation_z(-0.3)),
        // The cockpit on top, forward, with its visor.
        plate([0.62, 0.34, 0.78], Hull, Vec3::new(0.3, c + 0.84, 0.0)),
        plate([0.2, 0.3, 0.8], Dark, Vec3::new(0.6, c + 0.8, 0.0))
            .rot(Quat::from_rotation_z(-0.45)),
        plate([0.05, 0.08, 0.66], Glow, Vec3::new(0.73, c + 0.82, 0.0))
            .rot(Quat::from_rotation_z(-0.45)),
        // The reactor pack and its stacks.
        plate([0.45, 0.75, 1.0], Dark, Vec3::new(-0.78, c + 0.1, 0.0)),
        plate([0.1, 0.5, 0.8], Joint, Vec3::new(-1.03, c + 0.1, 0.0)),
        // The skirt over the hips.
        plate([1.0, 0.22, 1.2], Dark, Vec3::new(0.0, -0.02, 0.0)),
        plate([0.18, 0.3, 1.0], Joint, Vec3::new(0.5, 0.0, 0.0)).rot(Quat::from_rotation_z(0.25)),
    ];
    for z in [-1.0f32, 1.0] {
        // Shoulder pods with a stripe, a stack, a lamp.
        v.push(plate(
            [0.95, 0.5, 0.28],
            Hull,
            Vec3::new(0.0, c + 0.25, z * 0.84),
        ));
        v.push(plate(
            [0.7, 0.1, 0.3],
            Accent,
            Vec3::new(0.05, c + 0.38, z * 0.86),
        ));
        v.push(plate(
            [0.7, 0.05, 0.3],
            Dark,
            Vec3::new(0.05, c + 0.12, z * 0.86),
        ));
        v.push(part(
            Cylinder::new(0.1, 0.45),
            Chrome,
            Vec3::new(-0.85, c + 0.7, z * 0.28),
        ));
        v.push(part(
            Cylinder::new(0.12, 0.08),
            Dark,
            Vec3::new(-0.85, c + 0.95, z * 0.28),
        ));
        v.push(part(
            Sphere::new(0.06),
            Glow,
            Vec3::new(0.48, c + 0.75, z * 0.5),
        ));
    }
    // An antenna.
    v.push(part(
        Cylinder::new(0.015, 0.9),
        Chrome,
        Vec3::new(-0.6, c + 1.1, 0.55),
    ));
    v
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Inspector>()
        .init_resource::<Drawn>()
        .add_systems(Startup, palette)
        .add_systems(
            PostUpdate,
            (sync_bodies, place_links)
                .chain()
                .before(TransformSystems::Propagate),
        );
}

fn palette(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(Palette {
        hull: materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.37, 0.36),
            // Low metalness throughout: there is no environment map, so a
            // metal surface in shade would go black.
            metallic: 0.15,
            perceptual_roughness: 0.5,
            ..default()
        }),
        accent: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.52, 0.12),
            metallic: 0.05,
            perceptual_roughness: 0.5,
            ..default()
        }),
        joint: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.16, 0.17),
            metallic: 0.3,
            perceptual_roughness: 0.4,
            ..default()
        }),
        dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.085, 0.09),
            metallic: 0.1,
            perceptual_roughness: 0.7,
            ..default()
        }),
        chrome: materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.76, 0.78),
            metallic: 0.5,
            perceptual_roughness: 0.25,
            ..default()
        }),
        glow: materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.9, 1.0),
            emissive: LinearRgba::rgb(2.0, 7.0, 9.0),
            unlit: true,
            ..default()
        }),
    });
}

/// Spawns or rebuilds each lane's links when the lanes or their bodies change.
fn sync_bodies(
    mut commands: Commands,
    lanes: Res<Lanes>,
    mut drawn: ResMut<Drawn>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    while drawn.lanes.len() > lanes.lanes.len() {
        let (_, ents) = drawn.lanes.pop().unwrap();
        for e in ents {
            commands.entity(e).despawn();
        }
    }
    for (i, lane) in lanes.lanes.iter().enumerate() {
        let shapes = lane.env.sim.link_shapes();
        if drawn.lanes.get(i).is_some_and(|(s, _)| *s == shapes) {
            continue;
        }
        if let Some((_, ents)) = drawn.lanes.get_mut(i) {
            for e in ents.drain(..) {
                commands.entity(e).despawn();
            }
        }
        let mut ents = Vec::new();
        for (link, shape) in shapes.iter().enumerate() {
            let parent = commands
                .spawn((
                    LinkView { lane: i, link },
                    Transform::default(),
                    Visibility::default(),
                ))
                .id();
            // Each actuated link has its own copies of the materials so the
            // inspector can tint it (the glow keeps the shared one).
            let mut own: Vec<(Finish, Handle<StandardMaterial>)> = Vec::new();
            for p in parts(link, shape) {
                let shared = palette.get(p.finish);
                let mat = if link == 0 || p.finish == Finish::Glow {
                    shared.clone()
                } else if let Some((_, m)) = own.iter().find(|(f, _)| *f == p.finish) {
                    m.clone()
                } else {
                    let copy = materials.get(shared).cloned().unwrap_or_default();
                    let m = materials.add(copy);
                    own.push((p.finish, m.clone()));
                    m
                };
                commands.spawn((
                    Mesh3d(meshes.add(p.mesh)),
                    MeshMaterial3d(mat),
                    p.at,
                    ChildOf(parent),
                ));
            }
            ents.push(parent);
        }
        if i < drawn.lanes.len() {
            drawn.lanes[i] = (shapes, ents);
        } else {
            drawn.lanes.push((shapes, ents));
        }
    }
}

fn lerp_pose(a: &BodyPose, b: &BodyPose, t: f32) -> Transform {
    let pa = Vec3::from(a.position);
    let pb = Vec3::from(b.position);
    let ra = Quat::from_array(a.rotation);
    let rb = Quat::from_array(b.rotation);
    Transform {
        translation: pa.lerp(pb, t),
        rotation: ra.slerp(rb, t),
        scale: Vec3::ONE,
    }
}

fn place_links(
    lanes: Res<Lanes>,
    inspector: Res<Inspector>,
    mut links: Query<(&LinkView, &mut Transform, &Children)>,
    mats: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = lanes.blend;
    for (view, mut tf, children) in &mut links {
        let Some(lane) = lanes.lanes.get(view.lane) else {
            continue;
        };
        let (Some(a), Some(b)) = (lane.prev.get(view.link), lane.cur.get(view.link)) else {
            continue;
        };
        *tf = lerp_pose(a, b, t);
        if view.link == 0 {
            continue;
        }
        // Link i (after the torso) is moved by joint i - 1.
        let act = &lane.env.sim.actuators[view.link - 1];
        let heat = ((act.temperature - AMBIENT) / (TRIP - AMBIENT)).clamp(0.0, 1.0);
        for &c in children {
            let Ok(m) = mats.get(c) else { continue };
            let Some(mut mat) = materials.get_mut(&m.0) else {
                continue;
            };
            mat.emissive = if !inspector.0 {
                LinearRgba::BLACK
            } else if act.tripped || act.fault_left > 0.0 {
                LinearRgba::rgb(4.0, 0.2, 0.2)
            } else {
                LinearRgba::rgb(3.0 * heat * heat, 0.6 * heat * heat, 0.0)
            };
        }
    }
}
