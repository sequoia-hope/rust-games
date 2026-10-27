//! Gamepads in the level, read once a frame into one [`PadFrame`]: the
//! browser's Gamepad API on the web (`navigator.getGamepads()`, its
//! `standard` layout), gilrs through Bevy natively. Every connected pad
//! counts; the strongest stick and any pressed button win.
//!
//! The layout (`player::pad`): left stick walks (up and down) and turns
//! (left and right), as does the d-pad; the right trigger or a click of
//! the left stick walks faster; the right stick looks. A or RB works the
//! blade, Y changes the view, a click of the right stick looks ahead
//! again, Start pauses and Back/Select starts over.

use bevy::prelude::*;

/// The `standard` layout's buttons, as bits of [`PadFrame::buttons`].
pub mod button {
    pub const SOUTH: u32 = 1 << 0;
    pub const EAST: u32 = 1 << 1;
    pub const WEST: u32 = 1 << 2;
    pub const NORTH: u32 = 1 << 3;
    pub const LB: u32 = 1 << 4;
    pub const RB: u32 = 1 << 5;
    pub const LT: u32 = 1 << 6;
    pub const RT: u32 = 1 << 7;
    pub const SELECT: u32 = 1 << 8;
    pub const START: u32 = 1 << 9;
    pub const L3: u32 = 1 << 10;
    pub const R3: u32 = 1 << 11;
    pub const UP: u32 = 1 << 12;
    pub const DOWN: u32 = 1 << 13;
    pub const LEFT: u32 = 1 << 14;
    pub const RIGHT: u32 = 1 << 15;
}

/// The sticks' dead zone (of full travel), and how far a trigger must go
/// to count as pressed.
const DEAD: f32 = 0.18;
const TRIGGER: f32 = 0.3;

/// This frame's pads, merged.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct PadFrame {
    /// Any pad connected.
    pub connected: bool,
    /// The sticks, past the dead zone and rescaled to it: x right, y up,
    /// each -1..1.
    pub left: Vec2,
    pub right: Vec2,
    /// The right trigger, 0..1.
    pub throttle: f32,
    /// The buttons held now, and a frame ago.
    pub buttons: u32,
    pub before: u32,
}

impl PadFrame {
    pub fn held(&self, b: u32) -> bool {
        self.buttons & b != 0
    }

    pub fn just_pressed(&self, b: u32) -> bool {
        self.buttons & b != 0 && self.before & b == 0
    }

    /// Something on a pad is in use (a stick out of its dead zone, a
    /// trigger, a button).
    pub fn active(&self) -> bool {
        self.left != Vec2::ZERO
            || self.right != Vec2::ZERO
            || self.throttle > 0.0
            || self.buttons != 0
    }

    /// Takes in one pad's raw reading (y up): the stronger stick wins.
    fn merge(&mut self, left: Vec2, right: Vec2, throttle: f32, buttons: u32) {
        self.connected = true;
        let (left, right) = (dead_zone(left), dead_zone(right));
        if left.length() > self.left.length() {
            self.left = left;
        }
        if right.length() > self.right.length() {
            self.right = right;
        }
        self.throttle = self.throttle.max(throttle.clamp(0.0, 1.0));
        self.buttons |= buttons;
        if throttle > TRIGGER {
            self.buttons |= button::RT;
        }
    }
}

/// A radial dead zone, the rest of the travel rescaled to 0..1.
pub fn dead_zone(v: Vec2) -> Vec2 {
    let l = v.length();
    if !l.is_finite() || l <= DEAD {
        return Vec2::ZERO;
    }
    v / l * ((l - DEAD) / (1.0 - DEAD)).min(1.0)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<PadFrame>().add_systems(PreUpdate, read);
}

#[cfg(not(target_arch = "wasm32"))]
fn read(mut frame: ResMut<PadFrame>, pads: Query<&bevy::input::gamepad::Gamepad>) {
    use bevy::input::gamepad::{GamepadAxis, GamepadButton as B};
    let before = frame.buttons;
    *frame = PadFrame {
        before,
        ..default()
    };
    let map = [
        (B::South, button::SOUTH),
        (B::East, button::EAST),
        (B::West, button::WEST),
        (B::North, button::NORTH),
        (B::LeftTrigger, button::LB),
        (B::RightTrigger, button::RB),
        (B::LeftTrigger2, button::LT),
        (B::Select, button::SELECT),
        (B::Start, button::START),
        (B::LeftThumb, button::L3),
        (B::RightThumb, button::R3),
        (B::DPadUp, button::UP),
        (B::DPadDown, button::DOWN),
        (B::DPadLeft, button::LEFT),
        (B::DPadRight, button::RIGHT),
    ];
    for pad in &pads {
        let buttons = map
            .iter()
            .filter(|(b, _)| pad.pressed(*b))
            .fold(0, |acc, (_, bit)| acc | bit);
        // gilrs reports the triggers as buttons with a value; some pads as
        // an axis too.
        let throttle = pad
            .get(B::RightTrigger2)
            .unwrap_or(0.0)
            .max(pad.get(GamepadAxis::RightZ).unwrap_or(0.0));
        frame.merge(pad.left_stick(), pad.right_stick(), throttle, buttons);
    }
}

#[cfg(target_arch = "wasm32")]
fn read(mut frame: ResMut<PadFrame>) {
    let before = frame.buttons;
    *frame = PadFrame {
        before,
        ..default()
    };
    web::read(&mut frame);
}

#[cfg(target_arch = "wasm32")]
mod web {
    //! `navigator.getGamepads()` through `Reflect` (no web-sys features to
    //! turn on, and a page's own `getGamepads` is read as the browser's).
    use super::PadFrame;
    use bevy::math::Vec2;
    use js_sys::{Array, Function, Reflect};
    use wasm_bindgen::{JsCast, JsValue};

    fn get(o: &JsValue, k: &str) -> JsValue {
        Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
    }

    pub fn read(frame: &mut PadFrame) {
        let Some(w) = web_sys::window() else { return };
        let nav: JsValue = w.navigator().into();
        let f = get(&nav, "getGamepads");
        let Some(f) = f.dyn_ref::<Function>() else {
            return;
        };
        let Ok(list) = f.call0(&nav) else { return };
        for p in Array::from(&list).iter() {
            if p.is_null() || p.is_undefined() || !get(&p, "connected").is_truthy() {
                continue;
            }
            let axes: Vec<f32> = Array::from(&get(&p, "axes"))
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            let values: Vec<(bool, f32)> = Array::from(&get(&p, "buttons"))
                .iter()
                .map(|b| {
                    let v = get(&b, "value").as_f64().unwrap_or(0.0) as f32;
                    (
                        get(&b, "pressed").is_truthy(),
                        if v.is_nan() { 0.0 } else { v },
                    )
                })
                .collect();
            let axis = |i: usize| axes.get(i).copied().unwrap_or(0.0);
            // The standard layout: buttons 0..16 are our bits, in order; the
            // right trigger (7) is read by its value instead.
            let buttons = values
                .iter()
                .take(16)
                .enumerate()
                .filter(|&(i, &(pressed, _))| pressed && i != 7)
                .fold(0, |acc, (i, _)| acc | 1 << i);
            let throttle = values.get(7).map_or(0.0, |b| b.1);
            // The browser's y is down.
            frame.merge(
                Vec2::new(axis(0), -axis(1)),
                Vec2::new(axis(2), -axis(3)),
                throttle,
                buttons,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dead_zone_swallows_drift_and_rescales_the_rest() {
        assert_eq!(dead_zone(Vec2::new(0.1, -0.1)), Vec2::ZERO);
        assert_eq!(dead_zone(Vec2::new(f32::NAN, 0.0)), Vec2::ZERO);
        let full = dead_zone(Vec2::new(0.0, 1.0));
        assert!((full.y - 1.0).abs() < 1e-6);
        let half = dead_zone(Vec2::new(0.0, 0.5 + 0.5 * DEAD));
        assert!((half.y - 0.5).abs() < 1e-5, "{half}");
    }

    #[test]
    fn pads_merge_the_strongest_stick_and_every_button() {
        let mut f = PadFrame::default();
        f.merge(Vec2::new(0.0, 0.5), Vec2::ZERO, 0.0, button::SOUTH);
        f.merge(Vec2::new(0.0, -0.9), Vec2::ZERO, 0.8, button::NORTH);
        assert!(f.left.y < -0.8);
        assert!(f.held(button::SOUTH) && f.held(button::NORTH) && f.held(button::RT));
        assert!(f.active());
        f.before = f.buttons;
        assert!(!f.just_pressed(button::SOUTH));
    }
}
