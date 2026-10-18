---
name: engine-sound
description: Make an engine exhaust sound preset for the Playground Kit physical exhaust model (today Midnight Playground's mp_exhaust and the Engine Sound Lab): cylinders, firing order and crank angles, bank gain, header lengths, pipe, muffler sections, intake, pops, turbo; a stock-versus-aftermarket pair; listen in the lab; keep the goldens and the per-sample cost. Use when asked to add a car's engine sound, an exhaust option, tune how an engine sounds, or fix a lag or tone complaint about the exhaust.
---

# Engine sound: a preset on the physical exhaust

Today's code is Midnight Playground's (`~/Software/midnight-playground`;
paths relative to it); `mp_exhaust` becomes `pk_exhaust` in kit phase 4
with presets as files. The model and the lab do not change.

## Read first

- `docs/vision/sound.md` section 2: the model (combustion pulses, header
  waveguides, collector and pipe, muffler sections, intake, mechanical
  noise, overrun crackle), the small-speaker rules (2.2), the cost (2.3).
- `crates/mp_exhaust/src/lib.rs`: the signal flow diagram in the header,
  `Params` (every knob with its lab name), `Response` (`LAB` vs `GAME`),
  `Engine::new(params, state, seed, sample_rate)`.
- `crates/mp_exhaust/src/presets.rs`: `ORDER` and `preset(key)`, each with
  the comment that explains its firing order. Read `crossV8` (unequal cast
  manifolds: the burble), `flatV8` (equal 4-into-1: the scream), `v6tt`
  (turbo), and the pair `audiV8` and `audiV8awe`.
- `tools/engine-lab/presets.js` (the same numbers in camelCase, plus
  `name`, `gameCar`, `revUp`, `revDown`), `tools/engine-lab/worklet.js`
  (the model, the oracle), `drive.js` (the driver: manual, rev, drive).
- DECISIONS D1110 (the model in the game; the lab stays the oracle) and
  D1201 (the game's `Response::GAME`: the gas warming at 12/s and the
  compressor's release at 0.0007, against the lab's slow tails).

## What a preset is

A `Params` (the lab's `P`). The parts, in signal order:

| Group | Knobs | What it does to the sound |
|---|---|---|
| Combustion | `idle`, `redline`, `events` (bank, header) in firing order, `angles` (crank degrees, `None` even), `bank_gain`, `tau_deg`, `sharp`, `variation`, `jitter_deg`, `misfire`, `turb_noise` | the pulse train: its unevenness is the character |
| Headers | `headers` (metres, one per event or one shared), `header_scale`, `header_spread`, `header_r`, `header_rv`, `header_loss_hz` | unequal lengths burble, equal lengths scream |
| Collector and pipe | `pipes` (1 or 2), `xpipe`, `pipe_len`, `pipe_r`, `pipe_rc`, `pipe_loss_hz` | the drone and its bore |
| Muffler | `muffler_lp`, `sections` as `[Hz, Q, dB]`, `muffler_tune`, `muffler_gain` | the resonances a phone speaker actually plays |
| Tail and body | `tail_len`, `tail_r`, `rad_hz`, `rad_low`, `body`, `drive` | the tailpipe radiation and the cabin |
| Intake, mech, pops | `intake`, `intake_len`, `mech`, `pops`, `pop_hz`, `imbalance` | the induction roar, ticks and whine, overrun crackle |
| Turbo | `turbo`, `bov`, `anti_lag` (`boost_target` maps rpm and throttle to boost) | whistle, hiss, blow-off |
| Output | `level`, `width`, `comp_thresh`, `comp_ratio`, `makeup` | the bus |

## Procedure for a new engine

1. **Decide the engine on paper**: cylinders, V angle, firing order as a
   bank sequence (GM V8 1-8-4-3-6-5-7-2 is L R R L R L L R), even or
   uneven firing (a 90° V-twin or an odd-fire V6 needs `angles`), stock
   manifolds or headers (cast manifolds are unequal and short; tubular
   headers equal and long), one or two pipes, X-pipe, turbo. Write the
   comment that explains the order; every preset has one.
2. **Start from the nearest preset** in `presets.js` and change the
   combustion and header groups first; listen; then the pipe and muffler;
   then intake, pops and turbo; `level` last, matched to the others.
3. **Listen in the lab** through the registered server:

   ```bash
   proj up midnight-playground      # then /tools/engine-lab.html
   ```

   **B · new model** against **A · current game engine** (the `gameCar`
   it stands in for), **Manual** sliders, **Auto rev** (blips, limiter,
   overrun), **Drive (a pull)** through the gears. **Copy preset JSON**
   gives the numbers back. Listen on a phone speaker too (2.2: the
   fundamental is not there; the muffler sections are what is heard).
4. **A stock-versus-aftermarket pair** is two presets sharing combustion
   and headers: the cat-back changes `pipe_loss_hz` up (less loss),
   `pipe_r` and `tail_len` (bigger bore and tips), a lower first
   `sections` entry a few dB up (the drone), `pops` up a little. See
   `audiV8` and `audiV8awe`: the `awe` one is built as `audi_v8()` with
   overrides. In the game this is a garage choice.
5. **Port the numbers** to `presets.rs` (snake_case, `vec!`s, the same
   comment) and add the key to `ORDER` in the lab's order. Then:

   ```bash
   node tools/engine-lab/test/golden.mjs          # regenerates parity/golden/exhaust/
   git diff --stat parity/golden/exhaust/         # only the new preset's file should appear
   cargo test -p mp_exhaust                       # the golden test: 50 ms RMS to 0.1 %, samples to 1e-5
   cargo run --release -p mp_exhaust --example throttle_step -- <key>   # the response tails (D1201)
   ```

6. **Wire the car**: `mp_audio::game::exhaust` maps a car to a preset by
   the lab's `gameCar`; the game's rpm range (800 to rpmMax) is mapped
   onto the preset's `idle` to `redline`. The game's engines are made
   with `Response::GAME`; the goldens use `Response::LAB`. Do not change
   `LAB`.
7. **Build the worklet wasm** and hear it in the game: `cargo xtask web`
   puts `mp_exhaust.wasm` and `exhaust-worklet.js` in `dist/next/`; the
   native node runs the same engine. Drive it through `proj up`.
8. **DECISIONS** entry: the engine, the pair, what was listened to on
   which speaker, and the cost measurement.

## The cost rule

The model runs **per sample** on the audio thread: each cylinder is a
waveguide, each pipe another, every section a biquad. Keep `MAX_CYL` (16)
and `MAX_POPS` (10) as the bound; no allocation in `process`. The worklet
wasm is built at `opt-level = 3` (the `release` profile even in a debug
web build, as the music worklet is): never let it inherit `opt-level = "s"`.
Measure before adding a stage: the lab's **Measure all presets** gives
render time per preset, and a native `cargo bench`-style loop over
`Engine::process` for 10 s of audio at 48 kHz should stay far under 10 s.
Rival engines stay on the cheap wavetable voice (2.3).

## Common mistakes

- Editing `presets.rs` and not `presets.js` (or the reverse): the golden
  test fails on purpose. The lab is the oracle; change it first.
- An even `angles: None` on an engine that is not even-firing, or a
  `headers` list shorter than `events` when the lengths were meant to be
  per cylinder (one entry means one shared length).
- Tuning for headphones only. Check a phone: psychoacoustic bass and the
  muffler sections carry the rumble there.
- Making the engine answer the pedal slowly (D1201): the model's warm-gas
  glide and the compressor release are the lag the owner hears, not the
  input path. `Response::GAME` fixes it; measure with `throttle_step`.
- Reaching for `pk_math`'s kernel: audio is not simulation; the platform
  `f64` functions are correct here (`check-deps` has `exact_math: false`
  for the audio crates).
- Starting another server to open the lab, or hardcoding a port.
