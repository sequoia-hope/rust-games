# A new game on Playground Kit: the template

The skeleton of a game that builds on the kit: `cargo xtask` on
`pk_xtask`, the loader page, the dev server, CI and Pages. Nothing in it
names a game: the places to change are marked.

## Instantiate

1. Copy this directory beside the kit, so the game sits at
   `../<game>` and the kit at `../game-engine`:

       cp -r ../game-engine/template ../<game> && cd ../<game> && git init

2. Register it with the port registry (`proj add <game> .`), which gives
   it a port; `serve.sh` asks for it, never stores it.
3. Write the client crate, `crates/game/`, on `pk_shell` (below), and the
   simulation crate as a `pk_lockstep::Simulation`. Add each crate's rule
   to `deps.toml`.
4. Rename `game` where it appears: `xtask/src/main.rs` (`crate_name`,
   the out-names, `page_dir`), `web/index.html` (the three BRANDING blocks
   and `OUT_NAME`), `tools/serve.py` (`TITLE`, `PINNED`), `CLAUDE.md`,
   `.github/workflows/ci.yml` (the `-p game` lines) and `docs/DECISIONS.md`.
   `web/` goes to `crates/game/web/` (or point `page_dir` elsewhere).
5. `npm init -y && npm install --save-dev puppeteer-core` for the kit's
   headless-Chrome tools (`../game-engine/js/README.md`).
6. `cargo xtask web --release`, `proj up <game>`, open `/`.

## What is where

| File | What it is |
|---|---|
| `xtask/` | the `WebBuild` config and `pk_xtask::main_with`; add the game's own commands there |
| `web/index.html` | the loader page: backend pick (WebGPU, else WebGL2), download progress, gesture bridge, crash screen, `?stats=1`, `?perf=1`; branding in three marked blocks, the status global's name (`GLOBAL`) in one place, game hooks in one object |
| `web/perf.js` | the on-device measurement behind `?perf=1`, read by `js/perf.mjs` |
| `tools/serve.py`, `serve.sh` | the dev server on the registered port: the build over the root, no-store, precompressed files, a "not built yet" page |
| `.github/workflows/` | CI (fmt, clippy native and per wasm backend, tests, check-deps, the lavapipe smoke test, web build, size budget) and Pages, both cloning the kit beside the game |
| `Cargo.toml`, `.cargo/`, `rust-toolchain.toml` | the workspace: the `pk_*` path dependencies, the profiles, the kit's `[patch.crates-io]` block, the `cargo xtask` alias |
| `deps.toml` | the crate dependency rules `cargo xtask check-deps` enforces |
| `CLAUDE.md`, `docs/DECISIONS.md` | the working rules for agents and the decisions log |

## What the client crate gets from `pk_shell`

The crate depends on `pk_shell` (`pk_shell.workspace = true`, and
`features = ["webgpu"]` or `["webgl2"]` in its two wasm target tables, beside
Bevy's own backend feature) and, in order, writes:

| In the crate | From `pk_shell` |
|---|---|
| `App::new().add_plugins(ClientPlugins.build().set(WindowPlugin {..}))` | `plugins::ClientPlugins`: `DefaultPlugins` without the 2D sprites (0.25 MB of wasm) |
| its options | `params::Params::from_query(web::query())` on the web, `Params::from_args(&args, &ALIASES)` natively; `get`, `flag`, `switch`, `num`, `parse` |
| `add_plugins((status::StatusPlugin, FocusPlugin))` | `status::LoadStatus` (`state: LoadState`, `progress`, `frames`, `ready`, …), readiness with the GPU fence and the late frames, `ReadyBlockers`; `FocusLost`/`FocusGained` on both platforms |
| `store::Store::platform(&StoreConfig { prefix, app_dir, env_var, legacy_dirs })` | the settings store: JS-exact strings, `localStorage` or an atomic file |
| natively `native::WindowStatePlugin { config: WindowConfig {..}, keys, persist }`, `SmokeTestPlugin`, `StatusTitle` + `title`, `LogPlugin { custom_layer: native::log_tally::layer }` | saved bounds, F11, Ctrl+Q, the icon; `--smoke-test` and `--screenshot`; the title line; the warning and error tally |
| on the web `web::WebPlugin { global: "__game", aliases: &[] }` and the exports `gesture`, `gesture_at`, `screenshot`, `stage`, `backend` as `#[wasm_bindgen]` wrappers over `web::gesture`, `web::gesture_at`, `web::screenshot`, `web::stage`, `backend()` | the status contract published each frame (`bridge`), `on_gesture`, `RenderScale` + `PixelRatioPolicy` (the pixel ratio and the split-screen camera), `CanvasScale`, `FullscreenOnTap`/`PermissionOnTap` + `TapPolicy`, `page_touch`, `insets`, `footer`, `copy_text`, `prompt`, `register_stage` |
| while building a scene, `warmup::Combos<M, K>::note(..)` then `spawn(..)`; `prewarm_glyphs(sizes)`, `prewarm_ui`; `end_warm_up` in `Update` | every pipeline compiled behind the loading screen |
| `debug::DebugOverlayPlugin { on: params.flag("debug"), ..default() }` and a system writing `debug::DebugLines` | the frame-time overlay (F3) |
| `recorder::RecorderPlugin {..}` when `recorder::wanted(&params)`, `RecordLine` messages and `recorder::InputLog<I>` for the inputs | the JSON-lines session log |
| `worker::Worker { script }`, `stations::Stations<P>` | a Web Worker executor; screenshot stations natively |

The status contract (`window.__game`: `state`, `frames`, `ready`,
`backend`, `error`, `shot`, `wasmMemoryBytes`, `stage`) is documented in
`web/index.html` and in `pk_shell::bridge`; the kit's `js/` tools read it.
Midnight Playground (`crates/mp_game` there) is the worked example of
every row.

A `cargo generate` layer (`{{project-name}}` substitution) is not part of
this template yet; the rename list above is short enough by hand.
