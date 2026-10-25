//! An orbit camera around the lanes: drag to turn, wheel or pinch to zoom.
//! It follows the lanes' centre (or the selected lane, with one lane).

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::input::touch::Touches;
use bevy::light::cluster::{ClusterConfig, GlobalClusterSettings};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::lanes::Lanes;

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Debug)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    focus: Vec3,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: -0.9,
            pitch: 0.22,
            distance: 14.0,
            focus: Vec3::new(0.0, 1.8, 0.0),
        }
    }
}

impl Orbit {
    /// `camera {yaw?, pitch?, distance?}`.
    pub fn apply(&mut self, v: &serde_json::Value) {
        let num = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_f64)
                .map(|x| x as f32)
        };
        self.yaw = num("yaw").unwrap_or(self.yaw);
        self.pitch = num("pitch").unwrap_or(self.pitch);
        self.distance = num("distance").unwrap_or(self.distance);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Orbit>()
        .add_systems(Startup, (spawn, no_light_clustering))
        .add_systems(
            Update,
            drive
                .after(crate::lanes::step_lanes)
                .run_if(|mode: Res<crate::Mode>| *mode != crate::Mode::Level),
        );
}

/// No light clustering (as Snowboarding and Midnight Playground): the sun
/// is the only light, and on WebGPU Bevy otherwise clusters on the GPU every
/// frame with fresh storage buffers.
fn no_light_clustering(settings: Option<ResMut<GlobalClusterSettings>>) {
    if let Some(mut s) = settings {
        s.gpu_clustering = None;
    }
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        ClusterConfig::None,
        Msaa::Sample4,
        Tonemapping::AcesFitted,
        DistanceFog {
            color: crate::scene::SKY,
            directional_light_color: Color::srgba(1.0, 0.92, 0.8, 0.3),
            directional_light_exponent: 20.0,
            falloff: FogFalloff::Linear {
                start: 60.0,
                end: 190.0,
            },
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 50f32.to_radians(),
            near: 0.1,
            far: 500.0,
            ..default()
        }),
        Transform::from_xyz(10.0, 5.0, 10.0).looking_at(Vec3::Y * 1.8, Vec3::Y),
    ));
}

#[allow(clippy::too_many_arguments)] // A Bevy system: each argument is a resource it reads.
fn drive(
    time: Res<Time>,
    lanes: Res<Lanes>,
    mut orbit: ResMut<Orbit>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    if buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right) {
        orbit.yaw -= motion.delta.x * 0.005;
        orbit.pitch = (orbit.pitch + motion.delta.y * 0.004).clamp(-0.05, 1.3);
    }
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    let mut zoom = (-lines * 0.1).exp();
    // One finger turns, two pinch.
    let active: Vec<_> = touches.iter().collect();
    if active.len() == 1 {
        let d = active[0].delta();
        orbit.yaw -= d.x * 0.006;
        orbit.pitch = (orbit.pitch + d.y * 0.005).clamp(-0.05, 1.3);
    } else if active.len() == 2 {
        let now = active[0].position().distance(active[1].position());
        let before = (active[0].position() - active[0].delta())
            .distance(active[1].position() - active[1].delta());
        if now > 1.0 && before > 1.0 {
            zoom *= before / now;
        }
    }
    let spread = (lanes.lanes.len().saturating_sub(1)) as f32 * crate::lanes::LANE_GAP;
    orbit.distance = (orbit.distance * zoom).clamp(5.0, 60.0 + spread);

    let c = lanes.centre();
    let target = Vec3::new(c.x, 1.8, c.z);
    let k = 1.0 - (-time.delta_secs() * 4.0).exp();
    orbit.focus = orbit.focus.lerp(target, k);
    let dir = Vec3::new(
        orbit.yaw.cos() * orbit.pitch.cos(),
        orbit.pitch.sin(),
        orbit.yaw.sin() * orbit.pitch.cos(),
    );
    for mut t in &mut cam {
        *t = Transform::from_translation(orbit.focus + dir * orbit.distance)
            .looking_at(orbit.focus, Vec3::Y);
    }
}
