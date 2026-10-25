# The Peninsula slice: building the open world

Status: **started 2026-10-13**; stage 6, the bay side, started 2026-10-24 (vision ROADMAP M18, brought forward: the
owner asked to start building out the open world). The existing levels
stay as they are and need not connect to the world; abstract levels (the
owner's holodeck idea) are welcome later as destinations of their own.
The atlas (`docs/vision/atlas.md`) is the world's plan; this document is
how the plan becomes a drivable world, in stages that are each playable
on their own.

## 1. What the open world needs that a level does not

A level today is one road: a ribbon the simulation projects the car onto
(`Track::project`, `surface_y`, the walls either side), which the world
generator shapes land around and dresses with a scenery kit. The arcade
physics keeps the car between the walls. Everything else (traffic,
rivals, police, the HUD's route bar) is measured along that ribbon.

An open world needs four things on top:

1. **Roads from real lines.** A corridor's centreline comes from the
   atlas (a real road, compressed), not from a turtle walk of
   `[length, turn, rise]` segments.
2. **Ground everywhere.** A height field under the whole box, with the
   roads blended into it, so the car can leave the road. The port kept
   this seam (port SPEC 4.5): the sim car already reads the world through
   `mp_vdyn::Ground`; the arcade physics still reads the ribbon directly.
3. **A road network.** Several ribbons meeting at junctions; the
   projection picks the nearest road; the junction surface is shared.
4. **Streaming.** The world generated per tile, deterministically from
   the atlas, the plan, the seed and the tile's coordinates, built in a
   ring around the player.

Multiplayer relies on determinism (CLAUDE.md principles 3 to 6), so every
one of these stays a plain function of its inputs: no clock, no hash
maps, exact maths through `mp_math`'s kernel.

## 2. The stages

Each stage is playable, each has a gate the owner can try, and the
design of a stage is written here before its work starts.

### Stage 1. A corridor from the atlas (started 2026-10-13)

The first race, **Midnight on the Coast** (the crest above San Gregorio
to Half Moon Bay's big intersection), as a level the menu lists, built
from the real road through the existing coast generator.

- **`mp-atlas corridor <event>`** writes `assets/atlas/peninsula/corridors/<event>.txt`:
  the event's route resolved over the real roads and profiled over the
  real ground, then compressed to the plan's scale about the world's
  origin, resampled every 5 game metres, with the land cover beside each
  sample. The file is committed, small (about 60 kB) and diffable, and a
  test fails when it is stale, as `resolved.json`'s does. The game never
  reads the atlas itself: `mp_levels` includes the corridor file at
  compile time, as Seaside's survey is read, so the simulation crates'
  rules (no dependencies beyond `mp_math` and `mp_track`) hold.
- **`Route::Path`** in `mp_track`: an open road given as a path (x, z,
  y per point), beside `Segments` and `Loop`. It is `Loop`'s builder
  without the wrap: the same resample to a metre, tags and road types by
  metres, zones by metres, curvature from the geometry. A point-to-point
  race on it runs as one on segments does (start, finish, runout, the
  route bar).
- **The level** (`mp_levels::south_coast`, id `south-coast`): the
  corridor's path splined as Seaside's is, the coast kit's cliff road
  (`coastal`) for the bluffs and the beach kit for the town's last
  stretch, zones switched where the real cover turns urban, a night sky
  with the moon over the sea, Coast Highway's traffic, police and rivals.
  The ground round the road is still the kit's own landform: the real
  ground arrives with stage 2.
- **Scale 0.5** (the owner, 2026-10-13), horizontal and vertical alike,
  so grades stay true: the climb from the junction to the crest is
  62 game metres over 1.05 km, as steep as the real one. The crest stays
  a crest.
- **Gate:** the owner drives the first race from the menu and recognises
  the road: the climb to the crest, the farms, the drop to the creeks,
  the town's lights at the end.

### Stage 2. Ground under the whole box (built 2026-10-13)

- **`tools/atlas/ground.py <corridor>...`** writes `<corridor>.ground.bin`
  beside the corridor files: the Terrarium terrain tiles (15 m real, the
  same source as the planning grid, cached in `tools/atlas/cache/`)
  resampled onto a grid of game metres over the corridors' box plus a
  margin (16 m and 1.2 km since stage 3; 12 m and 1.5 km before), heights
  compressed as the road is. 541 kB zlib in git;
  `mp_levels::ground::HeightGrid` reads it (bilinear, clamped) and the
  level includes it at compile time, inflated once when the level is
  first made. Packing it into the wasm rather than fetching it as
  Seaside's survey is fetched is a delivery choice for the owner: this
  way needed no page, loader or host plumbing.
- **An open level** (`Level::open`): its `ground` is the land under the
  whole box. The Track blends the road into it (`Track::ground_y`): the
  road's own surface out to the drawn shoulder, a metre's step down to
  the land with the road blended in by the coast landform's law
  (`OPEN_R0`, `OPEN_BAND`, `OPEN_POW`, `OPEN_DROP` in `mp_track`), and
  the land itself past the band. World generation's terrain uses the
  same constants in its near field and the level's ground as every
  zone's landform, so the car sits on what is drawn: a test holds the
  two within 0.75 m along the corridor, and equal past the band.
- **The arcade car off the road:** on an open level the walls are gone
  (no `collide_walls`), the height comes from `ground_y`, and past the
  shoulder the car is on loose ground (the circuit run-off's drag). The
  sim car's `TrackGround` reads the same ground and offers no wall
  colliders. Rivals, traffic and police are unchanged: they live on the
  ribbon.
- Not yet: the land's cover (farms, scrub, forest) is still the kit's
  colouring and planting over the real shape; the coast kit's sea stacks
  and cliffs are placed by its own rules, so some stand on dry land.
  Progress along the road is still the nearest point of the ribbon, so
  a field cuts a corner; a road network (stage 3) is where that gets
  its proper answer.
- **Gate:** pull off Highway 1 at the crest and drive across a field.

### Stage 3. Two roads meeting (built 2026-10-13)

- **Two corridors.** The level's road is now the whole of `coast-south`
  (the junction at San Gregorio to Half Moon Bay, 8.7 km); the race
  still starts at the crest, which the level finds as the highest point
  of the road's first 2 km. `hwy-84-lower` (a route added to the plan:
  the junction to La Honda, 7.1 km) is the first branch. The corridor
  writer now smooths the map's line (a triangular window 30 m either
  side, real) and writes the samples to the centimetre, so the road is a
  curve and not the map's chain of segments.
- **A branch** (`Level::branches`, `mp_track::Branch`): a corridor of its
  own with a road type and a zone of the level's, starting where it meets
  the parent. `Track::new` builds each as an open path
  (`Track::branches`, `BranchTrack`): the path is trimmed to where it
  leaves the parent's pavement and started on the parent's centreline;
  over the parent its heights are the parent's surface (lifted 2 cm over
  the shoulder so the meshes do not fight), and its own again 120 m on;
  the parent's fences open at the junction. The branch is one zone, its
  own road type the whole way.
- **The road under the car** (`Track::project_road`): the road the car
  was on keeps it while its shoulder holds it; otherwise the road whose
  shoulder does, or the one it is least far off. `Vehicle` carries
  `road`, `road_s` and `road_lat` beside `s` and `lat`: on a branch `s`
  and `lat` are still the car's place on the level's road, so the race,
  the rivals, the police and the HUD measure progress as before (a car
  up 84 is stopped at the junction, far off the road, as far as the race
  is concerned). The arcade physics, the sim car's ground adapter and
  the steering assists drive the road under the car.
- **World generation**: the terrain's box covers every road; the far
  field's distance and road height take every road (the mesh's detail
  and scenery's road clearance follow 84 too), its s, side and zone stay
  the level's road's; the near field blends every road into the land, so
  the junction is one surface (a test holds 84's first metres to Highway
  1's surface within 8 cm and the drawn ground beside the mouth within
  0.75 m). On an open level the zones follow the roads, not x
  (`Terrain::zone_weights_at`: the nearest road sample's zone, blended
  between the far field's cells), which also puts the crest in the coast
  zone's colours. The road builder makes a `Road` per branch, begun at
  the parent's paved edge, with no start or finish line and nothing
  across its mouth.
- Not yet: traffic, rivals and police stay on the level's road; the
  branch has no scenery (the redwoods of 84 are planting by land cover,
  which the whole open world still wants); the end of a road is open
  (the car drives off its end onto the land); the branch's own s is not
  shown anywhere.
- **Gate:** turn off Highway 1 onto 84 and drive up to La Honda.
- 2026-10-14 (D1211): the camera's floor follows the road under the car
  (it had been the level's road's plane, which on 84 is far off and
  banked); and the level card offers **Cruise** beside Race and Hot
  Pursuit: the corridor with no race on it, the Night City Cruise's
  score, no rivals, the level's traffic. The owner's "light night city
  cruise but the new map".

### Stage 4. The loop as one world (started 2026-10-14)

The loop (1, 92, Skyline, 84) as one drivable world, in parts that are
each playable. The whole-level build the game has (plan, terrain fields,
tiles, road, scenery, then the scene handed to the client) stays the
shape of a build; streaming (4b) makes it incremental rather than
replacing it.

**4a. The loop as one road, and planting by land cover** (built
2026-10-14, D1212).

- **The data.** Corridors `hwy-92`, `skyline-south` and `hwy-84` (the
  whole of 84, Skylonda to the sea; `hwy-84-lower` is its last 7 km) join
  `coast-south`: the four meet within 4 m and close the loop, 34.8 km of
  game road. `peninsula.ground.bin` is the real ground for the loop's box
  (16 m cells, 1.2 km margin, 660 kB) and the south coast level drives on
  it too (`coast-south.ground.bin` is gone; `ground.py --name`). Beside
  it `peninsula.cover.bin`: the land cover of every ground cell from the
  atlas's planning grid (ESA WorldCover and Overture's land cover, 50 m
  real), one byte a cell, read by `mp_levels::ground::CoverGrid`, carried
  by `Level::cover`. Both still packed into the wasm (D1208's choice).
- **The level** `peninsula`, "The Loop": `Route::Loop` through the four
  corridors splined closed; a race is one lap, from the crest above San
  Gregorio round to the crest; the card offers Cruise (D1211). Zones by
  metres along the lap: the coast (Highway 1, the Coast kit), Half Moon
  Bay (the Beach kit), over the hill (92, the Land kit), Skyline (35 and
  84, the Land kit); road types coastal, boulevard, valley, mountain.
- **The Land kit** (`mp_worldgen::land`): the open world's planting by
  land cover, which stages 2 and 3 wanted. Within reach of the roads
  (the far field's distance) it walks the land in 512 m cells and plants
  what the cover says: firs and redwood-dark spruce where it is forest
  (verge, mid and far detail by distance from the road, as the Sierra's
  forest is built), oaks and scrub where it is shrub, scattered oaks on
  grass, nothing on crop, urban, barren, sand or water, nothing on the
  road's clearance, below the sea or on the steepest ground. Every cell
  is seeded from the level's seed and its coordinates, so a cell's trees
  are a function of the cell alone: what 4b streams.
- **Gate:** the owner drives the loop from the menu, built whole behind
  the loading screen.

**4b. Streaming.** The plan once (the track, the terrain's fields, every
module's `plan()`), then the tiles, the road's chunks and the kits'
chunks built in sections round the player (`mp_worldgen::section`,
cached by section), spawned as they arrive and dropped behind; the level
starts with the section round the start, so there is no loading screen
to speak of. Natively on the build thread; on the web stepped on the
main thread between frames with a small budget (a hitch now and then,
until 4c). Landed 2026-10-14: the world's half (D1213:
`mp_worldgen::stream::Planned`, partitioned sections, the base layer)
and the client's (D1214: `mp_game::stream`, the ring round the camera,
several scenes' animators at once, shared images, `?stream=0` for the
whole build).

**4c. The web build off the main thread** (built 2026-10-14, D1215): a
worker running the builder as its own wasm (`mp_builder`, as `mp_exhaust`
and `mp_music` are), each section handed back packed with its animators
as data (`mp_worldgen::anim`), read and spawned by the client; `?worker=0`
keeps the stepped build.

**4d. The branches, the ends, the fields, the fog** (built 2026-10-14,
D1216): Tunitas Creek Road (Highway 1 to Skyline, open at both ends) and
Stage Road (San Gregorio to Pescadero) as branches, streamed as sections
of their own road with the ring crossing the junctions; every road end
barred with sawhorses and a ROAD CLOSED board; crop painted as fields and
fenced along the road and across; Skyline's fog by zone, drifting.

- **Gate (4b):** the owner drives the whole loop without a loading screen.

### Stage 5. Events at places (built 2026-10-14, D1219)

- The plan's places and events become starts in the world: hold gas and
  brake at a marked intersection, as Burnout Paradise does. Races, time
  trials, a delivery from a garage in Half Moon Bay.
- **Gate (M18's):** from pressing Drive, the owner finds and plays a
  race, a time trial and a delivery without opening a menu.

**The design.**

- **The events are the level's data** (`Level::events`, `mp_track::
  EventSpot`): a name, a kind (race, time trial, delivery), where it
  starts (metres along the loop, road 0), where a race or a trial ends
  (metres along the loop, the lap's way), or where a delivery goes (a
  point, reached by any road) and how long it may take. The loop's are
  the plan's four, at the corridors' joins: Midnight on the Coast (the
  crest to Half Moon Bay, a race, its start where Cruise begins), Over
  the Hill (92, a race), Skyline at Dawn (Skyline, a trial) and The
  Redwood Descent (84, a trial); and a delivery from the garage in
  town to the San Gregorio General Store at Stage Road's mouth, the
  short way south down the coast or the long way round. The plan gains
  the garage, the store's delivery and the kind `delivery`.
- **The simulation starts them** (`mp_sim::event`), so a recording
  replays and the rules stay one implementation. While cruising, a car
  stopped on a start's marker with both pedals down fills a one-second
  hold; then the race leaves the cruise: the event's rivals join on a
  grid behind the marker (a race's), the traffic round the grid goes,
  the countdown runs, and the progress that finishes is the start's
  plus the event's length, counted as a circuit's is (so rivals finish,
  park and are placed by the code that does it on a circuit). A
  delivery ends within 15 m of its point or when its time runs out, and
  knows no wrong way. At the results the cruise comes back: the rivals
  go, the cruise's score and multiplier are as they were.
- **The client shows them**: a beacon of light over each start (and the
  delivery's drop), a card when the car is near one ("hold gas and
  brake"), the hold's fill, the race HUD during the event (a delivery's
  distance and time left), the results, then the cruise. The field's
  models are rebuilt when the rivals come and go. The minimap shows the
  starts within 2 km and a delivery's drop, kept on its rim when off it
  (D1220).

### Stage 6. The bay side: the city from Redwood City to San Francisco (started 2026-10-24)

The owner (2026-10-24): "build out the inland area, Redwood City up to
SF, to give more variety to where we can start the open world mode.
Coastside is a bit too sleepy to start there." The atlas's Bayside and
City regions (atlas.md 3.3, WORLD 3: "over the hill the city is the
peninsula") become drivable, joined to the loop over the hill, and the
cruise can start in them.

**What stands in the way** (a survey of the code, 2026-10-24). The open
world is a star: road 0 (the loop) and branches that leave it and may
come back to it, never to each other. Nearly everything measures along
road 0: race progress, traffic, rivals, police, events, zones (the card,
the route bar, the radio, the fog), the far field's s, side and zone
(the land's shape and colour), the minimap. Finding the road under a
point is O(roads) (`Track::project_road` off the shoulder,
`Section::s_of` with a 900 m reach on every call). Sections are
(road, metres) and the ring round the camera runs along one road. The
far field stamps a 3 km disc every 12 m of road, and the near field's
tiles exist wherever a road passes and are held for the whole level.
The open world has no walls and nothing in the simulation stands up
off the ground, so a car would drive through a city's buildings. None
of that is wrong for the loop; all of it breaks with a hundred
kilometres of crossing streets.

**The parts**, each landed on its own and each keeping the loop as it
plays today:

- **6a. Frame stutter tests** (D1238). Before the world grows, a way to
  see when a frame is late and why, so each part that follows is
  measured: a browser suite that drives the open world on the GPU and
  holds frame times to budgets (desktop, phone emulation, both
  backends), and native tests that count the work a frame does
  (entities spawned, mesh and texture bytes uploaded, pipelines first
  used after the warm-up) and hold it to budgets in CI, where no GPU
  times anything.
- **6b. The bay side's data** (D1239). Routes for the plan (below),
  their corridor files, the ground and land cover under the bay side's
  box, and a **city grid**: the real buildings (Overture Maps, about
  half a million footprints, 80 % with a height) reduced to cells of
  how much of the ground is built on and how tall it stands (the median
  and the ninetieth percentile). Buildings are **not** shrunk with the
  map: the roads are half size, the cars and the buildings are full
  size, so the city is the real one's density and skyline drawn with
  fewer, full-size buildings.
  Built 2026-10-24: the 17 routes below in `plan.json` and their
  corridors (222 km real, 111 km game), `bayside.ground.bin` and
  `.cover.bin` (24 m cells), `bayside.city.bin` (`tools/atlas/city.py`,
  `mp_levels::ground::CityGrid`).
- **6c. A road network** (D1240). Roads that meet any road, at their
  ends or crossing in the middle: junctions found from the geometry (a
  road that ends on another, two that cross), each naming both roads and
  the metres along each. Where two roads meet, the lesser fits its
  surface to the greater's (freeway, then the numbered highways, then
  streets), as a branch fits its parent today. One spatial index over
  every road's samples, tagged with the road, answers "which roads are
  near here" in constant time for the simulation, the terrain and the
  sections. A dead end is barred only where the real road goes on and
  the world's does not. All crossings are at grade for now: the arcade
  game has intersections where the real freeways have interchanges.
- **6d. Streaming by place** (D1241). On a network the sections are
  cells of the ground (1 km squares), not stretches of one road; the
  ring is the cells round the camera. The far field's distance becomes
  a distance transform (cost by area, not by road length), the near
  field's tiles are built with their section and dropped with it, and
  the terrain's lookups go through the index. The loop keeps its
  sections along the road: a level is one or the other.
- **6e. The city kit** (D1242). Blocks are the land between the roads
  where the cover is urban; the city grid says how full and how tall.
  Lots along each block's frontage, buildings from the City and Streets
  kits' parts (facades, the atlas of windows, lamps, signals), towers
  where the grid stands tall (downtown San Francisco, San Mateo,
  Redwood City), houses and low shops where it does not. **The
  buildings are the simulation's**: the lots are placed by the level
  (`mp_levels`), deterministic from the grid, the roads and the seed, as
  the events are; the simulation collides cars with their footprints
  (the walls' response: push out, scrub, the impact sound), and world
  generation draws what the level placed.
- **6f. Starts** (D1243). The cruise can start at a place: a start is a
  road and metres along it, the grid and the camera and the first
  sections there. The card offers them: the crest above San Gregorio
  (today's), Redwood City, San Mateo, South San Francisco, the Mission,
  Lombard Street.
- **6g. Traffic on the network.** Traffic, rivals and police live on
  road 0 today. Traffic cars that drive any road and turn at junctions
  come after the city stands; until then the city's streets are empty
  and its races wait.

**The first network** (6b; a draft the planner can change). At the
plan's scale of 0.5, Redwood City is about 9 km east and 1 km north of
Half Moon Bay's intersection, downtown San Francisco 1 km east and
18 km north. The roads, from the atlas (tertiary and up):

- **Over the hill:** Highway 92 from Skyline down past Crystal Springs
  to 280 and on to 101 at San Mateo; Highway 84 from Skylonda down
  Woodside Road to 280 and on to 101 at Redwood City. These join the
  bay side to the loop.
- **The spines:** 101 from Redwood City to San Francisco; 280 from
  Woodside Road to San Francisco; El Camino Real (82) from Redwood City
  to Daly City, then Mission Street.
- **Across:** six streets between 101 and 280 or El Camino (Ralston,
  Hillsdale, Third Avenue, Broadway, Millbrae Avenue, San Bruno Avenue).
- **San Francisco:** Highway 1 (19th Avenue, Park Presidio) from 280 to
  Lombard; Lombard to Van Ness; Van Ness and Market, Geary, the
  Embarcadero.

About 230 km of real road, 115 km in the game, three times the loop. The
residential streets stay out: the city is the arterials and the blocks
between them, drawn full.

- **Gate:** start a cruise in Redwood City, drive up El Camino through
  the towns to San Francisco and back over 92 to the coast, without a
  loading screen and without a frame the stutter suite flags.

**Built (2026-10-24).** 6b's data (D1239), the network (6c, D1240),
streaming by place and the far field by propagation (6d, D1241), the
buildings and the Town kit (6e, D1242), the starts (6f, D1243) and the
traffic round the player (6g, D1244), and the peninsula level joined to
the bay side: 20 roads, 48 junctions, 21 thousand buildings, six starts,
streamed in 239 kilometre squares. Looked at in the browser
(`tools/parity/cruise-shots.mjs`): Redwood City, San Mateo and Lombard
Street at night, every window lit by its building's seed, San
Francisco's towers on the skyline. Not yet: the at-grade crossings meet
the freeways' real ramps; the ground between the kerb and the buildings
is the land's colour, not sidewalk; Market Street's end stops 175 m
short of the Embarcadero; rivals and police are still the loop's; the
events are all on the loop.

6a's tests (D1238) found the stream's work in one piece where a frame
could not take it; fixed with the bay side's own hitch (D1245): a
warm-up scene of every pipeline the level draws, a texture's pixels sent
once and converted in slices, oversized meshes split. The native budget
test passes; the browser suite's budgets still want a quiet machine's
baseline before they are trusted.

## 3. Decisions

- **The levels stay.** Nothing in the menu changes; the corridor is a
  seventh entry. The remix of the existing levels (atlas.md 3.5) waits.
- **Road trips, not remixes, for the Sierra, the desert and the
  raceway:** they remain levels; the open world is home. Abstract
  levels (a holodeck grid, a figure of eight in the void) fit the same
  slot when they come.
- **The corridor file is text.** One line a sample: `x z y cover`, in
  game metres. It is read with `split_whitespace`, so `mp_levels` gains
  no dependency, and a change to the road shows in a diff.
- **Heights scale with the ground plan** (0.5 both ways). A vertical
  exaggeration can be added per route in `plan.json` if a road feels
  flat; none is applied now.
- **The real profile is smoothed** over 60 real metres before it is
  compressed: the planning grid is 50 m cells, and the road's own
  smoothing in `Track` (`elevation_smooth`) handles the rest. Stage 2's
  15 m tiles replace this.
