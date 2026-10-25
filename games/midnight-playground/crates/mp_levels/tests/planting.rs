//! The open world's planting (docs/vision/peninsula.md, stage 7d): the
//! peninsula's trees and fences are planted the same every time, tile by
//! tile or a cell at a time, cheaply enough to plant as a car drives.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Instant;

use mp_levels::peninsula;
use mp_track::Track;
use mp_track::planting::{Kind, TILE};

fn tile_of(x: f64, z: f64) -> (i64, i64) {
    ((x / TILE).floor() as i64, (z / TILE).floor() as i64)
}

/// The tiles under the loop's road every `step` metres.
fn tiles_along(t: &Track, step: f64) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = Vec::new();
    let mut s = 0.0;
    while s < t.length {
        let f = t.frame(s);
        let k = tile_of(f.x + f.rx * 40.0, f.z + f.rz * 40.0);
        if !out.contains(&k) {
            out.push(k);
        }
        s += step;
    }
    out
}

#[test]
fn the_peninsula_plants_the_same_every_time_and_cheaply() {
    let level = peninsula::level();
    let t0 = Instant::now();
    let a = Track::new(&level).unwrap();
    let built = t0.elapsed();
    let b = Track::new(&level).unwrap();
    // What the solid scenery adds to the level's load: the town's layout
    // and its solids (the planting is made as cars come near).
    let t2 = Instant::now();
    let town = mp_track::seabright::town(&a).expect("a town");
    let _ = town.solids(&|x, z| x + z);
    let laid = t2.elapsed();
    println!("the town's layout: {laid:?}");
    let pa = a
        .props
        .planting()
        .expect("an open level with a cover is planted");
    let pb = b.props.planting().unwrap();
    let tiles = tiles_along(&a, 64.0);
    let t1 = Instant::now();
    let mut kinds = std::collections::BTreeMap::new();
    let (mut solids, mut rails) = (0, 0);
    for &(i, j) in &tiles {
        let ta = pa.tile(&a, i, j);
        for p in &ta.plants {
            *kinds.entry(p.kind).or_insert(0usize) += 1;
        }
        rails += ta.fences.rails.len();
        solids += pa.solids(&a, i, j).len();
    }
    let per = t1.elapsed().as_secs_f64() * 1e3 / tiles.len() as f64 / 2.0;
    println!(
        "Track::new {built:?}; {} tiles beside the loop, {per:.3} ms a tile; {kinds:?}, {rails} rails, {solids} solids",
        tiles.len()
    );
    for &(i, j) in tiles.iter().step_by(7) {
        assert_eq!(pa.tile(&a, i, j), pb.tile(&b, i, j), "tile {i},{j}");
        assert_eq!(pa.solids(&a, i, j), a.props.tile(&a, i, j).to_vec());
    }
    // The loop runs through the redwoods and the oaks, and past fields.
    for k in [
        Kind::Fir,
        Kind::Spruce,
        Kind::Oak,
        Kind::Scrub,
        Kind::Cypress,
    ] {
        assert!(kinds.get(&k).copied().unwrap_or(0) > 20, "{k:?}: {kinds:?}");
    }
    assert!(rails > 50, "{rails} rails");
    // Unoptimised builds (and a busy machine) take several times longer.
    let most = if cfg!(debug_assertions) { 20.0 } else { 2.0 };
    assert!(per < most, "{per} ms a tile");
}

#[test]
fn a_cells_fences_are_its_tiles_fences() {
    let level = peninsula::level();
    let t = Track::new(&level).unwrap();
    let p = t.props.planting().unwrap();
    // Cells by the loop until one has fences.
    let mut done = 0;
    for (i, j) in tiles_along(&t, 400.0) {
        let (ci, cj) = (i.div_euclid(8), j.div_euclid(8));
        let (x0, z0) = (ci as f64 * 8.0 * TILE, cj as f64 * 8.0 * TILE);
        let whole = p.fences(&t, x0, z0, x0 + 8.0 * TILE, z0 + 8.0 * TILE);
        if whole.rails.is_empty() {
            continue;
        }
        let mut posts = Vec::new();
        let mut rails = Vec::new();
        for tj in cj * 8..cj * 8 + 8 {
            for ti in ci * 8..ci * 8 + 8 {
                let f = p.tile(&t, ti, tj).fences;
                posts.extend(f.posts);
                rails.extend(f.rails);
            }
        }
        let key = |a: &[f64; 2]| (a[0].to_bits(), a[1].to_bits());
        let mut wp: Vec<_> = whole.posts.iter().map(key).collect();
        let mut tp: Vec<_> = posts.iter().map(key).collect();
        wp.sort();
        tp.sort();
        assert_eq!(wp, tp);
        let rk = |r: &[[f64; 2]; 2]| (key(&r[0]), key(&r[1]));
        let mut wr: Vec<_> = whole.rails.iter().map(rk).collect();
        let mut tr: Vec<_> = rails.iter().map(rk).collect();
        wr.sort();
        tr.sort();
        assert_eq!(wr, tr);
        done += 1;
        if done == 3 {
            break;
        }
    }
    assert!(done > 0, "no fenced cell by the loop");
}

#[test]
fn half_moon_bay_stands_where_it_is_drawn_the_same_every_time() {
    let level = peninsula::level();
    let a = Track::new(&level).unwrap();
    let b = Track::new(&level).unwrap();
    let town = a
        .props
        .town()
        .expect("the peninsula's Beach zone has its town");
    assert_eq!(Some(town), b.props.town());
    assert_eq!(a.props.fixed(), b.props.fixed());
    eprintln!(
        "town: {} lots, {} houses, {} on the hill, {} palms; {} solids",
        town.lots.len(),
        town.houses.len(),
        town.hill.len(),
        town.palms.len(),
        a.props.fixed().len()
    );
    assert!(town.lots.len() > 8 && town.houses.len() > 40 && town.palms.len() > 40);
    // Nothing of it stands on a road: every solid is clear of every road's
    // tarmac.
    for s in a.props.fixed() {
        for (k, r) in a.roads().enumerate() {
            let d = r.distance_to_road(s.x, s.z, 60.0);
            let Some(ds) = d.s else { continue };
            if !(r.is_loop || (0.0..=r.length).contains(&ds)) {
                continue;
            }
            let f = r.frame(ds.clamp(0.0, r.length));
            let reach = match s.shape {
                mp_track::Shape::Round { r } => r,
                mp_track::Shape::Box { hx, hz, .. } => hx.min(hz),
            };
            assert!(
                d.d - reach > f.hw,
                "a solid on road {k} at s {ds:.0}: {:.1} m from its centre, {:.1} wide",
                d.d,
                f.hw
            );
        }
    }
}
