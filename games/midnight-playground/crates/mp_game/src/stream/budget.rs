//! The stream's work budgets, where no GPU times anything (docs/vision/
//! peninsula.md, stage 6a; DECISIONS D1238). Every section of the loop and
//! its branches is built as the client streams it, read back as the
//! worker hands it over, and spawned by the loader one item at a time (a
//! texture or a node: the least one slice of `step_section` does, so the
//! least a frame that spawns it does). Held to budgets:
//!
//! - **No pipeline first used after the warm-up.** The level's warm-up
//!   compiles what the base and the start's ring draw; a material ×
//!   mesh-layout × shadow combination any other section draws compiles
//!   when that section comes up, while the player drives, and on WebGL2
//!   that is a frame of 100 to 700 ms (HANDOFF's spikes). Budget: none.
//! - **Each item fits a frame**: its entities, its meshes' bytes, its
//!   image's bytes and its instance stream, each under what one slice of
//!   the running budget (`SPAWN_RUNNING_MS`, 5 ms) can make on the web. A
//!   texture is converted once a level (the stream's image cache, by its
//!   pixels): only those first met after the start's ring are held.
//! - **The worker's hand-over fits a frame**: a section is read back
//!   (`section::unpack`) whole in one frame on the main thread, so its
//!   packed bytes are held to a budget too.
//!
//! The figures are printed (`--no-capture`) so a failure says which
//! section, which item and which material.

use std::collections::{BTreeSet, HashMap, HashSet};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use super::{AHEAD, BEHIND, plan, run};
use crate::loader::{self, Drawn, Uploads};
use crate::render::{SharedImages, SkyMaterial, ThreeMaterial};

/// Entities one item may spawn (an instanced node's material groups, a
/// node's parts).
const ITEM_ENTITIES: usize = 16;
/// Bytes one item may make for the GPU: a node's meshes, or a texture's
/// pixels (with its mipmaps), or an instanced node's stream. Measured in
/// the release web build by the stutter suite (D1238): a frame spawning
/// 3.4 to 3.9 MB of meshes spent 4 to 7 ms; one converting 27 MB of
/// images 128 ms (WebGPU) and 185 ms (WebGL2), about 5 to 7 ms a MB. So
/// 2 MB of meshes and 1 MB of image are each about one 5 ms slice.
const ITEM_MESH_BYTES: u64 = 2 << 20;
const ITEM_IMAGE_BYTES: u64 = 1 << 20;
const ITEM_INSTANCE_BYTES: u64 = 2 << 20;
/// A section's packed bytes, read back whole in one frame on the main
/// thread: measured about 0.5 ms a MB (43 MB in 10 to 21 ms), so 16 MB is
/// about 8 ms.
const PACKED_BYTES: usize = 16 << 20;

/// One item's work.
#[derive(Clone, Debug, Default)]
struct Item {
    what: String,
    entities: usize,
    up: Uploads,
}

/// A section spawned item by item.
#[derive(Debug, Default)]
struct Report {
    key: Option<i64>,
    nodes: usize,
    packed: usize,
    items: usize,
    entities: usize,
    total: Uploads,
    /// The heaviest item by each measure.
    most_entities: Item,
    most_mesh: Item,
    most_instance: Item,
    /// Every texture item, by its pixels' hash (the image cache's key).
    textures: Vec<(u64, Item)>,
    drawn: Vec<Drawn>,
}

/// Spawns a section's scene through the loader, one item at a time, into
/// a world of its own.
fn spawn(key: Option<i64>, wb: mp_worldgen::world::WorldBuild, packed: usize) -> Report {
    let scene = wb.scene;
    let hashes: Vec<u64> = scene
        .textures
        .iter()
        .map(|t| loader::texture_hash(&scene, t))
        .collect();
    let mut world = World::new();
    let root = world.spawn_empty().id();
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<ThreeMaterial>::default();
    let mut sky = Assets::<SkyMaterial>::default();
    let mut images = Assets::<Image>::default();
    let shared = SharedImages {
        globals: Handle::default(),
        env: Handle::default(),
    };
    let mut r = Report {
        key,
        nodes: scene.nodes.len(),
        packed,
        ..Report::default()
    };
    let mut b = loader::Build::new(scene, shared).under(root);
    loop {
        let what = loader::next_item(&b);
        let (e0, u0) = (b.entities(), b.uploads());
        if loader::step_item(
            &mut b,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut sky,
            &mut images,
        ) {
            break;
        }
        let u1 = b.uploads();
        let it = Item {
            what,
            entities: b.entities() - e0,
            up: Uploads {
                mesh_bytes: u1.mesh_bytes - u0.mesh_bytes,
                image_bytes: u1.image_bytes - u0.image_bytes,
                instance_bytes: u1.instance_bytes - u0.instance_bytes,
            },
        };
        r.items += 1;
        if it.what.starts_with("texture ") {
            let h = hashes.get(r.textures.len()).copied().unwrap_or(0);
            r.textures.push((h, it.clone()));
        }
        if it.entities > r.most_entities.entities {
            r.most_entities = it.clone();
        }
        if it.up.mesh_bytes > r.most_mesh.up.mesh_bytes {
            r.most_mesh = it.clone();
        }
        if it.up.instance_bytes > r.most_instance.up.instance_bytes {
            r.most_instance = it;
        }
    }
    r.entities = b.entities();
    r.total = b.uploads();
    r.drawn = b.drawn().to_vec();
    r
}

/// Every section's report (the base's key is None), built on a few
/// threads: a section takes seconds in the debug profile.
fn reports() -> (std::sync::Arc<mp_worldgen::stream::Planned>, Vec<Report>) {
    let p = plan(mp_levels::level_by_id("peninsula")).expect("the loop plans");
    let mut keys: Vec<Option<i64>> = vec![None];
    keys.extend((0..p.sections()).map(Some));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().clamp(2, 8));
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&key) = keys.get(i) else { break };
                    let sec = key.map_or_else(|| p.base_section(), |k| p.section(k));
                    let (wb, stats) = run(p.build(sec), None).expect("a section builds");
                    // The worker's hand-over: what the client reads back.
                    let bytes = mp_worldgen::section::pack(&wb, stats).expect("packs");
                    let (wb, _) = mp_worldgen::section::unpack(&bytes).expect("reads back");
                    let r = spawn(key, wb, bytes.len());
                    out.lock().unwrap_or_else(|e| e.into_inner()).push(r);
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap_or_else(|e| e.into_inner());
    v.sort_by_key(|r| r.key.unwrap_or(-1));
    (p, v)
}

fn kb(b: u64) -> String {
    format!("{:.0} kB", b as f64 / 1024.0)
}

/// What a combination is, for a message: its material, the key's main
/// switches and the layout's attribute count.
fn describe(d: &Drawn) -> String {
    let k = &d.key;
    format!(
        "'{}' ({:?}{}{}{}{}{}, side {}, blending {}, {} attributes{})",
        d.material,
        k.model,
        if k.map { ", map" } else { "" },
        if k.instanced { ", instanced" } else { "" },
        if k.instance_color {
            ", instance colour"
        } else {
            ""
        },
        if k.alpha_test { ", alpha test" } else { "" },
        if k.opaque { "" } else { ", transparent" },
        k.side,
        k.blending,
        d.layout.1.len(),
        if d.casts { ", casts" } else { "" },
    )
}

/// The pipeline identity of a combination (as `pk_shell::warmup::Combos`
/// tells them apart).
type Pipe = (crate::render::material::ThreeKey, String, bool);

fn pipe(d: &Drawn) -> Pipe {
    (d.key, format!("{:?}", d.layout), d.casts)
}

#[test]
fn every_section_streams_within_the_frame_budgets() {
    let t0 = std::time::Instant::now();
    let (p, reports) = reports();
    eprintln!(
        "{} sections and the base built, read back and spawned item by item in {:.1} s",
        reports.len() - 1,
        t0.elapsed().as_secs_f64()
    );
    let mut failures: Vec<String> = Vec::new();

    // The figures, section by section.
    for r in &reports {
        let name = r.key.map_or_else(
            || "base".to_string(),
            |k| {
                let (road, i) = p.road_of(k);
                format!("{k} (road {road} #{i})")
            },
        );
        eprintln!(
            "  section {name}: {} nodes, {} items, {} entities, packed {}, meshes {}, images {}, instances {}; \
             most in one item: {} entities ({}), meshes {} ({}), instances {} ({}); {} pipelines",
            r.nodes,
            r.items,
            r.entities,
            kb(r.packed as u64),
            kb(r.total.mesh_bytes),
            kb(r.total.image_bytes),
            kb(r.total.instance_bytes),
            r.most_entities.entities,
            r.most_entities.what,
            kb(r.most_mesh.up.mesh_bytes),
            r.most_mesh.what,
            kb(r.most_instance.up.instance_bytes),
            r.most_instance.what,
            r.drawn.len(),
        );
        let over = |what: &str, it: &Item, v: u64, max: u64| {
            (v > max).then(|| {
                format!(
                    "section {name}: one item, {}, makes {what} {} (budget {})",
                    it.what,
                    if what == "entities" {
                        v.to_string()
                    } else {
                        kb(v)
                    },
                    if what == "entities" {
                        max.to_string()
                    } else {
                        kb(max)
                    }
                )
            })
        };
        // The base is spawned behind the loading screen, once.
        if r.key.is_none() {
            continue;
        }
        failures.extend(over(
            "entities",
            &r.most_entities,
            r.most_entities.entities as u64,
            ITEM_ENTITIES as u64,
        ));
        failures.extend(over(
            "mesh bytes",
            &r.most_mesh,
            r.most_mesh.up.mesh_bytes,
            ITEM_MESH_BYTES,
        ));
        failures.extend(over(
            "instance bytes",
            &r.most_instance,
            r.most_instance.up.instance_bytes,
            ITEM_INSTANCE_BYTES,
        ));
        if r.packed > PACKED_BYTES {
            failures.push(format!(
                "section {name}: packed {} for the worker's hand-over, read back in one frame (budget {})",
                kb(r.packed as u64),
                kb(PACKED_BYTES as u64)
            ));
        }
    }

    // The warm-up: the base and the start's ring.
    let start = p.section_at(p.track.start_s);
    let ring: BTreeSet<i64> = p.ring(start, BEHIND, AHEAD).into_iter().collect();
    let mut warm: HashSet<Pipe> = HashSet::new();
    for r in &reports {
        if r.key.is_none_or(|k| ring.contains(&k)) {
            warm.extend(r.drawn.iter().map(pipe));
        }
    }
    // The textures first met after the start (the image cache has the
    // start's, and each other once it is met).
    let mut met: HashSet<u64> = HashSet::new();
    for r in &reports {
        if r.key.is_none_or(|k| ring.contains(&k)) {
            met.extend(r.textures.iter().map(|(h, _)| *h));
        }
    }
    for r in &reports {
        let Some(k) = r.key else { continue };
        if ring.contains(&k) {
            continue;
        }
        for (h, it) in &r.textures {
            if !met.insert(*h) {
                continue;
            }
            eprintln!(
                "  section {k}: {}, first met here: {}",
                it.what,
                kb(it.up.image_bytes)
            );
            if it.up.image_bytes > ITEM_IMAGE_BYTES {
                failures.push(format!(
                    "section {k}: one item, {}, makes image bytes {} (budget {})",
                    it.what,
                    kb(it.up.image_bytes),
                    kb(ITEM_IMAGE_BYTES)
                ));
            }
        }
    }

    // Every other combination, with the sections that draw it.
    let mut late: HashMap<Pipe, (Drawn, Vec<i64>)> = HashMap::new();
    for r in &reports {
        let Some(k) = r.key else { continue };
        for d in &r.drawn {
            let id = pipe(d);
            if warm.contains(&id) {
                continue;
            }
            late.entry(id)
                .or_insert_with(|| (d.clone(), Vec::new()))
                .1
                .push(k);
        }
    }
    eprintln!(
        "warm-up: {} pipelines (the base and sections {ring:?}); {} more drawn by the other sections",
        warm.len(),
        late.len()
    );
    let mut late: Vec<(Drawn, Vec<i64>)> = late.into_values().collect();
    late.sort_by(|a, b| (&a.1, &a.0.material).cmp(&(&b.1, &b.0.material)));
    for (d, ks) in &late {
        let msg = format!(
            "a pipeline first used after the warm-up: {} in sections {ks:?}",
            describe(d)
        );
        eprintln!("  {msg}\n    {:?}, layout {:?}", d.key, d.layout);
        failures.push(msg);
    }
    assert!(
        failures.is_empty(),
        "{} over budget:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
