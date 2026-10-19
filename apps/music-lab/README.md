# Music Lab

The music engine's workbench (`crates/music`, Rust): make a track from a
genre and a seed, play it, and take it apart — its sections, its parts
(mute, solo, live patch edits), every instrument on a keyboard, the drum
machine on pads, the mix, a spectrum, offline measurements. It is the JS
Music Lab's page (Midnight Playground's `tools/music-lab.html`, now frozen)
pointed at the Rust engine: what you hear is what the games' radios play.

| What | How |
|---|---|
| Serve it (the registered way; `proj up music-lab`) | `./serve.sh` |
| Build it into `dist/` | `./build.sh [--release]` |
| Check it in headless Chrome (no server) | `./build.sh && PK_PROJECT_ROOT=../../games/midnight-playground node check.mjs` |
| Publish it | `pages.sh`, run by the site's workflow (`site/games.toml`) |

- `index.html`, `main.js`: the page. It runs the wasm on the main thread
  too, for the tables (genres, patches, kits) and to make the same track the
  audio thread plays, to show it.
- `lab-worklet.js`: the audio thread: `music.wasm`'s lab session
  (`crates/music/src/lab.rs`), driven by the page's messages as JSON.
- `knobs.js`: which knobs each instrument and drum voice shows.
- `measure.js`: offline renders and their numbers.

A URL can pick the first track: `?genre=techno&seed=42`.
