//! Level 6 — "Redwood Ridge": dusk-into-fog climb and descent through tall
//! coastal conifers, down through a misty creek valley, and across a suspension
//! span into the illuminated container docks.
//!
//! Every zone transition advances strictly along +X so `Terrain::zone_weights`
//! blends monotonically between Mountain, Valley, and Harbor.

use crate::{police, zone};
use mp_track::{Level, Mode, Route, Segment, seg};

fn segments() -> Vec<Segment> {
    vec![
        // ── REDWOOD RIDGE: tall conifers and marine-layer fog ──────────
        seg(180., 0., 2.).zone(0).road("mountain").tag("start"),
        seg(150., 20., 8.),
        seg(130., -36., 10.),
        seg(140., 28., 11.),
        seg(110., 68., 9.).tag("switchbacks"),
        seg(92., -176., 7.).tag("hairpin"),
        seg(160., 0., 14.),
        seg(92., 176., 7.).tag("hairpin"),
        seg(150., -64., 11.),
        seg(100., -36., 7.).tag("canyon"),
        seg(95., 44., 6.),
        seg(110., -24., 5.),
        seg(160., 0., 2.).tag("summit"),
        seg(150., -30., -16.).tag("descent"),
        seg(160., 48., -18.),
        seg(150., -36., -18.),
        seg(170., 26., -20.),
        seg(180., -8., -20.),
        seg(200., 16., -22.),
        seg(200., -16., -22.),
        seg(220., 12., -24.),
        seg(220., -12., -24.),
        // ── CREEK HOLLOW: orchards and the old creek bridge ────────────
        seg(200., 18., -18.)
            .zone(1)
            .road("valley")
            .tag("valley-entry"),
        seg(220., -28., -18.).tag("farm"),
        seg(200., 22., -16.),
        seg(50., 0., 3.0).tag("crest"),
        seg(50., 0., -3.6),
        seg(220., -20., -12.).tag("farm"),
        seg(240., 8., -8.).tag("bridge"),
        seg(240., 24., -8.).tag("fields"),
        seg(200., -32., -6.).tag("farm"),
        seg(220., 8., -4.4),
        // ── FOGHORN SPAN: suspension bridge into the harbour docks ─────
        seg(220., 10., 0.).zone(2).road("freeway").tag("approach"),
        seg(400., -10., 36.).tag("bridge-up").elevated(),
        seg(840., 0., 0.).tag("bridge").elevated(),
        seg(400., 0., -38.).tag("bridge-down").elevated(),
        seg(240., -24., 0.).tag("port"),
        seg(280., 28., 0.).tag("containers"),
        seg(240., -4., 0.),
        seg(240., 0., 0.).tag("finish"),
    ]
}

pub fn level() -> Level {
    Level {
        id: "redwoods",
        mode: Mode::Race,
        num: "LEVEL 6",
        title: "Redwood Ridge",
        desc: "Climb through dripping coastal conifers as the marine layer rolls over the summit, drop past the creek farms at dusk, and sprint over the foghorn span into the port.",
        laps: None,
        lap_length: None,
        start_height: Some(185.),
        start_heading: Some(0.),
        start_x: None,
        start_z: None,
        finish_runoff: Some(180.),
        elevation_smooth: None,
        route: Route::Segments(segments()),
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
        sea_y: Some(0.),
        zones: vec![
            zone(
                "mountain",
                "REDWOOD RIDGE",
                "Fog in the tall timber",
                "mountain",
                "Mountain",
                "#4e7a5c",
            )
            .fog(0.00055, 0.32),
            zone(
                "valley",
                "CREEK HOLLOW",
                "Orchards under the mist",
                "valley",
                "Valley",
                "#6d9b48",
            )
            .blend(340.),
            zone(
                "harbor",
                "FOGHORN SPAN",
                "Across the water into the docks",
                "harbor",
                "Harbor",
                "#3d7ea8",
            )
            .blend(300.),
        ],
        // Cool overcast dusk in the fog belt, deepening into twilight and sodium-lit night on the bridge.
        sky: vec![
            sky!(
                0.00, 9, 0x38587a, 0xa8bcc8, 0xf2dfc2, 1.9, 0x9eb4c8, 0x4c584a, 1.25, 0x9fb2be,
                0.00042, 1.04, 0.05
            ),
            sky!(
                0.28, 3, 0x2d4668, 0xc89e8c, 0xffa870, 1.35, 0x8698b4, 0x42483f, 0.95, 0x9c8e94,
                0.00046, 1.08, 0.22
            ),
            sky!(
                0.52, -2.5, 0x1a2848, 0x7c5a74, 0xd87458, 0.55, 0x5c6a90, 0x2c3034, 0.6, 0x56506c,
                0.00038, 1.12, 0.55
            ),
            sky!(
                0.76, -8, 0x0c142c, 0x343a5e, 0x98b0dc, 0.34, 0x3c4870, 0x1b1e26, 0.38, 0x28304a,
                0.00034, 1.18, 0.85
            ),
            sky!(
                1.00, -14, 0x060b1c, 0x222844, 0x9ab2e0, 0.3, 0x303a60, 0x161820, 0.32, 0x1c2238,
                0.00032, 1.2, 1.0
            ),
        ],
        sun_azimuth: 2.35,
        moon_dir: None,
        traffic_paint: None,
        traffic: vec![
            traffic!(
                [260, 520],
                [
                    ("sedan", 0.35),
                    ("pickup", 0.3),
                    ("hatch", 0.2),
                    ("van", 0.15)
                ],
                0.8,
                [12, 16]
            ),
            traffic!(
                [170, 340],
                [
                    ("pickup", 0.35),
                    ("sedan", 0.25),
                    ("tractor", 0.15),
                    ("van", 0.15),
                    ("boxtruck", 0.1)
                ],
                0.65,
                [15, 20]
            ),
            traffic!(
                [50, 120],
                [
                    ("sedan", 0.3),
                    ("hatch", 0.2),
                    ("van", 0.15),
                    ("pickup", 0.15),
                    ("boxtruck", 0.2)
                ],
                0,
                [21, 30],
                0.5
            ),
        ],
        police: Some(police(&[2, 3, 5], &[false, false, true])),
        rivals: vec![
            rival!("Razor", "super", 0xe9ecef, 0.988, 528),
            rival!("Rook", "rally", 0x2a6ee8, 0.976, 520),
            rival!("Kaito", "sports", 0x19c46b, 0.966, 505),
            rival!("Marlowe", "electric", 0x1ab8c4, 0.955, 518),
            rival!("Vex", "muscle", 0x8b2cff, 0.942, 540),
        ],
        events: Vec::new(),
    }
}
