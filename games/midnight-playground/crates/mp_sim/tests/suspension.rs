//! An open level's suspension (docs/vision/peninsula.md, stage 7a;
//! DECISIONS D1246): the arcade car rides a spring over the ground under
//! its four wheels, so a kerb or a bump it meets at speed is taken by the
//! suspension, not thrown into the air, and the body leans with the
//! ground.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use std::sync::Arc;

use common::{make_vehicle, straight_level};
use mp_sim::input::Input;
use mp_sim::physics::{CarPhysics, car_spec};
use mp_track::Track;

const DT: f64 = 1.0 / 120.0;
/// Where along the road the land's features lie, and how far off it the
/// car drives over them.
const AT: f64 = 300.0;
const OFF: f64 = 40.0;

/// A straight open road whose land, `OFF` metres to its side, carries
/// `ground(d)` where `d` is the distance along it past `AT`.
fn track(ground: impl Fn(f64) -> f64 + Send + Sync + 'static) -> Track {
    let mut l = straight_level(1200.0, "valley");
    l.open = true;
    let t0 = Track::new(&l).unwrap();
    let (f0, f1) = (t0.frame(0.0), t0.frame(1.0));
    let (dx, dz) = (f1.x - f0.x, f1.z - f0.z);
    let (ox, oz) = (f0.x, f0.z);
    l.ground = Some(Arc::new(move |x, z| {
        ground((x - ox) * dx + (z - oz) * dz - AT)
    }));
    Track::new(&l).unwrap()
}

/// The car driven straight along the land at `speed` from 100 m before
/// `AT` to 100 m past it: the most it was off the ground (s), the highest
/// it rose over the ground under it (m), and the most it pitched (the
/// normal's tilt from up).
fn drive(t: &Track, speed: f64) -> (f64, f64, f64) {
    let spec = car_spec("sports").unwrap();
    let mut v = make_vehicle("sports", spec.mass);
    let mut phys = CarPhysics::new(&v, spec);
    phys.reset(&mut v, t, AT - 100.0, OFF);
    let f = t.frame(AT - 100.0);
    v.vx = f.fx * speed;
    v.vz = f.fz * speed;
    let inp = Input {
        throttle: 0.3,
        ..Input::default()
    };
    let (mut air, mut longest, mut rise, mut tilt) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    while v.s < AT + 100.0 {
        phys.update(&mut v, t, DT, &inp);
        if v.on_ground {
            air = 0.0;
        } else {
            air += DT;
        }
        longest = longest.max(air);
        // Over the ground under the four wheels.
        let g = phys.suspension.map_or(v.y, |s| s.contact);
        rise = rise.max(v.y - g);
        if let Some(n) = v.ground_normal {
            tilt = tilt.max((1.0 - n[1]).max(0.0));
        }
    }
    (longest, rise, tilt)
}

#[test]
fn a_kerb_at_speed_does_not_throw_the_car() {
    // A 15 cm step up, square: the hardest a kerb can be.
    let t = track(|d| if d > 0.0 { 0.15 } else { 0.0 });
    let (air, rise, _) = drive(&t, 30.0);
    assert!(air < 0.05, "in the air {air:.3} s");
    assert!(rise < 0.2, "rose {rise:.3} m over the ground");
}

/// A hump `h` high and `len` long across the car's path.
fn hump(h: f64, len: f64) -> Track {
    track(move |d| {
        if (0.0..len).contains(&d) {
            h * pk_math::kernel::sin(d / len * core::f64::consts::PI)
        } else {
            0.0
        }
    })
}

#[test]
fn a_bump_at_speed_is_taken_by_the_suspension() {
    // 15 cm over 3 m: never off the ground, up to 45 m/s (160 km/h).
    let t = hump(0.15, 3.0);
    for speed in [15.0, 30.0, 45.0] {
        let (air, rise, _) = drive(&t, speed);
        assert!(air < 0.02, "at {speed} m/s in the air {air:.3} s");
        assert!(rise < 0.16, "at {speed} m/s rose {rise:.3} m");
    }
    // 30 cm over 8 m: not at 25 m/s; a hop at 35.
    let t = hump(0.3, 8.0);
    let r = drive(&t, 25.0);
    assert!(r.0 < 0.02, "{r:?}");
    let (air, rise, _) = drive(&t, 35.0);
    assert!(air < 0.2 && rise < 0.35, "{air:.3} s, {rise:.3} m");
}

#[test]
fn a_crest_still_jumps() {
    // A two-metre ramp up over 12 m and a sheer drop after it: at 35 m/s
    // the car flies, as it should.
    let t = track(|d| {
        if (0.0..12.0).contains(&d) {
            d / 6.0
        } else {
            0.0
        }
    });
    let (air, _, _) = drive(&t, 35.0);
    assert!(air > 0.2, "in the air only {air:.3} s");
}

#[test]
fn the_body_leans_with_the_ground() {
    // A long 10 % slope across the car's path: the normal under the
    // wheels tilts with it.
    let t = track(|d| if d > 0.0 { (d * 0.1).min(20.0) } else { 0.0 });
    let (_, _, tilt) = drive(&t, 15.0);
    // 1 - cos(atan(0.1)) is 0.005.
    assert!(tilt > 0.003, "tilt {tilt}");
}
