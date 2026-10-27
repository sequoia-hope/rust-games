//! The buildings of an open level's towns and cities
//! (docs/vision/peninsula.md, stage 6e): footprints the level places from
//! its network ([`crate::Level::buildings`]), which the simulation
//! collides cars with and world generation draws. They are the
//! simulation's, so a multiplayer race agrees on every wall.

use std::collections::BTreeMap;

/// A building's footprint: a box on the ground, `hx` by `hz` either side
/// of its centre along its own axes (`ux`, `uz` the unit x axis; the z
/// axis is `(-uz, ux)`), standing `height` metres over `y`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub x: f64,
    pub z: f64,
    pub hx: f64,
    pub hz: f64,
    pub ux: f64,
    pub uz: f64,
    /// The ground under it (the lowest corner's) and its height.
    pub y: f64,
    pub height: f64,
    /// What kind of building world generation draws (the level's own
    /// numbering) and a seed for its looks.
    pub style: u8,
    pub seed: u32,
    /// A parking garage's decks, which the car drives into and up (stage
    /// 8c, [`crate::garage`]); `None` for a building that stands solid.
    pub garage: Option<crate::garage::Garage>,
}

impl Footprint {
    /// (x, z) in the footprint's own frame.
    pub fn local(&self, x: f64, z: f64) -> (f64, f64) {
        let (dx, dz) = (x - self.x, z - self.z);
        (dx * self.ux + dz * self.uz, -dx * self.uz + dz * self.ux)
    }

    /// The radius of a circle round it.
    pub fn radius(&self) -> f64 {
        (self.hx * self.hx + self.hz * self.hz).sqrt()
    }

    /// The four corners, counter-clockwise from (-hx, -hz).
    pub fn corners(&self) -> [[f64; 2]; 4] {
        let p = |a: f64, b: f64| {
            [
                self.x + a * self.ux - b * self.uz,
                self.z + a * self.uz + b * self.ux,
            ]
        };
        [
            p(-self.hx, -self.hz),
            p(self.hx, -self.hz),
            p(self.hx, self.hz),
            p(-self.hx, self.hz),
        ]
    }
}

/// The level's buildings and an index of them by place.
#[derive(Clone, Debug, Default)]
pub struct Buildings {
    pub list: Vec<Footprint>,
    grid: BuildingGrid,
    has_garages: bool,
}

#[derive(Clone, Debug, Default)]
struct BuildingGrid {
    min_a: i64,
    max_a: i64,
    min_b: i64,
    max_b: i64,
    w: usize,
    h: usize,
    offsets: Vec<u32>,
    items: Vec<u32>,
}

/// How a box pushes out of a building: the direction (unit, horizontal)
/// and how far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Push {
    pub nx: f64,
    pub nz: f64,
    pub depth: f64,
    /// The building's index in [`Buildings::list`].
    pub id: usize,
}

impl Buildings {
    pub const CELL: f64 = 32.0;

    fn key(x: f64, z: f64) -> (i64, i64) {
        (
            (x / Self::CELL).floor() as i64,
            (z / Self::CELL).floor() as i64,
        )
    }

    pub fn new(list: Vec<Footprint>) -> Buildings {
        let mut cells: BTreeMap<(i64, i64), Vec<u32>> = BTreeMap::new();
        let mut has_garages = false;
        for (i, f) in list.iter().enumerate() {
            has_garages |= f.garage.is_some();
            let r = f.radius();
            let (a0, b0) = Self::key(f.x - r, f.z - r);
            let (a1, b1) = Self::key(f.x + r, f.z + r);
            for a in a0..=a1 {
                for b in b0..=b1 {
                    cells.entry((a, b)).or_default().push(i as u32);
                }
            }
        }
        let grid = if cells.is_empty() {
            BuildingGrid::default()
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
            let mut items = vec![0u32; offsets[w * h] as usize];
            for (&(a, b), v) in &cells {
                let idx = (a - min_a) as usize * h + (b - min_b) as usize;
                let start = offsets[idx] as usize;
                items[start..start + v.len()].copy_from_slice(v);
            }
            BuildingGrid {
                min_a,
                max_a,
                min_b,
                max_b,
                w,
                h,
                offsets,
                items,
            }
        };
        Buildings {
            list,
            grid,
            has_garages,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    #[inline]
    fn cell_slice(&self, a: i64, b: i64) -> &[u32] {
        let g = &self.grid;
        if g.w == 0 || a < g.min_a || a > g.max_a || b < g.min_b || b > g.max_b {
            return &[];
        }
        let idx = (a - g.min_a) as usize * g.h + (b - g.min_b) as usize;
        &g.items[g.offsets[idx] as usize..g.offsets[idx + 1] as usize]
    }

    fn for_each_near(&self, x: f64, z: f64, r: f64, mut f: impl FnMut(usize) -> bool) {
        let g = &self.grid;
        if g.w == 0 {
            return;
        }
        let (a0, b0) = Self::key(x - r, z - r);
        let (a1, b1) = Self::key(x + r, z + r);
        if a0 == a1 && b0 == b1 {
            for &i in self.cell_slice(a0, b0) {
                if !f(i as usize) {
                    break;
                }
            }
            return;
        }
        let (a0, a1) = (a0.max(g.min_a), a1.min(g.max_a));
        let (b0, b1) = (b0.max(g.min_b), b1.min(g.max_b));
        if a0 > a1 || b0 > b1 {
            return;
        }
        let mut stack = [0usize; 32];
        let mut len = 0usize;
        let mut heap: Option<Vec<usize>> = None;
        for a in a0..=a1 {
            let row = (a - g.min_a) as usize * g.h;
            for b in b0..=b1 {
                let idx = row + (b - g.min_b) as usize;
                for &i in &g.items[g.offsets[idx] as usize..g.offsets[idx + 1] as usize] {
                    let i = i as usize;
                    if let Some(v) = &mut heap {
                        if let Err(k) = v.binary_search(&i) {
                            v.insert(k, i);
                        }
                    } else if let Err(k) = stack[..len].binary_search(&i) {
                        if len < stack.len() {
                            stack.copy_within(k..len, k + 1);
                            stack[k] = i;
                            len += 1;
                        } else {
                            let mut v = Vec::with_capacity(64);
                            v.extend_from_slice(&stack[..len]);
                            v.insert(k, i);
                            heap = Some(v);
                        }
                    }
                }
            }
        }
        let ids: &[usize] = match &heap {
            Some(v) => v,
            None => &stack[..len],
        };
        for &i in ids {
            if !f(i) {
                break;
            }
        }
    }

    /// The buildings whose cells reach within `r` of (x, z), each once,
    /// in index order.
    pub fn near(&self, x: f64, z: f64, r: f64) -> Vec<usize> {
        let mut out = Vec::new();
        self.for_each_near(x, z, r, |i| {
            out.push(i);
            true
        });
        out
    }

    /// Whether (x, z) is inside a building.
    pub fn inside(&self, x: f64, z: f64) -> bool {
        let (a, b) = Self::key(x, z);
        self.cell_slice(a, b).iter().any(|&i| {
            let f = &self.list[i as usize];
            if f.garage.is_some() {
                return false;
            }
            let (lx, lz) = f.local(x, z);
            lx.abs() < f.hx && lz.abs() < f.hz
        })
    }

    /// The deepest overlap of a car's box (centre (x, z), half width `hw`
    /// across and half length `hl` along `yaw`) with a building, and the
    /// shortest way out of it (separating axes over both boxes' edges).
    pub fn push_out(
        &self,
        x: f64,
        z: f64,
        yaw_c: f64,
        yaw_s: f64,
        hw: f64,
        hl: f64,
    ) -> Option<Push> {
        if self.list.is_empty() {
            return None;
        }
        let reach = (hw * hw + hl * hl).sqrt();
        let mut best: Option<Push> = None;
        self.for_each_near(x, z, reach, |i| {
            let f = &self.list[i];
            // A garage is not solid: its walls and decks are its own.
            if f.garage.is_some() {
                return true;
            }
            let (dx, dz) = (x - f.x, z - f.z);
            let rr = reach + f.radius();
            if dx * dx + dz * dz > rr * rr {
                return true;
            }
            // The car's axes: forward (c, s), right (-s, c).
            let axes = [(f.ux, f.uz), (-f.uz, f.ux), (yaw_c, yaw_s), (-yaw_s, yaw_c)];
            let mut min: Option<(f64, f64, f64)> = None;
            let mut apart = false;
            for (ax, az) in axes {
                // Half extents projected on the axis.
                let rb =
                    f.hx * (f.ux * ax + f.uz * az).abs() + f.hz * (-f.uz * ax + f.ux * az).abs();
                let rc =
                    hl * (yaw_c * ax + yaw_s * az).abs() + hw * (-yaw_s * ax + yaw_c * az).abs();
                let d = dx * ax + dz * az;
                let over = rb + rc - d.abs();
                if over <= 0.0 {
                    apart = true;
                    break;
                }
                if min.is_none_or(|m| over < m.0) {
                    let sg = if d >= 0.0 { 1.0 } else { -1.0 };
                    min = Some((over, ax * sg, az * sg));
                }
            }
            if apart {
                return true;
            }
            if let Some((depth, nx, nz)) = min
                && best.is_none_or(|b| depth > b.depth)
            {
                best = Some(Push {
                    nx,
                    nz,
                    depth,
                    id: i,
                });
            }
            true
        });
        best
    }

    /// The ground in a garage at (x, z) for a car at height `y`, if a
    /// garage stands there.
    pub fn garage_ground(&self, x: f64, z: f64, y: f64) -> Option<f64> {
        if !self.has_garages {
            return None;
        }
        let (ca, cb) = Self::key(x, z);
        self.cell_slice(ca, cb).iter().find_map(|&i| {
            let f = &self.list[i as usize];
            let g = f.garage.as_ref()?;
            let (a, b) = f.local(x, z);
            (a.abs() <= f.hx && b.abs() <= f.hz).then(|| g.ground(f.hz, a, b, y))
        })
    }

    /// The walls of the garages within `r` of (x, z).
    pub fn garage_walls(&self, x: f64, z: f64, r: f64) -> Vec<crate::garage::Wall> {
        if !self.has_garages {
            return Vec::new();
        }
        let mut out = Vec::new();
        self.for_each_near(x, z, r, |i| {
            if self.list[i].garage.is_some() {
                out.extend(self.list[i].garage_walls());
            }
            true
        });
        out
    }

    /// The face of building `id` that (x, z) is furthest outside of (or
    /// least inside): a point on it and its outward normal, for a
    /// half-space collider.
    pub fn face(&self, id: usize, x: f64, z: f64) -> ([f64; 2], [f64; 2]) {
        let f = &self.list[id];
        let (a, b) = f.local(x, z);
        let faces = [
            (a - f.hx, [f.ux, f.uz], f.hx),
            (-a - f.hx, [-f.ux, -f.uz], f.hx),
            (b - f.hz, [-f.uz, f.ux], f.hz),
            (-b - f.hz, [f.uz, -f.ux], f.hz),
        ];
        let mut best = faces[0];
        for fc in faces {
            if fc.0 > best.0 {
                best = fc;
            }
        }
        let n = best.1;
        ([f.x + n[0] * best.2, f.z + n[1] * best.2], n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, z: f64, h: f64, yaw: f64) -> Footprint {
        Footprint {
            x,
            z,
            hx: h,
            hz: h,
            ux: pk_math::kernel::cos(yaw),
            uz: pk_math::kernel::sin(yaw),
            y: 0.0,
            height: 10.0,
            style: 0,
            seed: 0,
            garage: None,
        }
    }

    #[test]
    fn a_car_pushes_out_of_the_nearest_face() {
        let b = Buildings::new(vec![square(0.0, 0.0, 10.0, 0.0)]);
        // A car facing +x whose nose is 0.5 m into the west face.
        let p = b.push_out(-11.5, 0.0, 1.0, 0.0, 1.0, 2.0).unwrap();
        assert!((p.depth - 0.5).abs() < 1e-9, "{p:?}");
        assert!((p.nx + 1.0).abs() < 1e-9 && p.nz.abs() < 1e-9);
        // Clear of it: nothing.
        assert!(b.push_out(-12.5, 0.0, 1.0, 0.0, 1.0, 2.0).is_none());
        assert!(b.inside(3.0, -4.0) && !b.inside(10.5, 0.0));
        let (pt, n) = b.face(0, 0.0, 30.0);
        assert_eq!((pt, n), ([0.0, 10.0], [0.0, 1.0]));
    }

    #[test]
    fn a_turned_building_is_found_by_its_cells_and_its_axes() {
        let q = std::f64::consts::FRAC_PI_4;
        let b = Buildings::new(vec![
            square(100.0, 100.0, 10.0, q),
            square(400.0, 0.0, 5.0, 0.0),
        ]);
        assert_eq!(b.near(100.0, 100.0, 1.0), vec![0]);
        // Turned 45 degrees, its own x axis is the diagonal and its face is
        // 10 m out along it: a car a metre either way centred 10.5 m out
        // overlaps it, and one 11.5 m out is clear.
        let d = 10.5 / 2f64.sqrt();
        let (c, sn) = (pk_math::kernel::cos(q), pk_math::kernel::sin(q));
        let p = b.push_out(100.0 + d, 100.0 + d, c, sn, 1.0, 1.0);
        assert!((p.unwrap().depth - 0.5).abs() < 1e-9, "{p:?}");
        let d = 11.5 / 2f64.sqrt();
        assert!(b.push_out(100.0 + d, 100.0 + d, c, sn, 1.0, 1.0).is_none());
        assert!(
            b.push_out(100.0 + 12.0, 100.0 - 12.0, 1.0, 0.0, 1.0, 1.0)
                .is_none()
        );
    }
}
