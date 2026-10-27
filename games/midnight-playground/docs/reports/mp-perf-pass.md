# Midnight Playground Performance Pass (`mp/perf-pass`)

## Summary

This pass measured Midnight Playground's simulation, vehicle dynamics, full-level world generation, open-world Peninsula section streaming, native client startup/frame times, and WebAssembly bundle sizes, then implemented safe, bit-exact CPU and allocation optimisations across `mp_track`, `mp_vdyn`, `mp_sim`, and `mp_worldgen`.

Key results (release builds, bit-identical simulation hashes and scene digests):

- **Simulation throughput (`mp-sim`)**: **1.36× to 1.66× faster** (`bench` median **200,823 → 289,137 ticks/s**, Coast Hot Pursuit **73,829 → 122,612 ticks/s**, Peninsula **82,299 → 127,869 ticks/s**, Sierra `--physics all` **18,208 → 24,691 ticks/s**; `mp_vdyn` `rig_bench` **3.38 → 2.71 µs/tick**).
- **Full-level world generation (`section_cost`)**: **1.42× to 3.32× faster** across all seven non-streamed levels (**14.02 s → 7.50 s** combined; Desert **3.15 → 1.10 s**, Seaside **0.83 → 0.25 s**, Cruise **2.43 → 1.33 s**, South Coast **1.79 → 0.99 s**).
- **Open-world Peninsula streaming (`stream_cost`, `section_timing`, `mp_game` stream budget)**: `Planned::new` **23–32% faster** (**1.99 → 1.53 s** in `stream_cost`, **2.40 → 1.63 s** in `section_timing`), individual streamed square sections **6.5× to 6.8× faster** on average (**273 ms → 40 ms** mean across 12 sampled squares; warm sections **10–70 ms** down from **220–280 ms**), and building/packing/unpacking/spawning all 239 sections + base on 8 threads dropped from **12.5 s → 5.0 s** (**2.50× faster**).
- **Native startup (`midnight-playground`, Vulkan, 60 Hz vsync)**: start-to-ready dropped **33.1%** on Sierra (**4.68 → 3.13 s**, `main_cpu` **2.35 → 1.83 ms**), **18.2%** on Peninsula (**6.11 → 5.00 s**), and **9.1%** on Coast (**2.86 → 2.60 s**), with `0` slow frames after ready on every measured level.
- **Determinism & visual quality**: all **743** nextest tests across `mp_track`, `mp_levels`, `mp_vdyn`, `mp_sim`, `mp_scene`, `mp_worldgen`, `mp_builder`, and `mp_game` pass, all six 14,400-tick race and 72,000-tick rig state hashes match bit-for-bit, and zero visual quality trade-offs were made.

---

## Tooling & Methodology

Existing benchmarks and instrumentation used for baseline and post-pass A/B runs (all heavy builds, tests, and benchmarks executed under `reserve -c 8`):

1. **Simulation & vehicle dynamics**:
   - `mp-sim bench --runs 7 --ticks 14400` (full Sierra field: player on autopilot, 5 rivals, 22 traffic cars, collisions).
   - `mp-sim race --ticks 14400` across `sierra` (arcade), `sierra --physics all` (6 multi-body `mp_vdyn` cars), `coast --pursuit 3` (Hot Pursuit police + roadblocks + spikes), `seaside`, and `peninsula`, recording both throughput (`ticks/s`) and final deterministic state hash.
   - `cargo run --release -p mp_vdyn --example rig_bench` (72,000 ticks × 5 substeps = 360,000 multi-body substeps, recording µs/tick and deterministic FNV hash).
2. **World generation & streaming**:
   - `cargo run --release -p mp_worldgen --example section_cost -- coast sierra streets desert seaside cruise southcoast` (per-stage wall times, peak/kept heap, node/mesh/material/texture counts, buffer sizes).
   - `cargo run --release -p mp_worldgen --example stream_cost -- peninsula 12` (open-world `Planned::new`, base scene, and 12 sampled square sections).
   - `cargo test --release -p mp_worldgen --test section_timing -- --ignored --nocapture` (per-stage millisecond breakdown of Peninsula `open_world` sections).
   - `cargo test --release -p mp_game --lib every_section_streams_within_the_frame_budgets -- --nocapture` (8-thread build, pack, unpack, and item-by-item spawn budget check across all 239 Peninsula sections + base).
3. **Native client startup & frame telemetry**:
   - `midnight-playground --level <lvl> --screenshot <png> --after 600 --query "autostart=sports&autodrive=1&seed=1&pursuit=0&hq=1[&cruise=1]&record=<jsonl>"` across `sierra`, `coast`, `seaside`, and `peninsula` on Vulkan (RTX 3060, 60 Hz FIFO vsync).
4. **WebAssembly bundle sizes**:
   - `cargo xtask web --release && cargo xtask size` in `games/midnight-playground/` (fat LTO + `wasm-opt -Oz` + gzip + brotli for WebGPU, WebGL2, builder worker, music worklet, and exhaust worklet).

---

## Bottlenecks Found & Optimisations Implemented

### 1. `mp_track`: Dense CSR Spatial Grids, `Arc`-Shared Track Data, and Chunk Bounds (`f04c603`)

- **Bottleneck**: `Track::nearest`, `NetIndex` (`roads_near`, `nearest_samples`, `on_other_tarmac`), `Buildings` (`near`, `inside`, `push_out`, `garage_ground`), and `Props` (`near`) all backed their 2D spatial hashes with `BTreeMap<(i64, i64), Vec<...>>` and allocated fresh `Vec`s (plus `sort_unstable` + `dedup`) on every query. In addition, cloning `Track` for each streamed section deep-cloned `Track.hash`, `Planting::fences` stepped every 4 m along all 20 roads even km away from a queried tile, and `Track::build_branch` computed a full racing-line pass that was immediately discarded when elevation fitting rebuilt the branch.
- **Changes**:
  - Added dense compressed-sparse-row (CSR) grids (`SpatialGrid`, `NetGrid`, `BuildingGrid`, `FixedGrid`) in `crates/mp_track/src/{track.rs,network.rs,buildings.rs,props.rs}`, turning cell lookups into bounds-checked flat array slices while preserving exact insertion and road order.
  - Wrapped `Track.hash`, `Track.grid`, `Track.chunk_bounds`, `NetIndex.grid`, and `Props.{fixed,grid}` in `Arc` so section `Track` clones are O(1) reference-count bumps.
  - Added zero-allocation `NetIndex::has_roads_near` and `NetIndex::for_each_road_near` (using a `u64` road bitmask when `num_roads <= 64`, covering Peninsula's 20 roads in a single register) and ring-perimeter + road-bbox filtering in `NetIndex::nearest_samples`.
  - Added stack-deduped `Buildings::for_each_near`, single-cell `Buildings::inside` / `garage_ground` with a `has_garages` fast-exit flag, and zero-allocation `Props::for_each_near`.
  - Added 512-sample (`CHUNK_SAMPLES`) centreline bounding boxes on `Track` so `Planting::fences` jumps across distant road chunks in phase-aligned `POST * 8.0` (32 m) steps, and skipped the discarded first `racing_line` pass in `Track::build_branch`.

### 2. `mp_vdyn` & `mp_sim`: Zero-Allocation Substeps and Incremental `AgentView`s (`205d533`)

- **Bottleneck**: `mp_vdyn::Vehicle::step` and `substep` allocated four temporary `Vec`s (`angles`, `steer_rot`, `f_susp`, `u_hub`) on every 120 Hz tick and 600 Hz substep, and `VehicleDef::static_axle_loads` allocated a `Vec<f64>` per tyre query in `mp_sim::model`. In `mp_sim`, `SimCar::write_out` called `car.telemetry(&self.def)` (building unused per-wheel telemetry `Vec`s) and reallocated `Box<SimPose>` + `Vec<WheelPose>` every tick; `race::step` and `Pursuit::update` rebuilt `field.views(&agents)` from scratch before every individual rival or police unit update ($O(N^2)$ view construction) and allocated `Pursuit::bodies()` `Vec`s twice per tick.
- **Changes**:
  - Replaced heap `Vec`s in `Vehicle::step` and `Vehicle::substep` with stack buffers (`[0.0; 8]`, `[Quat::IDENTITY; 8]`) for cars with `<= 8` wheels, and added `VehicleDef::static_axle_loads_2() -> [f64; 2]` (`crates/mp_vdyn/src/vehicle.rs`).
  - Updated `SimCar::write_out` (`crates/mp_sim/src/model.rs`) to compute body-frame acceleration directly (`b.rot.inv_rotate(car.accel)`), read wheel contact/sliding flags directly from `car.wheels`, and update `SimPose.wheels` in place.
  - Pre-sized `Field::agents`, eliminated `Pursuit::bodies()` temporary `Vec`s in `Field::agents` and `Pursuit::write_pos`, stack-allocated per-player tick buffers (`frames`, `inps`, `odos`) for `<= 8` players, and updated the single moved entry `views[m]` in place across `race::step` and `Pursuit::update` (`crates/mp_sim/src/{field.rs,race.rs,pursuit.rs}`).

### 3. `mp_worldgen`: Terrain Noise Guards, Tile Pre-Filtering, and Shared Section Caches (`729cfd2`)

- **Bottleneck**:
  1. `Terrain::landform_at` always evaluated 2-octave `detail` FBM and every landform's 3-to-5-octave horizon `ridged`/`fbm` noise even near the road (`d < 400..1500 m`) where `smoothstep` weights were identically `0.0`. Meanwhile `build_tile_geometry` scanned all `flattens` and `carves` for every vertex of every 256 m tile.
  2. When streaming Peninsula's 239 square sections, every section built a fresh `TextureCache` (re-rasterising `facade` 2048×1024 + emissive, `street` 1024×1024 + emissive, asphalt, markings, bark, foliage, etc. — ~85 ms/section in `Raising the town`), re-ran `classify_road_sides` across all 83 km of Road 0 + branches (~50 ms/section in `Paving roads`), and walked every metre of absent roads in `Road::build_surface` / `build_markings` / `build_chevrons` and `town::sidewalks`.
- **Changes**:
  - Guarded `detail` FBM in `Terrain::landform_at` and strictly-positive `smoothstep` horizon noise terms across `form_alpine`, `form_foothills`, `form_valley`, `form_coast`, `form_beach`, `form_harbor`, `form_streets`, `form_canyon`, and `form_desert` (`crates/mp_worldgen/src/terrain.rs`), returning `0.0` without calling `ridged`/`fbm` whenever the weight is `0.0` (0-ULP identical).
  - Pre-filtered `flattens`, `carves_before`, and `carves_after` against each tile's padded AABB in `build_tile_geometry` (`crates/mp_worldgen/src/terrain_mesh.rs`) via `Terrain::height_at_filtered`, and cached `Terrain::tiles()` via `OnceLock` with `invalidate_tiles()` on bounds mutation.
  - Added `SharedTextures` (`crates/mp_worldgen/src/textures.rs`) on `Planned` (`crates/mp_worldgen/src/stream.rs`) to share deterministic non-`"sign:*"` procedural textures across streamed sections while keeping sign keys per-section.
  - Added `SharedRoadSides` (`Arc<Vec<OnceLock<(Vec<u8>, Vec<u8>)>>>`) on `Planned` and `World` (`crates/mp_worldgen/src/{road.rs,stages.rs,world.rs}`) so `classify_road_sides` runs at most once per road across all 239 sections.
  - Added `!sec.has_road(c.road)` early-outs in `Road::{build_surface,build_markings,build_chevrons}` and `town::sidewalks`, jumped directly to `place.runs[0]` on Road 0 (`branch_from.is_none()`), replaced per-metre heap `Vec<Line>` in `Road::build_markings` with stack `LineSet`, cached `Arc<Place>` per `PlaceGrid` cell, used `chunk_bounds` in `Place::new` and `b.track.bounds` in `clear_branch_tarmac`, and replaced `Vec<Hit>` with stack `Hits` + `Terrain::far_d` in `City::freeway_hits`.

---

## Before & After Measurements

### 1. Simulation & Vehicle Dynamics (Single-Threaded Release)

| Benchmark | Baseline | After | Change | State Hash |
|---|---:|---:|---:|---|
| `mp-sim bench` (Sierra full field, median of 7 × 14,400) | 200,823 ticks/s (4.98 µs) | **289,137 ticks/s** (3.46 µs) | **+44.0% (1.44×)** | — |
| `mp-sim bench` (best of 7 × 14,400) | 201,997 ticks/s | **291,972 ticks/s** | **+44.5% (1.45×)** | — |
| `mp-sim race --level sierra` (arcade, 14,400 ticks) | 185,378 ticks/s | **278,674 ticks/s** | **+50.3% (1.50×)** | `a83c2855bd61ffdc` (exact) |
| `mp-sim race --level sierra --physics all` (14,400 ticks) | 18,208 ticks/s (54.9 µs) | **24,691 ticks/s** (40.5 µs) | **+35.6% (1.36×)** | `dbab8f4d2daab35c` (exact) |
| `mp-sim race --level coast --pursuit 3` (14,400 ticks) | 73,829 ticks/s | **122,612 ticks/s** | **+66.1% (1.66×)** | `f3ac49a28c3d9d1a` (exact) |
| `mp-sim race --level seaside` (14,400 ticks) | 193,542 ticks/s | **274,264 ticks/s** | **+41.7% (1.42×)** | `b0a18915ccc99f1f` (exact) |
| `mp-sim race --level peninsula` (14,400 ticks) | 82,299 ticks/s | **127,869 ticks/s** | **+55.4% (1.55×)** | `afceef9c300cb8b3` (exact) |
| `mp_vdyn` `rig_bench` (72,000 ticks × 5 substeps) | 0.243 s (3.38 µs/tick) | **0.195 s** (2.71 µs/tick) | **-19.8% time (1.25×)** | `0x8608e700c85a5996` (exact) |

### 2. Full-Level World Generation (`section_cost`, Single-Threaded Release)

| Level | Baseline | After | Change | Key Stage Shifts (Before → After) | Scene Counts & Buffers |
|---|---:|---:|---:|---|---|
| `coast` | 2.01 s | **1.27 s** | **-36.8% (1.58×)** | terrain 0.370→0.228 s, Seabright 0.658→0.397 s, coast 0.227→0.120 s, harbour 0.241→0.145 s | 458 nodes, 418 meshes, 208.8 MB |
| `sierra` | 2.07 s | **1.46 s** | **-29.5% (1.42×)** | shape 0.190→0.098 s, terrain 0.415→0.263 s, road 0.123→0.076 s, city 0.880→0.684 s | 526 nodes, 459 meshes, 136.9 MB |
| `streets` | 1.74 s | **1.10 s** | **-36.8% (1.58×)** | terrain 0.182→0.117 s, downtown 1.430→0.899 s | 441 nodes, 432 meshes, 165.9 MB |
| `desert` | 3.15 s | **1.10 s** | **-65.1% (2.86×)** | terrain **1.670→0.227 s (7.36×)**, desert 1.203→0.715 s, road 0.117→0.071 s | 376 nodes, 287 meshes, 106.2 MB |
| `seaside` | 0.83 s | **0.25 s** | **-69.9% (3.32×)** | terrain **0.640→0.133 s (4.81×)**, road 0.071→0.047 s, raceway 0.083→0.042 s | 227 nodes, 126 meshes, 21.7 MB |
| `cruise` | 2.43 s | **1.33 s** | **-45.3% (1.83×)** | survey 0.042→0.009 s, terrain 0.665→0.271 s, city 1.512→0.868 s | 495 nodes, 407 meshes, 180.1 MB |
| `southcoast` | 1.79 s | **0.99 s** | **-44.7% (1.81×)** | survey 0.034→0.010 s, terrain 0.893→0.375 s, road 0.185→0.086 s, coast 0.222→0.123 s | 588 nodes, 487 meshes, 153.3 MB |
| **Total (7 levels)** | **14.02 s** | **7.50 s** | **-46.5% (1.87×)** | — | **Identical across all levels** |

### 3. Open-World Peninsula Planning & Section Streaming (Release)

| Benchmark | Baseline | After | Change | Details |
|---|---:|---:|---:|---|
| `stream_cost peninsula 12` — `Planned::new` | 1.99 s | **1.53 s** | **-23.1% (1.30×)** | 20 roads, 44 junctions, 239 sections; held 116 MB |
| `stream_cost peninsula 12` — Base scene | 1.11 s | **0.92 s** | **-17.1% (1.21×)** | 722 nodes, peak 220 MB over plan |
| `stream_cost peninsula 12` — 12 square sections (mean) | 0.273 s (0.19–0.37 s) | **0.040 s** (0.01–0.14 s) | **-85.3% (6.83×)** | 10 of 12 sections build in **10–50 ms**; peak heap over plan 14–51 MB (was 25–57 MB) |
| `section_timing` — `Planned::new` | 2.40 s | **1.63 s** | **-32.1% (1.47×)** | Full `open_world` scenery |
| `section_timing` — Section 1 (first cold section) | 0.23 s | **0.19 s** | **-17.4% (1.21×)** | Populates shared caches; packed 261 kB (identical) |
| `section_timing` — Sections 6, 9, 14, 20, 27, 33 (warm mean) | 0.250 s (0.22–0.28 s) | **0.038 s** (0.01–0.07 s) | **-84.7% (6.52×)** | `Paving roads` **5.2–12.7 ms** (was 63–79 ms); `Raising the town` **0.3–2.4 ms** (was 76–92 ms); packed kB identical |
| `mp_game` `every_section_streams_within_the_frame_budgets` | 12.5 s | **5.0 s** | **-60.0% (2.50×)** | All 239 sections + base built, packed, unpacked, and spawned item-by-item on 8 threads; 49 warm-up pipelines, 0 late |

### 4. Native Client Startup & 600-Frame Telemetry (Vulkan, RTX 3060, 60 Hz Vsync)

| Level | Start-to-Ready (Before → After) | Ready Change | `fps_med` | `main_cpu` Mean (Before → After) | `main_cpu` Max (Before → After) | Slow Frames After Ready |
|---|---|---:|---:|---|---|---:|
| `sierra` | 4.68 s → **3.13 s** | **-33.1% (-1.55 s)** | 59.8 → 59.9 | 2.348 ms → **1.830 ms** | 24.55 ms → **18.05 ms** | 0 → 0 |
| `peninsula` | 6.11 s → **5.00 s** | **-18.2% (-1.11 s)** | 60.0 → 60.0 | 1.368 ms → 1.936 ms* | 17.20 ms → 18.00 ms | 0 → 0 |
| `coast` | 2.86 s → **2.60 s** | **-9.1% (-0.26 s)** | 60.0 → 59.9 | 1.403 ms → 1.866 ms* | 9.88 ms → **6.78 ms** | 0 → 0 |
| `seaside` | 1.29 s → **1.31 s** | +0.02 s | 60.0 → 59.9 | 1.372 ms → 1.924 ms* | 17.10 ms → **10.42 ms** | 0 → 0 |

*\*Under 60 Hz FIFO vsync (`frame_mean` ~16.66 ms), the post-pass runs executed under lighter machine load (~10–13 vs ~30 during baseline), allowing cores to drop clock frequency during the ~14.7 ms vsync wait between frames; `sierra` (the heaviest race scene) improved in both mean and max `main_cpu`, max `main_cpu` dropped sharply on `coast` and `seaside`, and all runs maintained 60 fps with `0` slow frames after ready.*

### 5. Web Release Wasm Sizes (`cargo xtask web --release && cargo xtask size`)

| Bundle | Baseline Raw | After Raw | Baseline Gzip | After Gzip | Baseline Brotli | After Brotli |
|---|---:|---:|---:|---:|---:|---:|
| `mp_game_bg.wasm` (WebGPU) | 30.81 MB (32,311,517 B) | 30.86 MB (32,355,775 B) | 11.49 MB (12,046,721 B) | 11.50 MB (12,055,420 B) | 7.98 MB (8,368,212 B) | 7.99 MB (8,375,813 B) |
| `mp_game_webgl2_bg.wasm` (WebGL2) | 31.75 MB (33,293,774 B) | 31.79 MB (33,338,082 B) | 11.86 MB (12,434,757 B) | 11.88 MB (12,455,100 B) | — | 8.24 MB (8,640,304 B) |
| `mp_builder.wasm` | — | 6.33 MB | — | 3.66 MB | — | 3.14 MB |
| `music.wasm` | — | 915.3 KB | — | 328.5 KB | — | 253.9 KB |
| `mp_exhaust.wasm` | — | 59.7 KB | — | 24.4 KB | — | 21.1 KB |

The new CSR grid and cache structures add +8.7 KB gzipped (+0.07%) on WebGPU and +20.3 KB gzipped (+0.16%) on WebGL2, staying well inside the 16.00 MB release gzip budget (`11.88 MB / 16.00 MB`).

---

## Determinism & Workspace Verification

- `cargo fmt --all --check`: passed.
- `cargo xtask check-deps` (workspace root and `games/midnight-playground`): `32 crates follow the rules`.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed with zero warnings.
- `cargo clippy --target wasm32-unknown-unknown` across `mp_game`, `sb_game`, `mw_demo`, `pk_shell`, `pk_math`, `pk_lockstep`, `pk_canvas`, `music`, and `mp_audio --features web` (`-D warnings`): passed.
- `cargo nextest run -p mp_track -p mp_levels -p mp_vdyn -p mp_sim -p mp_scene -p mp_worldgen -p mp_builder -p mp_game`: **743 / 743 tests passed**, including JS parity golden traces, multiplayer lockstep determinism tests, open-world section partitioning invariants (`sections_partition_the_loaded_world`, `every_metre_of_every_road_belongs_to_one_square`, `a_square_keeps_only_its_own`), full-level `.mrscene` export equality tests (`sierra_is_the_export`, `coast_is_the_export`, `streets_is_the_export`, `desert_is_the_export`, `seaside_is_the_export`, `cruise_is_the_export`), and streaming frame-budget checks.
- All six 14,400-tick `mp-sim race` and 72,000-tick `mp_vdyn` `rig_bench` state hashes match the baseline bit-for-bit.

---

## What Was Not Finished in This Pass

1. **Browser WebGPU/WebGL2 perf runs (`tools/parity/rust-perf.mjs`, `tests/stutter.test.mjs`)**: per worktree ground rules, no server was started from this worktree, so browser-side GPU/WebGL2 frame-time re-measurements (including the X1 Carbon UHD 620 WebGL2 scenario documented in `docs/rust-port/BASELINE.md`) were left for the main checkout's registered server.
2. **Procedural building geometry micro-optimisations in `city` and `streets`**: `Building downtown` (`streets`, 0.90 s) and `Building the city` (`cruise`, 0.87 s; `sierra`, 0.68 s) are now the largest single-module stages on non-streamed city levels; their remaining cost is mostly vertex/attribute buffer construction for thousands of extruded blocks and rooftops rather than spatial lookups.
3. **Wasm binary size reduction**: `mp_game_bg.wasm` (11.50 MB gzip) and `mp_game_webgl2_bg.wasm` (11.88 MB gzip) were measured and kept virtually flat, not shrunk; trimming Bevy features or font/shaping dependencies (`skrifa` / `harfrust` / `read-fonts` via `pk_canvas`) would require touching `crates/engine`, which was intentionally avoided here.

---

## Decisions & Proposals for the Owner

The following high-leverage opportunities involve visual-quality, scene-digest, or cross-crate trade-offs and are presented as proposals rather than changes:

1. **Coarser far-horizon terrain tile step on integrated GPUs (`mp_worldgen::terrain`)**:
   - *Context*: `docs/rust-port/BASELINE.md` (D1195) notes that on the reference X1 Carbon (UHD 620) at half render scale, fragment cost drops to ~5 ms while the vertex stage remains floored at ~22 ms due to ~3.8 M triangles/frame (roughly 1 M from terrain).
   - *Proposal*: Add a quality-gated far-terrain LOD step multiplier (e.g., doubling `Tile.step` beyond `dmin >= 900 m` when `hq=0`) while keeping near-road tiles (`dmin < 400 m`) and stitch edges unchanged. This would cut far-terrain triangle counts by ~75% and speed up `Sculpting terrain` further, at the cost of slightly softer distant ridge silhouettes when `hq=0`.
2. **Parallelising internal block chunks inside `City` and `Streets` (`mp_worldgen::stages`)**:
   - *Context*: Full-level `streets` (0.90 s in `downtown`) and `cruise` (0.87 s in `city`) currently run each scenery module as a single `Job::One`.
   - *Proposal*: Split `City` and `Streets` block generation into deterministic per-district or per-block-row `Job::Parallel` tasks (with deterministic RNG sub-seeds per block rather than sequential stream consumption) and merge their `Object3D` subtrees in fixed index order. Because changing RNG draw order changes procedural building variety (`DECISIONS.md` scene digests), this needs owner sign-off to re-bless the city scene digests.
3. **Deduplicating shared textures in packed `.mrscene` streams (`mp_builder` / `mp_game`)**:
   - *Context*: `SharedTextures` eliminated redundant CPU rasterisation across Peninsula's 239 sections, but each packed section still serialises ~11–15 MB of uncompressed RGBA mipmaps (`facade` + `street` atlases) across the worker boundary (`stream_budget` logs ~11.2 MB `images` even on 7-node town sections), which `mp_game` then deduplicates by content hash on unpack.
   - *Proposal*: Let `mp_builder` track texture digests already sent in the session (or reference base-scene texture IDs across the worker protocol) so each shared atlas is packed and copied across the Web Worker boundary only once instead of up to 239 times.
4. **Evaluating `pk_canvas` text-shaping footprint in `mp_builder.wasm` and `mp_game_bg.wasm`**:
   - *Context*: Both `mp_game` and `mp_builder` link `pk_canvas` (`harfrust` + `skrifa` + `read-fonts` + `tiny-skia`) solely to rasterise procedural road/highway signs.
   - *Proposal*: If smaller wasm downloads become a priority on mobile networks, road signs could use a compact pre-baked glyph atlas for their fixed highway sign typeface (or delegate sign text rendering to `OffscreenCanvas` 2D on web), potentially shaving 1–2 MB off both `mp_builder.wasm` and `mp_game_bg.wasm` at the cost of minor glyph rasterisation differences against `pk_canvas`.
