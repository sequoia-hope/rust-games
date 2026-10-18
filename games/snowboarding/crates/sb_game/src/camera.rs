//! The camera: chasing the rider from behind and above, lagging a little
//! on turns, pulling back in the air and widening with speed; orbiting for
//! the menu's showcase, the countdown and the results.

use crate::world::{Built, apply_fog, look};
use bevy::camera::{PerspectiveProjection, Projection};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::cluster::{ClusterConfig, GlobalClusterSettings};
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use sb_sim::course::CourseId;
use sb_sim::noise::wrap_angle;

pub fn plugin(app: &mut App) {
    app.init_resource::<CamFocus>()
        .init_resource::<CamState>()
        .add_systems(Startup, (spawn_camera, no_light_clustering))
        .add_systems(PostUpdate, drive_camera.before(bevy::transform::TransformSystems::Propagate));
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CamMode {
    #[default]
    Chase,
    /// Circling the focus.
    Orbit { radius: f32, height: f32, rate: f32 },
    /// The countdown's swing from in front of the rider round behind
    /// (0 .. 1 done).
    Intro(f32),
}

/// What the camera follows, written each frame by the screen in charge.
#[derive(Resource, Clone, Debug, Default)]
pub struct CamFocus {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub airborne: bool,
    pub crashed: bool,
    pub mode: CamMode,
    /// Snap to the new place this frame (a new run).
    pub cut: bool,
    /// The focus's s on the course, where the camera's search for the
    /// snow under it starts.
    pub s: f32,
    /// In the half-pipe: look straight down it rather than along the
    /// rider's wall-to-wall path.
    pub pipe: bool,
}

#[derive(Resource, Default)]
pub struct CamState {
    yaw: f32,
    pos: Vec3,
    orbit: f32,
    init: bool,
}

#[derive(Component)]
pub struct MainCamera;

/// No light clustering (midnight-playground's DECISIONS D861). Bevy
/// clusters point and spot lights and decals, of which the game has none
/// (the sun is a directional light). Where there is compute (WebGPU) it
/// did so on the GPU every frame all the same: three passes, a readback,
/// and its storage buffers made afresh, about 500 KB of new GPU buffers a
/// frame (measured 2026-10-18, `tools/courses.mjs`). With `gpu_clustering`
/// off the CPU path runs instead, and the camera's `ClusterConfig::None`
/// leaves it nothing to do. (Headless there is no renderer to set.)
fn no_light_clustering(settings: Option<ResMut<GlobalClusterSettings>>) {
    if let Some(mut s) = settings {
        s.gpu_clustering = None;
    }
}

fn spawn_camera(mut commands: Commands) {
    let mut fog = DistanceFog::default();
    apply_fog(&mut fog, &look(CourseId::Crystal));
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        // Nothing to cluster: the sun is the only light (no_light_clustering).
        ClusterConfig::None,
        Msaa::Sample4,
        Tonemapping::None,
        fog,
        Projection::Perspective(PerspectiveProjection {
            fov: 64f32.to_radians(),
            near: 0.1,
            far: 12000.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn dir(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, -yaw.cos())
}

fn drive_camera(
    time: Res<Time>,
    mut focus: ResMut<CamFocus>,
    mut st: ResMut<CamState>,
    built: Res<Built>,
    mut cam: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let Ok((mut t, mut proj)) = cam.single_mut() else {
        return;
    };
    let dt = time.delta_secs().min(0.1);
    let f = focus.clone();
    let cut = f.cut || !st.init;
    focus.cut = false;
    st.init = true;
    let speed = f.vel.length();
    let ground = |p: Vec3| -> f32 {
        built
            .course
            .as_ref()
            .map_or(f32::MIN, |c| c.ground(p, f.s).height)
    };
    let (desired, look_at, fov) = match f.mode {
        CamMode::Chase => {
            let hv = Vec3::new(f.vel.x, 0.0, f.vel.z);
            let target_yaw = if f.pipe {
                f.yaw
            } else if hv.length() > 1.5 && !f.crashed {
                hv.x.atan2(-hv.z)
            } else {
                st.yaw
            };
            if cut {
                st.yaw = f.yaw;
            }
            let k = 1.0 - (-3.0 * dt).exp();
            st.yaw = wrap_angle(st.yaw + wrap_angle(target_yaw - st.yaw) * k);
            let d = dir(st.yaw);
            let dist = 3.7 + speed * 0.04 + if f.airborne { 1.0 } else { 0.0 };
            let height = 1.6 + if f.airborne { 0.7 } else { 0.0 };
            let desired = f.pos + Vec3::Y * height - d * dist;
            (desired, f.pos + Vec3::Y * 0.9 + d * 3.0, 56.0 + (speed * 0.28).min(16.0))
        }
        CamMode::Orbit { radius, height, rate } => {
            st.orbit += rate * dt;
            let a = st.orbit;
            let desired = f.pos + Vec3::new(a.cos() * radius, height, a.sin() * radius);
            (desired, f.pos + Vec3::Y * 0.9, 50.0)
        }
        CamMode::Intro(k) => {
            st.yaw = f.yaw;
            let e = k * k * (3.0 - 2.0 * k);
            let a = f.yaw + std::f32::consts::PI * (1.0 - e) + 0.6 * (1.0 - e);
            let d = dir(a);
            let dist = 4.0 + 2.5 * e;
            let desired = f.pos + Vec3::Y * (1.3 + 0.9 * e) - d * dist;
            (desired, f.pos + Vec3::Y * 0.9 + dir(f.yaw) * (4.0 * e), 55.0 + 7.0 * e)
        }
    };
    let mut desired = desired;
    desired.y = desired.y.max(ground(desired) + 1.2);
    st.pos = if cut || st.pos.distance(desired) > 60.0 {
        desired
    } else {
        let k = 1.0 - (-10.0 * dt).exp();
        st.pos.lerp(desired, k)
    };
    t.translation = st.pos;
    t.look_at(look_at, Vec3::Y);
    if let Projection::Perspective(p) = proj.as_mut() {
        let want = fov.to_radians();
        p.fov += (want - p.fov) * (1.0 - (-4.0 * dt).exp());
    }
}
