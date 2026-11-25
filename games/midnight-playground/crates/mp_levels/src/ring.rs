//! Level 8 — "Midnight Ring": a closed three-lap midnight expressway circuit
//! over elevated viaducts, past twin downtown cores, and through a sunken
//! harbourside tunnel with no civilian traffic.

use std::f64::consts::PI;
use std::sync::Arc;

use crate::zone;
use mp_track::{Level, LoopPath, LoopSpec, LoopTag, Mode, Route};
use pk_math::js;
use pk_math::kernel::{cos, hypot, sin};

pub fn loop_path() -> LoopPath {
    let (r0, n) = (740.0, 2400);
    let terms = [
        (2.0, 0.15, 0.4),
        (3.0, 0.065, 1.15),
        (5.0, 0.032, 2.05),
        (7.0, 0.016, 0.55),
    ];
    let mut x = Vec::with_capacity(n);
    let mut z = Vec::with_capacity(n);
    for i in 0..n {
        let th = (i as f64 / n as f64) * PI * 2.0;
        let mut r = 1.0;
        for (k, a, ph) in terms {
            r += a * cos(k * th + ph);
        }
        x.push(cos(th) * r * r0 * 1.18);
        z.push(sin(th) * r * r0);
    }
    LoopPath {
        x,
        z,
        ..LoopPath::default()
    }
}

fn loop_length() -> f64 {
    let p = loop_path();
    let n = p.x.len();
    let mut sum = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        sum += hypot(p.x[j] - p.x[i], p.z[j] - p.z[i]);
    }
    js::round(sum)
}

fn tag(tag: &'static str, f0: f64, f1: f64, elevated: Option<f64>) -> LoopTag {
    LoopTag {
        tag,
        s0: None,
        s1: None,
        f0: Some(f0),
        f1: Some(f1),
        elevated,
    }
}

pub fn level() -> Level {
    let night = sky!(
        0, -16, 0x040612, 0x2c1e36, 0x9eb4e4, 0.3, 0x303658, 0x18141e, 0.32, 0x1e182a, 0.00036,
        1.22, 1
    );
    Level {
        id: "ring",
        mode: Mode::Race,
        num: "LEVEL 8",
        title: "Midnight Ring",
        desc: "Three flat-out laps on a closed inner-ring expressway after midnight: high-speed sweeping viaducts, twin neon skyscraper canyons, and a tunnel straight with traffic cleared.",
        laps: Some(3),
        lap_length: Some(loop_length()),
        start_height: None,
        start_heading: None,
        start_x: None,
        start_z: None,
        finish_runoff: None,
        elevation_smooth: None,
        route: Route::Loop(LoopSpec {
            path: Arc::new(|| Ok(loop_path())),
            base_y: Some(20.),
            tags: vec![
                tag("viaduct", 0.10, 0.23, Some(10.)),
                tag("downtown", 0.25, 0.39, None),
                tag("tunnel", 0.43, 0.48, None),
                tag("viaduct", 0.54, 0.67, Some(11.)),
                tag("downtown", 0.70, 0.84, None),
            ],
            road: None,
            roads: Vec::new(),
            start_s: None,
            elevation_smooth: None,
            zones: None,
        }),
        elevation: None,
        ground: None,
        loose_ground: None,
        cover: None,
        open: false,
        branches: Vec::new(),
        stream_cell: None,
        buildings: None,
        starts: Vec::new(),
        start: None,
        sea_y: None,
        zones: vec![zone(
            "city",
            "MIDNIGHT RING",
            "Closed inner-loop expressway",
            "city",
            "City",
            "#e0487a",
        )],
        sky: vec![night, mp_track::SkyKey { s: 1.0, ..night }],
        sun_azimuth: 0.2,
        moon_dir: None,
        traffic_paint: None,
        traffic: vec![traffic!([300, 500], [], 0, [0, 0])],
        police: None,
        rivals: vec![
            rival!("Razor", "super", 0xe9ecef, 0.992, 535),
            rival!("Marlowe", "electric", 0x1ab8c4, 0.982, 525),
            rival!("Kaito", "sports", 0x19c46b, 0.972, 510),
            rival!("Vex", "muscle", 0x8b2cff, 0.96, 545),
            rival!("Rook", "rally", 0x2a6ee8, 0.95, 520),
        ],
        events: Vec::new(),
    }
}
