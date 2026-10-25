//! An open level's roadside (docs/vision/peninsula.md, stage 7b; DECISIONS
//! D1246): past a road's shoulder the ground slopes down to the land
//! rather than stepping, and a town's street has a kerb that rises as a
//! ramp to a pavement the car drives on, not one it falls through.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use std::sync::Arc;

use common::{make_vehicle, straight_level};
use mp_sim::input::Input;
use mp_sim::physics::{CarPhysics, car_spec};
use mp_track::{EDGE_RUN, KERB, KERB_RUN, Track, WALK, WALK_RUN};

const DT: f64 = 1.0 / 120.0;

/// A straight open road of `kind` on flat land 2 m below it, in a town
/// or not.
fn track(kind: &'static str, town: bool) -> Track {
    let mut l = straight_level(1200.0, kind);
    l.open = true;
    l.ground = Some(Arc::new(|_, _| -2.0));
    if town {
        l.zones[0].scenery = "Town";
    }
    Track::new(&l).unwrap()
}

/// The ground across the road at `s`, from the centre out to `to`, every
/// 5 cm: (lat, y).
fn across(t: &Track, s: f64, to: f64) -> Vec<(f64, f64)> {
    let f = t.frame(s);
    (0..=(to / 0.05) as usize)
        .map(|k| {
            let lat = k as f64 * 0.05;
            let (x, z) = (f.x + f.rx * lat, f.z + f.rz * lat);
            (lat, t.ground_at(0, x, z, s, lat))
        })
        .collect()
}

#[test]
fn a_town_street_has_a_kerb_ramp_and_a_pavement() {
    let t = track("street", true);
    assert!(t.walks(600.0));
    let f = t.frame(600.0);
    let edge = f.hw + f.margin;
    let road = t.surface_y(600.0, edge);
    let g = across(&t, 600.0, edge + WALK + WALK_RUN + 3.0);
    let at = |lat: f64| {
        g.iter()
            .min_by(|a, b| (a.0 - lat).abs().total_cmp(&(b.0 - lat).abs()))
            .unwrap()
            .1
    };
    assert!((at(edge) - road).abs() < 1e-6);
    assert!((at(edge + KERB_RUN / 2.0) - road - KERB / 2.0).abs() < 0.02);
    for d in [KERB_RUN + 0.1, 2.0, WALK - 0.1] {
        assert!(
            (at(edge + d) - road - KERB).abs() < 1e-6,
            "the pavement at {d}"
        );
    }
    // Past the pavement's slope, the land as off a road without one.
    let far = edge + WALK + WALK_RUN + 2.0;
    let (x, z) = (f.x + f.rx * far, f.z + f.rz * far);
    let land = t.ground_y(x, z, 600.0, far);
    assert!(
        (at(far) - land).abs() < 0.02,
        "{} back on the land at {land}",
        at(far)
    );
    // Nowhere steeper than the kerb's ramp (going up) or than a slope
    // down to land 2 m below over its run.
    for w in g.windows(2) {
        let slope = (w[1].1 - w[0].1) / 0.05;
        assert!(slope <= KERB / KERB_RUN + 1e-6, "{slope} up at {}", w[0].0);
        assert!(slope > -2.5, "{slope} down at {}", w[0].0);
    }
}

#[test]
fn a_road_out_of_town_slopes_to_the_land() {
    let t = track("valley", false);
    assert!(!t.walks(600.0));
    let f = t.frame(600.0);
    let edge = f.hw + f.margin;
    let g = across(&t, 600.0, edge + EDGE_RUN + 1.0);
    // Down from the shoulder no steeper than the run allows for the
    // drop to the near field (twice the mean, the ease's steepest).
    let drop = g.iter().find(|p| p.0 >= edge).unwrap().1 - g.last().unwrap().1;
    for w in g.windows(2) {
        let slope = (w[0].1 - w[1].1) / 0.05;
        assert!(
            slope <= 2.0 * drop / EDGE_RUN + 0.05,
            "{slope} down at {}",
            w[0].0
        );
    }
}

#[test]
fn the_car_climbs_the_kerb_and_drives_on_the_pavement() {
    let t = track("street", true);
    let spec = car_spec("sports").unwrap();
    let mut v = make_vehicle("sports", spec.mass);
    let mut phys = CarPhysics::new(&v, spec);
    phys.reset(&mut v, &t, 400.0, 0.0);
    // Angled 20° to the right, at 15 m/s.
    let f = t.frame(400.0);
    let yaw = pk_math::kernel::atan2(f.fz, f.fx) + 0.35;
    v.yaw = yaw;
    v.vx = pk_math::kernel::cos(yaw) * 15.0;
    v.vz = pk_math::kernel::sin(yaw) * 15.0;
    let inp = Input {
        throttle: 0.2,
        ..Input::default()
    };
    let edge = f.hw + f.margin;
    let mut air = 0.0f64;
    let mut on_walk = false;
    for _ in 0..600 {
        phys.update(&mut v, &t, DT, &inp);
        if !v.on_ground {
            air += DT;
        }
        let d = v.lat - edge;
        if (KERB_RUN + 0.5..WALK - 0.5).contains(&d) {
            on_walk = true;
            let top = t.walk_y(v.s, v.lat);
            assert!(v.y > top - 0.12, "y {} under the pavement at {top}", v.y);
        }
        if d > WALK - 0.5 {
            break;
        }
    }
    assert!(on_walk, "never reached the pavement (lat {})", v.lat);
    assert!(air < 0.02, "in the air {air:.3} s");
}
