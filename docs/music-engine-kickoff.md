# Music Engine — Kickoff Prompt

Oct 18, 2026 · @Sequoia

Run this **after** the workspace reorganization (see the Game Workspace Reorganization doc) is finished. Start a Claude Code session at the workspace root and paste the prompt below. The music engine lives in `crates/music`.

## Context

My hobby games now live in one Rust Cargo workspace: `crates/engine` (shared game engine), `crates/music` (procedural music generator), and games under `games/` (Midnight Playground, a snowboarding game). Read the root CLAUDE.md first and follow its rules (keep the whole workspace building and tests passing, work in a git worktree on a branch). The music code was moved into `crates/music` mostly as-is from Midnight Playground. Songs are currently split into 10–15 sections with callbacks and variants. It works, but it sounds much weaker than human-made synthwave and EDM. Your job is to turn it into a real, research-backed music engine.

## Phase 1 — Clean up the music crate

- Survey `crates/music` and how each game uses it. Document what's game-specific, what's shared, and what's tangled together.
- Design a clean public API with per-game configuration, and update each game to use it.
- Keep current behavior working first (regression: same seed → same output where practical) before adding features.
- Make the engine usable on its own, outside any game (needed for the Phase 4 player).

## Phase 2 — Research (use several parallel research agents)

Build the engine on research, not on music theory from memory. Save findings as markdown with citations in `research/`.

- **Genre history and production practice** for electronic dance music: house (Chicago, deep, French), techno (Detroit, Berlin, minimal), drum and bass / jungle, trance, synthwave/outrun, electro, ambient/downtempo, plus anything else that turns up. For each genre cover origins, key artists and landmark tracks, tempo range, drum patterns, bass style, signature synths and sounds, typical harmony, and typical arrangement and energy arc.
- **Arrangement and layering**: how tracks are built from instruments that enter and exit on phrase boundaries (8/16/32 bars), DJ-friendly intros and outros, builds, breakdowns, drops, risers and fills, and how these conventions differ between genres.
- **Corpus and cognition research**: harmony corpus studies (e.g. de Clercq & Temperley, Hooktheory data), expectation and surprise (Huron's *Sweet Anticipation*, Pearce's IDyOM, Cheung et al. 2019 on uncertainty and surprise in chord pleasure), and large-scale studies of how music has evolved (e.g. Mauch et al. 2015). Look specifically for work on electronic and dance music, not just pop.
- Distill the research into concrete, implementable rules and parameters per genre.

## Phase 3 — Generator upgrades

- **Per-instrument layering**: instead of whole-song section blocks, give each instrument its own timeline. Elements enter, drop out, and vary independently, aligned to phrase boundaries, and interact (e.g. kick alone → hi-hats → bass → airy pads → breakdown → drop).
- **Genre profiles** driven by the research: tempo, drum grammar, bass grammar, harmony palette, sound design presets, arrangement templates.
- **Surprise-aware generation**: generate candidate phrases and progressions, score their predictability and surprise, and prefer the middle range — neither boring nor chaotic.
- Keep the RNG making choices *inside* musical structure, not replacing it.

## Phase 4 — Listening and learning UI

- A player where I can generate tracks by genre and settings, listen, and regenerate.
- **Playlist builder**: add tracks, reorder them, add more, save and load playlists. The goal is to assemble an "album" of EDM I like.
- **Research notes in the UI**: for each genre and track, show short research-backed notes on why it sounds the way it does, where the sounds came from, and the history, with links to sources. This is a learning tool as much as a generator.
- A viewer that shows per-instrument layers over time (not just section blocks), so continuity across sections is visible.

## Working style

- Start by writing a plan to `PLAN.md` and check in with me before the big refactors.
- Prefer research-backed decisions and cite sources in code comments or docs where it matters.

---

## Appendix: what the reorganization found (2026-10-18)

Written by the reorganization's session as input for this one; the prompt
above is the owner's.

- `crates/music` is the Music Lab (`games/midnight-playground/tools/music-lab/`,
  JS) ported line for line, checked against goldens in
  `crates/music/tests/golden/` (`tools/music-lab/test/golden.mjs` regenerates
  them). Its package name is still `mp_music`.
- Snowboarding carries a fork, `games/snowboarding/crates/sb_music`: the
  same engine, with its own `radio::STATIONS` (one station, Ten-Eighty FM);
  its docs and one test differ.
- The station table is compiled into the AudioWorklet wasm (`src/wasm.rs`
  reads `radio::STATIONS`), so a shared engine needs a way for each game to
  supply its stations (a per-game worklet crate, data passed at start-up, …).
- Consumers: Midnight Playground's `mp_audio` (the native worklet processor,
  and the web worklet shim `crates/mp_audio/web/music-worklet.js`) and
  `mp_game` (station UI, store); Snowboarding's `sb_game` (through
  `sb_music`) and its `crates/sb_game/web/music-worklet.js`; Midnight
  Playground's `tools/radio*`, `tools/serve.py` and e2e tests, which load
  `mp_music.wasm` by name.
- `crates/music` is a shared crate: the root `CLAUDE.md` rules for those
  apply (a `CHANGELOG.md` entry for every breaking change, no reference to
  a game from the crate, the whole workspace green), and so does
  coordinating through `status/` while you change it.
- The Phase 4 player can go on the public site,
  https://sequoia-hope.github.io/rust-games/, like the games: give its
  folder a `pages.sh` and add it to `site/games.toml` (root `CLAUDE.md`,
  "The public site"). Locally it is viewed only through a registered server
  (`proj`), never an ad-hoc one.
