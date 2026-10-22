---
name: music-tuning
description: >-
  Use this skill when tuning, debugging, or evaluating instruments, patches,
  drum kits, genre profiles, or mix balance in `crates/music` (`synthwave`,
  `outrun`, `chicago`, `deep`, or classic genres). Guides rendering audio clips
  to WAV/MP3, listening to them directly via `read_file`, checking K-weighted
  LUFS balance, updating golden tables when base patches/kits change, and
  rebuilding `apps/music-lab`.
---

# Music Engine Tuning & Audio Verification Skill

Use this workflow whenever modifying DSP (`crates/music/src/dsp.rs`), instruments (`crates/music/src/instruments.rs`), patches/kits (`crates/music/src/patches.rs`, `kits.rs`), or the generator (`crates/music/src/generator/`).

## 1. Upgrade Base Patches, Kits & DSP Directly

- Base patches live in `crates/music/src/patches.rs` (`build()`) and base drum kits live in `crates/music/src/kits.rs` (`KITS`). Improve them directly so both the new generator (`synthwave`, `outrun`, `chicago`, `deep`) and classic genres (`house`, `techno`, `trance`, `dnb`, `garage`, `eurobeat`, `psytrance`, `chicha`, `country`) benefit from every sound-design upgrade.
- When `patches::build()` or `kits::KITS` changes:
  1. Update `LAB_SHA256` in `crates/music/src/patches.rs` and `KITS_SHA256` in `crates/music/src/kits.rs` to match the new canonical table hashes (`cargo test -p music --test patches --test kits` prints the left/right hashes on mismatch).
  2. Regenerate the classic track and render goldens with:
     ```bash
     REGEN_GOLDENS=1 cargo test -p music --test golden_tracks --test golden_renders
     ```

## 2. Render & Listen to Audio Clips by Ear

Never tune blind. Render the target genre and seed to WAV, slice the section of interest (e.g., intro, verse, or chorus) to MP3, and inspect the audio directly with `read_file`:

1. Run the helper script [render-clip.sh](./scripts/render-clip.sh):
   ```bash
   ./.agents/skills/music-tuning/scripts/render-clip.sh <genre> <seed> <start_sec> <duration_sec> <out.mp3>
   ```
   Example (render 35s covering the verse-to-chorus transition of `synthwave` seed 1):
   ```bash
   ./.agents/skills/music-tuning/scripts/render-clip.sh synthwave 1 50 35 /tmp/sw1-chorus.mp3
   ```
2. Call `read_file` on the resulting `.mp3` file to listen to the rendered audio and evaluate:
   - **Register separation:** Bass in `C1–C3` (`MIDI 29..48`), pads/chords in `A3–E5` (`hpf >= 170 Hz`, `sub = 0`), arpeggio in `A3–E5` (5 ascending chord tones, no modulo wrap on triads), lead in `E4–C6`.
   - **Envelope & filter articulation:** ADSR decay/release time constants in `dsp::Adsr::set_all` use `t / 4.0`, so a 400 ms pluck needs `d: 0.40..0.45`, `fd: 0.38..0.42`. Use `slope: Some(12.0)` (2-pole 12 dB/oct ladder tap) for open, airy Oberheim/Juno pads, arps, and leads.
   - **Stereo imaging:** Keep `< 180 Hz` mono on bass (`chorus_hp: Some(180.0)`), use measured Juno chorus (`chorus_ms: Some(0.5)` for I, `Some(1.1)` for II, `chorus_lp: Some(15000.0)`).

## 3. Verify Mix Balance & Tests

1. Run the K-weighted LUFS chorus balance test (`crates/music/tests/generator_balance.rs`):
   ```bash
   cargo test -p music --test generator_balance
   ```
   - In the chorus, relative to `bass` (K-weighted LUFS):
     - `arp` must sit within `-10.0..=0.0 LU`
     - `chords` (pads) must sit within `-16.0..=-3.0 LU`
     - `lead` must sit within `-8.0..=1.0 LU`
2. Run all `music` crate unit, golden, and integration tests:
   ```bash
   cargo test -p music
   ```
3. Rebuild the standalone Music Lab wasm bundle and run its headless browser check:
   ```bash
   ./apps/music-lab/build.sh && node apps/music-lab/check.mjs
   ```
