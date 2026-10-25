//! Streaming by place (docs/vision/peninsula.md, stage 6d): a level whose
//! `stream_cell` is set is built in squares of ground, which partition it
//! as the stretches of road do: every metre of every road is in exactly
//! one square, every drawable a square keeps stands in it, and the ring
//! round a square is the squares about it.

mod common;

use std::sync::Arc;

use mp_worldgen::section::{self, Section};
use mp_worldgen::stages::LevelSetup;
use mp_worldgen::stream::Planned;
use mp_worldgen::terrain_mesh::TerrainSetup;
use mp_worldgen::world::World;

fn planned() -> Planned {
    let mut level = common::level("peninsula");
    level.stream_cell = Some(1000.0);
    let setup = LevelSetup {
        terrain: TerrainSetup {
            plan: None,
            ..TerrainSetup::default()
        },
        road: None,
    };
    Planned::new(
        level,
        None,
        setup,
        Arc::new(mp_worldgen::scenery::ported),
        Arc::new(|_| Vec::new()),
    )
    .expect("the loop plans")
}

fn built(p: &Planned, sec: Section) -> World {
    let mut b = p.build(sec);
    while !b.is_done() {
        b.step().expect("a section's job");
    }
    let mut w = b.world;
    section::cut(&mut w);
    w
}

#[test]
fn every_metre_of_every_road_is_in_one_square() {
    let p = planned();
    let g = p.places.as_ref().expect("streamed by place");
    let n = p.sections();
    assert_eq!(n as usize, g.cells.len());
    assert!(n > 40, "{n} squares");
    let secs: Vec<Section> = (0..n).map(|k| p.section(k)).collect();
    for (r, t) in p.track.roads().enumerate() {
        let mut s = 0.0;
        while s < t.length - 1.0 {
            let holding = secs
                .iter()
                .filter(|sec| sec.holds_road(r, t, s, 0.0))
                .count();
            assert_eq!(holding, 1, "road {r} metre {s:.0}: in {holding} squares");
            s += 7.0;
        }
    }
    // The section under a point of a road is the square holding it.
    let t = &p.track;
    for s in [100.0, 9000.0, 21000.0] {
        let f = t.frame(s);
        let k = p.section_under(f.x, f.z).expect("on the road");
        assert!(p.section(k).holds_road(0, t, s.floor(), 0.0), "s {s}");
        assert_eq!(p.section_of(0, s), k);
        // The ring: the square itself first, then its neighbours.
        let ring = p.ring(k, 1, 2);
        assert_eq!(ring[0], k);
        assert!(ring.len() > 3 && ring.len() <= 9, "{ring:?}");
    }
    assert_eq!(p.section_under(-90_000.0, -90_000.0), None);
}

#[test]
fn a_square_keeps_only_what_stands_in_it() {
    let p = planned();
    let t = p.track.clone();
    let k = p
        .section_under(t.px[1200] as f64, t.pz[1200] as f64)
        .unwrap();
    let ring = p.ring(k, 1, 1);
    let mut seen: Vec<(usize, String, [i64; 3])> = Vec::new();
    for (si, &q) in ring.iter().take(4).enumerate() {
        let sec = p.section(q);
        let w = built(&p, sec.clone());
        let ds = section::drawables(&w);
        let place = sec.place.as_ref().unwrap();
        for (name, c) in &ds {
            assert!(
                place.contains(c[0], c[2]),
                "square {q}: {name} at ({:.0}, {:.0}) is not in it",
                c[0],
                c[2]
            );
            let key = [
                c[0].round() as i64,
                c[1].round() as i64,
                c[2].round() as i64,
            ];
            assert!(
                !seen
                    .iter()
                    .any(|(j, n, k)| *j != si && n == name && *k == key),
                "{name} at {key:?} in two squares"
            );
            seen.push((si, name.clone(), key));
        }
        if place.runs.iter().any(|r| !r.is_empty()) {
            assert!(
                ds.iter().any(|(n, _)| n == "road"),
                "square {q} has road metres and no road drawn"
            );
        }
    }
}
