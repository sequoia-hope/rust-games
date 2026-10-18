//! The riders drawn: a board and a body of capsules, posed every frame
//! from a [`RiderPose`] (written by the run, or by the menu for its
//! showcase): crouching, leaning into carves, tucking, reaching for grabs,
//! spinning and flipping, tumbling in a crash.
//!
//! The rider's own frame: x is where the rider faces (the toe edge), y up,
//! z along the board with the nose at -z. A regular rider rides left foot
//! forward; riding switch turns the whole body round.

use crate::meshes::{MeshBuilder, rgb};
use bevy::prelude::*;
use sb_sim::rider::Phase;
use sb_sim::tricks::Grab;
use std::f32::consts::PI;

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, pose_riders.before(TransformSystems::Propagate));
}

use bevy::transform::TransformSystems;

/// What the body is doing, in the world.
#[derive(Component, Clone, Debug)]
pub struct RiderPose {
    pub pos: Vec3,
    pub yaw: f32,
    pub normal: Vec3,
    pub pitch: f32,
    pub phase: Phase,
    pub crouch: f32,
    pub lean: f32,
    pub tuck: f32,
    pub skid: f32,
    pub grab: Option<Grab>,
    pub switch: bool,
    pub airborne: bool,
    /// An animation clock, seconds.
    pub t: f32,
    /// Standing at the gate: arms loose, a little bounce.
    pub idle: bool,
}

impl Default for RiderPose {
    fn default() -> Self {
        RiderPose {
            pos: Vec3::ZERO,
            yaw: 0.0,
            normal: Vec3::Y,
            pitch: 0.0,
            phase: Phase::Ground,
            crouch: 0.0,
            lean: 0.0,
            tuck: 0.0,
            skid: 0.0,
            grab: None,
            switch: false,
            airborne: false,
            t: 0.0,
            idle: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    ThighL,
    ShinL,
    ThighR,
    ShinR,
    Torso,
    Head,
    Goggles,
    UpperL,
    ForeL,
    UpperR,
    ForeR,
    HandL,
    HandR,
    BootL,
    BootR,
    Pelvis,
}

#[derive(Component)]
pub struct Bone(Part, f32);

/// A rider's colours: jacket, pants, helmet, board, board stripe.
pub fn palette(character: usize) -> [[f32; 3]; 5] {
    match character % 4 {
        0 => [[0.86, 0.12, 0.1], [0.1, 0.1, 0.13], [0.96, 0.96, 0.96], [0.08, 0.45, 0.95], [1.0, 1.0, 1.0]],
        1 => [[0.08, 0.68, 0.64], [0.92, 0.92, 0.95], [0.14, 0.14, 0.18], [0.98, 0.75, 0.08], [0.1, 0.1, 0.1]],
        2 => [[0.99, 0.78, 0.06], [0.1, 0.16, 0.38], [0.08, 0.08, 0.08], [0.92, 0.16, 0.3], [1.0, 0.9, 0.2]],
        _ => [[0.55, 0.24, 0.86], [0.34, 0.34, 0.38], [0.98, 0.5, 0.08], [0.12, 0.12, 0.12], [0.6, 1.0, 0.2]],
    }
}

fn mat(materials: &mut Assets<StandardMaterial>, c: [f32; 3], rough: f32) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(c[0], c[1], c[2]),
        perceptual_roughness: rough,
        ..default()
    })
}

/// Spawns a rider's body, posed at the origin until its pose is set.
pub fn spawn_rider(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    character: usize,
    extra: impl Bundle,
) -> Entity {
    let [jacket, pants, helmet, board, stripe] = palette(character);
    let jacket_m = mat(materials, jacket, 0.7);
    let pants_m = mat(materials, pants, 0.8);
    let helmet_m = mat(materials, helmet, 0.3);
    let glove_m = mat(materials, [0.08, 0.08, 0.09], 0.8);
    let boot_m = mat(materials, [0.15, 0.13, 0.12], 0.8);
    let goggle_m = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.55, 0.1),
        perceptual_roughness: 0.08,
        metallic: 0.6,
        ..default()
    });
    let white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.35,
        ..default()
    });

    // The board: a slab with turned-up ends, top graphic in vertex colours.
    let mut b = MeshBuilder::default();
    let top = rgb(board[0], board[1], board[2]);
    let st = rgb(stripe[0], stripe[1], stripe[2]);
    b.cuboid(Vec3::new(0.0, 0.025, 0.0), Vec3::new(0.135, 0.012, 0.62), top);
    b.cuboid(Vec3::new(0.0, 0.039, 0.0), Vec3::new(0.06, 0.003, 0.5), st);
    for end in [-1.0f32, 1.0] {
        let z0 = end * 0.62;
        let z1 = end * 0.76;
        let (a, bb) = (Vec3::new(-0.135, 0.037, z0), Vec3::new(0.135, 0.037, z0));
        let (c, d) = (Vec3::new(0.11, 0.12, z1), Vec3::new(-0.11, 0.12, z1));
        if end > 0.0 {
            b.quad(a, bb, c, d, top);
            b.quad(d, c, bb, a, top);
        } else {
            b.quad(bb, a, d, c, top);
            b.quad(c, d, a, bb, top);
        }
    }
    for z in [-0.25f32, 0.25] {
        b.cuboid(Vec3::new(0.0, 0.07, z), Vec3::new(0.12, 0.03, 0.07), rgb(0.1, 0.1, 0.1));
    }
    let board_mesh = meshes.add(b.build());

    let capsule = |meshes: &mut Assets<Mesh>, r: f32, len: f32| meshes.add(Capsule3d::new(r, len));
    let parts: [(Part, f32, f32, Handle<StandardMaterial>); 16] = [
        (Part::ThighL, 0.085, 0.45, pants_m.clone()),
        (Part::ShinL, 0.072, 0.45, pants_m.clone()),
        (Part::ThighR, 0.085, 0.45, pants_m.clone()),
        (Part::ShinR, 0.072, 0.45, pants_m.clone()),
        (Part::Torso, 0.17, 0.42, jacket_m.clone()),
        (Part::Head, 0.135, 0.0, helmet_m.clone()),
        (Part::Goggles, 0.0, 0.0, goggle_m.clone()),
        (Part::UpperL, 0.062, 0.3, jacket_m.clone()),
        (Part::ForeL, 0.056, 0.28, jacket_m.clone()),
        (Part::UpperR, 0.062, 0.3, jacket_m.clone()),
        (Part::ForeR, 0.056, 0.28, jacket_m.clone()),
        (Part::HandL, 0.058, 0.0, glove_m.clone()),
        (Part::HandR, 0.058, 0.0, glove_m.clone()),
        (Part::BootL, 0.0, 0.0, boot_m.clone()),
        (Part::BootR, 0.0, 0.0, boot_m.clone()),
        (Part::Pelvis, 0.15, 0.0, pants_m.clone()),
    ];
    let root = commands
        .spawn((
            RiderPose::default(),
            Transform::default(),
            Visibility::default(),
            extra,
        ))
        .id();
    commands.entity(root).with_children(|c| {
        c.spawn((Mesh3d(board_mesh), MeshMaterial3d(white.clone()), Transform::default()));
        for (part, r, len, m) in parts {
            let mesh = match part {
                Part::Head | Part::HandL | Part::HandR | Part::Pelvis => meshes.add(Sphere::new(r).mesh().ico(2).unwrap()),
                Part::Goggles => meshes.add(Cuboid::new(0.1, 0.07, 0.2)),
                Part::BootL | Part::BootR => meshes.add(Cuboid::new(0.2, 0.13, 0.13)),
                _ => capsule(meshes, r, len),
            };
            c.spawn((Mesh3d(mesh), MeshMaterial3d(m), Transform::default(), Bone(part, len)));
        }
    });
    root
}

/// Two-bone IK: the middle joint for a chain from `a` reaching toward `t`,
/// bending toward `hint`. Returns (middle, end).
fn ik(a: Vec3, t: Vec3, l1: f32, l2: f32, hint: Vec3) -> (Vec3, Vec3) {
    let d = t - a;
    let dist = d.length().clamp(0.02, l1 + l2 - 0.002);
    let dir = d.normalize_or(Vec3::NEG_Y);
    let end = a + dir * dist;
    let x = (l1 * l1 - l2 * l2 + dist * dist) / (2.0 * dist);
    let y = (l1 * l1 - x * x).max(0.0).sqrt();
    let perp = (hint - dir * hint.dot(dir)).normalize_or(Vec3::X);
    (a + dir * x + perp * y, end)
}

/// Where a grab puts a hand on the board (the rider's frame), and which
/// hand: true for the lead (left, nose-side) hand.
fn grab_target(g: Grab) -> (Vec3, bool) {
    match g {
        Grab::Indy => (Vec3::new(0.16, 0.1, 0.05), false),
        Grab::Melon => (Vec3::new(-0.16, 0.1, -0.05), true),
        Grab::Nose => (Vec3::new(0.0, 0.12, -0.72), true),
        Grab::Tail => (Vec3::new(0.0, 0.12, 0.72), false),
        Grab::Stalefish => (Vec3::new(-0.16, 0.1, 0.12), false),
        Grab::Method => (Vec3::new(-0.16, 0.1, -0.12), true),
        Grab::Mute => (Vec3::new(0.16, 0.1, -0.1), true),
        Grab::Japan => (Vec3::new(0.16, 0.1, -0.25), true),
    }
}

struct Joints {
    pelvis: Vec3,
    neck: Vec3,
    head: Vec3,
    knee: [Vec3; 2],
    ankle: [Vec3; 2],
    hip: [Vec3; 2],
    shoulder: [Vec3; 2],
    elbow: [Vec3; 2],
    hand: [Vec3; 2],
    bend: f32,
}

fn joints(p: &RiderPose) -> Joints {
    let crashed = matches!(p.phase, Phase::Crashed(_));
    let grabbing = p.grab.is_some();
    let c = if crashed { 0.1 } else if grabbing { 1.0 } else { p.crouch.clamp(0.0, 1.0) };
    let bob = if p.idle { (p.t * 2.2).sin() * 0.02 } else { 0.0 };
    let ankle = [Vec3::new(0.0, 0.12, -0.25), Vec3::new(0.0, 0.12, 0.25)];
    let pelvis = Vec3::new(-0.04 - 0.1 * c, 0.97 - 0.44 * c + bob, 0.0);
    let hip = [pelvis + Vec3::new(0.0, 0.0, -0.11), pelvis + Vec3::new(0.0, 0.0, 0.11)];
    let knee_hint = [Vec3::new(1.0, 0.0, -0.3), Vec3::new(1.0, 0.0, 0.3)];
    let mut knee = [Vec3::ZERO; 2];
    for i in 0..2 {
        knee[i] = ik(hip[i], ankle[i], 0.45, 0.45, knee_hint[i]).0;
    }
    let bend = 0.12 + 0.35 * c + 0.3 * p.tuck + if grabbing { 0.35 } else { 0.0 };
    let up = Vec3::new(bend.sin(), bend.cos(), 0.0);
    let neck = pelvis + up * 0.56;
    let head = neck + up * 0.2 + Vec3::new(0.03, 0.04, -0.03);
    let shoulder = [neck + Vec3::new(0.0, -0.06, -0.2), neck + Vec3::new(0.0, -0.06, 0.2)];
    let wave = (p.t * 3.1).sin();
    let mut hand = if crashed {
        [
            shoulder[0] + Vec3::new(0.2, 0.25 + 0.1 * wave, -0.45),
            shoulder[1] + Vec3::new(-0.2, 0.3 - 0.1 * wave, 0.45),
        ]
    } else if p.tuck > 0.5 {
        [
            shoulder[0] + Vec3::new(0.42, -0.22, 0.08),
            shoulder[1] + Vec3::new(0.42, -0.22, -0.08),
        ]
    } else if p.airborne {
        [
            shoulder[0] + Vec3::new(0.1, -0.05 + 0.05 * wave, -0.5),
            shoulder[1] + Vec3::new(0.05, -0.05 - 0.05 * wave, 0.5),
        ]
    } else if p.idle {
        [
            shoulder[0] + Vec3::new(0.12, -0.5, -0.08),
            shoulder[1] + Vec3::new(0.12, -0.5, 0.08),
        ]
    } else {
        let l = p.lean * 0.2;
        [
            shoulder[0] + Vec3::new(0.18 + l, -0.32, -0.36),
            shoulder[1] + Vec3::new(0.02 - l, -0.42, 0.3),
        ]
    };
    if let Some(g) = p.grab {
        let (t, lead) = grab_target(g);
        hand[if lead { 0 } else { 1 }] = t;
    }
    let mut elbow = [Vec3::ZERO; 2];
    for i in 0..2 {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let (e, h) = ik(shoulder[i], hand[i], 0.31, 0.3, Vec3::new(-0.2, -0.3, side));
        elbow[i] = e;
        hand[i] = h;
    }
    Joints {
        pelvis,
        neck,
        head,
        knee,
        ankle,
        hip,
        shoulder,
        elbow,
        hand,
        bend,
    }
}

fn bone_between(a: Vec3, b: Vec3, nominal: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(1e-3);
    Transform {
        translation: (a + b) * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::Y, d / len),
        scale: Vec3::new(1.0, if nominal > 0.0 { (len / nominal).clamp(0.4, 2.0) } else { 1.0 }, 1.0),
    }
}

/// The root's rotation: the board's heading on the snow's normal, leaned
/// into the carve; spun and flipped in the air; tumbling in a crash.
fn root_transform(p: &RiderPose) -> Transform {
    let yaw = p.yaw + if p.switch { PI } else { 0.0 };
    let side = if p.switch { -1.0 } else { 1.0 };
    let heading = Quat::from_rotation_y(-yaw);
    match p.phase {
        Phase::Crashed(t) => {
            let k = (1.0 - t / 1.1).max(0.0);
            let spin = t.min(1.1) * 7.0;
            let rot = heading
                * Quat::from_rotation_z(-(PI / 2.0) * (1.0 - k) - spin * k)
                * Quat::from_rotation_x(spin * 0.6 * k);
            Transform::from_translation(p.pos + Vec3::Y * (0.25 + 0.6 * k)).with_rotation(rot)
        }
        _ if p.airborne => {
            let rot = heading * Quat::from_rotation_z(-p.pitch);
            let pivot = Vec3::Y * 0.75;
            Transform::from_translation(p.pos + pivot - rot * pivot).with_rotation(rot)
        }
        _ => {
            let up = p.normal.normalize_or(Vec3::Y);
            let fwd = heading * Vec3::NEG_Z;
            let back = -(fwd - up * fwd.dot(up)).normalize_or(-fwd);
            let x = up.cross(back).normalize_or(Vec3::X);
            let back = x.cross(up);
            let base = Quat::from_mat3(&Mat3::from_cols(x, up, back));
            let rot = base
                * Quat::from_rotation_y(p.skid * 0.9 * side)
                * Quat::from_rotation_z(-p.lean * side * 0.55);
            Transform::from_translation(p.pos).with_rotation(rot)
        }
    }
}

fn pose_riders(
    mut roots: Query<(&RiderPose, &mut Transform, &Children), Without<Bone>>,
    mut bones: Query<(&Bone, &mut Transform)>,
) {
    for (pose, mut t, children) in &mut roots {
        *t = root_transform(pose);
        let j = joints(pose);
        for child in children.iter() {
            let Ok((bone, mut bt)) = bones.get_mut(child) else {
                continue;
            };
            let (part, len) = (bone.0, bone.1);
            *bt = match part {
                Part::ThighL => bone_between(j.hip[0], j.knee[0], len),
                Part::ShinL => bone_between(j.knee[0], j.ankle[0], len),
                Part::ThighR => bone_between(j.hip[1], j.knee[1], len),
                Part::ShinR => bone_between(j.knee[1], j.ankle[1], len),
                Part::Torso => bone_between(j.pelvis + Vec3::Y * 0.08, j.neck, len),
                Part::UpperL => bone_between(j.shoulder[0], j.elbow[0], len),
                Part::ForeL => bone_between(j.elbow[0], j.hand[0], len),
                Part::UpperR => bone_between(j.shoulder[1], j.elbow[1], len),
                Part::ForeR => bone_between(j.elbow[1], j.hand[1], len),
                Part::Head => Transform::from_translation(j.head),
                Part::Goggles => Transform::from_translation(j.head + Vec3::new(0.1, 0.0, -0.04))
                    .with_rotation(Quat::from_rotation_z(-j.bend) * Quat::from_rotation_y(0.35)),
                Part::HandL => Transform::from_translation(j.hand[0]),
                Part::HandR => Transform::from_translation(j.hand[1]),
                Part::BootL => Transform::from_translation(j.ankle[0] - Vec3::Y * 0.02),
                Part::BootR => Transform::from_translation(j.ankle[1] - Vec3::Y * 0.02),
                Part::Pelvis => Transform::from_translation(j.pelvis),
            };
        }
    }
}
