# Architecture

How the crates and games in this repo fit together. The rules for working
here are in the root `CLAUDE.md`; each crate's and game's own docs go
deeper.

## Dependency direction

```
games/midnight-playground (mp_*)      games/snowboarding (sb_*)
        │        │                            │
        │        └──────────────┐             │ (patches only, today;
        ▼                       ▼             ▼  pk_shell adoption planned)
 crates/music (mp_music)   crates/engine (pk_*) ──► crates/engine/patches
                                                    (wgpu, wgpu-hal,
                                                     bevy_ui_render)
```

Games depend on the shared crates; the shared crates never depend on a
game. `deps.toml` holds each crate's allowed workspace dependencies and
banned dependency sets, and `cargo xtask check-deps` enforces them.

## The shared crates

**crates/engine — Playground Kit**, carved from Midnight Playground so its
sister games build on one library instead of copies (`docs/PLAN.md` there):

| Crate | Layer | What |
|---|---|---|
| `pk_math` | 0 | the math kernel (libm with ECMAScript semantics), rng, noise, vectors: deterministic on every platform |
| `pk_lockstep` | 0 | lockstep simulation traits and helpers: recording, hashing, fuzz |
| `pk_canvas` | 0 | Canvas 2D on tiny-skia with runtime fonts |
| `pk_shell` | 1 | the Bevy client shell: plugins, params, load status, settings store, web glue, warm-up, debug overlay, recorder |
| `pk_xtask` | tools | the library behind each game's `cargo xtask`: web build, size, profiling build, check-deps, imgdiff |

Midnight Playground uses all of them. Snowboarding uses the patches today;
its adoption of `pk_shell` is planned (`crates/engine/docs/handoff-snowboarding.md`,
now simply a task for whichever session does it).

**crates/music — the music engine** (package `mp_music`): procedural music
from the Music Lab, per sample, also built as a standalone AudioWorklet
wasm. Midnight Playground's radio uses it; Snowboarding has a fork
(`sb_music`) awaiting the redesign (`crates/music/CLAUDE.md`).

## The games

- **games/midnight-playground** — the street racer: simulation crates
  (`mp_track`, `mp_levels`, `mp_vdyn`, `mp_sim`), world generation
  (`mp_scene`, `mp_worldgen`, `mp_builder`, `mp_atlas`), audio
  (`mp_exhaust`, `mp_audio`), networking (`mp_net`, `mp_host`) and the Bevy
  client `mp_game`; also the legacy JS game (frozen). Docs:
  `docs/rust-port/SPEC.md`, `ROADMAP.md`, `DECISIONS.md` there.
- **games/snowboarding** — the 1080°-style snowboarding game: `sb_sim`
  (simulation), `sb_radio` (announcers), `sb_music` (music fork), `sb_game`
  (Bevy client).

## One workspace

The root `Cargo.toml` is the only workspace manifest, because Cargo honours
`[patch]` and `[profile]` only at the root: every game gets the same patched
wgpu and the same profiles. One `Cargo.lock` means every game builds on the
same dependency versions, and `cargo check --workspace` proves a change to
a shared crate against every consumer here. Build output is one shared
`target/` per checkout; each game's web build goes to its own `dist/`
(`pk_xtask::set_root`).

## Outside consumers

Known today: `~/Software/render/bevy_runtime` patches wgpu from
`crates/engine/patches`. New games may live outside the repo and depend on
the shared crates by path or git URL; that is why their APIs are kept stable
and changes logged in each crate's `CHANGELOG.md`. Nothing in this repo
refers to code outside it.
