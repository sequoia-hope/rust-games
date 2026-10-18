# pk_lockstep

The `Simulation` seam of Playground Kit: what a fixed-tick, deterministic
game simulation looks like from the outside, so one frame loop, one
rollback netcode, one recorder and one set of determinism checks serve
every game. The design note is `docs/design/simulation.md`.

- `Simulation`: `Ctx` (built once, immutable), `State: Clone`, `Input:
  Copy + Default + PartialEq`, `Event`, `DT`; `step(ctx, &mut state,
  &[Input], &mut Vec<Event>)`, `hash(&State)`, `tick(&State)`. Static
  methods on a marker type; the caller owns the context and the state.
- `NetSim`: `INPUT_BYTES`, `encode_input`/`decode_input`, `predict(last)`
  (strip edges), `absent(last)` (a dropped player), `accept(input)` (what
  the host takes from a client). The hooks the rollback client and the
  authoritative host need and nothing else.
- `SnapshotSim`: `snapshot`/`restore`, dependency-free; what a late joiner
  or a desynced client will adopt once the netcode carries snapshots.
- `fnv1a64` and `Bytes`: the hash every check uses, and a little-endian
  writer for the bytes it covers.
- `RngStreams<N>`: named, draw-counted Mulberry32 streams inside the
  state, seeded `seed + 0x9E3779B9 * (k + 1)`.
- `Recording<I>`: seed, the game's setup bytes, run-length inputs for every
  player, hash checkpoints; `encode`/`decode` through the `NetSim` codec,
  `replay` against the checkpoints.
- `check_determinism`/`assert_deterministic` (two clones, every tick's
  hash, the first differing tick named) and `fuzz` (held random inputs, a
  predicate after every tick).
- `Pacing`/`advance`/`alpha`: the frame-to-tick arithmetic (frame clamp,
  tick cap, tolerance, NaN and negative frames) with no engine and no
  clock.

`#![forbid(unsafe_code)]`, no clock, threads, hash maps or randomness of its
own; depends on `pk_math` alone.

## Why not ggrs

`ggrs` (and `bevy_ggrs`) is a symmetric peer-to-peer rollback library: every
peer is an authority, inputs are exchanged all-to-all, and its `Config`
trait wants a serialisable state it checksums itself. The kit's games run a
host-authoritative star: the host's inputs are final, a dropped player is
driven by the simulation's own stand-in, a hash every 30 ticks catches a
desync and a rebuild from the confirmed inputs repairs it, and joining is by
a sealed signalling link. That is a different model with different hooks
(`predict`, `absent`, `accept`), so the netcode stays ours. The trait is
shaped so an adapter to ggrs is a small type later (`docs/ECOSYSTEM.md`
row 6; the design note's section 8): `Input` is already `Copy +
PartialEq` with a byte codec and `State: Clone`, which is all a ggrs
`Config` asks for.

## Tests

`cargo test -p pk_lockstep` natively and `cargo test -p pk_lockstep
--target wasm32-unknown-unknown` under Node (`wasm-bindgen-test`; the kit's
`.cargo/config.toml` sets the runner). `tests/toy.rs` is a small simulation
(bodies on a torus, two players, a jitter stream, an edge flag) that proves
the determinism check, the hash, the recording round trip and replay, the
snapshot, the net hooks and the frame arithmetic including the 1/20 s clamp
and the six-tick cap.
