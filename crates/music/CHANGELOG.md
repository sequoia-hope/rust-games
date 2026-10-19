# Changelog: the music engine (crates/music)

The music engine is shared by the games here and may be used by games
outside this repo. Every breaking change goes here, with how to migrate;
notable additions too. Newest first.

## Unreleased

### The new generator (2026-10-19)

Additions only; the classic generator, the radio and the worklet's radio
exports are unchanged.

- **`score`**: a song as per-instrument layers (`Song`, `Layer`, `Clip`,
  `Pattern`, `Note`, `Automation`, `Fx`, `Section`): clips on phrase
  boundaries, patterns of any length, notes at fractional 16ths with their
  own velocity, automation lanes, a sidechain depth and an energy
  threshold per layer. `Song::summary()` is its JSON for a page.
- **`song::SongPlayer`**: plays a `Song` on the lab's instruments, kits and
  mix (per-layer pump, bus filters, risers).
- **`generator`**: `generate(genre, seed, &Settings)` for the profiles in
  `generator::profile` (`synthwave`, `outrun`, `chicago`, `deep`), built
  from `research/RULES.md`; `generator::theory` (modes, numerals,
  voicings) and `generator::surprise` (the PPM expectation model, IC and
  entropy scores, the LHL and Witek syncopation indices).
- **Lab:** the `song` message and `lab::song_json`; worklet export
  `music_song`; `music_tables` lists the new generator's genres as
  `generators`.
- **`examples/render.rs`:** any song, new or classic, to a WAV file.

### The lab on this engine (2026-10-19)

Additions only.

- **`lab`**: a [`lab::Session`] is an engine driven by the Music Lab page's
  messages (JSON: track, play, stop, patch, kit, audition, on, off, hit,
  energy, mix, mute, solo); `lab::generate(genre, seed, dejavu)` and
  `lab::tables_json()` serve the page. The page is `apps/music-lab/`.
- **`Engine`**: the lab keyboard and pads, which the port had left out:
  `set_audition`, `note_on`, `note_off`, `hit`, and `set_kit_tweak`
  (per-lane kit voice edits). A game that never calls them renders as
  before.
- `Instrument::release(midi)`; `Lab::from_val`, `OscSpec::from_val`,
  `Op::from_val` (a patch from its JSON).
- **Worklet ABI:** `music_tables`, `music_generate`, and the lab node's
  `music_lab_new`, `music_lab_msg`, `music_lab_process`, `music_lab_poll`,
  `music_lab_free`.

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
