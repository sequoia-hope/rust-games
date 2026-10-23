//! Holds the default pose with PD control and reports how the stand
//! settles and how fast the physics steps.
use mw_sim::mech::{BodyParams, joint_design};
use mw_sim::sim::Sim;
use mw_sim::terrain::Terrain;

fn main() {
    let terrain = Terrain::flat(200.0);
    let mut sim = Sim::new(BodyParams::default(), &terrain, 0.0, 0.0);
    let q = sim.joint_angles();
    for (j, a) in q.iter().enumerate() {
        println!(
            "{:12} target {:+.3} read {:+.3}",
            joint_design(j).name,
            sim.targets[j],
            a
        );
    }
    println!(
        "mass {:.0} kg, torso y {:.3}",
        sim.total_mass(),
        sim.torso().translation().y
    );
    let t0 = std::time::Instant::now();
    let steps = 200 * 10;
    for i in 0..steps {
        sim.step();
        if i % 200 == 199 {
            let t = sim.torso();
            let g = sim.gravity_in_body();
            let c = sim.foot_contacts();
            let temps: Vec<String> = sim
                .actuators
                .iter()
                .map(|a| format!("{:.0}", a.temperature))
                .collect();
            println!(
                "t {:5.2} y {:.3} tilt {:.3} v {:.3} feet {:?} temps {}",
                sim.time,
                t.translation().y,
                (g.y + 1.0).abs(),
                t.linvel().length(),
                c.map(|(b, f)| (b, f as i32)),
                temps.join(" ")
            );
        }
    }
    let el = t0.elapsed();
    println!(
        "{steps} steps in {:?}: {:.1} us/step",
        el,
        el.as_secs_f64() * 1e6 / steps as f64
    );
}
