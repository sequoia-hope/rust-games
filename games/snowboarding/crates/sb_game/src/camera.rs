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
        .add_systems(
            PostUpdate,
            drive_camera.before(bevy::transform::TransformSystems::Propagate),
        );
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CamMode {
    #[default]
    Chase,
    /// Circling the focus.
    Orbit { radius: f32, height: f32, rate: f32 },
    /// The countdown's swing from in front of the rider round to a low
    /// over-the-shoulder gate shot (0 .. 1 done).
    Intro(f32),
    /// Low, close launch shot plunging down the start ramp right after `Go`
    /// (0 .. 1 done), handing off seamlessly into [`CamMode::Chase`].
    DropIn(f32),
}

/// What the camera follows, written each frame by the screen in charge.
#[derive(Resource, Clone, Debug, Default)]
pub struct CamFocus {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub airborne: bool,
    pub crashed: bool,
    /// 0 .. 1 aerodynamic tuck: lowers the chase camera and kicks FOV slightly.
    pub tuck: f32,
    /// 0 .. 1 edge carve / skid intensity on the snow.
    pub carve: f32,
    /// Landing or crash bump impulse added this frame.
    pub bump: f32,
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
    prev_focus: Vec3,
    orbit: f32,
    clock: f32,
    shake: f32,
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

/// Chase camera framing `(dist, height, look_ahead, fov_deg)` from speed and
/// posture: closes in, drops lower and widens FOV as speed and tuck build,
/// and pulls back in the air so spins and grabs read cleanly.
fn chase_framing(speed: f32, tuck: f32, airborne: bool) -> (f32, f32, f32, f32) {
    let fast = ((speed - 6.0) / 22.0).clamp(0.0, 1.0);
    let dist = 3.35 - 0.38 * fast - 0.22 * tuck + if airborne { 1.15 } else { 0.0 };
    let height = 1.42 - 0.30 * fast - 0.14 * tuck + if airborne { 0.72 } else { 0.0 };
    let ahead = 3.2 + 2.4 * fast;
    let s_norm = ((speed - 3.0) / 25.0).clamp(0.0, 1.15);
    let fov = (55.0 + 21.5 * s_norm.powf(1.12) + 2.5 * tuck).min(81.0);
    (dist, height, ahead, fov)
}

pub(crate) fn drive_camera(
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
    focus.bump = 0.0;
    st.init = true;
    st.clock += dt;
    st.shake = (st.shake + f.bump).min(1.0) * (-9.0 * dt).exp();

    let focus_delta = f.pos - st.prev_focus;
    st.prev_focus = f.pos;
    if !cut && focus_delta.length_squared() < 25.0 * 25.0 {
        st.pos += focus_delta;
    }

    let speed = f.vel.length();
    let fast = ((speed - 6.0) / 22.0).clamp(0.0, 1.0);
    let ground = |p: Vec3| -> f32 {
        built
            .course
            .as_ref()
            .map_or(f32::MIN, |c| c.ground(p, f.s).height)
    };
    let hv = Vec3::new(f.vel.x, 0.0, f.vel.z);
    let target_yaw = if f.pipe {
        f.yaw
    } else if hv.length() > 1.5 && !f.crashed {
        hv.x.atan2(-hv.z)
    } else {
        st.yaw
    };
    let (desired, look_at, fov) = match f.mode {
        CamMode::Chase => {
            if cut {
                st.yaw = f.yaw;
            }
            let k = 1.0 - (-3.6 * dt).exp();
            st.yaw = wrap_angle(st.yaw + wrap_angle(target_yaw - st.yaw) * k);
            let d = dir(st.yaw);
            let (dist, height, ahead, fov) = chase_framing(speed, f.tuck, f.airborne);
            let desired = f.pos + Vec3::Y * height - d * dist;
            (
                desired,
                f.pos + Vec3::Y * (0.85 - 0.12 * fast) + d * ahead,
                fov,
            )
        }
        CamMode::Orbit {
            radius,
            height,
            rate,
        } => {
            st.orbit += rate * dt;
            let a = st.orbit;
            let desired = f.pos + Vec3::new(a.cos() * radius, height, a.sin() * radius);
            (desired, f.pos + Vec3::Y * 0.9, 50.0)
        }
        CamMode::Intro(k) => {
            st.yaw = f.yaw;
            let e = k * k * (3.0 - 2.0 * k);
            // Swings from in front of the gate round to a tight, low
            // over-the-shoulder shot peering down the start ramp.
            let a = f.yaw + (std::f32::consts::PI + 0.55) * (1.0 - e) + 0.14 * e;
            let d = dir(a);
            let dist = 4.2 - 2.15 * e;
            let height = 1.45 - 0.37 * e;
            let desired = f.pos + Vec3::Y * height - d * dist;
            (
                desired,
                f.pos + Vec3::Y * (0.95 - 0.12 * e) + dir(f.yaw) * (5.2 * e),
                52.0 + 4.0 * e,
            )
        }
        CamMode::DropIn(k) => {
            if cut {
                st.yaw = f.yaw;
            }
            let ky = 1.0 - (-4.5 * dt).exp();
            st.yaw = wrap_angle(st.yaw + wrap_angle(target_yaw - st.yaw) * ky);
            let e = k * k * (3.0 - 2.0 * k);
            let arc = (k * std::f32::consts::PI).sin();
            let a = st.yaw + 0.14 * (1.0 - e);
            let d = dir(a);
            let (c_dist, c_height, c_ahead, c_fov) = chase_framing(speed, f.tuck, f.airborne);
            let dist = 2.05 + (c_dist - 2.05) * e + 0.35 * arc;
            let height = 1.08 + (c_height - 1.08) * e - 0.14 * arc;
            let look_y = 0.83 + ((0.85 - 0.12 * fast) - 0.83) * e;
            let ahead = 5.2 + (c_ahead - 5.2) * e;
            let fov = 56.0 + (c_fov - 56.0) * e + 3.5 * arc;
            (
                f.pos + Vec3::Y * height - d * dist,
                f.pos + Vec3::Y * look_y + dir(st.yaw) * ahead,
                fov,
            )
        }
    };
    let mut desired = desired;
    let clearance = if matches!(f.mode, CamMode::Orbit { .. }) {
        1.1
    } else {
        0.62
    };
    desired.y = desired.y.max(ground(desired) + clearance);
    st.pos = if cut || st.pos.distance(desired) > 60.0 {
        desired
    } else {
        let k = 1.0 - (-9.5 * dt).exp();
        st.pos.lerp(desired, k)
    };

    // Subtle high-speed snow chatter and landing thump, applied after st.pos
    // smoothing so the filter state stays drift-free.
    let jitter = if matches!(f.mode, CamMode::Chase | CamMode::DropIn(_)) {
        let on_snow = !f.airborne && !f.crashed;
        let chatter = if on_snow {
            ((speed - 13.0) / 18.0).clamp(0.0, 1.0) * (1.0 + 0.55 * f.carve)
        } else {
            0.0
        };
        let c = st.clock;
        let vx = ((c * 23.0).cos() * 0.009 + (c * 37.0).sin() * 0.005) * chatter
            + (c * 19.0).sin() * st.shake * 0.05;
        let vy = ((c * 31.0).sin() * 0.011 + (c * 47.0).cos() * 0.007) * chatter
            - (c * 26.0).cos() * st.shake * 0.085;
        let right = Vec3::new(st.yaw.cos(), 0.0, st.yaw.sin());
        right * vx + Vec3::Y * vy
    } else {
        Vec3::ZERO
    };
    t.translation = st.pos + jitter;
    t.look_at(look_at + jitter * 0.35, Vec3::Y);
    if let Projection::Perspective(p) = proj.as_mut() {
        let want = fov.to_radians();
        p.fov += (want - p.fov) * (1.0 - (-6.5 * dt).exp());
    }
}
