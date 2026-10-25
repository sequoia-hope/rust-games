//! The open world's section builder on its own (docs/vision/peninsula.md
//! stage 4c; DECISIONS D1215): the level planned once
//! (`mp_worldgen::stream::Planned`) and its sections built one at a time
//! and packed as bytes (`mp_worldgen::section::pack`), for the client to
//! read back (`unpack`) and spawn. On the web this crate is its own wasm
//! in a worker (`wasm`, and the shim `crates/mp_game/web/builder-worker.js`),
//! so a section's build never holds the main thread; natively the client
//! builds on a thread of its own and does not use it.
//!
//! The client plans the same level with the same modules
//! (`mp_game::stream`), so what the worker builds is what the client
//! would have built: the plan is deterministic from the level alone.

// Only the worker wasm's C-ABI exports (`wasm`) touch raw pointers.
#![cfg_attr(not(feature = "worker"), forbid(unsafe_code))]

use std::sync::Arc;

use mp_worldgen::section;
use mp_worldgen::stages::LevelSetup;
use mp_worldgen::stream::Planned;
use mp_worldgen::terrain_mesh::TerrainSetup;

#[cfg(all(target_arch = "wasm32", feature = "worker"))]
pub mod wasm;

/// The level `id` planned as the client plans a streamed level: the open
/// world's scenery and the living world's modules, no recorded terrain
/// plan.
pub fn plan(id: &str) -> Result<Planned, String> {
    let level = mp_levels::level_by_id(id);
    if !level.open {
        return Err(format!("{id} is not an open level"));
    }
    Planned::new(
        level,
        None,
        LevelSetup {
            terrain: TerrainSetup {
                plan: None,
                ..TerrainSetup::default()
            },
            road: None,
        },
        Arc::new(mp_worldgen::scenery::open_world),
        Arc::new(mp_worldgen::life::modules),
    )
}

/// Section `k` of the plan built to its end, cut, and packed, without the
/// pixels of the textures in `sent` (those an earlier section handed the
/// client), whose keys it adds to `sent` (D1245).
pub fn build(
    p: &Planned,
    k: i64,
    sent: &mut std::collections::BTreeSet<u64>,
) -> Result<Vec<u8>, String> {
    let mut b = p.build(p.section(k));
    while !b.is_done() {
        b.step()?;
    }
    let (wb, stats) = section::finish(b);
    section::pack_sent(&wb, stats, sent)
}

#[cfg(test)]
mod tests {
    #[test]
    fn plans_and_packs_a_section_of_the_loop() {
        let p = super::plan("peninsula").unwrap();
        let k = p.section_at(p.track.start_s);
        let mut sent = Default::default();
        let bytes = super::build(&p, k, &mut sent).unwrap();
        let u = mp_worldgen::section::unpack_sent(&bytes).unwrap();
        assert!(u.stats.kept > 0 && !u.wb.scene.nodes.is_empty());
        assert!(!u.wb.animators.is_empty());
        assert!(u.stripped.is_empty() && u.keys.len() == u.wb.scene.textures.len());
        // Built again, it sends none of its textures' pixels: the client
        // has them.
        let again = super::build(&p, k, &mut sent).unwrap();
        let v = mp_worldgen::section::unpack_sent(&again).unwrap();
        assert_eq!(v.stripped.len(), v.wb.scene.textures.len());
        assert_eq!(v.keys, u.keys);
        // Less by every pixel buffer the textures name, but for a pixel
        // and a line of the pack's list each (a buffer header, its key).
        let s = &u.wb.scene;
        let bufs: std::collections::BTreeSet<u32> = s.textures.iter().map(|t| t.pixels).collect();
        let pixels: usize = bufs.iter().map(|&b| s.buffers[b as usize].data.len()).sum();
        let room = 256 * s.textures.len();
        assert!(
            bytes.len() - again.len() + room >= pixels,
            "{} of {} without {pixels} bytes of pixels",
            again.len(),
            bytes.len()
        );
        assert!(super::plan("sierra").is_err());
    }
}
