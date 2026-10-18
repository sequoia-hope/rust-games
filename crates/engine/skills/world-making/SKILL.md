---
name: world-making
description: Build a game's physical world from real geography with the Playground Kit atlas pipeline: choose a bbox and origin, download terrain and map data, write plan.json (places, regions, routes, events), measure it, cut road corridors, lay real ground and land cover under them, plant scenery by land cover with nothing on the tarmac, stream it, and put events at places. Use when asked to add a region, a road, a corridor, an open-world level, real elevation, or to fix scenery or roads from the atlas.
---

# World making: an open world from the real map

Today the pipeline is Midnight Playground's (`~/Software/midnight-playground`;
paths below are relative to it). It becomes `pk_atlas` with
`tools/atlas/build.py --bbox --origin` in kit phase 5 (PLAN.md); today
`--bbox` and `--origin` are **constants at the top of `build.py`**
(`LON0, LON1, LAT0, LAT1`, `ORIGIN`), so a new world means editing them and
a new `assets/atlas/<world>/` directory.

## Read first

- `docs/vision/atlas.md`: the owner's direction, the data, the local frame,
  the plan file (2.3), how a route is measured (2.4), the planner (2.5).
- `docs/vision/peninsula.md`: the five stages, each playable, each with a
  gate the owner drives: corridor, ground, network, loop and streaming,
  events at places. Its section 3 is the decisions.
- `tools/atlas/build.py` and `tools/atlas/ground.py` headers (the file
  formats `MPGROUND` and `MPCOVER1` are in `ground.py`'s docstring).
- `crates/mp_atlas/src/bin/` and `crates/mp_atlas/src/{plan,graph,profile,corridor,report}.rs`.
- DECISIONS D1157 (the atlas), D1207 to D1220 (the stages, and what went
  wrong on the way: the junction freeze D1210, the camera on a branch
  D1211, the offset sections D1217, scenery on the road D1218).
- `docs/rust-port/SPEC.md` 8.6 and `docs/vision/WORLD.md` 10.1: the level
  viewer is the shared table where the owner and Claude annotate a draft.

## Sources and frame

- **Terrain:** Mapzen/Tilezen Terrarium tiles on AWS Open Data (zoom 13,
  about 15 m); public domain. The planning grid is 50 m, committed
  (`terrain.bin`); the 15 m tiles stay in `tools/atlas/cache/`.
- **Land cover, roads, water, parks, towns:** Overture Maps on AWS
  (ESA WorldCover CC BY 4.0; OpenStreetMap ODbL). `geo.json` carries the
  attribution. Roads are tertiary and up, split at junctions into a graph
  with class, name, route numbers, bridge and tunnel flags.
- **Frame:** x east, z south, y up, metres from the origin (a real
  junction the owner names), equirectangular about the origin's latitude.
  `mp_atlas::Projection` is the same formula; the crate is pure and uses
  square roots only, so world generation may read it one day.
- **Scale:** `plan.json`'s `scale` is game metres per real metre
  (0.5 on the peninsula), applied horizontally and vertically alike so
  grades stay true. A per-route `scale` overrides it.

## Procedure

1. **The owner's direction in plain words** (atlas.md 1): which real roads,
   where the first race starts and ends, what carries the colour. Write it
   into `docs/vision/<world>.md` before any data.
2. **Box and origin** in `build.py`, then (python3 with numpy, Pillow,
   shapely, pyarrow; network on the first run):

   ```bash
   python3 tools/atlas/build.py            # cached where it can
   python3 tools/atlas/build.py --refresh  # download again
   ```

   Writes `assets/atlas/<world>/terrain.bin` and `geo.json`. Never
   touches `plan.json`.
3. **`plan.json` by hand**, coordinates `[lat, lon]` as Google Maps copies
   them: `places` (junction, start, town, landmark, viewpoint, trailhead,
   offshore, garage), `regions` (stage first/planned/later, `character`,
   `real`, `kits`, `ring`), `routes` (by route `roads`, road `names` or
   `classes`, `from`/`via`/`to` places), `circuits`, `events` (race,
   time trial, delivery), `remix`. Ids unique; every id named must exist
   or the loader refuses the plan.
4. **Measure** and read the report before drawing anything:

   ```bash
   cargo run --release -p mp_atlas -- report
   cargo run --release -p mp_atlas -- resolve        # rewrites resolved.json (a test fails when stale)
   cargo run --release -p mp_atlas -- profile hwy-92 # CSV: d_m, lat, lon, h_m, cover
   ```

   Places snap to the nearest road the route may use (not the nearest
   junction); a place over 150 m off is flagged. Tunnels and bridges keep
   the road's level. Crests are 6 m drops both sides.
5. **Look at it with the owner** on the planner through the registered
   server (`proj up midnight-playground`, then `/tools/atlas.html`): the
   survey map, regions by stage, routes, profiles with crests. **Edit**
   drags places and region corners; **Copy plan.json** brings edits back
   to the file. Then `resolve` again.
6. **Corridor** (stage 1): `cargo run -p mp_atlas -- corridor <route-or-event>`
   writes `assets/atlas/<world>/corridors/<id>.txt`, one line a sample
   `x z y cover` in game metres every 5 m, smoothed ±60 real metres
   (and the line ±30 m, D1209: unsmoothed map nodes wobble the heading).
   Text, committed, diffable; a staleness test guards it. The level
   includes it at compile time and parses with `split_whitespace`, so the
   simulation crates gain no dependency.
7. **Ground and cover** (stages 2 and 4a):

   ```bash
   python3 tools/atlas/ground.py coast-south hwy-92 skyline-south hwy-84 --margin 1200 --step 16 --name peninsula
   ```

   Resamples the 15 m tiles onto 16 m game cells over all the corridors'
   box plus the margin, as `<name>.ground.bin` (zlib i16 decimetres) and
   `<name>.cover.bin` (one class byte per cell from the planning grid).
   Both are included in the wasm today (D1208: a packaging choice for the
   owner). An open level (`Level::open`) blends the road into the land
   (`Track::ground_y`: the road to the drawn shoulder, a metre's step,
   then the near-field blend). A test holds the drawn and simulated
   ground within 0.75 m beside the road.
8. **Network** (stage 3): a second corridor becomes a `Branch` (one zone,
   one road type); the Track trims it to the parent's pavement, fits its
   heights over 120 m, opens the parent's fences at its mouth. Roads are
   indexed: 0 the level's road, k its (k-1)th branch. Test every junction
   with the assist on (`mp_sim/tests/junction_race.rs`, `junction_fuzz.rs`)
   and drive it headless:

   ```bash
   node tools/parity/drive-branch.mjs --level peninsula --phone --secs 900
   node tools/parity/drive-branch.mjs --query "touch=1&assist=2&physics=player"
   ```

9. **Loop and streaming** (stage 4): chain the corridors (`chain`, join
   under 2.5 m), spline closed, `Route::Loop`. Plan once, build in
   sections that partition the level by each drawable's centre
   (D1213), a ring round the camera's focus (D1214), the builder in a
   worker (D1215), sections per road across junctions (D1216).
10. **Events at places** (stage 5, D1219): `Level::events` with
    `EventSpot`s from the plan's events; a cruise holds gas and brake on a
    marker for a second to start one; the minimap shows starts within 2 km.
11. **Gate**: the owner drives it and recognises the road. Then DECISIONS.

## Planting by land cover (D1212, D1218)

- Kits plant by the cover class under each cell (farm, grass, scrub,
  forest, town, sand, marsh), fields on crop, fences at field edges.
- **Nothing on the tarmac.** Within 60 m of any road, measure clearance
  against every road of the network (`road_clearance`), not the far
  field's 32 m raster (its blend misreads 3 m as 16 m). Rails, posts and
  chevrons stop where a branch runs onto its parent. `tests/stream_clear.rs`
  builds the sections round the start and every junction and fails on any
  instance whose foot stands on a road. Run it after any kit change.
- A ported kit that knows only road 0 is scaled to nothing where it
  stands on a branch's paved width (`clear_branch_tarmac`).

## Common mistakes

- Snapping places to junctions: coastal roads run kilometres between them.
- Rounding corridor samples to a decimetre: the heading wobbles half a
  degree per sample and the owner sees short segments. Write centimetres.
- Compressing the horizontal and not the vertical: the crest stops being
  a crest. Or the reverse: 16 % grades become 49 % through a tunnel.
- Letting a helper that takes "the road under the car" ask a branch for
  its branches (the D1210 freeze). Keep the level's track and let the
  helpers pick the road.
- A section scene with one root laid out by the models-scene grid (D1217):
  a scene spawned under a parent keeps its own places.
- Serving the planner with an ad-hoc server. `proj up`, the registered
  URL, and the port only from `proj port`.
- Changing the existing levels. The owner said: build the planning system
  first; the levels stay until the remix is decided.
