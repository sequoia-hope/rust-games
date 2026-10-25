//! The open world's beach town (docs/vision/peninsula.md, stage 7d;
//! D1248): on the peninsula the Beach kit draws Half Moon Bay where the
//! simulation laid it out (`mp_track::seabright`), so the storefronts,
//! houses and palms a car stops against are the ones drawn.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use mp_worldgen::beach::Beach;
use mp_worldgen::world::{Scenery, SceneryInfo};

#[test]
fn the_peninsulas_beach_town_draws_its_layout() {
    let (level, t, tr) = common::terrain("peninsula", None);
    let mut w = common::world("peninsula");
    w.track = Some(t);
    w.terrain = Some(tr);
    let zone = level
        .zones
        .iter()
        .position(|z| z.scenery == "Beach")
        .unwrap();
    let info = SceneryInfo {
        name: "Beach",
        zone,
        key: level.zones[zone].key,
    };
    let mut beach = Beach::new(&info);
    beach.plan(&mut w).expect("the beach plans");
    beach.build(&mut w).expect("the beach builds");
    let t = w.track.as_ref().unwrap();
    let town = t.props.town().expect("the town is laid out");
    eprintln!(
        "{} lots, {} houses, {} hill houses, {} palms drawn of {}",
        town.lots.len(),
        town.houses.len(),
        town.hill.len(),
        beach.palm_count,
        town.palms.len()
    );
    assert_eq!(beach.palm_count, town.palms.len());
    assert_eq!(beach.cross_s, town.cross_s);
    assert_eq!(
        (beach.town_s0, beach.town_s1, beach.pier_s),
        (town.town_s0, town.town_s1, town.pier_s)
    );
    // The hill houses the plan cut terraces for are the layout's, but for
    // any on another road.
    for h in &town.hill {
        assert!(
            beach
                .hill_houses
                .iter()
                .any(|p| (p.x, p.z, p.yaw, p.seed) == (h.at.x, h.at.z, h.at.yaw, h.seed)),
            "{h:?}"
        );
    }
}
