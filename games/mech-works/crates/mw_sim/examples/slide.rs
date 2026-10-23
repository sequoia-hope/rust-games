//! How far the feet slide while the mech stands, under solver settings
//! (the finding behind `num_internal_stabilization_iterations = 0` in
//! `Sim::new`; the dev log, 2026-10-23). The sim now sets it, so "default"
//! here is the fixed state and the others are variations on it.
use mw_sim::mech::BodyParams;
use mw_sim::rapier3d::prelude::*;
use mw_sim::sim::Sim;
use mw_sim::terrain::Terrain;

type Case = (&'static str, Box<dyn Fn(&mut IntegrationParameters)>);
fn main() {
    let cases: Vec<Case> = vec![
        ("default", Box::new(|_| {})),
        (
            "friction_in_bias",
            Box::new(|p| p.friction_in_bias_pass = true),
        ),
        (
            "stiff contacts",
            Box::new(|p| p.contact_softness = SpringCoefficients::new(120.0, 5.0)),
        ),
        ("warmstart joints", Box::new(|p| p.warmstart_joints = true)),
        ("16 iters", Box::new(|p| p.num_solver_iterations = 16)),
        (
            "stab 4",
            Box::new(|p| p.num_internal_stabilization_iterations = 4),
        ),
        (
            "no correction",
            Box::new(|p| p.normalized_max_corrective_velocity = 0.0),
        ),
        (
            "slow correction",
            Box::new(|p| p.normalized_max_corrective_velocity = 1.0),
        ),
        (
            "allowed err 0.01",
            Box::new(|p| p.normalized_allowed_linear_error = 0.01),
        ),
        (
            "no stab iters",
            Box::new(|p| p.num_internal_stabilization_iterations = 0),
        ),
    ];
    for (name, f) in cases {
        let terrain = Terrain::flat(200.0);
        let mut sim = Sim::new(BodyParams::default(), &terrain, 0.0, 0.0);
        f(&mut sim.world.integration_parameters);
        let x0 = sim.foot_collider_position(0).x;
        let mut max_slide: f32 = 0.0;
        let mut max_torso: f32 = 0.0;
        for _ in 0..800 {
            sim.step();
            max_slide = max_slide.max((sim.foot_collider_position(0).x - x0).abs());
            max_torso = max_torso.max(sim.torso().translation().x.abs());
        }
        println!(
            "{name:18}: foot slide {:.3} m, torso sway {:.3} m",
            max_slide, max_torso
        );
    }
}
