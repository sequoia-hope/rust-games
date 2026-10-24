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
    cells: BTreeMap<(i64, i64), Vec<u32>>,
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
        for (i, f) in list.iter().enumerate() {
            let r = f.radius();
            let (a0, b0) = Self::key(f.x - r, f.z - r);
            let (a1, b1) = Self::key(f.x + r, f.z + r);
            for a in a0..=a1 {
                for b in b0..=b1 {
                    cells.entry((a, b)).or_default().push(i as u32);
                }
            }
        }
        Buildings { list, cells }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The buildings whose cells reach within `r` of (x, z), each once,
    /// in index order.
    pub fn near(&self, x: f64, z: f64, r: f64) -> Vec<usize> {
        let (a0, b0) = Self::key(x - r, z - r);
        let (a1, b1) = Self::key(x + r, z + r);
        let mut out: Vec<usize> = Vec::new();
        for a in a0..=a1 {
            for b in b0..=b1 {
                if let Some(v) = self.cells.get(&(a, b)) {
                    for &i in v {
                        let i = i as usize;
                        if let Err(k) = out.binary_search(&i) {
                            out.insert(k, i);
                        }
                    }
                }
            }
        }
        out
    }

    /// Whether (x, z) is inside a building.
    pub fn inside(&self, x: f64, z: f64) -> bool {
        self.near(x, z, 0.0).into_iter().any(|i| {
            let f = &self.list[i];
            let (a, b) = f.local(x, z);
            a.abs() < f.hx && b.abs() < f.hz
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
        let reach = (hw * hw + hl * hl).sqrt();
        let mut best: Option<Push> = None;
        for i in self.near(x, z, reach) {
            let f = &self.list[i];
            let (dx, dz) = (x - f.x, z - f.z);
            let rr = reach + f.radius();
            if dx * dx + dz * dz > rr * rr {
                continue;
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
                continue;
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
        }
        best
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
