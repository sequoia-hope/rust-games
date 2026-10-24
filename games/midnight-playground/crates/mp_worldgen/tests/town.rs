//! The Town kit (docs/vision/peninsula.md, stage 6e) draws the level's
//! buildings: the loop with a built-up Half Moon Bay, streamed by place,
//! draws in each square the buildings that stand in it.

mod common;

use std::sync::Arc;

use mp_levels::city::{self, Density};
use mp_worldgen::section;
use mp_worldgen::stages::LevelSetup;
use mp_worldgen::stream::Planned;
use mp_worldgen::terrain_mesh::TerrainSetup;

#[test]
fn a_square_draws_the_buildings_standing_in_it() {
    let mut level = common::level("peninsula");
    level.stream_cell = Some(1000.0);
    level.zones[1].scenery = "Town";
    let ground = level.ground.clone().unwrap();
    level.buildings = Some(Arc::new(move |t: &mp_track::Track| {
        // Built up within 1.5 km of the town's intersection (the origin).
        let d = |x: f64, z: f64| {
            let near = (x * x + z * z).sqrt() < 1500.0;
            Density {
                coverage: if near { 0.4 } else { 0.0 },
                p50: 9.0,
                p90: 30.0,
            }
        };
        city::place(t, &d, &ground, 3, 0)
    }));
    let setup = LevelSetup {
        terrain: TerrainSetup {
            plan: None,
            ..TerrainSetup::default()
        },
        road: None,
    };
    let p = Planned::new(
        level,
        None,
        setup,
        Arc::new(mp_worldgen::scenery::ported),
        Arc::new(|_| Vec::new()),
    )
    .unwrap();
    let n = p.track.buildings.list.len();
    assert!(n > 50, "{n} buildings in town");
    let k = p.section_under(0.0, 0.0).expect("the town is a square");
    let sec = p.section(k);
    let mut b = p.build(sec.clone());
    while !b.is_done() {
        b.step().unwrap();
    }
    let mut w = b.world;
    section::cut(&mut w);
    let town: Vec<_> = section::drawables(&w)
        .into_iter()
        .filter(|(g, _)| g == "town")
        .collect();
    let mine = p
        .track
        .buildings
        .list
        .iter()
        .filter(|f| sec.place.as_ref().unwrap().contains(f.x, f.z))
        .count();
    eprintln!(
        "{n} buildings, {mine} in the square, {} town chunks",
        town.len()
    );
    assert!(mine > 10 && !town.is_empty());
    for (_, c) in &town {
        assert!(sec.place.as_ref().unwrap().contains(c[0], c[2]), "{c:?}");
    }
}
