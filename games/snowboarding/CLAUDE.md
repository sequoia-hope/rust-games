# Snowboarding: working rules for agents

A 1080°-style snowboarding game: Rust + Bevy 0.19.1 + wasm. Since
2026-10-18 it lives in the rust-games monorepo at `games/snowboarding/`:
read the root `CLAUDE.md` (two levels up) first; its rules (one agent
follows a task into the shared crates, branches and worktrees,
`scripts/land.sh`, `status/`) override anything here. The sister game is
`../midnight-playground`, which this one borrows from: when you need a Bevy
0.19 idiom, its `crates/mp_game` is the best reference, and so is
`~/.cargo/registry/src/*/bevy_*-0.19.1`. The shared engine (Playground
Kit) is `crates/engine/` at the repo root; adopting its `pk_shell`
(`crates/engine/docs/handoff-snowboarding.md`) is open work. The radio is
the shared music engine (`crates/music`) playing this game's station
table, `crates/sb_game/stations.json`.

## Principles

1. **The simulation is a library** (`sb_sim`): no Bevy, no clock, no
   threads. It steps a `Session` one fixed tick (`TICK`, 1/120 s) at a time
   through its countdown, scripted start-ramp drop-in, run and results.
   The client only reads it and feeds it `Input`.
2. **What is drawn is what is ridden**: the client meshes `Course::height`
   at `Course::mesh_rows`/`mesh_columns`; never invent terrain in the client.
3. Only `sb_game` depends on Bevy. Keep `ClientPlugins` (`plugins.rs`) in
   step with Bevy features you turn on.

## Commands

| What | Command |
|---|---|
| Tests (this game's crates; the whole workspace: root `CLAUDE.md`) | `cargo nextest run -p sb_sim -p sb_radio -p sb_game` |
| Lints | `cargo clippy -p sb_sim -p sb_radio -p sb_game --all-targets` |
| Web build into `dist/web/` | `cargo xtask web --release` (`--only webgpu\|webgl2`) |
| Wasm sizes | `cargo xtask size` |
| Headless CPU race, for tuning physics | `cargo run --release -p sb_sim --bin sb-sim -- <course> <mode>` |
| Native client | `cargo run --release -p sb_game -- --course pipe --go [--auto]` |
| The whole test suite (`--web`: also the browser checks, server up) | `cargo xtask check [--web]` |
| Random screen changes in the browser, failing on errors, freezes, heap growth | `node tools/stress.mjs [--steps 200] [--seed 1] [--backend webgl2] [--touch --size 844x390]` |
| The wasm heap across rides, restarts, course changes | `node tools/heap-probe.mjs [webgl2\|webgpu] [ride restart course long]` |
| Every course ridden, failing on heap growth or GPU buffers re-made every frame | `node tools/courses.mjs [--backend webgl2] [--touch --size 390x844]` |
| The music sounds and the announcers talk (web) | `node tools/radio-check.mjs [--backend webgl2]` |
| The announcers' script as JSON (after editing `sb_radio/src/script.rs`) | `cargo run -p sb_radio --example script > tools/announcers/lines.json` |
| Record the announcers' new or changed lines; check them by ASR | `/mnt/carson/qwen3-speech-bench/tts-venv/bin/python tools/announcers/render.py`, then `.../asr-venv/bin/python tools/announcers/check.py --model /mnt/carson/qwen3-speech-bench/models/Qwen3-ASR-1.7B` |
| A picture of the web build (needs the server up) | `node tools/shot.mjs --query "course=timber&go=1&auto=1&skip=20" --name x [--backend webgl2] [--touch]` |

Toolchain: Rust 1.99.0 with `wasm32-unknown-unknown` (`rust-toolchain.toml`),
`wasm-bindgen-cli` at exactly the `wasm-bindgen` version in `Cargo.lock`
(0.2.129; `cargo xtask web` checks it), `wasm-opt` for release builds.

## Tests to keep

- A crash "on a screen change" is caught by `sb_game/src/tests.rs`: add a
  test there that drives it by keys or touch, and let `Harness::check`
  hold the invariants. Headless frames are of even length; a bug that
  depends on uneven frame times (as the glyph-atlas leak did) needs them
  jittered (`tests/leak.rs` shows how).
- Nothing may be re-uploaded every frame in bulk: on an iPhone a fresh
  200 KB buffer a frame (Avalanche Bowl's falling snow, a mesh rebuilt on
  the CPU) killed the tab within seconds, though Chrome never minded. Move
  things with transforms or in a shader (`snowfall.wgsl`, `wind.wgsl`). Held by
  `every_course_uploads_little_per_frame` and `tools/courses.mjs`.
- Never animate a `TextFont` size: every distinct size gets glyph atlases
  that are never freed (it leaked about 8 MB a run until a phone killed the
  tab). Scale with `UiTransform` instead.
- The announcers' voices must stay unlike midnight-playground's DJs (and
  Dani and Wren are friends and old rivals, not a couple). Check a new
  voice with `tools/announcers/compare.py ../midnight-playground/tools/dj-voice/voices`
  (the tts venv's python).
- The announcers are a high-energy **television** sports booth
  (play-by-play and analyst), not radio DJs: in the words, and in the
  reference clips their voices are cloned from (the clone copies the
  delivery). When designing or tuning reference voices in
  `tools/announcers/voices.json` (`design.py` $\to$ `render.py`), design
  directly with `VoiceDesign` (avoid double-cloning via `"clone"`), anchor
  vocal register in `instruct` rather than putting `!` on every sentence
  (which shifts F0 up an octave into a shrill 400 Hz voice), end `ref_text`
  at peak landing excitement (because Base ICL cloning's causal recency bias
  makes short action calls copy the energy of the final 2–3 seconds of
  `ref_audio`), and set `"max_pause_s": 0.14` to tighten pauses. See the
  headers of `tools/announcers/design.py` and `render.py` and `"about"` in
  `tools/announcers/voices.json` for full notes. A line must be true of what
  happened: one that assumes something (a tree hit, a clean run) goes in a
  topic the director picks only when it did (`lose-trees`, `win-clean`),
  never in a generic one.
- Audio is presentation: the announcers' director draws its own random
  numbers, never the session's.

## The owner's phone

The owner plays on an **iPhone** (Safari, WebKit), which none of the
headless Chrome checks are. Bugs that only happen there:
- **Read `logs/client.jsonl` first** (the page's reports: starts, crashes
  with the console's last lines, and tabs that vanished, i.e. were killed,
  with the screen and heap last seen). The server's access log is in tmux:
  `tmux capture-pane -p -J -t '=web-snowboarding:0.0' -S -`.
- Over plain http (`http://<machine>:<port>/`) Safari has no WebGPU and no
  AudioWorklet (the music falls back to the main thread). The registry's
  `web_url` is the tailnet https front, which has both.
- Silent mode mutes Web Audio unless `navigator.audioSession.type` is
  `'playback'` (`audio.rs` asks for it).
- Playwright's WebKit is cached in `~/.cache/ms-playwright/webkit-2336`
  (`webkit.launch({executablePath: '<that>/pw_run.sh'})`, with
  `~/Software/synthwave/node_modules/playwright-core`): headless only, on
  software GL at about one frame a second, but it is WebKit.

## Viewing output

Only through the registered server: `proj up snowboarding` (or
`./serve.sh`). It serves the repo root with `dist/web/` laid over it, so
`/` is the game and `/shots/` holds the pictures `tools/shot.mjs` takes.
**Never start another server, and never write a port number** into a
script, config, default or doc. Anything that needs the port takes
`--port`/`--url`, then `$PORT`, then `proj port`, then fails loudly.
Every URL the page uses is relative.
