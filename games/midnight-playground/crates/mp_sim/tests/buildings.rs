//! The open world's buildings stand (docs/vision/peninsula.md, stage 6e):
//! a car driven flat out at a building across an open road stops against
//! it, or glances off, and never ends up inside it, on the arcade's
//! physics and on the sim model's.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use std::sync::Arc;

use common::{make_vehicle, straight_level};
use mp_sim::input::{Input, InputFrame};
use mp_sim::model::{SimCar, sim_def};
use mp_sim::physics::{CarPhysics, PhysEvent, car_spec};
use mp_track::{Footprint, Track};
use pk_math::kernel;

const DT: f64 = 1.0 / 120.0;

/// A straight open road with a 24 m square building across it 300 m on,
/// turned by `turn` from square to the road.
fn track(turn: f64) -> Track {
    let mut l = straight_level(1200.0, "valley");
    l.open = true;
    l.ground = Some(Arc::new(|_, _| 0.0));
    l.buildings = Some(Arc::new(move |t: &Track| {
        let f = t.frame(300.0);
        let yaw = kernel::atan2(f.fz, f.fx) + turn;
        vec![Footprint {
            x: f.x,
            z: f.z,
            hx: 12.0,
            hz: 12.0,
            ux: kernel::cos(yaw),
            uz: kernel::sin(yaw),
            y: 0.0,
            height: 15.0,
            style: 0,
            seed: 1,
            garage: None,
        }]
    }));
    Track::new(&l).unwrap()
}

/// How far the car's box is into the building, if at all.
fn depth(t: &Track, x: f64, z: f64, yaw: f64, hw: f64, hl: f64) -> f64 {
    t.buildings
        .push_out(x, z, kernel::cos(yaw), kernel::sin(yaw), hw, hl)
        .map_or(0.0, |p| p.depth)
}

#[test]
fn the_arcade_car_stops_against_a_building() {
    for turn in [0.0, 0.5] {
        let t = track(turn);
        assert_eq!(t.buildings.list.len(), 1);
        let spec = car_spec("sports").unwrap();
        let mut v = make_vehicle("sports", spec.mass);
        let mut phys = CarPhysics::new(&v, spec);
        phys.reset(&mut v, &t, 100.0, 0.0);
        let inp = Input {
            throttle: 1.0,
            ..Input::default()
        };
        let (mut worst, mut hits, mut top) = (0.0f64, 0, 0.0f64);
        for _ in 0..(12.0 / DT) as usize {
            phys.update(&mut v, &t, DT, &inp);
            top = top.max(kernel::hypot(v.vx, v.vz));
            for e in phys.events.drain(..) {
                if matches!(e, PhysEvent::Impact { .. }) {
                    hits += 1;
                }
            }
            worst = worst.max(depth(&t, v.x, v.z, v.yaw, v.half_w, v.half_l));
        }
        eprintln!(
            "turn {turn}: top {top:.1} m/s, {hits} impacts, worst depth {worst:.3} m, s {:.0}",
            v.s
        );
        assert!(hits >= 1, "turn {turn}: it hit the building");
        assert!(worst < 0.6, "turn {turn}: {worst:.2} m into the building");
        assert!(
            !t.buildings.inside(v.x, v.z),
            "turn {turn}: inside at the end"
        );
        if turn == 0.0 {
            assert!(
                v.s < 300.0 - 12.0,
                "square on, it stays short of it: s {}",
                v.s
            );
        }
    }
}

#[test]
fn the_sim_car_stops_against_a_building() {
    let t = track(0.0);
    let spec = car_spec("sports").unwrap();
    let mut v = make_vehicle("sports", spec.mass);
    let mut phys = CarPhysics::new(&v, spec);
    phys.reset(&mut v, &t, 100.0, 0.0);
    let mut car = SimCar::new(sim_def("sports", spec.mass), &v, &t);
    let inp = Input {
        throttle: 1.0,
        ..Input::default()
    };
    let frame = InputFrame::quantise(&inp);
    let mut worst = 0.0f64;
    for _ in 0..(12.0 / DT) as usize {
        car.update(&mut v, &mut phys, &t, DT, &inp, &frame);
        worst = worst.max(depth(&t, v.x, v.z, v.yaw, v.half_w, v.half_l));
    }
    eprintln!("sim: worst depth {worst:.3} m, s {:.0}", v.s);
    assert!(worst < 0.8, "{worst:.2} m into the building");
    assert!(v.s < 300.0, "short of the building's far side: s {}", v.s);
}
