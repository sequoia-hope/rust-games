//! The bay side's data (docs/vision/peninsula.md, stage 6b): its corridor
//! files, the ground under their box and the city grid, read as the levels
//! will read them. No level uses them yet.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use mp_levels::ground::{CityGrid, HeightGrid};
use mp_levels::south_coast::Corridor;

macro_rules! corridors {
    ($($id:literal),* $(,)?) => {
        [$(($id, include_str!(concat!("../../../assets/atlas/peninsula/corridors/", $id, ".txt")))),*]
    };
}

/// Every corridor of the bay side's and north coast's network.
const CORRIDORS: [(&str, &str); 22] = corridors![
    "hwy-92-east",
    "hwy-84-east",
    "canada-road",
    "coast-north",
    "skyline-north",
    "sharp-park-road",
    "us-101",
    "i-280",
    "el-camino",
    "ralston",
    "hillsdale",
    "third-ave",
    "broadway",
    "millbrae",
    "san-bruno",
    "sf-great-highway",
    "sf-19th-ave",
    "sf-lombard",
    "sf-van-ness",
    "sf-market",
    "sf-geary",
    "sf-embarcadero",
];

const GROUND: &[u8] =
    include_bytes!("../../../assets/atlas/peninsula/corridors/bayside.ground.bin");
const CITY: &[u8] = include_bytes!("../../../assets/atlas/peninsula/corridors/bayside.city.bin");

/// A real place in game metres: the atlas's projection about Half Moon
/// Bay's intersection (geo.json), at the plan's scale of 0.5.
fn game(lat: f64, lon: f64) -> (f64, f64) {
    const M_PER_DEG_LAT: f64 = 110_986.441_587_133_19;
    const M_PER_DEG_LON: f64 = 88_463.227_503_651_36;
    (
        (lon + 122.4335) * M_PER_DEG_LON * 0.5,
        -(lat - 37.46802) * M_PER_DEG_LAT * 0.5,
    )
}

#[test]
fn every_corridor_parses_and_the_ground_covers_it() {
    let g = HeightGrid::parse(GROUND).unwrap();
    let (x1, z1) = (
        g.x0 + (g.w - 1) as f64 * g.step,
        g.z0 + (g.h - 1) as f64 * g.step,
    );
    for (id, text) in CORRIDORS {
        let c = Corridor::parse(text).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert!(c.x.len() > 50, "{id}: {} samples", c.x.len());
        // The road lies on the ground, near enough: the corridor's profile
        // is smoothed over 60 real metres, and bridges and tunnels keep
        // their own level (280 crosses San Mateo Creek 45 game metres up,
        // the Presidio Parkway runs in tunnels and on viaducts).
        let mut d: Vec<f64> = (0..c.x.len())
            .map(|k| (c.y[k] - g.height(c.x[k], c.z[k])).abs())
            .collect();
        d.sort_by(f64::total_cmp);
        let (p50, p90) = (d[d.len() / 2], d[d.len() * 9 / 10]);
        assert!(
            p50 < 1.0 && p90 < 10.0,
            "{id}: road off the ground by {p50} (median), {p90} (p90)"
        );
        for (k, (&x, &z)) in c.x.iter().zip(&c.z).enumerate() {
            // Inside the grid, with the 1.2 km margin ground.py was given
            // less a cell.
            let m = 1_200.0 - g.step;
            assert!(
                x - g.x0 >= m && x1 - x >= m && z - g.z0 >= m && z1 - z >= m,
                "{id} sample {k} ({x}, {z}) is near the ground's edge"
            );
        }
    }
}

#[test]
fn the_city_grid_shares_the_grounds_box() {
    let g = HeightGrid::parse(GROUND).unwrap();
    let c = CityGrid::parse(CITY).unwrap();
    assert_eq!((c.x0, c.z0, c.step), (g.x0, g.z0, 32.0));
    assert!((c.w - 1) as f64 * c.step >= (g.w - 1) as f64 * g.step);
    assert!((c.h - 1) as f64 * c.step >= (g.h - 1) as f64 * g.step);
}

#[test]
fn the_city_stands_where_the_real_one_does() {
    let c = CityGrid::parse(CITY).unwrap();
    // Downtown San Francisco, Market at Second Street: towers.
    let (x, z) = game(37.7894, -122.4013);
    assert!(c.coverage(x, z).unwrap() > 0.4, "{:?}", c.coverage(x, z));
    assert!(
        c.height_p90(x, z).unwrap() > 100.0,
        "{:?}",
        c.height_p90(x, z)
    );
    // The tallest cell in the grid is downtown's (Salesforce Tower, 326 m),
    // within a few hundred game metres of Market and Second.
    let (mut best, mut at) = (0.0, (0.0, 0.0));
    for j in 0..c.h {
        for i in 0..c.w {
            let (cx, cz) = (c.x0 + i as f64 * c.step, c.z0 + j as f64 * c.step);
            let h = c.height_p90(cx, cz).unwrap();
            if h > best {
                (best, at) = (h, (cx, cz));
            }
        }
    }
    assert!(best > 300.0, "{best}");
    assert!((at.0 - x).hypot(at.1 - z) < 400.0, "{at:?}");
    // Redwood City's downtown, Broadway at Jefferson: built up.
    let near = |lat: f64, lon: f64, f: &dyn Fn(f64, f64) -> f64| -> f64 {
        let (x, z) = game(lat, lon);
        let mut best = 0.0f64;
        for dj in -2..=2 {
            for di in -2..=2 {
                best = best.max(f(x + f64::from(di) * c.step, z + f64::from(dj) * c.step));
            }
        }
        best
    };
    let built = |x: f64, z: f64| c.coverage(x, z).unwrap();
    let tall = |x: f64, z: f64| c.height_p90(x, z).unwrap();
    assert!(near(37.4857, -122.2290, &built) > 0.4);
    assert!(near(37.4857, -122.2290, &tall) > 10.0);
    // Lower Crystal Springs Reservoir: water, nothing built.
    for (lat, lon) in [(37.530, -122.360), (37.515, -122.345), (37.545, -122.372)] {
        let (x, z) = game(lat, lon);
        assert_eq!(c.coverage(x, z), Some(0.0), "({lat}, {lon})");
        assert_eq!(c.height_p50(x, z), Some(0.0), "({lat}, {lon})");
    }
    // Outside the box there is no city at all.
    assert_eq!(c.coverage(c.x0 - 100.0, c.z0), None);
}
