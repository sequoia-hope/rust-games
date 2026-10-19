//! Every way to play into one [`Controls`]: the keyboard, gamepads (Bevy's
//! natively; the browser's Gamepad API on the web, read directly as
//! midnight-playground does) and touch (a stick on the left half of the
//! screen, JUMP and GRAB on the right).
//!
//! | | keyboard | pad | touch |
//! |---|---|---|---|
//! | steer / spin | ← → or A D | left stick, d-pad | drag left half |
//! | tuck, brake / flip | ↑ ↓ or W S | | |
//! | jump (hold to charge) | Space | A (south) | JUMP |
//! | grab | Shift, K | X or B, triggers | GRAB |
//! | pause | Esc, P | Start | II |

use bevy::input::InputSystems;
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub fn plugin(app: &mut App) {
    app.init_resource::<Controls>()
        .init_resource::<PadMemory>()
        .add_systems(PreUpdate, gather.after(InputSystems));
}

#[derive(Resource, Default, Clone, Debug)]
pub struct Controls {
    pub steer: f32,
    pub pitch: f32,
    pub jump: bool,
    pub grab: bool,
    // Edges, for the menus.
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub confirm: bool,
    pub back: bool,
    pub pause: bool,
    /// Touch has been used: show the touch controls.
    pub touch_seen: bool,
    /// The touch stick, for drawing: anchor and knob in window pixels.
    pub stick: Option<(Vec2, Vec2)>,
    pub touch_jump: bool,
    pub touch_grab: bool,
    /// Any key, button or touch this frame (wakes the sound on the web).
    pub any: bool,
}

#[derive(Resource, Default)]
struct PadMemory {
    prev: PadState,
}

#[derive(Default, Clone, Copy)]
struct PadState {
    x: f32,
    y: f32,
    a: bool,
    b: bool,
    x_btn: bool,
    start: bool,
    grab: bool,
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

fn dead(v: f32) -> f32 {
    if v.abs() < 0.18 {
        0.0
    } else {
        (v - 0.18 * v.signum()) / 0.82
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_pads(pads: &Query<&Gamepad>) -> PadState {
    let mut s = PadState::default();
    for g in pads {
        let st = g.left_stick();
        if st.length() > s.x.hypot(s.y) {
            s.x = st.x;
            s.y = st.y;
        }
        s.a |= g.pressed(GamepadButton::South);
        s.b |= g.pressed(GamepadButton::East);
        s.x_btn |= g.pressed(GamepadButton::West);
        s.start |= g.pressed(GamepadButton::Start);
        s.grab |= g.pressed(GamepadButton::West)
            || g.pressed(GamepadButton::East)
            || g.pressed(GamepadButton::LeftTrigger2)
            || g.pressed(GamepadButton::RightTrigger2)
            || g.pressed(GamepadButton::RightTrigger);
        s.up |= g.pressed(GamepadButton::DPadUp);
        s.down |= g.pressed(GamepadButton::DPadDown);
        s.left |= g.pressed(GamepadButton::DPadLeft);
        s.right |= g.pressed(GamepadButton::DPadRight);
    }
    s
}

/// The browser's pads, standard mapping: buttons 0 A, 1 B, 2 X, 3 Y,
/// 4-7 bumpers and triggers, 9 Start, 12-15 the d-pad; axes 0, 1 the left
/// stick (y down).
#[cfg(target_arch = "wasm32")]
fn read_web_pads() -> PadState {
    use wasm_bindgen::JsCast;
    let mut s = PadState::default();
    let Some(w) = web_sys::window() else { return s };
    let Ok(list) = w.navigator().get_gamepads() else {
        return s;
    };
    for v in list.iter() {
        let Ok(g) = v.dyn_into::<web_sys::Gamepad>() else {
            continue;
        };
        let axes = g.axes();
        let ax = |i: u32| axes.get(i).as_f64().unwrap_or(0.0) as f32;
        let (x, y) = (ax(0), -ax(1));
        if x.hypot(y) > s.x.hypot(s.y) {
            s.x = x;
            s.y = y;
        }
        let buttons = g.buttons();
        let b = |i: u32| {
            buttons
                .get(i)
                .dyn_into::<web_sys::GamepadButton>()
                .is_ok_and(|b| b.pressed())
        };
        s.a |= b(0);
        s.b |= b(1);
        s.x_btn |= b(2);
        s.start |= b(9);
        s.grab |= b(1) || b(2) || b(5) || b(6) || b(7);
        s.up |= b(12);
        s.down |= b(13);
        s.left |= b(14);
        s.right |= b(15);
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn gather(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut c: ResMut<Controls>,
    mut mem: ResMut<PadMemory>,
    mut setup: ResMut<crate::Setup>,
    opts: Res<crate::Opts>,
    #[cfg(not(target_arch = "wasm32"))] pads: Query<&Gamepad>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let pad = read_pads(&pads);
    #[cfg(target_arch = "wasm32")]
    let pad = read_web_pads();
    let prev = mem.prev;
    mem.prev = pad;

    let k = |codes: &[KeyCode]| codes.iter().any(|c| keys.pressed(*c));
    let kj = |codes: &[KeyCode]| codes.iter().any(|c| keys.just_pressed(*c));
    let mut steer = 0.0;
    if k(&[KeyCode::ArrowLeft, KeyCode::KeyA]) {
        steer -= 1.0;
    }
    if k(&[KeyCode::ArrowRight, KeyCode::KeyD]) {
        steer += 1.0;
    }
    let mut pitch = 0.0;
    if k(&[KeyCode::ArrowUp, KeyCode::KeyW]) {
        pitch += 1.0;
    }
    if k(&[KeyCode::ArrowDown, KeyCode::KeyS]) {
        pitch -= 1.0;
    }
    let (px, py) = (dead(pad.x), dead(pad.y));
    let dx = (pad.right as i32 - pad.left as i32) as f32;
    let dy = (pad.up as i32 - pad.down as i32) as f32;
    steer = (steer + px + dx).clamp(-1.0, 1.0);
    pitch = (pitch + py + dy).clamp(-1.0, 1.0);
    let mut jump = k(&[KeyCode::Space, KeyCode::KeyJ]) || pad.a;
    let mut grab = k(&[
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::KeyK,
        KeyCode::KeyL,
    ]) || pad.grab;

    // Touch.
    let forced_touch = opts.0.touch == Some(true);
    if forced_touch {
        c.touch_seen = true;
    }
    let (w, h) = window
        .single()
        .map_or((1.0, 1.0), |w| (w.width(), w.height()));
    c.stick = None;
    c.touch_jump = false;
    c.touch_grab = false;
    let mut any_touch = false;
    for t in touches.iter() {
        any_touch = true;
        let start = t.start_position();
        let p = t.position();
        if start.y < h * 0.16 {
            continue; // the top strip: the pause button and the HUD
        }
        if start.x < w * 0.5 {
            let r = (w.min(h) * 0.12).max(40.0);
            let d = (p - start) / r;
            let d = if d.length() > 1.0 { d.normalize() } else { d };
            steer = (steer + d.x).clamp(-1.0, 1.0);
            pitch = (pitch - d.y).clamp(-1.0, 1.0);
            c.stick = Some((start, start + d * r));
        } else if p.x > w * 0.74 {
            jump = true;
            c.touch_jump = true;
        } else {
            grab = true;
            c.touch_grab = true;
        }
    }
    if any_touch {
        c.touch_seen = true;
    }
    if (any_touch || forced_touch) && !setup.assist_chosen && !setup.assist {
        setup.assist = true;
    }
    c.steer = steer;
    c.pitch = pitch;
    c.jump = jump;
    c.grab = grab;

    let edge = |now: bool, was: bool| now && !was;
    c.up = kj(&[KeyCode::ArrowUp, KeyCode::KeyW])
        || edge(pad.up || pad.y > 0.6, prev.up || prev.y > 0.6);
    c.down = kj(&[KeyCode::ArrowDown, KeyCode::KeyS])
        || edge(pad.down || pad.y < -0.6, prev.down || prev.y < -0.6);
    c.left = kj(&[KeyCode::ArrowLeft, KeyCode::KeyA])
        || edge(pad.left || pad.x < -0.6, prev.left || prev.x < -0.6);
    c.right = kj(&[KeyCode::ArrowRight, KeyCode::KeyD])
        || edge(pad.right || pad.x > 0.6, prev.right || prev.x > 0.6);
    c.confirm = kj(&[KeyCode::Enter, KeyCode::Space, KeyCode::NumpadEnter]) || edge(pad.a, prev.a);
    c.back = kj(&[KeyCode::Escape, KeyCode::Backspace]) || edge(pad.b, prev.b);
    c.pause = kj(&[KeyCode::Escape, KeyCode::KeyP]) || edge(pad.start, prev.start);
    c.any = keys.get_just_pressed().next().is_some()
        || mouse.get_just_pressed().next().is_some()
        || touches.any_just_pressed()
        || edge(pad.a, prev.a)
        || edge(pad.start, prev.start);
}
