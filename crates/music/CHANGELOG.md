# Changelog: the music engine (crates/music)

The music engine is shared by the games here and may be used by games
outside this repo. Every breaking change goes here, with how to migrate;
notable additions too. Newest first.

## Unreleased

### Moved out of Midnight Playground (2026-10-18)

- **Where:** `rust-games/crates/music/` (it was
  `midnight-playground/crates/mp_music`). The package is still named
  `mp_music`, so `use mp_music::…`, the profile overrides and the worklet
  wasm's file name (`mp_music.wasm`) are unchanged. A rename is left to the
  music engine's redesign.
- **Goldens:** the Music Lab goldens moved from Midnight Playground's
  `parity/golden/music/` to `tests/golden/` here; the lab
  (`games/midnight-playground/tools/music-lab/test/golden.mjs`) writes them
  there. No code change.
