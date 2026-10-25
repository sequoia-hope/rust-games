//! What stands at a road's edges (docs/vision/peninsula.md, stage 7c;
//! DECISIONS D1247): each metre and side, the road type's edge (a fence, a
//! concrete barrier, a kerb and railing, the land's rock face or guardrail,
//! or nothing), cleared across the mouths of the roads that meet it, where
//! another road crosses it, and where it lies on a greater road it meets.
//! The road kit draws what this says (choosing rock or guardrail by the
//! land, `mp_worldgen::road`), and on an open level the simulation stops a
//! car against it, so a fence that is drawn is one the car cannot drive
//! through, and a gap that is drawn is one it can.

use pk_math::js;

use crate::ROAD_TYPES;
use crate::network::JunctionKind;
use crate::track::Track;

/// The edge codes: nothing; a wooden fence; a concrete (jersey) barrier;
/// a kerb with a railing; a kerb (a pavement past it, no barrier); the
/// land's own edge, which world generation makes a rock face or a
/// guardrail by the ground beside it.
pub const NONE: u8 = 0;
pub const FENCE: u8 = 3;
pub const JERSEY: u8 = 4;
pub const RAIL: u8 = 5;
pub const CURB: u8 = 6;
pub const TERRAIN: u8 = 7;

/// A road's edges, one code a sample, each side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    pub left: Vec<u8>,
    pub right: Vec<u8>,
}

/// The code of a road type's `edge`.
pub fn edge_code(edge: &str) -> u8 {
    match edge {
        "fence" => FENCE,
        "jersey" => JERSEY,
        "rail" => RAIL,
        "curb" => CURB,
        "terrain" => TERRAIN,
        _ => NONE,
    }
}

impl Track {
    /// Road `road`'s edges by its type, cleared where the network meets it
    /// (the road kit's rules, D1218 and stage 6c): across a branch's mouth
    /// on the side it comes in from, where a branch's far end meets the
    /// road again, both sides where a road crosses it (each wider as the
    /// angle narrows), and wherever the road lies on a greater road's
    /// tarmac. Fence gaps are not applied here (they are per kind:
    /// [`Track::barrier_at`]).
    pub fn edges_of(&self, road: usize) -> Edges {
        let t = self.road(road);
        let n = t.n;
        let code = |i: usize| {
            ROAD_TYPES
                .get(t.road_type[i] as usize)
                .map_or(NONE, |rt| edge_code(rt.edge))
        };
        let mut l: Vec<u8> = (0..n).map(code).collect();
        let mut r = l.clone();
        let mut clear = |at: f64, w: f64, side: f64| {
            let (k0, k1) = ((at - w).floor() as i64, (at + w).ceil() as i64);
            for k in k0..=k1 {
                let i = if t.is_loop {
                    k.rem_euclid(n as i64)
                } else if (0..n as i64).contains(&k) {
                    k
                } else {
                    continue;
                } as usize;
                if side < 0.0 {
                    l[i] = NONE;
                } else {
                    r[i] = NONE;
                }
            }
        };
        // How much wider a road's mouth is along this one for the angle
        // it meets at (the road's direction a little in from its end).
        let slant = |bt: &Track, at: f64, other_s: f64| {
            let f = bt.frame(at);
            let g = t.frame(other_s);
            let cos = f.fx * g.fx + f.fz * g.fz;
            1.0 / (1.0 - cos * cos).max(0.09).sqrt()
        };
        for j in self.junctions.iter().filter(|j| j.other == road) {
            let bt = self.road(j.road);
            match j.kind {
                JunctionKind::Start => {
                    let k = slant(bt, (bt.length * 0.5).min(25.0), j.other_s);
                    let w = (f64::from(bt.hw[0]) + f64::from(bt.margin[0])) * k + 3.0;
                    clear(j.other_s, w, self.branches[j.road - 1].side);
                }
                JunctionKind::End => {
                    let last = bt.n - 1;
                    let k = slant(bt, (bt.length - 25.0).max(bt.length * 0.5), j.other_s);
                    let w = (f64::from(bt.hw[last]) + f64::from(bt.margin[last])) * k + 3.0;
                    // The side the branch comes in from.
                    let f = bt.frame((bt.length - 30.0).max(0.0));
                    let side = js::sign(t.project(f.x, f.z, j.other_s).lat);
                    clear(j.other_s, w, side);
                }
                JunctionKind::Cross => {
                    // The crossing's width along this road grows as the
                    // angle between them narrows.
                    let f = bt.frame(j.s);
                    let g = t.frame(j.other_s);
                    let cos = f.fx * g.fx + f.fz * g.fz;
                    let sin = (1.0 - cos * cos).max(0.09).sqrt();
                    let w = (f.hw + f.margin) / sin + 3.0;
                    clear(j.other_s, w, -1.0);
                    clear(j.other_s, w, 1.0);
                }
            }
        }
        // Nothing on the tarmac of the greater roads it meets.
        let mut greater: Vec<usize> = Vec::new();
        for j in self.junctions.iter().filter(|j| j.road == road) {
            if !greater.contains(&j.other) {
                greater.push(j.other);
            }
        }
        for i in 0..n {
            if greater.iter().any(|&p| on_road(t, self.road(p), i as f64)) {
                l[i] = NONE;
                r[i] = NONE;
            }
        }
        Edges { left: l, right: r }
    }

    /// Whether a barrier stands at `s` on road `road`'s side `side` (-1
    /// left, 1 right) of an open level: a fence, a concrete barrier, a
    /// railing (each but in the road's fence gaps) or the land's edge.
    pub fn barrier_at(&self, road: usize, s: f64, side: f64) -> bool {
        let Some(e) = self.edges.get(road) else {
            return false;
        };
        let t = self.road(road);
        let i = t.idx(if t.is_loop {
            t.wrap(s)
        } else {
            s.clamp(0.0, t.length)
        });
        let c = if side < 0.0 { e.left[i] } else { e.right[i] };
        match c {
            TERRAIN | JERSEY => true,
            FENCE | RAIL => !t
                .fence_gaps
                .iter()
                .any(|g| g.side == side && s > g.s0 && s < g.s1),
            _ => false,
        }
    }
}

/// Whether road `t`'s metre at `s` lies on road `p`'s tarmac (out to its
/// margin, this road's half width and a metre more).
fn on_road(t: &Track, p: &Track, s: f64) -> bool {
    let f = t.frame(s);
    let d = p.distance_to_road(f.x, f.z, 60.0);
    let Some(ps) = d.s else { return false };
    if !p.is_loop && !(0.0..=p.length).contains(&ps) {
        return false;
    }
    let pf = p.frame(if p.is_loop { p.wrap(ps) } else { ps });
    d.lat.abs() < pf.hw + pf.margin + f.hw + 1.0
}
