# Rework plan: a shared toolkit carved from Midnight Playground

Status: **proposal, 2026-10-16.** The inventory behind it is
`INVENTORY.md`; what the ecosystem already offers is `ECOSYSTEM.md`;
what this means for the racing game's roadmap is `RECOMMENDATIONS.md`.
The library is **Playground Kit**; its crates are `pk_*` (the owner's
choice, 2026-10-16, DECISIONS D1).

## 1. Goal

One library, in its own repository, that Midnight Playground, Snowboarding
and the next games (bicycle, kayak, a Mario-64-style platformer,
skydiving, diving, basketball, a Zelda-like) build on. The games keep
their content and their rules; the library holds everything a second game
would otherwise copy. The split is judged by three gates:

1. **Midnight Playground is unchanged to the player.** Every test it has
   today still passes, `cargo xtask check-deps` still passes, and the
   web build is pixel-identical to the pre-split build on the four levels
   `build-diff.mjs` checks (`dist/pre-split`, built from f9ca805, as the
   before). Coast has a known noise floor: its sea glitter is animated by
   the sea clock, so one build against itself across two Chrome runs
   differs by about 70 to 180 scattered pixels at a channel difference of
   1; a diff inside that floor on coast is not a change (phase 1 measured
   pre-split against itself at 118 pixels, the carve at 73 to 128). A
   later tooling step should let the harness freeze the sea clock so the
   gate is exact on coast too. Frame time on the reference phones is
   within noise.
2. **Snowboarding deletes code.** Its options, status, web glue, xtask
   web build, loader page, serve.py and audio shrink to calls into the
   library, and it gains what it lacks: settings persistence, window
   state, pause on focus loss, pipeline warm-up, readiness, render scale,
   debug overlay, recording, smoke test, native sound.
3. **A new game starts from a template** in an afternoon: menu, settings,
   input on keyboard, pad, touch and tilt, a fixed-tick simulation with
   recording and replay, web and native builds, CI and Pages.

## 2. Principles carried over, and two new ones

From Midnight's SPEC and CLAUDE.md, unchanged:

- The simulation is a library: no engine, clock, threads or global state;
  one fixed tick; deterministic by construction, every inexact function
  through the math kernel; `check-deps` rejects the rest.
- World generation is a library: engine-neutral scene data out.
- One implementation for every platform; platform differences behind
  small traits.
- Only the client crates depend on Bevy; Bevy stays pinned (0.19.1) and
  moves between milestones; no third-party Bevy plugins without a
  decision record.
- Every URL relative; the port comes from `--port`, `$PORT`, `proj port`,
  then fail loudly.
- Decisions go in a `DECISIONS.md`; work is pushed straight to `main`.

New for the library:

- **Seams are traits or data, never game names.** A crate in the library
  may not mention a car, a road, a station or a level. Where the audits
  found one (`InputFrame`, `Track`, `SimState`, `MaterialKind`,
  `STATIONS`, `CarState`), the library gets a trait or an open data type
  and the game implements it.
- **Each crate answers "which second game uses this?"** before it is
  carved. The planned games are the test: a crate no second game needs
  stays in Midnight.

## 3. Repository and dependency layout

```
~/Software/game-engine/            the library (this project; port 8046)
  Cargo.toml                       workspace of pk_* crates + xtask lib
  crates/pk_*/
  patches/{wgpu,wgpu-hal,bevy_ui_render}/   moved here from both games,
                                   until the upstream PRs land
  template/                        a cargo-generate template for a new game
  js/                              the headless-Chrome tools
  tools/{atlas,voice,signal-worker}/
  skills/                          Claude Code skills (section 8)
  docs/                            this plan, DECISIONS.md, per-crate design notes
  index.html, serve.sh             the status page on the registered port

~/Software/midnight-playground/    depends on ../game-engine by path
~/Software/snowboarding/           depends on ../game-engine by path
```

**How the games depend on it.** Path dependencies to the sibling
checkout (`pk_shell = { path = "../game-engine/crates/pk_shell" }`), and
the games' `[patch.crates-io]` blocks point at
`../game-engine/patches/*`. CI and the Pages workflow check out the
library beside the game. This matches the push-to-main workflow: both
trees are always the working trees on this machine, nothing is pinned
twice, and Cargo.lock stays stable (path sources have no lock entry).
The alternatives, a git submodule or `git = ..., rev = ...` deps with a
user-level `[patch]` for local work, pin harder but churn the lockfile or
add a submodule step to every clone. The owner decides (question 2).

**Version policy.** The library pins Bevy exactly as the games do and
moves first; a game moves by bumping its own Bevy and the library in one
commit. Crates are `publish = false` until the API has survived two
games; then `det-math`, `exhaust-synth`, `vehicle-dynamics` and the
signalling worker are the first to publish (ECOSYSTEM.md section 2).

## 4. The crate map

Nineteen crates in four layers. Lines are what the audits estimate lifts
from Midnight; the first number is code that moves nearly as-is, the
second is code that moves once the seam exists.

### Layer 0: no engine, no browser (build for native and wasm, `forbid(unsafe)`)

| Crate | From | Lines | What it offers |
|---|---|---:|---|
| `pk_math` | mp_math, mp_vdyn::math | 540 + 260 | `kernel::{sin..hypot_n}` on libm with ECMAScript semantics; `Rng`, `Mulberry32`, `hash2`, `Noise2D`, `fbm`, `ridged`; `DVec3`/`DQuat` (glam with `libm`, exact ops only); `js` feature for `Math.round`/`±0`/`||` semantics |
| `pk_lockstep` | mp_sim patterns, mp_net needs | 600 | `trait Simulation { type Ctx; type State: Clone; type Input: Copy+Default+Eq; type Event; const DT; fn step(ctx, &mut State, &[Input], &mut Vec<Event>); fn hash(&State)->u64 }`; `trait NetSim: Simulation { encode/decode input; predict(last)->Input; absent()->Input; new_multi }`; `fnv1a64`; `RngStreams` (named, draw-counted); `Recording{seed, rle inputs, hash checkpoints}`; `assert_deterministic`, `fuzz` |
| `pk_canvas` | mp_canvas | 4,900 | Canvas 2D on tiny-skia with harfrust/skrifa text; `FontBook` at runtime; `bundled-fonts` feature |
| `pk_scene` | mp_scene, mp_worldgen::{world, anim} | 1,250 + 400 | `Scene` with an open `kind: String` on materials; `.mrscene` read/write/digest; `SceneRef`, `Change`, `SceneEdit`; `trait Animator<G>` and an `AnimatorRegistry` (tag, data) for worker transfer |
| `pk_atlas` | mp_atlas, mp_levels::ground, tools/atlas | 2,200 + 740 py | `Atlas::load`, `Terrain::height/cover`, `graph::{snap, path, resolve}`, `profile`, `corridor`, `plan.json`; `build.py --bbox --origin --release`, `ground.py` |
| `pk_route` | mp_track, mp_worldgen::{terrain, terrain_mesh, road, section, stream} | 1,200 + 4,400 | `trait Route { len, frame(s), project(x,z)->(s,lat), is_loop, wrap }`; `Track` (1 m samples, branches, networks) as the reference impl; `Terrain::new(route, landform, ground)` with flattens and carves; `build_tile_geometry(height_fn, tile)`; `extrude(route, profile)`; `Planned::section(k)`, ring, `pack`/`unpack`; a `SectionKey` generic so a cell partition can join later |
| `pk_scenery` | mp_worldgen::{object, material, color, geom, builder, flora, land, life, sky, sea, textures (shared)} | 2,800 + 3,300 | `SceneGraph` and bucket merge into a `Scene`; `Flora` templates with baked AO; `plant_by_cover`; `Life::new(anchor_provider)` birds and people; `SkyKeys<T>::sample`; `sea_mesh(bounds, depth_fn)` |
| `pk_vdyn` | mp_vdyn | 1,900 | `VehicleDef` (N axles), brush `Tyre`, `Drivetrain`, `RigidBody`, `trait Ground { height, sdf, colliders }`, telemetry; later a single-track (bicycle, motorcycle) body |
| `pk_net` | mp_net, mp_game::net::{ws_native, ws_web, link}, mp_host | 1,850 + 1,200 + 800 | `trait Transport`, `SimNet` (latency/jitter/loss), `RtcNet` (matchbox), WebSocket transports, `JoinLink`/`RoomKey` sealed signalling, `Star`; `Host<S: NetSim, T>` and `Client<S, T>` (rollback ring, hash check, resimulate) with an opaque lobby payload; `mp-signal` relay and the static server with `port()`; `tools/signal-worker` |
| `pk_wa` | mp_audio::{wa, session, timers, game (shell part), radio/dj banks} | 5,100 + 1,500 + 530 | the Web-Audio facade over `Backend` (web, native via web-audio-api, null recorder); `trait Processor` + one macro emitting the wasm C-ABI exports and the native worklet wrapper; `Mixer` (master/sfx/music/voice buses, compressor, limiter, `Gate`, unlock in gesture, pause, iOS session, `duck`); `NoiseBurst`, `PlayOpts`, `tone`, noise beds; `ClipBank` (index, prefetch, decode) |
| `pk_music` | mp_music | 15,900 | `synth` (dsp + instruments, gaining `note_on/off` and typed patches), `compose`, `genres`, `Engine` with `energy`, `radio` (schedule on the wall clock, `cue`, `Tuner`, `Player`) taking `&[Station]` as input; favourites/blocklist as data; `worklet` feature |
| `pk_exhaust` | mp_exhaust | 1,800 | `Engine::new(Params, State, seed, sr)`, presets as files, `cycle_deg` (720 or 360), `Response::{Lab, Game}` |
| `pk_voice` | play/station.rs, tools/{dj-voice, radio-voice} | 300 + 1,200 py/html | `Director { gap, chance, history_len, quiet: fn(&Scene)->bool, rules }` and `LineGate`; the Python pipeline `design`, `render`, `verify` (Whisper, new), `audition`; `cast.json` + `script.json` schema with post-processing profiles (full, radio, phone, PA) and template slots |

### Layer 1: Bevy, no game

| Crate | From | Lines | What it offers |
|---|---|---:|---|
| `pk_shell` | plugins, options, status, bridge, native, window_state, warmup, debug_overlay, recording, worker, stations, web.rs (generic half), ui/store, ui/web (pixel ratio, gestures) | 3,000 + 3,500 | `ClientPlugins`; `Params` (query/args, aliases); `Inbox<T>`; `AppInfo`; `LoadState` + `ReadyBlockers` (GPU fence, pipeline count, late frames); `WarmUp<M: Material>`, `prewarm_glyphs`, `prewarm_ui`; `Store` (Memory, LocalStorage, FileStore atomic, `StoreConfig{prefix, app_dir, env_var, legacy_dirs}`); `WindowStatePlugin(WindowConfig)` with F11/Ctrl+Q/icon; `FocusLost` from web visibility and native focus; `RenderScale`, `set_pixel_ratio`, `SplitScreenCamera` (D1199); `FullscreenOnTap`, `PermissionOnTap`, insets, clipboard; `Bridge::global("__game")` lazy value tree, `screenshot_hook`, `register_stage`; `SmokeTest`, `--screenshot`, `log_tally`, `Stations`; `DebugOverlayPlugin` + `DebugLines`; `RecorderPlugin` + `RecordLine` + `InputLog<I>`; `Worker` executor |
| `pk_ui` | ui/{widgets, mod (generic 700), nav, screens, settings rows, pad_setup} | 2,500 | `Theme`, `Bp` breakpoints, `T` text runs with fallback faces, `Control<A>`, button/checkbox/slider/select/dropdown/focus ring, `Cx` + `screen()`, `ScreenHost<S, A>` (rebuild on dirty, hit test with slop, scroll, Tab focus, reveal), `MenuNav` in list and spatial modes, `PadSetupView<A>` |
| `pk_input` | play/{input, gamepad, gamepad_io, touch, touch_ui, tilt} | 3,900 | `trait Action`; `ActionState<A>` (axis, held, consume); `KeyMap<A>`, `DigitalRamp`, analog curve; `Pads<A>` with `Binding`, per-pad-id `PadMap<A>` JSON, capture, mute set, `Nav`; `trait Rumble` with web and gilrs backends, `RumbleKick/Feel/Hush` messages, `trait PadMapStore`; `TouchLayout<A>` (stick, slider bands, tap buttons, insets, pointer map) and its Bevy UI renderer; `TiltSteer`, `TiltState`, the web DeviceOrientation module with permission-in-gesture |
| `pk_play` | play/{session, camera, pose::Springs, effects, fx (drawer), hud/{dials, material, minimap}, flow helpers, nametags, beacons, guide}, stream.rs ring | 4,000 | `Session<S: Simulation>` (accumulator, max ticks, prev/curr, alpha, per-tick sampler, online mode); `ChaseRig` with modes as data, `Springs`; `Particles`, `TrailRing`, `rate60` and a quad drawer; the SDF `UiMaterial` with persistent per-slot uniforms, `Gauge`, `Minimap` (vector), `SpeedLines`, `StarMeter`, `HudTimers`, CSS easing, change-only setters, `fmt_time`, `ordinal`; `WorldLabel`, `BeaconMesh`, `PathChevrons`; `SectionStreamer` trait + `StreamRing` + thread/worker/stepped executors |
| `pk_render` | render/{lighting (globals), instancing, sky, sort, frame, mod}, convert, animate (generic half), viewer; `three` feature: material.rs + WGSL, pmrem, post | 4,500 + 5,300 (three) | `GlobalsPlugin` (shared uniform + `AnimBlocks`); `InstancedMeshPlugin<M>` with `InstanceStream` partial writes and an instanced shadow prepass; `SkyDomePlugin`; `RenderOrder` sort; the empty-submit skip; `trait SceneMaterials` + `spawn_scene` + `apply_edits`; `ViewerPlugin<R: ViewerRoute>` with free/orbit/overview/ride cameras, panel, links, stats; `three` feature: `three_std` + `ThreeMaterial` with an open patch registry, UnrealBloom + ACES, PMREM |
| `pk_audio` | play/audio.rs glue | 200 | the `NonSend` holder, gesture wake, `FocusLost` pause, `?audiolog=1` null backend, the test bridge node |

### Layer 2: tooling

| Crate / dir | From | What it offers |
|---|---|---|
| `pk_xtask` (lib) | xtask/{web, size, prof, deps, shots metric} | `WebBuild{crate, backends, page_dir, dist_dir, runtime_files, side_wasm, build_json_extra}`, `check_bindgen_version`, `check_wasm_opt_version` (new), `precompress`, `swap_in`; `size --budget` with job summary; `prof` names-kept copy; `deps` rules engine reading `deps.toml`; `imgdiff` CIEDE2000 + block metric + HTML report |
| `template/` | both games | `xtask/main.rs` (40 lines of calls), `web/index.html` (branding, `__game`, load hook; merges snowboarding's crash screen and backends check), `tools/serve.py`, `serve.sh`, CI (fmt, clippy ×3, tests, lavapipe smoke, wasm tests, web, size), Pages, `rust-toolchain.toml`, `.cargo/config.toml`, profiles + `[patch]` block, `.gitignore`, `CLAUDE.md` with the port rule and the DECISIONS convention, `DECISIONS.md` |
| `js/` | tools/parity (generic set), snowboarding tools | `server.mjs` (`baseUrl`, Chrome flags, `CHROME_PATH`), `harness.mjs` (interception, device presets, `Game` wait/frames/tap/shot), `perf.mjs`, `prof.mjs` (CPU by Bevy system), `gl-calls.mjs`, `build-diff.mjs`, `shot.mjs`, `console.mjs`, `stress.mjs` runner, `png.mjs`, page-side `perf.js` |
| `patches/` | both games | the three patches, each with `PATCHED.md` and the upstream issue/PR it waits on |

### Layer 3: what stays in Midnight Playground

`mp_sim` (cars, AI, traffic, pursuit, events, `SimState`, `InputFrame`),
`mp_levels`, the level kits and car models of `mp_worldgen`, `mp_game`'s
menu, lobby, HUD layout, flow, police, models, audio driver, radio
widget, parity scenes, and all of `tools/parity`'s JS-oracle tooling.
`mp_audio::game` keeps the car content (engines, tyres, sirens, police
lines) on top of `pk_wa::Mixer`. Midnight's `xtask` becomes the template's
40 lines plus its parity subcommands.

## 5. The seams, in order of consequence

These are the traits the whole plan hangs on. Each gets a short design
note in `docs/design/` before its phase starts, and a DECISIONS entry.

1. **`Simulation` / `NetSim`** (`pk_lockstep`). Sits below `pk_net` and
   `pk_play::Session`. Midnight implements it with `SimState`/`InputFrame`
   unchanged; the `RESET`/`AUTOPILOT`/`AWAY` flag semantics move into
   `NetSim::{predict, absent}` hooks. Snowboarding's `Session` fits after
   three changes: step over `&[Input]` into an out-vec, `Course` split
   into an immutable `Ctx`, a `hash()`.
2. **`Action`** (`pk_input`). The game's enum implements it; devices
   merge into `ActionState<A>`; the game's `quantise(&ActionState<A>) ->
   S::Input` closes the loop. Assists and autopilot sit in the per-tick
   sampler between the two.
3. **`Route`** (`pk_route`). `Track` implements it; terrain sculpting,
   extrusion, sections, the viewer and the chase camera's floor all take
   `&impl Route`. `Level` splits into the route (library) and the racing
   fields (Midnight).
4. **Open material kinds and the patch registry** (`pk_scene`,
   `pk_render`). `kind: String` plus a registry of shader-define blocks
   replaces two closed enums; Midnight registers its ~30 kinds.
5. **`Processor`** (`pk_wa`). One registered custom-node type replaces the
   hard-coded exhaust and radio nodes; the same macro emits the wasm
   exports and the native worklet.
6. **`SectionStreamer<K>`** (`pk_play`). Keyed by route stretch now;
   `K` leaves room for a cell key.
7. **`__game` status contract**. One global name and field set (`state`,
   `frames`, `ready`, `backend`, `error`, `shot`, `wasmMemoryBytes`,
   `stage`) for both games and every JS tool.

## 6. Phases

Each phase ends with Midnight Playground green on its own gates (tests,
clippy, `check-deps`, `cargo xtask web --release`, the e2e suites on the
GPU machine, `build-diff.mjs` against the pre-split build) and pushed.
Phases are sized for one long agent session or two; the order puts the
already-forked and the highest-value-for-snowboarding pieces first.

| Phase | Carve | Midnight rework | Gate |
|---|---|---|---|
| **0. Bootstrap** | `git init`, LICENSE, workspace, CI, `DECISIONS.md`, move `patches/`, draft the three upstream PRs/comments | `[patch.crates-io]` paths point at the library | both games build with the moved patches |
| **1. Foundation** (this session) | `pk_xtask` + `template/` + `js/`; `pk_math`; `pk_lockstep` (trait + helpers, no consumers yet beyond tests); `pk_canvas`; `pk_shell` | `mp_math` becomes a re-export shim then is deleted; `mp_canvas` likewise; `mp_game` drops plugins/options parsing/status/bridge/native/window_state/warmup/debug_overlay/recording/store/pixel-ratio in favour of `pk_shell`; `xtask` shrinks to the template's main plus parity commands; `web/index.html` built from the template; `__mp` becomes `__game` with an `__mp` alias for the JS suites | gate as above; wasm size within 1% |
| **2. Input, UI, play** | `pk_input`, `pk_ui`, `pk_play` (session, camera, fx, hud primitives, labels/beacons/chevrons, stream ring) | `CarAction` implements `Action`; `mp_sim` implements `Simulation`; menus on `ScreenHost`; HUD dials/minimap on `pk_play::hud`; `stream.rs` becomes a `SectionStreamer` impl | gate; a pad remap round-trips; touch e2e suites pass |
| **3. Net** | `pk_net` generic over `NetSim`; `pk_net::host` and the Worker | `mp_net` client/host/proto become the `NetSim` impl plus Midnight's lobby payload; `mp_host` main shrinks | eight headless clients for an hour (MULTIPLAYER gate) still pass; protocol version bump recorded |
| **4. Audio** | `pk_wa` (facade, processor registry, `Mixer`, `ClipBank`), `pk_music`, `pk_exhaust`, `pk_voice` + `tools/voice` | `mp_audio::game` on `Mixer`; exhaust and radio as registered processors; the DJ director on `pk_voice::Director`; `tools/dj-voice` moves, `audio/dj` stays | audio goldens (`parity/golden/audio`, `music`) unchanged; the Radio page works |
| **5. World and render** | `pk_scene` open kinds, `pk_render` (globals, instancing, sky, sort, scene spawn/animate, viewer; `three` feature), `pk_route`, `pk_scenery`, `pk_atlas`, `pk_vdyn` | `MaterialKind` registered as strings; `Track` implements `Route`; `Level` split; level kits use `pk_scenery`; the viewer on `ViewerPlugin<Track>` | L3/L4 digests and stations unchanged; `build-diff` identical |
| **6. Snowboarding adoption** | hand-off document for its agent (section 7) | — | snowboarding deletes its copies; gains the missing pieces |
| **7. Template and skills** | `cargo generate` a throwaway game and play it; the skills in section 8 | — | a new game in an afternoon |

Phase 1 is the one with no design risk: every module it moves was judged
GENERIC or needs only config. Phases 2 to 5 each open with a design note
for their seam and a short consult.

### Phase 1 in detail (this session)

1. Bootstrap (phase 0) with the owner's answers to section 9.
2. `pk_xtask`: lift `web.rs`, `size.rs`, `prof.rs`, `deps.rs` (rules from
   `deps.toml`), `shots.rs`'s metric; add the wasm-opt version check.
   Midnight's `xtask` calls it; snowboarding's can later.
3. `template/` and `js/` from the audit's lists, with the `__game`
   contract; Midnight's `web/index.html` regenerated from the template
   with its branding block.
4. `pk_math`: `mp_math` moved, `js` behind a feature, `DVec3` added;
   `mp_math` becomes `pub use pk_math::*` for one commit, then every
   `use mp_math` is rewritten and the crate deleted. The kernel bit test
   moves to `pk_xtask`.
5. `pk_canvas`: moved; `FontBook` at runtime; Midnight passes its
   `fonts.json` set.
6. `pk_lockstep`: the trait and helpers written fresh from the audit's
   sketch, with `mp_sim`'s `fnv1a64`, `RngStreams` and replay format moved
   under it; `mp_sim` implements `Simulation` only (its `step` wrapper),
   no consumer changes yet.
7. `pk_shell`: module by module in the audit's order (plugins, bridge,
   Params, status, store, window state, FocusLost, pixel ratio and render
   scale, gestures, warm-up, debug overlay, recorder, smoke test,
   stations, worker executor). Each module: lift, add the seam, swap
   Midnight onto it, run the gate.
8. Record each move in both DECISIONS logs; push both repos.

## 7. Snowboarding: hand-off, not a touch

Its agent is mid-task (a headless test harness and crash fixes;
uncommitted tree). This plan does not modify that repository. At phase 6,
`docs/handoff-snowboarding.md` tells its agent what to adopt and in what
order: `pk_xtask` and the template (its copies are already behind
Midnight's), `pk_shell` (store, window state, focus pause, warm-up,
readiness, render scale, debug overlay, recording, smoke test),
`pk_wa::Mixer` (its 203-line web-only audio becomes ~60 lines and gains
native sound), `pk_input` (its flat `Controls` becomes an `Action` enum),
`pk_play::Session` (three sim changes), `pk_math` (its `noise.rs` calls
platform trig eight times, so its "same on every platform" claim does not
hold today).

## 8. Skills

Claude Code skills, kept in the library under `skills/` and installed by
symlink into `~/.claude/skills/` (or as a plugin). Each is a procedure
with the commands, the files to read first, the gates, and the common
mistakes, written from what the audits found:

| Skill | Covers |
|---|---|
| `voice-cast` | spoken-word character building: the character sheet, three voice candidates, `design`, `render`, Whisper `verify`, the audition page, 👍/👎 to rewrites, the director's rules, template slots pre-rendered per combination, post-processing profiles (radio band, phone, PA) |
| `world-making` | physical world from real geography: bbox and origin, `build.py`, `ground.py`, `plan.json` places/regions/routes, `report`, `corridor`, planting by land cover, events at places, the viewer as the shared table |
| `music-genre` | adding a genre grammar on the composer: the vocabulary (kit, progressions, motif cells, form), goldens, favourites and rejects, a station's weights |
| `engine-sound` | an exhaust preset: cylinders, firing order, headers, muffler sections, stock versus aftermarket pairs, the lab, `Response::Game` |
| `lockstep-sim` | writing a `Simulation`: the determinism rules, `deps.toml`, `RngStreams`, events not side effects, recording and replay, fuzz, the hash |
| `client-shell` | starting or wiring a game: the template, `Params`, `Store`, `Action` enum, `TouchLayout`, screens on `ScreenHost`, `__game`, the port rule |
| `web-perf` | `xtask web/size/prof`, `perf.mjs`, `prof.mjs` by Bevy system, `gl-calls.mjs`, `build-diff.mjs`, phone emulation and throttling, reading the debug overlay, the budgets |
| `level-viewer` | god mode: cameras, links that carry a pose, groups, stats, annotating a draft with the owner |
| `decisions-log` | the `DECISIONS.md` and `DEVIATIONS.md` convention, push-to-main, the commit message shape |

## 9. Questions for the owner (asked before execution)

1. The library's name and crate prefix.
2. How the games depend on it: sibling path (recommended), git
   submodule, or git deps by rev.
3. Licence: `MIT OR Apache-2.0` (the Bevy ecosystem's norm; needed
   anyway to publish and to contribute the patches) or something else;
   and whether Midnight Playground itself gets the same licence (it has
   no LICENSE file today, though WORLD.md calls it open source).
4. Whether to keep the policy of own code over third-party Bevy plugins
   for input, widgets and persistence (ECOSYSTEM.md rows 1 to 5), or to
   adopt bevy_enhanced_input, feathers and bevy_pkv where the audit says
   they fit.
5. Whether to run phases 2 to 5 autonomously after phase 1, pausing only
   at each seam's design note, or to stop for review after every phase.
