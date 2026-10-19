# Changelog: the music engine (crates/music)

The music engine is shared by the games here and may be used by games
outside this repo. Every breaking change goes here, with how to migrate;
notable additions too. Newest first.

## Unreleased

### Renamed `music`; stations come from the game (2026-10-19)

Breaking, for every consumer; the first step of the redesign (`PLAN.md`).

- **The package is `music`** (it was `mp_music`): `use mp_music::…`
  becomes `use music::…`, the dependency `mp_music.workspace = true`
  becomes `music.workspace = true`, and the worklet wasm is `music.wasm`.
- **Stations are data.** `radio::STATIONS`, `radio::station` and
  `radio::level_station` are gone: they were Midnight Playground's. A
  `radio::Station` now owns its fields (`String`s, `Vec<(String, f64)>`
  genres) and is read from JSON with `Station::parse_list` (written by
  `Station::list_json`). Migrate: put your table in a JSON file, parse it
  once, and keep your own `station(key)` and level map.
- **`Player::new(seed, sample_rate, stations)`** takes the table; the
  `station` param indexes it. `Player::stations()` reads it back.
- **The worklet ABI** is renamed and takes the table first: `mpm_*`
  became `music_*` (`music_new`, `music_params`, `music_process`,
  `music_free`, `music_stations`, `music_schedule`, `music_text_len`), and
  `music_alloc` + `music_set_stations(ptr, len)` hand it the JSON
  (`music_text` holds the error when it returns -1).
- **One shim for every game:** `web/music-worklet.js`, registering
  `music-radio`, with `processorOptions` `{module, seed, stations}`
  (`stations`: the JSON as a `Uint8Array`). Copy it beside your page with
  `music.wasm`; the games' own copies are gone.
- **Added** `json::parse` (and `Val::get`, `as_str`, `as_f64`, `as_arr`).
- Snowboarding's fork, `sb_music`, is deleted: it now plays this crate
  with its own table.

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
