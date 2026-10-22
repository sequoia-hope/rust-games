//! The riders drawn: a sculpted snowboard and bindings, articulated body,
//! layered technical outerwear (color-blocked parka, articulated storm hood,
//! 4-panel snow skirt, flowing knit scarf, baggy cargo snow pants with lower
//! cuffs, gauntleted mittens, lugged snowboard boots, helmet and wrap-around
//! chrome goggles), posed every frame from a [`RiderPose`] with expressive
//! whole-body kinematics and secondary spring-damper cloth & gear physics.
//!
//! The rider's own frame: `x` is where the rider faces (the toe edge), `y` up,
//! `z` along the board with the nose at `-z`. A regular rider rides left foot
//! forward; riding switch turns the whole body round.

use crate::meshes::{MeshBuilder, Ring, mix, rgb};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use sb_sim::noise::wrap_angle;
use sb_sim::rider::{CRASH_TIME, Phase};
use sb_sim::tricks::Grab;
use std::f32::consts::{PI, TAU};

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, pose_riders.before(TransformSystems::Propagate));
}

/// What the body is doing, in the world.
#[derive(Component, Clone, Debug)]
pub struct RiderPose {
    pub pos: Vec3,
    pub vel: Vec3,
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
    /// Seconds spent in the current jump.
    pub air_time: f32,
    /// Total spin accumulated in the current jump (radians).
    pub air_spin: f32,
    /// An animation clock, seconds.
    pub t: f32,
    /// Standing at the gate: arms loose, a little bounce.
    pub idle: bool,
    /// Crossed the finish line.
    pub finished: bool,
}

impl Default for RiderPose {
    fn default() -> Self {
        RiderPose {
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
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
            air_time: 0.0,
            air_spin: 0.0,
            t: 0.0,
            idle: false,
            finished: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Board,
    ThighL,
    ShinL,
    CuffL,
    ThighR,
    ShinR,
    CuffR,
    Pelvis,
    Waist,
    Torso,
    SkirtUpper,
    SkirtLower,
    Hood,
    Head,
    Goggles,
    Pom,
    UpperL,
    ForeL,
    UpperR,
    ForeR,
    HandL,
    HandR,
    BootL,
    BootR,
    Scarf0,
    Scarf1,
    Scarf2,
}

#[derive(Component)]
pub struct Bone(Part, f32);

/// Secondary body motion and cloth physics state per rider.
#[derive(Component, Clone, Debug)]
struct ClothSim {
    init: bool,
    prev_pos: Vec3,
    prev_vel: Vec3,
    prev_yaw: f32,
    prev_airborne: bool,
    prev_t: f32,
    smooth_lean: f32,
    smooth_crouch: f32,
    smooth_tuck: f32,
    smooth_skid: f32,
    yaw_rate: f32,
    /// Spring-damped landing compression impulse.
    land_comp: f32,
    land_vel: f32,
    /// Arm balance-recovery wave after a heavy landing.
    land_wobble: f32,
    /// Brief ankle/knee extension at jump takeoff ("pop").
    pop_ext: f32,
    /// Smoothly blended grab weight (0..1) and most recent grab.
    grab_blend: f32,
    last_grab: Grab,
    /// 4 jacket skirt hem panels (Front, Back, Left, Right): flare angle & angular velocity.
    skirt_angle: [f32; 4],
    skirt_vel: [f32; 4],
    /// Parka hood spring angles (pitch, roll) and velocity.
    hood_angles: Vec2,
    hood_vel: Vec2,
    /// Baggy lower pant cuff vertical/radial jounce (left, right).
    cuff_disp: [f32; 2],
    cuff_vel: [f32; 2],
    /// Helmet tassel / pom spring offset in local head frame.
    pom_pos: Vec3,
    pom_vel: Vec3,
    /// 4 world-space cloth nodes (collar anchor + 3 simulated links) for the flowing scarf.
    scarf_pos: [Vec3; 4],
    scarf_vel: [Vec3; 4],
}

impl Default for ClothSim {
    fn default() -> Self {
        Self {
            init: false,
            prev_pos: Vec3::ZERO,
            prev_vel: Vec3::ZERO,
            prev_yaw: 0.0,
            prev_airborne: false,
            prev_t: 0.0,
            smooth_lean: 0.0,
            smooth_crouch: 0.0,
            smooth_tuck: 0.0,
            smooth_skid: 0.0,
            yaw_rate: 0.0,
            land_comp: 0.0,
            land_vel: 0.0,
            land_wobble: 0.0,
            pop_ext: 0.0,
            grab_blend: 0.0,
            last_grab: Grab::Indy,
            skirt_angle: [0.08; 4],
            skirt_vel: [0.0; 4],
            hood_angles: Vec2::ZERO,
            hood_vel: Vec2::ZERO,
            cuff_disp: [0.0; 2],
            cuff_vel: [0.0; 2],
            pom_pos: Vec3::ZERO,
            pom_vel: Vec3::ZERO,
            scarf_pos: [Vec3::ZERO; 4],
            scarf_vel: [Vec3::ZERO; 4],
        }
    }
}

/// A rider's primary colours: jacket, pants, helmet, board, board stripe.
pub fn palette(character: usize) -> [[f32; 3]; 5] {
    match character % 4 {
        0 => [
            [0.86, 0.12, 0.1],
            [0.12, 0.13, 0.17],
            [0.95, 0.96, 0.98],
            [0.08, 0.45, 0.95],
            [1.0, 0.96, 0.92],
        ],
        1 => [
            [0.06, 0.68, 0.64],
            [0.9, 0.92, 0.96],
            [0.14, 0.15, 0.2],
            [0.98, 0.75, 0.08],
            [0.12, 0.12, 0.15],
        ],
        2 => [
            [0.98, 0.76, 0.06],
            [0.1, 0.15, 0.34],
            [0.09, 0.09, 0.11],
            [0.92, 0.14, 0.28],
            [1.0, 0.9, 0.2],
        ],
        _ => [
            [0.54, 0.22, 0.86],
            [0.28, 0.29, 0.34],
            [0.98, 0.48, 0.08],
            [0.12, 0.12, 0.14],
            [0.58, 1.0, 0.18],
        ],
    }
}

struct StyleColors {
    jacket: [f32; 4],
    jacket_yoke: [f32; 4],
    jacket_trim: [f32; 4],
    pants: [f32; 4],
    pants_dark: [f32; 4],
    helmet: [f32; 4],
    board: [f32; 4],
    stripe: [f32; 4],
    scarf_a: [f32; 4],
    scarf_b: [f32; 4],
    lens_a: [f32; 4],
    lens_b: [f32; 4],
    boot: [f32; 4],
    glove: [f32; 4],
}

fn style_colors(character: usize) -> StyleColors {
    let [j, p, h, b, s] = palette(character);
    let jacket = rgb(j[0], j[1], j[2]);
    let pants = rgb(p[0], p[1], p[2]);
    let helmet = rgb(h[0], h[1], h[2]);
    let board = rgb(b[0], b[1], b[2]);
    let stripe = rgb(s[0], s[1], s[2]);
    let dark = [0.0, 0.0, 0.0, 1.0];
    match character % 4 {
        0 => StyleColors {
            jacket,
            jacket_yoke: rgb(0.14, 0.15, 0.2),
            jacket_trim: rgb(0.96, 0.96, 0.98),
            pants,
            pants_dark: mix(pants, dark, 0.35),
            helmet,
            board,
            stripe,
            scarf_a: rgb(0.92, 0.16, 0.12),
            scarf_b: rgb(0.98, 0.96, 0.92),
            lens_a: rgb(1.0, 0.45, 0.06),
            lens_b: rgb(1.0, 0.85, 0.2),
            boot: rgb(0.14, 0.13, 0.15),
            glove: rgb(0.1, 0.1, 0.12),
        },
        1 => StyleColors {
            jacket,
            jacket_yoke: rgb(0.94, 0.96, 0.99),
            jacket_trim: rgb(0.95, 0.22, 0.55),
            pants,
            pants_dark: rgb(0.22, 0.25, 0.32),
            helmet,
            board,
            stripe,
            scarf_a: rgb(0.92, 0.2, 0.54),
            scarf_b: rgb(0.15, 0.85, 0.82),
            lens_a: rgb(0.1, 0.78, 0.98),
            lens_b: rgb(0.75, 0.96, 1.0),
            boot: rgb(0.16, 0.18, 0.22),
            glove: rgb(0.14, 0.16, 0.2),
        },
        2 => StyleColors {
            jacket,
            jacket_yoke: rgb(0.09, 0.13, 0.28),
            jacket_trim: rgb(0.98, 0.98, 0.98),
            pants,
            pants_dark: mix(pants, dark, 0.4),
            helmet,
            board,
            stripe,
            scarf_a: rgb(0.98, 0.76, 0.06),
            scarf_b: rgb(0.1, 0.15, 0.34),
            lens_a: rgb(0.96, 0.16, 0.22),
            lens_b: rgb(1.0, 0.65, 0.2),
            boot: rgb(0.11, 0.11, 0.14),
            glove: rgb(0.08, 0.1, 0.18),
        },
        _ => StyleColors {
            jacket,
            jacket_yoke: rgb(0.16, 0.15, 0.22),
            jacket_trim: rgb(0.62, 1.0, 0.18),
            pants,
            pants_dark: mix(pants, dark, 0.35),
            helmet,
            board,
            stripe,
            scarf_a: rgb(0.62, 1.0, 0.18),
            scarf_b: rgb(0.54, 0.22, 0.86),
            lens_a: rgb(0.15, 0.95, 0.52),
            lens_b: rgb(0.8, 1.0, 0.35),
            boot: rgb(0.18, 0.16, 0.15),
            glove: rgb(0.14, 0.13, 0.16),
        },
    }
}

/// Spawns a rider's body, posed at the origin until its pose is set.
pub fn spawn_rider(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    character: usize,
    extra: impl Bundle,
) -> Entity {
    let sc = style_colors(character);
    let fabric_m = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.78,
        reflectance: 0.22,
        ..default()
    });
    let shell_m = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.32,
        reflectance: 0.48,
        ..default()
    });
    let goggle_m = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.08,
        metallic: 0.62,
        reflectance: 0.85,
        ..default()
    });

    let thigh_l = meshes.add(thigh_mesh(&sc, -1.0));
    let thigh_r = meshes.add(thigh_mesh(&sc, 1.0));
    let shin_l = meshes.add(shin_mesh(&sc, -1.0));
    let shin_r = meshes.add(shin_mesh(&sc, 1.0));
    let cuff_m = meshes.add(cuff_mesh(&sc));
    let upper_l = meshes.add(upper_arm_mesh(&sc, -1.0));
    let upper_r = meshes.add(upper_arm_mesh(&sc, 1.0));
    let fore_m = meshes.add(forearm_mesh(&sc));
    let hand_l = meshes.add(mitten_mesh(&sc, -1.0));
    let hand_r = meshes.add(mitten_mesh(&sc, 1.0));
    let boot_m = meshes.add(boot_mesh(&sc));
    let scarf_body = meshes.add(scarf_seg_mesh(&sc, false));
    let scarf_tip = meshes.add(scarf_seg_mesh(&sc, true));
    let skirt_upper = meshes.add(skirt_bell_mesh(&sc, false));
    let skirt_lower = meshes.add(skirt_bell_mesh(&sc, true));

    let parts: [(Part, f32, Handle<Mesh>, Handle<StandardMaterial>); 27] = [
        (
            Part::Board,
            0.0,
            meshes.add(board_mesh(&sc)),
            shell_m.clone(),
        ),
        (Part::ThighL, 0.45, thigh_l, fabric_m.clone()),
        (Part::ShinL, 0.45, shin_l, fabric_m.clone()),
        (Part::CuffL, 0.0, cuff_m.clone(), fabric_m.clone()),
        (Part::ThighR, 0.45, thigh_r, fabric_m.clone()),
        (Part::ShinR, 0.45, shin_r, fabric_m.clone()),
        (Part::CuffR, 0.0, cuff_m, fabric_m.clone()),
        (
            Part::Pelvis,
            0.0,
            meshes.add(pelvis_mesh(&sc)),
            fabric_m.clone(),
        ),
        (
            Part::Waist,
            0.22,
            meshes.add(waist_mesh(&sc)),
            fabric_m.clone(),
        ),
        (
            Part::Torso,
            0.34,
            meshes.add(torso_mesh(&sc, character)),
            fabric_m.clone(),
        ),
        (Part::SkirtUpper, 0.0, skirt_upper, fabric_m.clone()),
        (Part::SkirtLower, 0.0, skirt_lower, fabric_m.clone()),
        (
            Part::Hood,
            0.0,
            meshes.add(hood_mesh(&sc)),
            fabric_m.clone(),
        ),
        (
            Part::Head,
            0.0,
            meshes.add(head_mesh(&sc, character)),
            shell_m.clone(),
        ),
        (
            Part::Goggles,
            0.0,
            meshes.add(goggles_mesh(&sc)),
            goggle_m.clone(),
        ),
        (
            Part::Pom,
            0.0,
            meshes.add(pom_mesh(&sc, character)),
            fabric_m.clone(),
        ),
        (Part::UpperL, 0.31, upper_l, fabric_m.clone()),
        (Part::ForeL, 0.29, fore_m.clone(), fabric_m.clone()),
        (Part::UpperR, 0.31, upper_r, fabric_m.clone()),
        (Part::ForeR, 0.29, fore_m, fabric_m.clone()),
        (Part::HandL, 0.0, hand_l, fabric_m.clone()),
        (Part::HandR, 0.0, hand_r, fabric_m.clone()),
        (Part::BootL, 0.0, boot_m.clone(), fabric_m.clone()),
        (Part::BootR, 0.0, boot_m, fabric_m.clone()),
        (Part::Scarf0, 0.17, scarf_body.clone(), fabric_m.clone()),
        (Part::Scarf1, 0.17, scarf_body, fabric_m.clone()),
        (Part::Scarf2, 0.17, scarf_tip, fabric_m),
    ];

    let root = commands
        .spawn((
            RiderPose::default(),
            ClothSim::default(),
            Transform::default(),
            Visibility::default(),
            extra,
        ))
        .id();
    commands.entity(root).with_children(|c| {
        for (part, len, mesh, m) in parts {
            c.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(m),
                Transform::default(),
                Bone(part, len),
            ));
        }
    });
    root
}

fn board_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let top = sc.board;
    let st = sc.stripe;
    let base_col = rgb(0.07, 0.07, 0.09);
    let edge_col = rgb(0.76, 0.8, 0.86);
    let bind_dark = rgb(0.1, 0.1, 0.12);

    // Multi-segment cambered deck with rounded upturned nose (-z) and tail (+z).
    let stations: [(f32, f32, f32); 11] = [
        (-0.79, 0.055, 0.125),
        (-0.74, 0.105, 0.088),
        (-0.67, 0.132, 0.052),
        (-0.58, 0.142, 0.028),
        (-0.28, 0.132, 0.024),
        (0.0, 0.128, 0.028),
        (0.28, 0.132, 0.024),
        (0.58, 0.142, 0.028),
        (0.67, 0.132, 0.052),
        (0.74, 0.105, 0.088),
        (0.79, 0.055, 0.125),
    ];
    let thick = 0.015;
    for i in 0..stations.len() - 1 {
        let (z0, w0, y0) = stations[i];
        let (z1, w1, y1) = stations[i + 1];
        let tip = i <= 1 || i >= 8;
        let t_col = if tip { st } else { top };
        let b_col = if i == 4 || i == 5 { st } else { base_col };
        let tl0 = Vec3::new(-w0, y0 + thick, z0);
        let tr0 = Vec3::new(w0, y0 + thick, z0);
        let tl1 = Vec3::new(-w1, y1 + thick, z1);
        let tr1 = Vec3::new(w1, y1 + thick, z1);
        let bl0 = Vec3::new(-w0, y0, z0);
        let br0 = Vec3::new(w0, y0, z0);
        let bl1 = Vec3::new(-w1, y1, z1);
        let br1 = Vec3::new(w1, y1, z1);
        b.quad(tl0, tl1, tr1, tr0, t_col);
        b.quad(bl0, br0, br1, bl1, b_col);
        b.quad(tl0, bl0, bl1, tl1, edge_col);
        b.quad(tr0, tr1, br1, br0, edge_col);
        if i == 0 {
            b.quad(tl0, tr0, br0, bl0, edge_col);
        }
        if i + 2 == stations.len() {
            b.quad(tl1, bl1, br1, tr1, edge_col);
        }
    }

    // Center racing stripe & chevron graphics on the topsheet.
    b.cuboid(
        Vec3::new(0.0, 0.043, 0.0),
        Vec3::new(0.042, 0.002, 0.52),
        st,
    );
    b.cuboid(
        Vec3::new(0.0, 0.0435, 0.0),
        Vec3::new(0.014, 0.002, 0.56),
        sc.jacket,
    );

    // Detailed bindings: baseplate, heel hoop, angled highback, ankle & toe straps.
    for (z, ang) in [(-0.25f32, -0.22f32), (0.25, 0.10)] {
        let rot = Quat::from_rotation_y(ang);
        let o = Vec3::new(0.0, 0.042, z);
        // Baseplate disc and chassis.
        b.cuboid_rot(
            o + Vec3::Y * 0.008,
            rot,
            Vec3::new(0.115, 0.008, 0.068),
            bind_dark,
        );
        for side in [-1.0f32, 1.0] {
            b.cuboid_rot(
                o + rot * Vec3::new(-0.01, 0.032, side * 0.064),
                rot,
                Vec3::new(0.085, 0.022, 0.008),
                bind_dark,
            );
        }
        // Sculpted highback rising behind the calf (-x side).
        let hb_rot = rot * Quat::from_rotation_z(-0.18);
        b.cuboid_rot(
            o + rot * Vec3::new(-0.095, 0.095, 0.0),
            hb_rot,
            Vec3::new(0.01, 0.075, 0.052),
            bind_dark,
        );
        b.cuboid_rot(
            o + rot * Vec3::new(-0.082, 0.165, 0.0),
            hb_rot,
            Vec3::new(0.012, 0.015, 0.048),
            sc.jacket_trim,
        );
        // Ankle & toe straps arching across the boot.
        b.cuboid_rot(
            o + rot * Vec3::new(0.015, 0.095, 0.0),
            rot * Quat::from_rotation_z(0.35),
            Vec3::new(0.024, 0.012, 0.072),
            sc.jacket,
        );
        b.cuboid_rot(
            o + rot * Vec3::new(0.095, 0.048, 0.0),
            rot * Quat::from_rotation_z(0.2),
            Vec3::new(0.02, 0.012, 0.064),
            bind_dark,
        );
    }
    b.build()
}

fn boot_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let sole = rgb(0.06, 0.06, 0.07);
    let mid = sc.jacket_trim;
    let body = sc.boot;
    let tongue = mix(sc.boot, sc.jacket, 0.45);
    // Lugged outsole and contrast midsole.
    b.cuboid(
        Vec3::new(0.018, -0.042, 0.0),
        Vec3::new(0.115, 0.014, 0.058),
        sole,
    );
    b.cuboid(
        Vec3::new(0.018, -0.024, 0.0),
        Vec3::new(0.112, 0.006, 0.056),
        mid,
    );
    // Foot lower shell & rounded toe box.
    let foot_rings = [
        Ring {
            center: Vec3::new(0.015, -0.018, 0.0),
            rx: 0.108,
            rz: 0.055,
            front_col: tongue,
            ..Ring::new(-0.018, 0.108, 0.055, body)
        },
        Ring {
            center: Vec3::new(0.018, 0.018, 0.0),
            rx: 0.104,
            rz: 0.054,
            front_col: tongue,
            ..Ring::new(0.018, 0.104, 0.054, body)
        },
        Ring {
            center: Vec3::new(-0.012, 0.055, 0.0),
            rx: 0.068,
            rz: 0.052,
            tilt_z: -0.14,
            front_col: tongue,
            ..Ring::new(0.055, 0.068, 0.052, body)
        },
        Ring {
            center: Vec3::new(-0.004, 0.115, 0.0),
            rx: 0.064,
            rz: 0.054,
            tilt_z: -0.16,
            front_col: tongue,
            ..Ring::new(0.115, 0.064, 0.054, body)
        },
    ];
    b.loft(&foot_rings, 12, false, true);
    // BOA dial & lace detail on front tongue.
    b.cuboid(
        Vec3::new(0.058, 0.088, 0.0),
        Vec3::new(0.012, 0.016, 0.016),
        sc.stripe,
    );
    b.build()
}

fn thigh_mesh(sc: &StyleColors, side: f32) -> Mesh {
    let mut b = MeshBuilder::default();
    let p = sc.pants;
    let pd = sc.pants_dark;
    let rings = [
        Ring {
            rx: 0.088,
            rz: 0.09,
            front_col: pd,
            ripple: 0.04,
            freq: 3.0,
            ..Ring::new(-0.225, 0.088, 0.09, p)
        },
        Ring {
            rx: 0.098,
            rz: 0.102,
            front_col: pd,
            ripple: 0.05,
            freq: 4.0,
            ..Ring::new(-0.12, 0.098, 0.102, p)
        },
        Ring {
            rx: 0.106,
            rz: 0.11,
            front_col: p,
            ripple: 0.045,
            freq: 3.0,
            ..Ring::new(0.02, 0.106, 0.11, p)
        },
        Ring {
            rx: 0.112,
            rz: 0.114,
            front_col: p,
            ripple: 0.03,
            freq: 3.0,
            ..Ring::new(0.14, 0.112, 0.114, p)
        },
        Ring {
            rx: 0.104,
            rz: 0.106,
            front_col: p,
            ..Ring::new(0.225, 0.104, 0.106, p)
        },
    ];
    b.loft(&rings, 12, true, true);
    // Outer cargo pocket + contrast pocket flap.
    let pz = side * 0.098;
    b.cuboid(
        Vec3::new(0.01, -0.01, pz),
        Vec3::new(0.055, 0.068, 0.018),
        pd,
    );
    b.cuboid(
        Vec3::new(0.01, 0.052, pz + side * 0.004),
        Vec3::new(0.058, 0.016, 0.02),
        sc.jacket_trim,
    );
    b.build()
}

fn shin_mesh(sc: &StyleColors, side: f32) -> Mesh {
    let mut b = MeshBuilder::default();
    let p = sc.pants;
    let pd = sc.pants_dark;
    let rings = [
        Ring {
            rx: 0.098,
            rz: 0.096,
            front_col: p,
            ripple: 0.06,
            freq: 4.0,
            ..Ring::new(-0.225, 0.098, 0.096, pd)
        },
        Ring {
            rx: 0.106,
            rz: 0.102,
            front_col: p,
            ripple: 0.07,
            freq: 5.0,
            ..Ring::new(-0.13, 0.106, 0.102, p)
        },
        Ring {
            rx: 0.094,
            rz: 0.092,
            front_col: p,
            ripple: 0.05,
            freq: 4.0,
            ..Ring::new(-0.02, 0.094, 0.092, p)
        },
        Ring {
            rx: 0.092,
            rz: 0.09,
            front_col: pd,
            ripple: 0.04,
            freq: 3.0,
            ..Ring::new(0.11, 0.092, 0.09, p)
        },
        Ring {
            rx: 0.088,
            rz: 0.088,
            front_col: pd,
            ..Ring::new(0.225, 0.088, 0.088, p)
        },
    ];
    b.loft(&rings, 12, true, true);
    // Inner ankle Cordura scuff guard.
    b.cuboid(
        Vec3::new(0.0, -0.15, -side * 0.088),
        Vec3::new(0.048, 0.055, 0.012),
        rgb(0.09, 0.09, 0.11),
    );
    b.build()
}

fn cuff_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let p = sc.pants;
    let trim = sc.jacket_trim;
    let rings = [
        Ring {
            rx: 0.114,
            rz: 0.108,
            ripple: 0.06,
            freq: 5.0,
            ..Ring::new(-0.045, 0.114, 0.108, trim)
        },
        Ring {
            rx: 0.11,
            rz: 0.104,
            ripple: 0.05,
            freq: 5.0,
            ..Ring::new(0.0, 0.11, 0.104, p)
        },
        Ring {
            rx: 0.098,
            rz: 0.095,
            ..Ring::new(0.045, 0.098, 0.095, sc.pants_dark)
        },
    ];
    b.loft(&rings, 12, false, false);
    b.build()
}

fn pelvis_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let p = sc.pants;
    let pd = sc.pants_dark;
    let belt = rgb(0.1, 0.1, 0.12);
    let rings = [
        Ring::new(-0.08, 0.125, 0.165, pd),
        Ring {
            ripple: 0.03,
            freq: 4.0,
            ..Ring::new(-0.01, 0.152, 0.195, p)
        },
        Ring::new(0.055, 0.148, 0.19, p),
        Ring::new(0.09, 0.144, 0.184, belt),
    ];
    b.loft(&rings, 14, true, true);
    // Belt buckle in front (+x) and rear pockets on seat (-x).
    b.cuboid(
        Vec3::new(0.148, 0.072, 0.0),
        Vec3::new(0.012, 0.02, 0.032),
        sc.stripe,
    );
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            Vec3::new(-0.142, 0.0, side * 0.075),
            Vec3::new(0.014, 0.038, 0.042),
            pd,
        );
    }
    b.build()
}

fn waist_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let j = sc.jacket;
    let jd = mix(j, [0.0, 0.0, 0.0, 1.0], 0.18);
    let rings = [
        Ring {
            ripple: 0.04,
            freq: 4.0,
            ..Ring::new(-0.11, 0.148, 0.188, jd)
        },
        Ring {
            ripple: 0.045,
            freq: 5.0,
            ..Ring::new(0.0, 0.154, 0.195, j)
        },
        Ring {
            ripple: 0.03,
            freq: 4.0,
            ..Ring::new(0.11, 0.16, 0.204, j)
        },
    ];
    b.loft(&rings, 14, true, true);
    // Storm-flap center zipper placket on front (+x).
    b.cuboid(
        Vec3::new(0.151, 0.0, 0.0),
        Vec3::new(0.005, 0.11, 0.011),
        sc.jacket_trim,
    );
    b.build()
}

fn torso_mesh(sc: &StyleColors, character: usize) -> Mesh {
    let mut b = MeshBuilder::default();
    let j = sc.jacket;
    let yoke = sc.jacket_yoke;
    let trim = sc.jacket_trim;
    let baffle = if character % 4 == 2 { 0.055 } else { 0.025 };
    let rings = [
        Ring {
            ripple: baffle,
            freq: 4.0,
            ..Ring::new(-0.17, 0.156, 0.20, j)
        },
        Ring {
            ripple: baffle,
            freq: 4.0,
            ..Ring::new(-0.05, 0.166, 0.216, j)
        },
        Ring {
            front_col: trim,
            ripple: 0.02,
            freq: 4.0,
            ..Ring::new(0.02, 0.174, 0.23, trim)
        },
        Ring {
            front_col: yoke,
            ..Ring::new(0.09, 0.176, 0.244, yoke)
        },
        Ring {
            front_col: yoke,
            ..Ring::new(0.17, 0.144, 0.212, yoke)
        },
    ];
    b.loft(&rings, 14, true, true);

    // Center zipper storm placket & chest pocket.
    b.cuboid(
        Vec3::new(0.167, 0.0, 0.0),
        Vec3::new(0.006, 0.168, 0.011),
        trim,
    );
    b.cuboid(
        Vec3::new(0.164, 0.05, -0.075),
        Vec3::new(0.008, 0.04, 0.009),
        sc.stripe,
    );

    // Character-specific upper-body gear details:
    match character % 4 {
        1 => {
            // Mika: sculpted backcountry hydration pack on the back (-x) with shoulder straps.
            let pack = rgb(0.12, 0.14, 0.18);
            b.cuboid(
                Vec3::new(-0.185, -0.01, 0.0),
                Vec3::new(0.045, 0.135, 0.115),
                pack,
            );
            b.cuboid(
                Vec3::new(-0.228, 0.01, 0.0),
                Vec3::new(0.012, 0.088, 0.075),
                trim,
            );
            for side in [-1.0f32, 1.0] {
                b.cuboid(
                    Vec3::new(0.0, 0.172, side * 0.11),
                    Vec3::new(0.145, 0.014, 0.022),
                    pack,
                );
            }
        }
        2 => {
            // Rob: competition number bib across the torso.
            let bib = rgb(0.96, 0.96, 0.98);
            b.cuboid(
                Vec3::new(0.172, -0.04, 0.0),
                Vec3::new(0.008, 0.095, 0.115),
                bib,
            );
            b.cuboid(
                Vec3::new(-0.172, -0.04, 0.0),
                Vec3::new(0.008, 0.095, 0.115),
                bib,
            );
            b.cuboid(
                Vec3::new(0.178, -0.04, 0.0),
                Vec3::new(0.006, 0.045, 0.065),
                rgb(0.1, 0.12, 0.2),
            );
        }
        3 => {
            // Jules: puffer vest collar ridges over freestyle hoodie.
            for side in [-1.0f32, 1.0] {
                b.cuboid(
                    Vec3::new(0.0, 0.12, side * 0.22),
                    Vec3::new(0.135, 0.042, 0.02),
                    trim,
                );
            }
        }
        _ => {}
    }
    b.build()
}

/// Continuous pleated 360° parka snow-skirt ring hinged at `(0, 0, 0)` that
/// flares and tilts with cloth physics without ever opening corner gaps.
fn skirt_bell_mesh(sc: &StyleColors, lower: bool) -> Mesh {
    let mut b = MeshBuilder::default();
    let j = sc.jacket;
    let jd = mix(j, [0.0, 0.0, 0.0, 1.0], 0.16);
    let trim = sc.jacket_trim;
    let rings = if !lower {
        [
            Ring {
                rx: 0.162,
                rz: 0.204,
                ripple: 0.04,
                freq: 6.0,
                ..Ring::new(-0.065, 0.162, 0.204, j)
            },
            Ring {
                rx: 0.156,
                rz: 0.198,
                ripple: 0.03,
                freq: 5.0,
                ..Ring::new(-0.02, 0.156, 0.198, j)
            },
            Ring {
                rx: 0.150,
                rz: 0.190,
                ..Ring::new(0.025, 0.150, 0.190, jd)
            },
        ]
    } else {
        [
            Ring {
                rx: 0.174,
                rz: 0.216,
                ripple: 0.055,
                freq: 6.0,
                ..Ring::new(-0.085, 0.174, 0.216, trim)
            },
            Ring {
                rx: 0.168,
                rz: 0.210,
                ripple: 0.045,
                freq: 6.0,
                ..Ring::new(-0.04, 0.168, 0.210, j)
            },
            Ring {
                rx: 0.158,
                rz: 0.198,
                ripple: 0.03,
                freq: 6.0,
                ..Ring::new(0.015, 0.158, 0.198, j)
            },
        ]
    };
    b.loft(&rings, 16, false, true);
    if !lower {
        // Side hand-warmer pocket flaps.
        for side in [-1.0f32, 1.0] {
            b.cuboid_rot(
                Vec3::new(0.04, -0.035, side * 0.198),
                Quat::from_rotation_y(-side * 0.2),
                Vec3::new(0.052, 0.014, 0.012),
                trim,
            );
        }
    }
    b.build()
}

fn hood_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let outer = sc.jacket;
    let lining = sc.jacket_trim;
    let rings = [
        Ring {
            center: Vec3::new(0.01, -0.015, 0.0),
            rx: 0.082,
            rz: 0.118,
            front_col: lining,
            ..Ring::new(-0.015, 0.082, 0.118, outer)
        },
        Ring {
            center: Vec3::new(-0.035, 0.02, 0.0),
            rx: 0.095,
            rz: 0.132,
            tilt_z: 0.3,
            front_col: lining,
            ripple: 0.04,
            freq: 3.0,
            ..Ring::new(0.02, 0.095, 0.132, outer)
        },
        Ring {
            center: Vec3::new(-0.08, 0.045, 0.0),
            rx: 0.075,
            rz: 0.105,
            tilt_z: 0.45,
            front_col: outer,
            ripple: 0.03,
            freq: 3.0,
            ..Ring::new(0.045, 0.075, 0.105, outer)
        },
    ];
    b.loft(&rings, 12, true, true);
    // Drawstring toggles at front of collar.
    for side in [-1.0f32, 1.0] {
        b.cuboid(
            Vec3::new(0.115, -0.03, side * 0.07),
            Vec3::new(0.01, 0.032, 0.01),
            sc.stripe,
        );
    }
    b.build()
}

fn upper_arm_mesh(sc: &StyleColors, side: f32) -> Mesh {
    let mut b = MeshBuilder::default();
    let j = sc.jacket;
    let yoke = sc.jacket_yoke;
    let trim = sc.jacket_trim;
    let rings = [
        Ring {
            rx: 0.068,
            rz: 0.068,
            ripple: 0.05,
            freq: 4.0,
            ..Ring::new(-0.155, 0.068, 0.068, j)
        },
        Ring {
            rx: 0.076,
            rz: 0.076,
            ripple: 0.04,
            freq: 3.0,
            ..Ring::new(-0.03, 0.076, 0.076, j)
        },
        Ring {
            rx: 0.079,
            rz: 0.079,
            ..Ring::new(0.04, 0.079, 0.079, trim)
        },
        Ring {
            rx: 0.082,
            rz: 0.082,
            ..Ring::new(0.155, 0.082, 0.082, yoke)
        },
    ];
    b.loft(&rings, 10, true, true);
    // Outer sleeve patch.
    b.cuboid(
        Vec3::new(0.0, 0.04, side * 0.078),
        Vec3::new(0.028, 0.025, 0.008),
        sc.stripe,
    );
    b.build()
}

fn forearm_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let j = sc.jacket;
    let cuff = sc.jacket_yoke;
    let rings = [
        Ring {
            rx: 0.064,
            rz: 0.064,
            ..Ring::new(-0.145, 0.064, 0.064, cuff)
        },
        Ring {
            rx: 0.072,
            rz: 0.07,
            ripple: 0.06,
            freq: 4.0,
            ..Ring::new(-0.07, 0.072, 0.07, j)
        },
        Ring {
            rx: 0.074,
            rz: 0.072,
            ripple: 0.04,
            freq: 3.0,
            ..Ring::new(0.04, 0.074, 0.072, j)
        },
        Ring {
            rx: 0.068,
            rz: 0.068,
            ..Ring::new(0.145, 0.068, 0.068, j)
        },
    ];
    b.loft(&rings, 10, true, true);
    b.build()
}

fn mitten_mesh(sc: &StyleColors, side: f32) -> Mesh {
    let mut b = MeshBuilder::default();
    let g = sc.glove;
    let palm = rgb(0.22, 0.2, 0.2);
    let trim = sc.jacket_trim;
    // Mitten body from fingertip (-Y) up to gauntlet wrist cuff (+Y).
    let rings = [
        Ring {
            rx: 0.042,
            rz: 0.05,
            front_col: palm,
            ..Ring::new(-0.065, 0.042, 0.05, g)
        },
        Ring {
            rx: 0.048,
            rz: 0.056,
            front_col: palm,
            ..Ring::new(-0.015, 0.048, 0.056, g)
        },
        Ring {
            rx: 0.046,
            rz: 0.05,
            ..Ring::new(0.02, 0.046, 0.05, trim)
        },
        Ring {
            rx: 0.062,
            rz: 0.064,
            ..Ring::new(0.065, 0.062, 0.064, g)
        },
    ];
    b.loft(&rings, 10, true, false);
    // Distinct angled 3D thumb on the inner side.
    let thumb_rot = Quat::from_rotation_x(side * 0.55) * Quat::from_rotation_z(0.25);
    b.cuboid_rot(
        Vec3::new(0.022, -0.018, -side * 0.055),
        thumb_rot,
        Vec3::new(0.022, 0.036, 0.022),
        g,
    );
    b.build()
}

fn head_mesh(sc: &StyleColors, character: usize) -> Mesh {
    let mut b = MeshBuilder::default();
    let gaiter = sc.jacket_yoke;
    let skin = rgb(0.88, 0.7, 0.58);
    let helm = sc.helmet;
    let vent = rgb(0.1, 0.1, 0.12);
    let rings = [
        // Ribbed fleece neck gaiter / chin warmer.
        Ring {
            rx: 0.095,
            rz: 0.098,
            ripple: 0.05,
            freq: 6.0,
            ..Ring::new(-0.115, 0.095, 0.098, gaiter)
        },
        Ring {
            rx: 0.118,
            rz: 0.115,
            front_col: sc.jacket_trim,
            ..Ring::new(-0.045, 0.118, 0.115, gaiter)
        },
        // Face cheek/nose bridge (+x) and helmet ear pads.
        Ring {
            rx: 0.132,
            rz: 0.136,
            front_col: skin,
            ..Ring::new(0.0, 0.132, 0.136, helm)
        },
        // Main helmet shell.
        Ring {
            rx: 0.145,
            rz: 0.142,
            ..Ring::new(0.055, 0.145, 0.142, helm)
        },
        Ring {
            rx: 0.122,
            rz: 0.12,
            ..Ring::new(0.115, 0.122, 0.12, helm)
        },
    ];
    b.loft(&rings, 14, true, true);

    // Front helmet visor brim (+x), top vent slots, and rear goggle strap clip (-x).
    b.cuboid_rot(
        Vec3::new(0.125, 0.065, 0.0),
        Quat::from_rotation_z(-0.15),
        Vec3::new(0.035, 0.01, 0.095),
        if character % 4 == 3 {
            sc.jacket_trim
        } else {
            helm
        },
    );
    for z in [-0.045f32, 0.045] {
        b.cuboid(
            Vec3::new(0.03, 0.135, z),
            Vec3::new(0.042, 0.008, 0.014),
            vent,
        );
    }
    b.cuboid(
        Vec3::new(-0.142, 0.015, 0.0),
        Vec3::new(0.01, 0.032, 0.022),
        sc.stripe,
    );
    b.build()
}

fn goggles_mesh(sc: &StyleColors) -> Mesh {
    let mut b = MeshBuilder::default();
    let frame = rgb(0.08, 0.08, 0.1);
    let strap = sc.jacket_trim;
    // Elastic goggle strap wrapping around the helmet (outward normals).
    let seg = 16u32;
    let r = 0.146;
    for i in 0..seg {
        let a0 = TAU * i as f32 / seg as f32;
        let a1 = TAU * (i + 1) as f32 / seg as f32;
        if a0.cos() > 0.55 && a1.cos() > 0.55 {
            continue;
        }
        let p0 = Vec3::new(a0.cos() * r, 0.016, a0.sin() * r);
        let p1 = Vec3::new(a1.cos() * r, 0.016, a1.sin() * r);
        let h = Vec3::Y * 0.022;
        b.quad(p0 - h, p0 + h, p1 + h, p1 - h, strap);
    }
    // Curved wrap-around goggle frame and multi-faceted reflective lens on the +x brow.
    let lens_segs = 8i32;
    for i in -lens_segs / 2..lens_segs / 2 {
        let t0 = i as f32 / (lens_segs as f32 * 0.5);
        let t1 = (i + 1) as f32 / (lens_segs as f32 * 0.5);
        let a0 = t0 * 0.72;
        let a1 = t1 * 0.72;
        let rf = 0.138;
        let rl = 0.162;
        let hy = 0.038 * (1.0 - 0.16 * t0 * t0);
        let hy1 = 0.038 * (1.0 - 0.16 * t1 * t1);
        let f0_top = Vec3::new(a0.cos() * rf, 0.016 + hy + 0.012, a0.sin() * rf);
        let f1_top = Vec3::new(a1.cos() * rf, 0.016 + hy1 + 0.012, a1.sin() * rf);
        let f0_bot = Vec3::new(a0.cos() * rf, 0.016 - hy - 0.012, a0.sin() * rf);
        let f1_bot = Vec3::new(a1.cos() * rf, 0.016 - hy1 - 0.012, a1.sin() * rf);
        let l0_top = Vec3::new(a0.cos() * rl, 0.016 + hy, a0.sin() * rl);
        let l1_top = Vec3::new(a1.cos() * rl, 0.016 + hy1, a1.sin() * rl);
        let l0_bot = Vec3::new(a0.cos() * rl, 0.016 - hy, a0.sin() * rl);
        let l1_bot = Vec3::new(a1.cos() * rl, 0.016 - hy1, a1.sin() * rl);
        let col = mix(sc.lens_a, sc.lens_b, (t0 * 0.5 + 0.5).clamp(0.0, 1.0));
        // Outward (+X) winding: (l0_top - l0_bot) is +Y, (l1_bot - l0_bot) is +Z, (+Y).cross(+Z) = +X.
        b.quad(l0_bot, l0_top, l1_top, l1_bot, col);
        b.quad(l0_top, f0_top, f1_top, l1_top, frame);
        b.quad(f0_bot, l0_bot, l1_bot, f1_bot, frame);
    }
    b.build()
}

fn pom_mesh(sc: &StyleColors, character: usize) -> Mesh {
    let mut b = MeshBuilder::default();
    let c1 = sc.scarf_a;
    let c2 = sc.scarf_b;
    let r = if character % 4 == 3 { 0.052 } else { 0.038 };
    let rings = [
        Ring::new(-r * 0.6, r * 0.75, r * 0.75, c2),
        Ring {
            ripple: 0.14,
            freq: 6.0,
            ..Ring::new(0.0, r, r, c1)
        },
        Ring::new(r * 0.6, r * 0.75, r * 0.75, c2),
    ];
    b.loft(&rings, 10, true, true);
    b.build()
}

fn scarf_seg_mesh(sc: &StyleColors, tip: bool) -> Mesh {
    let mut b = MeshBuilder::default();
    let ca = sc.scarf_a;
    let cb = sc.scarf_b;
    let rings = [
        Ring {
            rx: 0.024,
            rz: if tip { 0.072 } else { 0.064 },
            ripple: 0.08,
            freq: 4.0,
            ..Ring::new(-0.085, 0.024, 0.064, if tip { cb } else { ca })
        },
        Ring {
            rx: 0.026,
            rz: 0.068,
            ripple: 0.08,
            freq: 4.0,
            ..Ring::new(0.0, 0.026, 0.068, cb)
        },
        Ring {
            rx: 0.024,
            rz: 0.06,
            ripple: 0.06,
            freq: 4.0,
            ..Ring::new(0.085, 0.024, 0.06, ca)
        },
    ];
    b.loft(&rings, 10, !tip, true);
    if tip {
        // Fringed tassels at the end of the scarf (-Y).
        for k in -2..=2 {
            let z = k as f32 * 0.026;
            b.cuboid(Vec3::new(0.0, -0.11, z), Vec3::new(0.009, 0.03, 0.008), ca);
        }
    }
    b.build()
}

/// Two-bone IK: the middle joint for a chain from `a` reaching toward `t`,
/// bending toward `hint`. Returns `(middle, end)`.
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

/// Where a grab puts a hand on the board (in the board's own frame),which
/// hand (`true` for the lead / left nose-side hand), and how the board itself
/// tweaks `(offset, roll_z, pitch_x, shifty_y)`.
fn grab_spec(g: Grab) -> (Vec3, bool, Vec3, f32, f32, f32) {
    match g {
        Grab::Indy => (
            Vec3::new(0.145, 0.045, 0.04),
            false,
            Vec3::new(0.08, 0.19, 0.0),
            0.38,
            0.06,
            -0.12,
        ),
        Grab::Melon => (
            Vec3::new(-0.145, 0.045, -0.05),
            true,
            Vec3::new(-0.12, 0.21, -0.03),
            -0.46,
            -0.14,
            0.18,
        ),
        Grab::Nose => (
            Vec3::new(0.0, 0.11, -0.73),
            true,
            Vec3::new(0.03, 0.18, 0.08),
            0.12,
            0.46,
            0.1,
        ),
        Grab::Tail => (
            Vec3::new(0.0, 0.11, 0.73),
            false,
            Vec3::new(0.03, 0.18, -0.08),
            0.12,
            -0.46,
            -0.1,
        ),
        Grab::Stalefish => (
            Vec3::new(-0.145, 0.045, 0.11),
            false,
            Vec3::new(-0.11, 0.21, 0.03),
            -0.44,
            0.1,
            0.26,
        ),
        Grab::Method => (
            Vec3::new(-0.145, 0.045, -0.10),
            true,
            Vec3::new(-0.23, 0.27, 0.0),
            -0.72,
            -0.08,
            0.24,
        ),
        Grab::Mute => (
            Vec3::new(0.145, 0.045, -0.09),
            true,
            Vec3::new(0.09, 0.2, -0.02),
            0.4,
            0.12,
            0.24,
        ),
        Grab::Japan => (
            Vec3::new(0.145, 0.045, -0.22),
            true,
            Vec3::new(0.11, 0.23, -0.04),
            0.52,
            0.16,
            -0.3,
        ),
    }
}

struct Joints {
    board_pos: Vec3,
    board_rot: Quat,
    pelvis: Vec3,
    pelvis_rot: Quat,
    chest: Vec3,
    chest_rot: Quat,
    chest_fwd: Vec3,
    neck: Vec3,
    head: Vec3,
    head_rot: Quat,
    knee: [Vec3; 2],
    knee_fwd: [Vec3; 2],
    ankle: [Vec3; 2],
    hip: [Vec3; 2],
    shoulder: [Vec3; 2],
    elbow: [Vec3; 2],
    arm_fwd: [Vec3; 2],
    hand: [Vec3; 2],
    hand_rot: [Quat; 2],
}

fn step_cloth_and_body(pose: &RiderPose, root_tr: &Transform, sim: &mut ClothSim) -> Joints {
    let raw_dt = if sim.init {
        pose.t - sim.prev_t
    } else {
        1.0 / 60.0
    };
    let teleported = !sim.init || (pose.pos - sim.prev_pos).length_squared() > 25.0 || raw_dt < 0.0;
    let dt = if teleported {
        1.0 / 60.0
    } else {
        raw_dt.clamp(0.0, 0.05)
    };

    let world_vel = if pose.vel.length_squared() > 0.01 {
        pose.vel
    } else if dt > 1e-4 && !teleported {
        (pose.pos - sim.prev_pos) / dt
    } else {
        Vec3::ZERO
    };
    let world_acc = if dt > 1e-4 && !teleported {
        ((world_vel - sim.prev_vel) / dt).clamp_length_max(75.0)
    } else {
        Vec3::ZERO
    };

    if teleported {
        sim.smooth_lean = pose.lean;
        sim.smooth_crouch = pose.crouch;
        sim.smooth_tuck = pose.tuck;
        sim.smooth_skid = pose.skid;
        sim.yaw_rate = 0.0;
        sim.land_comp = 0.0;
        sim.land_vel = 0.0;
        sim.land_wobble = 0.0;
        sim.pop_ext = 0.0;
        sim.grab_blend = if pose.grab.is_some() { 1.0 } else { 0.0 };
        if let Some(g) = pose.grab {
            sim.last_grab = g;
        }
    } else if dt > 1e-5 {
        let k_in = (14.0 * dt).min(1.0);
        sim.smooth_lean += (pose.lean - sim.smooth_lean) * k_in;
        sim.smooth_crouch += (pose.crouch - sim.smooth_crouch) * k_in;
        sim.smooth_tuck += (pose.tuck - sim.smooth_tuck) * k_in;
        sim.smooth_skid += (pose.skid - sim.smooth_skid) * k_in;
        let inst_yaw_rate = (wrap_angle(pose.yaw - sim.prev_yaw) / dt).clamp(-18.0, 18.0);
        sim.yaw_rate += (inst_yaw_rate - sim.yaw_rate) * (10.0 * dt).min(1.0);

        // Detect takeoff pop vs landing impact.
        if !sim.prev_airborne && pose.airborne {
            sim.pop_ext = 1.0;
            sim.land_vel -= 2.0;
            sim.hood_vel.x -= 3.5;
        }
        if sim.prev_airborne && !pose.airborne && !matches!(pose.phase, Phase::Crashed(_)) {
            let impact = (sim.prev_vel.y - world_vel.y).abs().max(2.5);
            sim.land_vel += (impact * 0.95).clamp(2.5, 13.0);
            if impact > 5.5 {
                sim.land_wobble = ((impact - 4.5) * 0.16).clamp(0.0, 1.0);
            }
            for sv in &mut sim.skirt_vel {
                *sv += (impact * 0.65).min(8.0);
            }
            sim.hood_vel.x += (impact * 0.85).min(9.0);
            for cv in &mut sim.cuff_vel {
                *cv += (impact * 0.7).min(7.0);
            }
        }

        sim.pop_ext = (sim.pop_ext - dt * 5.5).max(0.0);
        sim.land_wobble = (sim.land_wobble - dt * 2.2).max(0.0);

        // Landing absorption spring-damper.
        let land_acc = -135.0 * sim.land_comp - 15.0 * sim.land_vel;
        sim.land_vel += land_acc * dt;
        sim.land_comp = (sim.land_comp + sim.land_vel * dt).clamp(-0.18, 0.55);

        if let Some(g) = pose.grab {
            sim.last_grab = g;
            sim.grab_blend = (sim.grab_blend + dt * 11.0).min(1.0);
        } else {
            sim.grab_blend = (sim.grab_blend - dt * 9.0).max(0.0);
        }
    }

    let j = compute_joints(pose, sim);

    // Cloth & gear secondary spring simulation.
    let inv_root = root_tr.rotation.inverse();
    let local_vel = inv_root * world_vel;
    let local_acc = inv_root * world_acc;
    let speed = world_vel.length();

    if dt > 1e-5 {
        let up_wind = (-local_vel.y).max(0.0);
        let crouch_push = (sim.smooth_crouch * 0.35 + sim.grab_blend * 0.38 + sim.land_comp * 0.4)
            .clamp(0.0, 0.75);
        let air_billow = if pose.airborne { 0.22 } else { 0.0 } + (up_wind * 0.045).min(0.35);
        let base_targets = [
            // Front (+x): pushed out by thighs when crouching/grabbing.
            0.08 + crouch_push * 0.85 + air_billow * 0.7 + (-local_vel.x).max(0.0) * 0.015,
            // Back (-x): streams up with speed and tuck.
            0.10 + crouch_push * 0.35
                + air_billow
                + (speed * 0.018).min(0.38)
                + sim.smooth_tuck * 0.22,
            // Left/Nose (-z): flares with carving and speed.
            0.09 + crouch_push * 0.55 + air_billow + (local_vel.z).max(0.0) * 0.025,
            // Right/Tail (+z): flares with carving and trailing wind.
            0.09 + crouch_push * 0.55 + air_billow + (speed * 0.014).min(0.3),
        ];
        let freq = 13.0 + speed * 0.85;
        let amp = (0.025 + speed * 0.0045).min(0.14);
        for (q, &base_target) in base_targets.iter().enumerate() {
            let wave = (pose.t * freq + q as f32 * 1.65).sin() * amp
                + (pose.t * (freq * 1.7) + q as f32 * 2.4).cos() * (amp * 0.45);
            let inertia = (-local_acc.y * 0.012).clamp(-0.25, 0.35);
            let target = (base_target + wave + inertia).clamp(0.04, 0.92);
            let acc = (target - sim.skirt_angle[q]) * 95.0 - sim.skirt_vel[q] * 9.5;
            sim.skirt_vel[q] += acc * dt;
            sim.skirt_angle[q] = (sim.skirt_angle[q] + sim.skirt_vel[q] * dt).clamp(0.02, 0.95);
        }

        // Hood 2-axis spring (x = lift pitch, y = lateral roll).
        let hood_target = Vec2::new(
            (air_billow * 1.1 - sim.smooth_tuck * 0.25
                + (pose.t * (11.0 + speed * 0.6)).sin() * amp
                - local_acc.y * 0.012)
                .clamp(-0.35, 0.75),
            (-sim.smooth_lean * 0.35
                + (sim.yaw_rate * 0.04).clamp(-0.4, 0.4)
                + (pose.t * 9.5).cos() * amp)
                .clamp(-0.55, 0.55),
        );
        let h_acc = (hood_target - sim.hood_angles) * 85.0 - sim.hood_vel * 8.5;
        sim.hood_vel += h_acc * dt;
        sim.hood_angles = (sim.hood_angles + sim.hood_vel * dt)
            .clamp(Vec2::new(-0.4, -0.6), Vec2::new(0.85, 0.6));

        // Pant cuffs spring jounce & wind rustle.
        for side in 0..2 {
            let chatter = if !pose.airborne {
                (pose.t * 26.0 + side as f32 * 1.9).sin() * (speed * 0.0018).min(0.03)
            } else {
                (pose.t * 16.0 + side as f32).sin() * 0.018
            };
            let target = (-local_acc.y * 0.0025 + chatter).clamp(-0.045, 0.055);
            let c_acc = (target - sim.cuff_disp[side]) * 120.0 - sim.cuff_vel[side] * 10.0;
            sim.cuff_vel[side] += c_acc * dt;
            sim.cuff_disp[side] =
                (sim.cuff_disp[side] + sim.cuff_vel[side] * dt).clamp(-0.05, 0.06);
        }

        // Helmet pom spring in head space.
        let pom_target = Vec3::new(
            (-local_vel.x * 0.003 + (pose.t * 15.0).sin() * amp * 0.25).clamp(-0.04, 0.04),
            (-local_acc.y * 0.0015 + air_billow * 0.04).clamp(-0.03, 0.04),
            (local_vel.z * 0.003 + sim.smooth_lean * 0.025).clamp(-0.04, 0.04),
        );
        let p_acc = (pom_target - sim.pom_pos) * 110.0 - sim.pom_vel * 9.0;
        sim.pom_vel += p_acc * dt;
        sim.pom_pos = (sim.pom_pos + sim.pom_vel * dt).clamp_length_max(0.055);
    }

    // World-space 3-link scarf simulation.
    let anchor_local = j.neck + j.chest_rot * Vec3::new(-0.085, -0.015, 0.075);
    let anchor_world = root_tr.transform_point(anchor_local);
    let seg_len = 0.17;
    let back_world =
        (root_tr.rotation * (j.chest_rot * Vec3::new(-0.8, -0.3, 0.45))).normalize_or(Vec3::NEG_Y);

    if teleported {
        sim.scarf_pos[0] = anchor_world;
        sim.scarf_vel[0] = world_vel;
        for k in 1..4 {
            sim.scarf_pos[k] = anchor_world + back_world * (seg_len * k as f32);
            sim.scarf_vel[k] = world_vel;
        }
    } else {
        sim.scarf_pos[0] = anchor_world;
        sim.scarf_vel[0] = world_vel;
        if dt > 1e-5 {
            let sub_steps = 3;
            let sub_dt = dt / sub_steps as f32;
            let breeze = Vec3::new(
                (pose.t * 1.7).sin() * 1.4 - 0.8,
                (pose.t * 2.3).cos() * 0.4,
                (pose.t * 1.3).cos() * 1.2 + 0.6,
            );
            for _ in 0..sub_steps {
                for k in 1..4 {
                    let air_vy = if pose.airborne {
                        world_vel.y * 0.55
                    } else {
                        world_vel.y * 0.18
                    };
                    let rel_air = breeze - Vec3::new(world_vel.x, air_vy, world_vel.z);
                    let flutter_freq = 11.0 + speed * 0.9;
                    let flutter_amp = (0.8 + speed * 0.22).min(5.5);
                    let turb = Vec3::new(
                        (pose.t * flutter_freq + k as f32 * 1.4).sin(),
                        (pose.t * (flutter_freq * 1.3) + k as f32 * 2.1).cos() * 0.6,
                        (pose.t * (flutter_freq * 0.85) + k as f32 * 1.8).cos(),
                    ) * flutter_amp;
                    let gravity = Vec3::new(0.0, -9.81, 0.0);
                    let aero =
                        (rel_air * 2.8 + turb - sim.scarf_vel[k] * 3.2).clamp_length_max(90.0);
                    sim.scarf_vel[k] += (gravity + aero) * sub_dt;
                    sim.scarf_pos[k] += sim.scarf_vel[k] * sub_dt;

                    // Distance constraint to parent node.
                    let prev = sim.scarf_pos[k - 1];
                    let delta = sim.scarf_pos[k] - prev;
                    let dir = delta.normalize_or(back_world);
                    let constrained = prev + dir * seg_len;
                    sim.scarf_vel[k] += (constrained - sim.scarf_pos[k]) / sub_dt * 0.45;
                    sim.scarf_pos[k] = constrained;

                    // Keep scarf outside the torso and head in rider-local space.
                    let mut lp = inv_root * (sim.scarf_pos[k] - root_tr.translation);
                    let torso_c = j.chest + Vec3::new(0.02, 0.14, 0.0);
                    let td = lp - torso_c;
                    if td.length_squared() < 0.22 * 0.22 {
                        lp = torso_c + td.normalize_or(Vec3::NEG_X) * 0.22;
                    }
                    let hd = lp - j.head;
                    if hd.length_squared() < 0.165 * 0.165 {
                        lp = j.head + hd.normalize_or(Vec3::NEG_X) * 0.165;
                    }
                    if lp.y > j.chest.y - 0.05 && lp.x > -0.04 {
                        lp.x = -0.04;
                    }
                    sim.scarf_pos[k] = root_tr.translation + root_tr.rotation * lp;
                }
            }
        }
    }

    sim.init = true;
    sim.prev_pos = pose.pos;
    sim.prev_vel = world_vel;
    sim.prev_yaw = pose.yaw;
    sim.prev_airborne = pose.airborne;
    sim.prev_t = pose.t;
    j
}

fn compute_joints(p: &RiderPose, sim: &ClothSim) -> Joints {
    let crashed = matches!(p.phase, Phase::Crashed(_));
    let crash_t = match p.phase {
        Phase::Crashed(t) => t,
        _ => 0.0,
    };
    let recovering = crashed && crash_t > 1.15;
    let gb = if crashed { 0.0 } else { sim.grab_blend };
    let (grab_local, grab_lead, g_ofs, g_roll, g_pitch, g_yaw) = grab_spec(sim.last_grab);

    // Effective crouch combines input crouch, grab tuck, landing shock compression, and ollie pop.
    let base_c = if crashed {
        if recovering { 0.65 } else { 0.18 }
    } else {
        (sim.smooth_crouch * (1.0 - gb) + 0.92 * gb + sim.land_comp - 0.38 * sim.pop_ext)
            .clamp(0.0, 1.0)
    };

    let idle_bob = if p.idle {
        (p.t * 2.2).sin() * 0.024 + (p.t * 4.4).cos() * 0.008
    } else {
        0.0
    };
    let idle_sway = if p.idle {
        (p.t * 1.1).sin() * 0.035
    } else {
        0.0
    };

    // Board position & orientation in the rider's root frame (tweaked during grabs & pops).
    let pop_pitch = sim.pop_ext * 0.22;
    let board_pos = if crashed && !recovering {
        Vec3::new(0.0, 0.04 * (p.t * 9.0).sin(), 0.0)
    } else {
        g_ofs * gb + Vec3::Y * (sim.pop_ext * 0.05)
    };
    let board_rot = Quat::from_rotation_y(g_yaw * gb)
        * Quat::from_rotation_x(g_pitch * gb + pop_pitch)
        * Quat::from_rotation_z(g_roll * gb);

    let ankle = [
        board_pos + board_rot * Vec3::new(0.0, 0.11, -0.25),
        board_pos + board_rot * Vec3::new(0.0, 0.11, 0.25),
    ];

    // Pelvis position with carving hip angulation, braking weight-shift, and idle sway.
    let carve_hip_x = sim.smooth_lean * 0.085;
    let skid_back_z = sim.smooth_skid * 0.075;
    let pelvis = Vec3::new(
        -0.045 - 0.11 * base_c + carve_hip_x - 0.06 * gb,
        0.96 - 0.42 * base_c + idle_bob,
        skid_back_z + idle_sway,
    );

    let pelvis_yaw = if p.idle {
        (p.t * 0.9).sin() * 0.08
    } else {
        -sim.smooth_lean * 0.14 + sim.smooth_skid * 0.18 + g_yaw * gb * 0.5
    };
    let pelvis_roll = sim.smooth_lean * 0.12;
    let pelvis_rot = Quat::from_rotation_y(pelvis_yaw) * Quat::from_rotation_x(pelvis_roll);
    let pelvis_fwd = (pelvis_rot * Vec3::X).normalize_or(Vec3::X);

    let hip = [
        pelvis + pelvis_rot * Vec3::new(0.0, -0.02, -0.11),
        pelvis + pelvis_rot * Vec3::new(0.0, -0.02, 0.11),
    ];

    // Expressive knee drive: angulating into carves and tweaking knees inward on style grabs.
    let (tweak_l, tweak_r) = match sim.last_grab {
        Grab::Japan | Grab::Mute => (0.45 * gb, -0.15 * gb),
        Grab::Indy | Grab::Stalefish => (-0.15 * gb, -0.45 * gb),
        Grab::Method => (0.25 * gb, -0.25 * gb),
        _ => (0.0, 0.0),
    };
    let carve_knee_z = -sim.smooth_lean.abs() * 0.18;
    let knee_hint = [
        Vec3::new(1.0, 0.05, -0.28 + tweak_l + carve_knee_z),
        Vec3::new(1.0, 0.05, 0.28 + tweak_r + carve_knee_z * 0.5),
    ];
    let mut knee = [Vec3::ZERO; 2];
    let mut knee_fwd = [Vec3::X; 2];
    for i in 0..2 {
        knee[i] = ik(hip[i], ankle[i], 0.45, 0.45, knee_hint[i]).0;
        let mid = (hip[i] + ankle[i]) * 0.5;
        knee_fwd[i] = (knee[i] - mid).normalize_or(pelvis_fwd);
    }

    // Two-segment articulated spine: pelvis -> chest -> neck, with forward bend,
    // lateral counter-angulation, and 3D torso twist.
    let method_arch = if sim.last_grab == Grab::Method {
        -0.32 * gb
    } else {
        0.0
    };
    let bend = (0.12
        + 0.36 * base_c
        + 0.34 * sim.smooth_tuck
        + 0.28 * gb
        + sim.land_comp * 0.35
        + method_arch)
        .clamp(-0.15, 1.15);
    let side_lean = (-sim.smooth_lean * 0.22
        + if p.idle {
            (p.t * 1.1).cos() * 0.05
        } else {
            0.0
        })
    .clamp(-0.35, 0.35);
    let spin_lead = if p.airborne {
        (sim.yaw_rate * 0.065).clamp(-0.42, 0.42)
    } else {
        0.0
    };
    let grab_twist = if gb > 0.01 {
        if grab_lead { -0.28 * gb } else { 0.28 * gb }
    } else {
        0.0
    };
    let torso_twist = if p.idle {
        0.22 + (p.t * 0.85).sin() * 0.18
    } else {
        0.16 - sim.smooth_lean * 0.26 + spin_lead + grab_twist
    };

    let lower_up =
        (Quat::from_rotation_x(side_lean * 0.5) * Quat::from_rotation_z(-bend * 0.55) * Vec3::Y)
            .normalize_or(Vec3::Y);
    let chest = pelvis + Vec3::Y * 0.04 + lower_up * 0.22;

    let upper_up = (Quat::from_rotation_x(side_lean) * Quat::from_rotation_z(-bend) * Vec3::Y)
        .normalize_or(Vec3::Y);
    let neck = chest + upper_up * 0.34;

    let chest_rot = Quat::from_rotation_y(torso_twist)
        * Quat::from_rotation_x(side_lean)
        * Quat::from_rotation_z(-bend);
    let chest_fwd = (chest_rot * Vec3::X).normalize_or(Vec3::X);

    // Expressive head look-ahead, horizon levelling, and tuck chin-lift.
    let head = neck + upper_up * 0.19 + chest_rot * Vec3::new(0.025, 0.02, -0.015);
    let head_yaw = if crashed {
        (p.t * 4.5).sin() * 0.35
    } else if p.idle {
        0.48 + (p.t * 1.15).sin() * 0.32
    } else if p.finished {
        0.25 + (p.t * 2.5).sin() * 0.2
    } else if p.airborne {
        (0.35 + spin_lead * 1.2 + grab_twist * 0.6).clamp(-0.75, 0.85)
    } else {
        (0.46 - sim.smooth_lean * 0.28).clamp(-0.65, 0.85)
    };
    let head_pitch = if p.idle {
        (p.t * 1.8).cos() * 0.06
    } else {
        // Lift the chin when crouched/tucked so the rider looks down the piste.
        (-bend * 0.35 + 0.12 * gb - 0.15 * sim.pop_ext).clamp(-0.45, 0.35)
    };
    let head_roll = (-side_lean * 0.6 + sim.smooth_lean * 0.18).clamp(-0.35, 0.35);
    let head_rot = Quat::from_rotation_y(head_yaw)
        * Quat::from_rotation_x(head_roll)
        * Quat::from_rotation_z(head_pitch);

    let shoulder = [
        neck + chest_rot * Vec3::new(0.0, -0.055, -0.215),
        neck + chest_rot * Vec3::new(0.0, -0.055, 0.215),
    ];

    let wave = (p.t * 3.4).sin();
    let wave_cos = (p.t * 3.4).cos();
    let wobble = sim.land_wobble * (p.t * 14.0).sin();

    // Base non-grabbing hand targets for each state.
    let base_hand = if crashed {
        if recovering {
            // Pushing up off the snow with the lead hand while righting the body.
            let push = ((crash_t - 1.15) / (CRASH_TIME - 1.15).max(0.1)).clamp(0.0, 1.0);
            [
                shoulder[0] + Vec3::new(0.28, -0.38 + 0.12 * push, -0.18),
                shoulder[1] + Vec3::new(0.18, -0.25 + 0.1 * push, 0.28),
            ]
        } else {
            [
                shoulder[0] + Vec3::new(0.24 + 0.12 * wave_cos, 0.22 + 0.16 * wave, -0.42),
                shoulder[1] + Vec3::new(-0.22 - 0.12 * wave, 0.26 - 0.16 * wave_cos, 0.42),
            ]
        }
    } else if p.finished {
        // Celebratory fist pump across the finish line!
        let pump = (p.t * 6.5).sin().max(0.0) * 0.12;
        [
            shoulder[0] + Vec3::new(0.16, 0.36 + pump, -0.22),
            shoulder[1] + Vec3::new(0.12, -0.26, 0.32),
        ]
    } else if sim.smooth_tuck > 0.45 {
        // Aerodynamic downhill racer tuck: hands together in front of chest/chin.
        let tk = ((sim.smooth_tuck - 0.45) / 0.55).clamp(0.0, 1.0);
        let ride_l = shoulder[0] + chest_rot * Vec3::new(0.22, -0.28, -0.28);
        let ride_r = shoulder[1] + chest_rot * Vec3::new(0.12, -0.34, 0.26);
        let tuck_l = shoulder[0] + chest_rot * Vec3::new(0.36, -0.14, 0.12);
        let tuck_r = shoulder[1] + chest_rot * Vec3::new(0.36, -0.14, -0.12);
        [ride_l.lerp(tuck_l, tk), ride_r.lerp(tuck_r, tk)]
    } else if p.airborne {
        // Stylish air poise: arms wide and fluidly counter-balancing spin & height.
        [
            shoulder[0] + chest_rot * Vec3::new(0.16 + 0.06 * wave_cos, 0.04 + 0.08 * wave, -0.46),
            shoulder[1] + chest_rot * Vec3::new(0.08 - 0.06 * wave, 0.06 - 0.08 * wave_cos, 0.46),
        ]
    } else if p.idle {
        // Relaxed start-gate readiness: subtle breathing and weight shift.
        let look_gesture = ((p.t * 0.55).sin() - 0.65).max(0.0) * 0.6;
        [
            shoulder[0]
                + chest_rot
                    * Vec3::new(
                        0.16 + 0.2 * look_gesture,
                        -0.44 + 0.32 * look_gesture,
                        -0.08 + 0.06 * look_gesture,
                    ),
            shoulder[1] + chest_rot * Vec3::new(0.12, -0.46 + 0.02 * wave, 0.09),
        ]
    } else {
        // Dynamic carving & skidding poise with landing wobble recovery.
        let l = sim.smooth_lean;
        let sk = sim.smooth_skid;
        let pop = sim.pop_ext * 0.22;
        [
            shoulder[0]
                + chest_rot
                    * Vec3::new(
                        0.24 + l * 0.16 + sk * 0.14,
                        -0.28 + l.abs() * 0.12 + pop + wobble * 0.22,
                        -0.34 - l.abs() * 0.06,
                    ),
            shoulder[1]
                + chest_rot
                    * Vec3::new(
                        0.06 - l * 0.18 + sk * 0.12,
                        -0.34 + l.abs() * 0.16 + pop - wobble * 0.22,
                        0.32 + l.abs() * 0.08,
                    ),
        ]
    };

    // When grabbing, the grabbing hand reaches to the tweaked board edge while
    // the free hand lifts into a stylish counter-balance pose!
    let mut hand = base_hand;
    if gb > 0.005 {
        let board_grab_world = board_pos + board_rot * grab_local;
        let free_target = if grab_lead {
            shoulder[1] + chest_rot * Vec3::new(-0.08, 0.24, 0.46)
        } else {
            shoulder[0] + chest_rot * Vec3::new(0.18, 0.22, -0.46)
        };
        if grab_lead {
            hand[0] = base_hand[0].lerp(board_grab_world, gb);
            hand[1] = base_hand[1].lerp(free_target, gb);
        } else {
            hand[1] = base_hand[1].lerp(board_grab_world, gb);
            hand[0] = base_hand[0].lerp(free_target, gb);
        }
    }

    let mut elbow = [Vec3::ZERO; 2];
    let mut arm_fwd = [Vec3::X; 2];
    let mut hand_rot = [Quat::IDENTITY; 2];
    for i in 0..2 {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hint = chest_rot * Vec3::new(-0.22, -0.28, side * 0.85);
        let (e, h) = ik(shoulder[i], hand[i], 0.31, 0.29, hint);
        elbow[i] = e;
        hand[i] = h;
        let mid = (shoulder[i] + h) * 0.5;
        arm_fwd[i] = (mid - e).normalize_or(chest_fwd);
        let fore_up = (e - h).normalize_or(Vec3::Y);
        let hx = (arm_fwd[i] - fore_up * arm_fwd[i].dot(fore_up)).normalize_or(Vec3::X);
        let hz = hx.cross(fore_up).normalize_or(Vec3::Z);
        hand_rot[i] = Quat::from_mat3(&Mat3::from_cols(hx, fore_up, hz));
    }

    Joints {
        board_pos,
        board_rot,
        pelvis,
        pelvis_rot,
        chest,
        chest_rot,
        chest_fwd,
        neck,
        head,
        head_rot,
        knee,
        knee_fwd,
        ankle,
        hip,
        shoulder,
        elbow,
        arm_fwd,
        hand,
        hand_rot,
    }
}

/// Positions a bone with local `-Y` at `distal`, `+Y` at `proximal`, and `+X`
/// facing `forward_hint` so garment details (pockets, knees, zippers) never twist.
fn bone_oriented(distal: Vec3, proximal: Vec3, forward_hint: Vec3, nominal: f32) -> Transform {
    let d = proximal - distal;
    let len = d.length().max(1e-3);
    let up = d / len;
    let x = (forward_hint - up * forward_hint.dot(up))
        .normalize_or(up.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::X));
    let z = x.cross(up).normalize_or(Vec3::Z);
    let x = up.cross(z).normalize_or(Vec3::X);
    Transform {
        translation: (distal + proximal) * 0.5,
        rotation: Quat::from_mat3(&Mat3::from_cols(x, up, z)),
        scale: Vec3::new(
            1.0,
            if nominal > 0.0 {
                (len / nominal).clamp(0.4, 2.0)
            } else {
                1.0
            },
            1.0,
        ),
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
            let k = (1.0 - t / 1.15).max(0.0);
            let recover = ((t - 1.15) / (CRASH_TIME - 1.15).max(0.1)).clamp(0.0, 1.0);
            let spin = t.min(1.15) * 7.0;
            let fall_z = -(PI / 2.0) * (1.0 - k) * (1.0 - recover * 0.85) - spin * k;
            let rot =
                heading * Quat::from_rotation_z(fall_z) * Quat::from_rotation_x(spin * 0.6 * k);
            Transform::from_translation(p.pos + Vec3::Y * (0.22 * (1.0 - recover) + 0.6 * k))
                .with_rotation(rot)
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
    mut roots: Query<(&RiderPose, &mut ClothSim, &mut Transform, &Children), Without<Bone>>,
    mut bones: Query<(&Bone, &mut Transform)>,
) {
    for (pose, mut sim, mut t, children) in &mut roots {
        *t = root_transform(pose);
        let j = step_cloth_and_body(pose, &t, &mut sim);
        let inv_root = t.rotation.inverse();
        let scarf_local: [Vec3; 4] =
            std::array::from_fn(|k| inv_root * (sim.scarf_pos[k] - t.translation));
        let waist_rim = j.pelvis + j.pelvis_rot * Vec3::new(0.0, 0.055, 0.0);
        let rustle = 1.0 + (pose.vel.length() * 0.0025).min(0.05) * (pose.t * 24.0).sin();

        for child in children.iter() {
            let Ok((bone, mut bt)) = bones.get_mut(child) else {
                continue;
            };
            let (part, len) = (bone.0, bone.1);
            *bt = match part {
                Part::Board => Transform::from_translation(j.board_pos).with_rotation(j.board_rot),
                Part::ThighL => bone_oriented(j.knee[0], j.hip[0], j.knee_fwd[0], len),
                Part::ShinL => bone_oriented(j.ankle[0], j.knee[0], j.knee_fwd[0], len),
                Part::CuffL => {
                    let mut tr = bone_oriented(j.ankle[0], j.knee[0], j.knee_fwd[0], 0.0);
                    let dir = (j.knee[0] - j.ankle[0]).normalize_or(Vec3::Y);
                    tr.translation = j.ankle[0] + dir * (0.065 + sim.cuff_disp[0]);
                    tr.scale = Vec3::new(rustle, 1.0, rustle);
                    tr
                }
                Part::ThighR => bone_oriented(j.knee[1], j.hip[1], j.knee_fwd[1], len),
                Part::ShinR => bone_oriented(j.ankle[1], j.knee[1], j.knee_fwd[1], len),
                Part::CuffR => {
                    let mut tr = bone_oriented(j.ankle[1], j.knee[1], j.knee_fwd[1], 0.0);
                    let dir = (j.knee[1] - j.ankle[1]).normalize_or(Vec3::Y);
                    tr.translation = j.ankle[1] + dir * (0.065 + sim.cuff_disp[1]);
                    tr.scale = Vec3::new(rustle, 1.0, rustle);
                    tr
                }
                Part::Pelvis => Transform::from_translation(j.pelvis).with_rotation(j.pelvis_rot),
                Part::Waist => bone_oriented(j.pelvis, j.chest, j.chest_fwd, len),
                Part::Torso => bone_oriented(j.chest, j.neck, j.chest_fwd, len),
                Part::SkirtUpper => {
                    let sx = 1.0 + (sim.skirt_angle[0] + sim.skirt_angle[1]) * 0.18;
                    let sz = 1.0 + (sim.skirt_angle[2] + sim.skirt_angle[3]) * 0.16;
                    let pitch = (sim.skirt_angle[0] - sim.skirt_angle[1]) * 0.22;
                    let roll = (sim.skirt_angle[3] - sim.skirt_angle[2]) * 0.22;
                    Transform {
                        translation: waist_rim,
                        rotation: j.pelvis_rot
                            * Quat::from_rotation_x(roll)
                            * Quat::from_rotation_z(pitch),
                        scale: Vec3::new(sx, 1.0, sz),
                    }
                }
                Part::SkirtLower => {
                    let up_pitch = (sim.skirt_angle[0] - sim.skirt_angle[1]) * 0.22;
                    let up_roll = (sim.skirt_angle[3] - sim.skirt_angle[2]) * 0.22;
                    let up_rot = j.pelvis_rot
                        * Quat::from_rotation_x(up_roll)
                        * Quat::from_rotation_z(up_pitch);
                    let sx = 1.02 + (sim.skirt_angle[0] + sim.skirt_angle[1]) * 0.34;
                    let sz = 1.02 + (sim.skirt_angle[2] + sim.skirt_angle[3]) * 0.30;
                    let pitch = (sim.skirt_angle[0] - sim.skirt_angle[1]) * 0.38;
                    let roll = (sim.skirt_angle[3] - sim.skirt_angle[2]) * 0.35;
                    Transform {
                        translation: waist_rim + up_rot * Vec3::new(0.0, -0.055, 0.0),
                        rotation: j.pelvis_rot
                            * Quat::from_rotation_x(roll)
                            * Quat::from_rotation_z(pitch),
                        scale: Vec3::new(sx, 1.0, sz),
                    }
                }
                Part::Hood => {
                    let hinge = j.neck + j.chest_rot * Vec3::new(-0.06, -0.025, 0.0);
                    let rot = j.chest_rot
                        * Quat::from_rotation_x(sim.hood_angles.y)
                        * Quat::from_rotation_z(-sim.hood_angles.x);
                    Transform::from_translation(hinge).with_rotation(rot)
                }
                Part::UpperL => bone_oriented(j.elbow[0], j.shoulder[0], j.arm_fwd[0], len),
                Part::ForeL => bone_oriented(j.hand[0], j.elbow[0], j.arm_fwd[0], len),
                Part::UpperR => bone_oriented(j.elbow[1], j.shoulder[1], j.arm_fwd[1], len),
                Part::ForeR => bone_oriented(j.hand[1], j.elbow[1], j.arm_fwd[1], len),
                Part::Head => Transform::from_translation(j.head).with_rotation(j.head_rot),
                Part::Goggles => Transform::from_translation(j.head).with_rotation(j.head_rot),
                Part::Pom => {
                    let pos = j.head + j.head_rot * Vec3::new(-0.065, 0.145, 0.0) + sim.pom_pos;
                    Transform::from_translation(pos).with_rotation(j.head_rot)
                }
                Part::HandL => Transform::from_translation(j.hand[0]).with_rotation(j.hand_rot[0]),
                Part::HandR => Transform::from_translation(j.hand[1]).with_rotation(j.hand_rot[1]),
                Part::BootL => Transform::from_translation(j.ankle[0])
                    .with_rotation(j.board_rot * Quat::from_rotation_y(-0.22)),
                Part::BootR => Transform::from_translation(j.ankle[1])
                    .with_rotation(j.board_rot * Quat::from_rotation_y(0.10)),
                Part::Scarf0 => bone_oriented(scarf_local[1], scarf_local[0], j.chest_fwd, len),
                Part::Scarf1 => bone_oriented(scarf_local[2], scarf_local[1], j.chest_fwd, len),
                Part::Scarf2 => bone_oriented(scarf_local[3], scarf_local[2], j.chest_fwd, len),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grabs_and_cloth_stay_finite_and_lock_boots_to_board() {
        let mut sim = ClothSim::default();
        for grab in [
            None,
            Some(Grab::Indy),
            Some(Grab::Melon),
            Some(Grab::Nose),
            Some(Grab::Tail),
            Some(Grab::Stalefish),
            Some(Grab::Method),
            Some(Grab::Mute),
            Some(Grab::Japan),
        ] {
            for step in 0..12 {
                let pose = RiderPose {
                    pos: Vec3::new(0.0, 5.0, -step as f32 * 0.5),
                    vel: Vec3::new(1.5, -3.0, -18.0),
                    yaw: 0.2 * step as f32,
                    pitch: 0.1 * step as f32,
                    phase: if grab.is_some() {
                        Phase::Air
                    } else {
                        Phase::Ground
                    },
                    crouch: 0.5,
                    lean: 0.6,
                    tuck: 0.2,
                    skid: 0.1,
                    grab,
                    airborne: grab.is_some(),
                    t: step as f32 / 60.0,
                    ..default()
                };
                let root = root_transform(&pose);
                let j = step_cloth_and_body(&pose, &root, &mut sim);
                assert!(j.pelvis.is_finite() && j.head.is_finite());
                assert!(j.hand[0].is_finite() && j.hand[1].is_finite());
                assert!(sim.scarf_pos.iter().all(|p| p.is_finite()));
                // Boots stay locked to the board's bindings at z = ±0.25.
                let b0 = j.board_pos + j.board_rot * Vec3::new(0.0, 0.11, -0.25);
                let b1 = j.board_pos + j.board_rot * Vec3::new(0.0, 0.11, 0.25);
                assert!((j.ankle[0] - b0).length() < 1e-4);
                assert!((j.ankle[1] - b1).length() < 1e-4);
            }
        }
    }
}
