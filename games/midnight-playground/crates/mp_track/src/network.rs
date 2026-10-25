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
    cells: BTreeMap<(i64, i64), Vec<(u32, u32)>>,
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
        for (r, rt) in t.roads().enumerate() {
            for k in (0..rt.n).step_by(2) {
                let key = Self::key(f64::from(rt.px[k]), f64::from(rt.pz[k]));
                cells.entry(key).or_default().push((r as u32, k as u32));
            }
        }
        NetIndex { cells }
    }

    /// The roads with a sample within `radius` of (x, z) (and perhaps a
    /// little further: whole cells are taken), in road order.
    pub fn roads_near(&self, x: f64, z: f64, radius: f64) -> Vec<usize> {
        let (a0, b0) = Self::key(x - radius, z - radius);
        let (a1, b1) = Self::key(x + radius, z + radius);
        let mut out: Vec<usize> = Vec::new();
        for a in a0..=a1 {
            for b in b0..=b1 {
                if let Some(v) = self.cells.get(&(a, b)) {
                    for &(r, _) in v {
                        let r = r as usize;
                        if let Err(i) = out.binary_search(&r) {
                            out.insert(i, r);
                        }
                    }
                }
            }
        }
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
        let (a0, b0) = Self::key(x - radius, z - radius);
        let (a1, b1) = Self::key(x + radius, z + radius);
        let r2 = radius * radius;
        let mut out = Vec::new();
        for a in a0..=a1 {
            for b in b0..=b1 {
                let Some(v) = self.cells.get(&(a, b)) else {
                    continue;
                };
                for &(r, k) in v {
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
        let (ca, cb) = Self::key(x, z);
        let rings = (radius / Self::CELL).ceil() as i64 + 1;
        let mut best: Vec<(usize, usize, f64)> = Vec::new();
        let mut nearest = f64::INFINITY;
        for ring in 0..=rings {
            // Every point of this ring's cells is at least this far.
            let reach = (ring - 1).max(0) as f64 * Self::CELL;
            if reach > radius || reach > nearest + slack {
                break;
            }
            for a in ca - ring..=ca + ring {
                for b in cb - ring..=cb + ring {
                    if (a - ca).abs() != ring && (b - cb).abs() != ring {
                        continue;
                    }
                    let Some(v) = self.cells.get(&(a, b)) else {
                        continue;
                    };
                    for &(r, k) in v {
                        let rt = t.road(r as usize);
                        let dx = x - f64::from(rt.px[k as usize]);
                        let dz = z - f64::from(rt.pz[k as usize]);
                        let d = pk_math::kernel::hypot(dx, dz);
                        if d > radius {
                            continue;
                        }
                        nearest = nearest.min(d);
                        match best.iter_mut().find(|e| e.0 == r as usize) {
                            Some(e) => {
                                if d < e.2 || (d == e.2 && (k as usize) < e.1) {
                                    *e = (r as usize, k as usize, d);
                                }
                            }
                            None => best.push((r as usize, k as usize, d)),
                        }
                    }
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
        self.net.roads_near(x, z, 30.0).into_iter().any(|r| {
            if r == road {
                return false;
            }
            let t = self.road(r);
            let d = t.distance_to_road(x, z, 30.0);
            d.s.is_some_and(|s| {
                let f = t.frame(s.clamp(0.0, t.length));
                d.d < f.hw + f.margin + 0.5
            })
        })
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
            for a in t.net.roads_near(x, z, 24.0) {
                if a >= b {
                    continue;
                }
                let at = t.road(a);
                let d = at.distance_to_road(x, z, 24.0);
                let Some(sa) = d.s else { continue };
                // Near the other road's own ends a meeting is that end's
                // junction, not a crossing.
                if !at.is_loop && !(40.0..=at.length - 40.0).contains(&sa) {
                    last.remove(&a);
                    continue;
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
            }
        }
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
