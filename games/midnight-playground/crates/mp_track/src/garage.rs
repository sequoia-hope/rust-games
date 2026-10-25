//! The open world's parking garages (docs/vision/peninsula.md, stage 8c;
//! DECISIONS D1249): a building of the city's ([`crate::Footprint`] with a
//! [`Garage`]) that a car drives into from the street and up, deck over
//! deck. One layout, read by the simulation (the ground under a car at its
//! height, the walls it meets) and by world generation (what it draws).
//!
//! In the footprint's own frame (`a` along its x axis, the street's way;
//! `b` across, `front` the side toward the street): decks `DECK_H` apart
//! from the floor, which is the pavement's height; along the back a strip
//! `STRIP_W` wide holds the ramps, each `RAMP_L` long and climbing one deck
//! the same way (`-a` to `+a`), stacked a deck apart; at the strip's ends
//! the decks run on, so a car off a ramp turns on the deck and drives back
//! to the next. A wall along the strip's inner edge, but at its ends, keeps
//! a car from dropping off a deck onto a ramp half a deck below; the
//! perimeter's walls stand at every deck, with the entrance, `ENTRANCE_W`
//! wide at the front's `-a` end, open at the floor.

/// A garage's decks (the floor and those over it; the top one open to the
/// sky), which side of its footprint faces the street (`b` of that sign),
/// and the floor's height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Garage {
    pub decks: u8,
    pub front: f64,
    pub floor: f64,
}

pub const DECK_H: f64 = 3.2;
pub const RAMP_L: f64 = 32.0;
pub const STRIP_W: f64 = 7.0;
pub const ENTRANCE_W: f64 = 7.0;
/// The parapet's height over a deck, and how far in from the footprint's
/// edge the strip's back lies.
pub const PARAPET: f64 = 1.1;
pub const STRIP_IN: f64 = 0.6;
/// How far above the car a deck or a ramp may be and still take it: a
/// ramp's rise in a tick and the suspension's give, not the deck above.
const STEP_UP: f64 = 0.6;

/// A wall a car meets, in world space: from `p` to `q`, between heights
/// `y0` and `y1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub p: [f64; 2],
    pub q: [f64; 2],
    pub y0: f64,
    pub y1: f64,
}

impl Garage {
    /// The top deck's height over the floor.
    pub fn top(&self) -> f64 {
        f64::from(self.decks.saturating_sub(1)) * DECK_H
    }

    /// The strip's `b` range (low, high) for a footprint `hz` deep.
    pub fn strip(&self, hz: f64) -> (f64, f64) {
        let back = -self.front;
        let (b0, b1) = (back * (hz - STRIP_IN - STRIP_W), back * (hz - STRIP_IN));
        (b0.min(b1), b0.max(b1))
    }

    /// The entrance's `a` range at the front, for a footprint `hx` long.
    pub fn entrance(&self, hx: f64) -> (f64, f64) {
        (-hx + 3.0, -hx + 3.0 + ENTRANCE_W)
    }

    /// The surfaces a car can stand on at (a, b): each ramp's where the
    /// strip holds them, each deck's elsewhere.
    pub fn surfaces(&self, hz: f64, a: f64, b: f64) -> Vec<f64> {
        let (s0, s1) = self.strip(hz);
        let n = self.decks as usize;
        if (s0..=s1).contains(&b) && a.abs() <= RAMP_L / 2.0 {
            let u = (a + RAMP_L / 2.0) / RAMP_L;
            (0..n.saturating_sub(1))
                .map(|k| self.floor + (k as f64 + u) * DECK_H)
                .collect()
        } else {
            (0..n).map(|k| self.floor + k as f64 * DECK_H).collect()
        }
    }

    /// The ground at (a, b) for a car at height `y`: the highest surface
    /// it can reach ([`STEP_UP`] over it at most), or the floor.
    pub fn ground(&self, hz: f64, a: f64, b: f64, y: f64) -> f64 {
        self.surfaces(hz, a, b)
            .into_iter()
            .filter(|&h| h <= y + STEP_UP)
            .fold(self.floor, f64::max)
    }
}

impl crate::Footprint {
    /// A garage's walls in world space: the perimeter's (the entrance's
    /// stretch only above the floor's deck) and the strip's inner edge.
    pub fn garage_walls(&self) -> Vec<Wall> {
        let Some(g) = &self.garage else {
            return Vec::new();
        };
        let w = |a0: f64, b0: f64, a1: f64, b1: f64, y0: f64, y1: f64| Wall {
            p: [
                self.x + a0 * self.ux - b0 * self.uz,
                self.z + a0 * self.uz + b0 * self.ux,
            ],
            q: [
                self.x + a1 * self.ux - b1 * self.uz,
                self.z + a1 * self.uz + b1 * self.ux,
            ],
            y0,
            y1,
        };
        let (hx, hz) = (self.hx, self.hz);
        let (lo, hi) = (g.floor - 1.0, g.floor + g.top() + PARAPET);
        let fb = g.front * hz;
        let bb = -fb;
        let (e0, e1) = g.entrance(hx);
        let mut out = vec![
            // The back and the two ends, whole.
            w(-hx, bb, hx, bb, lo, hi),
            w(-hx, -hz, -hx, hz, lo, hi),
            w(hx, -hz, hx, hz, lo, hi),
            // The front either side of the entrance, and over it from the
            // deck above the floor.
            w(-hx, fb, e0, fb, lo, hi),
            w(e1, fb, hx, fb, lo, hi),
            w(e0, fb, e1, fb, g.floor + DECK_H - 0.4, hi),
        ];
        // The strip's inner edge, open at its ends.
        let (s0, s1) = g.strip(hz);
        // The strip lies at the back: its inner edge is the one nearer the
        // middle.
        let inner = if g.front > 0.0 { s1 } else { s0 };
        let edge = RAMP_L / 2.0 - 4.0;
        out.push(w(-edge, inner, edge, inner, lo, hi));
        out
    }
}
