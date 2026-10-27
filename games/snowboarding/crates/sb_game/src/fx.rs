//! Snow and wind in the air: the spray off a carving edge, bursts on
//! landings and crashes, falling snow on the stormy courses, and peripheral
//! air streaks at speed.
//!
//! The spray is one mesh of camera-facing quads rebuilt each frame while
//! any is in the air, and left alone while none is. The falling snow and the
//! speed streaks are static meshes moved in `snowfall.wgsl` and `wind.wgsl`:
//! rebuilt on the CPU each frame, Avalanche Bowl's 900 flakes were a fresh
//! 200 KB buffer a frame, and an iPhone killed the tab within seconds of the
//! course appearing (2026-10-18). `tests.rs` holds the bytes uploaded per
//! frame to a budget on every course.

use crate::camera::{CamFocus, CamMode, MainCamera};
use crate::ride::Run;
use crate::world::Built;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use sb_sim::noise::Rng;
use std::f32::consts::TAU;

pub fn plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "snowfall.wgsl");
    bevy::asset::embedded_asset!(app, "wind.wgsl");
    app.add_plugins((
        MaterialPlugin::<SnowfallMaterial>::default(),
        MaterialPlugin::<WindMaterial>::default(),
    ))
    .init_resource::<WindState>()
    .insert_resource(Particles {
        list: Vec::new(),
        rng: Rng::new(9),
        drawn: true,
    })
    .add_systems(Startup, spawn_mesh)
    .add_systems(Update, ensure_snowfall.after(crate::world::ensure_world))
    .add_systems(
        PostUpdate,
        drive_wind
            .after(crate::camera::drive_camera)
            .before(bevy::transform::TransformSystems::Propagate),
    )
    .add_systems(
        PostUpdate,
        (step, rebuild)
            .chain()
            .after(bevy::transform::TransformSystems::Propagate),
    );
}

struct Particle {
    pos: Vec3,
    vel: Vec3,
    life: f32,
    max_life: f32,
    size: f32,
}

#[derive(Resource)]
pub struct Particles {
    list: Vec<Particle>,
    rng: Rng,
    /// Whether the mesh holds any spray (so an empty list still clears it).
    drawn: bool,
}

const MAX: usize = 2500;

impl Particles {
    /// Snow thrown from `pos`, moving with `vel` and spread by `spread`.
    pub fn emit(&mut self, pos: Vec3, vel: Vec3, spread: f32, n: usize, size: f32) {
        for _ in 0..n {
            if self.list.len() >= MAX {
                self.list.swap_remove(0);
            }
            let r = &mut self.rng;
            let jitter =
                Vec3::new(r.range(-1.0, 1.0), r.range(0.2, 1.0), r.range(-1.0, 1.0)) * spread;
            let life = r.range(0.4, 1.1);
            self.list.push(Particle {
                pos: pos + Vec3::new(r.range(-0.2, 0.2), r.range(0.0, 0.15), r.range(-0.2, 0.2)),
                vel: vel + jitter,
                life,
                max_life: life,
                size: size * r.range(0.6, 1.4),
            });
        }
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }
}

#[derive(Component)]
struct SnowMesh(Handle<Mesh>);

/// The falling snow's material: `snowfall.wgsl` places every flake.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SnowfallMaterial {
    #[texture(0)]
    #[sampler(1)]
    puff: Handle<Image>,
}

impl Material for SnowfallMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://sb_game/snowfall.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://sb_game/snowfall.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// The falling snow on screen, for the course it was made for.
#[derive(Component)]
pub struct Snowfall;

/// Peripheral wind-sheared air streaks at high speed (`wind.wgsl`).
#[derive(Component)]
pub struct WindStreaks;

#[derive(Resource, Default)]
struct WindState {
    intensity: f32,
    phase: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct WindMaterial {
    #[texture(0)]
    #[sampler(1)]
    puff: Handle<Image>,
}

impl Material for WindMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://sb_game/wind.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://sb_game/wind.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// The flakes at full snowfall.
const FLAKES: usize = 900;
const STREAKS: usize = 112;

/// A soft white dot: alpha falling off from the middle.
fn puff(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const N: u32 = 32;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let dx = (x as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let r = (dx * dx + dy * dy).sqrt();
            let a = (1.0 - r).clamp(0.0, 1.0);
            data.extend([255, 255, 255, (a * a * (3.0 - 2.0 * a) * 255.0) as u8]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

#[derive(Resource)]
struct Puff(Handle<Image>);

fn spawn_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut wind_materials: ResMut<Assets<WindMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let puff = puff(&mut images);
    commands.insert_resource(Puff(puff.clone()));
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32, 1.0, 1.0, 0.0]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; 3]);
    m.insert_indices(Indices::U32(vec![0, 1, 2]));
    let h = meshes.add(m);
    commands.spawn((
        SnowMesh(h.clone()),
        Mesh3d(h),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(puff.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
    ));

    let mut rng = Rng::new(31);
    let mut pos = Vec::with_capacity(STREAKS * 4);
    let mut corner = Vec::with_capacity(STREAKS * 4);
    let mut motion = Vec::with_capacity(STREAKS * 4);
    let mut idx = Vec::with_capacity(STREAKS * 6);
    for _ in 0..STREAKS {
        let p = [rng.range(0.0, TAU), rng.f(), rng.f()];
        // Quantise speed to 1/16ths so wrapping `phase` mod 64 is seamless.
        let mo = [
            0.75 + (rng.next_u32() % 9) as f32 * 0.0625,
            rng.range(0.7, 1.35),
        ];
        let at = pos.len() as u32;
        for c in [[-1.0f32, -1.0f32], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            pos.push(p);
            corner.push(c);
            motion.push(mo);
        }
        idx.extend([at, at + 1, at + 2, at, at + 2, at + 3]);
    }
    let mut wm = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    wm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    wm.insert_attribute(Mesh::ATTRIBUTE_UV_0, corner);
    wm.insert_attribute(Mesh::ATTRIBUTE_UV_1, motion);
    wm.insert_indices(Indices::U32(idx));
    commands.spawn((
        WindStreaks,
        Mesh3d(meshes.add(wm)),
        MeshMaterial3d(wind_materials.add(WindMaterial { puff })),
        Transform::default(),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
    ));
}

fn drive_wind(
    time: Res<Time>,
    focus: Res<CamFocus>,
    run: Option<Res<Run>>,
    mut st: ResMut<WindState>,
    cam: Query<&Transform, (With<MainCamera>, Without<WindStreaks>)>,
    mut q: Query<(&mut Transform, &mut Visibility), With<WindStreaks>>,
) {
    let Ok((mut tr, mut vis)) = q.single_mut() else {
        return;
    };
    let paused = run.as_ref().is_some_and(|r| r.paused);
    let dt = if paused {
        0.0
    } else {
        time.delta_secs().min(0.1)
    };
    let active = matches!(focus.mode, CamMode::Chase | CamMode::DropIn(_)) && !focus.crashed;
    let speed = if active { focus.vel.length() } else { 0.0 };
    let target = if active {
        ((speed - 13.5) / 15.5).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if !paused {
        st.intensity += (target - st.intensity) * (1.0 - (-7.0 * dt).exp());
        st.phase = (st.phase + dt * speed / 18.5).rem_euclid(64.0);
    }
    if st.intensity < 0.01 {
        *vis = Visibility::Hidden;
        return;
    }
    let Ok(cam_tr) = cam.single() else {
        return;
    };
    *vis = Visibility::Inherited;
    let cam_fwd = cam_tr.forward().as_vec3();
    let v_dir = focus.vel.try_normalize().unwrap_or(cam_fwd);
    let flow = (v_dir * 0.65 + cam_fwd * 0.35)
        .try_normalize()
        .unwrap_or(cam_fwd);
    tr.translation = cam_tr.translation;
    tr.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, flow);
    tr.scale = Vec3::new(1.0 + st.intensity, 1.0 + st.phase, 1.0 + speed.min(45.0));
}

fn step(time: Res<Time>, mut p: ResMut<Particles>) {
    let dt = time.delta_secs().min(0.05);
    p.list.retain_mut(|q| {
        q.life -= dt;
        q.vel.y -= 4.5 * dt;
        q.vel *= 1.0 - (2.2 * dt).min(1.0);
        q.pos += q.vel * dt;
        q.life > 0.0
    });
}

/// The falling snow for the course on screen: made when the course
/// changes, `amount × FLAKES` flakes at random in the box round the eye,
/// each four corners of one quad with its sway phase and fall speed.
fn ensure_snowfall(
    mut commands: Commands,
    built: Res<Built>,
    old: Query<Entity, With<Snowfall>>,
    puff: Res<Puff>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SnowfallMaterial>>,
    mut made: Local<Option<f32>>,
) {
    let amount = built.look.map_or(0.0, |l| l.snowfall);
    if *made == Some(amount) {
        return;
    }
    *made = Some(amount);
    for e in &old {
        commands.entity(e).despawn();
    }
    let n = (amount * FLAKES as f32) as usize;
    if n == 0 {
        return;
    }
    let mut rng = Rng::new(17);
    let mut pos = Vec::with_capacity(n * 4);
    let mut corner = Vec::with_capacity(n * 4);
    let mut motion = Vec::with_capacity(n * 4);
    let mut idx = Vec::with_capacity(n * 6);
    for i in 0..n {
        let p = [
            rng.range(0.0, 60.0),
            rng.range(0.0, 30.0),
            rng.range(0.0, 60.0),
        ];
        let k = i as f32;
        let m = [k, 1.4 + (k * 0.37).sin() * 0.4];
        let at = pos.len() as u32;
        for c in [[-1.0f32, -1.0f32], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            pos.push(p);
            corner.push(c);
            motion.push(m);
        }
        idx.extend([at, at + 1, at + 2, at, at + 2, at + 3]);
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, corner);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_1, motion);
    m.insert_indices(Indices::U32(idx));
    commands.spawn((
        Snowfall,
        Mesh3d(meshes.add(m)),
        MeshMaterial3d(materials.add(SnowfallMaterial {
            puff: puff.0.clone(),
        })),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
    ));
}

fn rebuild(
    mut p: ResMut<Particles>,
    cam: Query<&GlobalTransform, With<MainCamera>>,
    q: Query<&SnowMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Nothing in the air and nothing drawn: the mesh is left alone, not
    // written (a write is an upload).
    if p.list.is_empty() && !p.drawn {
        return;
    }
    let (Ok(cam), Ok(sm)) = (cam.single(), q.single()) else {
        return;
    };
    p.drawn = !p.list.is_empty();
    let Some(mut mesh) = meshes.get_mut(&sm.0) else {
        return;
    };
    let right = cam.right().as_vec3();
    let up = cam.up().as_vec3();
    let back = cam.back().as_vec3();
    let n = p.list.len();
    let mut pos = Vec::with_capacity(n * 4 + 3);
    let mut col = Vec::with_capacity(n * 4 + 3);
    let mut uv = Vec::with_capacity(n * 4 + 3);
    let mut idx = Vec::with_capacity(n * 6 + 3);
    let eye = cam.translation();
    let mut quad = |c: Vec3, s: f32, a: f32| {
        // Fade out what is about to hit the lens.
        let a = a * ((c.distance(eye) - 0.8) / 2.5).clamp(0.0, 1.0);
        if a <= 0.01 {
            return;
        }
        let i = pos.len() as u32;
        for (x, y) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            pos.push(<[f32; 3]>::from(c + (right * x + up * y) * s));
            col.push([1.0, 1.0, 1.0, a]);
            uv.push([(x + 1.0) * 0.5, (1.0 - y) * 0.5]);
        }
        idx.extend([i, i + 1, i + 2, i, i + 2, i + 3]);
    };
    for q in &p.list {
        let k = q.life / q.max_life;
        let s = q.size * (2.2 - 1.0 * k);
        quad(q.pos, s, (k * 1.2).min(0.9));
    }
    if pos.is_empty() {
        for _ in 0..3 {
            pos.push([0.0; 3]);
            col.push([0.0; 4]);
            uv.push([0.0; 2]);
        }
        idx.extend([0, 1, 2]);
    }
    let normals = vec![<[f32; 3]>::from(back); pos.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh.insert_indices(Indices::U32(idx));
}
