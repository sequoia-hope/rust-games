//! The open world's freeways over its streets (docs/vision/peninsula.md,
//! stage 8; DECISIONS D1248): where a freeway crosses a lesser road it
//! rises over it on a viaduct, with no junction; a car on the street
//! under it stays on the street, one on the deck on the freeway.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use mp_sim::race::LevelRuntime;
use mp_track::network::{OVERPASS_CLEAR, OVERPASS_GRADE};
use mp_track::{JunctionKind, ROAD_TYPES, Track};

fn freeway(t: &Track, r: usize, s: f64) -> bool {
    let rt = t.road(r);
    ROAD_TYPES
        .get(rt.road_type[rt.idx(s.clamp(0.0, rt.length))] as usize)
        .is_some_and(|ty| ty.key == "freeway")
}

/// Every place a freeway passes over another road: (freeway, s on it,
/// the other road, s on it).
fn overpasses(t: &Track) -> Vec<(usize, f64, usize, f64)> {
    t.overpasses
        .iter()
        .map(|j| {
            if freeway(t, j.road, j.s) {
                (j.road, j.s, j.other, j.other_s)
            } else {
                (j.other, j.other_s, j.road, j.s)
            }
        })
        .collect()
}

#[test]
fn the_freeways_pass_over_the_streets() {
    let lr = LevelRuntime::new(mp_levels::level_by_id("peninsula")).unwrap();
    let t = &lr.track;
    // A freeway crosses a road at grade only near one of its own
    // junctions, too near to rise over it.
    let reach = (OVERPASS_CLEAR + 1.0) / OVERPASS_GRADE + 25.0;
    for j in t.junctions.iter().filter(|j| j.kind == JunctionKind::Cross) {
        if freeway(t, j.road, j.s) == freeway(t, j.other, j.other_s) {
            continue;
        }
        let (f, fs) = if freeway(t, j.road, j.s) {
            (j.road, j.s)
        } else {
            (j.other, j.other_s)
        };
        let near = t
            .junctions
            .iter()
            .filter(|k| k.kind != JunctionKind::Cross)
            .any(|k| {
                let at = if k.other == f {
                    k.other_s
                } else if k.road == f {
                    k.s
                } else {
                    return false;
                };
                (at - fs).abs() < reach
            });
        assert!(
            near,
            "a freeway at grade far from its junctions: road {f} at {fs:.0}"
        );
    }
    let over = overpasses(t);
    assert!(over.len() >= 3, "{} overpasses", over.len());
    for &(f, fs, o, os) in &over {
        let deck = t.road(f).surface_y(fs, 0.0);
        let street = t.road(o).surface_y(os, 0.0);
        // `OVERPASS_CLEAR` where it can rise to it; by its own ends,
        // where it comes down to meet its junctions (101 over El Camino 73 m
        // before it ends on it), at least 4 m: 2 m of headroom under the
        // deck's 1.8, more than a car's height.
        assert!(
            deck - street > (OVERPASS_CLEAR - 0.5).min(4.0),
            "road {f} at {fs:.0} over road {o}: {:.1} m",
            deck - street
        );
    }
    // The ramps no steeper than the lift's grade (and the corridor's own).
    for f in 1..=t.branches.len() {
        let ft = t.road(f);
        for k in 1..ft.n {
            if ft.elevated[k] != 0 {
                let g = (ft.py[k] - ft.py[k - 1]).abs() as f64;
                assert!(g < OVERPASS_GRADE + 0.08, "road {f} at {k}: grade {g}");
            }
        }
    }
}

#[test]
fn a_car_under_the_viaduct_stays_on_the_street_and_one_on_it_on_the_freeway() {
    let lr = LevelRuntime::new(mp_levels::level_by_id("peninsula")).unwrap();
    let t = &lr.track;
    let over = overpasses(t);
    assert!(!over.is_empty());
    for &(f, fs, o, os) in &over {
        let of = t.road(o).frame(os);
        let street = t.road(o).surface_y(os, 0.0);
        let (k, _) = t.project_road_at(of.x, of.z, street + 0.3, o, os);
        assert_eq!(k, o, "under road {f} at {fs:.0}: held by {k}");
        let ff = t.road(f).frame(fs);
        let deck = t.road(f).surface_y(fs, 0.0);
        let (k, _) = t.project_road_at(ff.x, ff.z, deck + 0.3, f, fs);
        assert_eq!(k, f, "on road {f} at {fs:.0} over {o}: held by {k}");
        // And the ground under each is its own road's.
        let g = t.ground_at(o, of.x, of.z, os, 0.0);
        assert!((g - street).abs() < 0.05);
    }
}
