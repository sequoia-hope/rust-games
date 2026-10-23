//! The ground and the light: a bare gravel lot like a test map, one strong
//! sun, a hazy sky. (The procedural hillside and real gravel are M6.)

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{CascadeShadowConfigBuilder, GlobalAmbientLight, NotShadowCaster};
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use mw_sim::rng::Rng;

pub const SKY: Color = Color::srgb(0.70, 0.74, 0.78);
pub const SUN_DIR: Vec3 = Vec3::new(0.45, 0.8, 0.35);

/// The ground's side, m.
pub const GROUND: f32 = 400.0;

#[derive(Component)]
pub struct Sun;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn)
        .add_systems(Update, follow_sun);
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    ambient.color = Color::srgb(0.75, 0.8, 0.9);
    ambient.brightness = 350.0;

    commands.spawn((
        Sun,
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.86),
            illuminance: 22_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: if cfg!(pk_webgl2) { 1 } else { 3 },
            minimum_distance: 0.5,
            maximum_distance: 90.0,
            first_cascade_far_bound: 18.0,
            overlap_proportion: 0.25,
        }
        .build(),
        Transform::from_translation(SUN_DIR * 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Gravel: noise in a tiling texture, a metre to a tile, with a faint
    // line every 10 m for scale.
    let tex = images.add(gravel_texture(256));
    let ground = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(tex),
        perceptual_roughness: 0.95,
        reflectance: 0.2,
        uv_transform: Affine2::from_scale(Vec2::splat(GROUND / 4.0)),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(GROUND, GROUND))),
        MeshMaterial3d(ground),
        NotShadowCaster,
        NoFrustumCulling,
    ));
    // Distance markers along the walk, every 10 m.
    let marker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.55, 0.12),
        perceptual_roughness: 0.6,
        ..default()
    });
    let post = meshes.add(Cuboid::new(0.15, 0.6, 0.15));
    for i in -15..=15 {
        commands.spawn((
            Mesh3d(post.clone()),
            MeshMaterial3d(marker.clone()),
            Transform::from_xyz(i as f32 * 10.0, 0.3, -4.0),
        ));
    }
}

/// The sun's shadows follow the camera's focus.
fn follow_sun(lanes: Res<crate::lanes::Lanes>, mut sun: Query<&mut Transform, With<Sun>>) {
    let c = lanes.centre();
    let focus = Vec3::new(c.x, 0.0, c.z);
    for mut t in &mut sun {
        *t = Transform::from_translation(focus + SUN_DIR * 100.0).looking_at(focus, Vec3::Y);
    }
}

/// A 4 m tile of gravel: grey-brown stones of a few sizes, and the 1 m grid
/// faintly over it.
fn gravel_texture(n: usize) -> Image {
    let mut rng = Rng::new(7);
    let mut px = vec![0u8; n * n * 4];
    let base = [118.0, 110.0, 99.0];
    for (i, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (i % n, i / n);
        let grain = rng.range(-14.0, 14.0);
        let grid = if x % (n / 4) == 0 || y % (n / 4) == 0 {
            -22.0
        } else {
            0.0
        };
        for c in 0..3 {
            p[c] = (base[c] + grain + grid).clamp(0.0, 255.0) as u8;
        }
        p[3] = 255;
    }
    // Stones: small blobs lighter or darker than the bed.
    for _ in 0..(n * n / 40) {
        let (cx, cy) = (rng.below(n) as i32, rng.below(n) as i32);
        let r = rng.range(0.8, 2.6);
        let shade = rng.range(-35.0, 30.0);
        let tint = rng.range(-8.0, 8.0);
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
                for (c, v) in p.iter_mut().enumerate() {
                    let t = if c == 0 { tint } else { -tint * 0.5 };
                    *v = (*v as f32 + (shade + t) * (0.4 + 0.6 * edge)).clamp(0.0, 255.0) as u8;
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
