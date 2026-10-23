//! The ground: a height field (flat for now), with the same heights for
//! physics and for the policy's height scan.

use rapier3d::prelude::*;

/// A square height field centred on the origin.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// Samples per side.
    pub n: usize,
    /// Side length, m.
    pub size: f32,
    /// Heights, row-major in (x, z): `heights[ix * n + iz]`.
    pub heights: Vec<f32>,
}

impl Terrain {
    pub fn flat(size: f32) -> Self {
        let n = 2;
        Self {
            n,
            size,
            heights: vec![0.0; n * n],
        }
    }

    /// The ground's height at (x, z), bilinear between samples.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let cell = self.size / (self.n - 1) as f32;
        let fx = ((x + 0.5 * self.size) / cell).clamp(0.0, (self.n - 1) as f32);
        let fz = ((z + 0.5 * self.size) / cell).clamp(0.0, (self.n - 1) as f32);
        let (ix, iz) = ((fx as usize).min(self.n - 2), (fz as usize).min(self.n - 2));
        let (tx, tz) = (fx - ix as f32, fz - iz as f32);
        let h = |i: usize, k: usize| self.heights[i * self.n + k];
        let a = h(ix, iz) + (h(ix + 1, iz) - h(ix, iz)) * tx;
        let b = h(ix, iz + 1) + (h(ix + 1, iz + 1) - h(ix, iz + 1)) * tx;
        a + (b - a) * tz
    }

    /// Adds the ground to `world`.
    pub fn insert(&self, world: &mut PhysicsWorld, friction: f32) {
        let body = world.insert_body(RigidBodyBuilder::fixed());
        let groups =
            InteractionGroups::new(Group::GROUP_2, Group::GROUP_1, InteractionTestMode::And);
        let collider = if self.heights.iter().all(|&h| h == 0.0) {
            // A box: cheaper contacts than a height field.
            let half = 0.5 * self.size;
            ColliderBuilder::cuboid(half, 1.0, half).translation(Vector::new(0.0, -1.0, 0.0))
        } else {
            // Parry's height field is indexed (row: z, column: x).
            let mut zx = Array2::zeros(self.n, self.n);
            for ix in 0..self.n {
                for iz in 0..self.n {
                    zx[(iz, ix)] = self.heights[ix * self.n + iz];
                }
            }
            ColliderBuilder::heightfield(zx, Vector::new(self.size, 1.0, self.size))
        };
        world.insert_collider(
            collider.friction(friction).collision_groups(groups),
            Some(body),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_is_zero_everywhere() {
        let t = Terrain::flat(100.0);
        assert_eq!(t.height(3.0, -7.0), 0.0);
        assert_eq!(t.height(500.0, 0.0), 0.0);
    }
}
