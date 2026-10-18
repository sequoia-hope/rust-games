---
name: client-shell
description: Start a new game on Playground Kit or wire an existing one onto it: instantiate the template, Params for query and args, the Store for settings, the Action enum and ActionState, a TouchLayout, screens on ScreenHost, the __game status contract, the xtask web build, and serving through proj. Use when asked to create a new game, scaffold a client, add a menu or settings screen, add input bindings, or hook a game up to the kit's shell.
---

# Client shell: a new game on the kit

**DRAFT: the APIs land in phases 1 and 2 (PLAN.md section 6). Names below
are the plan's; update this file when `pk_shell`, `pk_input`, `pk_ui` and
`pk_play` are in `crates/` and the template is generated for the first
time (phase 7's gate: a new game in an afternoon).**

## Read first

- `docs/PLAN.md` section 1 (the three gates), section 2 (principles),
  section 4 "Layer 1" (`pk_shell`, `pk_ui`, `pk_input`, `pk_play`,
  `pk_render`, `pk_audio`) and "Layer 2" (`template/`, `js/`), section 5
  seams 1, 2 and 7.
- `docs/design/*.md` for the seams that have notes (`simulation.md`
  first).
- `docs/INVENTORY.md` for what each module did in Midnight, when a name
  here is unfamiliar.
- The kit's `README.md` "Working rules".

## What the shell gives a game

| Crate | You get | You write |
|---|---|---|
| `pk_shell` | `ClientPlugins`; `Params` (query string and args with aliases); `AppInfo`; `LoadState` + `ReadyBlockers`; `WarmUp`, `prewarm_glyphs`, `prewarm_ui`; `Store` (Memory, LocalStorage, atomic FileStore) with `StoreConfig`; `WindowStatePlugin`; `FocusLost`; `RenderScale`, `set_pixel_ratio`; `FullscreenOnTap`, `PermissionOnTap`; `Bridge::global("__game")`; `SmokeTest`, `--screenshot`; `DebugOverlayPlugin`; `RecorderPlugin` + `InputLog<I>`; `Worker` | the `AppInfo`, the `StoreConfig`, which blockers gate readiness |
| `pk_input` | `trait Action`; `ActionState<A>`; `KeyMap<A>`; `Pads<A>` with `PadMap<A>` JSON and capture; `trait Rumble`; `TouchLayout<A>`; `TiltSteer` | the `Action` enum, default bindings, the touch layout, `quantise(&ActionState<A>) -> S::Input` |
| `pk_ui` | `Theme`, `Bp` breakpoints, `T` text runs, `Control<A>` widgets, `Cx` + `screen()`, `ScreenHost<S, A>`, `MenuNav`, `PadSetupView<A>` | the screens (main menu, settings rows, pause, results) as functions of state |
| `pk_play` | `Session<S: Simulation>` (accumulator, max ticks, interpolation alpha, per-tick sampler); `ChaseRig`, `Springs`; `Particles`, `TrailRing`; the SDF `UiMaterial`, `Gauge`, `Minimap`; `WorldLabel`, `BeaconMesh`; `SectionStreamer<K>` | the `Simulation` (see `lockstep-sim`), the camera modes as data, the HUD layout |
| `pk_lockstep` | the `Simulation` trait, `RngStreams`, `Recording`, `fnv1a64`, `assert_deterministic`, `fuzz` | `step`, `hash`, `Input`, `Event` |
| `pk_xtask` + `template/` | `xtask web/size/prof/check-deps/imgdiff`, the loader page, `tools/serve.py`, `serve.sh`, CI, Pages | `xtask/main.rs` (about 40 lines of calls), branding, `deps.toml` rules |

## Procedure (draft)

1. **Instantiate the template** (phase 7 names the generator; until then
   copy `template/`):

   ```bash
   cargo generate --path ../game-engine/template --name kayak    # planned
   cd ~/Software/kayak && proj add kayak ~/Software/kayak --desc "Kayak" --web-start "./serve.sh"
   ```

   The template brings `Cargo.toml` with `pk_*` by sibling path and the
   `[patch.crates-io]` block for `../game-engine/patches/*`,
   `rust-toolchain.toml`, `.cargo/config.toml`, `xtask/`, `web/index.html`
   (the loader with its crash screen and backends check), `tools/serve.py`,
   `serve.sh` (reads `$PORT`, then `proj port`, then fails), CI (fmt,
   clippy, tests, lavapipe smoke, wasm tests, web, size), Pages,
   `CLAUDE.md` with the port rule and the DECISIONS convention, and an
   empty `DECISIONS.md`. The first entry is yours: the game's name, its
   `proj` name and what it is.
2. **`deps.toml`**: a rule per crate. The sim crate `pure = true`,
   `exact_math = true`; the client crate may depend on Bevy; nothing else
   may. `cargo xtask check-deps` from the first commit.
3. **The simulation crate** on `pk_lockstep::Simulation` (the
   `lockstep-sim` skill). Tests before the client: `assert_deterministic`
   and `fuzz` pass on native and under Node.
4. **`Params`**: declare the query and argument names the game answers
   (`level`, `seed`, `debug`, `record`, `scale`, `view`), with aliases
   where the JS tools expect old names. Every URL relative.
5. **`Store`**: `StoreConfig { prefix, app_dir, env_var, legacy_dirs }`;
   settings are keys with defaults and a validator each; the web uses
   LocalStorage, native an atomic file under the app dir.
6. **The `Action` enum** implements `pk_input::Action` (axes and buttons,
   with names for the pad-setup screen). A `KeyMap<A>` default, a
   `PadMap<A>` default, a `TouchLayout<A>` (stick, slider bands, tap
   buttons, insets), `TiltSteer` if the game steers. Then `quantise`
   turns `ActionState<A>` into the sim's `Input` once per tick in the
   `Session`'s sampler; assists live there too, on both the client and
   the headless path.
7. **Screens on `ScreenHost<S, A>`**: a function from the game's screen
   state to a tree of `Control<A>`; rebuild on dirty; `MenuNav` for pad
   and keyboard; the settings rows from the `Store`'s keys; `PadSetupView`
   as is. Breakpoints from `Bp`.
8. **`__game`**: the status contract every JS tool reads (`state`,
   `frames`, `ready`, `backend`, `error`, `shot`, `wasmMemoryBytes`,
   `stage`), from `Bridge::global("__game")`; add the game's own fields
   under it, never beside it. `register_stage` names the load stages the
   loader shows. Midnight keeps `__mp` as an alias.
9. **Readiness**: `ReadyBlockers` (the GPU fence, pipeline count, late
   frames) plus the game's own (the first section streamed); `WarmUp` the
   materials and glyphs behind the loading screen; `ready` is what the
   smoke test and Pages' check wait for.
10. **Build and serve**:

    ```bash
    cargo xtask web --release && cargo xtask size
    proj up kayak                      # the registered server; never another
    node ../game-engine/js/console.mjs --global __game      # planned paths
    node ../game-engine/js/perf.mjs --phone
    ```

    The `web-perf` skill has the measuring. The smoke test
    (`--screenshot`, `SmokeTest`) runs in CI under lavapipe.
11. **Debug overlay, recording**: `?debug=1` and F3 come free; `?record=1`
    writes the run recording the `lockstep-sim` skill replays.
12. **DECISIONS** entries for every choice above the template made for
    you that you changed, and push to `main`.

## Gates

- `cargo test --workspace`, clippy with `-D warnings`, `check-deps`, the
  wasm tests under Node, `cargo xtask web --release`, `size` within budget.
- The page comes up through `proj up` on a phone over the tailnet URL,
  `__game.ready` true, the overlay shows three times, a pad remap
  round-trips through the `Store`, touch and tilt work in a secure
  context, a recording replays with matching hashes.

## Common mistakes

- A game type leaking into a kit crate (a car, a road, a station, a
  level): the kit takes a trait or open data; the game implements it.
- A second copy of anything the template provides (a serve script, an
  xtask subcommand, a loader page). Delete the copy; call the kit.
- A port literal anywhere, including `argparse` defaults and compose files.
- Reading input in the sim, or quantising twice.
- Reaching above the page with `../../`; Pages serves the game under a
  sub-path.
- Pulling in a third-party Bevy plugin for input, widgets or persistence
  without a DECISIONS entry naming the alternative (kit D1).
