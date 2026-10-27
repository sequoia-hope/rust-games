//! The buildings of an open level's towns and cities
//! (docs/vision/peninsula.md, stage 6e), placed from the built road
//! network and a field of how built-up the real ground is and how tall it
//! stands (the atlas's city grid). The roads are half size; the buildings
//! are full size, so the real density is drawn with fewer of them.
//!
//! Two passes. **Frontage:** along every street (not the freeways), rows
//! of lots either side behind a sidewalk, as wide and deep as the kind of
//! building the cell's heights call for. **Infill:** a jittered grid over
//! the built-up land between the streets, each building turned to the
//! nearest road, so a downtown's towers stand between its streets too.
//! Every lot is kept clear of every road's pavement and sidewalk and of
//! the buildings already placed. The placement is the simulation's: a
//! plain function of the network, the field and the seed, with exact
//! maths only.

use std::collections::BTreeMap;

use mp_track::{Footprint, GroundFn, Track};
use pk_math::{Mulberry32, kernel};

/// What a cell of the field says: the share of the ground built on
/// (0..1) and the median and ninetieth-percentile heights there (real
/// metres).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Density {
    pub coverage: f64,
    pub p50: f64,
    pub p90: f64,
}

pub type DensityFn = dyn Fn(f64, f64) -> Density + Send + Sync;

/// A building's seed, drawn from the stream.
fn seed_of(rng: &mut Mulberry32) -> u32 {
    (rng.next_f64() * 4_294_967_296.0) as u32
}

/// The kinds of building, as [`Footprint::style`] carries them.
pub mod style {
    pub const HOUSE: u8 = 0;
    pub const LOW: u8 = 1;
    pub const MID: u8 = 2;
    pub const TOWER: u8 = 3;
    /// A parking garage (stage 8c): driven into, not stood against.
    pub const GARAGE: u8 = 4;
}

/// Where a lot may be a parking garage (built ground at least this share)
/// and how often one is; its size along the street and deep, and decks.
const GARAGE_COVER: f64 = 0.4;
const GARAGE_ODDS: f64 = 0.05;
const GARAGE_W: f64 = 48.0;
const GARAGE_D: f64 = 40.0;
const GARAGE_DECKS: u8 = 4;

/// The sidewalk between the paved edge and the lots (m).
pub const WALK: f64 = 4.0;
/// Below this share of built ground nothing is placed.
const SPARSE: f64 = 0.05;
/// The infill grid's spacing (m) and the share it needs.
const INFILL: f64 = 46.0;
const INFILL_COVER: f64 = 0.18;

struct Placer<'a> {
    t: &'a Track,
    density: &'a DensityFn,
    ground: &'a GroundFn,
    out: Vec<Footprint>,
    cells: BTreeMap<(i64, i64), Vec<u32>>,
    /// The first road that gets buildings: the ones before it are left
    /// as their own kits draw them.
    first: usize,
}

const CELL: f64 = 64.0;

/// The road types whose sides have no lots.
fn bare(road_type: u8) -> bool {
    mp_track::ROAD_TYPES
        .get(road_type as usize)
        .is_some_and(|r| r.key == "freeway")
}

/// A height for a cell: between its median and its ninetieth percentile,
/// more often low, now and then a little over.
fn height(d: &Density, rng: &mut Mulberry32) -> f64 {
    let u = rng.next_f64();
    let lo = d.p50.max(4.0);
    let hi = d.p90.max(lo);
    let mut h = lo + (hi - lo) * u * u;
    if rng.next_f64() < 0.04 {
        h = hi * (1.0 + 0.15 * rng.next_f64());
    }
    h.max(4.0)
}

fn kind(h: f64, d: &Density) -> u8 {
    if h < 8.0 && d.coverage < 0.35 {
        style::HOUSE
    } else if h < 14.0 {
        style::LOW
    } else if h < 45.0 {
        style::MID
    } else {
        style::TOWER
    }
}

/// A lot's width along the street and depth back from it, by kind.
fn size(k: u8, rng: &mut Mulberry32) -> (f64, f64) {
    let r = |rng: &mut Mulberry32, a: f64, b: f64| a + (b - a) * rng.next_f64();
    match k {
        style::HOUSE => (r(rng, 9.0, 14.0), r(rng, 10.0, 15.0)),
        style::LOW => (r(rng, 10.0, 26.0), r(rng, 12.0, 24.0)),
        style::MID => (r(rng, 18.0, 36.0), r(rng, 18.0, 32.0)),
        _ => (r(rng, 26.0, 46.0), r(rng, 24.0, 42.0)),
    }
}

/// The gap left before the next lot along the street.
fn gap(k: u8, rng: &mut Mulberry32) -> f64 {
    match k {
        style::HOUSE => 4.0 + 5.0 * rng.next_f64(),
        style::LOW => 0.5 + 3.0 * rng.next_f64(),
        _ => 1.0 + 6.0 * rng.next_f64(),
    }
}

impl Placer<'_> {
    fn key(x: f64, z: f64) -> (i64, i64) {
        ((x / CELL).floor() as i64, (z / CELL).floor() as i64)
    }

    /// How far past the nearest road's paved edge (x, z) lies (negative:
    /// on the pavement), out to `reach`.
    fn clear_of_roads(&self, x: f64, z: f64, reach: f64) -> f64 {
        let mut best = reach;
        for (r, k, _) in self
            .t
            .net
            .nearest_samples(self.t, x, z, reach + 12.0, reach)
        {
            let rt = self.t.road(r);
            let p = rt.project_window(x, z, k as f64, 4);
            // Past an open road's end the lateral distance says nothing.
            let s = if rt.is_loop {
                rt.wrap(p.s)
            } else {
                p.s.clamp(0.0, rt.length)
            };
            let f = rt.frame(s);
            let d = kernel::hypot(x - f.x, z - f.z) - f.hw - f.margin;
            best = best.min(d);
        }
        best
    }

    /// Whether `rt` at station `u` is in a zone where city buildings are
    /// placed: any zone when the level has no `Town` zones (synthetic
    /// tests), else `Town` zones only (leaving `Land`, `Coast` and `Beach`
    /// stretches to their own scenery kits).
    fn builds_here(&self, rt: &Track, u: f64) -> bool {
        if !self.t.zones.iter().any(|zn| zn.zone.scenery == "Town") {
            return true;
        }
        let z = rt.zone[rt.idx(u)] as usize;
        rt.zones
            .iter()
            .find(|zn| zn.id == z)
            .is_some_and(|zn| zn.zone.scenery == "Town")
    }

    /// Whether the footprint stands clear of every road's sidewalk, and
    /// away from the pre-bayside roads (`0..first`) whose own kits draw
    /// their towns.
    fn clear(&self, f: &Footprint) -> bool {
        if self.first > 0
            && self
                .t
                .nearest_place(f.x, f.z, 250.0)
                .is_some_and(|(nr, _)| nr < self.first)
        {
            return false;
        }
        let c = f.corners();
        let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let pts = [
            c[0],
            c[1],
            c[2],
            c[3],
            mid(c[0], c[1]),
            mid(c[1], c[2]),
            mid(c[2], c[3]),
            mid(c[3], c[0]),
            [f.x, f.z],
        ];
        pts.iter()
            .all(|p| self.clear_of_roads(p[0], p[1], WALK + 1.0) >= WALK)
    }

    /// Whether it overlaps a building already placed (with `pad` metres
    /// between).
    fn overlaps(&self, f: &Footprint, pad: f64) -> bool {
        let r = f.radius() + pad;
        let (a0, b0) = Self::key(f.x - r, f.z - r);
        let (a1, b1) = Self::key(f.x + r, f.z + r);
        for a in a0..=a1 {
            for b in b0..=b1 {
                let Some(v) = self.cells.get(&(a, b)) else {
                    continue;
                };
                for &i in v {
                    if boxes_overlap(f, &self.out[i as usize], pad) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn push(&mut self, f: Footprint) {
        let i = self.out.len() as u32;
        let r = f.radius();
        let (a0, b0) = Self::key(f.x - r, f.z - r);
        let (a1, b1) = Self::key(f.x + r, f.z + r);
        for a in a0..=a1 {
            for b in b0..=b1 {
                self.cells.entry((a, b)).or_default().push(i);
            }
        }
        self.out.push(f);
    }

    /// A footprint centred at (x, z), its x axis along (ux, uz), standing
    /// on the lowest ground under its corners.
    #[allow(clippy::too_many_arguments)]
    fn footprint(
        &self,
        x: f64,
        z: f64,
        ux: f64,
        uz: f64,
        w: f64,
        d: f64,
        h: f64,
        k: u8,
        seed: u32,
    ) -> Footprint {
        let mut f = Footprint {
            x,
            z,
            hx: w / 2.0,
            hz: d / 2.0,
            ux,
            uz,
            y: 0.0,
            height: h,
            style: k,
            seed,
            garage: None,
        };
        let mut lo = f64::INFINITY;
        for c in f.corners() {
            lo = lo.min((self.ground)(c[0], c[1]));
        }
        f.y = lo;
        f
    }

    /// Rows of lots along both sides of road `r`.
    fn frontage(&mut self, r: usize, seed: u32) {
        let t = self.t.road(r);
        for (si, side) in [-1.0f64, 1.0].into_iter().enumerate() {
            let mut rng = Mulberry32::new(
                seed ^ (r as u32).wrapping_mul(0x9e37_79b9) ^ (si as u32 * 0x85eb_ca6b),
            );
            let mut u = 0.0;
            while u < t.length {
                let f0 = t.frame(u);
                if !self.builds_here(t, u) || bare(t.road_type[t.idx(u)]) {
                    u += 20.0;
                    continue;
                }
                let edge = f0.hw + f0.margin + WALK;
                let probe = (
                    f0.x + f0.rx * side * (edge + 12.0),
                    f0.z + f0.rz * side * (edge + 12.0),
                );
                let dn = (self.density)(probe.0, probe.1);
                if dn.coverage < SPARSE {
                    u += 24.0;
                    continue;
                }
                // Fewer lots where less of the ground is built on.
                if rng.next_f64() > 0.35 + dn.coverage * 1.6 {
                    u += 10.0 + 10.0 * rng.next_f64();
                    continue;
                }
                // A parking garage now and then where the ground is built
                // up (stage 8c), chosen by a draw of its own so the lots
                // round it are as they were.
                let pick = Mulberry32::new(
                    seed ^ 0x6a12_0f5b
                        ^ (r as u32).wrapping_mul(0x2c1b_3c6d)
                        ^ (u as u32).wrapping_mul(0x297a_2d39)
                        ^ si as u32,
                )
                .next_f64();
                let garage = (t.open_ground.is_none() || t.walks(u))
                    && dn.coverage > GARAGE_COVER
                    && pick < GARAGE_ODDS;
                let h = height(&dn, &mut rng);
                let k = kind(h, &dn);
                let (w, d) = size(k, &mut rng);
                let (h, k, w, d) = if garage {
                    let top = f64::from(GARAGE_DECKS - 1) * mp_track::garage::DECK_H;
                    (
                        top + mp_track::garage::PARAPET,
                        style::GARAGE,
                        GARAGE_W,
                        GARAGE_D,
                    )
                } else {
                    (h, k, w, d)
                };
                let setback = if k == style::HOUSE {
                    3.0 + 4.0 * rng.next_f64()
                } else {
                    0.3
                };
                let mid = (u + w / 2.0).min(t.length);
                let f = t.frame(mid);
                let lat = side * (f.hw + f.margin + WALK + setback + d / 2.0);
                let fp = self.footprint(
                    f.x + f.rx * lat,
                    f.z + f.rz * lat,
                    f.fx,
                    f.fz,
                    w,
                    d,
                    h,
                    k,
                    seed_of(&mut rng),
                );
                let mut fp = fp;
                if garage {
                    // Its floor the pavement's height, its front the
                    // street's side (the footprint's b runs to the road's
                    // right).
                    fp.garage = Some(mp_track::garage::Garage {
                        decks: GARAGE_DECKS,
                        front: -side,
                        floor: t.walk_y(mid, side),
                    });
                }
                if self.clear(&fp) && !self.overlaps(&fp, 0.4) {
                    self.push(fp);
                    // A second row behind where the ground is built up.
                    if k != style::HOUSE
                        && k != style::GARAGE
                        && dn.coverage > 0.3
                        && rng.next_f64() < dn.coverage * 1.4
                    {
                        let h2 = height(&dn, &mut rng);
                        let k2 = kind(h2, &dn).max(style::LOW);
                        let (_, d2) = size(k2, &mut rng);
                        let lat2 = side * (lat.abs() + d / 2.0 + 2.0 + d2 / 2.0);
                        let fp2 = self.footprint(
                            f.x + f.rx * lat2,
                            f.z + f.rz * lat2,
                            f.fx,
                            f.fz,
                            w,
                            d2,
                            h2,
                            k2,
                            seed_of(&mut rng),
                        );
                        if self.clear(&fp2) && !self.overlaps(&fp2, 0.4) {
                            self.push(fp2);
                        }
                    }
                }
                u += w + gap(k, &mut rng);
            }
        }
    }

    /// The built-up land between the streets, on a jittered grid.
    fn infill(&mut self, seed: u32) {
        let t = self.t;
        let mut b = t.bounds;
        for r in t.roads() {
            b.min_x = b.min_x.min(r.bounds.min_x);
            b.max_x = b.max_x.max(r.bounds.max_x);
            b.min_z = b.min_z.min(r.bounds.min_z);
            b.max_z = b.max_z.max(r.bounds.max_z);
        }
        let i0 = (b.min_x / INFILL).floor() as i64;
        let i1 = (b.max_x / INFILL).ceil() as i64;
        let j0 = (b.min_z / INFILL).floor() as i64;
        let j1 = (b.max_z / INFILL).ceil() as i64;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let cell_seed = seed
                    ^ (i as u32).wrapping_mul(0x27d4_eb2d)
                    ^ (j as u32).wrapping_mul(0x1656_67b1);
                let mut rng = Mulberry32::new(cell_seed);
                let x = (i as f64 + 0.2 + 0.6 * rng.next_f64()) * INFILL;
                let z = (j as f64 + 0.2 + 0.6 * rng.next_f64()) * INFILL;
                let dn = (self.density)(x, z);
                if dn.coverage < INFILL_COVER || rng.next_f64() > dn.coverage * 2.2 {
                    continue;
                }
                // Turned to the nearest road, within reach of one.
                let near = t.net.nearest_samples(t, x, z, 700.0, 0.0);
                let Some(&(r, k, _)) = near.first() else {
                    continue;
                };
                if r < self.first {
                    continue;
                }
                let rt = t.road(r);
                if !self.builds_here(rt, k as f64) {
                    continue;
                }
                let f = rt.frame(k as f64);
                let h = height(&dn, &mut rng);
                let kd = kind(h, &dn);
                let (w, d) = size(kd, &mut rng);
                let fp = self.footprint(x, z, f.fx, f.fz, w, d, h, kd, seed_of(&mut rng));
                if self.clear(&fp) && !self.overlaps(&fp, 3.0) {
                    self.push(fp);
                }
            }
        }
    }
}

/// Whether two footprints, each grown by `pad / 2`, overlap (separating
/// axes over both boxes' edges).
pub fn boxes_overlap(a: &Footprint, b: &Footprint, pad: f64) -> bool {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let rr = a.radius() + b.radius() + pad;
    if dx * dx + dz * dz > rr * rr {
        return false;
    }
    let (ahx, ahz, bhx, bhz) = (
        a.hx + pad / 2.0,
        a.hz + pad / 2.0,
        b.hx + pad / 2.0,
        b.hz + pad / 2.0,
    );
    for (ax, az) in [(a.ux, a.uz), (-a.uz, a.ux), (b.ux, b.uz), (-b.uz, b.ux)] {
        let ra = ahx * (a.ux * ax + a.uz * az).abs() + ahz * (-a.uz * ax + a.ux * az).abs();
        let rb = bhx * (b.ux * ax + b.uz * az).abs() + bhz * (-b.uz * ax + b.ux * az).abs();
        if (dx * ax + dz * az).abs() >= ra + rb {
            return false;
        }
    }
    true
}

/// The buildings of the network: frontage along every street from road
/// `first` on, then the infill nearest those streets.
pub fn place(
    t: &Track,
    density: &DensityFn,
    ground: &GroundFn,
    seed: u32,
    first: usize,
) -> Vec<Footprint> {
    let mut p = Placer {
        t,
        density,
        ground,
        out: Vec::new(),
        cells: BTreeMap::new(),
        first,
    };
    for r in first..t.roads().count() {
        p.frontage(r, seed);
    }
    p.infill(seed ^ 0x5bd1_e995);
    p.out
}
