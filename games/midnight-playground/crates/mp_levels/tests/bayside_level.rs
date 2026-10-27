//! The bay side joined to the loop (docs/vision/peninsula.md, stage 6):
//! each road starts on the road it should, ends open where it meets
//! another, the streets cross, the city stands and the starts resolve.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use mp_levels::peninsula::{self, BAYSIDE, FIRST_BAYSIDE};
use mp_track::{JunctionKind, Track};

fn track() -> Track {
    #[cfg(not(target_arch = "wasm32"))]
    let t0 = std::time::Instant::now();
    let t = Track::new(&peninsula::level()).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("the world's track in {:.2} s", t0.elapsed().as_secs_f64());
    t
}

fn idx(t: &Track, id: &str) -> usize {
    if id.is_empty() {
        return 0;
    }
    1 + t.branches.iter().position(|b| b.id == id).expect(id)
}

#[test]
fn the_bay_side_is_one_network_with_the_loop() {
    let t = track();
    let name = |r: usize| if r == 0 { "loop" } else { t.branches[r - 1].id };
    for (k, b) in t.branches.iter().enumerate().skip(FIRST_BAYSIDE - 1) {
        let crosses = t
            .junctions
            .iter()
            .filter(|j| j.kind == JunctionKind::Cross && (j.road == k + 1 || j.other == k + 1))
            .count();
        eprintln!(
            "{:<15} {:>6.0} m, from {} at {:.0}, {} , {crosses} crossings",
            b.id,
            b.track.length,
            name(b.parent),
            b.s,
            match (b.end_road, b.end) {
                (Some(r), Some(e)) => format!("to {} at {e:.0}", name(r)),
                _ => "a dead end".into(),
            },
        );
    }
    let start = |id: &str| name(t.branches[idx(&t, id) - 1].parent);
    let end = |id: &str| t.branches[idx(&t, id) - 1].end_road.map(name);
    // Over the hill from the loop, and down to the bay.
    assert_eq!(start("hwy-84-east"), "loop");
    assert_eq!(start("hwy-92-east"), "loop");
    assert_eq!(end("hwy-84-east"), Some("us-101"));
    assert_eq!(end("hwy-92-east"), Some("us-101"));
    // Crystal Springs, the north coast, Sweeney Ridge and the city's ocean loop.
    assert_eq!(start("canada-road"), "hwy-84-east");
    assert_eq!(end("canada-road"), Some("hwy-92-east"));
    assert_eq!(start("coast-north"), "loop");
    assert_eq!(end("coast-north"), Some("i-280"));
    assert_eq!(start("skyline-north"), "i-280");
    assert_eq!(end("skyline-north"), Some("coast-north"));
    assert_eq!(start("sharp-park-road"), "skyline-north");
    assert_eq!(end("sharp-park-road"), Some("coast-north"));
    assert!(matches!(
        start("sf-great-highway"),
        "coast-north" | "skyline-north"
    ));
    assert_eq!(end("sf-great-highway"), Some("sf-geary"));
    assert_eq!(start("sf-market"), "sf-embarcadero");
    assert_eq!(end("sf-market"), Some("sf-great-highway"));
    assert_eq!(start("sf-geary"), "sf-great-highway");
    assert_eq!(end("sf-geary"), Some("sf-market"));
    // The spines leave 84 at Redwood City.
    for id in ["i-280", "us-101", "el-camino"] {
        assert_eq!(start(id), "hwy-84-east", "{id}");
    }
    // The streets across run from one spine to another.
    for id in [
        "ralston",
        "hillsdale",
        "third-ave",
        "broadway",
        "millbrae",
        "san-bruno",
    ] {
        assert_eq!(start(id), "us-101", "{id}");
        assert!(end(id).is_some(), "{id} meets another road");
    }
    // The city's streets cross one another, and the freeways pass over
    // the roads they cross (stage 8a).
    let crossings = t
        .junctions
        .iter()
        .filter(|j| j.kind == JunctionKind::Cross)
        .count();
    eprintln!(
        "{} junctions ({crossings} crossings) and {} overpasses",
        t.junctions.len(),
        t.overpasses.len()
    );
    assert!(
        crossings + t.overpasses.len() >= 10,
        "{crossings} crossings and {} overpasses",
        t.overpasses.len()
    );
    assert_eq!(BAYSIDE.len() + FIRST_BAYSIDE, t.roads().count());
}

#[test]
fn the_city_stands_and_the_starts_are_on_their_roads() {
    let t = track();
    let b = &t.buildings.list;
    let towers = b
        .iter()
        .filter(|f| f.style == mp_levels::city::style::TOWER)
        .count();
    let tallest = b.iter().map(|f| f.height).fold(0.0, f64::max);
    eprintln!(
        "{} buildings, {towers} towers, the tallest {tallest:.0} m",
        b.len()
    );
    assert!(b.len() > 5000, "{}", b.len());
    assert!(towers > 50 && tallest > 150.0);
    // Downtown San Francisco is tall; the loop's roads have none.
    let sf = b
        .iter()
        .filter(|f| f.x > 0.0 && f.x < 2500.0 && f.z < -17000.0 && f.z > -18500.0)
        .map(|f| f.height)
        .fold(0.0, f64::max);
    assert!(sf > 100.0, "downtown's tallest {sf:.0} m");
    for f in b {
        let (r, _) = t.nearest_place(f.x, f.z, 900.0).unwrap_or((99, 0.0));
        assert!(
            r >= FIRST_BAYSIDE || r == 99,
            "a building at ({:.0}, {:.0}) by road {r}",
            f.x,
            f.z
        );
    }
    for sp in &t.level.starts {
        let mut l = t.level.clone();
        l.start = Some(sp.id);
        let (r, s) = mp_track::Track::chosen_start(&Track {
            level: l,
            ..t.clone()
        })
        .unwrap();
        assert_eq!(r, idx(&t, sp.road), "{}", sp.id);
        let f = t.road(r).frame(s);
        eprintln!(
            "start {}: road {r} at {s:.0} ({:.0}, {:.0})",
            sp.id, f.x, f.z
        );
        assert!(
            !t.buildings.inside(f.x, f.z),
            "{} is inside a building",
            sp.id
        );
    }
}
