//! The UI's fonts (Rajdhani, borrowed from midnight-playground, compiled
//! in) and small helpers for text and panels. Sizes are in `vmin`, so the
//! screens scale from a phone to a desktop.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSource, FontWeight};

/// The fonts go in as the app is built, so every screen's first frame
/// (the menu's `OnEnter` runs before `Startup`) can use them.
pub fn plugin(app: &mut App) {
    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let bold = fonts.add(Font::from_bytes(
        include_bytes!("../../../assets/fonts/rajdhani/Rajdhani-Bold.ttf").to_vec(),
    ));
    let semi = fonts.add(Font::from_bytes(
        include_bytes!("../../../assets/fonts/rajdhani/Rajdhani-SemiBold.ttf").to_vec(),
    ));
    app.insert_resource(Fonts { bold, semi });
}

#[derive(Resource, Clone)]
pub struct Fonts {
    pub bold: Handle<Font>,
    pub semi: Handle<Font>,
}

pub const WHITE: Color = Color::srgb(0.97, 0.98, 1.0);
pub const ICE: Color = Color::srgb(0.55, 0.85, 1.0);
pub const HOT: Color = Color::srgb(1.0, 0.42, 0.18);
pub const GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
pub const DIM: Color = Color::srgba(0.85, 0.9, 1.0, 0.7);
pub const PANEL: Color = Color::srgba(0.02, 0.05, 0.13, 0.93);

/// Text at `size` vmin.
pub fn text(
    fonts: &Fonts,
    s: impl Into<String>,
    size: f32,
    color: Color,
    bold: bool,
) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: FontSource::Handle(if bold {
                fonts.bold.clone()
            } else {
                fonts.semi.clone()
            }),
            font_size: FontSize::VMin(size),
            weight: if bold {
                FontWeight::BOLD
            } else {
                FontWeight::SEMIBOLD
            },
            ..default()
        },
        TextColor(color),
    )
}

/// A soft drop shadow under text, legible over bright snow.
pub fn shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::new(1.5, 2.0),
        color: Color::srgba(0.0, 0.03, 0.1, 0.8),
    }
}

/// A node covering the window.
pub fn full_screen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

/// `m:ss.cc`.
pub fn clock(t: f32) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0).floor() as u32;
    let s = t - m as f32 * 60.0;
    format!("{m}:{s:05.2}")
}

pub fn ordinal(n: usize) -> &'static str {
    match n {
        1 => "1ST",
        2 => "2ND",
        3 => "3RD",
        _ => "4TH",
    }
}

/// `12,340`.
pub fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats() {
        assert_eq!(super::clock(83.456), "1:23.46");
        assert_eq!(super::thousands(1234567), "1,234,567");
        assert_eq!(super::thousands(980), "980");
    }
}
