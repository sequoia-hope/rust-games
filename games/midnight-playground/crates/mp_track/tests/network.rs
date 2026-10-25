//! A road network beyond a star (docs/vision/peninsula.md, stage 6c): a
//! branch may leave a branch, a street may cross another in the middle,
//! and the lesser road fits its surface to the greater's where they meet.

use std::sync::Arc;

use mp_track::{
    BRANCH_LIFT, Branch, JunctionKind, Level, LoopPath, LoopSpec, Mode, Route, Track, Zone,
};

fn path(pts: &[(f64, f64, f64)]) -> LoopPath {
    LoopPath {
        x: pts.iter().map(|p| p.0).collect(),
        z: pts.iter().map(|p| p.1).collect(),
        y: Some(pts.iter().map(|p| p.2).collect()),
        ..Default::default()
    }
}

fn spec(p: LoopPath, road: &'static str) -> LoopSpec {
    LoopSpec {
        path: Arc::new(move || Ok(p.clone())),
        base_y: None,
        tags: Vec::new(),
        road: Some(road),
        roads: Vec::new(),
        start_s: Some(60.0),
        elevation_smooth: Some(4.0),
        zones: None,
    }
}

fn branch(id: &'static str, pts: Vec<(f64, f64, f64)>) -> Branch {
    Branch {
        id,
        name: id,
        path: {
            let p = path(&pts);
            Arc::new(move || Ok(p.clone()))
        },
        road: "mountain",
        zone: 0,
        elevation_smooth: None,
        lift: None,
    }
}

/// Road 0 runs east along z = 0 at 10 m. Road 1 ("a") leaves it south at
/// x = 300, flat at 10 m. Road 2 ("b") leaves road 1 east at z = 400, at
/// 10 m. Road 3 ("c") leaves road 0 south at x = 600, climbing 5 cm a
/// metre, so it crosses road 2 at (600, 400) some 20 m above it.
fn level() -> Level {
    let main: Vec<_> = (0..=20).map(|k| (k as f64 * 50.0, 0.0, 10.0)).collect();
    let a: Vec<_> = (0..=16)
        .map(|k| (300.0, 1.5 + k as f64 * 50.0, 10.0))
        .collect();
    let b: Vec<_> = (0..=12)
        .map(|k| (301.5 + k as f64 * 50.0, 400.0, 10.0))
        .collect();
    let c: Vec<_> = (0..=14)
        .map(|k| (600.0, 1.5 + k as f64 * 50.0, 10.0 + k as f64 * 2.5))
        .collect();
    let branches = vec![branch("a", a), branch("b", b), branch("c", c)];
    Level {
        id: "grid",
        mode: Mode::Cruise,
        num: "",
        title: "",
        desc: "",
        laps: None,
        lap_length: None,
        start_height: None,
        start_heading: None,
        start_x: None,
        start_z: None,
        finish_runoff: None,
        elevation_smooth: Some(4.0),
        route: Route::Path(spec(path(&main), "coastal")),
        elevation: None,
        ground: Some(Arc::new(|_, _| 0.0)),
        loose_ground: None,
        cover: None,
        open: true,
        branches,
        stream_cell: None,
        buildings: None,
        starts: Vec::new(),
        start: None,
        sea_y: None,
        zones: vec![Zone {
            key: "coast",
            name: "",
            sub: "",
            landform: "coast",
            scenery: "Coast",
            color: "#000000",
            blend: None,
            blend_offset: None,
            fog: None,
        }],
        sky: Vec::new(),
        sun_azimuth: 0.0,
        moon_dir: None,
        traffic_paint: None,
        traffic: Vec::new(),
        police: None,
        rivals: Vec::new(),
        events: Vec::new(),
    }
}

#[test]
fn a_branch_may_leave_a_branch() {
    let t = Track::new(&level()).unwrap();
    assert_eq!(t.branches.len(), 3);
    assert_eq!(t.branches[0].parent, 0);
    assert_eq!(t.branches[1].parent, 1, "b leaves a");
    assert!((t.branches[1].s - 398.5).abs() < 3.0, "{}", t.branches[1].s);
    assert_eq!(t.branches[2].parent, 0);
    let starts: Vec<_> = t
        .junctions
        .iter()
        .filter(|j| j.kind == JunctionKind::Start)
        .map(|j| (j.road, j.other))
        .collect();
    assert_eq!(starts, [(1, 0), (2, 1), (3, 0)]);
    // b's first metre sits on a's surface.
    let bt = &t.branches[1].track;
    let q = t
        .road(1)
        .project(f64::from(bt.px[0]), f64::from(bt.pz[0]), 400.0);
    assert!((f64::from(bt.py[0]) - t.road(1).surface_y(q.s, q.lat)).abs() < 0.05);
}

#[test]
fn a_street_crossing_another_fits_its_surface_to_it() {
    let t = Track::new(&level()).unwrap();
    let cross: Vec<_> = t
        .junctions
        .iter()
        .filter(|j| j.kind == JunctionKind::Cross)
        .collect();
    assert_eq!(cross.len(), 1, "{:?}", t.junctions);
    let j = cross[0];
    assert_eq!((j.road, j.other), (3, 2), "c crosses b");
    let (c, b) = (t.road(3), t.road(2));
    let fc = c.frame(j.s);
    assert!(
        (fc.x - 600.0).abs() < 1.0 && (fc.z - 400.0).abs() < 2.0,
        "{fc:?}"
    );
    // On b's pavement c is b's surface, lifted.
    for ds in [-4.0, 0.0, 4.0] {
        let f = c.frame(j.s + ds);
        let q = b.project(f.x, f.z, j.other_s);
        let want = b.surface_y(q.s, q.lat) + BRANCH_LIFT;
        assert!((f.y - want).abs() < 0.05, "{ds}: {} vs {want}", f.y);
    }
    // Far from the crossing, c is its own climb again.
    let far = c.frame(j.s + 200.0);
    assert!(
        (far.y - (10.0 + (far.z - 1.5) * 0.05)).abs() < 0.6,
        "{far:?}"
    );
    assert!(
        c.frame(j.s + 60.0).y > b.frame(j.other_s).y + 0.5,
        "eases back"
    );
    // Both roads' fences open at the crossing.
    assert!(c.fence_gaps.iter().any(|g| g.s0 < j.s && g.s1 > j.s));
    assert!(
        b.fence_gaps
            .iter()
            .any(|g| g.s0 < j.other_s && g.s1 > j.other_s)
    );
}

#[test]
fn the_network_index_finds_the_road_under_a_point() {
    let t = Track::new(&level()).unwrap();
    // On b, far from road 0: the car on road 0's hint is handed to b.
    let (r, p) = t.project_road(800.0, 401.0, 0, 800.0);
    assert_eq!(r, 2);
    assert!((p.lat.abs() - 1.0).abs() < 0.1);
    assert_eq!(t.nearest_place(800.0, 420.0, 900.0).map(|p| p.0), Some(2));
    assert_eq!(t.nearest_place(300.0, 700.0, 900.0).map(|p| p.0), Some(1));
    assert_eq!(t.nearest_place(5000.0, 5000.0, 900.0), None);
    let near = t.net.roads_near(600.0, 400.0, 10.0);
    assert!(
        near.contains(&2) && near.contains(&3) && !near.contains(&0),
        "{near:?}"
    );
}
