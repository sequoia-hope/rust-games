//! The pilot's controls in the level (`Mode::Level`): walking on the keys,
//! looking with the mouse, and the view from the cockpit (first person, the
//! default) or from behind the mech (third person, V).
//!
//! The keys set the policy's walk command while held: W/S forward and back
//! (Shift faster), A/D turn; E or Space works the blade (`level`), R starts
//! the level over, P pauses. The mouse looks while a button is held, or
//! freely once the pointer is locked (a click on the game: the page asks
//! for the lock on the web, the client grabs the cursor natively; Esc lets
//! go). On a touch screen one finger looks, two pinch the third-person
//! view, and the page's Walk sliders and buttons walk and shove. A gamepad
//! does all of it (`gamepad` has the layout): the left stick walks and
//! turns, the right stick looks.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::input::touch::Touches;
use bevy::prelude::*;
use mw_sim::env::Command;

use crate::Mode;
use crate::camera::MainCamera;
use crate::gamepad::{PadFrame, button};
use crate::lanes::{Lanes, apply as apply_command};
use crate::view::LinkView;

/// Walk speeds the keys ask for, m/s, and the turn rate, rad/s (the
/// policy's command range).
const WALK: f32 = 1.0;
const RUN: f32 = 2.0;
const BACK: f32 = -0.3;
const TURN: f32 = 0.8;

/// How fast the right stick looks, rad/s at full tilt.
const PAD_LOOK: Vec2 = Vec2::new(2.4, 1.6);

/// The cockpit, in the torso's frame: forward of its middle and above its
/// top plate, m.
const COCKPIT: Vec3 = Vec3::new(0.45, 1.3, 0.0);

#[derive(Resource, Debug)]
pub struct PlayerView {
    pub first_person: bool,
    /// Where the pilot looks, from the mech's heading, rad.
    pub look_yaw: f32,
    pub look_pitch: f32,
    /// The third-person camera's distance, m.
    pub distance: f32,
    /// The pointer is locked to the game: the mouse looks without a button.
    pub pointer_locked: bool,
    /// The mech's heading as the camera follows it (smoothed), rad.
    heading: Option<f32>,
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            first_person: true,
            look_yaw: 0.0,
            look_pitch: -0.12,
            distance: 11.0,
            pointer_locked: false,
            heading: None,
        }
    }
}

/// `view {first?}` (toggles without `first`), `pointer {locked}`.
pub fn apply(view: &mut PlayerView, v: &serde_json::Value) {
    match v.get("cmd").and_then(|c| c.as_str()) {
        Some("view") => {
            view.first_person = v
                .get("first")
                .and_then(|b| b.as_bool())
                .unwrap_or(!view.first_person);
            view.look_yaw = 0.0;
            view.look_pitch = if view.first_person { -0.12 } else { 0.0 };
        }
        Some("pointer") => {
            view.pointer_locked = v.get("locked").and_then(|b| b.as_bool()).unwrap_or(false);
        }
        _ => {}
    }
}

fn in_level(mode: Res<Mode>) -> bool {
    *mode == Mode::Level
}

pub fn plugin(app: &mut App) {
    app.init_resource::<PlayerView>().add_systems(
        Update,
        (
            (keys, pad).chain().before(crate::lanes::step_lanes),
            (look, follow, hide_own_hull)
                .chain()
                .after(crate::lanes::step_lanes),
        )
            .run_if(in_level),
    );
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, grab_cursor.run_if(in_level));
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut lanes: ResMut<Lanes>, mut view: ResMut<PlayerView>) {
    use KeyCode::*;
    let walk_keys = [
        KeyW, KeyS, KeyA, KeyD, ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    ];
    let held = |k: &[KeyCode]| keys.any_pressed(k.iter().copied());
    // The keys own the command only while they are in use, so the page's
    // sliders (the touch screen's way to walk) still work.
    if keys.any_pressed(walk_keys) || keys.any_just_released(walk_keys) {
        let fast = held(&[ShiftLeft, ShiftRight]);
        let mut c = Command::default();
        if held(&[KeyW, ArrowUp]) {
            c.forward = if fast { RUN } else { WALK };
        } else if held(&[KeyS, ArrowDown]) {
            c.forward = BACK;
        }
        if held(&[KeyA, ArrowLeft]) {
            c.yaw += TURN;
        }
        if held(&[KeyD, ArrowRight]) {
            c.yaw -= TURN;
        }
        if c != lanes.command {
            let v = serde_json::json!({"cmd": "walk", "forward": c.forward, "yaw": c.yaw});
            apply_command(&mut lanes, &v);
        }
    }
    // Held, the blade works as often as it can.
    if held(&[KeyE, KeyF, Space])
        && let Some(level) = &mut lanes.level
    {
        level.want_shove = true;
    }
    if keys.just_pressed(KeyV) {
        let first = !view.first_person;
        apply(
            &mut view,
            &serde_json::json!({"cmd": "view", "first": first}),
        );
    }
    if keys.just_pressed(KeyR) {
        lanes.reset_all();
    }
    if keys.just_pressed(KeyP) {
        lanes.paused = !lanes.paused;
    }
}

/// The walk command from a pad's left stick (or d-pad): up walks, faster
/// with the right trigger or the stick clicked, down backs, sideways turns.
pub fn pad_command(p: &PadFrame) -> Command {
    let mut s = p.left;
    if p.held(button::UP) {
        s.y = 1.0;
    } else if p.held(button::DOWN) {
        s.y = -1.0;
    }
    if p.held(button::LEFT) {
        s.x = -1.0;
    } else if p.held(button::RIGHT) {
        s.x = 1.0;
    }
    let fast = if p.held(button::L3) { 1.0 } else { p.throttle };
    Command {
        forward: if s.y > 0.0 {
            s.y * (WALK + (RUN - WALK) * fast)
        } else {
            -s.y * BACK
        },
        yaw: -s.x * TURN,
    }
}

/// The pad's buttons and walking (the right stick looks, in `look`).
fn pad(
    p: Res<PadFrame>,
    mut lanes: ResMut<Lanes>,
    mut view: ResMut<PlayerView>,
    mut walking: Local<bool>,
) {
    // Like the keys, the stick owns the command only while in use, and once
    // more as it comes back to rest (to stop the mech).
    let c = pad_command(&p);
    let now = c != Command::default();
    if (now || *walking) && c != lanes.command {
        let v = serde_json::json!({"cmd": "walk", "forward": c.forward, "yaw": c.yaw});
        apply_command(&mut lanes, &v);
    }
    *walking = now;
    if p.held(button::SOUTH | button::RB)
        && let Some(level) = &mut lanes.level
    {
        level.want_shove = true;
    }
    if p.just_pressed(button::NORTH) {
        let first = !view.first_person;
        apply(
            &mut view,
            &serde_json::json!({"cmd": "view", "first": first}),
        );
    }
    if p.just_pressed(button::R3) {
        view.look_yaw = 0.0;
        view.look_pitch = if view.first_person { -0.12 } else { 0.0 };
    }
    if p.just_pressed(button::SELECT) {
        lanes.reset_all();
    }
    if p.just_pressed(button::START) {
        lanes.paused = !lanes.paused;
    }
}

fn look(
    time: Res<Time>,
    pad: Res<PadFrame>,
    mut view: ResMut<PlayerView>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
) {
    let mut d = Vec2::ZERO;
    if view.pointer_locked
        || buttons.pressed(MouseButton::Left)
        || buttons.pressed(MouseButton::Right)
    {
        d += motion.delta * 0.0025;
    }
    // The right stick, up to look up (the mouse's y is down).
    d += Vec2::new(pad.right.x, -pad.right.y) * PAD_LOOK * time.delta_secs();
    let active: Vec<_> = touches.iter().collect();
    let mut zoom = match scroll.unit {
        MouseScrollUnit::Line => (-scroll.delta.y * 0.1).exp(),
        MouseScrollUnit::Pixel => (-scroll.delta.y / 400.0).exp(),
    };
    if active.len() == 1 {
        d += active[0].delta() * 0.004;
    } else if active.len() == 2 {
        let now = active[0].position().distance(active[1].position());
        let before = (active[0].position() - active[0].delta())
            .distance(active[1].position() - active[1].delta());
        if now > 1.0 && before > 1.0 {
            zoom *= before / now;
        }
    }
    view.look_yaw -= d.x;
    if view.first_person {
        view.look_yaw = view.look_yaw.clamp(-2.6, 2.6);
        view.look_pitch = (view.look_pitch - d.y).clamp(-1.2, 0.9);
    } else {
        view.look_yaw = (view.look_yaw + PI).rem_euclid(TAU) - PI;
        view.look_pitch = (view.look_pitch + d.y).clamp(-0.35, 1.1);
        view.distance = (view.distance * zoom).clamp(5.0, 30.0);
    }
}

/// The camera: in the cockpit, or behind and above the mech, turning with
/// its heading.
fn follow(
    time: Res<Time>,
    lanes: Res<Lanes>,
    mut view: ResMut<PlayerView>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(lane) = lanes.lanes.first() else {
        return;
    };
    let (Some(a), Some(b)) = (lane.prev.first(), lane.cur.first()) else {
        return;
    };
    let t = lanes.blend;
    let pos = Vec3::from(a.position).lerp(Vec3::from(b.position), t);
    let rot = Quat::from_array(a.rotation).slerp(Quat::from_array(b.rotation), t);
    let f = rot * Vec3::X;
    let target = (-f.z).atan2(f.x);
    // Follow the heading, not the sway of every step.
    let k = 1.0 - (-time.delta_secs() * 8.0).exp();
    let h = match view.heading {
        Some(h) => h + ((target - h + PI).rem_euclid(TAU) - PI) * k,
        None => target,
    };
    view.heading = Some(h);
    let yaw = Quat::from_rotation_y(h);
    let Ok(mut tf) = cam.single_mut() else { return };
    if view.first_person {
        let eye = pos + yaw * COCKPIT;
        *tf = Transform::from_translation(eye).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            h - FRAC_PI_2 + view.look_yaw,
            view.look_pitch,
            0.0,
        ));
    } else {
        let focus = pos + Vec3::Y * 0.8;
        let a = h + PI + view.look_yaw;
        let p = 0.28 + view.look_pitch;
        let dir = Vec3::new(a.cos() * p.cos(), p.sin(), -a.sin() * p.cos());
        let eye = focus + dir * clear_distance(focus, dir, view.distance);
        // Above the ground, the hills' toes included.
        let ground = crate::scene::hills()
            .iter()
            .map(|h| h.height(eye.x, eye.z))
            .fold(0.0, f32::max);
        *tf = Transform::from_translation(Vec3::new(eye.x, eye.y.max(ground + 0.6), eye.z))
            .looking_at(focus, Vec3::Y);
    }
}

/// How far back the third-person camera can sit along `dir` from `focus`
/// before its line of sight enters one of the level's walls (the pass's
/// spurs, the edges), so it never ends up inside a hill.
fn clear_distance(focus: Vec3, dir: Vec3, want: f32) -> f32 {
    let walls = crate::level::walls();
    let inside = |p: Vec3| {
        walls.iter().any(|w| {
            let margin = 0.6;
            (p.x - w.centre[0]).abs() < w.half[0] + margin
                && (p.y - w.centre[1]).abs() < w.half[1] + 3.0
                && (p.z - w.centre[2]).abs() < w.half[2] + margin
        })
    };
    let mut d = 0.0;
    while d < want {
        let next = (d + 0.25).min(want);
        if inside(focus + dir * next) {
            return d.max(1.5);
        }
        d = next;
    }
    want
}

/// From the cockpit, the mech's own hull would fill the view: it is not
/// drawn (the legs still are, looking down).
fn hide_own_hull(view: Res<PlayerView>, mut links: Query<(&LinkView, &mut Visibility)>) {
    for (l, mut v) in &mut links {
        if l.lane != 0 || l.link != 0 {
            continue;
        }
        let want = if view.first_person {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *v != want {
            *v = want;
        }
    }
}

/// Natively, a click on the game grabs the cursor for looking; Esc lets go.
#[cfg(not(target_arch = "wasm32"))]
fn grab_cursor(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<PlayerView>,
    mut cursor: Query<&mut bevy::window::CursorOptions>,
) {
    use bevy::window::CursorGrabMode;
    let Ok(mut c) = cursor.single_mut() else {
        return;
    };
    if buttons.just_pressed(MouseButton::Left) && !view.pointer_locked {
        c.grab_mode = CursorGrabMode::Locked;
        c.visible = false;
        view.pointer_locked = true;
    } else if keys.just_pressed(KeyCode::Escape) && view.pointer_locked {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
        view.pointer_locked = false;
    }
}
