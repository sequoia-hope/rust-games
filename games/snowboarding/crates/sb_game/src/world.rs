//! The course drawn: the snow in chunks (frustum-culled), the sky dome, the
//! valley floor and the far peaks, the trees, rocks, flags, kicker markings
//! and the start and finish gates, and each course's light: sun, ambient,
//! fog and sky colours.
//!
//! The snow's mesh samples `sb_sim`'s height function at the course's own
//! rows and columns, so what is drawn is what the riders ride on.

use crate::Setup;
use crate::meshes::{MeshBuilder, mix, rgb};
use bevy::asset::RenderAssetUsages;
use bevy::light::{CascadeShadowConfigBuilder, GlobalAmbientLight, NotShadowCaster};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use sb_sim::course::{Course, CourseId, ObstacleKind, PIPE_COPING};
use sb_sim::noise::{fbm, hash, unit};
use std::sync::Arc;

pub fn plugin(app: &mut App) {
    app.init_resource::<Built>()
        .add_systems(Update, ensure_world)
        .add_systems(PostUpdate, follow_sky);
}

/// Everything spawned for the current course.
#[derive(Component)]
pub struct CourseEntity;

#[derive(Component)]
pub struct SkyDome;

#[derive(Component)]
pub struct Sun;

/// The course on screen, and how it looks.
#[derive(Resource, Default)]
pub struct Built {
    pub course: Option<Arc<Course>>,
    pub look: Option<Look>,
}

/// A course's light and colour.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub sky_top: [f32; 3],
    pub sky_horizon: [f32; 3],
    pub sky_below: [f32; 3],
    /// Toward the sun.
    pub sun_dir: Vec3,
    pub sun_color: [f32; 3],
    pub sun_lux: f32,
    pub ambient: [f32; 3],
    pub ambient_brightness: f32,
    pub fog: [f32; 3],
    pub fog_start: f32,
    pub fog_end: f32,
    pub snow: [f32; 3],
    /// Falling snow, 0 .. 1.
    pub snowfall: f32,
}

pub fn look(id: CourseId) -> Look {
    match id {
        CourseId::Crystal => Look {
            sky_top: [0.12, 0.36, 0.82],
            sky_horizon: [0.68, 0.84, 0.98],
            sky_below: [0.82, 0.88, 0.95],
            sun_dir: Vec3::new(-0.45, 0.62, 0.55).normalize(),
            sun_color: [1.0, 0.97, 0.92],
            sun_lux: 5400.0,
            ambient: [0.62, 0.74, 1.0],
            ambient_brightness: 770.0,
            fog: [0.74, 0.85, 0.97],
            fog_start: 300.0,
            fog_end: 2600.0,
            snow: [0.97, 0.98, 1.0],
            snowfall: 0.0,
        },
        CourseId::Timber => Look {
            sky_top: [0.22, 0.32, 0.62],
            sky_horizon: [0.98, 0.76, 0.56],
            sky_below: [0.86, 0.74, 0.7],
            sun_dir: Vec3::new(0.75, 0.3, -0.35).normalize(),
            sun_color: [1.0, 0.8, 0.58],
            sun_lux: 4800.0,
            ambient: [0.55, 0.6, 0.9],
            ambient_brightness: 605.0,
            fog: [0.9, 0.77, 0.7],
            fog_start: 200.0,
            fog_end: 2000.0,
            snow: [1.0, 0.98, 0.96],
            snowfall: 0.25,
        },
        CourseId::Avalanche => Look {
            sky_top: [0.42, 0.48, 0.58],
            sky_horizon: [0.8, 0.83, 0.88],
            sky_below: [0.8, 0.83, 0.88],
            sun_dir: Vec3::new(0.2, 0.75, 0.6).normalize(),
            sun_color: [0.92, 0.94, 1.0],
            sun_lux: 2700.0,
            ambient: [0.75, 0.8, 0.9],
            ambient_brightness: 1430.0,
            fog: [0.8, 0.83, 0.88],
            fog_start: 60.0,
            fog_end: 900.0,
            snow: [0.95, 0.96, 0.98],
            snowfall: 1.0,
        },
        CourseId::Pipe => Look {
            sky_top: [0.1, 0.1, 0.32],
            sky_horizon: [0.95, 0.5, 0.42],
            sky_below: [0.5, 0.36, 0.5],
            sun_dir: Vec3::new(-0.6, 0.22, -0.6).normalize(),
            sun_color: [1.0, 0.62, 0.45],
            sun_lux: 4200.0,
            ambient: [0.55, 0.5, 0.95],
            ambient_brightness: 715.0,
            fog: [0.7, 0.48, 0.55],
            fog_start: 220.0,
            fog_end: 2200.0,
            snow: [1.0, 0.97, 0.98],
            snowfall: 0.0,
        },
    }
}

fn c3(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

/// Rebuilds the scene when the wanted course changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn ensure_world(
    mut commands: Commands,
    setup: Res<Setup>,
    mut built: ResMut<Built>,
    old: Query<Entity, With<CourseEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut fogs: Query<&mut DistanceFog>,
) {
    if built.course.as_ref().is_some_and(|c| c.id == setup.course) {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let course = Arc::new(Course::new(setup.course));
    let look = look(setup.course);
    build(&mut commands, &course, &look, &mut meshes, &mut materials);
    ambient.color = c3(look.ambient);
    ambient.brightness = look.ambient_brightness;
    clear.0 = c3(look.sky_horizon);
    for mut f in &mut fogs {
        apply_fog(&mut f, &look);
    }
    built.course = Some(course);
    built.look = Some(look);
}

pub fn apply_fog(f: &mut DistanceFog, look: &Look) {
    f.color = c3(look.fog);
    f.directional_light_color = c3(look.sun_color).with_alpha(0.35);
    f.directional_light_exponent = 18.0;
    f.falloff = FogFalloff::Linear {
        start: look.fog_start,
        end: look.fog_end,
    };
}

fn build(
    commands: &mut Commands,
    course: &Course,
    look: &Look,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let snow_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.62,
        reflectance: 0.28,
        ..default()
    });
    let solid = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        reflectance: 0.15,
        ..default()
    });
    let shiny = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.35,
        ..default()
    });

    // The sun.
    commands.spawn((
        CourseEntity,
        Sun,
        DirectionalLight {
            color: c3(look.sun_color),
            illuminance: look.sun_lux,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: if cfg!(sb_webgl2) { 1 } else { 2 },
            minimum_distance: 0.5,
            maximum_distance: if cfg!(sb_webgl2) { 60.0 } else { 140.0 },
            first_cascade_far_bound: 22.0,
            overlap_proportion: 0.25,
        }
        .build(),
        Transform::from_translation(look.sun_dir * 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // The snow, in chunks of rows.
    let rows = course.mesh_rows();
    let cols = course.mesh_columns();
    const CHUNK: usize = 48;
    let mut r0 = 0;
    while r0 + 1 < rows.len() {
        let r1 = (r0 + CHUNK).min(rows.len() - 1);
        let mesh = snow_chunk(course, look, &rows[r0..=r1], &cols);
        commands.spawn((
            CourseEntity,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(snow_mat.clone()),
        ));
        r0 = r1;
    }

    // The valley floor below the bottom and the peaks around.
    spawn_scenery(commands, course, look, meshes, solid.clone());

    // The sky.
    let sky = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        CourseEntity,
        SkyDome,
        Mesh3d(meshes.add(sky_mesh(look))),
        MeshMaterial3d(sky),
        NotShadowCaster,
        Transform::default(),
        bevy::camera::visibility::NoFrustumCulling,
    ));

    // Trees and rocks.
    let tree_meshes: Vec<Handle<Mesh>> = (0..3).map(|v| meshes.add(tree_mesh(v))).collect();
    let rock_meshes: Vec<Handle<Mesh>> = (0..4).map(|v| meshes.add(rock_mesh(v))).collect();
    for o in &course.obstacles {
        let yaw = unit(hash(o.variant ^ (o.s * 7.0) as u32)) * std::f32::consts::TAU;
        let (mesh, near) = match o.kind {
            ObstacleKind::Tree => (tree_meshes[o.variant as usize % 3].clone(), true),
            ObstacleKind::Rock => (rock_meshes[o.variant as usize % 4].clone(), true),
        };
        let mut e = commands.spawn((
            CourseEntity,
            Mesh3d(mesh),
            MeshMaterial3d(solid.clone()),
            Transform::from_translation(o.pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(o.scale)),
        ));
        let edge = course.half_width(o.s) + 12.0;
        if !near || o.u.abs() > edge {
            e.insert(NotShadowCaster);
        }
    }

    // Course flags.
    let pole = meshes.add(flag_mesh(rgb(0.95, 0.25, 0.15)));
    let pole_b = meshes.add(flag_mesh(rgb(0.15, 0.4, 0.95)));
    for m in &course.markers {
        let yaw = course.frame(m.s).theta;
        commands.spawn((
            CourseEntity,
            Mesh3d(if m.right_side {
                pole.clone()
            } else {
                pole_b.clone()
            }),
            MeshMaterial3d(shiny.clone()),
            NotShadowCaster,
            Transform::from_translation(m.pos).with_rotation(Quat::from_rotation_y(-yaw)),
        ));
    }

    // Kicker lips, dyed.
    for k in &course.kickers {
        let f = course.frame(k.s);
        let mut b = MeshBuilder::default();
        let n = 12;
        let dye = rgb(0.1, 0.35, 0.95);
        for i in 0..n {
            let u0 = k.u - k.half_width + 2.0 * k.half_width * i as f32 / n as f32;
            let u1 = k.u - k.half_width + 2.0 * k.half_width * (i + 1) as f32 / n as f32;
            let s = k.s - 0.12;
            let a = course.world(s, u0) + Vec3::Y * 0.03;
            let bb = course.world(s, u1) + Vec3::Y * 0.03;
            let c = course.world(s - 0.7, u1) + Vec3::Y * 0.03;
            let d = course.world(s - 0.7, u0) + Vec3::Y * 0.03;
            b.quad(a, d, c, bb, dye);
        }
        for side in [-1.0, 1.0] {
            let base = course.world(k.s - 0.5, k.u + side * (k.half_width + 1.6));
            b.cone(
                base,
                0.25,
                0.0,
                0.8,
                8,
                rgb(1.0, 0.45, 0.05),
                rgb(1.0, 0.6, 0.2),
                0.0,
                1,
            );
        }
        let _ = f;
        commands.spawn((
            CourseEntity,
            Mesh3d(meshes.add(b.build())),
            MeshMaterial3d(shiny.clone()),
            NotShadowCaster,
        ));
    }

    // The start and finish gates.
    for (s, finish) in [(course.start_s + 1.0, false), (course.finish_s, true)] {
        let f = course.frame(s);
        let hw = if course.pipe_weight(s) > 0.5 {
            PIPE_COPING
        } else {
            course.half_width(s)
        }
        .min(16.0);
        let mesh = gate_mesh(hw, finish);
        let ground = course
            .world(s, 0.0)
            .y
            .min(course.world(s, -hw).y)
            .min(course.world(s, hw).y);
        commands.spawn((
            CourseEntity,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(shiny.clone()),
            Transform::from_translation(Vec3::new(f.center.x, ground - 0.5, f.center.z))
                .with_rotation(Quat::from_rotation_y(-f.theta)),
        ));
    }
}

/// One chunk of the snow: the rows given, every column. The inside of a
/// bend is pulled in short of the centre of curvature, so the ribbon of
/// snow never folds over itself far up the valley walls.
fn snow_chunk(course: &Course, look: &Look, rows: &[f32], cols: &[f32]) -> Mesh {
    let (nr, nc) = (rows.len(), cols.len());
    let mut pos = Vec::with_capacity(nr * nc);
    let mut uu = Vec::with_capacity(nr * nc);
    for &s in rows {
        let f = course.frame(s);
        for &u in cols {
            let u = if f.kappa * u > 0.6 { 0.6 / f.kappa } else { u };
            let p = f.center + f.right * u;
            pos.push(Vec3::new(p.x, course.height(s, u), p.z));
            uu.push(u);
        }
    }
    let at = |r: usize, c: usize| pos[r * nc + c];
    let snow = rgb(look.snow[0], look.snow[1], look.snow[2]);
    let shade = mix(snow, rgb(0.82, 0.88, 0.98), 0.6);
    let rock = rgb(0.34, 0.32, 0.33);
    let rock_dark = rgb(0.22, 0.21, 0.23);
    let mut nor = Vec::with_capacity(nr * nc);
    let mut col = Vec::with_capacity(nr * nc);
    for r in 0..nr {
        for c in 0..nc {
            let du = at(r, (c + 1).min(nc - 1)) - at(r, c.saturating_sub(1));
            let ds = at((r + 1).min(nr - 1), c) - at(r.saturating_sub(1), c);
            // Chunks share their edge rows; at a chunk's edge the step
            // either side is the course's own.
            let ds = if r == 0 || r == nr - 1 {
                let s = rows[r];
                let u = uu[r * nc + c];
                course.world(s + 0.5, u) - course.world(s - 0.5, u)
            } else {
                ds
            };
            let n = du.cross(ds).normalize_or(Vec3::Y);
            nor.push(<[f32; 3]>::from(n));
            let p = at(r, c);
            let s = rows[r];
            let u = uu[r * nc + c];
            let hw = course.half_width(s);
            let off = (u.abs() - hw).max(0.0);
            // Rock where it is too steep to hold snow, and out on the walls.
            let n1 = fbm(p.x * 0.02, p.z * 0.02, 99, 3);
            let steep = ((0.62 - n.y) * 6.0 + n1 * 1.5 + if off > 60.0 { 0.3 } else { -1.0 })
                .clamp(0.0, 1.0);
            let wind = fbm(p.x * 0.06, p.z * 0.06, 7, 2) * 0.5 + 0.5;
            let mut cc = mix(
                snow,
                shade,
                wind * 0.35 + if off > 1.0 { 0.15 } else { 0.0 },
            );
            if steep > 0.0 && course.pipe_weight(s) < 0.5 {
                cc = mix(cc, mix(rock, rock_dark, n1 * 0.5 + 0.5), steep);
            }
            // The pipe's coping and its centre line, dyed.
            let pw = course.pipe_weight(s);
            if pw > 0.5 && ((u.abs() - PIPE_COPING).abs() < 0.3 || u.abs() < 0.12) {
                cc = rgb(0.15, 0.4, 0.95);
            }
            col.push(cc);
        }
    }
    let mut idx = Vec::with_capacity((nr - 1) * (nc - 1) * 6);
    for r in 0..nr - 1 {
        for c in 0..nc - 1 {
            let a = (r * nc + c) as u32;
            let b = a + 1;
            let cc = a + nc as u32;
            let d = cc + 1;
            idx.extend([a, b, d, a, d, cc]);
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        pos.iter().map(|p| <[f32; 3]>::from(*p)).collect::<Vec<_>>(),
    );
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    m
}

fn sky_mesh(look: &Look) -> Mesh {
    let mut b = MeshBuilder::default();
    let (top, hor, below) = (
        rgb(look.sky_top[0], look.sky_top[1], look.sky_top[2]),
        rgb(
            look.sky_horizon[0],
            look.sky_horizon[1],
            look.sky_horizon[2],
        ),
        rgb(look.sky_below[0], look.sky_below[1], look.sky_below[2]),
    );
    let sun = rgb(look.sun_color[0], look.sun_color[1], look.sun_color[2]);
    let (rings, segs) = (40u32, 64u32);
    const R: f32 = 5000.0;
    for i in 0..=rings {
        let el = -0.35 + (std::f32::consts::FRAC_PI_2 + 0.35) * i as f32 / rings as f32;
        for j in 0..=segs {
            let az = std::f32::consts::TAU * j as f32 / segs as f32;
            let d = Vec3::new(el.cos() * az.cos(), el.sin(), el.cos() * az.sin());
            let mut c = if d.y >= 0.0 {
                mix(hor, top, d.y.powf(0.55))
            } else {
                mix(hor, below, (-d.y * 6.0).min(1.0))
            };
            let k = d.dot(look.sun_dir).max(0.0);
            let glow = k.powf(400.0) * 2.5 + k.powf(24.0) * 0.35 + k.powf(4.0) * 0.08;
            for ch in 0..3 {
                c[ch] += sun[ch] * glow;
            }
            b.vertex(d * R, -d, c);
        }
    }
    for i in 0..rings {
        for j in 0..segs {
            let a = i * (segs + 1) + j;
            let bb = a + segs + 1;
            b.idx.extend([a, bb, a + 1, a + 1, bb, bb + 1]);
        }
    }
    b.build()
}

/// The valley's floor far below, and a ring of peaks.
fn spawn_scenery(
    commands: &mut Commands,
    course: &Course,
    look: &Look,
    meshes: &mut Assets<Mesh>,
    mat: Handle<StandardMaterial>,
) {
    let top = course.world(0.0, 0.0);
    let bottom = course.world(course.length, 0.0);
    let mid = (top + bottom) * 0.5;
    let snow = rgb(look.snow[0], look.snow[1], look.snow[2]);
    let rock = rgb(0.3, 0.29, 0.32);
    let mut b = MeshBuilder::default();
    // The floor.
    let y = bottom.y - 25.0;
    let e = 6000.0;
    b.quad(
        Vec3::new(mid.x - e, y, mid.z + e),
        Vec3::new(mid.x + e, y, mid.z + e),
        Vec3::new(mid.x + e, y, mid.z - e),
        Vec3::new(mid.x - e, y, mid.z - e),
        mix(snow, rgb(0.8, 0.86, 0.96), 0.3),
    );
    // The peaks, each pushed out until its foot is clear of the whole
    // valley the course runs down (a cone's foot is wide: one placed by
    // distance from the middle alone stood across Crystal Ridge's finish).
    let span = (top - bottom).length().max(600.0);
    let path: Vec<Vec3> = (0..=(course.length / 25.0) as usize)
        .map(|k| course.frame(k as f32 * 25.0).center)
        .collect();
    let clearance = sb_sim::course::FAR + course.half_width(course.finish_s) + 120.0;
    for i in 0..26u32 {
        let a = std::f32::consts::TAU * (i as f32 + unit(hash(i * 3 + course.seed)) * 0.6) / 26.0;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let mut dist = span * 0.6 + 500.0 + unit(hash(i * 7 + 1)) * 900.0;
        let h = top.y - y + 250.0 + unit(hash(i * 11 + 5)) * 700.0;
        let r = h * (0.8 + unit(hash(i * 13)) * 0.5);
        for _ in 0..40 {
            let base = Vec3::new(mid.x, y, mid.z) + out * dist;
            let near = path
                .iter()
                .map(|p| Vec2::new(p.x - base.x, p.z - base.z).length())
                .fold(f32::MAX, f32::min);
            let need = r + clearance;
            if near >= need {
                break;
            }
            dist += need - near + 10.0;
        }
        let base = Vec3::new(mid.x, y, mid.z) + out * dist;
        b.cone(base, r, 0.0, h, 9, rock, snow, 0.45, i);
    }
    commands.spawn((
        CourseEntity,
        Mesh3d(meshes.add(b.build())),
        MeshMaterial3d(mat),
        NotShadowCaster,
    ));
}

fn tree_mesh(variant: u32) -> Mesh {
    let mut b = MeshBuilder::default();
    let trunk = rgb(0.32, 0.2, 0.12);
    b.cone(Vec3::ZERO, 0.32, 0.18, 2.2, 6, trunk, trunk, 0.0, 0);
    let green = [
        rgb(0.06, 0.2, 0.12),
        rgb(0.08, 0.24, 0.13),
        rgb(0.05, 0.17, 0.14),
    ][variant as usize % 3];
    let snow = rgb(0.92, 0.95, 1.0);
    let layers = 3 + variant % 2;
    let mut y = 1.3;
    let mut r = 2.3 - 0.2 * variant as f32;
    for l in 0..layers {
        let h = 3.3 - 0.35 * l as f32;
        b.cone(
            Vec3::Y * y,
            r,
            0.0,
            h,
            9,
            green,
            mix(green, snow, 0.9),
            0.25,
            variant * 31 + l,
        );
        y += h * 0.55;
        r *= 0.74;
    }
    b.build()
}

fn rock_mesh(variant: u32) -> Mesh {
    let mut b = MeshBuilder::default();
    b.rock(
        Vec3::Y * 0.4,
        1.0,
        variant * 101 + 3,
        rgb(0.36, 0.34, 0.35),
        rgb(0.93, 0.95, 1.0),
    );
    b.build()
}

fn flag_mesh(color: [f32; 4]) -> Mesh {
    let mut b = MeshBuilder::default();
    b.cuboid(
        Vec3::Y * 1.0,
        Vec3::new(0.035, 1.0, 0.035),
        rgb(0.95, 0.95, 0.95),
    );
    b.cuboid(
        Vec3::new(0.0, 1.68, 0.32),
        Vec3::new(0.01, 0.28, 0.32),
        color,
    );
    b.build()
}

/// A gate across the piste, `hw` either side of the centre, in the
/// gate's own frame (x across, -z downhill).
fn gate_mesh(hw: f32, finish: bool) -> Mesh {
    let mut b = MeshBuilder::default();
    let post = rgb(0.85, 0.85, 0.9);
    let h = 7.5;
    for side in [-1.0, 1.0] {
        b.cuboid(
            Vec3::new(side * (hw + 0.6), h * 0.5, 0.0),
            Vec3::new(0.35, h * 0.5, 0.35),
            post,
        );
    }
    let n = 24;
    let w = 2.0 * (hw + 0.6) / n as f32;
    for i in 0..n {
        let x = -(hw + 0.6) + w * (i as f32 + 0.5);
        for row in 0..2 {
            let c = if finish {
                if (i + row) % 2 == 0 {
                    rgb(0.05, 0.05, 0.05)
                } else {
                    rgb(0.97, 0.97, 0.97)
                }
            } else if i % 2 == 0 {
                rgb(0.9, 0.12, 0.1)
            } else {
                rgb(0.98, 0.95, 0.9)
            };
            b.cuboid(
                Vec3::new(x, h - 0.45 - row as f32 * 0.7, 0.0),
                Vec3::new(w * 0.5, 0.35, 0.12),
                c,
            );
        }
    }
    b.build()
}

/// Keeps the sky dome centred on the camera.
fn follow_sky(
    cam: Query<&GlobalTransform, With<Camera3d>>,
    mut sky: Query<&mut Transform, With<SkyDome>>,
) {
    let Ok(c) = cam.single() else { return };
    for mut t in &mut sky {
        t.translation = c.translation();
    }
}
