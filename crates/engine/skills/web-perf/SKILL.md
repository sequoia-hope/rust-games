---
name: web-perf
description: Build, measure and profile the web build of a Playground Kit game (today Midnight Playground): cargo xtask web/size/prof, frame times on desktop, emulated phone or throttled CPU, CPU profiles by Bevy system, WebGL2 calls and GPU time by pass and program, pixel diffs between builds, the page console, and reading the debug overlay against the budgets. Use when asked why the game is slow, to find a frame cost, to check a change did not slow the game or change the picture, or to measure wasm size.
---

# Web perf: measure before you touch anything

Today the tools are Midnight Playground's `tools/parity/*.mjs`
(`~/Software/midnight-playground`; paths relative to it). They are moving
to the kit's `js/` as `perf.mjs`, `prof.mjs`, `gl-calls.mjs`,
`build-diff.mjs`, `console.mjs` with a `--global` flag (the `__game`
contract) and `CHROME_PATH` for the browser (PLAN.md Layer 2). Until that
lands, use the paths below; the flags are the same.

## Read first

- The game's `CLAUDE.md` commands table: the exact invocations.
- `docs/rust-port/BASELINE.md`: "How to read the numbers", "Phones", and
  the sections for D1180 to D1182 (a phone pass), D1190 (a 2017 laptop),
  D1191 (Mesa's fragment shaders), D1194 and D1195 (render scale and the
  canvas's pixel size). The budgets and how each was measured are there.
- DECISIONS D1180, D1182, D1190 to D1195: six worked examples of finding
  a frame cost and proving the fix.
- `crates/mp_game/src/debug_overlay.rs` header: what the overlay shows.
- `tools/parity/lib/server.mjs`: every tool gets the registered server's
  address and Chrome's flags from here. Nothing else knows a port.

## Build

```bash
cargo xtask web                   # debug build into dist/next/
cargo xtask web --release         # the optimised build (wasm-opt), what Pages ships
cargo xtask size                  # wasm size report after a release build
cargo xtask prof                  # dist/prof/: the release build with names kept (no rebuild)
```

`xtask web` checks `wasm-bindgen-cli` matches `Cargo.lock` exactly. The
build is self-contained and every URL relative: it must work at
`dist/next/` locally and at the site root on Pages. View it only through
`proj up midnight-playground`.

## Measure

Frame time, memory, load, reloads (the page's own `?perf=1` recorder):

```bash
node tools/parity/rust-perf.mjs --level sierra --race                    # desktop, WebGPU
node tools/parity/rust-perf.mjs --level sierra --race --backend webgl2
node tools/parity/rust-perf.mjs --level sierra --race --phone            # 844x390 at dpr 3, hq off
node tools/parity/rust-perf.mjs --level sierra --race --throttle 4       # CPU 4x slower
node tools/parity/rust-perf.mjs --level sierra --race --game js          # the JS game beside it
# a laptop over ssh (headless may sit on SwiftShader): a window on $DISPLAY
node tools/parity/rust-perf.mjs --level sierra --race --headed --angle gl --size 1456x740 --dpr 1.25
node tools/parity/rust-perf.mjs --level sierra --race --query "scale=0.75"   # the render scale (D1194)
```

Read: median and 95th frame, frames over 33 ms, main-thread CPU per frame,
wasm memory high-water, load to ready. Compare like with like: same level,
backend, device preset and seconds. Run twice; take the middle.

## Profile

```bash
cargo xtask web --release && cargo xtask prof
node tools/parity/rust-prof.mjs --level sierra --backend webgl2 --throttle 4
node tools/parity/rust-prof.mjs --from parity/report/perf/prof-sierra-webgl2.cpuprofile --callers 'InstanceStream' --depth 6
```

Three reports: **self** (functions with the most own samples),
**inclusive** (without the schedule's glue), **systems** (time by the
nearest enclosing Bevy system: the thing you can act on). `--callers`
answers "who calls this and how often" for one pattern. The build must be
`dist/prof/`; `dist/next/` shows `wasm-function[1234]`.

GPU side and WebGL2 calls:

```bash
node tools/parity/gl-calls.mjs --game rust --level sierra               # calls a frame by name; gpu ms by pass
node tools/parity/gl-calls.mjs --game js --level sierra                 # the JS game for comparison
node tools/parity/gl-calls.mjs --game rust --by-program --passes 30     # GPU time by shader program
node tools/parity/gl-calls.mjs --game rust --headed --angle gl --sub-frag 'textureSampleGrad\(.*?\);=>vec4(0.0);'   # what-if: a shader without X
MP_DUMPIO=1 INTEL_DEBUG=perf node tools/parity/gl-calls.mjs --game rust --headed --angle gl   # Mesa's compile notes (SIMD16 failures)
node tools/parity/gl-calls.mjs --game rust --dump-shaders /tmp/shaders
```

`EXT_disjoint_timer_query_webgl2` gives the GPU's own time per frame, so
you can tell a CPU-bound frame from a GPU-bound one before changing
anything (D1190 was CPU: 24,600 calls a frame; D1191 was GPU: the
terrain's fragment shader).

## Prove the fix

```bash
cargo xtask prof                                   # before: copies dist/next/ to dist/prof/
# ... make the change, cargo xtask web --release ...
node tools/parity/build-diff.mjs --a prof --b next   # four levels, pixel for pixel, red overlay in parity/report/build-diff/
node tools/parity/rust-perf.mjs ... (the same flags as before)
```

A performance change should be **pixel-identical** unless the DECISIONS
entry says what changed and why. Then measure again with the identical
flags, and write both numbers in the entry. A build that does not come up:

```bash
node tools/parity/web-console.mjs --backend webgl2 --secs 60
node tools/parity/drive-branch.mjs --level peninsula --phone --secs 300   # a hang while driving: console, ticks, frame time
```

## The debug overlay

`?debug=1` (`--debug` natively), F3 toggles. Three times, all ms:
**frame** (interval between frames: what smoothness feels like), **main**
(the main world's CPU from `First` to `Last`), **render** (the render
world's schedule, presentation included; on the web it runs after main,
so main + render is the frame's CPU). Each over 5 s, 30 s and all-time,
as min, mean, 99th, max, with a graph. A frame far above main + render
is the GPU or the compositor; a main spike with a flat render is the
simulation, streaming or UI. The owner reads this on a phone; ask for
the three numbers and the level.

## Budgets

BASELINE.md holds them per device: the JS game's numbers on the reference
phones and the dev machine are the ceiling the Rust build must stay under
(SPEC 6.6), measured with the same tools and the same flight. Wasm size has
a per-crate budget in `cargo xtask size` (CI fails over it). When a number
moves, update BASELINE.md's section for the decision, not the budget.

## Common mistakes

- Measuring on the dev machine's GPU and calling a phone fine. Use
  `--phone`, `--throttle 4`, and ask the owner for the overlay numbers.
- Profiling `dist/next/` (stripped). `cargo xtask prof` first.
- Changing two things between measurements.
- Trusting ANGLE's Vulkan backend for a laptop problem: Chrome on Linux
  Intel uses the native GL driver (`--angle gl`), and the shaders differ.
- A `#[uniform]` field in a material derive: a buffer per material,
  re-prepared every frame (D1191). Share one globals buffer.
- Rebuilding every instance of an instanced mesh when one moves (D1180);
  reading device limits per draw (D1182, patched wgpu); rebinding what is
  bound (D1190, patched wgpu-hal).
- Assuming the render scale drew fewer pixels (D1195): check the pass size
  in `gl-calls.mjs`.
- Starting a server for the tools. `lib/server.mjs` reads the registered one.
