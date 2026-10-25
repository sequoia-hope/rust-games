//! The open world's planting (docs/vision/peninsula.md, stage 7d): the
//! trees and scrub the Land kit plants by land cover (stage 4a, D1212),
//! the cypress, pines and outcrops the Coast kit sets along its coast, and the
//! post-and-rail fences of the crop land (stage 4d), placed here from the
//! level's own ground and cover and the road network, so that world
//! generation draws them and the simulation stops a car against every
//! trunk and fence ([`crate::props`]).
//!
//! Candidates stand on a lattice fixed in the world, [`SPACING`] apart,
//! each jittered and drawn from its own seed, so what grows on a tile of
//! [`TILE`] metres is a function of the tile alone: the simulation plants
//! a tile when a car first comes near it, world generation a cell of tiles
//! at a time, and both get the same. A fence's posts stand along the roads
//! every [`POST`] metres; each post belongs to the place it stands, each
//! rail to the place its middle is, so any box of land finds the same.
//!
//! The rules are the Land kit's and the Coast kit's as they were with the
//! terrain's ground, its far field and its zone weights (which only world
//! generation has); here the level's ground, the nearest road's sample and
//! its zone say instead.

use pk_math::{Mulberry32, Noise2D, fbm, kernel, lerp, smoothstep};

use crate::props::{Shape, Solid};
use crate::{Cover, CoverFn, GroundFn, Level, Track};

/// Candidates this far apart (m), on a lattice fixed in the world.
pub const SPACING: f64 = 8.0;
/// Candidates a tile is on a side.
const PER: i64 = 8;
/// Metres of land a tile is on a side: what the simulation plants at once.
pub const TILE: f64 = SPACING * PER as f64;
/// Nothing is planted farther than this from the nearest road (m).
pub const REACH: f64 = 450.0;
/// The forest's bands from the road (m): detailed trees on the verge,
/// cheaper ones behind, far quads beyond.
const VERGE: f64 = 45.0;
const MID: f64 = 180.0;
/// Nothing within this of the road's edge (m), and trees farther still.
const CLEAR: f64 = 2.0;
const CLEAR_TREE: f64 = 4.5;
/// Within this of a road's sample (m) a place is measured against the
/// roads themselves.
const NEAR: f64 = 60.0;
/// The JS kits write a random yaw as `6.3`, not 2π.
const TURN: f64 = 6.3;
/// The kits' densities were for candidates 7 m apart.
const DENSITY: f64 = SPACING * SPACING / 49.0;
/// The Coast kit plants within this of the road (m).
const COAST_REACH: f64 = 360.0;
/// The terrain's second noise, which the kits' clumps are drawn from:
/// world generation's terrain seed (7) plus 101.
const CLUMP_SEED: u32 = 108;

/// Fence posts this far apart (m), and the rail between two.
pub const POST: f64 = 4.0;
/// A field boundary fence leaves the road about every this many metres.
const FIELD: f64 = 190.0;
/// How far from the road a field boundary runs at most (m).
const FIELD_DEPTH: f64 = 240.0;
/// The road-edge fence stands this far outside the road's margin (m).
const FENCE_OFF: f64 = 2.2;
/// The steepest ground a fence or a tree stands on (rise per metre).
const FENCE_SLOPE: f64 = 1.2;

/// How high a fence stands to the simulation over the level's ground (m):
/// its posts are 1.25 m, and the ground the car rides near a road is the
/// road's blend, not the land's own.
pub const FENCE_TOP: f64 = 2.0;
/// How high a trunk stands to the simulation (m).
pub const TRUNK_TOP: f64 = 8.0;

/// What grows: the Land kit's templates and the Coast kit's two trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Fir,
    Spruce,
    SpruceMid,
    SpruceFar,
    Oak,
    OakFar,
    Scrub,
    Cypress,
    Pine,
    /// The Coast kit's outcrops on the landward slopes.
    Rock,
}

/// A plant: where it stands, what it is, how it is scaled and turned, and
/// the seed of the rest of its looks (world generation's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plant {
    pub x: f64,
    pub z: f64,
    pub kind: Kind,
    pub sx: f64,
    pub sy: f64,
    pub sz: f64,
    pub ry: f64,
    /// How steep the ground is under it (rise per metre).
    pub slope: f64,
    pub look: Mulberry32,
}

impl Plant {
    /// The radius of its trunk at the foot (m), as its template draws it
    /// scaled, or of a rock where it meets the ground; none for scrub,
    /// which a car drives through.
    pub fn trunk(&self) -> Option<f64> {
        let r = match self.kind {
            // `rock_geometry`'s unit rock, squashed and sunk a third of its
            // size: about 0.7 of its radius at the ground.
            Kind::Rock => return Some(0.7 * self.sx.min(self.sz)),
            // `conifer_geometry`: 0.22 of the crown's radius over 1.8.
            Kind::Fir => 0.22 * 2.3 / 1.8,
            Kind::Spruce | Kind::SpruceMid | Kind::SpruceFar => 0.22 * 1.75 / 1.8,
            Kind::Oak | Kind::OakFar => 0.14,
            Kind::Cypress => 0.38,
            Kind::Pine => 0.42,
            Kind::Scrub => return None,
        };
        Some((r * self.sx).max(0.25))
    }
}

/// A tile's planting and fences.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tile {
    pub plants: Vec<Plant>,
    pub fences: Fences,
}

/// Fence posts, and the rails between two (their two posts).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fences {
    pub posts: Vec<[f64; 2]>,
    pub rails: Vec<[[f64; 2]; 2]>,
}

/// An open level's planting: what it is planted by.
#[derive(Clone)]
pub struct Planting {
    cover: CoverFn,
    ground: GroundFn,
    /// The Land kit keeps its trees this far over the sea (none: no sea).
    sea_y: f64,
    /// The Coast kit's sea (0 without one).
    coast_sea_y: f64,
    noise: Noise2D,
    /// The zones of the Coast kit's scenery.
    coast: Vec<usize>,
    /// Discs the Coast kit keeps clear (its pull-outs): x, z, r.
    exclusions: Vec<[f64; 3]>,
}

/// A candidate's seed, from its place on the lattice.
fn seed(i: i64, j: i64) -> u32 {
    let mut h = 0x4042_5eed_u32
        ^ (i as u32).wrapping_mul(0x9E37_79B9)
        ^ (j as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

fn at(a: &[f32], i: usize) -> f64 {
    a.get(i).map_or(f64::NAN, |&v| f64::from(v))
}

/// The road sample nearest a candidate.
#[derive(Clone, Copy, Debug)]
struct Near {
    d: f64,
    road: usize,
    k: usize,
    sx: f64,
    sz: f64,
}

impl Planting {
    /// The planting of an open level with a land cover; none otherwise.
    /// `t` is the level's built network (the Coast kit's pull-outs are
    /// placed on its road).
    pub fn new(t: &Track, level: &Level) -> Option<Planting> {
        if !level.open {
            return None;
        }
        let (cover, ground) = (level.cover.clone()?, level.ground.clone()?);
        let coast: Vec<usize> = level
            .zones
            .iter()
            .enumerate()
            .filter(|(_, z)| z.scenery == "Coast")
            .map(|(i, _)| i)
            .collect();
        let mut exclusions = Vec::new();
        if !coast.is_empty() {
            // `Coast::plan`'s: the coffee pull-out by the start, the
            // lighthouse's headland and the bluff's vista.
            let s0 = 30.0;
            let wr = at(&t.wall_r, t.idx(s0));
            let p = t.point_at(s0, wr + 14.0);
            exclusions.push([p.x, p.z, 20.0]);
            if let Some(lh) = t.tag("lighthouse").first() {
                let s = pk_math::js::round((lh.s0 + lh.s1) / 2.0) + 20.0;
                let wl = at(&t.wall_l, t.idx(s));
                let p = t.point_at(s, -(wl + 58.0));
                exclusions.push([p.x, p.z, 27.0]);
            }
            if let Some(bl) = t.tag("bluff").first() {
                let s = pk_math::js::round((bl.s0 + bl.s1) / 2.0);
                let wl = at(&t.wall_l, t.idx(s));
                let p = t.point_at(s, -(wl + 11.0));
                exclusions.push([p.x, p.z, 14.0]);
            }
        }
        Some(Planting {
            cover,
            ground,
            sea_y: level.sea_y.unwrap_or(f64::NEG_INFINITY),
            coast_sea_y: level.sea_y.unwrap_or(0.0),
            noise: Noise2D::new(CLUMP_SEED),
            coast,
            exclusions,
        })
    }

    /// The level's ground at (x, z), and its slope (rise per metre).
    fn ground(&self, x: f64, z: f64) -> (f64, f64) {
        let g = &self.ground;
        let h = g(x, z);
        let gx = (g(x + 1.0, z) - g(x - 1.0, z)) / 2.0;
        let gz = (g(x, z + 1.0) - g(x, z - 1.0)) / 2.0;
        (h, (gx * gx + gz * gz).sqrt())
    }

    fn excluded(&self, x: f64, z: f64, pad: f64) -> bool {
        self.exclusions.iter().any(|e| {
            let (dx, dz) = (x - e[0], z - e[1]);
            dx * dx + dz * dz < (e[2] + pad) * (e[2] + pad)
        })
    }

    /// The plants of tile (ti, tj): the candidates whose lattice points lie
    /// in it, in lattice order.
    pub fn plants(&self, t: &Track, ti: i64, tj: i64) -> Vec<Plant> {
        let mut out = Vec::new();
        let (cx, cz) = ((ti as f64 + 0.5) * TILE, (tj as f64 + 0.5) * TILE);
        // A candidate lies within this of the tile's centre.
        let h = (TILE / 2.0 + SPACING * 0.45) * core::f64::consts::SQRT_2;
        let cap = REACH.max(COAST_REACH);
        let Some(dc) = t
            .net
            .nearest_samples(t, cx, cz, cap + h, 0.0)
            .iter()
            .map(|e| e.2)
            .reduce(f64::min)
        else {
            return out;
        };
        if dc - h > cap {
            return out;
        }
        // Every candidate's nearest sample within `cap` lies within this of
        // the centre: the gathered samples hold it, whichever they are.
        let samples = t.net.samples_within(t, cx, cz, (dc + 2.0 * h).min(cap + h));
        for j in 0..PER {
            for i in 0..PER {
                if let Some(p) = self.candidate(t, ti * PER + i, tj * PER + j, &samples) {
                    out.push(p);
                }
            }
        }
        out
    }

    fn candidate(
        &self,
        t: &Track,
        gi: i64,
        gj: i64,
        samples: &[(u32, u32, f64, f64)],
    ) -> Option<Plant> {
        let mut rng = Mulberry32::new(seed(gi, gj));
        let px = (gi as f64 + 0.5) * SPACING + (rng.next_f64() - 0.5) * SPACING * 0.9;
        let pz = (gj as f64 + 0.5) * SPACING + (rng.next_f64() - 0.5) * SPACING * 0.9;
        let r = rng.next_f64();
        let rc = rng.next_f64();
        let ro = rng.next_f64();
        // The nearest sample, the lesser (road, sample) on a tie.
        let mut best: Option<(f64, u32, u32, f64, f64)> = None;
        for &(road, k, sx, sz) in samples {
            let (dx, dz) = (px - sx, pz - sz);
            let d2 = dx * dx + dz * dz;
            if best.is_none_or(|b| d2 < b.0 || (d2 == b.0 && (road, k) < (b.1, b.2))) {
                best = Some((d2, road, k, sx, sz));
            }
        }
        let (d2, road, k, sx, sz) = best?;
        let near = Near {
            d: d2.sqrt(),
            road: road as usize,
            k: k as usize,
            sx,
            sz,
        };
        if near.d > REACH.max(COAST_REACH) || self.excluded(px, pz, 2.0) {
            return None;
        }
        if let Some(p) = self.land(t, px, pz, r, &near, &mut rng) {
            return Some(p);
        }
        if let Some(p) = self.coast(t, px, pz, rc, &near, &mut rng) {
            return Some(p);
        }
        self.outcrop(t, px, pz, ro, &near, &mut rng)
    }

    /// The Land kit's rule: by cover and distance from the road.
    fn land(
        &self,
        t: &Track,
        px: f64,
        pz: f64,
        r: f64,
        near: &Near,
        rng: &mut Mulberry32,
    ) -> Option<Plant> {
        let d = near.d;
        if d > REACH {
            return None;
        }
        let cover = Cover::of((self.cover)(px, pz));
        if !matches!(
            cover,
            Cover::Forest | Cover::Shrub | Cover::Grass | Cover::Wetland
        ) {
            return None;
        }
        let clump = smoothstep(-0.2, 0.4, fbm(&self.noise, px / 260.0 + 1.7, pz / 260.0, 3));
        let kind = choose(cover, d, clump, r)?;
        let tree = kind != Kind::Scrub;
        let clear = if tree { CLEAR_TREE } else { CLEAR };
        let rt = t.road(near.road);
        let road_w = at(&rt.hw, near.k).max(5.0) + 1.5;
        if d < road_w + clear {
            return None;
        }
        if d < road_w + clear + NEAR && road_clearance(t, px, pz, NEAR + 20.0) < 1.5 + clear {
            return None;
        }
        let (h, slope) = self.ground(px, pz);
        if h < self.sea_y + 1.5 || slope > if tree { 1.1 } else { 1.4 } {
            return None;
        }
        let (sx, sy, sz) = match kind {
            Kind::Fir | Kind::Spruce | Kind::SpruceMid | Kind::SpruceFar => {
                // Redwood country: tall. The far quads a little bigger
                // still, to read at distance.
                let sc = lerp(1.0, 2.2, kernel::pow(rng.next_f64(), 1.2))
                    * if kind == Kind::SpruceFar { 1.15 } else { 1.0 };
                let sx = sc * lerp(0.85, 1.15, rng.next_f64());
                (sx, sc * lerp(0.95, 1.35, rng.next_f64()), sx)
            }
            Kind::Oak | Kind::OakFar => {
                let sc = lerp(2.2, 4.2, kernel::pow(rng.next_f64(), 1.3));
                let sx = sc * lerp(0.85, 1.2, rng.next_f64());
                (sx, sc * lerp(0.8, 1.1, rng.next_f64()), sx)
            }
            _ => {
                let steep = smoothstep(0.35, 0.8, slope);
                let sc = lerp(0.6, 1.9, kernel::pow(rng.next_f64(), 1.3));
                let sx = sc * lerp(0.8, 1.3, rng.next_f64());
                let sy = sc * lerp(lerp(0.6, 1.0, rng.next_f64()), 1.25, steep);
                (sx, sy, sc)
            }
        };
        let ry = rng.next_f64() * TURN;
        Some(Plant {
            x: px,
            z: pz,
            kind,
            sx,
            sy,
            sz,
            ry,
            slope,
            look: *rng,
        })
    }

    /// The Coast kit's rule: cypress taking the wind on the headlands and
    /// in groves inland, pines in the sheltered groves, in its zones.
    fn coast(
        &self,
        t: &Track,
        px: f64,
        pz: f64,
        r: f64,
        near: &Near,
        rng: &mut Mulberry32,
    ) -> Option<Plant> {
        let rt = t.road(near.road);
        let zone = usize::from(*rt.zone.get(near.k)?);
        if near.d > COAST_REACH || !self.coast.contains(&zone) {
            return None;
        }
        let (rx, rz) = (at(&rt.rx, near.k), at(&rt.rz, near.k));
        let sea_side = (px - near.sx) * rx + (pz - near.sz) * rz < 0.0;
        let (h, slope) = self.ground(px, pz);
        if h < self.coast_sea_y + 3.0 {
            return None;
        }
        let road_y = at(&rt.py, near.k);
        // Sea side: only the cliff tops, not the faces.
        if sea_side && (h < road_y - 25.0 || slope > 0.45) {
            return None;
        }
        if slope > 0.75 {
            return None;
        }
        let clump = smoothstep(
            -0.15,
            0.35,
            fbm(&self.noise, px / 230.0 + 3.1, pz / 230.0, 3),
        );
        let pc = (if sea_side { 0.03 } else { 0.16 })
            * (0.25 + clump)
            * (1.0 - smoothstep(0.4, 0.75, slope))
            * DENSITY;
        if r >= pc || !wall_clear(t, px, pz, 4.5) {
            return None;
        }
        let pine = !sea_side && near.d > 25.0 && rng.next_f64() < 0.45;
        let ry = if pine {
            rng.next_f64() * TURN
        } else {
            // Leaning inland, along the road's right vector.
            kernel::atan2(-rz, rx) + (rng.next_f64() - 0.5) * 0.6
        };
        let sc = if pine {
            lerp(0.85, 1.35, rng.next_f64())
        } else {
            lerp(0.8, 1.6, kernel::pow(rng.next_f64(), 1.2))
        };
        let sy = sc * lerp(0.85, 1.15, rng.next_f64());
        Some(Plant {
            x: px,
            z: pz,
            kind: if pine { Kind::Pine } else { Kind::Cypress },
            sx: sc,
            sy,
            sz: sc,
            ry,
            slope,
            look: *rng,
        })
    }

    /// The Coast kit's outcrops: rock on the steep landward slopes of its
    /// zones (its odds for candidates 12 m apart scaled to these).
    fn outcrop(
        &self,
        t: &Track,
        px: f64,
        pz: f64,
        r: f64,
        near: &Near,
        rng: &mut Mulberry32,
    ) -> Option<Plant> {
        let rt = t.road(near.road);
        let zone = usize::from(*rt.zone.get(near.k)?);
        if !(14.0..=320.0).contains(&near.d) || !self.coast.contains(&zone) {
            return None;
        }
        let (rx, rz) = (at(&rt.rx, near.k), at(&rt.rz, near.k));
        if (px - near.sx) * rx + (pz - near.sz) * rz < 0.0 {
            return None;
        }
        let (_, slope) = self.ground(px, pz);
        if slope < 0.8
            || r >= 0.25 * smoothstep(0.8, 1.5, slope) * SPACING * SPACING / 144.0
            || !wall_clear(t, px, pz, 6.0)
        {
            return None;
        }
        let size = lerp(2.5, 8.0, kernel::pow(rng.next_f64(), 1.5))
            * if near.d < 50.0 { 0.6 } else { 1.0 };
        let sx = size * lerp(1.0, 1.7, rng.next_f64());
        let sy = size * lerp(0.6, 1.0, rng.next_f64());
        let ry = rng.next_f64() * TURN;
        Some(Plant {
            x: px,
            z: pz,
            kind: Kind::Rock,
            sx,
            sy,
            sz: size,
            ry,
            slope,
            look: *rng,
        })
    }

    /// The crop land's fences standing in `[x0, x1) × [z0, z1)`: a post
    /// and rail line along every road's edge where the verge is crop, and
    /// field boundaries running out from the road every [`FIELD`] metres
    /// or so while the land is crop.
    pub fn fences(&self, t: &Track, x0: f64, z0: f64, x1: f64, z1: f64) -> Fences {
        let mut out = Fences::default();
        let inside = |x: f64, z: f64| x >= x0 && x < x1 && z >= z0 && z < z1;
        let near =
            |x: f64, z: f64| x >= x0 - POST && x < x1 + POST && z >= z0 - POST && z < z1 + POST;
        let crop_at = |x: f64, z: f64| Cover::of((self.cover)(x, z)) == Cover::Crop;
        let pad = FIELD_DEPTH + 40.0;
        for (ri, road) in t.roads().enumerate() {
            let b = road.bounds;
            if b.max_x < x0 - pad || b.min_x > x1 + pad || b.max_z < z0 - pad || b.min_z > z1 + pad
            {
                continue;
            }
            let mut s = 0.0;
            while s < road.length {
                let f = road.frame(s);
                // Only the road near the box matters.
                if f.x < x0 - pad || f.x > x1 + pad || f.z < z0 - pad || f.z > z1 + pad {
                    s += POST * 8.0;
                    continue;
                }
                let off = f.hw + f.margin + FENCE_OFF;
                // Not on another road's tarmac.
                let edge = off - f.hw - 0.5;
                for side in [-1.0, 1.0] {
                    let post = |fr: &crate::Frame| {
                        let (x, z) = (fr.x + fr.rx * off * side, fr.z + fr.rz * off * side);
                        let ok = crop_at(x, z) && road_clearance(t, x, z, off + 40.0) >= edge && {
                            let (h, slope) = self.ground(x, z);
                            h >= self.sea_y + 1.0 && slope <= FENCE_SLOPE
                        };
                        ([x, z], ok)
                    };
                    let (p, ok) = post(&f);
                    if !ok {
                        continue;
                    }
                    if inside(p[0], p[1]) {
                        out.posts.push(p);
                    }
                    // The rail back to the post before.
                    if s >= POST {
                        let (q, prev) = post(&road.frame(s - POST));
                        if prev && inside((p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0) {
                            out.rails.push([q, p]);
                        }
                    }
                    // A field boundary out from the road, every FIELD or so
                    // (where in each window is the window's own draw).
                    let win = (s / FIELD).floor();
                    let at = win * FIELD
                        + pk_math::hash2(win, ri as f64 + side, 11.0) * (FIELD - POST * 2.0)
                        + POST;
                    if s < at || s - POST >= at {
                        continue;
                    }
                    let dir = [f.rx * side, f.rz * side];
                    let mut prev: Option<[f64; 2]> = None;
                    let mut d = POST;
                    while d <= FIELD_DEPTH + POST {
                        let (x, z) = (p[0] + dir[0] * d, p[1] + dir[1] * d);
                        d += POST;
                        if !near(x, z) {
                            prev = None;
                            continue;
                        }
                        let keep =
                            crop_at(x, z) && road_clearance(t, x, z, off + NEAR) >= edge && {
                                let (h, slope) = self.ground(x, z);
                                h >= self.sea_y + 1.0 && slope <= FENCE_SLOPE
                            };
                        if !keep {
                            prev = None;
                            continue;
                        }
                        if inside(x, z) {
                            out.posts.push([x, z]);
                        }
                        if let Some(q) = prev
                            && inside((q[0] + x) / 2.0, (q[1] + z) / 2.0)
                        {
                            out.rails.push([q, [x, z]]);
                        }
                        prev = Some([x, z]);
                    }
                }
                s += POST;
            }
        }
        out
    }

    /// Tile (ti, tj)'s plants and fences.
    pub fn tile(&self, t: &Track, ti: i64, tj: i64) -> Tile {
        let (x0, z0) = (ti as f64 * TILE, tj as f64 * TILE);
        Tile {
            plants: self.plants(t, ti, tj),
            fences: self.fences(t, x0, z0, x0 + TILE, z0 + TILE),
        }
    }

    /// What stops a car on tile (ti, tj): every trunk, rock and rail.
    pub fn solids(&self, t: &Track, ti: i64, tj: i64) -> Vec<Solid> {
        let tile = self.tile(t, ti, tj);
        let mut out = Vec::new();
        for p in &tile.plants {
            if let Some(r) = p.trunk() {
                out.push(Solid {
                    x: p.x,
                    z: p.z,
                    y: (self.ground)(p.x, p.z),
                    height: TRUNK_TOP,
                    shape: Shape::Round { r },
                });
            }
        }
        for [a, b] in &tile.fences.rails {
            let y = (self.ground)(a[0], a[1]).min((self.ground)(b[0], b[1]));
            // The posts are 13 cm square: the bar takes them in.
            out.push(Solid::bar(*a, *b, 0.08, 0.07, y, FENCE_TOP));
        }
        out
    }
}

/// How likely a candidate is planted, by cover and distance from the
/// road, and with what (the Land kit's rule, its odds for its 7 m
/// candidates scaled to these).
fn choose(cover: Cover, d: f64, clump: f64, r: f64) -> Option<Kind> {
    match cover {
        Cover::Forest => {
            let p = DENSITY
                * if d < VERGE {
                    0.14
                } else if d < MID {
                    0.11
                } else {
                    0.085 * (0.6 + 0.8 * clump)
                };
            if r >= p {
                return None;
            }
            Some(if d < VERGE {
                if (r / p) < 0.5 {
                    Kind::Fir
                } else {
                    Kind::Spruce
                }
            } else if d < MID {
                Kind::SpruceMid
            } else {
                Kind::SpruceFar
            })
        }
        Cover::Shrub => {
            if d > MID + 60.0 {
                return None;
            }
            let oak = DENSITY * 0.014 * (0.5 + clump);
            let scrub = DENSITY * 0.10;
            if r < oak {
                Some(if d < MID { Kind::Oak } else { Kind::OakFar })
            } else if r < oak + scrub {
                Some(Kind::Scrub)
            } else {
                None
            }
        }
        Cover::Grass | Cover::Wetland => {
            if d > MID {
                return None;
            }
            let oak = DENSITY * if cover == Cover::Grass { 0.006 } else { 0.0 };
            let scrub = DENSITY * if cover == Cover::Grass { 0.03 } else { 0.05 };
            if r < oak {
                Some(Kind::Oak)
            } else if r < oak + scrub {
                Some(Kind::Scrub)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The roads near (x, z), each's distance from (x, z) and frame there:
/// across from the road, or past an open end from the end itself.
fn each_road(t: &Track, x: f64, z: f64, radius: f64, mut f: impl FnMut(f64, f64, &crate::Frame)) {
    for r in t.net.roads_near(x, z, radius + 2.0) {
        let rt = t.road(r);
        let d = rt.distance_to_road(x, z, radius);
        let Some(s) = d.s else { continue };
        let inside = rt.is_loop || (0.0..=rt.length).contains(&s);
        let fr = rt.frame(if rt.is_loop {
            rt.wrap(s)
        } else {
            s.clamp(0.0, rt.length)
        });
        let dd = if inside {
            d.lat.abs()
        } else {
            kernel::hypot(x - fr.x, z - fr.z)
        };
        f(dd, d.lat, &fr);
    }
}

/// How far (x, z) lies outside the nearest road of the network, from its
/// centreline less its half-width (negative: on the road), measured on the
/// roads within `radius` (the Land kit's `road_clearance`).
pub fn road_clearance(t: &Track, x: f64, z: f64, radius: f64) -> f64 {
    let mut best = f64::INFINITY;
    each_road(t, x, z, radius, |d, _, f| best = best.min(d - f.hw));
    best
}

/// Whether a disc `radius` round (x, z) lies outside every road's walls,
/// with room to spare (the Coast kit's `clear_of_road`).
pub fn wall_clear(t: &Track, x: f64, z: f64, radius: f64) -> bool {
    let mut ok = true;
    each_road(t, x, z, radius + 30.0, |d, lat, f| {
        let wall = if lat < 0.0 { f.wall_l } else { f.wall_r };
        ok &= d - radius >= wall + 0.6;
    });
    ok
}
