# Music engine redesign — kickoff (stub)

**Stub, to be written by the owner.** The reorganization created
`crates/music` by moving Midnight Playground's `mp_music` with no design
changes; a separate session will redesign it from this document. Facts the
reorganization found, as input:

- `crates/music` is the Music Lab (`games/midnight-playground/tools/music-lab/`,
  JS) ported line for line, checked against goldens in
  `crates/music/tests/golden/`. Package name still `mp_music`.
- Snowboarding carries a fork, `games/snowboarding/crates/sb_music`: same
  engine, its own `radio::STATIONS` (one station, Ten-Eighty FM), docs and
  one test differ.
- The station table is compiled into the AudioWorklet wasm (`src/wasm.rs`
  reads `radio::STATIONS`), so a shared engine needs a way for each game to
  supply its stations (per-game worklet crate, data passed at start-up, …).
- Consumers: Midnight Playground's `mp_audio` (native worklet processor,
  web worklet shim `crates/mp_audio/web/music-worklet.js`) and `mp_game`
  (station UI, store); Snowboarding's `sb_game` (via `sb_music`) and its
  `crates/sb_game/web/music-worklet.js`; Midnight Playground's
  `tools/radio*`, `tools/serve.py` and e2e tests load `mp_music.wasm` by name.

## Goals

_(owner)_

## Constraints

_(owner)_ — plus the root `CLAUDE.md` rules for shared crates.
