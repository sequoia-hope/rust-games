//! A cruise can start anywhere on the open world's network
//! (docs/vision/peninsula.md, stage 6f): given a start on a branch, the
//! car begins on that road, and a reset there puts it back on it.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use mp_sim::race::{LevelRuntime, RaceOpts, SimState};
use mp_track::{Mode, StartSpot};

fn cruise(start: Option<&'static str>) -> LevelRuntime {
    let mut l = mp_levels::level_by_id("peninsula");
    l.mode = Mode::Cruise;
    l.laps = None;
    l.lap_length = None;
    l.starts = vec![
        StartSpot {
            id: "kings-mountain",
            name: "Kings Mountain",
            place: "TUNITAS CREEK ROAD",
            road: "tunitas",
            s: -400.0,
        },
        StartSpot {
            id: "crest",
            name: "The crest",
            place: "HIGHWAY 1",
            road: "",
            s: 3000.0,
        },
    ];
    l.start = start;
    LevelRuntime::new(l).unwrap()
}

fn opts() -> RaceOpts {
    RaceOpts {
        car: "sports",
        seed: 1,
        pursuit: false,
        heat: 1.0,
        physics: Default::default(),
    }
}

#[test]
fn a_cruise_starts_where_it_was_asked_to() {
    let lr = cruise(Some("kings-mountain"));
    let t = &*lr.track;
    let (r, s) = t.chosen_start().expect("the start resolves");
    assert_eq!(r, 1);
    assert!((s - (t.road(1).length - 400.0)).abs() < 1e-9);
    let st = SimState::new(&lr, opts());
    let v = &st.players[0].v;
    assert_eq!(v.road, 1, "on Tunitas Creek Road");
    assert!((v.road_s - s).abs() < 1e-6);
    let f = t.road(1).frame(s);
    assert!((v.x - f.x).abs() < 3.0 && (v.z - f.z).abs() < 3.0);

    // On the level's road: the car stands at the start's metres.
    let lr = cruise(Some("crest"));
    let st = SimState::new(&lr, opts());
    let v = &st.players[0].v;
    assert_eq!(v.road, 0);
    assert!((v.s - 3000.0).abs() < 1.0, "{}", v.s);

    // No start, or one that names nothing: the level's own.
    for start in [None, Some("nowhere")] {
        let lr = cruise(start);
        let st = SimState::new(&lr, opts());
        assert_eq!(st.players[0].v.road, 0);
        assert!((st.players[0].v.s - (lr.track.start_s - 5.0)).abs() < 1.0);
    }
}
