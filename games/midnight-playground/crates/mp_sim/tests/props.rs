//! The open world's solid scenery (docs/vision/peninsula.md, stage 7d;
//! DECISIONS D1248): a car driven off the road at a tree stops against its
//! trunk, on the arcade's physics and on the sim model's; a crop field's
//! fence stops it; scrub does not.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use std::sync::Arc;

use common::{make_vehicle, straight_level};
use mp_sim::input::{Input, InputFrame};
use mp_sim::model::{SimCar, sim_def};
use mp_sim::physics::{CarPhysics, PhysEvent, car_spec};
use mp_sim::vehicle::Vehicle;
use mp_track::planting::{Kind, TILE};
use mp_track::{Cover, Shape, Solid, Track};
use pk_math::kernel;

const DT: f64 = 1.0 / 120.0;

/// A straight open street on flat land, all of it `cover` (none: no
/// planting).
fn track(cover: Option<Cover>) -> Track {
    let mut l = straight_level(1200.0, "street");
    l.open = true;
    l.ground = Some(Arc::new(|_, _| -0.3));
    l.cover = cover.map(|c| -> mp_track::CoverFn { Arc::new(move |_, _| c as u8) });
    Track::new(&l).unwrap()
}

/// The trunks to the road's right between `s0` and `s1`, with their (s, lat).
fn trunks(t: &Track, s0: f64, s1: f64) -> Vec<(Solid, f64, f64)> {
    let mut out = Vec::new();
    let mut s = s0;
    while s < s1 {
        let f = t.frame(s);
        let (x, z) = (f.x + f.rx * 40.0, f.z + f.rz * 40.0);
        for p in t.props.near(t, x, z, 40.0) {
            if !matches!(p.shape, Shape::Round { .. })
                || out.iter().any(|o: &(Solid, f64, f64)| o.0 == p)
            {
                continue;
            }
            let d = t.distance_to_road(p.x, p.z, 200.0);
            let (Some(ps), lat) = (d.s, d.lat) else {
                continue;
            };
            if lat > 0.0 && (s0..s1).contains(&ps) {
                out.push((p, ps, lat));
            }
        }
        s += 40.0;
    }
    out
}

/// The car on the road at `s`, heading square off it to the right.
fn car_at(t: &Track, s: f64) -> (Vehicle, CarPhysics, [f64; 2], [f64; 2]) {
    let spec = car_spec("sports").unwrap();
    let mut v = make_vehicle("sports", spec.mass);
    let mut phys = CarPhysics::new(&v, spec);
    phys.reset(&mut v, t, s, 0.0);
    let f = t.frame(s);
    v.yaw = kernel::atan2(f.rz, f.rx);
    v.vx = 0.0;
    v.vz = 0.0;
    (v, phys, [f.x, f.z], [f.rx, f.rz])
}

/// How far the car's box is into a solid, at worst.
fn depth(t: &Track, v: &Vehicle) -> f64 {
    let (c, s) = (kernel::cos(v.yaw), kernel::sin(v.yaw));
    t.props
        .push_out(t, v.x, v.z, v.y, c, s, v.half_w, v.half_l)
        .map_or(0.0, |p| p.depth)
}

/// Drives flat out for `secs` (the sim model's car when `sim`), or until
/// it is `until` metres off the road: the furthest the car's centre got
/// along `dir` from `from`, the worst depth into a solid, and the impacts.
fn drive(t: &Track, s: f64, sim: bool, secs: f64) -> (f64, f64, usize) {
    drive_until(t, s, sim, secs, f64::INFINITY)
}

fn drive_until(t: &Track, s: f64, sim: bool, secs: f64, until: f64) -> (f64, f64, usize) {
    let (mut v, mut phys, from, dir) = car_at(t, s);
    let inp = Input {
        throttle: 1.0,
        ..Input::default()
    };
    let frame = InputFrame::quantise(&inp);
    let mass = car_spec("sports").unwrap().mass;
    let mut car = sim.then(|| SimCar::new(sim_def("sports", mass), &v, t));
    let (mut far, mut worst, mut hits) = (f64::MIN, 0.0f64, 0);
    for _ in 0..(secs / DT) as usize {
        match &mut car {
            Some(c) => c.update(&mut v, &mut phys, t, DT, &inp, &frame),
            None => phys.update(&mut v, t, DT, &inp),
        }
        for e in phys.events.drain(..) {
            if matches!(e, PhysEvent::Impact { .. }) {
                hits += 1;
            }
        }
        far = far.max((v.x - from[0]) * dir[0] + (v.z - from[1]) * dir[1]);
        worst = worst.max(depth(t, &v));
        if far > until {
            break;
        }
    }
    (far, worst, hits)
}

#[test]
fn a_car_driven_at_a_tree_stops_against_its_trunk() {
    let t = track(Some(Cover::Forest));
    assert!(!t.props.is_empty());
    // The trunk nearest the road on its stretch: nothing stands between.
    let list = trunks(&t, 300.0, 500.0);
    assert!(list.len() > 20, "a forest: {} trunks", list.len());
    let &(tree, s, lat) = list.iter().min_by(|a, b| a.2.total_cmp(&b.2)).unwrap();
    let Shape::Round { r } = tree.shape else {
        unreachable!()
    };
    assert!(lat > 10.0, "trees stand clear of the road: {lat:.1} m");
    for sim in [false, true] {
        let (far, worst, hits) = drive(&t, s, sim, 6.0);
        eprintln!(
            "sim {sim}: trunk {r:.2} m at {lat:.1} m; the car got {far:.1} m, worst depth {worst:.3}, {hits} impacts"
        );
        assert!(
            far < lat - r,
            "sim {sim}: past the trunk ({far:.1} m of {lat:.1})"
        );
        assert!(worst < 0.5, "sim {sim}: {worst:.2} m into the trunk");
        if !sim {
            assert!(hits >= 1, "it hit the tree");
        }
    }
    // Without the planting it drives on past.
    let bare = track(None);
    assert!(bare.props.is_empty());
    let (far, _, _) = drive(&bare, s, false, 6.0);
    assert!(far > lat + 10.0, "the bare land: {far:.1} m");
}

#[test]
fn scrub_lets_the_car_through() {
    let t = track(Some(Cover::Shrub));
    let p = t.props.planting().unwrap();
    // A bush with no trunk within 3 m of the car's way to it and on.
    let list = trunks(&t, 200.0, 1000.0);
    let mut chosen = None;
    'find: for ti in (200.0 / TILE) as i64..(1000.0 / TILE) as i64 {
        for tj in 0..3 {
            for b in p.plants(&t, ti, tj) {
                if b.kind != Kind::Scrub {
                    continue;
                }
                let d = t.distance_to_road(b.x, b.z, 200.0);
                let (Some(s), lat) = (d.s, d.lat) else {
                    continue;
                };
                if !(15.0..60.0).contains(&lat) {
                    continue;
                }
                if list
                    .iter()
                    .all(|o| (o.1 - s).abs() > 3.0 || o.2 > lat + 15.0)
                {
                    chosen = Some((s, lat));
                    break 'find;
                }
            }
        }
    }
    let (s, lat) = chosen.expect("a bush in the clear");
    let (far, worst, hits) = drive_until(&t, s, false, 6.0, lat + 6.0);
    eprintln!("bush at {lat:.1} m: the car got {far:.1} m, {hits} impacts");
    assert!(far > lat + 5.0, "through the scrub: {far:.1} of {lat:.1}");
    assert_eq!((worst, hits), (0.0, 0));
}

#[test]
fn a_crop_fields_fence_stops_the_car() {
    let t = track(Some(Cover::Crop));
    let f = t.frame(400.0);
    let fence = f.hw + f.margin + 2.2;
    for sim in [false, true] {
        let (far, worst, hits) = drive(&t, 400.0, sim, 5.0);
        eprintln!(
            "sim {sim}: fence at {fence:.1} m; the car got {far:.1} m, worst {worst:.3}, {hits} impacts"
        );
        assert!(
            far < fence,
            "sim {sim}: through the fence ({far:.1} m of {fence:.1})"
        );
        assert!(
            far > fence - 4.0,
            "sim {sim}: it reached the fence ({far:.1} m)"
        );
        assert!(worst < 0.5, "sim {sim}: {worst:.2} m into the fence");
    }
    let (far, _, _) = drive(&track(None), 400.0, false, 5.0);
    assert!(far > fence + 10.0, "the bare land: {far:.1} m");
}
