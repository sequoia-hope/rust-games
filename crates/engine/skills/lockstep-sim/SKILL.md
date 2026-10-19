---
name: lockstep-sim
description: Write or change a deterministic fixed-tick game simulation for Playground Kit (the pk_lockstep Simulation trait; today Midnight Playground's mp_sim): the purity and exact-math rules check-deps enforces, named counted RNG streams, events not side effects, the state hash, the trace record, recordings and replay, fuzz, and what to do when two runs diverge. Use when asked to add a simulation feature, a new sim crate, a rule in the tick, fix a desync, a replay mismatch, or a check-deps failure.
---

# Lockstep sim: deterministic by construction

A new kit game starts from the `pk_lockstep` `Simulation` trait
(`docs/design/simulation.md` in this repo, being written now; PLAN.md
Layer 0 has the sketch: `type Ctx; type State: Clone; type Input: Copy +
Default + Eq; type Event; const DT; fn step(ctx, &mut State, &[Input],
&mut Vec<Event>); fn hash(&State) -> u64`, with `RngStreams`,
`Recording`, `assert_deterministic` and `fuzz` beside it). Midnight's
`mp_sim` is the reference implementation and the source of every rule
below (`~/Software/midnight-playground`; paths relative to it).

## Read first

- The game's `CLAUDE.md` principles 3 to 6 and "JS semantics to keep".
- `docs/rust-port/SPEC.md` 4.1 (time), 4.2 (numeric rules), 4.3 (state and
  API, named streams, tick order as a contract), 4.6 (tests, the trace
  record).
- `xtask/src/deps.rs`: `Rule { allowed, banned, pure, exact_math }`,
  `SIM_BANNED`, `INEXACT`, `IMPURE`. Moving to the kit as `deps.toml`
  read by `pk_xtask`; the rules are the same.
- `crates/mp_sim/src/rng.rs` (streams count their draws), `trace.rs`
  (the record is the format; `fnv1a64`), `replay.rs` (a recorded race).
- `crates/mp_sim/bin/mp-sim.rs`: `race`, `bench`, `replay`.
- `docs/rust-port/RECORDING.md`: the client's run recording.

## The rules

1. **The simulation is a library.** No engine, renderer, clock, threads,
   global state or I/O. `#![forbid(unsafe_code)]`. The crate's rule in
   `deps.rs` says `pure: true` (no `std::time`, `std::thread`, `HashMap`,
   `HashSet`, `Instant`, `SystemTime` in the source) and bans `bevy*`,
   `wgpu*`, `web-sys`, `js-sys`, `wasm-bindgen*`, `rand*`, `getrandom`,
   `rayon*`, `crossbeam*` from its tree. A new crate needs a rule or
   `check-deps` fails.
2. **One fixed tick** (1/120 s in Midnight) for everything. The client
   interpolates between the previous and current state; at most six ticks
   a frame. Only `step` fixes the tick.
3. **Exact math**: `+ - * /` and `sqrt` are exact; everything else goes
   through `pk_math::kernel` (`sin`, `cos`, `tan`, `asin`, `acos`, `atan`,
   `atan2`, `exp`, `log`, `log2`, `pow`, `tanh`, `hypot`) on `libm`, the
   same bits on every platform. `exact_math: true` rejects `x.sin()`,
   `powi`, `powf`, `mul_add`, `exp`, `ln`, `hypot`, `cbrt` and friends in
   the source. Keep the order of arithmetic; never reassociate.
4. **Same widths at the same points**: `f64` scalars; round to `f32` where
   the stored data is `f32`; integer hashing (`mulberry32`, `hash2`) with
   wrapping 32-bit arithmetic.
5. **Ordered iteration only**: `Vec`, arrays, `BTreeMap` when a map is
   unavoidable. Sorting is stable (`sort_by`). A body's identity is its
   index in its pool.
6. **Named, counted random streams in the state** (`RngStreams`: `ai`,
   `police`, `pursuit`, `traffic`), seeded from the race seed as
   `(seed + 0x9E3779B9 * (k + 1)) as u32`. Presentation never draws from
   them; effects use their own generator. The draw count is in the trace,
   so a moved draw is visible.
7. **Events, not side effects.** `step` writes what happened into
   `&mut Vec<Event>` (a lap, a hit, a finish, an event's start) and the
   client reacts (sound, particles, HUD). The state never holds a handle,
   closure, callback or `Rc`; a parking target is an enum, a height rule
   is an enum naming the surface. Everything that changes is plain data:
   `Clone`, `PartialEq`, small (Midnight's target: under 32 KB).
8. **Inputs are quantised** before they reach `step` (`InputFrame`:
   `steer: i16`, `throttle: u8`, `brake: u8`, `flags: u8`), so the local
   run, the network and a replay see identical values. Players are a
   `Vec` from the first commit.
9. **The tick order is a contract.** Write it as a numbered list in the
   module header (SPEC 4.3's); every item changes results if moved.
10. **Audio, music and world builders are not simulation**: they may use
    platform `f64` math (`exact_math: false`) but still ban the engine
    and ambient randomness.

## Procedure for a change

1. Read the rule for the crate in `deps.rs`; read the tick order; find
   where the change belongs in it.
2. Write the test first: a staged scenario or a golden where one exists;
   otherwise a determinism test (clone the state, step both, compare
   hashes) and a fuzz run.
3. Implement with plain data and events. Add any new state to the hash and
   to the trace record only if it affects later ticks (D1219's rule: hash
   the event only while it exists, so every other race hashes as before).
4. Run the gates:

   ```bash
   cargo test -p mp_sim
   cargo xtask check-deps
   cargo run --release -p mp_sim --bin mp-sim -- race --level sierra --car sports
   cargo run --release -p mp_sim --bin mp-sim -- race --level desert --fuzz 1 --ticks 21600
   cargo run --release -p mp_sim --bin mp-sim -- bench
   cargo test -p mp_sim --target wasm32-unknown-unknown    # the same hashes under Node
   ```

   Fuzz wants every tick finite, the state not growing, no body far
   outside the walls. Bench must not get slower; write the number down.
5. If the goldens moved on purpose, regenerate them and say why in
   DECISIONS (a behaviour change against the JS oracle is a DEVIATIONS
   row). If they moved by accident, step 6.

## When a trace diverges

It is a bug until shown otherwise. Find **the first differing field at the
first differing tick**, never a later one:

```bash
cargo run --release -p mp_sim --bin mp-sim -- race --level coast --pursuit 3 --trace a.trace
cargo run --release -p mp_sim --bin mp-sim -- race --level coast --pursuit 3 --state-at 6000
node tools/parity/trace-inspect.mjs a.trace b.trace          # first tick and field that differ
```

The record (`trace.rs`, `parity/trace-format.md`) is `f64` bits and
`i32`s in a fixed order with the stream draw counts at the end; a golden
keeps the per-tick FNV-1a 64 hash and the full record every 120 ticks.
Typical causes, in order of likelihood: a draw added or reordered (the
stream counts differ first); a platform math call that slipped past
`check-deps` through a dependency; an `f32` store missed; a `HashMap` in a
helper; an iteration order; a presentation value read into the sim (the
camera, the wall clock, the frame dt).

## Recording and replay

The client records a run (`--record`, `?record=1`; `recordings/*.jsonl`,
ignored by git) with the options, every tick's input run-length encoded,
and the state hash at checkpoints. Replay it headless:

```bash
cargo run --release -p mp_sim --bin mp-sim -- replay recordings/2026-10-14T03-21-09Z-peninsula.jsonl --trace out.trace
cargo run --release -p mp_sim --bin mp-sim -- replay recordings/run.jsonl --race 2 --state-at 3000
```

It fails at the first checkpoint whose hash differs: a desync the owner
saw becomes a tick number you can inspect. A restart inside a recording is
a race of its own. Anything that must reach the simulation (a restart of
only an event, D1220) has to travel in the inputs or it cannot be
replayed; a new flag byte changes the recording format and the network
protocol, a step of its own.

## Common mistakes

- Reading the clock, the frame dt or the camera inside the tick.
- `f64::sin` because "it is only the AI". `check-deps` catches the method
  call but not a dependency that does it: keep the sim's dependency list
  to the kit's pure crates.
- Applying an assist or autopilot on the client before the tick and
  forgetting it in the headless drive (D1210: the freeze the tests missed).
  Put the per-tick sampler on both paths or in the sim.
- A `Vec<Event>` that is read and also mutated by the sim; events go out,
  never back in.
- Putting the state's hash on constants (`sim_rivals` is not hashed).
- Fixing the trace by widening a tolerance. Bit-identical or a bug.
