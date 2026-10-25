//! The open world's solid scenery (docs/vision/peninsula.md, stage 7d):
//! the trunks, fences and houses off the road that world generation draws
//! and the simulation stops a car against. Like the buildings
//! ([`crate::buildings`]) they are the simulation's, placed from the level
//! and the road network alone, so every player of a race meets the same
//! ones and what is drawn is what stands.
//!
//! Two kinds: the fixed ones, placed once when the track is built (a beach
//! town's storefronts, houses and palms, [`crate::seabright`]), and the planted ones
//! ([`crate::planting`]: the land's trees and the crop land's fences),
//! far too many to place over the whole world at once, so each tile of
//! land is planted the first time a car comes near it and kept. A tile's
//! planting is a function of the tile alone, so when it is planted does
//! not change what grows there.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::Track;
use crate::planting::{Planting, TILE};
use crate::seabright::Town;

/// A solid's outline on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A trunk: a circle of radius `r`.
    Round { r: f64 },
    /// A box `hx` by `hz` either side of the centre along its own axes
    /// (`ux`, `uz` the unit x axis; the z axis is `(-uz, ux)`): a fence's
    /// rails, a house.
    Box { hx: f64, hz: f64, ux: f64, uz: f64 },
}

/// A solid thing standing on the ground: its outline at (x, z), the ground
/// under it and how high it stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub x: f64,
    pub z: f64,
    pub y: f64,
    pub height: f64,
    pub shape: Shape,
}

/// How a car's box pushes out of a solid: the direction (unit,
/// horizontal, away from the solid) and how far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropPush {
    pub nx: f64,
    pub nz: f64,
    pub depth: f64,
    pub solid: Solid,
}

impl Solid {
    /// The radius of a circle round it.
    pub fn radius(&self) -> f64 {
        match self.shape {
            Shape::Round { r } => r,
            Shape::Box { hx, hz, .. } => (hx * hx + hz * hz).sqrt(),
        }
    }

    /// A box between two points, `half` thick either side of the line and
    /// `pad` longer at each end.
    pub fn bar(a: [f64; 2], b: [f64; 2], half: f64, pad: f64, y: f64, height: f64) -> Solid {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dz * dz).sqrt();
        let (ux, uz) = if len > 1e-9 {
            (dx / len, dz / len)
        } else {
            (1.0, 0.0)
        };
        Solid {
            x: (a[0] + b[0]) / 2.0,
            z: (a[1] + b[1]) / 2.0,
            y,
            height,
            shape: Shape::Box {
                hx: len / 2.0 + pad,
                hz: half,
                ux,
                uz,
            },
        }
    }

    /// Whether a circle `r` round (x, z) reaches it.
    pub fn reaches(&self, x: f64, z: f64, r: f64) -> bool {
        let (dx, dz) = (x - self.x, z - self.z);
        match self.shape {
            Shape::Round { r: rr } => dx * dx + dz * dz <= (r + rr) * (r + rr),
            Shape::Box { hx, hz, ux, uz } => {
                let (a, b) = (dx * ux + dz * uz, -dx * uz + dz * ux);
                a.abs() <= hx + r && b.abs() <= hz + r
            }
        }
    }

    /// The deepest overlap of a car's box (centre (x, z), half width `hw`
    /// across and half length `hl` along the heading (c, s)) with the
    /// solid, and the shortest way out: separating axes for a box, the
    /// box's nearest point for a trunk.
    pub fn push_out(&self, x: f64, z: f64, c: f64, s: f64, hw: f64, hl: f64) -> Option<PropPush> {
        let (dx, dz) = (x - self.x, z - self.z);
        let reach = (hw * hw + hl * hl).sqrt() + self.radius();
        if dx * dx + dz * dz > reach * reach {
            return None;
        }
        let (nx, nz, depth) = match self.shape {
            Shape::Round { r } => {
                // The trunk's centre in the car's frame, and the car's
                // nearest point to it.
                let (a, b) = (-dx * c - dz * s, dx * s - dz * c);
                let (qa, qb) = (a.clamp(-hl, hl), b.clamp(-hw, hw));
                let (ea, eb) = (qa - a, qb - b);
                let d2 = ea * ea + eb * eb;
                if d2 >= r * r {
                    return None;
                }
                if d2 > 1e-12 {
                    let d = d2.sqrt();
                    // Back to the world: forward (c, s), right (-s, c).
                    let (wx, wz) = (ea * c - eb * s, ea * s + eb * c);
                    (wx / d, wz / d, r - d)
                } else {
                    // The trunk's centre inside the car: out the nearest side.
                    let (oa, ob) = (hl - a.abs(), hw - b.abs());
                    if oa < ob {
                        let sg = if a > 0.0 { -1.0 } else { 1.0 };
                        (c * sg, s * sg, oa + r)
                    } else {
                        let sg = if b > 0.0 { -1.0 } else { 1.0 };
                        (-s * sg, c * sg, ob + r)
                    }
                }
            }
            Shape::Box { hx, hz, ux, uz } => {
                let axes = [(ux, uz), (-uz, ux), (c, s), (-s, c)];
                let mut min: Option<(f64, f64, f64)> = None;
                for (ax, az) in axes {
                    let rb = hx * (ux * ax + uz * az).abs() + hz * (-uz * ax + ux * az).abs();
                    let rc = hl * (c * ax + s * az).abs() + hw * (-s * ax + c * az).abs();
                    let d = dx * ax + dz * az;
                    let over = rb + rc - d.abs();
                    if over <= 0.0 {
                        return None;
                    }
                    if min.is_none_or(|m| over < m.0) {
                        let sg = if d >= 0.0 { 1.0 } else { -1.0 };
                        min = Some((over, ax * sg, az * sg));
                    }
                }
                let (depth, nx, nz) = min?;
                (nx, nz, depth)
            }
        };
        Some(PropPush {
            nx,
            nz,
            depth,
            solid: *self,
        })
    }

    /// The face of the solid toward (x, z): a point on it and its outward
    /// normal, for a half-space collider. A trunk's is its tangent facing
    /// the place; a box's the face the place is furthest outside of.
    pub fn face(&self, x: f64, z: f64) -> ([f64; 2], [f64; 2]) {
        let (dx, dz) = (x - self.x, z - self.z);
        match self.shape {
            Shape::Round { r } => {
                let d = (dx * dx + dz * dz).sqrt();
                let n = if d > 1e-9 {
                    [dx / d, dz / d]
                } else {
                    [1.0, 0.0]
                };
                ([self.x + n[0] * r, self.z + n[1] * r], n)
            }
            Shape::Box { hx, hz, ux, uz } => {
                let (a, b) = (dx * ux + dz * uz, -dx * uz + dz * ux);
                let faces = [
                    (a - hx, [ux, uz], hx),
                    (-a - hx, [-ux, -uz], hx),
                    (b - hz, [-uz, ux], hz),
                    (-b - hz, [uz, -ux], hz),
                ];
                let mut best = faces[0];
                for fc in faces {
                    if fc.0 > best.0 {
                        best = fc;
                    }
                }
                let n = best.1;
                ([self.x + n[0] * best.2, self.z + n[1] * best.2], n)
            }
        }
    }
}

/// The level's solid scenery: the fixed solids indexed by place, and the
/// planting with the tiles planted so far.
#[derive(Default)]
pub struct Props {
    fixed: Vec<Solid>,
    cells: BTreeMap<(i64, i64), Vec<u32>>,
    planting: Option<Planting>,
    town: Option<Town>,
    tiles: Mutex<Tiles>,
}

/// The planted tiles so far, by tile.
type Tiles = BTreeMap<(i64, i64), Arc<Vec<Solid>>>;

/// The planted tiles kept at most: past it they are planted again as they
/// are needed (the same, being functions of the tile).
const KEEP: usize = 4096;
/// How far a planted solid reaches out of its tile at most (m): a rail
/// between two posts belongs to the tile its middle is in.
const SPILL: f64 = 3.0;

impl Props {
    pub const CELL: f64 = 32.0;

    fn key(x: f64, z: f64) -> (i64, i64) {
        (
            (x / Self::CELL).floor() as i64,
            (z / Self::CELL).floor() as i64,
        )
    }

    pub fn new(fixed: Vec<Solid>, planting: Option<Planting>) -> Props {
        let mut cells: BTreeMap<(i64, i64), Vec<u32>> = BTreeMap::new();
        for (i, f) in fixed.iter().enumerate() {
            let r = f.radius();
            let (a0, b0) = Self::key(f.x - r, f.z - r);
            let (a1, b1) = Self::key(f.x + r, f.z + r);
            for a in a0..=a1 {
                for b in b0..=b1 {
                    cells.entry((a, b)).or_default().push(i as u32);
                }
            }
        }
        Props {
            fixed,
            cells,
            planting,
            town: None,
            tiles: Mutex::new(BTreeMap::new()),
        }
    }

    /// With the beach town whose solids are among the fixed ones.
    pub fn with_town(mut self, town: Option<Town>) -> Props {
        self.town = town;
        self
    }

    /// The level's beach town, for world generation to draw.
    pub fn town(&self) -> Option<&Town> {
        self.town.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.fixed.is_empty() && self.planting.is_none()
    }

    /// The solids placed when the track was built.
    pub fn fixed(&self) -> &[Solid] {
        &self.fixed
    }

    /// The level's planting, for world generation to draw.
    pub fn planting(&self) -> Option<&Planting> {
        self.planting.as_ref()
    }

    /// The planted solids of tile (ti, tj), planted now if they are not yet.
    pub fn tile(&self, t: &Track, ti: i64, tj: i64) -> Arc<Vec<Solid>> {
        let Some(p) = &self.planting else {
            return Arc::new(Vec::new());
        };
        let mut tiles = self.tiles.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(v) = tiles.get(&(ti, tj)) {
            return v.clone();
        }
        if tiles.len() >= KEEP {
            tiles.clear();
        }
        let v = Arc::new(p.solids(t, ti, tj));
        tiles.insert((ti, tj), v.clone());
        v
    }

    /// The fixed solids whose cells reach within `r` of (x, z), each once,
    /// in index order (not yet culled by distance).
    fn fixed_near(&self, x: f64, z: f64, r: f64) -> Vec<Solid> {
        let mut out = Vec::new();
        if !self.fixed.is_empty() {
            let (a0, b0) = Self::key(x - r, z - r);
            let (a1, b1) = Self::key(x + r, z + r);
            let mut ids: Vec<u32> = Vec::new();
            for a in a0..=a1 {
                for b in b0..=b1 {
                    if let Some(v) = self.cells.get(&(a, b)) {
                        for &i in v {
                            if let Err(k) = ids.binary_search(&i) {
                                ids.insert(k, i);
                            }
                        }
                    }
                }
            }
            out.extend(ids.into_iter().map(|i| self.fixed[i as usize]));
        }
        out
    }

    /// Every solid whose circle reaches within `r` of (x, z): the fixed
    /// ones in index order, then the planted ones tile by tile.
    pub fn near(&self, t: &Track, x: f64, z: f64, r: f64) -> Vec<Solid> {
        let mut out = self.fixed_near(x, z, r);
        if self.planting.is_some() {
            let pad = r + SPILL;
            let (i0, i1) = (
                ((x - pad) / TILE).floor() as i64,
                ((x + pad) / TILE).floor() as i64,
            );
            let (j0, j1) = (
                ((z - pad) / TILE).floor() as i64,
                ((z + pad) / TILE).floor() as i64,
            );
            for j in j0..=j1 {
                for i in i0..=i1 {
                    out.extend(self.tile(t, i, j).iter().copied());
                }
            }
        }
        out.retain(|s| {
            let (dx, dz) = (x - s.x, z - s.z);
            let rr = r + s.radius();
            dx * dx + dz * dz <= rr * rr
        });
        out
    }

    /// The deepest overlap of a car's box with a solid it is not above
    /// (`y`, the car's height), and the shortest way out of it.
    #[allow(clippy::too_many_arguments)]
    pub fn push_out(
        &self,
        t: &Track,
        x: f64,
        z: f64,
        y: f64,
        c: f64,
        s: f64,
        hw: f64,
        hl: f64,
    ) -> Option<PropPush> {
        let reach = (hw * hw + hl * hl).sqrt();
        let mut best: Option<PropPush> = None;
        for p in self.near(t, x, z, reach) {
            if y > p.y + p.height {
                continue;
            }
            if let Some(q) = p.push_out(x, z, c, s, hw, hl)
                && best.is_none_or(|b| q.depth > b.depth)
            {
                best = Some(q);
            }
        }
        best
    }
}

impl Clone for Props {
    fn clone(&self) -> Props {
        Props {
            fixed: self.fixed.clone(),
            cells: self.cells.clone(),
            planting: self.planting.clone(),
            town: self.town.clone(),
            tiles: Mutex::new(BTreeMap::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trunk(x: f64, z: f64, r: f64) -> Solid {
        Solid {
            x,
            z,
            y: 0.0,
            height: 8.0,
            shape: Shape::Round { r },
        }
    }

    #[test]
    fn a_car_pushes_off_a_trunk_the_way_it_met_it() {
        let t = trunk(0.0, 0.0, 0.5);
        // A car facing +x whose nose is 0.2 m into the trunk.
        let p = t.push_out(-2.3, 0.0, 1.0, 0.0, 1.0, 2.0).unwrap();
        assert!((p.depth - 0.2).abs() < 1e-9, "{p:?}");
        assert!((p.nx + 1.0).abs() < 1e-9 && p.nz.abs() < 1e-9, "{p:?}");
        // Its right side against it: pushed to the left (-z).
        let p = t.push_out(0.0, -1.3, 1.0, 0.0, 1.0, 2.0).unwrap();
        assert!(
            (p.depth - 0.2).abs() < 1e-9 && (p.nz + 1.0).abs() < 1e-9,
            "{p:?}"
        );
        // Clear past its corner.
        assert!(t.push_out(-2.4, 1.4, 1.0, 0.0, 1.0, 2.0).is_none());
        let (pt, n) = t.face(0.0, 10.0);
        assert_eq!((pt, n), ([0.0, 0.5], [0.0, 1.0]));
    }

    #[test]
    fn a_bar_lies_between_its_ends() {
        let b = Solid::bar([0.0, 0.0], [4.0, 0.0], 0.1, 0.0, 0.0, 1.2);
        assert!(b.reaches(2.0, 0.5, 0.45) && !b.reaches(2.0, 0.6, 0.45));
        assert!(!b.reaches(4.5, 0.0, 0.45));
        let p = b.push_out(2.0, 1.0, 1.0, 0.0, 0.95, 2.0).unwrap();
        assert!(
            (p.depth - 0.05).abs() < 1e-9 && (p.nz - 1.0).abs() < 1e-9,
            "{p:?}"
        );
    }

    #[test]
    fn fixed_solids_are_found_by_place() {
        let props = Props::new(vec![trunk(100.0, 100.0, 0.4), trunk(0.0, 0.0, 0.4)], None);
        assert_eq!(props.fixed_near(100.0, 101.0, 1.0), vec![props.fixed[0]]);
        assert!(props.fixed_near(50.0, 50.0, 1.0).is_empty());
    }
}
