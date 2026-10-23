//! The mech as drawn: one mesh per link, at the link's pose between the last
//! two policy steps. The shapes are the collision primitives for now (the
//! AMP-style visual mesh is M6). In the inspector each actuator's link is
//! tinted by its winding temperature.

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
            base_color: Color::srgb(0.32, 0.34, 0.33),
            metallic: 0.6,
            perceptual_roughness: 0.45,
            ..default()
        }),
        accent: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.52, 0.12),
            metallic: 0.2,
            perceptual_roughness: 0.5,
            ..default()
        }),
        joint: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.12, 0.13),
            metallic: 0.8,
            perceptual_roughness: 0.35,
            ..default()
        }),
    });
}

fn shape_mesh(shape: &LinkShape) -> (Mesh, Vec3) {
    match *shape {
        LinkShape::Cuboid { half, offset } => (
            Cuboid::new(2.0 * half[0], 2.0 * half[1], 2.0 * half[2]).into(),
            Vec3::from(offset),
        ),
        LinkShape::Capsule {
            half_height,
            radius,
            offset,
        } => (
            Capsule3d::new(radius, 2.0 * half_height).into(),
            Vec3::from(offset),
        ),
        LinkShape::Ball { radius } => (Sphere::new(radius).into(), Vec3::ZERO),
    }
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
            let (mesh, offset) = shape_mesh(shape);
            // Torso and feet in hull grey, thighs and shins in the accent
            // colour, the small joint links dark. Each actuated link has its
            // own material so the inspector can tint it.
            let base = match shape {
                LinkShape::Ball { .. } => palette.joint.clone(),
                LinkShape::Capsule { .. } => palette.accent.clone(),
                LinkShape::Cuboid { .. } => palette.hull.clone(),
            };
            let mat = if link == 0 {
                base
            } else {
                {
                    let copy = materials.get(&base).cloned().unwrap_or_default();
                    materials.add(copy)
                }
            };
            let parent = commands
                .spawn((
                    LinkView { lane: i, link },
                    Transform::default(),
                    Visibility::default(),
                ))
                .id();
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(mat),
                Transform::from_translation(offset),
                ChildOf(parent),
            ));
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
