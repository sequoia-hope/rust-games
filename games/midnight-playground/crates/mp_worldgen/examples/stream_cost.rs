//! Times a streamed level (docs/vision/peninsula.md, stages 4b and 6d):
//! the plan once, then sections built over it, each sized and timed, as
//! the client's worker builds them: `cargo run --release -p mp_worldgen
//! --example stream_cost -- peninsula [N]` from the repo root builds the
//! base and N sections (default 6) spread over the level; `AT=x,z;x,z`
//! builds the sections under those points instead.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use mp_levels::level_by_id;
use mp_worldgen::stages::LevelSetup;
use mp_worldgen::stream::Planned;
use mp_worldgen::terrain_mesh::TerrainSetup;

/// Heap in use and its peak.
struct Counting;
static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let n = NOW.fetch_add(l.size(), Ordering::Relaxed) + l.size();
        PEAK.fetch_max(n, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        NOW.fetch_sub(l.size(), Ordering::Relaxed);
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn mb(b: usize) -> f64 {
    b as f64 / 1048576.0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id = args.first().map_or("peninsula", String::as_str);
    let n: i64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(6);
    let setup = LevelSetup {
        terrain: TerrainSetup {
            plan: None,
            ..TerrainSetup::default()
        },
        road: None,
    };
    let base = NOW.load(Ordering::Relaxed);
    let t0 = Instant::now();
    let p = Planned::new(
        level_by_id(id),
        None,
        setup,
        Arc::new(mp_worldgen::scenery::ported),
        Arc::new(|_| Vec::new()),
    )
    .expect("the level plans");
    println!(
        "{id}: planned in {:.2} s, {} roads, {} junctions, {} sections; held {:.0} MB (peak {:.0} MB)",
        t0.elapsed().as_secs_f64(),
        p.track.roads().count(),
        p.track.junctions.len(),
        p.sections(),
        mb(NOW.load(Ordering::Relaxed) - base),
        mb(PEAK.load(Ordering::Relaxed) - base),
    );
    let total = p.sections();
    let mut secs = vec![p.base_section()];
    match std::env::var("AT") {
        Ok(at) => {
            for pt in at.split(';') {
                let v: Vec<f64> = pt
                    .split(',')
                    .filter_map(|c| c.trim().parse().ok())
                    .collect();
                if let [x, z] = v[..]
                    && let Some(k) = p.section_under(x, z)
                {
                    secs.push(p.section(k));
                }
            }
        }
        Err(_) => {
            for k in 0..n.min(total) {
                secs.push(p.section(k * total / n.min(total).max(1)));
            }
        }
    }
    for sec in secs {
        let before = NOW.load(Ordering::Relaxed);
        PEAK.store(before, Ordering::Relaxed);
        let t = Instant::now();
        let mut b = p.build(sec.clone());
        while !b.is_done() {
            b.step().expect("a section's job");
        }
        let built = t.elapsed().as_secs_f64();
        let (wb, cut) = mp_worldgen::section::finish(b);
        let scene = wb.scene.nodes.len();
        let at = match &sec.place {
            Some(pl) => format!("square ({:.0}, {:.0})", pl.x0, pl.z0),
            None => format!("road {} {:.0}..{:.0}", sec.road, sec.s0, sec.s1),
        };
        println!(
            "  {:>9}: {at:<24} build {:.2} s, peak {:.0} MB over the plan, {} nodes, kept {} cut {}",
            if sec.base { "base" } else { "section" },
            built,
            mb(PEAK.load(Ordering::Relaxed) - before),
            scene,
            cut.kept,
            cut.cut
        );
    }
}
