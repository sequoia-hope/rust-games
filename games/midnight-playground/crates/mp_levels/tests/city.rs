//! The city's buildings (docs/vision/peninsula.md, stage 6e) placed on a
//! grid of streets under a built-up field: many of them, none on a road
//! or its sidewalk, none on another, the same every time.

use std::sync::Arc;

use mp_levels::city::{self, Density, WALK};
use mp_track::{Branch, Level, LoopPath, LoopSpec, Mode, Route, Track, Zone};

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

/// Road 0 runs east along z = 0; four streets run south from it 250 m
/// apart, and two cross them east-west at z = 300 and z = 600.
fn level() -> Level {
    let main: Vec<_> = (0..=30).map(|k| (k as f64 * 40.0, 0.0, 10.0)).collect();
    let mut branches = Vec::new();
    for (n, x) in ["a", "b", "c", "d"]
        .into_iter()
        .zip([150.0, 400.0, 650.0, 900.0])
    {
        let p: Vec<_> = (0..=20).map(|k| (x, 1.5 + k as f64 * 40.0, 10.0)).collect();
        branches.push(branch(n, p));
    }
    for (n, z) in ["e", "f"].into_iter().zip([300.0, 600.0]) {
        let p: Vec<_> = (0..=25)
            .map(|k| (151.5 + k as f64 * 30.0, z, 10.0))
            .collect();
        branches.push(branch(n, p));
    }
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

fn place(seed: u32) -> (Track, Vec<mp_track::Footprint>) {
    let t = Track::new(&level()).unwrap();
    let dense = |_: f64, z: f64| Density {
        coverage: if z < 450.0 { 0.45 } else { 0.2 },
        p50: if z < 450.0 { 18.0 } else { 6.0 },
        p90: if z < 450.0 { 90.0 } else { 9.0 },
    };
    let ground: mp_track::GroundFn = Arc::new(|_, _| 10.0);
    let b = city::place(&t, &dense, &ground, seed, 0);
    (t, b)
}

#[test]
fn the_buildings_stand_clear_of_the_roads_and_each_other() {
    let (t, b) = place(7);
    let towers = b.iter().filter(|f| f.style == city::style::TOWER).count();
    let houses = b.iter().filter(|f| f.style == city::style::HOUSE).count();
    eprintln!("{} buildings, {towers} towers, {houses} houses", b.len());
    assert!(b.len() > 150, "{}", b.len());
    assert!(towers > 3 && houses > 10);
    for (i, f) in b.iter().enumerate() {
        for g in &b[i + 1..] {
            assert!(!city::boxes_overlap(f, g, 0.0), "{f:?} and {g:?} overlap");
        }
        for c in f.corners() {
            for rt in t.roads() {
                let d = rt.distance_to_road(c[0], c[1], 30.0);
                if let Some(s) = d.s
                    && s > 0.0
                    && s < rt.length
                {
                    let fr = rt.frame(s);
                    assert!(
                        d.d > fr.hw + fr.margin + WALK - 0.6,
                        "a corner {c:?} is {:.1} m off a road's centre",
                        d.d
                    );
                }
            }
        }
        assert_eq!(f.y, 10.0);
    }
    // MP_CITY_DUMP=<file> writes the roads and buildings, to look at.
    if let Ok(path) = std::env::var("MP_CITY_DUMP") {
        let mut out = String::new();
        for (r, rt) in t.roads().enumerate() {
            for k in (0..rt.n).step_by(5) {
                out += &format!("r {r} {} {} {}\n", rt.px[k], rt.pz[k], rt.hw[k]);
            }
        }
        for f in &b {
            let c = f.corners();
            out += &format!(
                "b {} {} {} {} {} {} {} {} {} {}\n",
                f.style,
                f.height,
                c[0][0],
                c[0][1],
                c[1][0],
                c[1][1],
                c[2][0],
                c[2][1],
                c[3][0],
                c[3][1]
            );
        }
        std::fs::write(path, out).unwrap();
    }
    // The same network and seed place the same buildings.
    assert_eq!(place(7).1, b);
    assert_ne!(place(8).1, b);
    // Drawn into the Track, they stand.
    let mut l = level();
    l.buildings = Some(Arc::new(|t: &Track| {
        let ground: mp_track::GroundFn = Arc::new(|_, _| 10.0);
        city::place(
            t,
            &|_, _| Density {
                coverage: 0.5,
                p50: 20.0,
                p90: 60.0,
            },
            &ground,
            1,
            0,
        )
    }));
    let t = Track::new(&l).unwrap();
    assert!(t.buildings.list.len() > 100);
    let f = &t.buildings.list[0];
    assert!(t.buildings.inside(f.x, f.z));
}
