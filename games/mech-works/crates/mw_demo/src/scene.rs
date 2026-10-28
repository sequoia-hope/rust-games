//! The ground, the eroded hillside, the settled 3D gravel, the endless path's
//! obstacles, the 5×3 height scan visualizer, and the light (M5 & M6).

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{CascadeShadowConfigBuilder, GlobalAmbientLight, NotShadowCaster};
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mw_sim::rng::Rng;
use mw_sim::terrain::{HEIGHT_SCAN, ObstacleKind};

use crate::Mode;
use crate::camera::MainCamera;
use crate::lanes::Lanes;
use crate::view::Inspector;

pub const SKY: Color = Color::srgb(0.68, 0.73, 0.79);
pub const SUN_DIR: Vec3 = Vec3::new(0.45, 0.76, 0.38);
/// The level's sun: high behind the base, over the pilot's shoulder on the
/// walk east, so what lies ahead (the spurs, the rubble) faces it rather
/// than standing dark against the light.
pub const LEVEL_SUN_DIR: Vec3 = Vec3::new(-0.4, 0.82, -0.42);
/// The sky's light everywhere, and the level's (brighter: in the cockpit
/// view the hull no longer shows, and the shade must still read).
const AMBIENT: f32 = 280.0;
const LEVEL_AMBIENT: f32 = 520.0;
/// WebGL2 draws one directional light (Bevy's limit there), so the sun is the
/// only one spawned and the sky and ground fills come back as ambient light.
const WEBGL2_FILL_AMBIENT: f32 = if cfg!(pk_webgl2) { 750.0 } else { 0.0 };

/// The ground's side, m.
pub const GROUND: f32 = 400.0;

#[derive(Component)]
pub struct Sun;

/// The cool fill from the sky opposite the sun.
#[derive(Component)]
struct SkyFill;

/// A chipped 3D stone near the camera that wraps along X as the mech walks.
#[derive(Component)]
struct NearStone {
    offset_x: f32,
    z: f32,
    scale: Vec3,
    rot: Quat,
}

/// Visual representation of one streamed obstacle on the endless path (M5).
#[derive(Component)]
struct PathObstacleView;

/// Tracks which obstacle IDs currently have spawned visual entities.
#[derive(Resource, Default)]
struct DrawnObstacles {
    by_id: HashMap<u64, Entity>,
}

/// Shared meshes and materials for path obstacles and height scan markers.
#[derive(Resource)]
struct SceneAssets {
    rock_meshes: Vec<Handle<Mesh>>,
    rock_mat: Handle<StandardMaterial>,
    branch_mesh: Handle<Mesh>,
    limb_mesh: Handle<Mesh>,
    bark_mat: Handle<StandardMaterial>,
    wreck_box: Handle<Mesh>,
    wreck_wheel: Handle<Mesh>,
    wreck_rust: Handle<StandardMaterial>,
    wreck_primer: Handle<StandardMaterial>,
    wreck_dark: Handle<StandardMaterial>,
    scan_flat: Handle<StandardMaterial>,
    scan_high: Handle<StandardMaterial>,
}

/// One of the 15 sample points in the 5×3 terrain height scan grid.
#[derive(Component)]
struct ScanDot(usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<DrawnObstacles>()
        .add_systems(Startup, spawn)
        .add_systems(
            Update,
            (
                follow_sun_and_gravel,
                sync_path_obstacles,
                update_scan_dots,
                update_atmosphere,
            )
                .after(crate::lanes::step_lanes),
        );
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    ambient.color = Color::srgb(0.74, 0.79, 0.88);
    ambient.brightness = AMBIENT + WEBGL2_FILL_AMBIENT;

    // 1. Strong warm sun with cascaded soft shadows.
    commands.spawn((
        Sun,
        DirectionalLight {
            color: Color::srgb(1.0, 0.94, 0.84),
            illuminance: 17_500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: if cfg!(pk_webgl2) { 1 } else { 3 },
            minimum_distance: 0.5,
            maximum_distance: 95.0,
            first_cascade_far_bound: 18.0,
            overlap_proportion: 0.25,
        }
        .build(),
        Transform::from_translation(SUN_DIR * 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // The fills (2, 3) need more directional lights than WebGL2 has: there
    // they are folded into the ambient light.
    if !cfg!(pk_webgl2) {
        // 2. Cool sky hemisphere fill from opposite the sun.
        commands.spawn((
            SkyFill,
            DirectionalLight {
                color: Color::srgb(0.68, 0.78, 0.96),
                illuminance: 3_600.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(Vec3::new(-SUN_DIR.x, 0.55, -SUN_DIR.z))
                .looking_at(Vec3::ZERO, Vec3::Y),
        ));

        // 3. Warm gravel ground-bounce fill from below so under-chassis and inner-leg
        // pistons read clearly without crushing to black.
        commands.spawn((
            DirectionalLight {
                color: Color::srgb(0.62, 0.53, 0.42),
                illuminance: 1_350.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, -10.0, 0.0).looking_at(Vec3::ZERO, Vec3::X),
        ));
    }

    // 4. High-detail gravel lot texture with subtle 1 m / 10 m range grid.
    let tex = images.add(gravel_texture(512));
    let ground = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(tex.clone()),
        perceptual_roughness: 0.94,
        reflectance: 0.18,
        uv_transform: Affine2::from_scale(Vec2::splat(GROUND / 4.0)),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(GROUND, GROUND))),
        MeshMaterial3d(ground),
        NotShadowCaster,
        NoFrustumCulling,
    ));

    // 5. M6 Hydraulically & thermally eroded hillside rising behind the range (-Z)
    // and distant ridge (+Z), with channels, talus slumps, and alluvial sediment fans.
    let hill_tex = images.add(hillside_detail_texture(256));
    let hill_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(hill_tex),
        perceptual_roughness: 0.92,
        reflectance: 0.16,
        uv_transform: Affine2::from_scale(Vec2::splat(36.0)),
        ..default()
    });
    let hill_mesh = eroded_hillside(&hills()[0]);
    commands.spawn((
        Mesh3d(meshes.add(hill_mesh)),
        MeshMaterial3d(hill_mat.clone()),
        NotShadowCaster,
        NoFrustumCulling,
    ));
    let far_hill_mesh = eroded_hillside(&hills()[1]);
    commands.spawn((
        Mesh3d(meshes.add(far_hill_mesh)),
        MeshMaterial3d(hill_mat),
        NotShadowCaster,
        NoFrustumCulling,
    ));

    // 6. M6 Chipped convex rock meshes: settled along the hillside's alluvial
    // wash-out edge and scattered near the camera.
    let rock_meshes: Vec<Handle<Mesh>> = (0..ROCK_MESHES as u64)
        .map(|k| meshes.add(chipped_rock_mesh(100 + k * 37)))
        .collect();
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.88,
        reflectance: 0.22,
        ..default()
    });

    // Alluvial fan boulders & talus where erosion channels wash out onto the lot.
    for b in boulders() {
        commands.spawn((
            Mesh3d(rock_meshes[b.mesh % rock_meshes.len()].clone()),
            MeshMaterial3d(rock_mat.clone()),
            Transform {
                translation: b.position,
                rotation: b.rotation,
                scale: b.scale,
            },
        ));
    }
    let mut rng = Rng::new(405);

    // Real 3D chipped gravel stones near the camera (wrapping along X with focus).
    for _ in 0..240 {
        let offset_x = rng.range(-32.0, 32.0);
        // Keep the immediate spawn footprint clear of large rocks, scatter smaller
        // chipped gravel everywhere and larger stones toward the edges.
        let z = rng.range(-11.5, 16.0);
        let edge_boost = ((z.abs() - 1.8).max(0.0) / 8.0).clamp(0.0, 1.0);
        let s = rng.range(0.035, 0.085 + 0.14 * edge_boost);
        let scale = Vec3::new(
            s * rng.range(0.8, 1.4),
            s * rng.range(0.38, 0.72),
            s * rng.range(0.8, 1.4),
        );
        let rot = Quat::from_euler(
            EulerRot::YXZ,
            rng.range(0.0, TAU),
            rng.range(-0.25, 0.25),
            rng.range(-0.25, 0.25),
        );
        let m = rock_meshes[rng.below(rock_meshes.len())].clone();
        commands.spawn((
            NearStone {
                offset_x,
                z,
                scale,
                rot,
            },
            Mesh3d(m),
            MeshMaterial3d(rock_mat.clone()),
            Transform {
                translation: Vec3::new(offset_x, scale.y * 0.32, z),
                rotation: rot,
                scale,
            },
            NotShadowCaster,
        ));
    }

    // Distance markers along the walk, every 10 m.
    let marker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.52, 0.12),
        perceptual_roughness: 0.55,
        ..default()
    });
    let post = meshes.add(Cuboid::new(0.14, 0.58, 0.14));
    for i in -15..=18 {
        commands.spawn((
            Mesh3d(post.clone()),
            MeshMaterial3d(marker.clone()),
            Transform::from_xyz(i as f32 * 10.0, 0.29, -4.2),
        ));
    }

    // 7. M5 Endless Path obstacle assets & 5×3 Height Scan indicator dots.
    let scan_mesh = meshes.add(Sphere::new(0.048));
    let scan_flat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.3, 0.85, 0.95, 0.85),
        emissive: LinearRgba::rgb(0.6, 2.8, 3.4),
        unlit: true,
        ..default()
    });
    let scan_high = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.62, 0.15, 0.95),
        emissive: LinearRgba::rgb(6.5, 2.6, 0.4),
        unlit: true,
        ..default()
    });
    for i in 0..HEIGHT_SCAN {
        commands.spawn((
            ScanDot(i),
            Mesh3d(scan_mesh.clone()),
            MeshMaterial3d(scan_flat.clone()),
            Transform::from_xyz(0.0, -10.0, 0.0),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }

    commands.insert_resource(SceneAssets {
        rock_meshes,
        rock_mat,
        branch_mesh: meshes.add(Cylinder::new(1.0, 2.0)),
        limb_mesh: meshes.add(Cylinder::new(0.45, 1.2)),
        bark_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.31, 0.23, 0.16),
            perceptual_roughness: 0.92,
            reflectance: 0.12,
            ..default()
        }),
        wreck_box: meshes.add(Cuboid::new(2.0, 2.0, 2.0)),
        wreck_wheel: meshes.add(Cylinder::new(1.0, 1.0)),
        wreck_rust: materials.add(StandardMaterial {
            base_color: Color::srgb(0.44, 0.23, 0.15),
            metallic: 0.22,
            perceptual_roughness: 0.82,
            ..default()
        }),
        wreck_primer: materials.add(StandardMaterial {
            base_color: Color::srgb(0.46, 0.45, 0.41),
            metallic: 0.18,
            perceptual_roughness: 0.76,
            ..default()
        }),
        wreck_dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.13, 0.12),
            metallic: 0.25,
            perceptual_roughness: 0.85,
            ..default()
        }),
        scan_flat,
        scan_high,
    });
}

/// The sun's shadows and the near-camera 3D gravel follow the camera's focus.
#[allow(clippy::type_complexity)]
fn follow_sun_and_gravel(
    lanes: Res<Lanes>,
    mode: Res<Mode>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<&mut Transform, (With<Sun>, Without<NearStone>, Without<SkyFill>)>,
    mut fill: Query<&mut Transform, (With<SkyFill>, Without<NearStone>, Without<Sun>)>,
    mut stones: Query<(&NearStone, &mut Transform), (Without<Sun>, Without<SkyFill>)>,
) {
    let c = lanes.centre();
    let focus = Vec3::new(c.x, 0.0, c.z);
    let level = *mode == Mode::Level;
    let dir = if level { LEVEL_SUN_DIR } else { SUN_DIR };
    for mut t in &mut sun {
        *t = Transform::from_translation(focus + dir * 100.0).looking_at(focus, Vec3::Y);
    }
    for mut t in &mut fill {
        *t = Transform::from_translation(Vec3::new(-dir.x, 0.55, -dir.z))
            .looking_at(Vec3::ZERO, Vec3::Y);
    }
    let brightness = (if level { LEVEL_AMBIENT } else { AMBIENT }) + WEBGL2_FILL_AMBIENT;
    if ambient.brightness != brightness {
        ambient.brightness = brightness;
    }
    let sel_terrain = lanes.lanes.get(lanes.selected).map(|l| &l.env.terrain);
    let span = 64.0;
    for (s, mut tf) in &mut stones {
        let rel = (s.offset_x - c.x + 0.5 * span).rem_euclid(span) - 0.5 * span;
        let wx = c.x + rel;
        let gy = sel_terrain.map_or(0.0, |t| t.base_height(wx, s.z));
        tf.translation = Vec3::new(wx, gy + s.scale.y * 0.32, s.z);
        tf.rotation = s.rot;
    }
}

/// Spawns and despawns detailed visual meshes for streamed obstacles on the
/// endless dirt path (boulders, fallen tree branches, and old car wrecks).
fn sync_path_obstacles(
    mut commands: Commands,
    lanes: Res<Lanes>,
    assets: Res<SceneAssets>,
    mut drawn: ResMut<DrawnObstacles>,
) {
    let Some(lane) = lanes.lanes.get(lanes.selected) else {
        return;
    };
    let obstacles = &lane.env.terrain.obstacles;
    drawn.by_id.retain(|&id, &mut entity| {
        if obstacles.iter().any(|o| o.id == id) {
            true
        } else {
            commands.entity(entity).despawn();
            false
        }
    });

    for o in obstacles {
        if drawn.by_id.contains_key(&o.id) {
            continue;
        }
        let [hx, hy, hz] = o.half;
        let root = commands
            .spawn((
                PathObstacleView,
                Transform {
                    translation: Vec3::from(o.position),
                    rotation: Quat::from_rotation_y(o.yaw),
                    scale: Vec3::ONE,
                },
                Visibility::default(),
            ))
            .id();
        let mut r = Rng::new(o.id ^ 0x9E37_79B9);
        match o.kind {
            ObstacleKind::Boulder => {
                // Main boulder slab plus 2 smaller companion stones settled beside it.
                let vy = hy * 2.1;
                let m0 = assets.rock_meshes[r.below(assets.rock_meshes.len())].clone();
                commands.spawn((
                    Mesh3d(m0),
                    MeshMaterial3d(assets.rock_mat.clone()),
                    Transform::from_xyz(0.0, 0.55 * vy, 0.0).with_scale(Vec3::new(
                        hx * 1.25,
                        vy,
                        hz * 1.25,
                    )),
                    ChildOf(root),
                ));
                for side in [-1.0f32, 1.0] {
                    let m = assets.rock_meshes[r.below(assets.rock_meshes.len())].clone();
                    commands.spawn((
                        Mesh3d(m),
                        MeshMaterial3d(assets.rock_mat.clone()),
                        Transform::from_xyz(side * hx * 0.72, 0.35 * vy, side * hz * 0.48)
                            .with_rotation(Quat::from_rotation_y(r.range(0.0, TAU)))
                            .with_scale(Vec3::new(hx * 0.6, vy * 0.6, hz * 0.6)),
                        ChildOf(root),
                    ));
                }
            }
            ObstacleKind::Branch => {
                // Fallen log lying along local Z (cylinder Y rotated onto Z) + broken limbs.
                let vy = hy * 1.35;
                commands.spawn((
                    Mesh3d(assets.branch_mesh.clone()),
                    MeshMaterial3d(assets.bark_mat.clone()),
                    Transform::from_xyz(0.0, 0.72 * vy, 0.0)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                        .with_scale(Vec3::new(hx * 1.25, hz, vy)),
                    ChildOf(root),
                ));
                for s in [-0.45f32, 0.38] {
                    commands.spawn((
                        Mesh3d(assets.limb_mesh.clone()),
                        MeshMaterial3d(assets.bark_mat.clone()),
                        Transform::from_xyz(s * hx * 2.0, 0.9 * vy, s * hz * 0.7)
                            .with_rotation(Quat::from_euler(EulerRot::XYZ, 1.15, s * 0.7, s * 0.5))
                            .with_scale(Vec3::new(hx * 0.85, hz * 0.38, vy * 0.85)),
                        ChildOf(root),
                    ));
                }
            }
            ObstacleKind::Wreck => {
                // Crushed old car wreck: rusted lower chassis, crumpled hood, caved-in roof,
                // and sunken wheel hubs.
                let vy = hy * 2.8;
                let body_mat = if (o.id & 1) == 0 {
                    assets.wreck_rust.clone()
                } else {
                    assets.wreck_primer.clone()
                };
                // Lower chassis hulk
                commands.spawn((
                    Mesh3d(assets.wreck_box.clone()),
                    MeshMaterial3d(body_mat.clone()),
                    Transform::from_xyz(0.0, 0.45 * vy, 0.0).with_scale(Vec3::new(
                        hx,
                        0.48 * vy,
                        hz,
                    )),
                    ChildOf(root),
                ));
                // Crumpled cabin / roof tilted at an angle
                commands.spawn((
                    Mesh3d(assets.wreck_box.clone()),
                    MeshMaterial3d(assets.wreck_primer.clone()),
                    Transform::from_xyz(-0.12 * hx, 0.92 * vy, 0.0)
                        .with_rotation(Quat::from_rotation_z(-0.14))
                        .with_scale(Vec3::new(0.52 * hx, 0.38 * vy, 0.82 * hz)),
                    ChildOf(root),
                ));
                // Engine block / crumpled hood
                commands.spawn((
                    Mesh3d(assets.wreck_box.clone()),
                    MeshMaterial3d(assets.wreck_dark.clone()),
                    Transform::from_xyz(0.55 * hx, 0.52 * vy, 0.0)
                        .with_rotation(Quat::from_rotation_z(0.18))
                        .with_scale(Vec3::new(0.38 * hx, 0.32 * vy, 0.72 * hz)),
                    ChildOf(root),
                ));
                // Four rusted wheels half-buried in the dirt
                for sx in [-0.58f32, 0.58] {
                    for sz in [-0.92f32, 0.92] {
                        commands.spawn((
                            Mesh3d(assets.wreck_wheel.clone()),
                            MeshMaterial3d(assets.wreck_dark.clone()),
                            Transform::from_xyz(sx * hx, 0.32 * vy, sz * hz)
                                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                                .with_scale(Vec3::new(0.26 * hx, 0.16 * hz, 0.48 * vy)),
                            ChildOf(root),
                        ));
                    }
                }
            }
        }
        drawn.by_id.insert(o.id, root);
    }
}

/// Updates the 15 height-scan sample indicators ahead of the selected mech.
fn update_scan_dots(
    mode: Res<Mode>,
    inspector: Res<Inspector>,
    lanes: Res<Lanes>,
    assets: Res<SceneAssets>,
    mut dots: Query<(
        &ScanDot,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let show = *mode == Mode::Path || inspector.0;
    let Some(lane) = lanes.lanes.get(lanes.selected) else {
        return;
    };
    if !show {
        for (_, _, mut vis, _) in &mut dots {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let pts = lane.env.scan_points();
    let heights = lane.env.height_scan();
    for (dot, mut tf, mut vis, mut mat) in &mut dots {
        *vis = Visibility::Visible;
        let p = pts[dot.0];
        let h = heights[dot.0];
        tf.translation = Vec3::new(p[0], p[1] + 0.04, p[2]);
        let s = 1.0 + (h.max(0.0) * 4.5).min(1.4);
        tf.scale = Vec3::splat(s);
        mat.0 = if h > 0.025 {
            assets.scan_high.clone()
        } else {
            assets.scan_flat.clone()
        };
    }
}

/// Updates sky and distance fog with `lanes.dust_storm`.
fn update_atmosphere(
    lanes: Res<Lanes>,
    mut clear: ResMut<ClearColor>,
    mut cam: Query<&mut DistanceFog, With<MainCamera>>,
) {
    let d = lanes.dust_storm.clamp(0.0, 1.0);
    let sky = Color::srgb(0.68 + 0.14 * d, 0.73 - 0.09 * d, 0.79 - 0.28 * d);
    clear.0 = sky;
    for mut fog in &mut cam {
        fog.color = sky;
        fog.falloff = FogFalloff::Linear {
            start: 62.0 - 42.0 * d,
            end: 195.0 - 130.0 * d,
        };
    }
}

/// A hillside's heights on a regular grid: `nx` across `width_x` m of X
/// (centred on 0), `nz` rows from its toe at `start_z` out `depth_z` m
/// toward `dir_z` (±1); row-major from the toe. Shared by its mesh and, in
/// the level, its collider (`level::Level::attach`).
pub struct Hill {
    pub nx: usize,
    pub nz: usize,
    pub width_x: f32,
    pub depth_z: f32,
    pub start_z: f32,
    pub dir_z: f32,
    pub h: Vec<f32>,
    sediment: Vec<f32>,
    wear: Vec<f32>,
}

/// The two hillsides: behind the lot (+Z) and the far ridge (-Z). Eroded
/// once, on first use.
pub fn hills() -> &'static [Hill; 2] {
    static HILLS: std::sync::OnceLock<[Hill; 2]> = std::sync::OnceLock::new();
    HILLS.get_or_init(|| {
        [
            Hill::erode(156, 76, 420.0, 150.0, 13.5, 1.0, 19),
            Hill::erode(96, 48, 420.0, 120.0, -38.0, -1.0, 73),
        ]
    })
}

impl Hill {
    /// Layered ridge noise followed by a droplet-based hydraulic erosion and
    /// a thermal talus slumping pass (M6).
    fn erode(
        nx: usize,
        nz: usize,
        width_x: f32,
        depth_z: f32,
        start_z: f32,
        dir_z: f32,
        seed: u64,
    ) -> Self {
        let mut h = vec![0.0f32; nx * nz];
        let mut sediment = vec![0.0f32; nx * nz];
        let mut wear = vec![0.0f32; nx * nz];
        let s = seed as f32 * 1.37;

        // 1. Initial tectonic & layered noise elevation profile.
        for iz in 0..nz {
            let v = iz as f32 / (nz - 1) as f32;
            // Smooth S-ramp from 0 at the gravel lot edge (v=0) up to the crest.
            let ramp = v * v * (3.0 - 2.0 * v);
            for ix in 0..nx {
                let u = ix as f32 / (nx - 1) as f32 - 0.5;
                let wx = u * width_x;
                let wz = v * depth_z;
                // Multi-octave ridge + valley structure
                let n1 = (0.022 * wx + 0.018 * wz + s).sin() * (0.015 * wx - 0.024 * wz - s).cos();
                let ridge1 = 1.0 - (0.038 * wx + 0.029 * wz + 1.7 + s).sin().abs();
                let ridge2 = 1.0 - (0.075 * wx - 0.052 * wz + 0.8).sin().abs();
                let detail = (0.16 * wx + 0.14 * wz).sin() * (0.19 * wx - 0.12 * wz).cos();
                let elev = (42.0 * ramp)
                    + (14.0 * n1 + 11.0 * ridge1 * ridge1 + 4.5 * ridge2 + 1.2 * detail)
                        * v.powf(0.65);
                h[iz * nx + ix] = elev.max(0.0);
            }
        }

        // 2. Hydraulic droplet erosion: rain droplets flow downslope with inertia,
        // carving gullies/channels on steep slopes and depositing alluvial sediment
        // fans as they reach the flat toe (v -> 0).
        let mut rng = Rng::new(seed ^ 0xE205_1040);
        let droplets = (nx * nz) * 7 / 8;
        for _ in 0..droplets {
            let mut px = rng.range(2.0, (nx - 3) as f32);
            let mut pz = rng.range(0.18 * nz as f32, (nz - 3) as f32);
            let mut dx = 0.0f32;
            let mut dz = 0.0f32;
            let mut speed = 1.0f32;
            let mut water = 1.0f32;
            let mut carried = 0.0f32;

            for _step in 0..42 {
                let ix = px as usize;
                let iz = pz as usize;
                if ix < 1 || ix >= nx - 2 || iz < 1 || iz >= nz - 2 {
                    break;
                }
                let idx = iz * nx + ix;
                let gx = 0.5 * (h[idx + 1] - h[idx - 1]);
                let gz = 0.5 * (h[idx + nx] - h[idx - nx]);
                dx = dx * 0.22 - gx * 0.78;
                dz = dz * 0.22 - gz * 0.78;
                let len = (dx * dx + dz * dz).sqrt();
                if len < 1e-4 {
                    break;
                }
                dx /= len;
                dz /= len;
                let npx = px + dx;
                let npz = pz + dz;
                let nix = (npx as usize).clamp(1, nx - 2);
                let niz = (npz as usize).clamp(1, nz - 2);
                let dh = h[niz * nx + nix] - h[idx];
                let slope = (-dh).max(0.01);
                // Near the toe (iz < nz / 5), flatten capacity so sediment fans deposit!
                let toe_factor = ((iz as f32 / (0.22 * nz as f32)) - 0.15).clamp(0.0, 1.0);
                let capacity = (slope * speed * water * 4.5 * toe_factor).max(0.0);
                if carried > capacity || dh > 0.0 {
                    let drop = if dh > 0.0 {
                        dh.min(carried)
                    } else {
                        (carried - capacity) * 0.38
                    };
                    carried -= drop;
                    h[idx] += drop;
                    sediment[idx] += drop;
                } else {
                    let erode = ((capacity - carried) * 0.28).min(-dh * 0.9);
                    carried += erode;
                    h[idx] -= erode;
                    wear[idx] += erode;
                }
                speed = (speed * speed + (-dh).max(0.0) * 3.5).sqrt().min(4.0);
                water *= 0.96;
                px = npx;
                pz = npz;
            }
        }

        // 3. Thermal erosion (talus slumping where slope exceeds angle of repose)
        // and pinning the very front edge (iz == 0) to y = 0 so it meets the lot cleanly.
        let talus = 0.85;
        for _pass in 0..3 {
            for iz in 1..(nz - 1) {
                for ix in 1..(nx - 1) {
                    let idx = iz * nx + ix;
                    let hc = h[idx];
                    for nidx in [idx - 1, idx + 1, idx - nx, idx + nx] {
                        let diff = hc - h[nidx];
                        if diff > talus {
                            let move_h = 0.18 * (diff - talus);
                            h[idx] -= move_h;
                            h[nidx] += move_h;
                            sediment[nidx] += move_h * 0.5;
                        }
                    }
                }
            }
        }
        for ix in 0..nx {
            h[ix] = 0.0;
            h[nx + ix] *= 0.35;
        }

        Self {
            nx,
            nz,
            width_x,
            depth_z,
            start_z,
            dir_z,
            h,
            sediment,
            wear,
        }
    }

    /// The ground's height at (x, z): the grid's, between its points; 0
    /// off the hill.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let u = (x / self.width_x + 0.5) * (self.nx - 1) as f32;
        let v = (z - self.start_z) * self.dir_z / self.depth_z * (self.nz - 1) as f32;
        if !(0.0..=(self.nx - 1) as f32).contains(&u) || !(0.0..=(self.nz - 1) as f32).contains(&v)
        {
            return 0.0;
        }
        let (i, j) = ((u as usize).min(self.nx - 2), (v as usize).min(self.nz - 2));
        let (fu, fv) = (u - i as f32, v - j as f32);
        let at = |i: usize, j: usize| self.h[j * self.nx + i];
        let a = at(i, j) + (at(i + 1, j) - at(i, j)) * fu;
        let b = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu;
        a + (b - a) * fv
    }

    /// The alluvial wash-out sites along the toe, where sediment deposited:
    /// (x, z, strength).
    fn fans(&self) -> Vec<(f32, f32, f32)> {
        let Hill {
            nx,
            width_x,
            start_z,
            dir_z,
            ..
        } = *self;
        let mut fans = Vec::new();
        for ix in (3..nx - 3).step_by(2) {
            let mut fan_sed = 0.0f32;
            for iz in 1..14 {
                fan_sed += self.sediment[iz * nx + ix];
            }
            if fan_sed > 0.012 {
                let u = ix as f32 / (nx - 1) as f32 - 0.5;
                let wx = u * width_x;
                let wz = start_z - dir_z * 1.8;
                fans.push((wx, wz, (fan_sed * 9.0).clamp(0.25, 1.4)));
            }
        }
        fans
    }
}

/// The chipped rock meshes the boulders and gravel pick from.
const ROCK_MESHES: usize = 6;

/// A boulder washed out onto the lot at one of the near hill's fans: drawn
/// in every mode, solid in the level (`level::Level::attach`).
pub struct Boulder {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    mesh: usize,
}

/// The fans' boulders and talus. Placed once, on first use.
pub fn boulders() -> &'static [Boulder] {
    static BOULDERS: std::sync::OnceLock<Vec<Boulder>> = std::sync::OnceLock::new();
    BOULDERS.get_or_init(|| {
        let mut rng = Rng::new(404);
        let mut out = Vec::new();
        for (fx, fz, fan_strength) in hills()[0].fans() {
            let count = (3.0 + 5.0 * fan_strength) as usize;
            for _ in 0..count {
                let rx = fx + rng.range(-3.6, 3.6);
                let rz = fz + rng.range(-2.2, 3.2);
                let s = rng.range(0.18, 0.58) * (0.6 + 0.55 * fan_strength);
                let scale = Vec3::new(
                    s * rng.range(0.8, 1.35),
                    s * rng.range(0.45, 0.85),
                    s * rng.range(0.8, 1.35),
                );
                let rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    rng.range(0.0, TAU),
                    rng.range(-0.2, 0.2),
                    rng.range(-0.2, 0.2),
                );
                out.push(Boulder {
                    position: Vec3::new(rx, scale.y * 0.36, rz),
                    rotation,
                    scale,
                    mesh: rng.below(ROCK_MESHES),
                });
            }
        }
        out
    })
}

/// An eroded hillside's mesh (`nx × nz` grid).
fn eroded_hillside(hill: &Hill) -> Mesh {
    let Hill {
        nx,
        nz,
        width_x,
        depth_z,
        start_z,
        dir_z,
        ref h,
        ref sediment,
        ref wear,
    } = *hill;
    // 4. Build vertex positions, normals, UVs, and geomorphic vertex colors.
    let cell_x = width_x / (nx - 1) as f32;
    let cell_z = depth_z / (nz - 1) as f32;
    let mut pos = Vec::with_capacity(nx * nz);
    let mut nor = Vec::with_capacity(nx * nz);
    let mut uv = Vec::with_capacity(nx * nz);
    let mut col = Vec::with_capacity(nx * nz);

    let bedrock = Color::srgb(0.25, 0.26, 0.28).to_linear();
    let soil = Color::srgb(0.45, 0.39, 0.32).to_linear();
    let wash = Color::srgb(0.51, 0.47, 0.42).to_linear();
    let ridge_sun = Color::srgb(0.56, 0.48, 0.38).to_linear();

    for iz in 0..nz {
        let v = iz as f32 / (nz - 1) as f32;
        let wz = start_z + dir_z * (v * depth_z);
        for ix in 0..nx {
            let u = ix as f32 / (nx - 1) as f32 - 0.5;
            let wx = u * width_x;
            let wy = h[iz * nx + ix];
            pos.push([wx, wy, wz]);
            uv.push([u * 12.0, v * 6.0]);

            let ix0 = ix.saturating_sub(1);
            let ix1 = (ix + 1).min(nx - 1);
            let iz0 = iz.saturating_sub(1);
            let iz1 = (iz + 1).min(nz - 1);
            let dhdx = (h[iz * nx + ix1] - h[iz * nx + ix0]) / ((ix1 - ix0).max(1) as f32 * cell_x);
            let dhdz = (h[iz1 * nx + ix] - h[iz0 * nx + ix])
                / ((iz1 - iz0).max(1) as f32 * cell_z * dir_z);
            let n = Vec3::new(-dhdx, 1.0, -dhdz).normalize_or_zero();
            nor.push([n.x, n.y, n.z]);

            let steep = (1.0 - n.y).clamp(0.0, 1.0);
            let w_wear = (wear[iz * nx + ix] * 1.4).clamp(0.0, 1.0);
            let w_sed = (sediment[iz * nx + ix] * 1.8 + (1.0 - v * 4.5).max(0.0)).clamp(0.0, 1.0);
            let rock_t = (steep * 2.4 + w_wear * 0.65).clamp(0.0, 1.0);

            let mut c = [
                soil.red + (bedrock.red - soil.red) * rock_t,
                soil.green + (bedrock.green - soil.green) * rock_t,
                soil.blue + (bedrock.blue - soil.blue) * rock_t,
                1.0,
            ];
            let crest = (wy / 42.0).clamp(0.0, 1.0) * (1.0 - rock_t);
            c[0] += (ridge_sun.red - c[0]) * crest * 0.45;
            c[1] += (ridge_sun.green - c[1]) * crest * 0.45;
            c[2] += (ridge_sun.blue - c[2]) * crest * 0.45;

            c[0] += (wash.red - c[0]) * w_sed;
            c[1] += (wash.green - c[1]) * w_sed;
            c[2] += (wash.blue - c[2]) * w_sed;
            col.push(c);
        }
    }

    let mut idx = Vec::with_capacity((nx - 1) * (nz - 1) * 6);
    for iz in 0..(nz - 1) {
        for ix in 0..(nx - 1) {
            let a = (iz * nx + ix) as u32;
            let b = a + 1;
            let c = a + nx as u32;
            let d = c + 1;
            if dir_z < 0.0 {
                idx.extend_from_slice(&[a, b, c, b, d, c]);
            } else {
                idx.extend_from_slice(&[a, c, b, b, c, d]);
            }
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

/// Builds a noise-displaced, planar-chipped convex stone mesh with flat facet
/// normals and natural mineral color variation (M6).
fn chipped_rock_mesh(seed: u64) -> Mesh {
    let mut rng = Rng::new(seed);
    let phi = 0.5 * (1.0 + 5.0f32.sqrt());
    let base_verts = [
        Vec3::new(-1.0, phi, 0.0),
        Vec3::new(1.0, phi, 0.0),
        Vec3::new(-1.0, -phi, 0.0),
        Vec3::new(1.0, -phi, 0.0),
        Vec3::new(0.0, -1.0, phi),
        Vec3::new(0.0, 1.0, phi),
        Vec3::new(0.0, -1.0, -phi),
        Vec3::new(0.0, 1.0, -phi),
        Vec3::new(phi, 0.0, -1.0),
        Vec3::new(phi, 0.0, 1.0),
        Vec3::new(-phi, 0.0, -1.0),
        Vec3::new(-phi, 0.0, 1.0),
    ]
    .map(|v| v.normalize());

    let faces: [[usize; 3]; 20] = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    // 5 random cleavage/chipping planes to create sharp stone facets.
    let planes: Vec<(Vec3, f32)> = (0..5)
        .map(|_| {
            let n = Vec3::new(rng.signed(), rng.signed(), rng.signed()).normalize_or(Vec3::Y);
            (n, rng.range(0.62, 0.86))
        })
        .collect();

    let displace = |v: Vec3, r: &mut Rng| -> Vec3 {
        let mut p = v * r.range(0.84, 1.16);
        for &(n, d) in &planes {
            let proj = p.dot(n);
            if proj > d {
                p -= n * (proj - d) * 0.78;
            }
        }
        p
    };

    let verts: Vec<Vec3> = base_verts.iter().map(|&v| displace(v, &mut rng)).collect();
    let mut pos = Vec::with_capacity(60);
    let mut nor = Vec::with_capacity(60);
    let mut col = Vec::with_capacity(60);
    let mut idx = Vec::with_capacity(60);

    let tint = rng.range(-0.04, 0.04);
    let base_c = Color::srgb(0.44 + tint, 0.41 + tint * 0.8, 0.37 + tint * 0.5).to_linear();

    for [ia, ib, ic] in faces {
        let (a, b, c) = (verts[ia], verts[ib], verts[ic]);
        let n = (b - a).cross(c - a).normalize_or(Vec3::Y);
        let shade = 0.88 + 0.24 * rng.f32();
        let vc = [
            base_c.red * shade,
            base_c.green * shade,
            base_c.blue * shade,
            1.0,
        ];
        let base = pos.len() as u32;
        pos.extend_from_slice(&[a.to_array(), b.to_array(), c.to_array()]);
        nor.extend_from_slice(&[n.to_array(); 3]);
        col.extend_from_slice(&[vc; 3]);
        idx.extend_from_slice(&[base, base + 1, base + 2]);
    }

    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    m
}

/// High-frequency mineral & striation detail texture for the eroded hillside.
pub(crate) fn hillside_detail_texture(n: usize) -> Image {
    let mut rng = Rng::new(31);
    let mut px = vec![0u8; n * n * 4];
    for (i, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (i % n, i / n);
        let strata = ((y as f32 * 0.22 + (x as f32 * 0.09).sin() * 2.5).sin()) * 14.0;
        let grain = rng.range(-16.0, 16.0);
        let v = (218.0 + strata + grain).clamp(150.0, 255.0) as u8;
        p[0] = v;
        p[1] = v;
        p[2] = v;
        p[3] = 255;
    }
    let mut img = Image::new(
        Extent3d {
            width: n as u32,
            height: n as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

/// A 4 m tile of gravel: grey-brown stones of several sizes, subtle wheel/foot
/// ruts, and the faint 1 m grid over it.
fn gravel_texture(n: usize) -> Image {
    let mut rng = Rng::new(7);
    let mut px = vec![0u8; n * n * 4];
    let base = [104.0, 96.0, 86.0];
    for (i, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (i % n, i / n);
        let grain = rng.range(-15.0, 15.0);
        let grid = if x % (n / 4) == 0 || y % (n / 4) == 0 {
            -18.0
        } else {
            0.0
        };
        for c in 0..3 {
            p[c] = (base[c] + grain + grid).clamp(0.0, 255.0) as u8;
        }
        p[3] = 255;
    }
    // Stones: small and medium pebbles with highlight and shadow rims.
    for _ in 0..(n * n / 32) {
        let (cx, cy) = (rng.below(n) as i32, rng.below(n) as i32);
        let r = rng.range(0.9, 3.2);
        let shade = rng.range(-38.0, 34.0);
        let tint = rng.range(-10.0, 10.0);
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if ((dx * dx + dy * dy) as f32) > r * r {
                    continue;
                }
                let x = (cx + dx).rem_euclid(n as i32) as usize;
                let y = (cy + dy).rem_euclid(n as i32) as usize;
                let p = &mut px[(y * n + x) * 4..][..3];
                let edge = 1.0 - ((dx * dx + dy * dy) as f32).sqrt() / r;
                let sun_bias = (-dx - dy) as f32 / (2.0 * r) * 10.0;
                for (c, v) in p.iter_mut().enumerate() {
                    let t = if c == 0 { tint } else { -tint * 0.5 };
                    *v = (*v as f32 + (shade + t + sun_bias) * (0.35 + 0.65 * edge))
                        .clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: n as u32,
            height: n as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}
