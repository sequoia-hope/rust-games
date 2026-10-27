//! The road network of an open level (docs/vision/peninsula.md, stage 6c):
//! where its roads meet ([`Junction`]), one spatial index over all of them
//! ([`NetIndex`]), and the crossings found from the roads' geometry, where
//! the lesser road fits its surface to the greater's.
//!
//! Road 0 is the level's road and road k its (k - 1)th branch
//! ([`Track::road`]); a road's rank is its index, so the level lists its
//! roads greatest first (the loop or the freeway, then the highways, then
//! the streets) and a road meets only roads listed before it.

use std::collections::BTreeMap;

use pk_math::smoothstep;

use crate::track::{BRANCH_FIT, BRANCH_LIFT, FenceGap, Track};

/// How a road meets a greater one: its start or its far end lies on it,
/// or it crosses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JunctionKind {
    Start,
    End,
    Cross,
}

/// Where road `road` meets the greater road `other` (`other < road`): at
/// `s` metres along `road` and `other_s` along `other`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Junction {
    pub road: usize,
    pub s: f64,
    pub other: usize,
    pub other_s: f64,
    pub kind: JunctionKind,
}

/// Every road's centreline samples (every second one, as each road's own
/// hash keeps them) in cells of [`NetIndex::CELL`] metres, tagged with the
/// road: which roads are near a point, and the nearest place on any of
/// them, without asking every road.
#[derive(Clone, Debug, Default)]
pub struct NetIndex {
    grid: std::sync::Arc<NetGrid>,
}

#[derive(Debug, Default)]
struct NetGrid {
    min_a: i64,
    max_a: i64,
    min_b: i64,
    max_b: i64,
    w: usize,
    h: usize,
    num_roads: usize,
    offsets: Vec<u32>,
    items: Vec<(u32, u32)>,
}

impl NetIndex {
    pub const CELL: f64 = 64.0;

    fn key(x: f64, z: f64) -> (i64, i64) {
        (
            (x / Self::CELL).floor() as i64,
            (z / Self::CELL).floor() as i64,
        )
    }

    pub fn build(t: &Track) -> NetIndex {
        let mut cells: BTreeMap<(i64, i64), Vec<(u32, u32)>> = BTreeMap::new();
        let mut num_roads = 0usize;
        for (r, rt) in t.roads().enumerate() {
            num_roads = r + 1;
            for k in (0..rt.n).step_by(2) {
                let key = Self::key(f64::from(rt.px[k]), f64::from(rt.pz[k]));
                cells.entry(key).or_default().push((r as u32, k as u32));
            }
        }
        let grid = if cells.is_empty() {
            NetGrid::default()
        } else {
            let (mut min_a, mut max_a, mut min_b, mut max_b) =
                (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
            for &(a, b) in cells.keys() {
                min_a = min_a.min(a);
                max_a = max_a.max(a);
                min_b = min_b.min(b);
                max_b = max_b.max(b);
            }
            let w = (max_a - min_a + 1) as usize;
            let h = (max_b - min_b + 1) as usize;
            let mut offsets = vec![0u32; w * h + 1];
            for (&(a, b), v) in &cells {
                let idx = (a - min_a) as usize * h + (b - min_b) as usize;
                offsets[idx + 1] = v.len() as u32;
            }
            for i in 0..w * h {
                offsets[i + 1] += offsets[i];
            }
            let mut items = vec![(0u32, 0u32); offsets[w * h] as usize];
            for (&(a, b), v) in &cells {
                let idx = (a - min_a) as usize * h + (b - min_b) as usize;
                let start = offsets[idx] as usize;
                items[start..start + v.len()].copy_from_slice(v);
            }
            NetGrid {
                min_a,
                max_a,
                min_b,
                max_b,
                w,
                h,
                num_roads,
                offsets,
                items,
            }
        };
        NetIndex {
            grid: std::sync::Arc::new(grid),
        }
    }

    #[inline]
    fn cell_slice(&self, a: i64, b: i64) -> &[(u32, u32)] {
        let g = &*self.grid;
        if g.w == 0 || a < g.min_a || a > g.max_a || b < g.min_b || b > g.max_b {
            return &[];
        }
        let idx = (a - g.min_a) as usize * g.h + (b - g.min_b) as usize;
        let start = g.offsets[idx] as usize;
        let end = g.offsets[idx + 1] as usize;
        &g.items[start..end]
    }

    /// Whether any road has an indexed sample in the cells covering `radius`
    /// round `(x, z)`.
    pub fn has_roads_near(&self, x: f64, z: f64, radius: f64) -> bool {
        let g = &*self.grid;
        if g.w == 0 {
            return false;
        }
        let (a0, b0) = Self::key(x - radius, z - radius);
        let (a1, b1) = Self::key(x + radius, z + radius);
        let (a0, a1) = (a0.max(g.min_a), a1.min(g.max_a));
        let (b0, b1) = (b0.max(g.min_b), b1.min(g.max_b));
        if a0 > a1 || b0 > b1 {
            return false;
        }
        for a in a0..=a1 {
            let row = (a - g.min_a) as usize * g.h;
            for b in b0..=b1 {
                let idx = row + (b - g.min_b) as usize;
                if g.offsets[idx] < g.offsets[idx + 1] {
                    return true;
                }
            }
        }
        false
    }

    /// Calls `f` for each road with a sample in the cells covering `radius`
    /// round `(x, z)`, in ascending road order, stopping early if `f` returns
    /// `false`.
    pub fn for_each_road_near(
        &self,
        x: f64,
        z: f64,
        radius: f64,
        mut f: impl FnMut(usize) -> bool,
    ) {
        let g = &*self.grid;
        if g.w == 0 {
            return;
        }
        let (a0, b0) = Self::key(x - radius, z - radius);
        let (a1, b1) = Self::key(x + radius, z + radius);
        let (a0, a1) = (a0.max(g.min_a), a1.min(g.max_a));
        let (b0, b1) = (b0.max(g.min_b), b1.min(g.max_b));
        if a0 > a1 || b0 > b1 {
            return;
        }
        if g.num_roads <= 64 {
            let mut mask = 0u64;
            for a in a0..=a1 {
                let row = (a - g.min_a) as usize * g.h;
                for b in b0..=b1 {
                    let idx = row + (b - g.min_b) as usize;
                    let start = g.offsets[idx] as usize;
                    let end = g.offsets[idx + 1] as usize;
                    for &(r, _) in &g.items[start..end] {
                        mask |= 1u64 << r;
                    }
                }
            }
            while mask != 0 {
                let r = mask.trailing_zeros() as usize;
                mask &= mask - 1;
                if !f(r) {
                    break;
                }
            }
        } else {
            let mut out: Vec<usize> = Vec::new();
            for a in a0..=a1 {
                let row = (a - g.min_a) as usize * g.h;
                for b in b0..=b1 {
                    let idx = row + (b - g.min_b) as usize;
                    let start = g.offsets[idx] as usize;
                    let end = g.offsets[idx + 1] as usize;
                    for &(r, _) in &g.items[start..end] {
                        let r = r as usize;
                        if let Err(i) = out.binary_search(&r) {
                            out.insert(i, r);
                        }
                    }
                }
            }
            for r in out {
                if !f(r) {
                    break;
                }
            }
        }
    }

    /// The roads with a sample within `radius` of (x, z) (and perhaps a
    /// little further: whole cells are taken), in road order.
    pub fn roads_near(&self, x: f64, z: f64, radius: f64) -> Vec<usize> {
        let mut out = Vec::new();
        self.for_each_road_near(x, z, radius, |r| {
            out.push(r);
            true
        });
        out
    }

    /// Every indexed sample within `radius` of (x, z): `(road, sample, x,
    /// z)`, cell by cell.
    pub fn samples_within(
        &self,
        t: &Track,
        x: f64,
        z: f64,
        radius: f64,
    ) -> Vec<(u32, u32, f64, f64)> {
        let g = &*self.grid;
        let mut out = Vec::new();
        if g.w == 0 {
            return out;
        }
        let (a0, b0) = Self::key(x - radius, z - radius);
        let (a1, b1) = Self::key(x + radius, z + radius);
        let (a0, a1) = (a0.max(g.min_a), a1.min(g.max_a));
        let (b0, b1) = (b0.max(g.min_b), b1.min(g.max_b));
        if a0 > a1 || b0 > b1 {
            return out;
        }
        let r2 = radius * radius;
        for a in a0..=a1 {
            let row = (a - g.min_a) as usize * g.h;
            for b in b0..=b1 {
                let idx = row + (b - g.min_b) as usize;
                let start = g.offsets[idx] as usize;
                let end = g.offsets[idx + 1] as usize;
                for &(r, k) in &g.items[start..end] {
                    let rt = t.road(r as usize);
                    let (sx, sz) = (f64::from(rt.px[k as usize]), f64::from(rt.pz[k as usize]));
                    let (dx, dz) = (x - sx, z - sz);
                    if dx * dx + dz * dz <= r2 {
                        out.push((r, k, sx, sz));
                    }
                }
            }
        }
        out
    }

    /// Each road's nearest indexed sample within `radius` of (x, z), for
    /// the roads whose nearest lies within `slack` of the nearest of all:
    /// `(road, sample, distance)` in road order. The cells are searched in
    /// rings outward from the point's and the search stops once a ring
    /// lies further than the nearest found plus the slack.
    pub fn nearest_samples(
        &self,
        t: &Track,
        x: f64,
        z: f64,
        radius: f64,
        slack: f64,
    ) -> Vec<(usize, usize, f64)> {
        let g = &*self.grid;
        let mut best: Vec<(usize, usize, f64)> = Vec::new();
        if g.w == 0 {
            return best;
        }
        let (ca, cb) = Self::key(x, z);
        let rings = (radius / Self::CELL).ceil() as i64 + 1;
        let mut nearest = f64::INFINITY;
        let visit_cell =
            |a: i64, b: i64, best: &mut Vec<(usize, usize, f64)>, nearest: &mut f64| {
                let v = self.cell_slice(a, b);
                for &(r, k) in v {
                    let rt = t.road(r as usize);
                    let dx = x - f64::from(rt.px[k as usize]);
                    let dz = z - f64::from(rt.pz[k as usize]);
                    if dx.abs() > radius || dz.abs() > radius {
                        continue;
                    }
                    let d = pk_math::kernel::hypot(dx, dz);
                    if d > radius {
                        continue;
                    }
                    *nearest = nearest.min(d);
                    match best.iter_mut().find(|e| e.0 == r as usize) {
                        Some(e) => {
                            if d < e.2 || (d == e.2 && (k as usize) < e.1) {
                                *e = (r as usize, k as usize, d);
                            }
                        }
                        None => best.push((r as usize, k as usize, d)),
                    }
                }
            };
        for ring in 0..=rings {
            // Every point of this ring's cells is at least this far.
            let reach = (ring - 1).max(0) as f64 * Self::CELL;
            if reach > radius || reach > nearest + slack {
                break;
            }
            if ring == 0 {
                visit_cell(ca, cb, &mut best, &mut nearest);
            } else {
                for b in cb - ring..=cb + ring {
                    visit_cell(ca - ring, b, &mut best, &mut nearest);
                }
                for a in (ca - ring + 1)..=(ca + ring - 1) {
                    visit_cell(a, cb - ring, &mut best, &mut nearest);
                    visit_cell(a, cb + ring, &mut best, &mut nearest);
                }
                for b in cb - ring..=cb + ring {
                    visit_cell(ca + ring, b, &mut best, &mut nearest);
                }
            }
        }
        best.retain(|e| e.2 <= nearest + slack);
        best.sort_by_key(|e| e.0);
        best
    }
}

impl Track {
    /// The nearest place on the network to (x, z) within `radius`: the
    /// road and the metres along it, judged by the distance to the place
    /// itself (beyond an open road's end the lateral distance from its
    /// last segment says nothing). Ties go to the lesser road index.
    pub fn nearest_place(&self, x: f64, z: f64, radius: f64) -> Option<(usize, f64)> {
        let mut out: Option<(usize, f64, f64)> = None;
        for (r, k, _) in self.net.nearest_samples(self, x, z, radius, 8.0) {
            let t = self.road(r);
            let p = t.project_window(x, z, k as f64, 4);
            let f = t.frame(if t.is_loop {
                t.wrap(p.s)
            } else {
                p.s.clamp(0.0, t.length)
            });
            let dd = pk_math::kernel::hypot(x - f.x, z - f.z);
            if dd <= radius && out.is_none_or(|b| dd < b.2) {
                out = Some((r, p.s, dd));
            }
        }
        out.map(|(r, s, _)| (r, s))
    }

    /// Whether (x, z) lies on the tarmac of a road other than `road` (its
    /// pavement and half a metre more): where a road's pavement would
    /// cross another road, at a junction's mouth or a crossing, there is
    /// none (stage 7b; the Town kit draws none there either).
    pub fn on_other_tarmac(&self, road: usize, x: f64, z: f64) -> bool {
        let mut hit = false;
        self.net.for_each_road_near(x, z, 30.0, |r| {
            if r == road {
                return true;
            }
            let t = self.road(r);
            let d = t.distance_to_road(x, z, 30.0);
            if d.s.is_some_and(|s| {
                let f = t.frame(s.clamp(0.0, t.length));
                d.d < f.hw + f.margin + 0.5
            }) {
                hit = true;
                return false;
            }
            true
        });
        hit
    }

    /// The open world's ground under (x, z) on road `road` at (s, lat) of
    /// it: the road's profile ([`Track::ground_y_walk`]), with its kerb and
    /// pavement where it has them and no other road's tarmac covers the
    /// place (stage 7b).
    pub fn ground_at(&self, road: usize, x: f64, z: f64, s: f64, lat: f64) -> f64 {
        let t = self.road(road);
        let walk = t.open_ground.is_some() && {
            let f = t.frame(s.clamp(0.0, t.length));
            lat.abs() > f.hw + f.margin && t.walks(s) && !self.on_other_tarmac(road, x, z)
        };
        t.ground_y_walk(x, z, s, lat, walk)
    }

    /// The junctions road `road` takes part in, either side.
    pub fn junctions_of(&self, road: usize) -> impl Iterator<Item = &Junction> {
        self.junctions
            .iter()
            .filter(move |j| j.road == road || j.other == road)
    }
}

/// The crossings: where a road crosses one listed before it, away from
/// the ends it joins the network by. The lesser road's surface is fitted
/// to the greater's over the greater's pavement (lifted a little, as a
/// branch's mouth is) and its own heights return over [`BRANCH_FIT`]
/// either side; both roads' fences open there.
pub fn find_crossings(t: &mut Track) -> Vec<Junction> {
    let mut found: Vec<Junction> = Vec::new();
    for b in 1..=t.branches.len() {
        let bt = &t.branches[b - 1];
        let road = &bt.track;
        // Away from the ends: the junctions there are the road's own.
        let lo = bt.clear + 30.0;
        let hi = if bt.end.is_some() {
            road.length - bt.clear - 30.0
        } else {
            road.length
        };
        // The last side of each nearby greater road the samples were on.
        let mut last: BTreeMap<usize, (usize, f64)> = BTreeMap::new();
        for k in 0..road.n {
            let (x, z) = (f64::from(road.px[k]), f64::from(road.pz[k]));
            t.net.for_each_road_near(x, z, 24.0, |a| {
                if a >= b {
                    return false;
                }
                let at = t.road(a);
                let d = at.distance_to_road(x, z, 24.0);
                let Some(sa) = d.s else { return true };
                // Near the other road's own ends a meeting is that end's
                // junction, not a crossing.
                if !at.is_loop && !(40.0..=at.length - 40.0).contains(&sa) {
                    last.remove(&a);
                    return true;
                }
                if let Some(&(kp, lp)) = last.get(&a)
                    && kp + 1 == k
                    && (lp < 0.0) != (d.lat < 0.0)
                {
                    let s = (k - 1) as f64 + lp.abs() / (lp.abs() + d.lat.abs()).max(1e-9);
                    let near = found
                        .iter()
                        .any(|j| j.road == b && j.other == a && (j.s - s).abs() < 30.0);
                    if s > lo && s < hi && !near {
                        found.push(Junction {
                            road: b,
                            s,
                            other: a,
                            other_s: sa,
                            kind: JunctionKind::Cross,
                        });
                    }
                }
                last.insert(a, (k, d.lat));
                true
            });
        }
    }
    // A freeway meets nothing at grade (stage 8a, D1248): it rises over
    // the road it crosses on a viaduct, and the crossing is no junction.
    let freeway = |t: &Track, r: usize, s: f64| {
        let rt = t.road(r);
        crate::ROAD_TYPES
            .get(rt.road_type[rt.idx(s.clamp(0.0, rt.length))] as usize)
            .is_some_and(|ty| ty.key == "freeway")
    };
    // How much a freeway may rise at `s`: nothing at its own junctions
    // (where a road starts or ends on it, fitted to its surface as it was),
    // then `OVERPASS_GRADE` a metre away from them, past the junction's
    // width.
    let room = |t: &Track, f: usize, s: f64| -> f64 {
        let ft = t.road(f);
        t.junctions
            .iter()
            .filter(|j| j.other == f || j.road == f)
            .map(|j| {
                let at = if j.other == f { j.other_s } else { j.s };
                (ft.ds(at, s).abs() - 25.0).max(0.0) * OVERPASS_GRADE
            })
            .fold(f64::MAX, f64::min)
    };
    let (over, found): (Vec<Junction>, Vec<Junction>) = found.into_iter().partition(|j| {
        if freeway(t, j.road, j.s) == freeway(t, j.other, j.other_s) {
            return false;
        }
        let (f, fs) = if freeway(t, j.road, j.s) {
            (j.road, j.s)
        } else {
            (j.other, j.other_s)
        };
        // On its viaduct already (the level's lift), or room to rise over
        // the road; too near one of its junctions to clear it: at grade.
        let ft = t.road(f);
        ft.elevated[ft.idx(fs.clamp(0.0, ft.length))] != 0 || room(t, f, fs) >= OVERPASS_CLEAR + 1.0
    });
    let mut before: BTreeMap<usize, Vec<f32>> = BTreeMap::new();
    for j in &over {
        let (f, fs, o, os) = if freeway(t, j.road, j.s) {
            (j.road, j.s, j.other, j.other_s)
        } else {
            (j.other, j.other_s, j.road, j.s)
        };
        let base = before
            .entry(f)
            .or_insert_with(|| t.road(f).py.clone())
            .clone();
        lift_over(t, f, fs, o, os, &base);
    }
    t.overpasses.extend(over.iter().copied());
    for (f, own) in before {
        let rt = t.road_mut(f);
        for (i, &y) in own.iter().enumerate() {
            if f64::from(rt.py[i]) - f64::from(y) > ELEVATED_LIFT {
                rt.elevated[i] = 1;
            }
        }
        rt.grade = crate::track::grade_of(&rt.py, rt.is_loop);
    }
    for j in &found {
        let py = fit_crossing(t.road(j.road), t.road(j.other), j.s, j.other_s);
        let w = 3.0;
        let mf = t.road(j.road).frame(j.s);
        let of = t.road(j.other).frame(j.other_s);
        let (mw, ow) = (mf.hw + mf.margin, of.hw + of.margin);
        let mine = t.road_mut(j.road);
        mine.py = py;
        for side in [-1.0, 1.0] {
            mine.fence_gaps.push(FenceGap {
                s0: j.s - ow - w * 4.0,
                s1: j.s + ow + w * 4.0,
                side,
            });
        }
        let other = t.road_mut(j.other);
        for side in [-1.0, 1.0] {
            other.fence_gaps.push(FenceGap {
                s0: j.other_s - mw - w * 4.0,
                s1: j.other_s + mw + w * 4.0,
                side,
            });
        }
    }
    found
}

/// How far a freeway's deck stands over a road it crosses (m), and the
/// steepest its ramps up to it climb.
pub const OVERPASS_CLEAR: f64 = 7.0;
pub const OVERPASS_GRADE: f64 = 0.045;
/// How much higher than its own ground a road must stand to count as
/// elevated: the terrain is not raised to it, and the road kit puts it on
/// piers.
pub const ELEVATED_LIFT: f64 = 1.5;

/// Lifts freeway `f` over road `o` where they cross (stage 8a): at its
/// crossing `fs` the deck stands `OVERPASS_CLEAR` over the other road's
/// surface across that road's width (wider as the angle narrows), and on
/// either side it comes down at `OVERPASS_GRADE` to its own heights. Lifts
/// only ever raise, so those of crossings close together merge into one
/// viaduct; where they raise it past `ELEVATED_LIFT` it is elevated
/// (`find_crossings`).
fn lift_over(t: &mut Track, f: usize, fs: f64, o: usize, os: f64, base: &[f32]) {
    let (deck, plateau) = {
        let ot = t.road(o);
        let of = ot.frame(os.clamp(0.0, ot.length));
        let ff = t.road(f).frame(fs);
        let cos = ff.fx * of.fx + ff.fz * of.fz;
        let sin = (1.0 - cos * cos).max(0.09).sqrt();
        (
            ot.surface_y(os, 0.0) + OVERPASS_CLEAR,
            (of.hw + of.margin) / sin + 12.0,
        )
    };
    // Never more than it may rise by its own junctions, over its heights
    // before any crossing lifted it (`base`): two crossings near one
    // junction must not stack their room.
    let ats: Vec<f64> = t
        .junctions
        .iter()
        .filter(|j| j.other == f || j.road == f)
        .map(|j| if j.other == f { j.other_s } else { j.s })
        .collect();
    let rt = t.road_mut(f);
    for i in 0..rt.n {
        let s = i as f64;
        let d = rt.ds(fs, s).abs();
        let room = ats
            .iter()
            .map(|&a| (rt.ds(a, s).abs() - 25.0).max(0.0) * OVERPASS_GRADE)
            .fold(f64::MAX, f64::min);
        let need = (deck - (d - plateau).max(0.0) * OVERPASS_GRADE).min(f64::from(base[i]) + room);
        if need > f64::from(rt.py[i]) {
            rt.py[i] = need as f32;
        }
    }
}

/// Fits the lesser road's heights round `s` to the greater road's surface
/// where it lies on the greater's pavement, and returns its own heights
/// over [`BRANCH_FIT`] either side: the lesser road's new heights.
fn fit_crossing(mine: &Track, other: &Track, s: f64, other_s: f64) -> Vec<f32> {
    let mut py = mine.py.clone();
    let n = mine.n as i64;
    let k0 = s.round() as i64;
    for dir in [-1i64, 1] {
        let mut hint = other_s;
        let mut delta = 0.0;
        let mut edge: Option<f64> = None;
        let mut k = if dir < 0 { k0 } else { k0 + 1 };
        while (0..n).contains(&k) {
            let i = k as usize;
            let sk = k as f64;
            let (x, z) = (f64::from(mine.px[i]), f64::from(mine.pz[i]));
            match edge {
                None => {
                    let q = other.project(x, z, hint);
                    hint = q.s;
                    let f = other.frame(q.s);
                    let on = q.lat.abs() <= f.hw + f.margin + 1.0;
                    if on {
                        let lift = BRANCH_LIFT;
                        let fitted = other.surface_y(q.s, q.lat) + lift;
                        delta = fitted - f64::from(py[i]);
                        py[i] = fitted as f32;
                    } else {
                        edge = Some(sk);
                        let w = 1.0;
                        py[i] = (f64::from(py[i]) + delta * w) as f32;
                    }
                }
                Some(e) => {
                    let w = 1.0 - smoothstep(0.0, BRANCH_FIT, (sk - e).abs());
                    if w <= 0.0 {
                        break;
                    }
                    py[i] = (f64::from(py[i]) + delta * w) as f32;
                }
            }
            k += dir;
        }
    }
    py
}
