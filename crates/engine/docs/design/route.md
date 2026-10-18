# Design note: the `Route` seam (`pk_route`)

Status: **written 2026-10-16, for the owner's yes before phase 5 carves
`pk_route` and `pk_scenery`** (PLAN.md section 5 items 3 and 6). Nothing
exists yet. Read with Midnight's `crates/mp_track/src/track.rs`,
`mp_worldgen/src/{terrain, terrain_mesh, road, section, stream}.rs`,
`mp_game/src/stream.rs` and `docs/vision/peninsula.md` stages 1 to 5, and
with snowboarding's `crates/sb_sim/src/course.rs` as the second
route-shaped game.

## 1. What the seam is for

Three of the planned games are a ribbon through a world (snowboard, river,
bicycle) and the one that ships already is. Everything in Midnight that
hangs off that ribbon reads `Track`'s 1 m `f32` arrays directly, and every
one of those readers is something a second game would copy:

- the chase camera's floor (`play/camera.rs floor()`: `surface_y` on a
  closed level, `project` then `ground_y` on an open one);
- terrain sculpting (`Terrain::new` and `build_fields`: the box round the
  roads, a far field of distance-to-road and road height on a 32 m grid, a
  near field of flattening strength on 4 m tiles, then `height = lerp(
  landform, road height, k)`, with flattens and carves);
- the tile mesher (`tile_list`: 4 m within 60 m of the road or where a
  near tile exists, 16 m to 700 m, 32 m beyond; `build_tile_geometry` with
  stitched edges and skirts);
- the extrusion of a cross-section along the centreline (`road::extrude`),
  under the road surface, its markings, barriers and the scenery kits'
  fences;
- the sky's progress along the route and its zone fog (`SkyParams`);
- planting (`land.rs`: cells within reach of a road, nothing on the road's
  clearance) and the living world's anchors (`life/`: flocks over stretches
  of a zone, people on a beach);
- streaming (`stream::Planned` and the client's ring: sections of a
  kilometre of `s` per road, two ahead, one behind, one of slack);
- the viewer (`viewer/mod.rs`: `frame(s)`, `start_s`, `is_loop`,
  `road_end`, `wrap`, the zones for the panel and the overview).

Each of those is written once in the kit against the trait below, and each
game implements the trait. The trait is the smallest thing those consumers
need. Racing (rivals in `(s, lat)`, traffic, police, events at places) and
the simulation's own reads of the track are not consumers: `mp_sim` keeps
calling `Track` directly, and the parity digests with it.

## 2. The traits

```rust
pub trait RouteFrame: Copy {
    fn pos(&self) -> DVec3;        // centreline point, at its surface height
    fn tangent(&self) -> DVec3;    // unit, horizontal, along +s
    fn right(&self) -> DVec3;      // unit, horizontal, +lat
    fn bank(&self) -> f64;         // rise of the surface per metre of lat, to the left
    fn half_width(&self) -> f64;   // the surface's reach either side (m)
}

pub trait Route {
    type Frame: RouteFrame;
    fn len(&self) -> f64;
    fn is_loop(&self) -> bool;
    fn frame(&self, s: f64) -> Self::Frame;
    fn point_at(&self, s: f64, lat: f64) -> DVec3;
    fn surface_y(&self, s: f64, lat: f64) -> f64;
    fn ground_y(&self, x: f64, z: f64, s: f64, lat: f64) -> f64 { self.surface_y(s, lat) }
    fn project(&self, x: f64, z: f64, hint: f64) -> (f64, f64);          // (s, lat), local search near hint
    fn locate(&self, x: f64, z: f64, radius: f64) -> Option<(f64, f64)>; // global, radius-limited
    fn wrap(&self, s: f64) -> f64;
    fn ds(&self, a: f64, b: f64) -> f64;
}

pub trait RouteNetwork {
    type Member: Route;
    fn members(&self) -> &[Self::Member];                  // member 0 is the main one
    fn junctions(&self) -> &[Junction];                    // { member, s, side, onto, onto_s, open_end }
    fn project_network(&self, x: f64, z: f64, member: usize, hint: f64) -> (usize, f64, f64);
}

pub trait ViewerRoute: Route {
    fn start_s(&self) -> f64 { 0.0 }
    fn view_len(&self) -> f64 { self.len() }
    fn marks(&self) -> Vec<Mark> { Vec::new() }            // { s0, s1, name }
}
```

Why each part exists, with the consumer that needs it:

| Item | Needed by |
|---|---|
| `Frame` as an associated type behind `RouteFrame` | `extrude`'s profile closures read the frame, and a game's closures read fields of its own beyond the five the kit knows. The game's own frame type stays the type the closures get, and the kit reads it through the five accessors |
| `len`, `is_loop`, `wrap`, `ds` | the camera's lerp between ticks across a loop's seam; sections (`holds`, `clip` over the wrap); the viewer's slider and `jump`; the sky's progress |
| `frame(s)` | the extrusion (one frame per step), the terrain's sample walk, the section's points every 10 m, the viewer's cameras and overview, planting's clearance |
| `point_at`, `surface_y` | the camera's floor where the player stays on the route; beacons, chevrons and labels placed at `(s, lat)`; the sim's kinematic bodies today |
| `ground_y` defaulting to `surface_y` | the camera's floor where the player may leave the route; the near field's "the route's plane out to the shoulder, then the land" so the player sits on what is drawn. A route with no ground under the whole box keeps the default |
| `project(x, z, hint)` | the camera's floor, the viewer's `nearest_s`, `project_network`; cheap because it searches near `hint` (a window of samples widened at its edge, or Newton steps on an analytic centreline) |
| `locate(x, z, radius) -> Option` | the partition (which member a point belongs to), planting's clearance, the far field's nearest member: the global search a sampled route does with a spatial hash. `None` means nothing in range, which the partition and the planting both act on |
| `RouteNetwork` on top, not in `Route` | the terrain's far field, the partition, planting and the streaming ring iterate every member; a junction is where the ring crosses and where the extrusion stops a joining member's surface. A single route is a network of one member with no junctions (a blanket impl), so a game with one ribbon never sees the network |
| `ViewerRoute` as a sub-trait with defaults | the viewer wants three things no other consumer does: where to start, how far the slider runs (a route may be drivable past `len`, as a runout), and named stretches for the panel and the overview. Defaults mean a route that implements nothing extra still opens in the viewer |

**Methods on `&self`, unlike `Simulation`.** A route is a value the whole
client shares behind an `Arc`, and the simulation's immutable context holds
one too; it is not a marker type: there is state and it is immutable. `f64`
throughout (Q1).

## 3. Splitting `Level`

`Level` today mixes the route definition with the ground and the racing.
It splits in three, with `Level` keeping all its fields so `mp_levels`'
sixty definitions do not change:

- **Kit, `pk_route::Site`**: the route or network, `ground: Option<GroundFn>`
  (`Arc<dyn Fn(f64, f64) -> f64>`, the land under the whole box),
  `cover: Option<CoverFn>` (`-> u8`, an open class byte; the atlas's numbering
  is `pk_atlas`'s), `sea_y`, the margin round the box. `Level::site()` builds
  one.
- **Kit, `pk_scenery::SkyKeys<T: Lerp>`**: keys at fractions of the route,
  `sample(p) -> T`. Midnight's `SkyKey` is the `T`; `progress(s)` (a loop
  pinned at 0.5, an open road 0 to 1) is a `Progress` enum the game picks.
  Zone fog stays Midnight's, on top of the sample.
- **Midnight, `Level`**: zones, `SkyKey` rows, `TrafficRule`, `Police`,
  `RivalDef`, `EventSpot`, laps, start pose, `loose_ground`, `elevation`,
  the mode and the card text.

## 4. Terrain, tiles and extrusion

```rust
pub struct Landform {
    pub forms: Vec<(Box<dyn Form>, LandformParams)>,     // Form: fn height(&self, x, z, &FarSample) -> f64
    pub weights: Box<dyn Fn(f64, f64) -> Weights>,        // which forms where, summing to 1
}
pub struct Shoulder { pub half_width: f64, pub margin: f64, pub wall_l: f64, pub wall_r: f64, pub elevated: bool }

impl Terrain {
    pub fn new(net: &impl RouteNetwork, landform: Landform, site: &Site,
               shoulder: impl Fn(usize, f64) -> Shoulder, opts: &TerrainOpts) -> Result<Terrain, String>;
    pub fn add_flatten(&mut self, x, z, r, falloff, y: Option<f64>);
    pub fn add_carve(&mut self, points: Vec<[f64; 2]>, width, depth, under_road: bool);
    pub fn build_fields(&mut self, net: &impl RouteNetwork);
    pub fn height_at(&self, x: f64, z: f64) -> f64;       // needs only &self: runs on threads
    pub fn far(&self, x, z) -> FarSample;                   // { d, y, s, side, lat, open }
    pub fn road_info(&self, x, z) -> RoadInfo;
    pub fn tile_list(&self, policy: &LodPolicy) -> Vec<Tile>;
}
```

The kit keeps what is a function of the network and the noise: the box,
the far and near fields, flattens, carves (before or under the road), the
bilinear road blend, `slope_at`. Midnight's eleven landforms become
`Form` impls in Midnight; what they read beyond `FarSample` and the two
noises (the lookout windows of `open_bias`, `canyon_width`'s smoothed
walls, the harbour's channels from `bridge` tags, the desert rail, level
ground for flat zones) becomes state those impls build from the `Track`
before `Terrain::new`. Midnight's `weights` closure is today's by-x edge
blend with `blend` and `blend_offset`; a game whose route does not run west
to east writes a different one. `Shoulder` is what the near field reads per
sample (`hw`, `margin`, the walls, `elevated`) and `LandformParams` (`r0`,
`band`, `pow`, `flat`, `corridor`) stays data.

**The static LOD is data.** `LodPolicy { tile: 256.0, fine_near: 60.0,
steps: [(60.0, 4.0), (700.0, 16.0), (f64::INFINITY, 32.0)], fine_where_near_tile: true }`
is Midnight's `tile_list` verbatim; a snowboard course that wants 1.5 m
rows on the piste writes another. `build_tile_geometry(height: &dyn Fn(x, z)
-> f64, paint: &dyn Fn(x, z, h) -> ([f64; 3], f64), tile: &Tile) ->
TileGeometry` takes closures, so the colouriser (zones, the aerial photo,
shorelines) stays Midnight's and `terrain.height_at` is passed as the
height. Stitching to a coarser neighbour and the skirt are the kit's.
Snowboarding's `(s, u)` grid (`mesh_rows`, `mesh_columns`) is a second
mesher, `build_ribbon_geometry(route, rows, cols, height: Fn(s, u))`, which
a river wants too.

**Extrusion is generic; what it extrudes is not.** `extrude(route: &impl
Route, ranges: &[[f64; 2]], profile: &[ProfilePoint<R::Frame>], opts) ->
Geometry`, with `runs`, `chunks` and `group_runs`, moves as it is. The
`Road` struct (surface, markings by `marks` scheme, barriers by `edge`,
chevrons, viaducts, `classify_sides`, the start and finish lines, the
branch's mouth) is Midnight's `pk_scenery` client. The profile's `Lat::Fn`
closures get the game's frame type, which is why `Route::Frame` is an
associated type.

## 5. Sections and streaming

```rust
pub trait Partition {
    type Key: Copy + Ord + Hash + Debug + Send + Sync + 'static;
    fn key_under(&self, x: f64, z: f64) -> Option<Self::Key>;
    fn ring(&self, k: Self::Key, behind: usize, ahead: usize) -> Vec<Self::Key>;
    fn start(&self) -> Self::Key;
}
pub struct Stretch<N: RouteNetwork> { pub net: Arc<N>, pub section_len: f64 }   // Key = (road, index)

pub trait SectionStreamer {
    type Part: Partition;
    type Planned: Send + Sync;
    fn plan(&self) -> Result<Arc<Self::Planned>, String>;
    fn partition(p: &Self::Planned) -> &Self::Part;
    fn base(p: &Self::Planned) -> Build;
    fn section(p: &Self::Planned, k: <Self::Part as Partition>::Key) -> Build;
    fn pack(wb: &WorldBuild) -> Result<Vec<u8>, String>;
    fn unpack(bytes: &[u8]) -> Result<WorldBuild, String>;
}
```

`Stretch` is today's `Planned::{road_sections, road_of, section_of, ring}`
with `SECTION_LEN = 1000`: keys per road from `s = 0`, the ring along a
road wrapped on a loop and clamped at an open end, and the junction rule
(the branch's first two sections while the ring covers its mouth, the
parent's two round the mouth while the ring is at the branch's start, the
same at an open far end). A cell partition for the platformer, skydiving,
diving and the fantasy game is `key_under = (floor(x / cell), floor(z /
cell))` with a square ring, and nothing in the client changes.

**What the cut needs.** A partitioned section keeps a drawable, a tile or
a planted cell when `partition.key_under(centre) == Some(key)`: that one
rule replaces `Section::owns` and `Section::holds`, and it is what makes
the key generic. `Section::clip` (the road's chunks cut to exactly the
stretch's metres, in one or two pieces round a loop's wrap) is a stretch's
own concern and stays in the kit's `Stretch`. The `base` section (coarse
tiles, sky, sea, no road) is the horizon under any partition.

**What the ring and the executors need.** The client (`pk_play::StreamRing<S:
SectionStreamer>`) keeps `BTreeMap<Key, Sec>`, `wanted = ring(at, 1, 2)`,
`keep = ring(at, 2, 3)`, drops what is outside `keep`, starts one build at
a time nearest first, spawns one section a frame within a budget, and lets
`ready` go once the start's ring is up. It needs `Key: Ord` for the map and
`Copy` for the messages, a generation stamp on every answer (the thread's
`THREAD_OUT`, the worker's `Msg::Built`, the plan) so a level change
discards late results, `Planned: Send + Sync` for the thread executor, and
`pack`/`unpack` for the worker executor, which carry a `pk_scene::Scene`,
the animators that are data, the handle map and the log, and know nothing
of the key. The stepped executor needs only `Build::step`. Midnight's
`stream.rs` becomes the `SectionStreamer` impl over `Planned`, and the
`car-or-start` focus rule stays Midnight's.

## 6. Scenery hooks: anchors and sky

`Life::new(anchors: &impl AnchorProvider, species: &[Species])` with
`trait AnchorProvider { fn anchors(&self, kind: &str) -> Vec<Anchor> }` and
`Anchor { pos: DVec3, s: f64, weight: f64 }`. Midnight derives them from
its zones' `scenery` names (gulls on the coast and at the raceway, raptors
inland, people on the beach's stretch and the raceway's banners). A game
with no zones hands over a list. Planting (`plant_by_cover`) takes the
`Terrain` (for `far().d`, the slope and the sea), the `CoverFn` and a
clearance closure, and keeps its 512 m cells seeded by coordinates, which
is what a cell partition would stream unchanged.

## 7. How `Track` implements it, unchanged

**What is Midnight-specific and stays off the trait.** `road_end` (the
runout and a branch's open far end are racing's "how far can the car go";
the viewer's `view_len` covers its one kit use). `branches` and `roads()`
as fields of one struct: the kit sees them through `RouteNetwork`, which
`Track` implements as "itself and its `BranchTrack`s". `project_road`'s
rule (stay on the road you were on while its shoulder holds you, else the
road whose shoulder does, else the least far off) is the kit's
`project_network` default, since every network wants it. `zones`,
`zone_blend`, `fog_at`, `tags`, `fence_gaps`, `racing_line`,
`speed_profile`, `laps`, `finish_s`, `runout`, `loose_at`, the walls and
`elevated` are racing, scenery-kit or level content. **The 1 m `f32`
sampling is an implementation detail**: nothing in the trait says how a
route is stored, and `locate` with a radius is the only method that
rewards an index.

`impl Route for Track` is thirty lines of forwarding: `len` is `length`,
`frame` is `frame`, `point_at` reads `.x .y .z` off `point_at`'s frame,
`project` drops `Projection::i`, `locate` is `distance_to_road` with its
`Option<s>`, `ground_y`, `surface_y`, `wrap`, `ds`, `is_loop` as they are.
`impl RouteFrame for Frame` maps `(x, y, z)`, `(fx, 0, fz)`, `(rx, 0, rz)`,
`bank`, `hw`. `impl RouteNetwork for Track` returns itself then its branches as
the members and the junctions from `BranchTrack { s, side, end, open_end }`.
`impl ViewerRoute` returns `start_s`, `road_end() - 1`, and the zones as
marks. The `f32` arrays, `js::round`, the spatial hash and every store that
rounds at the JS's point stay. `mp_sim` keeps calling the inherent methods,
so the trace format and the L3/L4 digests cannot move. The one place that
must be checked rather than argued is `Terrain::build_fields`, which reads
`px[i]`, `rx[i]`, `py[i]`, `hw[i]` raw per sample: through the trait it
reads `frame(i as f64)`, whose lerp at `t = 0` returns the stored value
exactly but whose tangent is normalised. The carve lands with a test that
the far and near fields of the four `build-diff` levels are byte-identical
before and after, and Track's `frame` keeps a raw-sample path if they are
not.

## 8. How snowboarding's `Course` would implement it

`Course` is a winding centreline sampled every metre (`DS = 1.0`, `f32`)
with the snow's height as a function `height(s, u)`: `frame(s)` gives
`center`, `dir`, `right`, `kappa`, `half_width`; `world(s, u)` is
`point_at`; `height(s, u)` is `surface_y` and `ground_y` alike (there is
no road to leave); `locate(p, hint)` is `project` with a hint, six Newton
steps with a curvature scale. `is_loop` is false and `wrap` the identity.
`bank` is 0: its banking is baked into `height`, which is fine because
`point_at` and `surface_y` are what consumers read. What it lacks: a global
`locate` with a radius (its Newton search from `hint` only), which the kit
supplies as a default impl that scans every `n` metres then refines with
`project`; the `f64` boundary (it converts, and keeps `f32` inside, as the
`Simulation` note lets its integrator keep `TICK` as `f32`); and
`half_width` on a pipe, where it returns the coping. It gains the viewer,
the chase rig's floor, streaming by stretch if a course grows long, and the
ribbon mesher that is already its own `mesh_rows`/`mesh_columns`.

## 9. What stays game-side

- `Level`'s racing fields, zones, road types and the `Frame` extras the
  kits read (`hw`, `margin`, `wall_l`, `zone`).
- The eleven landforms, their by-x blend, the colouriser, the aerial photo.
- The `Road` builder: markings, barriers, chevrons, viaducts, lines,
  `classify_sides`; `no_marks`; the branch's mouth.
- The scenery kits and their `plan()` registrations of flattens, carves
  and fence gaps.
- Zone fog, the sky's time-of-day keys, the sun azimuth and the moon.
- The simulation's reads of `Track` (`project`, `surface_y`, `ground_y`,
  the walls, `open_ground`, `speed_profile`) and the trace format.
- Which section the ring centres on (the car once the race is on, the start
  until then, an event's countdown round the car).

## 10. Open questions for the owner

Marked **Q**; each has a default the work proceeds on if unanswered.

- **Q1. `f32` or `f64` in the trait?** Default: `f64`, as Midnight reads
  (every `f32` store widens on read) and as `pk_math`'s `DVec3` is;
  snowboarding converts at the boundary and keeps `f32` inside.
- **Q2. `lat` sign and units.** Default: metres in the horizontal plane,
  positive to the right of the tangent (Track's `rx = -fz, rz = fx`;
  Course's `u` along `right`), measured before banking.
- **Q3. Where does `Route` live?** `pk_play`'s chase rig and `pk_render`'s
  viewer must not pull terrain, meshes and extrusion. Default: one crate
  `pk_route` with the trait, `RouteNetwork`, `Partition` and `Stretch` in
  its core, and `terrain`, `tiles`, `extrude` behind a default `world`
  feature; `pk_play` and `pk_render` depend on it with
  `default-features = false`, and `deps.toml` pins that. Alternative: a
  `pk_route_core` crate, cleaner for `check-deps`, one more crate to name.
- **Q4. How the viewer's zones are expressed.** Default: `ViewerRoute::marks()
  -> Vec<Mark { s0, s1, name }>`, named stretches and nothing else; the
  panel's "2.31 km  Coast" and the overview's boundaries need no more.
  Colours and the sub-name stay Midnight's panel.
- **Q5. Is `project_network`'s shoulder rule the kit's default?** Default:
  yes, with `Shoulder` from the same closure `Terrain::new` takes, so a
  game writes the shoulder once. Alternative: nearest road by `locate`,
  which a car straddling a junction would flicker on.
- **Q6. Does `Site` carry `cover` as `u8` or as a kit enum?** Default:
  `u8`, with `pk_atlas::Cover` the one numbering both the atlas tools and
  the planting agree on; a game without the atlas passes its own bytes.
- **Q7. Section length and radii as `Stretch` fields or as `LodPolicy`-like
  data on the streamer?** Default: fields of `Stretch { section_len: 1000.0,
  radius: 700.0 }`, since they are properties of the partition, not of the
  client's ring.
