//! The ground: a height field and physical obstacles along the path, with the
//! same heights for physics and for the policy's height scan.

use rapier3d::prelude::*;

use crate::rng::Rng;

/// Longitudinal offsets of the 5×3 height scan grid in the mech's heading frame, m.
pub const SCAN_X: [f32; 5] = [-0.25, 0.15, 0.50, 0.85, 1.25];
/// Lateral offsets of the 5×3 height scan grid (left foot, centre, right foot), m.
pub const SCAN_Z: [f32; 3] = [-0.45, 0.0, 0.45];
/// Number of height samples in [`Terrain::scan`].
pub const HEIGHT_SCAN: usize = SCAN_X.len() * SCAN_Z.len();

/// What kind of obstacle sits on the endless dirt path (Demo 2, M5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObstacleKind {
    /// A chipped boulder or rock slab settled in the dirt.
    Boulder,
    /// A fallen tree branch or log lying across the path.
    Branch,
    /// A rusted, crushed old car wreck hulk.
    Wreck,
}

/// One physical obstacle on the terrain.
#[derive(Clone, Debug, PartialEq)]
pub struct Obstacle {
    /// Unique serial so the renderer can spawn/despawn visual meshes as chunks stream.
    pub id: u64,
    pub kind: ObstacleKind,
    /// World position `[x, y, z]` of the obstacle's base on the ground, m.
    pub position: [f32; 3],
    /// Heading angle around Y, rad.
    pub yaw: f32,
    /// Half-extents `[hx, hy, hz]` of its collision box in local frame, m.
    pub half: [f32; 3],
}

impl Obstacle {
    /// Height of this obstacle's top surface at world `(x, z)`, or `None` if outside.
    /// Includes a 0.16 m bevel margin so the height scan sees the rise just before contact.
    pub fn height_at(&self, x: f32, z: f32) -> Option<f32> {
        let dx = x - self.position[0];
        let dz = z - self.position[2];
        let (s, c) = self.yaw.sin_cos();
        let lx = (c * dx + s * dz).abs();
        let lz = (-s * dx + c * dz).abs();
        let margin = 0.16;
        let [hx, hy, hz] = self.half;
        if lx > hx + margin || lz > hz + margin {
            return None;
        }
        let fx = (1.0 - (lx - hx).max(0.0) / margin).clamp(0.0, 1.0);
        let fz = (1.0 - (lz - hz).max(0.0) / margin).clamp(0.0, 1.0);
        let top = self.position[1] + 1.65 * hy * fx * fz;
        Some(top)
    }
}

/// A square height field centred on the origin, plus optional streamed obstacles.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// Samples per side.
    pub n: usize,
    /// Side length, m.
    pub size: f32,
    /// Heights, row-major in (x, z): `heights[ix * n + iz]`.
    pub heights: Vec<f32>,
    /// Obstacles on the path (boulders, branches, car wrecks).
    pub obstacles: Vec<Obstacle>,
    /// Path roughness amplitude, m (`0.0` for flat).
    pub roughness: f32,
    /// Obstacle density in `0.0..=1.0`.
    pub density: f32,
    /// Obstacle size scale (`0.5..=1.8`).
    pub obs_scale: f32,
    /// Furrows/undulations seed and streaming state.
    pub streamed_to_x: f32,
    next_id: u64,
    rng: Rng,
}

impl Terrain {
    pub fn flat(size: f32) -> Self {
        let n = 2;
        Self {
            n,
            size,
            heights: vec![0.0; n * n],
            obstacles: Vec::new(),
            roughness: 0.0,
            density: 0.0,
            obs_scale: 1.0,
            streamed_to_x: 0.0,
            next_id: 1,
            rng: Rng::new(0x5041_5448),
        }
    }

    /// An undulating dirt path with boulders, fallen tree branches, and old car
    /// wrecks ahead of the origin (`x > 3.5 m`).
    pub fn path(size: f32, roughness: f32, density: f32, obs_scale: f32, seed: u64) -> Self {
        let n = 129;
        let roughness = roughness.clamp(0.0, 1.0);
        let density = density.clamp(0.0, 1.0);
        let obs_scale = obs_scale.clamp(0.4, 2.0);
        let mut heights = vec![0.0; n * n];
        if roughness > 0.0 {
            let cell = size / (n - 1) as f32;
            for ix in 0..n {
                let x = -0.5 * size + ix as f32 * cell;
                // Keep the spawn pad near x = 0 level so the mech starts standing cleanly.
                let spawn_fade = ((x.abs() - 2.5) / 3.0).clamp(0.0, 1.0);
                for iz in 0..n {
                    let z = -0.5 * size + iz as f32 * cell;
                    let w1 = (0.19 * x + 0.11 * z + seed as f32 * 0.7).sin()
                        * (0.14 * x - 0.23 * z).cos();
                    let w2 = (0.47 * x - 0.31 * z + 1.3).sin() * 0.45;
                    let rut = -0.025 * (-(z * 1.4).powi(2)).exp();
                    heights[ix * n + iz] = spawn_fade * roughness * (0.065 * (w1 + w2) + rut);
                }
            }
        }
        let mut t = Self {
            n,
            size,
            heights,
            obstacles: Vec::new(),
            roughness,
            density,
            obs_scale,
            streamed_to_x: 2.0,
            next_id: 1,
            rng: Rng::new(seed ^ 0x6D77_7061),
        };
        t.spawn_chunk(2.0, 48.0, None, 0.8);
        t
    }

    /// Ground height at `(x, z)` from the continuous heightfield alone (no obstacles).
    pub fn base_height(&self, x: f32, z: f32) -> f32 {
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

    /// The ground's height at (x, z), bilinear between samples plus any obstacle.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let mut h = self.base_height(x, z);
        for o in &self.obstacles {
            if (x - o.position[0]).abs() <= 2.5
                && (z - o.position[2]).abs() <= 2.5
                && let Some(oh) = o.height_at(x, z)
            {
                h = h.max(oh);
            }
        }
        h
    }

    /// Samples the 5×3 height scan grid in the mech's heading frame relative to `base_y`.
    pub fn scan(
        &self,
        origin_x: f32,
        origin_z: f32,
        heading_x: f32,
        heading_z: f32,
        base_y: f32,
    ) -> [f32; HEIGHT_SCAN] {
        let side_x = -heading_z;
        let side_z = heading_x;
        let mut out = [0.0; HEIGHT_SCAN];
        let mut i = 0;
        for &sx in &SCAN_X {
            for &sz in &SCAN_Z {
                let wx = origin_x + heading_x * sx + side_x * sz;
                let wz = origin_z + heading_z * sx + side_z * sz;
                out[i] = (self.height(wx, wz) - base_y).clamp(-0.5, 0.8);
                i += 1;
            }
        }
        out
    }

    /// Adds the ground and any current obstacles to `world`.
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
        for o in &self.obstacles {
            Self::insert_obstacle_collider(o, world, friction);
        }
    }

    fn insert_obstacle_collider(o: &Obstacle, world: &mut PhysicsWorld, friction: f32) {
        let groups =
            InteractionGroups::new(Group::GROUP_2, Group::GROUP_1, InteractionTestMode::And);
        let [hx, hy, hz] = o.half;
        // Embed bottom slightly into the ground so a foot never snags underneath.
        let cy = o.position[1] + 0.65 * hy;
        let body = world.insert_body(
            RigidBodyBuilder::fixed()
                .translation(Vector::new(o.position[0], cy, o.position[2]))
                .rotation(Vector::new(0.0, o.yaw, 0.0))
                .user_data(o.id as u128),
        );
        let builder = ColliderBuilder::round_cuboid(
            (hx - 0.02).max(0.02),
            (hy - 0.02).max(0.01),
            (hz - 0.02).max(0.02),
            0.02,
        )
        .friction(friction)
        .collision_groups(groups)
        .user_data(o.id as u128);
        world.insert_collider(builder, Some(body));
    }

    /// Streams new obstacles ahead of `mech_x` and removes ones far behind.
    pub fn stream_ahead(&mut self, world: &mut PhysicsWorld, mech_x: f32, friction: f32) {
        let despawn_before = mech_x - 18.0;
        let mut removed_ids = Vec::new();
        self.obstacles.retain(|o| {
            if o.position[0] < despawn_before {
                removed_ids.push(o.id as u128);
                false
            } else {
                true
            }
        });
        if !removed_ids.is_empty() {
            let handles: Vec<RigidBodyHandle> = world
                .bodies
                .iter()
                .filter_map(|(h, b)| removed_ids.contains(&b.user_data).then_some(h))
                .collect();
            for h in handles {
                world.remove_body(h);
            }
        }
        let want_to = mech_x + 42.0;
        if self.streamed_to_x < want_to {
            let from = self.streamed_to_x;
            self.spawn_chunk(from, want_to, Some(world), friction);
        }
    }

    fn spawn_chunk(
        &mut self,
        from_x: f32,
        to_x: f32,
        mut world: Option<&mut PhysicsWorld>,
        friction: f32,
    ) {
        self.streamed_to_x = to_x;
        if self.density <= 0.01 {
            return;
        }
        // Spacing between obstacle clusters shrinks as density increases.
        let step_mean = (3.6 - 2.2 * self.density).max(1.2);
        let mut x = from_x + self.rng.range(0.4, step_mean * 0.7);
        while x < to_x {
            let slot = self.next_id % 3;
            let s = self.obs_scale;
            let (kind, half, z, yaw) = if slot == 0 {
                // Tree branch / fallen log lying across the path: low enough to step over.
                let len = self.rng.range(0.65, 1.25) * s;
                let thick = self.rng.range(0.055, 0.095) * s;
                let z = self.rng.range(-0.85, 0.85);
                let yaw = self.rng.range(-0.45, 0.45);
                (ObstacleKind::Branch, [thick, thick * 0.9, len], z, yaw)
            } else if slot == 1 {
                // Chipped boulder / stone slab on the path.
                let rx = self.rng.range(0.24, 0.46) * s;
                let rz = self.rng.range(0.26, 0.48) * s;
                let ry = self.rng.range(0.065, 0.125) * s;
                let z = self.rng.range(-1.15, 1.15);
                let yaw = self.rng.range(0.0, std::f32::consts::TAU);
                (ObstacleKind::Boulder, [rx, ry, rz], z, yaw)
            } else {
                // Old crushed car wreck: sits toward the shoulder of the dirt path
                // with its low crushed hood/fender extending into the lane.
                let side = if (self.next_id & 1) == 0 { -1.0 } else { 1.0 };
                let z = side * self.rng.range(0.85, 1.55);
                let yaw = self.rng.range(-0.35, 0.35);
                let hx = self.rng.range(0.72, 1.05) * s;
                let hy = self.rng.range(0.085, 0.135) * s;
                let hz = self.rng.range(0.45, 0.65) * s;
                (ObstacleKind::Wreck, [hx, hy, hz], z, yaw)
            };
            let y = self.base_height(x, z);
            let obs = Obstacle {
                id: self.next_id,
                kind,
                position: [x, y, z],
                yaw,
                half,
            };
            self.next_id += 1;
            if let Some(w) = world.as_deref_mut() {
                Self::insert_obstacle_collider(&obs, w, friction);
            }
            self.obstacles.push(obs);
            x += self.rng.range(0.55 * step_mean, 1.45 * step_mean);
        }
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
        assert_eq!(t.scan(0.0, 0.0, 1.0, 0.0, 0.0), [0.0; HEIGHT_SCAN]);
    }

    #[test]
    fn path_spawns_obstacles_and_height_scan_sees_them() {
        let t = Terrain::path(200.0, 0.5, 0.8, 1.0, 42);
        assert!(!t.obstacles.is_empty());
        let first = &t.obstacles[0];
        let h_obs = t.height(first.position[0], first.position[2]);
        let h_base = t.base_height(first.position[0], first.position[2]);
        assert!(h_obs > h_base + 0.04, "h_obs={h_obs}, h_base={h_base}");
    }
}
