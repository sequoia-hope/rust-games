# Snowboarding

A 1080°-style snowboarding game in Rust, Bevy 0.19 and WebAssembly. Race a
CPU rider down the mountain, or ride alone against the clock or for trick
points. You can carve, ollie, spin, flip and grab, but you have to land it.

- **Match Race**: one on one against a CPU rider (who rubber-bands a little).
- **Time Attack**: you and the clock, with checkpoint splits.
- **Trick Attack**: points for every landed trick. The Half Pipe is always this.

Courses: **Crystal Ridge** (easy, sunny), **Timber Run** (trees, dusk),
**Avalanche Bowl** (steep, a snowstorm), **Half Pipe** (sunset).
Riders: Kai (all-rounder), Mika (technical), Rob (speed), Jules (big air).

## Controls

| | keyboard | gamepad | touch |
|---|---|---|---|
| steer / spin in the air | ← → or A D | left stick, d-pad | drag on the left half |
| tuck · brake / flip in the air | ↑ ↓ or W S | left stick | drag up / down |
| jump (hold to charge, release to pop) | Space | A | JUMP |
| grab (the stick picks the grab) | Shift or K | X, B, triggers | GRAB |
| pause | Esc or P | Start | II |

Land with the board along your direction of travel (forwards or switch) and
level. If you land sideways or upside down, you crash. Let go of the grab
before you land.

**Landing assist** (the menu's fourth row; on by default once the screen is
touched): just before you touch down with the stick let go, the rider
squares the board up, levels a flip and lets go of the grab. Landings are a
little more forgiving too. It nudges rather than taking over; the CPU riders
never use it.

## Ten-Eighty FM

The game's radio, 108.0, broadcast from a shed at the summit. Its music is
generated as you listen: drum & bass, techno, eurobeat, trance, garage and
house, a new song every few minutes on a schedule kept by the wall clock,
fuller in a run and thinner in the menu. It plays in an AudioWorklet; a
page without one (Safari over plain http only has it on https pages) plays
the same music on the main thread. Its hosts call your runs: **Dani
Reyes** (retired halfpipe pro from New Jersey, loud, five broken
collarbones, a nacho habit) and **Wren Halvorsen** (a New Zealander,
boardercross racer retired early, bone dry, keeps a spreadsheet of every
trick landed this season): old rivals on the tour, friends ever since. They introduce the
course or your rider, react to big tricks, crashes and lead changes, call
the result, and chat in the quiet stretches and the menu. Their lines show
as captions too. The menu's RADIO row picks music and voices, music only,
voices only or off, and the choice is remembered.

The voices were made with Qwen3-TTS from written descriptions
(`tools/announcers/`): `voices.json` holds the candidates and the chosen
ones, `design.py` makes them, `render.py` records every line of
`crates/sb_radio/src/script.rs` into `audio/announcers/`, and `check.py`
transcribes the clips back to catch garbled takes. The voices are kept
unlike midnight-playground's radio DJs, whose pipeline this borrows:
`compare.py` measures that with Qwen3-TTS's own speaker embedding. Edit a line, run
`cargo run -p sb_radio --example script > tools/announcers/lines.json`
and `render.py`, and only that line is recorded again.

## Running it

```sh
cargo xtask web --release     # build both web backends into dist/web/
proj up snowboarding          # or ./serve.sh: serves on the registered port
cargo run --release -p sb_game -- --course timber --go   # natively
cargo run --release -p sb_sim --bin sb-sim -- avalanche race   # headless CPU run
node tools/shot.mjs --query "course=crystal&go=1&auto=1&skip=25" --name crystal   # a picture, into shots/
cargo xtask check             # the test suite; --web adds the browser checks (server up)
```

URL options: `?course=crystal|timber|avalanche|pipe&mode=race|time|trick&rider=0..3`,
`go` (skip the menu), `assist=0|1`, `radio=all|music|voices|off`, `auto` (the player's
rider rides itself), `skip=<s>` (fast-forward the first run), `touch=1`, `backend=webgl2`,
`dpr=<n>` (the most pixels per CSS pixel the canvas draws: 1.5 on a touch screen, 2 elsewhere).

On a phone, open the https address (`proj list` shows it:
`https://<machine>.<tailnet>.ts.net/snowboarding/`): over plain http, Safari
gives the page neither WebGPU nor AudioWorklet.

## Tests

`cargo xtask check` runs everything that needs no browser: the simulation's
tests (including a seeded masher of every control on every course and mode
that must never go NaN, fall through the snow or leave the mountain), the
music engine's, the announcers' director, and the whole game headless (no
window, no GPU; `crates/sb_game/src/tests.rs`). The headless tests drive
every screen change by key and touch: menu settings (each course change
rebuilds the world), starting, pausing, restarting, the results and quitting,
plus three thousand frames of random key mashing. After every frame they
check what must always hold, such as one HUD in a run, none in the menu,
every rider drawn, no NaN, and the world built exactly once. A separate
test (`crates/sb_game/tests/leak.rs`) counts the allocator's bytes and the
glyph atlases across twenty runs.

`cargo xtask check --web` also builds the release and, in headless Chrome on
the GPU, drives the real page:
- `tools/stress.mjs` makes seeded random screen changes on WebGPU, on
  WebGL2 and with touch at phone size. It fails on any error, panic, frozen
  frame or a heap that keeps growing.
- `tools/heap-probe.mjs` measures the heap across rides, restarts and
  course changes.
- `tools/radio-check.mjs` confirms the music sounds and the announcers
  talk, on a secure page and on a plain-http one.

Crashes show a WIPEOUT screen with a reload button, rather than a frozen
picture. The page also reports to the server, which appends to
`logs/client.jsonl` and prints a line: what it started on (browser, backend,
pixel ratio, whether it had AudioWorklet), every crash with the console's last
lines, and a tab that vanished (the page is killed without leaving, as a
phone does when it runs out of memory), with the screen and heap it was last
seen on.

## Layout

- `crates/sb_sim`: the game with no engine. It holds course generation
  (a winding centreline with a height function over (s, u)), rider physics
  at a fixed 1/120 s tick, trick naming and scoring, CPU riders, and the race
  session.
- `crates/sb_radio`: Ten-Eighty FM's announcers: the script, and the
  director that decides from the run's events when they speak and what
  about. Engine-free, tested on its own.
- `crates/sb_game`: the Bevy client. It handles terrain meshing from the
  sim's height function, the sky, trees and gates, the rider's IK-posed
  body, the chase camera, snow particles, the HUD and menus, input
  (keyboard, gamepad, touch), captions, and Web Audio sound (effects, the
  music worklet and the announcers' clips, ducking the music under them).
- `xtask`: `cargo xtask web` builds the WebGPU and WebGL2 wasm files,
  wasm-opt, precompression and `build.json`.

## Borrowed from midnight-playground

The workspace shape (an engine-free sim crate, one Bevy crate, xtask), the
pinned Bevy 0.19.1 with its lean feature set and `ClientPlugins` (no 2D
sprites), the patched crates, now Playground Kit's and shared by every game
(`../game-engine/patches/`: wgpu's cached WebGPU limits and its
destroy-on-drop, wgpu-hal's GL bind cache, bevy_ui_render's empty-draw
skip; each has a `PATCHED.md`), the web build pipeline (two backends, staging swap,
wasm-opt, brotli/gzip), the loader page (backend choice, progress,
gesture bridge, WebGPU→WebGL2 fallback), `tools/serve.py` (the build laid
over the root, precompressed files, no-store), the registry-aware
`serve.sh` with its tailnet https front, the Rajdhani fonts, the
headless-Chrome GPU flags, and the radio: the shared music engine (`crates/music`, with a station of our
own, `crates/sb_game/stations.json`) and its AudioWorklet shim, and the DJ voice
pipeline (`tools/dj-voice/`'s design and render scripts, adapted for two
voices in conversation).
