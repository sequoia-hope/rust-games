//! A UI node's box in CSS pixels, whole and as its scrolling ancestors show
//! it (Midnight Playground DECISIONS D1204: a tap reaches a control only
//! where it shows, not through what clips it). Used by a game's hit test
//! and test bridge, and by the web glue's tap-sensitive controls
//! (`web::FullscreenOnTap`).
//!
//! `css` is CSS px per logical px: 1 under the web's pixel-ratio override
//! (logical pixels are CSS pixels there, D1195) and natively; the measured
//! ratio otherwise (`web::CanvasScale`).

use bevy::prelude::*;
use bevy::ui::{CalculatedClip, ComputedNode, UiGlobalTransform};

/// A box: `(left, top, width, height)`.
pub type Rect = (f32, f32, f32, f32);

/// A node's box in CSS px.
pub fn css_rect(node: &ComputedNode, t: &UiGlobalTransform, css: f32) -> Rect {
    let k = node.inverse_scale_factor * css;
    let c = t.affine().translation * k;
    let s = node.size * k;
    (c.x - s.x / 2.0, c.y - s.y / 2.0, s.x, s.y)
}

/// The part of a node's box its scrolling screen shows, in CSS px:
/// [`css_rect`] cut to the clip its ancestors give it (empty, at its
/// corner, when none of it shows).
pub fn css_shown(
    node: &ComputedNode,
    t: &UiGlobalTransform,
    clip: Option<&CalculatedClip>,
    css: f32,
) -> Rect {
    let r = css_rect(node, t, css);
    let Some(clip) = clip else { return r };
    let k = node.inverse_scale_factor * css;
    let (x0, y0) = (r.0.max(clip.clip.min.x * k), r.1.max(clip.clip.min.y * k));
    let (x1, y1) = (
        (r.0 + r.2).min(clip.clip.max.x * k),
        (r.1 + r.3).min(clip.clip.max.y * k),
    );
    if x1 <= x0 || y1 <= y0 {
        return (r.0, r.1, 0.0, 0.0);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// Whether `p` is in `r` (edges included).
pub fn inside(r: Rect, p: Vec2) -> bool {
    p.x >= r.0 && p.x <= r.0 + r.2 && p.y >= r.1 && p.y <= r.1 + r.3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_edges() {
        let r = (10.0, 20.0, 30.0, 40.0);
        assert!(inside(r, Vec2::new(10.0, 20.0)));
        assert!(inside(r, Vec2::new(40.0, 60.0)));
        assert!(!inside(r, Vec2::new(40.1, 30.0)));
    }
}
