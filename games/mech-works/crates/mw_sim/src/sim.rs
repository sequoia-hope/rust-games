//! The physics: the mech as a Rapier multibody on the ground, stepped at a
//! fixed rate, its joints driven by PD controllers through the actuator
//! model as explicit torques.

use rapier3d::prelude::*;

use crate::actuator::Actuator;
use crate::mech::{self, BodyParams, JOINTS, JOINTS_PER_LEG, LEGS, dims, joint_design};
use crate::terrain::Terrain;

/// Physics rate, Hz, and its step.
pub const PHYSICS_HZ: u32 = 200;
pub const DT: f32 = 1.0 / PHYSICS_HZ as f32;

/// Solver iterations, fixed so training and the game step identically.
pub const SOLVER_ITERATIONS: usize = 4;

pub const GRAVITY: f32 = -9.81;

/// Collision groups: the mech's links collide with the world, not each other.
const MECH: Group = Group::GROUP_1;
const WORLD: Group = Group::GROUP_2;

/// A link of the mech: its body and the joint that connects it to its
/// parent (the torso has none).
#[derive(Clone, Copy, Debug)]
struct Link {
    body: RigidBodyHandle,
}

/// One actuated joint.
#[derive(Clone, Copy, Debug)]
struct Joint {
    parent: RigidBodyHandle,
    child: RigidBodyHandle,
    /// The axis in the parent's frame.
    axis: Vector,
    /// The joint's generalized coordinate in the multibody.
    dof: usize,
}

/// The mech in its world.
pub struct Sim {
    pub world: PhysicsWorld,
    torso: RigidBodyHandle,
    links: Vec<Link>,
    joints: [Joint; JOINTS],
    feet: [RigidBodyHandle; LEGS],
    foot_colliders: [ColliderHandle; LEGS],
    pub actuators: [Actuator; JOINTS],
    pub params: BodyParams,
    /// PD position targets, rad.
    pub targets: [f32; JOINTS],
    /// The torque each joint asked for and got in the last step.
    pub commanded: [f32; JOINTS],
    pub time: f32,
    /// Pushes waiting for the next step: a link and a force, N.
    pushes: Vec<(RigidBodyHandle, Vector, Vector)>,
}

/// A pose of a body: position and rotation (x, y, z, w).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPose {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

/// What a link looks like, for a renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkShape {
    Cuboid {
        half: [f32; 3],
        offset: [f32; 3],
    },
    Capsule {
        half_height: f32,
        radius: f32,
        offset: [f32; 3],
    },
    Ball {
        radius: f32,
    },
}

impl Sim {
    /// The mech standing in its default pose on `terrain` at (x, z), with
    /// `params`' body.
    pub fn new(params: BodyParams, terrain: &Terrain, x: f32, z: f32) -> Self {
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::new(0.0, GRAVITY, 0.0);
        world.integration_parameters.dt = DT;
        world.integration_parameters.num_solver_iterations = SOLVER_ITERATIONS;
        // Rapier's stabilization pass corrects penetration without friction,
        // and through the mech's bent legs that slides a loaded foot (19 cm
        // in 4 s of standing, against 2 mm without it).
        world
            .integration_parameters
            .num_internal_stabilization_iterations = 0;
        terrain.insert(&mut world, params.friction_coefficient());

        let groups = InteractionGroups::new(MECH, WORLD, InteractionTestMode::And);
        let friction = params.friction_coefficient();

        // The torso, with its centre of mass moved by the body parameters.
        let com = params.com_offset_m();
        let torso_mass = params.torso_mass_kg();
        let torso = world.insert_body(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(x, 3.0, z))
                .can_sleep(false),
        );
        let [hx, hy, hz] = dims::TORSO_HALF;
        let torso_inertia = Vector::new(
            torso_mass / 3.0 * (hy * hy + hz * hz),
            torso_mass / 3.0 * (hx * hx + hz * hz),
            torso_mass / 3.0 * (hx * hx + hy * hy),
        );
        world.insert_collider(
            ColliderBuilder::cuboid(hx, hy, hz)
                .translation(Vector::new(0.0, dims::TORSO_CENTRE_Y, 0.0))
                .collision_groups(groups)
                .friction(friction)
                .mass_properties(MassProperties::new(
                    Vector::new(com[0], dims::TORSO_CENTRE_Y + com[1], com[2]),
                    torso_mass,
                    torso_inertia,
                )),
            Some(torso),
        );
        // A tool on the hull: its own box and mass on the torso's body.
        if let Some((half, at)) = params_tool(&params) {
            let m = params.tool_kg();
            world.insert_collider(
                ColliderBuilder::cuboid(half[0], half[1], half[2])
                    .translation(Vector::new(at[0], at[1], at[2]))
                    .collision_groups(groups)
                    .friction(friction)
                    .mass_properties(MassProperties::new(
                        Vector::ZERO,
                        m,
                        Vector::new(
                            m / 3.0 * (half[1] * half[1] + half[2] * half[2]),
                            m / 3.0 * (half[0] * half[0] + half[2] * half[2]),
                            m / 3.0 * (half[0] * half[0] + half[1] * half[1]),
                        ),
                    )),
                Some(torso),
            );
        }

        let mut links = vec![Link { body: torso }];
        let mut joint_list = Vec::with_capacity(JOINTS);
        let mut feet = [torso; LEGS];
        let mut foot_colliders = [ColliderHandle::invalid(); LEGS];

        for leg in 0..LEGS {
            let side = if leg == 0 { -1.0 } else { 1.0 };
            let scale = params.leg_scale_factor(leg);
            let thigh = dims::THIGH_LENGTH * scale;
            let shin = dims::SHIN_LENGTH * scale;
            // Each joint's anchor in its parent's frame; every link's origin
            // is at its own joint.
            let anchors = [
                Vector::new(0.0, dims::HIP_Y, side * dims::HIP_Z),
                Vector::ZERO,
                Vector::ZERO,
                Vector::new(0.0, -thigh, 0.0),
                Vector::new(0.0, -shin, 0.0),
                Vector::ZERO,
            ];
            let mut parent = torso;
            for (j, &anchor) in anchors.iter().enumerate() {
                let design = joint_design(mech::joint_index(leg, j));
                let body = world.insert_body(RigidBodyBuilder::dynamic().can_sleep(false));
                let collider = match j {
                    0 | 1 | 4 => ColliderBuilder::ball(dims::SMALL_LINK_RADIUS)
                        .mass(match j {
                            4 => dims::ANKLE_LINK_MASS,
                            _ => dims::HIP_LINK_MASS,
                        })
                        .collision_groups(InteractionGroups::none()),
                    2 => ColliderBuilder::capsule_y(
                        0.5 * thigh - dims::THIGH_RADIUS,
                        dims::THIGH_RADIUS,
                    )
                    .translation(Vector::new(0.0, -0.5 * thigh, 0.0))
                    .mass(dims::THIGH_MASS)
                    .collision_groups(groups),
                    3 => ColliderBuilder::capsule_y(
                        0.5 * shin - dims::SHIN_RADIUS,
                        dims::SHIN_RADIUS,
                    )
                    .translation(Vector::new(0.0, -0.5 * shin, 0.0))
                    .mass(dims::SHIN_MASS)
                    .collision_groups(groups),
                    _ => {
                        let [fx, fy, fz] = dims::FOOT_HALF;
                        let [ox, oy, oz] = dims::FOOT_OFFSET;
                        ColliderBuilder::cuboid(fx, fy, fz)
                            .translation(Vector::new(ox, oy, oz))
                            .mass(dims::FOOT_MASS)
                            .collision_groups(groups)
                    }
                };
                let handle = world.insert_collider(collider.friction(friction), Some(body));
                if j == JOINTS_PER_LEG - 1 {
                    feet[leg] = body;
                    foot_colliders[leg] = handle;
                }
                let axis_v = design.axis.vector();
                let axis = Vector::new(axis_v[0], axis_v[1], axis_v[2]);
                let joint = RevoluteJointBuilder::new(axis)
                    .local_anchor1(anchor)
                    .local_anchor2(Vector::ZERO)
                    .limits(design.limits);
                // The PD controller is the joint's implicit motor (stable at
                // these gains where explicit torques are not, and explicit
                // torques on multibody links upset contact friction in
                // Rapier 0.36); the actuator model sets its torque limit.
                let joint = joint
                    .motor_model(MotorModel::ForceBased)
                    .motor_position(design.default, design.kp, design.kd)
                    .motor_max_force(design.peak_torque);
                world
                    .insert_multibody_joint(parent, body, joint)
                    .expect("the mech is a tree");
                joint_list.push(Joint {
                    parent,
                    child: body,
                    axis,
                    dof: 0,
                });
                links.push(Link { body });
                parent = body;
            }
        }

        let mut joints: [Joint; JOINTS] = joint_list.try_into().expect("12 joints");
        let mut actuators: [Actuator; JOINTS] =
            std::array::from_fn(|j| Actuator::new(mech::actuator_spec(j)));
        for (j, a) in actuators.iter_mut().enumerate() {
            a.health = params.health_factor(j);
            a.driver_strength = params.driver_factor(j);
        }

        // Each joint's degree of freedom, and the armature of its rotor.
        {
            let link_id = world
                .multibody_joints
                .rigid_body_link(torso)
                .copied()
                .unwrap();
            for (j, joint) in joints.iter_mut().enumerate() {
                let id = world
                    .multibody_joints
                    .rigid_body_link(joint.child)
                    .copied()
                    .unwrap();
                assert_eq!(id.multibody, link_id.multibody);
                let mb = world
                    .multibody_joints
                    .get_multibody_mut(id.multibody)
                    .unwrap();
                joint.dof = mb.link(id.id).unwrap().assembly_id();
                mb.armature_mut()[joint.dof] = joint_design(j).armature;
            }
        }

        let targets = std::array::from_fn(|j| joint_design(j).default);
        let mut sim = Self {
            world,
            torso,
            links,
            joints,
            feet,
            foot_colliders,
            actuators,
            params,
            targets,
            commanded: [0.0; JOINTS],
            time: 0.0,
            pushes: Vec::new(),
        };
        sim.set_pose(&targets);
        sim.place_on(terrain, x, z);
        sim
    }

    /// Puts every joint at `angles` (and at rest), keeping the torso where it is.
    pub fn set_pose(&mut self, angles: &[f32; JOINTS]) {
        let current = self.joint_angles();
        let dofs: Vec<usize> = self.joints.iter().map(|j| j.dof).collect();
        let world = &mut self.world;
        let id = *world.multibody_joints.rigid_body_link(self.torso).unwrap();
        let mb = world
            .multibody_joints
            .get_multibody_mut(id.multibody)
            .unwrap();
        let mut disp = vec![0.0; mb.ndofs()];
        for (j, &dof) in dofs.iter().enumerate() {
            disp[dof] = angles[j] - current[j];
        }
        mb.apply_displacements(&disp);
        mb.generalized_velocity_mut().fill(0.0);
        mb.forward_kinematics(&world.bodies, true);
        mb.update_rigid_bodies(&mut world.bodies, true);
    }

    /// Moves the mech so its lowest foot point rests on the terrain at (x, z).
    fn place_on(&mut self, terrain: &Terrain, x: f32, z: f32) {
        // From the bodies' poses: the colliders' own follow only on a step.
        let mut lowest = f32::INFINITY;
        for leg in 0..LEGS {
            let pose = *self.foot(leg).position();
            let [fx, fy, fz] = dims::FOOT_HALF;
            let [ox, oy, oz] = dims::FOOT_OFFSET;
            for corner in 0..8 {
                let s = |bit: usize| if corner & bit == 0 { -1.0 } else { 1.0 };
                let local = Vector::new(ox + s(1) * fx, oy + s(2) * fy, oz + s(4) * fz);
                lowest = lowest.min(pose.transform_point(local).y);
            }
        }
        let ground = terrain.height(x, z);
        let lift = ground - lowest + 0.005;
        let rb = &mut self.world.bodies[self.torso];
        let mut pose = *rb.position();
        pose.translation.y += lift;
        rb.set_position(pose, true);
        self.refresh_kinematics();
    }

    /// After the torso was moved by hand: puts the legs (and their
    /// colliders) where the joints say.
    pub fn refresh_kinematics(&mut self) {
        let id = *self
            .world
            .multibody_joints
            .rigid_body_link(self.torso)
            .unwrap();
        let mb = self
            .world
            .multibody_joints
            .get_multibody_mut(id.multibody)
            .unwrap();
        mb.forward_kinematics(&self.world.bodies, true);
        mb.update_rigid_bodies(&mut self.world.bodies, true);
        for l in &self.links {
            let pose = *self.world.bodies[l.body].position();
            for &c in self.world.bodies[l.body].colliders() {
                let local = *self.world.colliders[c].position_wrt_parent().unwrap();
                self.world.colliders[c].set_position(pose * local);
            }
        }
    }

    /// Joint angles, rad.
    pub fn joint_angles(&self) -> [f32; JOINTS] {
        std::array::from_fn(|j| {
            let joint = &self.joints[j];
            let p = self.world.bodies[joint.parent].rotation();
            let c = self.world.bodies[joint.child].rotation();
            let rel = p.inverse() * *c;
            let s = rel.x * joint.axis.x + rel.y * joint.axis.y + rel.z * joint.axis.z;
            let mut a = 2.0 * s.atan2(rel.w);
            if a > std::f32::consts::PI {
                a -= 2.0 * std::f32::consts::PI;
            } else if a < -std::f32::consts::PI {
                a += 2.0 * std::f32::consts::PI;
            }
            a
        })
    }

    /// Joint velocities, rad/s.
    pub fn joint_velocities(&self) -> [f32; JOINTS] {
        std::array::from_fn(|j| {
            let joint = &self.joints[j];
            let p = &self.world.bodies[joint.parent];
            let c = &self.world.bodies[joint.child];
            let axis = *p.rotation() * joint.axis;
            (c.angvel() - p.angvel()).dot(axis)
        })
    }

    /// One physics step: the PD motors, limited by the actuators, then Rapier.
    pub fn step(&mut self) {
        let q = self.joint_angles();
        let qd = self.joint_velocities();
        for j in 0..JOINTS {
            let d = joint_design(j);
            // What the PD law asks for, and what the actuator gives: the
            // motor solves the same law implicitly, clamped to this limit.
            let want = d.kp * (self.targets[j] - q[j]) - d.kd * qd[j];
            self.actuators[j].step(want, DT);
            self.commanded[j] = want;
            let limit = self.actuators[j].torque_limit();
            let joint = self.joints[j];
            let link = *self
                .world
                .multibody_joints
                .rigid_body_link(joint.child)
                .unwrap();
            let mb = self
                .world
                .multibody_joints
                .get_multibody_mut(link.multibody)
                .unwrap();
            let data = &mut mb.link_mut(link.id).unwrap().joint.data;
            data.set_motor_position(JointAxis::AngX, self.targets[j], d.kp, d.kd);
            data.set_motor_max_force(JointAxis::AngX, limit);
        }
        for &(body, force, point) in &self.pushes {
            self.world.bodies[body].add_force_at_point(force, point, true);
        }
        self.world.step();
        for (body, _, _) in self.pushes.drain(..) {
            self.world.bodies[body].reset_forces(false);
            self.world.bodies[body].reset_torques(false);
        }
        self.time += DT;
    }

    pub fn torso(&self) -> &RigidBody {
        &self.world.bodies[self.torso]
    }

    pub fn torso_mut(&mut self) -> &mut RigidBody {
        &mut self.world.bodies[self.torso]
    }

    /// The torso's up axis in the world (its Y), and the world's gravity in
    /// the torso's frame (unit).
    pub fn gravity_in_body(&self) -> Vector {
        self.torso().rotation().inverse() * Vector::new(0.0, -1.0, 0.0)
    }

    /// Whether each foot touches anything, and the contact force on it, N.
    pub fn foot_contacts(&self) -> [(bool, f32); LEGS] {
        std::array::from_fn(|leg| {
            let mut force = 0.0;
            let mut touching = false;
            for pair in self
                .world
                .narrow_phase
                .contact_pairs_with(self.foot_colliders[leg])
            {
                if pair.has_any_active_contact() {
                    touching = true;
                    force += pair.total_impulse().length() / DT;
                }
            }
            (touching, force)
        })
    }

    /// The foot collider's world position (for checks against the body's).
    pub fn foot_collider_position(&self, leg: usize) -> Vector {
        self.world.colliders[self.foot_colliders[leg]]
            .position()
            .translation
    }

    pub fn foot(&self, leg: usize) -> &RigidBody {
        &self.world.bodies[self.feet[leg]]
    }

    /// Pushes link `link` (in [`Sim::link_poses`]'s order) with `impulse`
    /// at its centre of mass, N·s, over the next physics step. (A force,
    /// not an impulse: Rapier overwrites a multibody link's velocity, so
    /// only forces reach it.)
    pub fn push(&mut self, link: usize, impulse: Vector) {
        let body = self.links[link % self.links.len()].body;
        let point = self.world.bodies[body].center_of_mass();
        self.push_at(link, impulse, point);
    }

    /// Pushes link `link` with `impulse` at the world point `point`.
    pub fn push_at(&mut self, link: usize, impulse: Vector, point: Vector) {
        let body = self.links[link % self.links.len()].body;
        self.pushes.push((body, impulse / DT, point));
    }

    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Every link's pose, the torso first, then each leg from the hip down.
    pub fn link_poses(&self) -> Vec<BodyPose> {
        self.links
            .iter()
            .map(|l| {
                let p = self.world.bodies[l.body].position();
                let r = p.rotation;
                BodyPose {
                    position: [p.translation.x, p.translation.y, p.translation.z],
                    rotation: [r.x, r.y, r.z, r.w],
                }
            })
            .collect()
    }

    /// Every link's shape, in [`Sim::link_poses`]'s order.
    pub fn link_shapes(&self) -> Vec<LinkShape> {
        let mut out = vec![LinkShape::Cuboid {
            half: dims::TORSO_HALF,
            offset: [0.0, dims::TORSO_CENTRE_Y, 0.0],
        }];
        for leg in 0..LEGS {
            let s = self.params.leg_scale_factor(leg);
            let (thigh, shin) = (dims::THIGH_LENGTH * s, dims::SHIN_LENGTH * s);
            out.push(LinkShape::Ball {
                radius: dims::SMALL_LINK_RADIUS,
            });
            out.push(LinkShape::Ball {
                radius: dims::SMALL_LINK_RADIUS,
            });
            out.push(LinkShape::Capsule {
                half_height: 0.5 * thigh - dims::THIGH_RADIUS,
                radius: dims::THIGH_RADIUS,
                offset: [0.0, -0.5 * thigh, 0.0],
            });
            out.push(LinkShape::Capsule {
                half_height: 0.5 * shin - dims::SHIN_RADIUS,
                radius: dims::SHIN_RADIUS,
                offset: [0.0, -0.5 * shin, 0.0],
            });
            out.push(LinkShape::Ball {
                radius: dims::SMALL_LINK_RADIUS,
            });
            out.push(LinkShape::Cuboid {
                half: dims::FOOT_HALF,
                offset: dims::FOOT_OFFSET,
            });
        }
        out
    }

    /// The tool on the hull, if it has one: its box's half extents and
    /// centre in the torso's frame, m.
    pub fn tool_shape(&self) -> Option<([f32; 3], [f32; 3])> {
        params_tool(&self.params)
    }

    /// Total mass of the mech, kg.
    pub fn total_mass(&self) -> f32 {
        self.links
            .iter()
            .map(|l| self.world.bodies[l.body].mass())
            .sum()
    }
}

/// A tool lighter than this is no tool, kg.
const TOOL_MIN_KG: f32 = 5.0;

fn params_tool(params: &BodyParams) -> Option<([f32; 3], [f32; 3])> {
    (params.tool_kg() >= TOOL_MIN_KG).then(|| ([0.4, 0.25, 0.2], params.tool_position_m()))
}
