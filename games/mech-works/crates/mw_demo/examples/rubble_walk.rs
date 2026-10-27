//! Walks the shipped policy into the level's rubble, natively, and reports
//! how far it gets, what it touches and whether it falls.
//!
//!   cargo run --release -p mw_demo --example rubble_walk -- [policy blob] [secs] [plough]
//!
//! `plough` holds the blade down (a shove whenever it is ready) as a player
//! holding E would, and stops once the route is open.
use mw_demo::lanes::Lanes;
use mw_sim::env::{Command, POLICY_DT};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let blob = args.first().cloned().unwrap_or_else(|| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/policies/teacher-v4.bin"
        )
        .into()
    });
    let secs: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20.0);
    let plough = args.iter().any(|a| a == "plough");
    let mut lanes = Lanes::default();
    lanes.start_level();
    let cfg = lanes.path_cfg.clone();
    let Lanes {
        lanes: list, level, ..
    } = &mut lanes;
    let lane = &mut list[0];
    let level = level.as_mut().unwrap();
    lane.policy = Some(mw_policy::Policy::from_bytes(&std::fs::read(&blob).unwrap()).unwrap());
    let params = lane.env.sim.params.clone();
    lane.reset_at(0, params, mw_demo::level::SITE_X - 12.0, -1.5, 0.0, &cfg);
    lane.env.command = Command {
        forward: 1.0,
        yaw: 0.0,
    };
    let steps = (secs / POLICY_DT) as usize;
    let mut dir = 1.0f32;
    for k in 0..steps {
        // Steer for the route's line, as a player would.
        let t = lane.env.sim.torso();
        let (p, f) = (
            t.translation(),
            t.rotation() * mw_sim::rapier3d::prelude::Vector::X,
        );
        let heading = (-f.z).atan2(f.x);
        // Back and forth through the pass until it is open.
        if p.x > mw_demo::level::SITE_X + 9.0 {
            dir = -1.0;
        } else if p.x < mw_demo::level::SITE_X - 9.0 {
            dir = 1.0;
        }
        let want = (-(mw_demo::level::ROUTE_Z - p.z)).atan2(2.5 * dir);
        let err = (want - heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        lane.env.command.yaw = (1.5 * err).clamp(-0.8, 0.8);
        // Facing well away from where it should go: turn nearly in place.
        lane.env.command.forward = if err.abs() > 0.8 { 0.15 } else { 1.0 };
        level.attach(lane);
        level.want_shove |= plough;
        level.before_step(lane);
        let fell = lane.step(false);
        level.after_step(lane);
        let p = lane.env.sim.torso().translation();
        let w = &lane.env.sim.world;
        let mech = |h: mw_sim::rapier3d::prelude::ColliderHandle| {
            w.colliders[h]
                .parent()
                .is_some_and(|b| w.multibody_joints.rigid_body_link(b).is_some())
        };
        let rock = |h: mw_sim::rapier3d::prelude::ColliderHandle| {
            w.colliders[h].parent().is_some_and(|b| {
                w.bodies[b].is_dynamic() && w.multibody_joints.rigid_body_link(b).is_none()
            })
        };
        let rock_contacts = w
            .narrow_phase
            .contact_pairs()
            .filter(|c| c.has_any_active_contact())
            .filter(|c| {
                (mech(c.collider1) && rock(c.collider2)) || (rock(c.collider1) && mech(c.collider2))
            })
            .count();
        if k % 50 == 0 || fell {
            println!(
                "t {:5.2} x {:6.2} y {:5.2} z {:5.2} rocks in pass {:2}/{} mech-rock contacts {} {}",
                k as f32 * POLICY_DT,
                p.x,
                p.y,
                p.z,
                level.left,
                level.total,
                rock_contacts,
                if fell { "FELL" } else { "" }
            );
        }
        if !level.blocked() {
            println!(
                "route open at t {:.2} after {} shoves, {} falls",
                k as f32 * POLICY_DT,
                level.shoves,
                lane.falls
            );
            break;
        }
        if let Some(t) = lane.fallen_for {
            lane.fallen_for = Some(t + POLICY_DT);
        }
        if lane
            .fallen_for
            .is_some_and(|t| t > mw_demo::lanes::RESET_AFTER)
        {
            level.stand_up(lane, &cfg);
        }
    }
}
