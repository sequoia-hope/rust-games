//! Contacts without the stabilization pass: penetration while standing, and
//! a drop from 1 m.
use mw_sim::mech::BodyParams;
use mw_sim::sim::Sim;
use mw_sim::terrain::Terrain;
fn main() {
    for lift in [0.0f32, 1.0] {
        let terrain = Terrain::flat(200.0);
        let mut sim = Sim::new(BodyParams::default(), &terrain, 0.0, 0.0);
        let mut pose = *sim.torso().position();
        pose.translation.y += lift;
        sim.torso_mut().set_position(pose, true);
        let mut worst: f32 = 0.0;
        for i in 0..2000 {
            sim.step();
            if i > 10 {
                worst = worst.max(dims_bottom(&sim));
            }
        }
        println!(
            "lift {lift}: deepest foot penetration {:.4} m, final torso y {:.3}, upright {}",
            worst,
            sim.torso().translation().y,
            sim.gravity_in_body().y < -0.9
        );
    }
}
fn dims_bottom(sim: &Sim) -> f32 {
    (0..2)
        .map(|l| mw_sim::mech::dims::FOOT_HALF[1] - sim.foot_collider_position(l).y)
        .fold(f32::MIN, f32::max)
}
