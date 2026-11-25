//! Level 7 — "Bayfront Sprint": sunset on the high coastal cliffs, twilight
//! neon along the seaside promenade, and a flat-out midnight run over the
//! double-deck freeway viaduct into downtown Meridian.

use crate::{police, zone};
use mp_track::{Level, Mode, Route, Segment, seg};

fn segments() -> Vec<Segment> {
    vec![
        // ── DEVIL'S SLIDE: coastal headlands at sunset ─────────────────
        seg(180., 0., 1.).zone(0).road("coastal").tag("start"),
        seg(150., -22., 3.),
        seg(140., 36., 2.).tag("cove"),
        seg(160., -14., 4.).tag("lighthouse"),
        seg(130., 44., -2.),
        seg(120., -56., 2.),
        seg(150., 24., -3.).tag("arch"),
        seg(170., -12., 4.).tag("bluff"),
        seg(150., -28., -12.).tag("descent"),
        seg(170., 38., -18.),
        seg(180., -18., -20.),
        seg(200., 8., -22.),
        // ── PACIFICA PROMENADE: twilight boardwalk & pier ──────────────
        seg(200., 16., -15.)
            .zone(1)
            .road("boulevard")
            .tag("town-entry"),
        seg(180., -16., -10.),
        seg(170., 0., 0.).tag("promenade"),
        seg(260., 8., 0.).tag("pier"),
        seg(200., -12., 0.),
        seg(240., 4., 0.).tag("boardwalk"),
        seg(220., -10., 0.).tag("marina"),
        seg(200., 10., 2.),
        // ── EMBARCADERO FREEWAY: viaduct & tunnel into downtown ────────
        seg(220., 18., 4.).zone(2).road("freeway").tag("onramp"),
        seg(280., -18., 2.).tag("merge"),
        seg(340., 14., 0.).tag("outskirts"),
        seg(280., -14., 10.).tag("viaduct-up"),
        seg(400., 0., 0.).tag("viaduct"),
        seg(280., 16., -10.).tag("viaduct-down"),
        seg(260., -16., 0.).tag("tunnel"),
        seg(360., 18., 0.).tag("downtown"),
        seg(260., -18., 0.),
        seg(260., 0., 0.).tag("finish"),
    ]
}

pub fn level() -> Level {
    Level {
        id: "bayfront",
        mode: Mode::Race,
        num: "LEVEL 7",
        title: "Bayfront Sprint",
        desc: "Drop off the coastal headlands as the sun sinks into the Pacific, blast past the lit Ferris wheel on the promenade, and thread the Embarcadero viaduct into downtown.",
        laps: None,
        lap_length: None,
        start_height: Some(92.),
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
                "coast",
                "DEVIL'S SLIDE",
                "High cliffs at sunset",
                "coast",
                "Coast",
                "#b87352",
            ),
            zone(
                "beach",
                "PACIFICA PROMENADE",
                "Neon along the surf",
                "beach",
                "Beach",
                "#d98254",
            )
            .blend(330.),
            zone(
                "city",
                "EMBARCADERO FREEWAY",
                "Double-deck into downtown",
                "city",
                "City",
                "#5a4acb",
            )
            .blend(300.)
            .blend_offset(110.),
        ],
        sky: vec![
            sky!(
                0.00, 6.5, 0x36599c, 0xf4ae72, 0xffae62, 2.8, 0xb0b8d4, 0x6a5648, 1.15, 0xdca47e,
                0.00025, 1.02, 0.05
            ),
            sky!(
                0.26, 1.2, 0x2d427c, 0xff8650, 0xff7438, 2.0, 0x8e8cb8, 0x564642, 0.88, 0xc47e66,
                0.00028, 1.06, 0.2
            ),
            sky!(
                0.50, -3.5, 0x1a2250, 0xa8586c, 0xe06444, 0.55, 0x646494, 0x3a323c, 0.58, 0x5a4664,
                0.00032, 1.12, 0.58
            ),
            sky!(
                0.76, -10.0, 0x090f28, 0x342c54, 0x9ab0e0, 0.3, 0x363e6a, 0x1a1822, 0.3, 0x242440,
                0.00038, 1.18, 0.88
            ),
            sky!(
                1.00, -16.0, 0x040610, 0x241c30, 0x9ab0e0, 0.26, 0x283050, 0x151218, 0.26,
                0x1a1626, 0.0004, 1.2, 1.0
            ),
        ],
        sun_azimuth: -2.4,
        moon_dir: None,
        traffic_paint: None,
        traffic: vec![
            traffic!(
                [230, 460],
                [
                    ("sedan", 0.35),
                    ("hatch", 0.25),
                    ("pickup", 0.2),
                    ("van", 0.2)
                ],
                0.75,
                [13, 18]
            ),
            traffic!(
                [75, 165],
                [
                    ("sedan", 0.32),
                    ("hatch", 0.28),
                    ("van", 0.2),
                    ("pickup", 0.2)
                ],
                0.5,
                [12, 17]
            ),
            traffic!(
                [45, 110],
                [
                    ("sedan", 0.34),
                    ("hatch", 0.2),
                    ("van", 0.14),
                    ("pickup", 0.14),
                    ("boxtruck", 0.18)
                ],
                0,
                [21, 31],
                0.5
            ),
        ],
        police: Some(police(&[2, 3, 5], &[false, false, false])),
        rivals: vec![
            rival!("Razor", "super", 0xe9ecef, 0.992, 530),
            rival!("Marlowe", "electric", 0x1ab8c4, 0.98, 520),
            rival!("Kaito", "sports", 0x19c46b, 0.97, 508),
            rival!("Nina", "rally", 0xff7a1a, 0.956, 502),
            rival!("Duke", "muscle", 0x30343b, 0.942, 535),
        ],
        events: Vec::new(),
    }
}
