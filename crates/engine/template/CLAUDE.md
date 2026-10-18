# New Game: working rules for agents

The game is **New Game**, in Rust on Bevy, built on Playground Kit (the
sibling checkout `../game-engine`, crates `pk_*`; its plan is
`../game-engine/docs/PLAN.md`). The repository is `<owner>/<repo>`; Pages
serves the web build at `https://<owner>.github.io/<repo>/`. The local
checkout and the `proj` registry name are `<repo>`.

## Principles

1. **The simulation is a library**: no engine, renderer, clock or global
   state; it steps a plain state by one fixed tick, as a
   `pk_lockstep::Simulation`.
2. **One implementation for every platform**; platform differences sit
   behind small traits (the kit's).
3. **Deterministic by construction**: fixed tick, seeded streams in the
   state, and every inexact math function through `pk_math`'s kernel.
   Never call `f64::sin` and friends, `powi`, or `mul_add` in the
   simulation (`cargo xtask check-deps` rejects them).
4. **Only the client crate depends on Bevy.** Bevy is pinned exactly and
   moves with the kit; no third-party Bevy plugins without a DECISIONS
   entry naming the alternative.

A choice the design does not cover: add it to `docs/DECISIONS.md` and carry
on with the option that best keeps the game as it is.

## Commands

| What | Command |
|---|---|
| Build and test the workspace | `cargo test --workspace` |
| Lints, as CI runs them | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` |
| Crate dependency rules (`deps.toml`) | `cargo xtask check-deps` |
| Web build into `dist/next/` | `cargo xtask web` (add `--release` for the optimised build; `--only webgl2` for one backend) |
| Wasm size report, and CI's budget | `cargo xtask size [--budget]` (after a release web build) |
| The names-kept build for CPU profiles | `cargo xtask prof` (into `dist/prof/`) |
| Native client | `cargo run -p game` |
| The page's console when a build does not come up | `node ../game-engine/js/console.mjs --backend webgl2` |
| Frame times on the GPU, desktop, phone or throttled | `node ../game-engine/js/perf.mjs [--backend webgl2] [--phone] [--throttle 4]` |
| CPU profile by Bevy system (after `cargo xtask prof`) | `node ../game-engine/js/prof.mjs --backend webgl2 --throttle 4` |
| WebGL2 calls a frame and the GPU's time by pass | `node ../game-engine/js/gl-calls.mjs` |
| Two builds compared pixel for pixel | `node ../game-engine/js/build-diff.mjs --a prof --b next` |
| Pictures of the web build | `node ../game-engine/js/shot.mjs --name menu` |
| Screen changes hammered for crashes | `node ../game-engine/js/stress.mjs --steps 200` |

The headless-Chrome tools need `puppeteer-core` (`npm install` here; see
`../game-engine/js/README.md`) and Chrome at `$CHROME_PATH` (default
`/usr/bin/google-chrome`); they read the status global named by `--global`
(the page's `GLOBAL`, `__game` by default).

Toolchain: stable Rust with the `wasm32-unknown-unknown` target
(`rust-toolchain.toml`), `wasm-bindgen-cli` at exactly the version of the
`wasm-bindgen` crate in `Cargo.lock`, and `wasm-opt` from binaryen at the
version `xtask/src/main.rs` pins (`cargo xtask web` checks both).

## Crate rules

`deps.toml` holds the rule for every crate; a new crate needs one there.
The simulation crates have `#![forbid(unsafe_code)]` and no clock,
threads, `rand`, or hash maps.

## Viewing output

Only through the registered server: `proj up <repo>` (or `./serve.sh`). It
serves the repository root with the web build (`dist/next/`, once built)
laid over it, so `/` is the game and `/dist/next/` still works; the tools
and reports are where they are in the tree. Phones and other machines use
the tailnet https front (the registry's `web_url` for this project), which
is a secure context, so WebGPU and tilt work there.

**Never start another server, and never write a port number** into a
script, config, default or doc. A program that needs a port takes
`--port`, then `$PORT`, then `proj port`, then fails loudly. Every URL the
client uses is relative, because the game is always served under a
sub-path.

## Working

1. Read the design note and the kit crate's README for the area first.
2. Write the test first where one can be written; build until it passes.
3. Check your own work: run the tests, render headless, look at the result.
4. Commit and push straight to `main` (no branches): imperative subject, a
   prose body. Build output (`dist/`, `target/`, `report/`) stays out of
   git. `.github/workflows/pages.yml` builds and publishes on every push to
   `main`. The web build is self-contained (`cargo xtask web` copies the
   runtime files into it), so it works at `dist/next/` locally and at the
   root on Pages: never reach above it with `../../` from the page.

**Parallel work.** Packages run in parallel only with disjoint files. A
shared module has one owner; everyone else may add exports to it but not
change what exists.
