# Reorganization plan: one Rust monorepo for the games

Status: **approved and carried out on 2026-10-18** (branch `reorg/monorepo`).
Written 2026-10-18 from the three `HANDOFF.md` files and a survey of the repos;
the owner approved every recommendation in section 2.

**Where execution differed from this plan:**
- The music crate's package keeps the name `mp_music` (section 3 said
  `music`). The worklet's file name `mp_music.wasm` is wired into about twenty
  JS, HTML and Python files, so renaming the package would not be the
  "minimal change" the kickoff asked for. The redesign can rename it.
- Snowboarding had never been run through rustfmt. Its formatting is a
  separate commit after the migration, so the pre-commit format check passes.
- The browser suites (Midnight Playground's `test:e2e:rust`, Snowboarding's
  `cargo xtask check --web`) were not run. They need each game's registered
  server, and the registry still points at the old folders (section 9).

## 1. What is there now

| Old folder | What it is | Git | Workspace members |
|---|---|---|---|
| `~/Software/game-engine` | Playground Kit, the shared engine | 15 commits, `origin` = `sequoia-hope/playground-kit`, 1 commit ahead (the handoff) | `pk_math`, `pk_lockstep`, `pk_canvas`, `pk_shell`, `pk_xtask`, `xtask`; also `patches/` (wgpu, wgpu-30, wgpu-hal, bevy_ui_render), `js/`, `template/`, `skills/`, `docs/` |
| `~/Software/midnight-playground` | Midnight Playground, Rust and the legacy JS | 462 commits, `origin` = `sequoia-hope/midnight-playground`, CI and Pages | 14 `mp_*` crates (`mp_music` among them), `xtask`, `tools/parity/kernel` (`mr_kernel`) |
| `~/Software/snowboarding` | the snowboarding game | 2 commits, **no remote** (this folder is the only copy) | `sb_sim`, `sb_radio`, `sb_music`, `sb_game`, `xtask` |

What they have in common:
- Rust 1.99.0 with wasm32.
- Bevy `=0.19.1`, so the Bevy unification is already done for these three.
- `wasm-bindgen =0.2.129`.
- The same three `[patch.crates-io]` entries, all pointing at `game-engine/patches/`.

**Finding: the music engine is forked, not shared.** `sb_music` is a copy of `mp_music`:
- The DSP, instruments, sequencer, composer and genre code are the same.
- `radio::STATIONS` differs: Midnight Playground has five stations, Snowboarding has one (Ten-Eighty FM).
- The `lib.rs` docs, one test constant and the examples also differ.
- Only `mp_music` has the golden tests against the JS Music Lab. They live in `parity/golden/music/`, which is in the game, not the crate.
- The station table is compiled into the AudioWorklet wasm (`wasm.rs` reads `STATIONS`), so it isn't just data a game passes in.

**External consumers today.**
- `~/Software/render/bevy_runtime` uses the kit's patches through `../../game-engine/patches/*`.
- No other Cargo project outside these three refers to `pk_*` or `mp_music`.
- The kit's `template/` produces games that expect a sibling `../game-engine` and clone `playground-kit` in CI.

## 2. Decisions I need from you

My recommendation is listed first in each.

1. **Disk space (blocking).** `/` has **39 GB free (98% used)**. The old build dirs alone are 194 + 43 + 16 GB. A clean build of the merged workspace (native, tests, wasm, web-release) will probably fill what is left, and every extra worktree adds its own `target/`.
   - *Recommended:* run `cargo clean` in the three old folders. That frees about 250 GB. Their running dev servers serve `dist/`, not `target/`, so they keep working. The cost is that the old folders would need a full rebuild if you went back to them.
   - Alternative: put `CARGO_TARGET_DIR` for the monorepo on `/mnt/carson` (2 TB free). Builds would be much slower if that is a spinning or network disk.
2. **History import.** See section 4. *Recommended:* commit a move in each repo, then merge, which keeps the original commit hashes.
3. **Music crate scope.** See section 5. *Recommended:* move `mp_music` to `crates/music` as it is, and leave `sb_music` as a fork for the redesign agent to merge.
4. **Directory name for Snowboarding.** *Recommended:* `games/snowboarding`, not `games/snowboard`. The package, binary, `proj` name, tmux session, `launch.sh` and memory directory are all called `snowboarding`. A bonus: its `../midnight-playground/tools/dj-voice/voices` paths keep working unchanged, because both games end up side by side under `games/`.
5. **GitHub, CI and Pages.** *Recommended for this reorg:* keep the monorepo local, with no remote.
   - The two GitHub repos stay frozen at their current `main`, and Pages keeps serving the last deployed build.
   - The workflows move into the tree but stay inactive. A root workflow is written when you decide where `main` is pushed: a new `rust-games` repo, which changes the Pages URL, or splitting back out to the old repos.
   - Snowboarding has no remote at all, so either way the monorepo becomes its backup only once it is pushed somewhere.
6. **Midnight Playground's leftovers.** The `hot-pursuit` branch and `origin/claude/optimistic-franklin-ijm0fb` are merged into `main`. The stash from 2026-09-30 is probably superseded. *Recommended:* import only `main`. The branch and the stash stay in the old folder until you delete it.

## 3. Target layout

```
rust-games/
  Cargo.toml              the one workspace: members, workspace deps, profiles, [patch.crates-io]
  Cargo.lock              one lockfile, seeded from Midnight Playground's
  rust-toolchain.toml     1.99.0 + wasm32 (identical in all three today)
  .cargo/config.toml      the wasm32 test runner
  deps.toml               merged crate-dependency rules (check-deps reads it at the workspace root)
  CLAUDE.md  README.md
  .githooks/              pre-commit, pre-push (section 8)
  scripts/land.sh         the checked merge to main (section 8)
  crates/
    engine/               Playground Kit, was ~/Software/game-engine
      pk_math/ pk_lockstep/ pk_canvas/ pk_shell/ pk_xtask/
      xtask/              the kit's own tooling, package renamed `engine_xtask`
      patches/            wgpu, wgpu-30, wgpu-hal, bevy_ui_render (excluded from the workspace)
      js/ template/ skills/ docs/ tools/
      README.md  CHANGELOG.md  CLAUDE.md
    music/                package `music`, was midnight-playground/crates/mp_music
      tests/golden/       the Music Lab goldens, was midnight-playground/parity/golden/music
      CHANGELOG.md  CLAUDE.md
  games/
    midnight-playground/  the whole repo except its workspace-level files
      crates/mp_*/ xtask/ (package `mp_xtask`) tools/ docs/ legacy JS, assets, ...
      .cargo/config.toml  alias `xtask = run --package mp_xtask --`
    snowboarding/
      crates/sb_*/ xtask/ (package `sb_xtask`) tools/ ...
      .cargo/config.toml  alias `xtask = run --package sb_xtask --`
  docs/                   ARCHITECTURE.md, REORG_PLAN.md, the kickoff prompts
  status/                 coordination (section 7)
  index.html serve.sh launch.sh   the existing proj scaffold for `rust-games`
```

Changes from the kickoff layout, and why:

- **`crates/engine` is a folder of five crates, not one crate.** The kit is already split by layer, and `deps.toml` enforces it: the `pure` and `exact_math` simulation crates must stay free of Bevy. Merging them into one crate would throw that away and break every `use pk_*` path. The folder still reads as "the engine". Inside it, the kit's own `crates/` level is dropped, so it's `crates/engine/pk_math`, not `crates/engine/crates/pk_math`.
- **`patches/` stays inside `crates/engine`.** It is part of what the kit offers outside consumers such as `render`, and its docs and template refer to it there. The root `[patch.crates-io]` points at it. Cargo only honours patches at the workspace root, which is fine because we only have one root.
- **The game repos move nearly whole.** Their `crates/`, `xtask/`, `tools/` and docs keep their relative shape, which keeps nearly all internal paths valid.

### Merging three workspaces into one

Cargo reads `[patch]` and `[profile]` only from the root manifest, so these move up:

- **Members:**
  ```
  crates/engine/pk_*
  crates/engine/xtask
  crates/music
  games/midnight-playground/crates/*
  games/midnight-playground/xtask
  games/midnight-playground/tools/parity/kernel
  games/snowboarding/crates/*
  games/snowboarding/xtask
  ```
  `exclude = ["crates/engine/patches", "crates/engine/template"]`. `default-members` are the same minus the xtasks and the parity kernel, as today.
- **`[workspace.dependencies]`:** the union of the three. There are no conflicts: `glam = 0.32.1` comes from Snowboarding, `bevy = =0.19.1` from the kit, and `wasm-bindgen` is identical in all three.
- **Workspace package fields:** `[workspace.package]` keeps only the shared fields (version, edition, publish). `license` and `repository` move to the crates that have them today, so the engine crates keep the kit's `MIT OR Apache-2.0` and URL.
- **Profiles:** these are the same in all three except the per-package overrides, which I'll keep: `mp_exhaust`, `music` (was `mp_music`), `sb_music` and the xtasks.
- **xtask package names collide.** Every repo has a package called `xtask`, and package names must be unique in one workspace. They become `engine_xtask`, `mp_xtask` and `sb_xtask`. Cargo reads `.cargo/config.toml` upward from the current directory, so a per-folder alias keeps `cargo xtask …` working when run from a game's own folder.
- **Lockfile:** I'll seed it from Midnight Playground's `Cargo.lock`, whose parity goldens are the most version-sensitive, and let cargo add the rest. I won't run `cargo update`. Afterwards I'll check `cargo tree -i wgpu`, which must show the patched path, and `cargo tree -d` for new duplicates.
- **`deps.toml`:** one merged file at the root, combining the kit's rules, Midnight Playground's rules, and new entries for the four `sb_*` crates, `music` and the renamed xtasks. Every member needs an entry, or `check-deps` fails.

### Fixes the merge needs

These are the only code changes in the migration phase:

- **`pk_xtask`** treats the workspace root as the game's root:
  - it writes `dist/next` to `root().join(...)`;
  - it builds into `root().join("target")`;
  - it reads `deps.toml` from there.

  In the monorepo, both games would write to the same `rust-games/dist/`. I'll add a game root, taken from the calling xtask's manifest directory, for `dist/`. Target paths will come from `cargo metadata`'s `target_directory`, and `deps.toml` stays at the workspace root. This changes `pk_xtask`'s public API, so it gets a CHANGELOG entry.
- **Snowboarding's xtask** already uses its own manifest directory for `dist/`. It needs the same `target_directory` fix.
- **Midnight Playground's JS tools** reach the kit through `../../../game-engine/js/*` (`tools/parity/*.mjs`, `e2e/harness.mjs`, `lib/server.mjs`). These become paths to `crates/engine/js`. `tools/parity/gen-index.py:10` has an absolute `/home/sequoia/Software/game-engine/...` path, which becomes a relative one.
- **The music golden tests** read `../../parity/golden/music/*.json`. The goldens move to `crates/music/tests/golden/`, and the Music Lab's `golden.mjs`, which stays in the game, writes them there.
- **CI workflows** that clone the kit to `../game-engine` aren't touched in this phase (decision 5).
- **Docs and comments that mention `../game-engine`.** I'll fix the operational ones: CLAUDE.md files, READMEs, Cargo.toml comments and scripts. Historical logs (`DECISIONS.md`, handoffs) stay as written, because they record what was true then.

## 4. Git history: options

| Option | How | Pros | Cons |
|---|---|---|---|
| **A. Move commit + merge** (recommended) | In a scratch clone of each repo: `git mv` everything into its prefix, commit, then `git merge --allow-unrelated-histories` into the reorg branch. This is what `git subtree add` does, without squashing. | Every original commit hash survives. The handoffs, `DECISIONS.md` and GitHub links cite hashes such as `dca0b2d` and `75679ca`. No extra tools needed. | Commits before the move show the old paths. `git log -- games/x/file` needs `--follow`. Blame follows the move automatically. |
| B. `git filter-repo --to-subdirectory-filter` | Rewrite each clone so it always lived in its prefix, then merge. | Path history looks native everywhere, with no `--follow` needed. | **Every hash changes**, so cited hashes and GitHub links stop resolving inside the monorepo. Needs `git-filter-repo`, which isn't installed. |
| C. Fresh copy, no history | Copy the files and make one commit. | Simplest. | `git log` and blame start today. The old folders, or GitHub for two of them, become the archive. Bad for Snowboarding, which has no remote. |

Import order: kit, then Midnight Playground, then Snowboarding. After that, `git mv games/midnight-playground/crates/mp_music crates/music`, so the music crate's history follows it.

## 5. The music crate

The kickoff calls for minimal changes now and a redesign later by its own agent (`docs/music-engine-kickoff.md`, **which doesn't exist yet**: you'll provide it, or I can start a stub). Options:

- **A. Move `mp_music` as is, and leave `sb_music` as a fork** (recommended for this reorg).
  - `crates/music` becomes package `music`.
  - Midnight Playground keeps compiling without code edits through `mp_music = { package = "music", path = "crates/music" }` in the workspace dependencies.
  - `sb_music` stays in `games/snowboarding`, unchanged.
  - I record in `crates/music/CLAUDE.md` and `status/` that the fork exists and how it differs, as input for the redesign. The station table compiled into the worklet wasm is the real design question, and it belongs to that agent.
- B. Remove the fork now: make the station table a parameter, add thin per-game crates for the worklet cdylib, and delete `sb_music`. It's a small code change, but it's design work, and it changes behaviour the kickoff wanted left alone.

## 6. Order of work

All work happens on the branch `reorg/monorepo`, in a worktree outside the tree at `~/Software/rust-games-worktrees/reorg-monorepo` (section 7 explains why outside).

0. **Before starting:** you decide section 2. If you choose to, clean the old build dirs. I record baselines in the old repos with their existing builds:
   - test counts: Midnight Playground's nextest total, Snowboarding's 62 passed and 1 ignored, and the kit's count;
   - `check-deps` passing;
   - `cargo tree -i wgpu`.
1. `git init` in `~/Software/rust-games`. Commit the scaffold to `main`: `index.html`, `serve.sh`, `launch.sh`, a `.gitignore`, the kickoff prompt and this plan, both in `docs/`. Then create the worktree and branch.
2. Import the three histories (section 4) and move the music crate.
3. Merge the workspaces and apply the fixes (section 3).
   **Gate A, all must pass before any other change:**
   - `cargo check --workspace --all-targets`
   - `cargo check --workspace --target wasm32-unknown-unknown`
   - `cargo nextest run --workspace`, with the same counts as the baseline
   - `cargo tree -i wgpu`, showing the patched path
   - `check-deps` from each folder
   - `cargo xtask web --release` for both games, each writing to its own `dist/`
   - the kit's and the games' headless-Chrome checks, if they run in reasonable time
4. Rules and docs (section 7): root and per-crate CLAUDE.md files, CHANGELOGs, `status/`, `docs/ARCHITECTURE.md`. Edit the existing CLAUDE.md files where they conflict with the new rules.
5. Enforcement (section 8). I'll time the hooks on this machine before choosing what goes into pre-commit.
6. Land on `main` through `scripts/land.sh`.
7. Copy the git-ignored state into the new tree. The old folders stay, so this is copying, not moving:
   - `parity/cache` (5.4 GB): copied as hard links (`cp -al`), which uses no extra space.
   - the atlas and seaside caches, `shots/` and `logs/client.jsonl`: copied normally.
   - `node_modules`: reinstalled with `npm ci`.
   - `dist/` and `target/`: rebuilt, not copied.
8. Report: what moved, what changed, and what to check by hand.

## 7. Rules for future sessions

### Root `CLAUDE.md`

- The layout, and what each crate and game is for.
- **External consumers:** `crates/engine` and `crates/music` are libraries with stable, documented public APIs. No needless breaking changes. Every breaking change goes in that crate's `CHANGELOG.md`. Nothing in this repo may reference code outside it.
- **One agent follows the task:** a game bug that is really in a shared crate gets fixed there, by the same session. This replaces the old rules such as "the kit's agent never edits Snowboarding" and "never edit `~/Software/snowboarding` from this repo", which I'll remove from the kit's docs and the game CLAUDE.md files.
- **Shared crates don't break other consumers:** a change under `crates/` must keep the whole workspace building and its tests passing.
- **Worktrees:** one branch and one worktree per task, under `~/Software/rust-games-worktrees/<branch>`. Merge to `main` only through `scripts/land.sh`.
  - *Why outside the tree:* `proj port` walks up to the nearest registered project. Inside `rust-games/`, a game's `serve.sh` running in a worktree would resolve to the `rust-games` status page's port. Outside, it fails loudly, as the port rules require. Serving from a worktree needs an explicit `--port`.
  - Claude Code's built-in worktrees go under `.claude/worktrees/` inside the repo, so the CLAUDE.md file says not to use them for anything that serves.
  - Each worktree has its own `target/`. With this disk, remove finished worktrees promptly.
- **Coordination:** before changing a shared crate, read `status/`. While working on one, keep a note there.

### `status/`

Notes there must be visible across worktrees, so they live in the **main checkout's** `status/` (`~/Software/rust-games/status/`), not in each worktree's copy:
- `status/README.md` explains the format and is the only tracked file. Notes are git-ignored.
- Each session writes one note, `status/<branch>.md`, with these fields:
  - who: the branch or session;
  - which shared crates it touches;
  - planned API changes, and whether they break anything;
  - open requests to other sessions;
  - when it was last updated.
- A session deletes its note when its branch lands.

### Per-crate `CLAUDE.md`

- `crates/engine`: the layer rules and `deps.toml`, the patches policy, the template and skills, plus a pointer to `docs/PLAN.md` and `DECISIONS.md`.
- `crates/music`: the port rules (D1150), the goldens, the fork note, the worklet build.
- Each game keeps its own, edited for the new paths and rules.

## 8. Enforcement

- **`.githooks/`, enabled with `core.hooksPath`.** That setting is shared config, so it covers every worktree.
  - `pre-commit`: `cargo fmt --all --check`, then `cargo check --workspace --all-targets`. Clippy is added only if an incremental run on this machine stays under about 30 s; I'll measure.
  - `pre-push`: `cargo nextest run --workspace` and `cargo clippy --workspace`. This only matters once a remote exists.
- **`scripts/land.sh <branch>`** is the required step for merging to `main`, since local merges don't trigger pre-push. It runs, in the branch's worktree:
  - fmt
  - clippy
  - `cargo nextest run --workspace`
  - `check-deps`
  - a wasm check

  Only if all pass does it merge into `main`.
- **Midnight Playground's flaky `mp_host` http test** gets a nextest retry in `.config/nextest.toml`, so it doesn't block merges. It stays listed as a known flake.
- **Claude Code hooks:** I recommend against a Stop hook that runs `cargo check`. With Bevy in the tree, it would add a build to every turn, and use disk this machine doesn't have. The git hooks and `land.sh` already gate what matters. If you want one, a cheap Stop hook could remind the session to update `status/` when a worktree has uncommitted changes under `crates/`. Only if you agree.

## 9. Outside the repo: done by you, or by me once you say so

None of this happens before you have verified the new tree.

- **Port registry:**
  - Repoint `midnight-playground`, `snowboarding` and `game-engine` in `~/scripts/launcher.toml` to `games/midnight-playground`, `games/snowboarding` and `crates/engine`.
  - Then `proj down` / `proj up` each one, and run `proj doctor`.
  - Midnight Playground's `stream`/`signal` extras move with it.
  - Until then, the old folders keep serving as they do now.
- **Claude memory:** copy the three memory folders (`~/.claude/projects/-home-sequoia-Software-{midnight-playground,snowboarding,game-engine}/memory/`) into the `rust-games` project memory, with a prefix per source and an updated index.
- **`render/bevy_runtime`:** its patch paths must point at `…/rust-games/crates/engine/patches/*` before the old `game-engine` folder is deleted. That repo is outside this one, so I won't edit it unless you ask.
- **The kit's `template/`** still assumes a sibling `../game-engine` and the `playground-kit` clone. It's excluded from the build, so it's left as is and noted in `crates/engine/CHANGELOG.md` and `status/` as follow-up work.
- **tmux sessions:** stop the old agent sessions (`game-engine`, `snowboarding`, Midnight Playground's) before you delete the old folders. The new tree's per-game `launch.sh` scripts keep unique session names.

## 10. To check by hand afterwards

- Run each game natively and on the web: Midnight Playground's Drive and open world, and Snowboarding's courses and radio. Check on the phone too.
- Midnight Playground's radio and Snowboarding's Ten-Eighty FM both play: the music crate moved, and the worklet wasm is built from the new path.
- `git log --follow` on a few files from each old repo.
- After the registry is repointed, the dashboard and the tailnet paths (`/midnight-playground/`, `/snowboarding/`) serve from the new tree.
