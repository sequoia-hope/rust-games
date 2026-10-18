# Inventory: what in Midnight Playground is generic

Audited 2026-10-16 from `~/Software/midnight-playground` at commit 5fee62a
(455 commits; 14 `mp_*` crates; about 250k lines of Rust, of which 56k
are the Bevy client `mp_game`). Snowboarding (`~/Software/snowboarding`,
no commits yet) was read for comparison: it is a hand-edited fork of
Midnight's skeleton and its copies are already drifting.

Verdicts: **GENERIC** lifts as-is; **SEAM** lifts once a named game type
becomes a trait, generic parameter or config; **GAME** stays with the
racing game. Line counts are source only unless noted.

## 1. Engine-free crates

| Crate | Lines | Generic (est.) | Verdict | Seam |
|---|---:|---:|---|---|
| mp_math | 609 | 540 | GENERIC | `js.rs` (JS parity semantics) becomes a feature |
| mp_track | 1,987 | 1,200 | SEAM | `Route` trait; split `Level` from racing fields (zones, police, rivals, events) |
| mp_levels | 3,426 | 194 | GAME | only `ground.rs` grid loaders (MPGROUND/MPCOVER) lift, to the atlas |
| mp_sim | 10,633 | ~600 | GAME | `Simulation` trait (step, hash, input); `RngStreams`, `fnv1a64`, replay and fuzz harness lift as patterns |
| mp_vdyn | 2,155 | 1,700 wheeled / 700 any | SEAM | car-only (two wheels per axle, no camber); bikes/boats reuse RigidBody, Ground, math |
| mp_net | 3,999 | 1,843 now, ~3,000 after trait | SEAM | transport/SimNet/rtc/signal are generic; client (rollback ring), host and proto need the `Simulation` trait and an opaque lobby payload |
| mp_host | 1,136 | 800 | mixed | signal relay GENERIC; static server needs URL layout as config; `port()` (the proj rule) GENERIC |
| tools/signal-worker | 189 JS | 189 | GENERIC | none; any matchbox game can use it |
| mp_atlas + tools/atlas | 1,983 + 739 py | ~2,500 | GENERIC | bbox, origin, Overture release become arguments |
| mp_scene | ~1,650 | ~1,250 | SEAM | `MaterialKind` closed enum (38 variants, ~30 Midnight-only) becomes an open tag; drop `cache.rs` |
| mp_canvas | ~4,900 | ~4,900 | GENERIC | runtime `FontBook` instead of build.rs font baking |
| mp_worldgen | 77,326 (+17k tests) | 10.6k + 9.1k with seam | 75% GAME | per-level kits 48k and car models 8.5k are content; terrain/road/sky/sea/sections/land/life need a `Route` seam |
| mp_builder | 180 | 180 | SEAM | planned-level type |
| mp_exhaust | 1,800 | 1,800 | GENERIC | add a cycle-degrees param for two-strokes; non-car presets |
| mp_music | 16,000 | ~15,900 | GENERIC | `STATIONS`/`level_station` are data; `energy` is the only game hook |
| mp_audio | 17,600 (+3.8k tests) | ~8,000 | SEAM | exhaust and radio nodes are hard-coded in the facade's `NodeKind`; buses/Gate/unlock fused into car-specific `GameAudio` |

Details worth keeping in mind:

- **mp_sim is cars on a track.** Rivals and traffic live in (s, lat)
  coordinates; there is no rigid body or character controller outside
  mp_vdyn. The three seams SPEC 4.5 named (Ground, VehicleModel, more than
  one road) are all built.
- **mp_net's rollback is not ggrs.** An authoritative host relays final
  inputs, AI takes over a dropped player, star topology over matchbox's
  mesh, signalling payloads sealed with XChaCha20-Poly1305. There is no
  snapshot message: a desynced client resimulates from tick 0.
- **The atlas pipeline** uses Terrarium elevation tiles on AWS (3DEP,
  SRTM, bathymetry), ESA WorldCover land cover via Overture, and
  Overture/OSM roads; equirectangular about an origin so the Rust side
  needs no trig. Nothing peninsula-specific is in the crate.
- **mp_worldgen's terrain** is not a plain height field: height is
  `lerp(landform, road height, k)` by distance to road, with flattens and
  carves; tile LOD is static by distance to road. Streaming partitions
  the level into stretches of route `s`, which fits snowboard, river and
  bike courses but not a free-roam world (that needs a cell partition).
- **mp_music's composer** is genre-neutral theory (scales, progressions,
  Euclidean rhythm, motifs, déjà vu variation, form); the nine grammars
  are vocabulary on top. Instruments are trigger-with-duration only (no
  note_on/off), untyped patches, per-sample f64 stereo.
- **mp_audio's facade** uses the browser's own nodes on the web and the
  `web-audio-api` crate (cpal) natively, so no wasm DSP, threads or
  COOP/COEP headers are needed; custom DSP (exhaust, music) is a separate
  small cdylib wasm in an AudioWorklet. That is a path nobody else in the
  ecosystem offers on stable Rust.

## 2. The Bevy client, `mp_game` (55,889 lines)

### Shell and platform

| Module | Lines | Verdict | Seam |
|---|---:|---|---|
| plugins.rs | 87 | GENERIC | none (snowboarding copied it byte for byte) |
| bridge.rs | 251 | GENERIC | the `__mp` global's name |
| options.rs | 409 | SEAM | ordered `Params` + an alias table; the game keeps its `Options` |
| status.rs | 214 | SEAM | readiness blockers the game registers (GPU fence, pipeline count, late frames) |
| web.rs | 263 | split | global name, gesture registry; scene hooks stay |
| native.rs | 438 | SEAM | ready signal, `FocusLost` message, banner; smoke test and `--screenshot` are generic |
| native/window_state.rs | 547 | SEAM | `WindowConfig{app_id, title, icon, sizes, persist}` |
| warmup.rs | 298 | SEAM | generic over `Material` with a key fn; glyph sizes as input |
| debug_overlay.rs | 900 | SEAM | `DebugLines` resource the game fills; anchor |
| recording.rs | 998 | SEAM (60%) | `RecordLine` message; `InputLog<I>` for the deterministic input log |
| worker.rs + builder-worker.js | 228 | SEAM | script URL, payload, wasm name |
| stream.rs | 906 | GAME, concept generic | `SectionStreamer` trait: plan, section_under(focus), build (Send), spawn |
| stations.rs | 124 | SEAM | pose callback, settled predicate |
| loader.rs | 969 | GAME | `AppState`, the 60 ms build budget and `SceneEntity` marker lift |
| lib.rs | 610 | GAME | `Inbox`, `Opts` patterns |
| fly.rs, cars.rs, levels/ | 740 | GAME | |

### UI

| Module | Lines | Verdict | Seam |
|---|---:|---|---|
| ui/widgets.rs | 1,085 | SEAM (~850) | `Control<A>`, `Theme`, fonts as input; logo is branding |
| ui/mod.rs | 2,706 | ~700 SEAM | action type `A`, screen id `S`, `build_screen` callback; pointer hit-test, scroll with slop, Tab focus and reveal are generic |
| ui/nav.rs | 638 | SEAM | control-kind classifier, default ids, `Nav` enum; spatial mode here, list mode in snowboarding: offer both |
| ui/store.rs | 858 | 345 SEAM | prefix, app folder, env var, legacy dirs; `Backend` trait, JS-exact stringify, atomic FileStore, private-mode-safe LocalStorage |
| ui/screens.rs | 731 | ~150 SEAM | `Cx`, `screen()` column with breakpoints |
| ui/settings.rs | 314 | ~120 SEAM | row helpers |
| ui/pad_setup.rs | 397 | SEAM | needs the gamepad layer lifted first |
| ui/web.rs | 790 | ~470 SEAM | `FullscreenOnTap`/`PermissionOnTap` markers, `RenderScale` policy, stage-command registry; the pixel-ratio system (D1194/D1195/D1199) is the key part |
| ui/menu.rs, ui/lobby.rs | 1,674 | GAME | |

### Play, input, HUD, net

| Module | Lines (code) | Verdict | Seam |
|---|---:|---|---|
| play/session.rs | 172 | SEAM | `Session<S: Simulation>` |
| play/flow.rs | 859 | GAME | HUD timers, fmt_time, ordinal, `Mode`, rumble outbox lift |
| play/mod.rs | 1,111 | GAME | event plumbing pattern |
| play/input.rs | 310 | SEAM | `Action` type param, keymap config, digital axis pairs for the ramp |
| play/gamepad.rs | 932 | SEAM | `Action` trait; axis pairs; bindings, capture, mute set, `Nav`, `Rumble` trait are generic |
| play/gamepad_io.rs | 652 | SEAM | rumble/hush as messages; `PadMapStore` trait; web Gamepad API and gilrs readers |
| play/touch.rs + touch_ui.rs | 1,496 | SEAM | declarative `TouchLayout<A>`; stick, slider bands, tap buttons, slop, insets are generic |
| play/tilt.rs | 665 | GENERIC core (~450) | `TiltSettings` for the plugin; iOS permission-in-gesture dance |
| play/camera.rs | 365 | SEAM | `ChaseTarget{pos,yaw,vel,boost}` + floor closure; modes already data |
| play/pose.rs | 157 | split | `Springs` generic; `root()` GAME |
| play/incar.rs | 169 | SEAM | geometry over mp_scene |
| play/effects.rs | 532 | split | `Particles`, `SkidMarks`, `rate60` (~220) generic |
| play/fx.rs | 946 | split | particle/trail drawer SEAM on material + `MeshWrites` |
| play/hud.rs + hud/model.rs | 3,307 | GAME | easing, change-only setters, timers (~250) lift |
| hud/dials.rs | 196 | SEAM | gauge config |
| hud/material.rs + hud.wgsl | 295 + 330 | SEAM | SDF UiMaterial with persistent per-slot uniform buffers (D865) |
| hud/minimap.rs | 270 | SEAM | vector minimap: Xform, polyline, Dot/Bar shapes are generic |
| play/audio.rs | 1,631 | GAME | gesture-to-wake pattern lifts with the audio crate |
| play/station.rs | 301 | SEAM (70%) | DJ director: rules as data, `Scene` quiet predicate, sink trait |
| play/radio.rs, radio_widget.rs | 531 | GAME | line gate generic |
| play/nametags.rs | 103 | SEAM | `WorldLabel` component |
| play/beacons.rs | 275 | SEAM | mesh builder + colour |
| play/guide.rs | 485 | SEAM | path sampler + urgency closure |
| play/models.rs, police.rs, web.rs, fx_stage.rs, pv_stage.rs | 3,400 | GAME | |
| net/mod.rs | 389 | SEAM | once mp_net is generic; `NetView`/`NetCmd` split is a good pattern |
| net/link.rs, tab.rs | 192 | SEAM | URL config, `Levels` |
| net/ws_native.rs, ws_web.rs | 282 | GENERIC | |

### Rendering, scene, viewer

| Module | Lines | Verdict | Seam |
|---|---:|---|---|
| render/material.rs + three_material.wgsl | 1,129 + 1,860 | SEAM | open patch registry instead of `Patch`/`MaterialKind`; ~30 of the patches are Midnight content |
| three_std.wgsl | 304 | GENERIC | three.js BRDF library |
| render/lighting.rs | 561 | SEAM | `Anim` and `headlight` become a user block; the shared "globals" uniform (D1191) is the valuable part |
| render/instancing.rs | 391 | SEAM | configurable instance layout; per-material plugin instead of replacing Bevy's `DrawMesh` globally |
| render/sky.rs + shaders | 388 + 214 | SEAM | keys indexed by any scalar; zone-fog hook |
| render/pmrem.rs | 487 + 145 | SEAM, parity-only | Bevy's generated environment map serves new games |
| render/post.rs | 400 + 107 | GENERIC, parity-only | Bevy Bloom + ACES serve new games |
| render/sort.rs | 121 | GENERIC | renderOrder sort for Transparent3d |
| render/frame.rs | 96 | GENERIC, upstream | skip the empty screenshot submit per frame (D862) |
| convert.rs | 878 | SEAM | attribute table from the registry |
| animate.rs | 1,856 | ~1,400 SEAM | move `SceneEdit`/`Change`/`Animator` protocol to the toolkit; `UpdateCtx<G>`; animator registry instead of a closed enum |
| matscene.rs, preview/ | 2,934 | GAME | |
| viewer/ | 3,685 | SEAM | `ViewerRoute` trait (length, is_loop, frame(s), ground_at, nearest_s, zones); needs the UI kit and pad layer |

Snowboarding renders with Bevy `StandardMaterial`, `DistanceFog`, cascaded
shadows and an unlit vertex-coloured dome, with no custom WGSL. The
three.js shading exists for pixel parity with the JS game, so it is an
optional layer, not the toolkit's base.

## 3. Tooling

| Item | Lines | Verdict | Config needed |
|---|---:|---|---|
| xtask/web.rs | 546 | SEAM (~300) | game crate, backends `{name, out, cfg, target_dir}`, page dir, dist dir, runtime files, side wasm builds, build.json extras |
| xtask/size.rs, prof.rs | 195 | SEAM | budget, dist dir, backends |
| xtask/deps.rs | 415 | SEAM (~250) | a rules table (`deps.toml`) |
| xtask/shots.rs | 462 | SEAM | CIEDE2000 image metric + HTML report are generic |
| xtask kernel/stations/materials/parity_scene | 536 | GAME | JS-parity only |
| web/index.html, perf.js | 701 | ~60% generic | title, branding, status global, output names, load hook; snowboarding's copy adds a crash screen and a backends check |
| ui/web.rs gesture_at | — | SEAM | "do X inside the next gesture if it lands in rect R" |
| tools/serve.py | 180 | ~90 generic | dist dir, title; the favourites POST is game |
| serve.sh, signal.sh | 69 | SEAM | project name; the `web_extra` lookup belongs in `proj` |
| .github/workflows/ci.yml, pages.yml | 252 | SEAM | crate names, cfg, the lavapipe smoke test is valuable |
| patches/ (3) | — | upstream | see ECOSYSTEM.md section 3 |
| tools/parity/{lib/server,rust-perf,rust-prof,gl-calls,build-diff,web-console,e2e/harness}.mjs | ~1,280 | SEAM | scenario as parameter; drop the JS-parity modes |
| the rest of tools/parity, test/e2e rust bridge | ~60 files | GAME | parity with the JS game |
| assets/fonts/{subset,oblique}.py | 199 | SEAM | font list, character set; oblique.py waits on bevy#25569 |
| desktop/ (Electron), play.sh, og-image.mjs | — | drop | the native binary replaces Electron |
| launch.sh | 34 | machine-level | belongs in `~/scripts` |

Both games use a different status global (`__mp`, `__sb`), so every JS
tool needs a parameter until one name and field contract is agreed.
