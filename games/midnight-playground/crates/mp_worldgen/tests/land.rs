//! The Land kit plants the open world by its land cover
//! (docs/vision/peninsula.md, stage 4a; D1212): on Skyline, which the
//! atlas has as forest nearly the whole way, the verge and the hillsides
//! fill with conifers; nothing stands on the road, under the sea or past
//! the kit's reach; and a cell plants the same whatever else is built.

mod common;

use mp_track::planting::TILE;
use mp_worldgen::land::{CELL, Land, REACH};
use mp_worldgen::section::Section;
use mp_worldgen::world::{Scenery, SceneryInfo};

fn build(s0: f64, s1: f64) -> (mp_worldgen::world::World, Land) {
    let (level, t, tr) = common::terrain("peninsula", None);
    let mut w = common::world("peninsula");
    w.section = Some(Section {
        s0,
        s1,
        terrain_radius: 600.0,
        scenery_radius: 600.0,
        scenery: true,
        ..Section::default()
    });
    w.track = Some(t);
    w.terrain = Some(tr);
    let info = SceneryInfo {
        name: "Land",
        zone: level
            .zones
            .iter()
            .position(|z| z.scenery == "Land")
            .unwrap(),
        key: "skyline",
    };
    let mut land = Land::new(&info);
    land.build(&mut w).expect("the land builds");
    (w, land)
}

#[test]
fn skyline_is_planted_with_conifers_clear_of_the_road() {
    let (w, land) = build(17_000.0, 17_600.0);
    let c = land.counts;
    eprintln!("land: {c:?}");
    assert!(c.cells >= 2 && c.cells <= 16, "{c:?}");
    assert!(c.conifers > 300, "{c:?}");
    assert!(c.far_conifers > 100, "{c:?}");
    assert!(c.conifers > c.oaks * 5, "{c:?}");
    // Every instance stands clear of the road, above the sea, within reach.
    let t = w.track.as_ref().unwrap();
    let tr = w.terrain.as_ref().unwrap();
    let root = mp_worldgen::land::group_of(&w.graph, w.root).expect("the land group");
    let mut n = 0;
    for &m in &w.graph.get(root).children {
        let o = w.graph.get(m);
        let inst = o.instances.as_ref().expect("instanced");
        for k in 0..inst.count as usize {
            let mat = inst.get_matrix_at(k);
            let (x, y, z) = (mat.elements[12], mat.elements[13], mat.elements[14]);
            let f = tr.far(x, z);
            // The planting measures from the roads' samples; the far field
            // is a 32 m raster's blend.
            assert!(
                f.d <= REACH + 32.0,
                "an instance {:.0} m from the road",
                f.d
            );
            // Clear of the roads themselves (the far field's distance is a
            // raster's, and to the nearest road of any).
            let clear = mp_track::planting::road_clearance(t, x, z, 80.0);
            assert!(
                clear > 1.5,
                "an instance on the road: {clear:.1} m out at ({x:.1}, {z:.1})"
            );
            assert!(y > 1.0, "an instance under the sea at y {y:.1}");
            n += 1;
        }
    }
    assert_eq!(
        n,
        c.conifers
            + c.far_conifers
            + c.oaks
            + c.scrub
            + c.coast_trees
            + c.rocks
            + c.posts
            + c.rails
    );
}

#[test]
fn a_cell_plants_the_same_whatever_section_asks_for_it() {
    // Two sections that both reach one cell: that cell's instances agree.
    let (a, _) = build(17_000.0, 17_300.0);
    let (b, _) = build(17_300.0, 17_600.0);
    let items = |w: &mp_worldgen::world::World| {
        let root = mp_worldgen::land::group_of(&w.graph, w.root).unwrap();
        let mut v: Vec<(i64, i64, usize, Vec<[f64; 3]>)> = Vec::new();
        for &m in &w.graph.get(root).children {
            let o = w.graph.get(m);
            let inst = o.instances.as_ref().unwrap();
            let mat = inst.get_matrix_at(0);
            let (ci, cj) = (
                (mat.elements[12] / CELL).floor() as i64,
                (mat.elements[14] / CELL).floor() as i64,
            );
            let pts = (0..inst.count as usize)
                .map(|k| {
                    let m = inst.get_matrix_at(k);
                    [m.elements[12], m.elements[13], m.elements[14]]
                })
                .collect();
            v.push((ci, cj, o.geometry.map_or(0, |g| g.0 as usize), pts));
        }
        v
    };
    let (ia, ib) = (items(&a), items(&b));
    let shared: Vec<_> = ia
        .iter()
        .filter(|x| ib.iter().any(|y| (y.0, y.1, y.2) == (x.0, x.1, x.2)))
        .collect();
    assert!(!shared.is_empty(), "the two sections share a cell");
    for x in shared {
        let y = ib
            .iter()
            .find(|y| (y.0, y.1, y.2) == (x.0, x.1, x.2))
            .unwrap();
        assert_eq!(x.3, y.3, "cell ({}, {}) template {}", x.0, x.1, x.2);
    }
}

#[test]
fn what_is_drawn_is_what_the_simulation_plants() {
    // Stage 7d (D1248): every plant, post and rail of a drawn cell is the
    // planting's, where the planting put it, and nothing else is drawn.
    let (w, land) = build(17_000.0, 17_600.0);
    let t = w.track.as_ref().unwrap();
    let p = t.props.planting().expect("the peninsula is planted");
    let root = mp_worldgen::land::group_of(&w.graph, w.root).unwrap();
    let mut drawn: Vec<(f64, f64)> = Vec::new();
    for &m in &w.graph.get(root).children {
        let inst = w.graph.get(m).instances.as_ref().unwrap();
        for k in 0..inst.count as usize {
            let mat = inst.get_matrix_at(k);
            drawn.push((mat.elements[12], mat.elements[14]));
        }
    }
    let mut cells: Vec<(i64, i64)> = drawn
        .iter()
        .map(|&(x, z)| ((x / CELL).floor() as i64, (z / CELL).floor() as i64))
        .collect();
    cells.sort();
    cells.dedup();
    let per = (CELL / TILE) as i64;
    let mut placed: Vec<(f64, f64)> = Vec::new();
    let mut trunks = 0;
    for &(ci, cj) in &cells {
        for tj in cj * per..(cj + 1) * per {
            for ti in ci * per..(ci + 1) * per {
                for q in p.plants(t, ti, tj) {
                    placed.push((q.x, q.z));
                }
                trunks += t.props.tile(t, ti, tj).len();
            }
        }
        let (x0, z0) = (ci as f64 * CELL, cj as f64 * CELL);
        let f = p.fences(t, x0, z0, x0 + CELL, z0 + CELL);
        placed.extend(f.posts.iter().map(|q| (q[0], q[1])));
        placed.extend(
            f.rails
                .iter()
                .map(|r| ((r[0][0] + r[1][0]) / 2.0, (r[0][1] + r[1][1]) / 2.0)),
        );
    }
    eprintln!(
        "{} cells: {} drawn, {} placed, {} solids",
        cells.len(),
        drawn.len(),
        placed.len(),
        trunks
    );
    assert_eq!(drawn.len(), placed.len());
    // Drawn at f32: each drawn item matches a placed one within a millimetre.
    let mut left = placed.clone();
    for d in &drawn {
        let k = left
            .iter()
            .position(|q| (d.0 - q.0).abs() < 1e-3 && (d.1 - q.1).abs() < 1e-3)
            .unwrap_or_else(|| panic!("drawn at {d:?}, placed nowhere near"));
        left.swap_remove(k);
    }
    assert!(trunks > land.counts.conifers / 2, "{trunks} solids");
}
