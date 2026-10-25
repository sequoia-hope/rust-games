//! The open world's parking garages (docs/vision/peninsula.md, stage 8c;
//! DECISIONS D1249): a car drives in from the pavement by the entrance,
//! up the ramps deck by deck, and is stopped by the walls.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use std::sync::Arc;

use common::{make_vehicle, straight_level};
use mp_sim::input::Input;
use mp_sim::physics::{CarPhysics, car_spec};
use mp_sim::vehicle::Vehicle;
use mp_track::garage::{DECK_H, Garage, RAMP_L};
use mp_track::{Footprint, Track};
use pk_math::kernel;

const DT: f64 = 1.0 / 120.0;

/// A straight open road on flat land with a four-deck garage on its right
/// at 300 m, its front on the street, floor at the road's height.
fn track() -> Track {
    let mut l = straight_level(1200.0, "street");
    l.open = true;
    l.ground = Some(Arc::new(|_, _| 0.0));
    l.buildings = Some(Arc::new(|t: &Track| {
        let f = t.frame(300.0);
        let lat = f.hw + f.margin + 4.3 + 20.0;
        vec![Footprint {
            x: f.x + f.rx * lat,
            z: f.z + f.rz * lat,
            hx: 24.0,
            hz: 20.0,
            ux: f.fx,
            uz: f.fz,
            y: 0.0,
            height: 3.0 * DECK_H + 1.1,
            style: 4,
            seed: 1,
            garage: Some(Garage {
                decks: 4,
                front: -1.0,
                floor: t.surface_y(300.0, 0.0),
            }),
        }]
    }));
    Track::new(&l).unwrap()
}

/// World (x, z) of the garage's local (a, b).
fn at(f: &Footprint, a: f64, b: f64) -> (f64, f64) {
    (f.x + a * f.ux - b * f.uz, f.z + a * f.uz + b * f.ux)
}

/// A car at the garage's local (a, b), heading along `dir` (local unit
/// a, b), on the floor, at `speed`.
fn car(t: &Track, a: f64, b: f64, dir: (f64, f64), y: f64, speed: f64) -> (Vehicle, CarPhysics) {
    let g = &t.buildings.list[0];
    let spec = car_spec("sports").unwrap();
    let mut v = make_vehicle("sports", spec.mass);
    let mut phys = CarPhysics::new(&v, spec);
    phys.reset(&mut v, t, 300.0, 0.0);
    let (x, z) = at(g, a, b);
    let (dx, dz) = (dir.0 * g.ux - dir.1 * g.uz, dir.0 * g.uz + dir.1 * g.ux);
    v.x = x;
    v.z = z;
    v.y = y;
    v.yaw = kernel::atan2(dz, dx);
    v.vx = dx * speed;
    v.vz = dz * speed;
    v.project_onto(t);
    (v, phys)
}

fn drive(t: &Track, v: &mut Vehicle, phys: &mut CarPhysics, throttle: f64, secs: f64) {
    let inp = Input {
        throttle,
        ..Input::default()
    };
    let mut el = 0.0;
    while el < secs {
        phys.update(v, t, DT, &inp);
        el += DT;
    }
}

#[test]
fn the_ramps_and_decks_are_where_the_layout_says() {
    let t = track();
    let f = &t.buildings.list[0];
    let g = f.garage.unwrap();
    let (s0, s1) = g.strip(f.hz);
    let sb = (s0 + s1) / 2.0;
    // Up the strip from the floor.
    assert!((g.ground(f.hz, -RAMP_L / 2.0, sb, g.floor) - g.floor).abs() < 1e-9);
    assert!(
        (g.ground(f.hz, 0.0, sb, g.floor + DECK_H / 2.0) - g.floor - DECK_H / 2.0).abs() < 1e-9
    );
    assert!((g.ground(f.hz, RAMP_L / 2.0, sb, g.floor + DECK_H) - g.floor - DECK_H).abs() < 1e-9);
    // On a deck, the deck, not the one over it.
    assert!((g.ground(f.hz, 0.0, 0.0, g.floor + DECK_H) - g.floor - DECK_H).abs() < 1e-9);
    assert!((g.ground(f.hz, 0.0, 0.0, g.floor + 0.1) - g.floor).abs() < 1e-9);
    // Its walls: the perimeter, the entrance's lintel, the strip's edge.
    assert_eq!(f.garage_walls().len(), 7);
}

#[test]
fn a_car_drives_up_the_ramp_to_the_next_deck() {
    let t = track();
    let f = t.buildings.list[0];
    let g = f.garage.unwrap();
    let (s0, s1) = g.strip(f.hz);
    let (mut v, mut phys) = car(
        &t,
        -RAMP_L / 2.0 - 4.0,
        (s0 + s1) / 2.0,
        (1.0, 0.0),
        g.floor,
        6.0,
    );
    drive(&t, &mut v, &mut phys, 0.25, 6.0);
    let (a, _) = f.local(v.x, v.z);
    assert!(a > RAMP_L / 2.0, "up the ramp: a {a}");
    assert!(
        (v.y - g.floor - DECK_H).abs() < 0.25,
        "on the deck above: y {} (floor {})",
        v.y,
        g.floor
    );
}

#[test]
fn the_back_wall_stops_a_car_and_the_entrance_lets_one_in() {
    let t = track();
    let f = t.buildings.list[0];
    let g = f.garage.unwrap();
    // Across the floor at the back wall.
    let (mut v, mut phys) = car(&t, 0.0, -5.0, (0.0, 1.0), g.floor, 10.0);
    drive(&t, &mut v, &mut phys, 0.4, 3.0);
    let (_, b) = f.local(v.x, v.z);
    assert!(b < f.hz, "through the back wall: b {b}");
    // From the pavement in by the entrance, the front being at -b.
    let (e0, e1) = g.entrance(f.hx);
    let (mut v, mut phys) = car(&t, (e0 + e1) / 2.0, -f.hz - 2.5, (0.0, 1.0), g.floor, 5.0);
    drive(&t, &mut v, &mut phys, 0.2, 2.0);
    let (_, b) = f.local(v.x, v.z);
    assert!(b > -f.hz + 3.0, "never got in: b {b}");
    // And by the wall beside the entrance, not.
    let (mut v, mut phys) = car(&t, e1 + 8.0, -f.hz - 2.5, (0.0, 1.0), g.floor, 5.0);
    drive(&t, &mut v, &mut phys, 0.2, 2.0);
    let (_, b) = f.local(v.x, v.z);
    assert!(b < -f.hz, "through the front wall: b {b}");
}

#[test]
fn the_peninsula_has_garages_on_its_pavements() {
    let lr = mp_sim::race::LevelRuntime::new(mp_levels::level_by_id("peninsula")).unwrap();
    let t = &lr.track;
    let garages: Vec<&Footprint> = t
        .buildings
        .list
        .iter()
        .filter(|f| f.garage.is_some())
        .collect();
    assert!(garages.len() >= 5, "{} garages", garages.len());
    eprintln!("{} garages", garages.len());
}
