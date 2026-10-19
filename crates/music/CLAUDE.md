# The music engine (crates/music): conventions

Midnight Playground's Music Lab (`games/midnight-playground/tools/music-lab/`,
JavaScript) ported line for line: DSP blocks, instruments and kits, the
sequencer, the composer, the genre grammars, the mix, and the radio
stations' player (a station is a deterministic schedule of generated songs
on the wall clock). The package is still named `mp_music`. Read the root
`CLAUDE.md` first: this is a shared crate, so its API rules and
`CHANGELOG.md` apply.

- **Port rules** (Midnight Playground's DECISIONS D1150, in `src/lib.rs`):
  same structure, constants and order of random draws as the lab; JS
  rounding and integer semantics kept; `f32` wherever the lab stores into a
  `Float32Array`. Paths in this crate's comments (`tools/…`, `docs/…`,
  `crates/mp_audio/…`) are inside `games/midnight-playground/`.
- **The lab is the oracle**: `tests/golden/` (written by
  `node games/midnight-playground/tools/music-lab/test/golden.mjs`) holds
  every grammar's tracks, some sequencer events and renders; the tests
  reproduce the first two exactly and the renders to a tolerance.
- **On the web** it is also its own small wasm (no wasm-bindgen) in an
  AudioWorklet: `cargo build -p mp_music --features worklet --target
  wasm32-unknown-unknown`, which a game's `cargo xtask web` does. Keep the
  `worklet` feature's C ABI (`src/wasm.rs`) stable: the games' worklet
  shims call it. It has no dependencies at all (`deps.toml`).
- **Known fork:** Snowboarding still carries its own copy,
  `games/snowboarding/crates/sb_music`. It is the same engine except
  `radio::STATIONS` (its one station, Ten-Eighty FM, in place of Midnight
  Playground's five), the crate docs and one test's station index. The
  station table is compiled into the worklet wasm, so merging the fork
  means letting each game supply its stations; that is the planned
  redesign's to decide (`docs/music-engine-kickoff.md`), not a quick fix.
- **Redesign pending.** A separate session will redesign this crate from
  `docs/music-engine-kickoff.md`; until then, change it minimally.
