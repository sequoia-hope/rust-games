# Playground Kit

A shared toolkit for Rust + Bevy games that run in the browser (wasm, WebGPU
with a WebGL2 fallback) and natively, carved from
[Midnight Playground](https://github.com/sequoia-hope/midnight-playground)
so that its sister games (a snowboarding game first; bicycle, kayak,
platformer, diving, basketball and fantasy games planned) build on one
library instead of copies.

Crates are `pk_*`. The games depend on them by sibling path
(`../game-engine/crates/pk_*`): this checkout lives beside the games.

| Where | What |
|---|---|
| `docs/PLAN.md` | the crate map, the seams, the phases and their gates |
| `docs/INVENTORY.md` | what in Midnight Playground is generic, module by module |
| `docs/ECOSYSTEM.md` | where this duplicates open source, what fills a gap, what to upstream |
| `docs/RECOMMENDATIONS.md` | what the split means for the racing game's roadmap |
| `docs/DECISIONS.md` | every choice the plan did not cover, numbered |
| `docs/design/` | one note per seam, written before its phase |
| `crates/` | the library |
| `patches/` | wgpu (29 for Bevy 0.19, `wgpu-30` for 0.20), wgpu-hal and bevy_ui_render as the games patch them, until upstream lands |
| `template/` | a new game's skeleton: xtask, loader page, server, CI |
| `js/` | headless-Chrome tools: perf, profile by Bevy system, GL calls, build diff, shots |
| `tools/` | the atlas, voice and signalling-relay pipelines |
| `skills/` | Claude Code skills for the sub-crafts |

## Working rules

- Only `pk_shell`, `pk_ui`, `pk_input`, `pk_play`, `pk_render` and
  `pk_audio` depend on Bevy. Everything in layer 0 builds for native and
  `wasm32-unknown-unknown` with `#![forbid(unsafe_code)]`, no clock, threads
  or hash maps, and every inexact math function through `pk_math`'s kernel.
  `cargo xtask check-deps` enforces `deps.toml`.
- A crate may not name a car, a road, a station or a level. A game type
  becomes a trait or an open data type; the game implements it.
- Bevy is pinned exactly (0.19.1) and moves between milestones; no
  third-party Bevy plugins without a DECISIONS entry naming the alternative.
- Output is viewed through the registered server only (`proj up
  game-engine`); never start another server or write a port anywhere.
- Commit and push straight to `main`: imperative subject, a prose body.

## Licence

MIT OR Apache-2.0, at your option (`LICENSE-MIT`, `LICENSE-APACHE`). The
patched crates under `patches/` keep their upstream licences.
