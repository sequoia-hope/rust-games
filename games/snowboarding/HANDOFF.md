# Handoff: before the move into the monorepo (2026-10-18)

Snowboarding: a 1080°-style snowboarding game in Rust + Bevy 0.19.1 + wasm
(local dir and `proj` name `snowboarding`). `README.md` says what the game
is; `CLAUDE.md` holds the working rules.

## Current state

- `main` is clean at the single commit `a3f4fe6`, plus this note. There are
  no stashes, no other branches and no worktrees. **There is no git remote**:
  this directory is the only copy of the history.
- `cargo test --workspace` passes, run on 2026-10-18 for this note: 62 passed,
  with 1 ignored on purpose (`mesh_bytes`, a report, not a check). The
  browser checks (`cargo xtask check --web`) were not re-run for this note.
- The game is whole: four courses, three modes, four riders, the
  Ten-Eighty FM radio (generated music in an AudioWorklet, two announcers
  with recorded lines and captions), touch, gamepad and keyboard, and the
  web build for WebGPU and WebGL2. `dist/web/` holds a release build from
  2026-10-18 (its `build.json` says `"commit": "uncommitted"`, as it was
  built before the first commit).
- `logs/client.jsonl` (ignored, 317 lines) has no real crash: the four
  `crash` and two `error` reports are the stress tools' injected faults
  (`?probe=throw`, "injected: submit threw once"). No tab has `vanished`.

## In progress / waiting

- **Playground Kit adoption has not started.** The kit's hand-off,
  `~/Software/game-engine/docs/handoff-snowboarding.md`, lists what
  `sb_game` swaps for `pk_shell` (`plugins.rs`, `web.rs`, the parsing half
  of `options.rs`, `status.rs`) and what it gains (settings persistence,
  focus pause, pipeline warm-up, render scale, debug overlay, recorder,
  smoke test). This repo's agent applies it; the kit's agent never edits
  this repo. The only kit piece in use today is its patches.
- **Bevy version unification** (an agreed plan, not this repo's work):
  `render` moves from Bevy 0.20 down to 0.19.1 so every game shares one
  Bevy and the kit's patches. Snowboarding is already on 0.19.1 and needs
  no change for it.

## Known issues

- `Cargo.toml`'s header comment still says the patched crates are "under
  patches/"; they are in `../game-engine/patches/` (the `[patch]` table and
  the README are right).
- Everything is checked headless in Chrome; the owner plays on an iPhone
  (Safari), which only Playwright's slow software-GL WebKit approximates
  (see `CLAUDE.md`, "The owner's phone").

## Concerns about the move

1. **Relative paths out of the repo.** These break unless the new layout
   keeps the same neighbours:
   - `Cargo.toml` `[patch.crates-io]`: `../game-engine/patches/{wgpu,
     wgpu-hal,bevy_ui_render}`. Without them it still builds, silently, on
     unpatched wgpu, which brings back the per-draw WebGPU limits query,
     buffers left for the JS garbage collector (the iPhone memory deaths)
     and WebGL2 call count. Check `cargo tree -i wgpu` shows the path after
     the move.
   - The kit's hand-off writes `pk_shell` as `../../../game-engine/crates/pk_shell`
     from `crates/sb_game`.
   - `tools/announcers/compare.py` and `CLAUDE.md`:
     `../midnight-playground/tools/dj-voice/voices` (the voice-likeness check).
   The simplest move keeps `game-engine` and `midnight-playground` as
   siblings of this directory, as the kit's own `HANDOFF.md` also asks.
2. **Cargo workspace.** This is its own workspace with its own
   `[patch.crates-io]`, `rust-toolchain.toml` (1.99.0 + wasm32) and
   `.cargo/config.toml` (the `xtask` alias). Cargo reads patches and
   `.cargo` config only at the workspace root: if the monorepo gets a root
   workspace, exclude this one or merge it deliberately, or a nested
   member loses the patches and `cargo xtask` stops working.
   `wasm-bindgen = "=0.2.129"` must match the installed `wasm-bindgen-cli`.
3. **Absolute paths outside the repo.**
   - The port registry: `~/scripts/launcher.toml` has
     `dir = "/home/sequoia/Software/snowboarding"`. Update it (or re-`proj
     add`), then `proj doctor`. `serve.sh` resolves its port with `proj
     port` from its own directory, so it fails loudly until the registry
     knows the new path.
   - The running server (tmux `web-snowboarding`, `tools/serve.py` on the
     registered port) serves the old directory: stop it and `proj up
     snowboarding` from the new place. `serve.sh` re-points the tailnet
     `tailscale serve --set-path /snowboarding` front on each start.
   - `launch.sh` names its tmux session `snowboarding`; keep it unique in
     the new workspace.
   - Claude's project memory is keyed by path
     (`~/.claude/projects/-home-sequoia-Software-snowboarding/`); it does
     not follow the move.
   - Tooling referenced by `CLAUDE.md` lives outside: the announcers' TTS
     and ASR venvs and models on `/mnt/carson/qwen3-speech-bench/`, and
     Playwright WebKit via `~/Software/synthwave/node_modules/playwright-core`.
4. **What git does not carry** (all ignored): `target/` (43 GB, absolute
   paths inside, rebuild rather than move), `dist/web/` (70 MB, rebuild
   with `cargo xtask web --release`), `node_modules/` (`npm ci`),
   `shots/` (26 MB of pictures) and `logs/client.jsonl` (the phone's crash
   history; copy it by hand if it should survive). Tracked binaries:
   `audio/announcers/*.mp3` (4.4 MB) and `tools/announcers/voices/*.flac`,
   the recorded lines and the reference voices they are cloned from;
   they cannot be regenerated identically, so keep them.
