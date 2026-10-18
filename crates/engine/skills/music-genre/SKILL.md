---
name: music-genre
description: Add a music genre grammar to the Playground Kit composer (today Midnight Playground's mp_music and the JS Music Lab): specify the vocabulary (kit, progressions, motif cells, form, energy mapping), build it in the lab first, port it, regenerate the goldens, give a radio station its weights, and listen on the Radio page. Use when asked to add a genre, a station's sound, a new instrument for a grammar, or to fix a grammar bug reported from a Reject note.
---

# Music genre: a grammar on the composer

Today's code is Midnight Playground's (`~/Software/midnight-playground`;
paths relative to it). `mp_music` becomes `pk_music` in kit phase 4, the
procedure unchanged.

## Read first

- `docs/vision/sound.md` 3.2 (arrangements that evolve), 3.4 (what makes a
  generated track grab: the composer's answers), 3.5 (three genres
  specified; 3.5.1 country and 3.5.3 chicha are the worked examples: the
  spec shape, then what was built and what was deferred).
- `crates/mp_music/src/lib.rs` (the port rules), `compose.rs` (keys,
  Euclid, motifs, pickups, déjà vu, arrangement), `genres.rs` (one grammar
  end to end: read `chicha` or `country` whole), `seq.rs` (the song format
  and the lab extensions: `auto`, `lay`, `'n'`, `'p'`, `swing8`),
  `engine.rs` (the mix), `radio.rs` (`Station`, `STATIONS`, blocks on the
  wall clock, favourites).
- `tools/music-lab/{compose.js,gen.js,seq.js,engine.js,instruments.js}`:
  the lab is the oracle. `tools/music-lab/test/golden.mjs` writes
  `parity/golden/music/{tracks,events,renders}.json`.
- DECISIONS D1150 (the port and its rules), D1151 (stations on the wall
  clock), D1152 (three genres specified), D1153 (fixing the oracle by
  numbers), D1154 and D1155 (favourites, the Radio page's Keep/Reject),
  D1183 (country, start to finish).

## The architecture in one paragraph

A song is `(genre, seed)`: the grammar is deterministic, so the pair names
the same song forever and everywhere. A grammar (`pub fn <genre>(seed,
dejavu) -> Track`) draws from one mulberry32 stream in a fixed order and
emits the song format the sequencer plays. A station is weights over
genres plus a seed; its programme is a pure function of (station, wall
time) cut into twenty-minute blocks. The goldens hash every grammar's
tracks over a few seeds, so **a draw moved or skipped is a different
song and a failed golden**. The lab (JS) and the crate (Rust) must agree
exactly on tracks and events, and to a tolerance on renders.

## Procedure

1. **Specify the vocabulary** in `docs/vision/sound.md` 3.5, to the depth
   of 3.4, before code. Headings the owner expects, in this order:
   - **The sound**: two to four sub-styles picked per seed, with real
     references and bpm ranges.
   - **Instruments**: each as parameters of an existing instrument (the
     plucked string, the FM organ, the bowed voice) or a named new one;
     the **kit** (which hits, which new hit types).
   - **Rhythm**: the patterns as 16-step strings (`x.x.X.x.`), Euclidean
     figures, the fill into the chorus, swing.
   - **Harmony**: scale, the progression family (`PROGS.<genre>`), the
     chord extensions ('dom' and so on).
   - **Melody**: scale degrees to `avoid`, who states the hook where, the
     answer (a third up), the thinned statement.
   - **Form**: the sections in bars; a track about four minutes.
   - **Energy**: what drops out first as the live energy falls (the band
     thins; a filter closes; the drop is silent on a kit with no snare).
   - **What the composer lacks**: the extensions this grammar needs.
   - **What makes it grab.**
2. **Build in the lab first** (`tools/music-lab/gen.js`, a new entry in
   `GENRES` with a salt; new instruments in `instruments.js`, kits in
   `KITS`, progressions in `compose.js` `PROGS`). Listen on
   `/tools/music-lab.html` through `proj up midnight-playground`. Run
   `npm run test:lab`.
3. **Regenerate the goldens** and check the old genres did not move:

   ```bash
   node tools/music-lab/test/golden.mjs
   git diff --stat parity/golden/music/     # only the new genre's entries should change
   ```

   If an existing genre's hash changed you reordered a shared draw. Undo
   it or record a grammar version (radio.md 8: seeds then mean different
   songs; favourites break).
4. **Port to Rust**, line for line, the draws in the JS order including
   those inside object literals (`genres.rs` header). Tables (BPATCH,
   KITS) are generated, not transcribed:

   ```bash
   node tools/music-lab/test/dump-tables.mjs | python3 tools/music-lab/test/gen-rust-tables.py
   cargo test -p mp_music            # golden_tracks, golden_events, golden_renders
   cargo xtask check-deps            # mp_music: no clock, no rand, no engine
   ```

   Audio is not simulation: `mp_music` uses the platform's `f64` math, so
   renders agree to a tolerance; tracks and events must be exact.
5. **A station's weights**: add or edit a `Station` in
   `crates/mp_music/src/radio.rs` (`key`, `name`, `freq`, `dj`, `genres`
   as `(key, weight)`, `seed`), in dial order; the index is the radio
   node's `station` param, so appending at the end keeps the dial stable.
   Map levels to it in `level_station`. A DJ is the `voice-cast` skill.
6. **Listen on the Radio page** through the registered server:
   `/tools/radio.html?station=<key>` plays the live schedule; **Skip song**
   (S) runs the clock ahead; **Keep** (K) and **Reject** (J) with a note
   POST to `crates/mp_music/favourites.json` only where the GET carries
   `X-Favourites: writable` (the dev server, not Pages). A song's link
   carries `wall=` so a note can be checked later. The tuner renders with
   `cargo run --release -p mp_music --example tuner -- out.wav`.
7. **Rejects are the backlog.** A Reject with a reason ("the bass fights
   the kick", "the chorus never arrives") is a grammar bug; fix it in the
   lab by numbers first (D1153: render offline, check every note against
   the chord under it), then port, then goldens.
8. **Verify** before claiming done: `cargo test -p mp_music`, `npm run
   test:lab`, `cargo xtask check-deps`, the page plays the new genre on
   its station. **DECISIONS** entry: the spec, what was built, what was
   deferred and why (D1183's shape).

## Common mistakes

- Making a lane step by step from independent probabilities. Use cells
  chosen once per bar, Euclidean figures, 4-bar phrases with a turnaround.
- No hook, or a hook realised over progression A and played over B
  (D1153): give `hooks` the bars it plays over (`nbars` for the 12-bar).
- A bass that only plays the current chord. Use 'n' (next root) pickups.
- Random walks between blocks. Variations are made from the core once;
  each block plays the core or a variation and sections open on the core.
- Landing a progression on the diminished degree with a fixed fifth in
  the bass (`no_dim`; 'f' is the chord's own fifth).
- A JS object literal that writes a key twice (D1153: the `r` ratio and
  release). The Rust port copies the evaluated object, so a bug in the
  oracle ports faithfully; fix the oracle, regenerate, then port.
- `HashMap` anywhere in `mp_music`: iteration order is the song. Pairs in
  insertion order (`track::Pairs`).
- Starting a second server to hear the page, or writing a port anywhere.
- Hand-editing `favourites.json`'s shape: the page and `serve.py` upsert
  by `(genre, seed)`, one object per line, sorted.
