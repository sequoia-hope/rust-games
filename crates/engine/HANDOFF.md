# Handoff: before the move into the monorepo (2026-10-18)

Playground Kit (`pk_*` crates, GitHub `sequoia-hope/playground-kit`, local
dir and `proj` name `game-engine`). The plan is `docs/PLAN.md`; every
move is recorded in `docs/DECISIONS.md` (D1 to D9).

## Current state

- `main` is clean and pushed up to `dca0b2d` (D9). There are no stashes,
  no extra branches and no worktrees. CI is green up to D8. The D9 run
  (39173407960) was still going when this was written.
- **Phase 1 (Foundation) has landed**: `pk_math`, `pk_canvas`,
  `pk_lockstep`, `pk_xtask`, `pk_shell`, plus `template/`, `js/` (the
  headless-Chrome tools), `skills/` (nine procedures), `patches/` (wgpu,
  wgpu-30, wgpu-hal, bevy_ui_render) and the upstream drafts in
  `docs/upstream/`. Midnight Playground runs on them (its D1230 to D1234).
- The 16 background agents the owner stopped on 2026-10-18 (audits,
  carves, design notes, skills, upstream drafts) had all finished first.
  Each one's work is in a commit above, and none left a file uncommitted.

## In progress / waiting on the owner

- **Phase 2 is blocked on a yes** for the Action seam
  (`docs/design/action.md`, `pk_input`) and the Route seam
  (`docs/design/route.md`, `pk_route`, phase 5). The Simulation seam's
  defaults were accepted (D7).
- `pk_lockstep` is written and tested but has no consumers yet.
  `mp_sim` implementing `Simulation` comes in phase 2.
- **Snowboarding adoption** is a hand-off document
  (`docs/handoff-snowboarding.md`, section 1 = `pk_shell`). Its own agent
  (tmux `snowboarding`) applies it. Never edit `~/Software/snowboarding`
  from this repo. Snowboarding already uses the kit's patches (D9).
- **Upstream contributions are drafted and none are posted**
  (`docs/upstream/README.md`), including D9's wgpu destroy-on-drop.

## Known issues

- The kit carries two wgpu patches, `patches/wgpu` (for Bevy 0.19.1) and
  `patches/wgpu-30` (for Bevy 0.20). They stay separate until every game
  is on one Bevy (D9).
- The pixel gate is only exact for sierra, seaside and desert. Coast has
  a sea-glitter noise floor of about 70 to 180 px at channel diff 1
  (PLAN.md gate 1).
- `target/` is 16 GB of build output that can be rebuilt. Don't move it;
  absolute paths make it rebuild anyway.

## Concerns about the move

1. **The `../game-engine` sibling layout holds other projects up.** These
   all reach this repo by relative path and break if it moves without
   them:
   - `~/Software/midnight-playground`: `Cargo.toml` (the `pk_*` path deps
     and `[patch.crates-io]`), its JS tools (`../../../game-engine/js/*`),
     and `ci.yml`/`pages.yml`, which clone the kit to
     `$GITHUB_WORKSPACE/../game-engine`. Its own `HANDOFF.md` lists them.
   - `~/Software/snowboarding/Cargo.toml`: the patches.
   - `~/Software/render/bevy_runtime/Cargo.toml`: the patches, through
     `../../game-engine`.
   - Inside this repo: `template/Cargo.toml`, `template/CLAUDE.md`,
     `template/README.md`, the template's `ci.yml`/`pages.yml` clone
     step, `skills/client-shell/SKILL.md`, `docs/handoff-snowboarding.md`
     and README/PLAN.

   The simplest move keeps the kit at a path called `game-engine` beside
   the games in the monorepo. Renaming it means changing every place
   listed above in one go.
2. **Cargo workspaces.** This is its own workspace, with its own
   `[patch.crates-io]`, `rust-toolchain.toml` (1.99.0 + wasm32),
   `.cargo/config.toml` (the `xtask` alias, the wasm test runner) and
   `deps.toml`/`check-deps` rules. Cargo applies patches and `.cargo`
   config only from the workspace root. If the monorepo gets a root
   workspace, either exclude this one or merge it in deliberately. A
   nested member silently loses the wgpu/Bevy patches. A merge also has
   to give every new member a `deps.toml` entry, or `check-deps` fails.
3. **The repo's identity.** `Cargo.toml` names
   `repository = sequoia-hope/playground-kit`, and the games' CI and the
   template clone that URL. If the monorepo replaces the GitHub repo,
   those clones need a new source. Actions only run workflows from the
   repo root's `.github/workflows/`, so `ci.yml` has to move or be merged.
   Decide which repo `main` gets pushed to.
4. **The port registry.** `~/scripts/launcher.toml` registers
   `game-engine` with `dir = /home/sequoia/Software/game-engine`.
   `serve.sh` gets its port from `proj port` via the current directory,
   and the server is running now on its registered port. Before the move,
   run `proj down game-engine`. After it, update the registry `dir`, then
   `proj up game-engine` and `proj doctor`. `launch.sh` names the tmux
   session `game-engine`. The `game-engine` and `web-game-engine` tmux
   sessions are live.
5. **Agent memory is keyed by path**:
   `~/.claude/projects/-home-sequoia-Software-game-engine/memory/` holds
   the owner's kit decisions and the snowboarding hands-off rule. Copy it
   to the directory for the new path.
6. **Sibling agents.** Midnight Playground and Snowboarding have their
   own live agents (tmux sessions). Move the three repos while those
   agents are stopped, or they will build against a path that has gone.
