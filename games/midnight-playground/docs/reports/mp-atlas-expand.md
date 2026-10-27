# Expanding the Midnight Playground Peninsula Atlas (`mp/atlas-expand`)

Date: 2026-10-27
Branch: `mp/atlas-expand`
Decision record: `docs/rust-port/DECISIONS.md` (`D1253`)
Updated vision docs: `docs/vision/atlas.md`, `docs/vision/peninsula.md`

## 1. What Was Built

Picked up the next owner-visible steps called for in `docs/vision/atlas.md`
(Section 5 "Next") and `docs/vision/peninsula.md` (Stage 6 "Not yet"):
expanding the Peninsula open-world network from 20 to 26 playable corridors
with six closed loops across coast, redwoods, reservoir and city; enabling
in-world `Race`, `TimeTrial` and `Delivery` events on any corridor of the
network (not only road 0); adding God Mode atlas coordinates and branch
corridors (`atlas.md` 5.3); and distinguishing built vs planned routes in
`mp_atlas` and `tools/atlas.html`.

### 1.1 Peninsula Plan, Corridors, and Real-World Grids (`assets/atlas/peninsula/`, `mp_atlas`, `tools/atlas.html`)

- **Six new drivable corridors + closed gaps (`plan.json`, `corridors/*.txt`):**
  - `pescadero-creek` (20.5 km real / 10.3 km game): continues Stage Road
    from Pescadero east through Memorial Park to Highway 84 south of La Honda,
    closing the southern spur (`south-redwoods-loop`).
  - `coast-north` (30.0 km real / 15.0 km game): Highway 1 north from Half
    Moon Bay past Pillar Point and Montara, through the Tom Lantos Tunnels at
    Devil's Slide, down through Pacifica (Linda Mar) and up to I-280 in Daly
    City (`north-coast-loop`).
  - `skyline-north` (9.5 km real / 4.8 km game) and `sharp-park-road` (3.3 km
    real / 1.6 km game): northern Highway 35 from I-280 at San Bruno along
    Sweeney Ridge to Highway 1 in Daly City, with Sharp Park Road's headland
    switchbacks dropping into Pacifica.
  - `canada-road` (11.9 km real / 6.0 km game): Woodside north along Upper
    Crystal Springs Reservoir to Highway 92 (`crystal-springs-loop`).
  - `sf-great-highway` (14.1 km real / 7.0 km game), extended `sf-market`
    (13.8 km real / 6.9 km game) and extended `sf-geary` (9.5 km real / 4.8 km
    game): closes the 175 m Stage 6 gap between Market Street and The
    Embarcadero at the Ferry Building, carries Market/Portola/Sloat over Twin
    Peaks to Ocean Beach, runs north beside the surf on the Great Highway to
    the Cliff House at Lands End, and returns east along Geary to Market
    (`sf-city-ocean-loop`).
- **Planner & report (`mp_atlas`, `tools/atlas.html`):**
  - Added `Route::built` (`#[serde(default)]`) to `plan.json` and
    `resolved.json`; `mp-atlas report` prints `built` vs `route` and the
    built-route summary count, while `tools/atlas.html` draws built corridors
    in solid highway red and unbuilt routes in dashed blue.
  - Added directed `-<route-id>` circuit legs in `mp_atlas::report` so circuits
    traverse two-way corridors in either direction without duplicating routes,
    and matched junction handoff tolerance to `160 m` real (`80 m` game,
    `mp_track::END_REACH`). All six circuits verify as `closes`.
- **Regenerated ground, cover and city grids:**
  - `peninsula.ground.bin` (`671 × 1044` @ 16 m, 887 kB) & `.cover.bin`
    (19 kB) extended south to cover Pescadero and Pescadero Creek Road.
  - `bayside.ground.bin` (`690 × 1104` @ 24 m, 783 kB) & `.cover.bin` (32 kB)
    extended west to the Pacific to cover Montara, Devil's Slide, Pacifica,
    Sweeney Ridge and Ocean Beach.
  - `bayside.city.bin` (`518 × 829` @ 32 m, 225 kB) rebuilt from 512,611
    Overture building footprints inside the expanded box.

### 1.2 Playable Network, Zone-Gated City Placement, and Overpasses (`mp_track`, `mp_levels`)

- **`mp_levels::peninsula`:** wired all 26 roads (`loop` + 3 southern redwood
  branches + 22 `BAYSIDE` corridors), 7 zones (adding `"midcoast"`), 8 cruise
  starts (adding `pacifica` at Linda Mar and `ocean-beach` at Sloat & Great
  Highway), and 10 world events.
- **Scenery-aware city lots (`mp_levels::city::Placer::builds_here`) and
  coastal shoulders (`mp_track::Track::walks`):** city buildings and garages
  spawn only on corridors whose zone uses `"Town"` scenery (and never closer to
  `0..FIRST_BAYSIDE` than to `BAYSIDE`), keeping `canada-road`,
  `pescadero-creek`, `skyline-north` and `sharp-park-road` forested under the
  Land kit. Excluding `"coastal"` and `"mountain"` road types from
  `Track::walks` gives Highway 1 North real town buildings through Montara and
  Pacifica without urban kerbs or sidewalks on the coastal bluffs.
- **Overpass clearance check (`mp_track::network::find_crossings`):** compares a
  freeway's available grade room against the actual clearance required above
  the crossed road (`surface_y + OVERPASS_CLEAR - ft.py[fi]`), preventing
  under-cleared viaduct lifts where a road crosses above a depressed freeway
  near a junction.

### 1.3 Network-Wide Events (`mp_track`, `mp_sim`, `mp_game`)

- **`EventSpot::road` and `EventRun::road`:** generalized Stage 5's loop-only
  events to any corridor in the network while keeping `road == 0` bit-for-bit
  identical (verified by existing event, replay and parity suites; `race::hash`
  mixes `e.road` only when `e.road != 0`).
- **Deterministic branch simulation (`mp_sim`):** `grid_on` places player and
  rivals on `t.road(r)`; `SimRival`, `AiDriver`, `autopilot_at`, `player_s`,
  `wrong_way` and `track_progress` follow `t.road(r)` with traffic filtered to
  `o.road == r` and unwrapped non-loop finish progress.
- **Client HUD, beacons, guide & minimap (`mp_game::play`):** event start
  beacons, HUD route progress bar, racing guide line (hidden during open-route
  deliveries) and the corner/paused atlas highlight all project onto the
  event's own corridor.
- **Five new playable branch events:**
  1. **Devil's Slide** (`devils-slide-run`, Race on `coast-north`, 9.5 km game)
  2. **The Tunitas Climb** (`tunitas-climb`, Time Trial on `tunitas`, 7.6 km game)
  3. **Crystal Springs** (`crystal-springs-run`, Time Trial on `canada-road`, 6.0 km game)
  4. **The King's Highway** (`kings-road`, Race on `el-camino`, 21.8 km game)
  5. **The Ferry Express** (`ferry-express`, Delivery from Lombard Street to the Ferry Building, 3.7 km game)

### 1.4 God Mode Atlas Readout & Overview Overlay (`mp_game::viewer`, `atlas.md` 5.3)

- Inverted `geo.json`'s exact equirectangular projection in `mp_game::viewer`
  (without adding `mp_atlas` or `serde_json` to the client/wasm dependency
  graph).
- Live viewer HUD readout and `--report` JSON now emit `[lat, lon]` and nearest
  corridor `id @ s/len km` on atlas levels (`southcoast`, `peninsula`), and
  Overview draws all branch corridor polylines and labels alongside road 0.

## 2. Measurements: Before and After

| Metric | Before (`7c4d836`) | After (`mp/atlas-expand`) |
|---|---:|---:|
| Regions in `plan.json` (`built` / `planned` / `later`) | 11 (`5` / `4` / `2`) | 11 (`9` / `0` / `2`) |
| Places in `plan.json` | 52 | 64 |
| Routes in `plan.json` (`built`) | 26 (`0` flagged) | 32 (`32` `built`) |
| Closed circuits verified by `mp-atlas report` | 1 (`69.8 km` real) | 6 (`340.5 km` real sum) |
| Corridor `.txt` files in `assets/atlas/peninsula/corridors/` | 24 | 30 (`1.1 MB`) |
| Playable roads in `mp_levels::peninsula` | 20 (`318.5 km` real / `159.2 km` game) | 26 (`420.0 km` real / `210.0 km` game) |
| Network junctions & freeway overpasses | 48 junctions | 68 junctions (19 crossings), 6 overpasses |
| Placed buildings & towers (`mp_levels::city`) | ~21,000 buildings | 24,736 buildings, 268 towers (tallest 311 m) |
| Cruise start locations | 6 | 8 (`+ pacifica`, `+ ocean-beach`) |
| Playable world events (`loop` / `branch`) | 5 (`5` / `0`) | 10 (`5` / `5`) |
| Streamed 1 km world squares (`Planned::sections`) | 239 | 295 (`stream_cost`: 2.57 s plan, 145 MB held) |
| Ground / cover / city binary assets | `1.56 MB` | `1.95 MB` (`887+19+783+32+225 kB`) |
| God Mode (`mp_game::viewer`) atlas readout & branches | None | `[lat, lon]`, `corridor @ s/len km`, Overview branches |

## 3. What Was Not Finished

- **Multi-corridor races and pursuits across junctions:** all six closed
  circuits (`the-loop`, `south-redwoods-loop`, `north-coast-loop`,
  `crystal-springs-loop`, `bayside-spine-loop`, `sf-city-ocean-loop`) can be
  driven seamlessly in Cruise and used by `Delivery` events, and single-corridor
  `Race` and `TimeTrial` events now work on any branch road, but AI race rivals
  and Hot Pursuit police do not yet follow a multi-road turn list across
  junctions.
- **Off-road dirt networks (`state-parks`, `crawler-hills`):** the two
  remaining `"later"` regions in `plan.json` (El Corte de Madera / Purisima
  Creek fire roads and Montara Mountain / McNee Ranch crawler trails) still
  need sub-tertiary dirt track extraction in `tools/atlas/build.py` before
  their corridors can resolve.
- **Caltrain & bayshore salt-pond set-pieces:** the rail line is in `geo.json`
  and `salt-ponds` is in `plan.json`, awaiting the `desert` level remix.
- **Browser GPU stutter run:** native streaming/warm-up budget tests
  (`every_section_streams`) pass on the expanded 26-road world; headless
  browser GPU stutter suites were not run from this worktree.

## 4. Questions & Decisions for the Owner

1. **World scale (`0.5`) vs regional compression (`atlas.md` 4.1):** at `0.5`,
   the expanded network is `210 km` of playable road (`21.8 km` for *The
   King's Highway* up El Camino Real; `13.8 km` around the SF Ocean Beach /
   Geary / Twin Peaks loop). Keep uniform `0.5`, or compress long bayside
   spines tighter than the coast and mountain climbs?
2. **Multi-corridor circuit races vs point-to-point corridor runs:** should the
   five new multi-road circuits (`south-redwoods-loop`, `north-coast-loop`,
   `crystal-springs-loop`, `bayside-spine-loop`, `sf-city-ocean-loop`) become
   lapped circuit races with junction barriers/chevrons (Burnout-style), or
   stay open-junction checkpoint loops?
3. **Old Devil's Slide cliff road vs Tom Lantos Tunnels:** `coast-north`
   currently runs through the real Highway 1 tunnels (16 % max grade); should
   we also open the old cliffside trail round Devil's Slide as a parallel
   scenic/shortcut branch?
4. **Region outlines, seasons, and classic-level road trips (`atlas.md` 4.2–4.4):**
   confirm whether Sierra, Desert and Seaside Raceway remain road-trip
   destinations off the edges of I-280 / US-101 / Highway 1, and which season
   *Midnight on the Coast* should wear by default.
