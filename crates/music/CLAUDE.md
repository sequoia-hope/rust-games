# The music engine (crates/music): conventions

Midnight Playground's Music Lab (`games/midnight-playground/tools/music-lab/`,
JavaScript) ported line for line: DSP blocks, instruments and kits, the
sequencer, the composer, the genre grammars, the mix, and the radio
stations' player (a station is a deterministic schedule of generated songs
on the wall clock). The package is `music`. Read the root `CLAUDE.md`
first: this is a shared crate, so its API rules and `CHANGELOG.md` apply.

**The redesign is under way: `PLAN.md`** (the owner's brief is
`docs/music-engine-kickoff.md`). Rust leads now: the JS lab's engine is
frozen, and its UI is moving onto this crate.

- **Port rules** (Midnight Playground's DECISIONS D1150, in `src/lib.rs`):
  same structure, constants and order of random draws as the lab; JS
  rounding and integer semantics kept; `f32` wherever the lab stores into a
  `Float32Array`. Paths in this crate's comments (`tools/…`, `docs/…`,
  `crates/mp_audio/…`) are inside `games/midnight-playground/`.
- **The lab was the oracle**, and still is for today's generator (the
  `classic` one, after the redesign): `tests/golden/` (written by
  `node games/midnight-playground/tools/music-lab/test/golden.mjs`) holds
  every grammar's tracks, some sequencer events and renders; the tests
  reproduce the first two exactly and the renders to a tolerance.
- **On the web** it is also its own small wasm (no wasm-bindgen) in an
  AudioWorklet: `cargo build -p music --features worklet --target
  wasm32-unknown-unknown`, which a game's `cargo xtask web` does, copying
  `music.wasm` and the shim `web/music-worklet.js` beside its page. Keep
  the `worklet` feature's C ABI (`src/wasm.rs`) and the shim stable: the
  games run them. It has no dependencies at all (`deps.toml`).
- **Stations are the game's.** Nothing here names a game's station, DJ or
  level: a game passes its table (JSON, `radio::Station::parse_list`) to
  `player::Player::new` natively and to the worklet at start-up (the
  shim's `stations` option). Midnight Playground's is
  `games/midnight-playground/crates/mp_audio/stations.json`,
  Snowboarding's `games/snowboarding/crates/sb_game/stations.json`; the
  tests and examples use `examples/stations.json`.
