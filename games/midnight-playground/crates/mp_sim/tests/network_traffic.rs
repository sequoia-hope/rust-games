//! Traffic on the open world's network (docs/vision/peninsula.md, stage
//! 6g): a cruise begun in Redwood City has cars round the player on the
//! bay side's roads, driving their roads and carrying on from one to the
//! next at the junctions, all within reach and finite.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use mp_sim::input::InputFrame;
use mp_sim::race::{LevelRuntime, RaceOpts, SimState, step};
use mp_sim::traffic::NET_FAR;
use pk_math::kernel;

#[test]
fn the_city_has_traffic_round_the_player() {
    let mut l = mp_levels::level_by_id("peninsula");
    l.mode = mp_track::Mode::Cruise;
    l.laps = None;
    l.lap_length = None;
    l.start = Some("redwood-city");
    let lr = LevelRuntime::new(l).unwrap();
    let mut st = SimState::new(
        &lr,
        RaceOpts {
            car: "sports",
            seed: 3,
            pursuit: false,
            heat: 1.0,
            physics: Default::default(),
        },
    );
    st.race.countdown = 0.0;
    let mut events = Vec::new();
    let idle = InputFrame::default();
    let mut most = 0;
    let mut roads_seen: Vec<usize> = Vec::new();
    let mut hops = 0;
    let mut last_road: Vec<Option<usize>> = vec![None; st.traffic.cars.len()];
    for tick in 0..(60 * 120) {
        step(&lr, &mut st, &[idle], &mut events);
        events.clear();
        let p = &st.players[0].v;
        assert!(p.road > 2, "the player stays in the city: road {}", p.road);
        let mut active = 0;
        for (i, c) in st.traffic.cars.iter().enumerate() {
            if !c.active {
                last_road[i] = None;
                continue;
            }
            active += 1;
            let v = &c.k.v;
            for x in [v.x, v.y, v.z, c.k.s, c.k.lat] {
                assert!(x.is_finite(), "tick {tick}: car {i} {c:?}");
            }
            let d = kernel::hypot(v.x - p.x, v.z - p.z);
            assert!(d < NET_FAR + 300.0, "tick {tick}: car {i} {d:.0} m off");
            if !roads_seen.contains(&c.k.road) {
                roads_seen.push(c.k.road);
            }
            if let Some(r) = last_road[i]
                && r != c.k.road
            {
                hops += 1;
            }
            last_road[i] = Some(c.k.road);
        }
        most = most.max(active);
    }
    eprintln!("at most {most} cars, on roads {roads_seen:?}, {hops} carried on to another road");
    assert!(most >= 15, "{most} cars");
    assert!(roads_seen.len() >= 3, "{roads_seen:?}");
    assert!(
        roads_seen.iter().all(|&r| r > 2),
        "only the bay side's roads: {roads_seen:?}"
    );
    // A car carries on at a junction now and then, not every tick.
    assert!(hops < 200, "{hops} hops");
}
