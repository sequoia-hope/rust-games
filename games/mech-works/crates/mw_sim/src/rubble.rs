//! Loose rubble and fixed walls for the game's levels: rocks as dynamic
//! convex bodies in the mech's own Rapier world, so the mech's feet and hull
//! push them for real, and walls (a pass's sides) as fixed boxes. Training
//! never adds either, so it steps exactly as before.
//!
//! The rocks outlive a world: [`Sim`](crate::sim::Sim) is rebuilt when the
//! mech stands up again, so [`Rubble::sync`] reads every rock back after a
//! step and [`Rubble::insert`] puts them, as they were, into a new world.

use rapier3d::prelude::*;

use crate::rng::Rng;
use crate::sim::BodyPose;

/// Rocks collide with the ground and walls (the world group), with the
/// mech (whose links are in group 1 and see only the world group), and with
/// each other: members of both groups, filtering both.
fn rock_groups() -> InteractionGroups {
    let both = Group::GROUP_1 | Group::GROUP_2;
    InteractionGroups::new(both, both, InteractionTestMode::And)
}

/// The world group, seen by the mech and the rocks: what the ground uses.
fn wall_groups() -> InteractionGroups {
    InteractionGroups::new(Group::GROUP_2, Group::GROUP_1, InteractionTestMode::And)
}

/// Broken rock, kg/m³ (lighter than solid stone: a slide's blocks are
/// fractured and the mech has to be able to move them).
pub const ROCK_DENSITY: f32 = 1700.0;

/// One loose rock.
#[derive(Clone, Debug)]
pub struct Rock {
    /// Its hull's points in its own frame, m.
    pub points: Vec<[f32; 3]>,
    /// Where it is now.
    pub pose: BodyPose,
    pub linvel: [f32; 3],
    pub angvel: [f32; 3],
    /// kg, once in a world.
    pub mass: f32,
    handle: Option<RigidBodyHandle>,
}

impl Rock {
    pub fn position(&self) -> [f32; 3] {
        self.pose.position
    }

    /// Its hull as triangles (points in its own frame, then index triples),
    /// for a renderer to draw the shape the physics uses.
    pub fn hull(&self) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
        let pts: Vec<Vector> = self.points.iter().map(|&p| Vector::from(p)).collect();
        match ColliderBuilder::convex_hull(&pts).and_then(|b| {
            b.build()
                .shape()
                .as_convex_polyhedron()
                .map(|c| c.to_trimesh())
        }) {
            Some((v, t)) => (v.iter().map(|p| [p.x, p.y, p.z]).collect(), t),
            None => (Vec::new(), Vec::new()),
        }
    }
}

/// An axis-aligned box, m: `min` and `max` corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Volume {
    pub fn contains(&self, p: [f32; 3]) -> bool {
        (0..3).all(|i| p[i] >= self.min[i] && p[i] <= self.max[i])
    }
}

/// A fixed box: the side of a pass, a cliff, the edge of the play area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub centre: [f32; 3],
    pub half: [f32; 3],
    /// About the vertical, rad.
    pub yaw: f32,
}

impl Wall {
    pub fn insert(&self, world: &mut PhysicsWorld, friction: f32) {
        let body = world.insert_body(
            RigidBodyBuilder::fixed()
                .translation(Vector::from(self.centre))
                .rotation(Vector::new(0.0, self.yaw, 0.0)),
        );
        let [hx, hy, hz] = self.half;
        world.insert_collider(
            ColliderBuilder::cuboid(hx, hy, hz)
                .friction(friction)
                .collision_groups(wall_groups()),
            Some(body),
        );
    }
}

/// What a shove did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shove {
    /// Rocks it moved.
    pub rocks: usize,
    /// Their mass, kg.
    pub mass: f32,
}

/// Every loose rock in a level.
#[derive(Clone, Debug, Default)]
pub struct Rubble {
    pub rocks: Vec<Rock>,
}

impl Rubble {
    /// `count` chipped rocks, `radius` m across each (a range), dropped in
    /// layers into the box `pile` so they settle into a heap. Deterministic
    /// in `seed`.
    pub fn pile(seed: u64, pile: Volume, count: usize, radius: [f32; 2]) -> Self {
        let mut r = Rng::new(seed ^ 0x5255_4242);
        let size = [
            pile.max[0] - pile.min[0],
            pile.max[1] - pile.min[1],
            pile.max[2] - pile.min[2],
        ];
        // Rocks go in on a loose grid over the pile's floor, layer upon layer.
        let cell = 2.1 * radius[1];
        let nx = ((size[0] / cell) as usize).max(1);
        let nz = ((size[2] / cell) as usize).max(1);
        let rocks = (0..count)
            .map(|i| {
                let rad = r.range(radius[0], radius[1]);
                let (layer, k) = (i / (nx * nz), i % (nx * nz));
                let x = pile.min[0] + (0.5 + (k % nx) as f32) * size[0] / nx as f32;
                let z = pile.min[2] + (0.5 + (k / nx) as f32) * size[2] / nz as f32;
                let y = pile.min[1] + radius[1] + layer as f32 * cell;
                let jitter = 0.25 * (cell - 2.0 * rad).max(0.0);
                let points = rock_points(&mut r, rad);
                let yaw = r.range(0.0, std::f32::consts::TAU);
                let q = Rotation::from_rotation_y(yaw);
                Rock {
                    points,
                    pose: BodyPose {
                        position: [x + r.signed() * jitter, y, z + r.signed() * jitter],
                        rotation: [q.x, q.y, q.z, q.w],
                    },
                    linvel: [0.0; 3],
                    angvel: [0.0; 3],
                    mass: 0.0,
                    handle: None,
                }
            })
            .collect();
        Self { rocks }
    }

    /// Puts every rock into `world`, where and as it was moving.
    pub fn insert(&mut self, world: &mut PhysicsWorld, friction: f32) {
        for rock in &mut self.rocks {
            let pts: Vec<Vector> = rock.points.iter().map(|&p| Vector::from(p)).collect();
            let Some(collider) = ColliderBuilder::convex_hull(&pts) else {
                continue;
            };
            let r = rock.pose.rotation;
            let pose = Pose::from_parts(
                Vector::from(rock.pose.position),
                Rotation::from_xyzw(r[0], r[1], r[2], r[3]),
            );
            let body = world.insert_body(
                RigidBodyBuilder::dynamic()
                    .pose(pose)
                    .linvel(Vector::from(rock.linvel))
                    .angvel(Vector::from(rock.angvel))
                    // Rubble comes to rest; a rolling rock should not roll on.
                    .angular_damping(0.6)
                    .linear_damping(0.05),
            );
            world.insert_collider(
                collider
                    .density(ROCK_DENSITY)
                    .friction(friction.max(0.8))
                    .restitution(0.05)
                    .collision_groups(rock_groups()),
                Some(body),
            );
            rock.mass = world.bodies[body].mass();
            rock.handle = Some(body);
        }
    }

    /// Reads every rock's state back from `world` (after a step).
    pub fn sync(&mut self, world: &PhysicsWorld) {
        for rock in &mut self.rocks {
            let Some(b) = rock.handle.and_then(|h| world.bodies.get(h)) else {
                continue;
            };
            let p = b.position();
            let q = p.rotation;
            rock.pose = BodyPose {
                position: [p.translation.x, p.translation.y, p.translation.z],
                rotation: [q.x, q.y, q.z, q.w],
            };
            let (v, w) = (b.linvel(), b.angvel());
            rock.linvel = [v.x, v.y, v.z];
            rock.angvel = [w.x, w.y, w.z];
        }
    }

    /// Rocks whose centre is inside `v`, and their mass, kg.
    pub fn inside(&self, v: &Volume) -> (usize, f32) {
        self.rocks
            .iter()
            .filter(|r| v.contains(r.pose.position))
            .fold((0, 0.0), |(n, m), r| (n + 1, m + r.mass))
    }

    /// A blade's shove from the mech: every rock whose centre is ahead of
    /// `origin` along `heading` (horizontal, unit) by up to `reach` m, and
    /// within `half_width` m to either side and `height` m above `origin`'s
    /// ground, gets `speed` m/s more along the heading (and a little lift
    /// and spread, so a heap breaks up rather than sliding as one). Applied
    /// as impulses, so a heavy rock moves less; the speed is what a rock of
    /// `ref_mass` kg gets.
    #[allow(clippy::too_many_arguments)] // A shove's whole shape; each is a plain number.
    pub fn shove(
        &mut self,
        world: &mut PhysicsWorld,
        origin: [f32; 3],
        heading: [f32; 2],
        reach: f32,
        half_width: f32,
        height: f32,
        speed: f32,
        ref_mass: f32,
    ) -> Shove {
        let [hx, hz] = heading;
        let mut done = Shove::default();
        for rock in &self.rocks {
            let Some(b) = rock.handle.and_then(|h| world.bodies.get_mut(h)) else {
                continue;
            };
            let p = b.translation();
            let (dx, dz) = (p.x - origin[0], p.z - origin[2]);
            let along = dx * hx + dz * hz;
            let side = -dx * hz + dz * hx;
            if !(0.0..=reach).contains(&along)
                || side.abs() > half_width
                || p.y > origin[1] + height
            {
                continue;
            }
            let m = b.mass();
            // Heavier rocks get the same push, so they move less.
            let dv = speed * (ref_mass / m.max(1.0)).sqrt().min(1.5);
            let spread = 0.35 * side.signum() * (side.abs() / half_width.max(0.1));
            let dir = Vector::new(hx - spread * hz, 0.3, hz + spread * hx);
            b.apply_impulse(dir * (m * dv), true);
            done.rocks += 1;
            done.mass += m;
        }
        done
    }
}

/// A chipped stone: an icosahedron's points pushed about and cut by a few
/// cleavage planes, `radius` m across.
fn rock_points(r: &mut Rng, radius: f32) -> Vec<[f32; 3]> {
    let phi = 0.5 * (1.0 + 5.0f32.sqrt());
    let base = [
        [-1.0, phi, 0.0],
        [1.0, phi, 0.0],
        [-1.0, -phi, 0.0],
        [1.0, -phi, 0.0],
        [0.0, -1.0, phi],
        [0.0, 1.0, phi],
        [0.0, -1.0, -phi],
        [0.0, 1.0, -phi],
        [phi, 0.0, -1.0],
        [phi, 0.0, 1.0],
        [-phi, 0.0, -1.0],
        [-phi, 0.0, 1.0],
    ];
    let planes: Vec<([f32; 3], f32)> = (0..4)
        .map(|_| {
            let n = [r.signed(), r.signed(), r.signed()];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-3);
            ([n[0] / l, n[1] / l, n[2] / l], r.range(0.62, 0.85))
        })
        .collect();
    // Flatter than tall: slide debris lies on its broad side.
    let squash = [r.range(0.85, 1.25), r.range(0.55, 0.8), r.range(0.85, 1.25)];
    base.iter()
        .map(|v| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let k = r.range(0.85, 1.15) / l;
            let mut p = [v[0] * k, v[1] * k, v[2] * k];
            for (n, d) in &planes {
                let proj = p[0] * n[0] + p[1] * n[1] + p[2] * n[2];
                if proj > *d {
                    for i in 0..3 {
                        p[i] -= n[i] * (proj - d) * 0.8;
                    }
                }
            }
            [
                p[0] * radius * squash[0],
                p[1] * radius * squash[1],
                p[2] * radius * squash[2],
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Terrain;

    fn world() -> PhysicsWorld {
        let mut w = PhysicsWorld::new();
        w.gravity = Vector::new(0.0, crate::sim::GRAVITY, 0.0);
        w.integration_parameters.dt = crate::sim::DT;
        Terrain::flat(100.0).insert(&mut w, 0.9);
        w
    }

    fn pile_volume() -> Volume {
        Volume {
            min: [-2.0, 0.0, -2.0],
            max: [2.0, 2.0, 2.0],
        }
    }

    #[test]
    fn a_pile_settles_on_the_ground_and_stays_put() {
        let mut w = world();
        let mut rubble = Rubble::pile(3, pile_volume(), 16, [0.3, 0.5]);
        rubble.insert(&mut w, 0.9);
        for _ in 0..(4 * crate::sim::PHYSICS_HZ) {
            w.step();
        }
        rubble.sync(&w);
        for rock in &rubble.rocks {
            let [x, y, z] = rock.position();
            assert!(x.is_finite() && y.is_finite() && z.is_finite());
            assert!(y > 0.0 && y < 3.0, "rock at height {y}");
            assert!(x.abs() < 6.0 && z.abs() < 6.0, "rock rolled to ({x}, {z})");
            assert!(rock.mass > 20.0 && rock.mass < 2000.0, "mass {}", rock.mass);
        }
        let (n, _) = rubble.inside(&Volume {
            min: [-3.0, -1.0, -3.0],
            max: [3.0, 3.0, 3.0],
        });
        assert!(n >= 12, "only {n} of 16 rocks stayed in the heap");
    }

    #[test]
    fn a_shove_moves_rocks_ahead_out_of_their_volume() {
        let mut w = world();
        let mut rubble = Rubble::pile(5, pile_volume(), 8, [0.3, 0.45]);
        rubble.insert(&mut w, 0.9);
        for _ in 0..crate::sim::PHYSICS_HZ {
            w.step();
        }
        let mut moved = 0;
        for _ in 0..6 {
            moved += rubble
                .shove(
                    &mut w,
                    [-3.0, 0.0, 0.0],
                    [1.0, 0.0],
                    8.0,
                    3.0,
                    3.0,
                    4.0,
                    300.0,
                )
                .rocks;
            for _ in 0..crate::sim::PHYSICS_HZ {
                w.step();
            }
            rubble.sync(&w);
        }
        assert!(moved > 0);
        let mean_x = rubble.rocks.iter().map(|r| r.position()[0]).sum::<f32>() / 8.0;
        assert!(mean_x > 2.0, "rocks did not go ahead: mean x {mean_x}");
    }

    #[test]
    fn rocks_survive_a_new_world() {
        let mut w = world();
        let mut rubble = Rubble::pile(9, pile_volume(), 6, [0.3, 0.45]);
        rubble.insert(&mut w, 0.9);
        for _ in 0..crate::sim::PHYSICS_HZ {
            w.step();
        }
        rubble.sync(&w);
        let before: Vec<[f32; 3]> = rubble.rocks.iter().map(Rock::position).collect();
        let mut w2 = world();
        rubble.insert(&mut w2, 0.9);
        w2.step();
        rubble.sync(&w2);
        for (a, r) in before.iter().zip(&rubble.rocks) {
            let b = r.position();
            let d = (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f32::max);
            assert!(d < 0.05, "a rock jumped {d} m into the new world");
        }
    }

    #[test]
    fn a_wall_stops_a_rock() {
        let mut w = world();
        Wall {
            centre: [2.0, 1.0, 0.0],
            half: [0.5, 1.0, 3.0],
            yaw: 0.0,
        }
        .insert(&mut w, 0.9);
        let mut rubble = Rubble::pile(
            1,
            Volume {
                min: [-0.5, 0.0, -0.5],
                max: [0.5, 1.0, 0.5],
            },
            1,
            [0.3, 0.3],
        );
        rubble.rocks[0].linvel = [6.0, 0.0, 0.0];
        rubble.insert(&mut w, 0.9);
        for _ in 0..(2 * crate::sim::PHYSICS_HZ) {
            w.step();
        }
        rubble.sync(&w);
        assert!(rubble.rocks[0].position()[0] < 1.5);
    }

    #[test]
    fn hulls_are_closed_triangle_meshes() {
        let rubble = Rubble::pile(2, pile_volume(), 4, [0.3, 0.5]);
        for rock in &rubble.rocks {
            let (v, t) = rock.hull();
            assert!(v.len() >= 4 && t.len() >= 4);
            assert!(t.iter().flatten().all(|&i| (i as usize) < v.len()));
        }
    }
}
