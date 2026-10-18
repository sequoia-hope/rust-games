# Recommendations for Midnight Playground's roadmap

Read with `docs/vision/ROADMAP.md` in the game's repository (M12 to M22).
These are reasoned from the audits in `INVENTORY.md` and the ecosystem
check in `ECOSYSTEM.md`.

## 1. Where the split sits in the roadmap

Put the split (this plan's phases 1 to 5) **between M11 and M12** as its
own milestone, "M11.5: the toolkit", and let M12 (sound) land in the
library directly. Reasons:

- Sound is the next milestone and most of what it needs (the facade, the
  mixer, the music engine, the exhaust model, the voice pipeline) is
  already generic; splitting it first means M12's work is written once
  for both games.
- Snowboarding exists now and is copying by hand. Every week before the
  split adds drift.
- The split has no design risk in phase 1 and well-understood seams after;
  it is not research.

Keep M14's "weather and light as simulation state" and M15's
"cameras, replays, photo mode, ghosts" in mind during the split: both are
cheaper in the library (section 3).

## 2. Features the roadmap should add, and why

Ordered by value over cost.

1. **Replays and ghosts (M15, pull forward).** Determinism plus the
   recorder's `InputLog` make a replay a file of inputs and a seed. A
   ghost is a replay drawn as a second car with no collision. Both come
   nearly free once `pk_lockstep::Recording` exists (phase 1), and both
   serve every game: a snowboard run against your best, a bike time trial
   against a friend. The library's `Recording` should be the save format
   from day one.
2. **Snapshots in the netcode (M10/M11 gap).** There is no snapshot
   message, so a desynced client resimulates from tick 0 and a late joiner
   cannot. Giving `SimState` serde (behind a feature so the simulation
   crates stay dependency-light) and adding `NetSim::{snapshot, restore}`
   closes MULTIPLAYER's 9.2 and makes car meets and spectating (M21)
   possible. Do it while `pk_net` is being made generic (phase 3), when
   the protocol version is bumping anyway.
3. **Photo mode = the viewer in the pause menu (M15).** The level viewer
   already has free, orbit and flight cameras, hide groups, and links that
   carry a pose. Opening it from pause over a frozen simulation is a day's
   work once `ViewerPlugin` exists, and gives every game a photo mode.
4. **A world clock as simulation state (M14).** Time of day and weather
   belong in `SimState` so rain changes grip identically on every device.
   The sky already samples keys by a scalar; make that scalar the clock
   and the library gets day/night for free. Snowboarding's dusk and
   snowstorm courses want the same.
5. **Telemetry export (M17).** `RecordLine` already carries per-second
   frames; add a per-tick telemetry channel (speed, inputs, and with sim
   handling tyre loads and slips) and a CSV exporter shaped for MoTeC
   i2's import. Cheap in `pk_shell`'s recorder, wanted by the owner, and
   useful to tune the snowboard's rider physics too.
6. **Favourites compiled in (R1.5).** `favourites.json` is written by the
   Radio page but the rotation and blocklist of D1154 are not built. It
   is a small generator step in `pk_music`'s build and raises the average
   song at once.
7. **A Whisper verify script for the voice pipeline.** D1184 and D1221
   did the check by hand. A `verify` command that writes a score per clip
   into `index.json` is an hour and makes the pipeline usable by others.
8. **Plaits as an instrument (M12).** sound.md asks for it; the Rust port
   exists (no_std, MIT). Adding it to `pk_music::synth` is cheaper than
   hand-building more voices, and it brings speech, modal and wavetable
   models.
9. **NPC barks from the DJ director (M18 and the Zelda-like).** The
   director's rules (gap, chance, no-repeat history, quiet predicate,
   topic from id) are exactly an NPC bark system. Design `pk_voice` with
   a `Speaker` that is not a radio station, and the delivery driver, the
   garage mechanic and the race marshal can talk with the same pipeline.
10. **Accessibility tokens in the theme.** One-handed play, a
    colour-blind-safe HUD and auto-steer are promised for every milestone;
    putting the palette and the "assist" settings in `pk_ui::Theme` and
    `pk_input` makes them a property of every game, not a per-game chore.

## 3. Things to stop or change

- **Retire `desktop/` (Electron), `play.sh` and `og-image.mjs`'s JS
  path.** The native binary replaced Electron at cutover (M9.3); the
  preview image should render from the Rust build.
- **Drop the bevy_ui_render patch now** and filter the Chrome warning in
  the e2e test; file the one-line upstream PR. Two fewer patched crates to
  carry per Bevy upgrade is worth more than a clean console.
- **Pin and check wasm-opt** as wasm-bindgen is pinned and checked.
  binaryen 116 is from 2023.
- **Read `CHROME_PATH` everywhere**; four tools hardcode the binary.
- **Move the `web_extra` port lookup into `proj`** (`proj port --extra
  signal`), so `signal.sh` stops importing the registry module.
- **Measure `StandardMaterial` against `three_std` on the reference
  phones** once, so the choice between Bevy's shading and the parity
  shading is a number, not a guess, for the next game.
- **Fix snowboarding's determinism claim** (platform trig in `noise.rs`)
  when it adopts `pk_math`; until then its courses are not the same on
  native and web.

## 4. Upstream work to schedule

From ECOSYSTEM.md section 3, in order of payoff: the wgpu-hal GL bind
cache PR (new, measured, three parts); the comment and offer on
wgpu#10550; the bevy_ui_render one-liner; the empty-submit skip; then
issues for the web pixel ratio, custom-material instancing, the shared
globals uniform, the transparent sort key and the fixed shadow box.
Each removes a patch or a hack the toolkit would otherwise carry across
every Bevy upgrade.
