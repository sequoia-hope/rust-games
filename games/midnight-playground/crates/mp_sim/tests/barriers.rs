//! The open world's roadside barriers (docs/vision/peninsula.md, stage
//! 7c; DECISIONS D1247): a fence, a concrete barrier or a railing that
//! the road kit draws is one the car stops against, from the road and
//! from the land behind it; a gap in it (a fence gap, a junction's mouth)
//! is one it drives through; a kerbed street stands nothing.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use std::sync::Arc;

use common::{make_vehicle, straight_level};
use mp_sim::input::Input;
use mp_sim::physics::{CarPhysics, car_spec};
use mp_sim::vehicle::Vehicle;
use mp_track::{FenceGap, Track};
use pk_math::kernel;

const DT: f64 = 1.0 / 120.0;

/// A straight open road of `kind` on flat land.
fn track(kind: &'static str) -> Track {
    let mut l = straight_level(1200.0, kind);
    l.open = true;
    l.ground = Some(Arc::new(|_, _| -0.3));
    Track::new(&l).unwrap()
}

/// The car at `s`, `lat` across, heading `turn` off the road's direction
/// at `speed`.
fn car(t: &Track, s: f64, lat: f64, turn: f64, speed: f64) -> (Vehicle, CarPhysics) {
    let spec = car_spec("sports").unwrap();
    let mut v = make_vehicle("sports", spec.mass);
    let mut phys = CarPhysics::new(&v, spec);
    phys.reset(&mut v, t, s, lat);
    let f = t.frame(s);
    let yaw = kernel::atan2(f.fz, f.fx) + turn;
    v.yaw = yaw;
    v.vx = kernel::cos(yaw) * speed;
    v.vz = kernel::sin(yaw) * speed;
    (v, phys)
}

/// Drives with the throttle at `throttle` for `secs`: the most the car's
/// centre was across (signed toward the right).
fn drive(t: &Track, v: &mut Vehicle, phys: &mut CarPhysics, secs: f64) -> (f64, f64) {
    let inp = Input {
        throttle: 0.5,
        ..Input::default()
    };
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut el = 0.0;
    while el < secs {
        phys.update(v, t, DT, &inp);
        lo = lo.min(v.lat);
        hi = hi.max(v.lat);
        el += DT;
    }
    (lo, hi)
}

#[test]
fn a_freeway_keeps_the_car_off_its_concrete_barrier() {
    let t = track("freeway");
    let f = t.frame(400.0);
    for turn in [0.3, -0.3, 0.8] {
        let (mut v, mut phys) = car(&t, 400.0, 0.0, turn, 30.0);
        let (lo, hi) = drive(&t, &mut v, &mut phys, 3.0);
        let reach = v.half_w.max(v.half_l);
        assert!(hi < f.wall_r, "{turn}: through the right barrier: {hi}");
        assert!(lo > -f.wall_l, "{turn}: through the left: {lo}");
        assert!(
            hi.max(-lo) > f.wall_r - reach - 0.5,
            "{turn}: never reached it"
        );
    }
}

#[test]
fn a_fence_stops_the_car_from_the_land_and_its_gap_lets_it_through() {
    let mut t = track("valley");
    t.fence_gaps.push(FenceGap {
        s0: 380.0,
        s1: 420.0,
        side: 1.0,
    });
    let f = t.frame(400.0);
    // Through the gap and out onto the land.
    let (mut v, mut phys) = car(&t, 385.0, 0.0, 0.6, 12.0);
    drive(&t, &mut v, &mut phys, 2.5);
    assert!(v.lat > f.wall_r + 3.0, "out through the gap: lat {}", v.lat);
    // Back toward the road where the fence stands, from behind it.
    let (mut v, mut phys) = car(&t, 500.0, f.wall_r + 8.0, -0.6, 12.0);
    let (lo, _) = drive(&t, &mut v, &mut phys, 2.5);
    assert!(lo > f.wall_r, "through the fence's back: {lo}");
    // And from the road where it stands, the fence stops it.
    let (mut v, mut phys) = car(&t, 500.0, 0.0, 0.6, 12.0);
    let (_, hi) = drive(&t, &mut v, &mut phys, 2.5);
    assert!(hi < f.wall_r, "through the fence: {hi}");
}

#[test]
fn a_kerbed_street_lets_the_car_off_the_road() {
    let t = track("street");
    let f = t.frame(400.0);
    let (mut v, mut phys) = car(&t, 400.0, 0.0, 0.5, 12.0);
    let (_, hi) = drive(&t, &mut v, &mut phys, 3.0);
    assert!(hi > f.wall_r + 5.0, "kept on: {hi}");
}

#[test]
fn the_peninsula_clears_its_barriers_at_junctions_and_crossings() {
    let lr = mp_sim::race::LevelRuntime::new(mp_levels::level_by_id("peninsula")).unwrap();
    let t = &lr.track;
    assert_eq!(t.edges.len(), t.branches.len() + 1);
    let mut checked = 0;
    for j in &t.junctions {
        let o = t.road(j.other);
        let i = o.idx(if o.is_loop {
            o.wrap(j.other_s)
        } else {
            j.other_s.clamp(0.0, o.length)
        });
        let e = &t.edges[j.other];
        match j.kind {
            mp_track::JunctionKind::Cross => {
                assert_eq!(
                    (e.left[i], e.right[i]),
                    (0, 0),
                    "road {} crossing road {} at {:.0}",
                    j.road,
                    j.other,
                    j.other_s
                );
                checked += 1;
            }
            mp_track::JunctionKind::Start => {
                let side = t.branches[j.road - 1].side;
                assert_eq!(if side < 0.0 { e.left[i] } else { e.right[i] }, 0);
                checked += 1;
            }
            mp_track::JunctionKind::End => {}
        }
    }
    assert!(checked > 10, "{checked} junctions checked");
}
