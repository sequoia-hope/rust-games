# Ecosystem: where Midnight Playground duplicates open source, and where it fills a gap

Checked 2026-10-16. Bevy 0.19 shipped 2026-06-19, 0.19.1 on 2026-08-13,
0.20 on 2026-10-08; most crates are on 0.19 with 0.20 ports landing this
month. Feature notes on third-party tools in section 4 are from the
research agent's reading of READMEs, not from running them.

## 1. Where you are duplicating existing work

Ordered by how clearly a maintained crate covers the need. "Use" means
adopt it; "wrap" means adopt it under your own thin API; "keep" means your
own is justified, with the reason.

| # | Your code | Covered by | Call |
|---|---|---|---|
| 1 | `ui/store.rs` web localStorage + native file | bevy_pkv 0.16 (redb native, localStorage wasm), bevy-persistent | **Keep, small.** Yours is 345 lines, has JS-exact number formatting (the saved records of the JS game read back) and atomic writes; pkv would force a key migration for no gain. Mirror its API shape. |
| 2 | `ui/nav.rs` gamepad menu navigation | bevy_input_focus directional navigation (official, 0.19) | **Wrap.** Keep your spatial scorer and repeat timing until bevy #21260 (gamepad activation of widgets) lands; expose the same `NavigationInput` shape so swapping is cheap. |
| 3 | `ui/widgets.rs` buttons, sliders, checkboxes, dropdowns | bevy_ui_widgets + bevy_feathers (official; 0.20 adds list, dropdown, tabs, theme context) | **Keep for now, plan to migrate.** Feathers was experimental in 0.19 and has no gamepad activation; your widgets are proven on phones under WebGL2. Build the toolkit's `Control<A>` on the headless `bevy_ui_widgets` traits where they exist, so the visuals can later become feathers. |
| 4 | `play/input.rs`, `gamepad.rs` action binding and remapping | bevy_enhanced_input 0.26/0.27, leafwing-input-manager 0.21 | **Keep, deliberately.** SPEC 2's policy is no third-party Bevy plugins without a decision record, because each is a reason an upgrade stalls. Your layer is ~1,900 lines, quantises for lockstep, reads the browser Gamepad API directly (D780, which neither crate does on the web with rumble) and persists per-pad-id maps. Document the decision; borrow enhanced_input's "context" idea for menu versus play. |
| 5 | `play/touch.rs` virtual stick | virtual_joystick 2.8 | **Keep.** Yours has the slider bands, slop, insets and multi-touch pointer map in one place and is tuned on the owner's phones. |
| 6 | `mp_net` client rollback | bevy_ggrs 0.22 + ggrs 0.13 | **Keep: a different model.** ggrs is symmetric P2P with no authority; yours is a host-authoritative star with AI takeover of dropped players and sealed signalling. A new P2P game would be better served by ggrs. The toolkit should offer the `Simulation` trait in a way that an adapter to ggrs is possible later. |
| 7 | `mp_host` signal relay | matchbox_server (Rust binary) | **Keep the Cloudflare Worker, drop nothing yet.** `mp-signal` duplicates matchbox_server except for no logging and sharing the static host's port; the Worker relay has no equivalent anywhere (section 2). |
| 8 | `play/session.rs` prev/curr interpolation | bevy_transform_interpolation 0.5 (also bundled in avian) | **Keep.** Yours interpolates a whole sim state, not transforms; it is 170 lines. |
| 9 | `render/sky.rs` + PMREM | bevy_pbr `Atmosphere` (0.16+, HDR cameras, WebGPU) | **Keep for WebGL2; let new games use Bevy's.** Bevy's atmosphere needs compute and HDR, which the WebGL2 build lacks. Make the sky a time-keyed data model with two renderers. |
| 10 | three.js shading, UnrealBloom, ACES post, PMREM | Bevy `StandardMaterial`, `Bloom`, `Tonemapping::AcesFitted`, generated environment maps | **Optional layer.** Parity-only. Snowboarding already uses Bevy's. Ship as a `three` feature for games that want the look; measure StandardMaterial on the reference phones before relying on it. |
| 11 | `debug_overlay.rs` frame-time overlay | bevy_dev_tools `FpsOverlayPlugin`, bevy_remote (BRP) | **Keep.** The maintained alternatives (iyes_perf_ui, bevy_screen_diagnostics) are dead; yours has p99 over three windows, render-world timing, pipelines waiting and wasm memory, which none provide. Add BRP for remote inspection; `bevy_brp_mcp` lets Claude drive the running game. |
| 12 | `worldgen/three_geom` (three.js geometry generators, 6.4k lines) | bevy primitives, bevy_math curves, earcutr, lyon | **Parity-only.** Not in the toolkit's base. |
| 13 | `mp_vdyn::math` f64 Vec3/Quat | glam `DVec3`/`DQuat` with the `libm` feature | **Replace** in the toolkit's `det-math`, keeping the exact-op discipline (glam does not promise cross-platform determinism; check SIMD is off). |
| 14 | `mp_math` noise | noise, fastnoise-lite | **Keep.** Neither promises bit-identical results across platforms. |
| 15 | xtask web build, loader page | bevy_cli (`bevy build web`, alpha, Bevy 0.18), trunk, bevy_new_2d/bevy_github_ci_template | **Keep.** None does two backends chosen by the page, precompression, the staging swap, size budgets or the registry port rule. Revisit when bevy_cli reaches 0.19/0.20. |
| 16 | `recording.rs`, `--smoke-test`, `--screenshot` | bevy_dev_tools `ci_testing` (RON: screenshot at frame N, exit) | **Keep.** Yours records input logs for replay and a tracing tally; add a `ci_testing`-compatible mode if cheap. |
| 17 | `mp_audio` Web-Audio facade | bevy_seedling + Firewheel (Bevy's future engine; `firewheel-web-audio` needs nightly, atomics, COOP/COEP), bevy_kira_audio 0.26 | **Keep, and watch Firewheel.** Yours runs on stable with no threads by using the browser's nodes; that is the differentiator. Re-check Firewheel's web backend each Bevy release; design the processor registry so an exhaust/music node could also be a Firewheel node. |
| 18 | `mp_music` DSP blocks (ladder, SVF, FDN reverb, delays, pluck) | fundsp 0.23, mi-plaits-dsp-rs | **Keep the voices, consider the blocks.** The tuned 303/Juno/FM/drum voices and the composer exist nowhere else. Plaits is what sound.md asked for and the port exists (no_std, MIT): add it as an instrument rather than growing more voices by hand. |
| 19 | mp3 decoding | symphonia, rodio | not duplicated (inside web-audio-api; the browser decodes on the web) |
| 20 | `assets/fonts/oblique.py` | bevy #25569 (open: no synthetic italic) | keep until fixed |

## 2. What you have that the ecosystem does not (publish these)

1. **A deterministic libm kernel with JavaScript Math semantics**, with
   the wasm-vs-native bit test. No Rust crate does this; the closest is
   the C++ v8math harness. Package: `det-math` (+ `det-lint`).
2. **Real geography to a drivable world**: Terrarium/3DEP DEM + ESA
   WorldCover + Overture/OSM roads, planned as corridors, built as
   streamed sections with sculpted terrain. Only osm_to_bedrock
   (Minecraft) and the weeks-old bevytiles come near. Package: `atlas` +
   `route-world`.
3. **A permissively licensed physical-model engine-sound synth** that
   runs in an AudioWorklet from stable Rust (ghurni is GPL;
   firewheel-web-audio needs nightly). Package: `exhaust-synth`.
4. **A vehicle-dynamics crate for Bevy** (brush tyre, drivetrain,
   N-axle `VehicleDef`, `Ground` trait). Nothing is on crates.io.
   Package: `vehicle-dynamics`.
5. **A headless-Chromium test rig for Bevy wasm builds** on both
   backends: CPU profile attributed to Bevy systems, WebGL call counts
   per program, pixel diff of two builds, phone emulation and throttling.
   Package: the `js/` tools.
6. **A Cloudflare Workers matchbox-protocol signalling relay** (Durable
   Object per room). Nobody has one; the community deploys matchbox_server
   on a VPS. Package: `signal-worker`.
7. **Generative music with genre grammars, wall-clock radio stations and a
   voiced DJ director**, and the **manifest-driven TTS voice-line pipeline**
   (character sheet to candidates to render to verify to audition to
   index). No equivalent in Rust or in any engine's open source.
   Packages: `gen-music`, `wallclock-radio`, `voice-cast`.
8. **Device-tilt input for wasm** with the iOS permission-in-gesture
   flow. Nobody ships it.
9. **A Canvas 2D on tiny-skia + harfrust/skrifa** with Chrome-matching
   coverage, usable for Bevy images. bevy-canvas-2d is unstable and on
   0.17.
10. **One-entity-N-instances with partial in-place updates** for custom
    materials, and a **shared globals uniform** so material parameters
    animate without re-preparing Bevy materials. Both are upstream
    candidates (section 3) as much as library code.

## 3. Upstream: what should be a PR to wgpu or Bevy

| Patch / idea | Status upstream (2026-10-16) | Recommendation |
|---|---|---|
| **wgpu**: cache the WebGPU device's limits and features (D1182, ~23 lines) | Issue gfx-rs/wgpu#10550 (opened 2026-10-06, no PR, no reply). The tooling agent also found PR #10513 "Backend Caching of Heavy Requests" awaiting review, and bevy#25998 caching limits on Bevy's side; verify which is live before acting. | Comment on #10550 with the 7%-of-a-phone-frame numbers and offer the patch as the PR if #10513 is not it. Drop the local patch when either ships. |
| **wgpu-hal**: GL backend skips redundant binds (D1190, ~118 lines) | Not filed. trunk's `gles/queue.rs` still binds unconditionally. | **Open the PR**, split in three: vertex-buffer dirty check; program/texture/sampler bind cache per command buffer; per-texture mip-range tracking. Include before and after call counts (24,600 to about 15,400 a frame on Sierra; gather the after number with `gl-calls.mjs`). Expect questions about external GL interop and cache invalidation around copies. |
| **wgpu**: the WebGPU backend destroys buffers and textures when the last user drops them (D9, ~200 lines) | gfx-rs/wgpu#4092 closed 2024-05 as not actionable (the refcounting was thought too costly); trunk still leaves them to the JS GC. | Comment with the A/B (no cost; 5th-percentile fps ~200 to ~300 on Sierra), then offer the PR. `docs/upstream/wgpu-webgpu-destroy-on-drop.md`. |
| **bevy_ui_render**: skip drawing an empty batch (D1200, ~35 lines) | Not filed. `DrawUiNode` on main has no emptiness check. | One-line PR (`if batch.range.is_empty() { return Skip }`), or fix at the source in `prepare_uinodes`. Meanwhile drop the patch and filter the Chrome warning in the e2e test. |
| Bevy `render_system`: don't submit an empty screenshot encoder every frame (`render/frame.rs`, D862) | Not filed | PR candidate; a full `GPUQueue.submit` saved per frame on the web. |
| Bevy/winit web: `set_scale_factor_override` does not change the canvas's pixel count (D1195) | Not filed | Issue first; Bevy needs a real render-resolution scale on the web. |
| Bevy: first-class "one entity, N instances via per-instance vertex buffer" for custom materials | example pattern only | Issue/RFC; today it needs replacing `DrawMesh` globally, fragile across upgrades. |
| Bevy: a shared uniform binding across all materials of a type (D1191) | hand-written `AsBindGroup` | Issue/RFC |
| Bevy: renderOrder-style sort key on Transparent3d (`render/sort.rs`) | — | small PR candidate |
| Bevy: fixed orthographic shadow box for a directional light | `override_cascades` hack | issue |
| Bevy #21260: gamepad activation of headless widgets | open ~1 year | your `ui/nav.rs` glue is a PR candidate once ported to bevy_input_focus |
| Bevy #25569: synthetic italic text | open | keep `oblique.py` |
| matchbox: 0.19 port | table stops at 0.18 | check before upgrading the games |

## 4. Ecosystem notes for the planned game types

| Game | Needs beyond the toolkit | Ecosystem answer |
|---|---|---|
| Bicycling | single-track lean dynamics (mp_vdyn is two wheels per axle); route world | own `vehicle-dynamics` extension; `route-world` fits |
| Kayak | water surface with wave height for buoyancy; river as a route | bevy_water 0.18 (webgl2 + webgpu, `get_wave_point`) or extend `sea.rs`; `route-world` fits |
| Mario-64 style | character controller, cell-partitioned streaming, deterministic rigid bodies | bevy-tnua 0.32 on avian3d 0.7 with `enhanced-determinism` and `parallel` off; add a cell partition to the streaming trait |
| Skydiving | free-fall body, atmosphere, far terrain streaming by cell | own `RigidBody` + Bevy `Atmosphere` (WebGPU) ; big_space if the world is large |
| Ocean diving | volumetric fog, water from below | Bevy volumetric fog (0.14+, WebGPU) ; bevy_water |
| Basketball | deterministic ball physics, no streaming | avian3d `enhanced-determinism`; the `Simulation` trait and `star-rollback-net` apply unchanged |
| Zelda / fantasy | character controller, cell streaming, scenery kits, NPC dialogue | bevy-tnua + avian; `scenery-kit`; the DJ director generalises to NPC barks with the same `voice-cast` pipeline |
