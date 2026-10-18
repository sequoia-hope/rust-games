# Hand-off to Snowboarding's agent

What the snowboarding game (`~/Software/snowboarding`, `crates/sb_game`)
replaces with Playground Kit, in the order PLAN.md section 7 gives, with
the kit API names as shipped. Nothing here was applied to that repository;
its agent adopts each section when its own work allows. Each section says
what is deleted, what replaces it, and what the game gains for free.

## 1. `pk_shell` (the client shell; kit DECISIONS D8)

Add to `crates/sb_game/Cargo.toml`: `pk_shell = { path =
"../../../game-engine/crates/pk_shell" }` (or a workspace entry), and in
the two wasm target tables, beside Bevy's backend feature, `pk_shell = {
..., features = ["webgpu"] }` under `not(sb_webgl2)` and `features =
["webgl2"]` under `sb_webgl2`. The `sb_webgl2` cfg stays the game's; the
kit reads its backend from the feature.

| Snowboarding file | Replace with | Notes |
|---|---|---|
| `plugins.rs` (88 lines, a byte copy of Midnight's) | delete; `pk_shell::plugins::ClientPlugins` | identical group, `UiSpriteSupport` included |
| `options.rs`: `from_query`, `from_args` (the parsing half) | `pk_shell::params::Params::from_query(&pk_shell::web::query())` on the web, `Params::from_args(&args, &ALIASES)` natively, then `Options::set(k, v)` over `params.pairs()` | `Params::get/flag/switch/num/parse`, `params::truthy` is the kit's; `--screenshot a.png --frames 9` become `("--screenshot", "screenshot")` and `("--frames", "frames")` in a `params::Aliases { flags, valued, escaped }`. The typed `Options` struct stays the game's. |
| `status.rs`: `frames`, the per-frame `update`, `native_screenshot` | `pk_shell::status::StatusPlugin` (`LoadStatus { state, progress, frames, ready, frame_ms, pipelines_waiting, late_frames, .. }`), `pk_shell::native::SmokeTestPlugin(SmokeTest { label, after, timeout_s, screenshot, smoke, extra })` | the game's own fields (`screen`, `course`, `mode`, `time`, `s`, `speed`, `phase`, `score`, `tricks`, `crashes`, `place`, `finish`, `lastTrick`) stay in a game resource and go out with `pk_shell::bridge::set(key, value)` in a `Last` system. Set `LoadStatus.state` to `Building`/`Warming` while the course builds, so the page's loading screen and `ready` work. |
| `web.rs` (97 lines) | `pk_shell::web::WebPlugin { global: "__game", aliases: &["__sb"] }`; keep `run`, and the exports as one-line `#[wasm_bindgen]` wrappers: `gesture(kind)` → `pk_shell::web::gesture(kind)` (register the sound's wake with `pk_shell::web::on_gesture(fn)`), `gesture_at` → `pk_shell::web::gesture_at`, `screenshot` → `pk_shell::web::screenshot`, `stage` → `pk_shell::web::stage`, `backend` → `pk_shell::backend()` | the page's `GLOBAL` becomes `'__game'` (the template page); `__sb` stays readable as the same object. `publish` is the kit's `publish_status` plus the game's `bridge::set`s. |
| nothing yet | `pk_shell::store::Store::platform(&StoreConfig { prefix: "sb.", app_dir: "snowboarding", env_var: "SB_STORE", legacy_dirs: &[] })` | **gains** settings persistence (rider, course, mode, radio, volumes) in `localStorage` and a native file, JS-exact strings |
| nothing yet | natively `pk_shell::native::WindowStatePlugin { config: WindowConfig { app_id, title, icon_png, default_size, min_size, state_path }, keys: true, persist: true }` and `LogPlugin { custom_layer: pk_shell::native::log_tally::layer }` | **gains** the window opening where it was closed, F11, Ctrl+Q, an icon, the warning/error tally for `--smoke-test` |
| nothing yet | `pk_shell::FocusPlugin`; a system reading `MessageReader<pk_shell::FocusLost>` that sets `Run.paused` | **gains** pause on focus loss and on a hidden page, before the frame's ticks |
| nothing yet | `pk_shell::warmup::Combos<StandardMaterial, K>` noted while the course's meshes are made, `spawn` with the loading screen up, `prewarm_glyphs(&HUD_SIZES, ..)`, `prewarm_ui`, `end_warm_up` | **gains** pipeline warm-up: no hitch the first time a material and mesh layout is drawn |
| nothing yet | `pk_shell::web::RenderScale { hq, scale, .. }` + `PixelRatioPolicy { on: true, screen_up }` | **gains** the render scale and the sharp-menu-over-soft-scene camera (D1199) |
| nothing yet | `pk_shell::debug::DebugOverlayPlugin { on: params.flag("debug"), ..default() }` and a system writing `DebugLines { head, touch_layout }` | **gains** the frame-time overlay (F3) |
| nothing yet | `pk_shell::recorder::RecorderPlugin { want, name, root, banner, web_label, extra, query }` when `recorder::wanted(&params)`; `RecordLine` messages; `InputLog<Controls>` with the game's encoder | **gains** the JSON-lines session log with per-second frame statistics, slow frames, and a replayable input log |
| the smoke test in `status.rs` | `SmokeTestPlugin` (above); `--smoke-test` prints the adapter and the tally and exits non-zero on an error | **gains** a CI smoke test under lavapipe (the template's CI has the step) |

Order of adoption: the manifest, `plugins`, `WebPlugin` and the exports
(the page's global), `Params`, `StatusPlugin`; then what the game gains,
one row at a time, each behind `cargo xtask web` and the headless
harness. `pk_shell::ui::{css_rect, css_shown, inside}` are there for a
hit test over Bevy UI nodes in CSS pixels when the menu moves to `pk_ui`.
