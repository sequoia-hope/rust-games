# rust-games: working rules for every session

One git repo and one Cargo workspace for the owner's hobby games and the
crates they share. Rust 1.99.0 + wasm32 (`rust-toolchain.toml`), Bevy
0.19.1 pinned, games for the web (WebGPU, WebGL2) and natively.
`docs/ARCHITECTURE.md` is the map; this file is the rules. Each crate and
game has its own `CLAUDE.md` with its conventions: read the one for the
code you touch.

## Layout

| Where | What |
|---|---|
| `crates/engine/` | **Playground Kit**, the shared engine (`pk_math` kernel/noise/rng, `pk_lockstep` deterministic simulation, `pk_canvas` 2D canvas, `pk_shell` the Bevy client shell, `pk_xtask` build tooling), plus `patches/` (the patched wgpu, wgpu-hal, bevy_ui_render every game builds on), `js/` (headless-Chrome tools), `template/` (a new game's skeleton), `skills/`, `docs/` (its plan and DECISIONS) |
| `crates/music/` | the **music engine** (package `mp_music`): the Music Lab's procedural music (instruments, sequencer, composer, genre grammars, mix) and the radio station player; also built as its own AudioWorklet wasm |
| `games/midnight-playground/` | **Midnight Playground**, the street racer (`mp_*` crates, plus the frozen legacy JS game) |
| `games/snowboarding/` | **Snowboarding**, the 1080°-style snowboarding game (`sb_*` crates) |
| `docs/` | architecture, the reorganization plan, kickoff prompts for new projects |
| `status/` | coordination between parallel sessions (below) |

Root files: `Cargo.toml` (the one workspace: members, shared dependency
versions, profiles, `[patch.crates-io]` — Cargo reads patches and profiles
only here), `Cargo.lock`, `deps.toml` (the crate dependency rules for every
member), `.config/nextest.toml`, `.githooks/`, `scripts/land.sh`.

## The rules

1. **One agent follows a task wherever it goes.** If a bug reported in a
   game is really in `crates/engine` or `crates/music`, fix it there, in
   the same session and branch. Don't hand it off to another session or
   write a hand-off document for another agent. (Older docs in the kit and
   the games describe hand-offs between per-repo agents; that rule is gone.)

2. **The shared crates are libraries with outside consumers.** Games outside
   this repo depend on `crates/engine` (and may on `crates/music`) by path
   or git URL. So:
   - keep their public APIs stable and documented; add rather than change;
   - no needless breaking changes, and record every breaking change (and
     every notable addition) in that crate's `CHANGELOG.md`;
   - a shared crate never depends on a game, names a game's car, course or
     station, or reaches into a game's files (`deps.toml` enforces the
     dependency half);
   - **never add a reference from this repo to code outside it** — no path
     dependency, script or doc path into `~/Software/<anything else>`.

3. **A change under `crates/` must not break any other consumer**: the whole
   workspace keeps building and passing its tests, not just the game you
   were working on. Run the workspace checks below, not just one crate's.

4. **Worktrees.** Every task gets its own branch and its own git worktree;
   never commit on `main` directly.
   ```sh
   git worktree add -b <branch> ~/Software/rust-games-worktrees/<branch>
   ```
   Worktrees go in `~/Software/rust-games-worktrees/`, **outside** this
   folder: `proj port` resolves a directory by walking up to the nearest
   registered project, so a game served from a worktree inside
   `rust-games/` would bind the `rust-games` status page's port. Outside,
   it fails loudly, as it should; serve from a worktree only with an
   explicit `--port` the owner gives you. (Claude Code's own worktrees go
   under `.claude/worktrees/`, inside: don't serve from those.) Each
   worktree builds its own `target/` (tens of GB): remove the worktree
   (`git worktree remove`) once its branch has landed.

5. **Landing on `main`** happens only through `scripts/land.sh <branch>`,
   which runs the workspace checks in the branch's worktree and merges only
   if they pass. Never `git merge` into `main` by hand, never bypass the hooks
   (`--no-verify`).

6. **Coordination.** Several sessions may run at once. Before changing a
   shared crate, read every note in `status/` of the **main checkout**
   (`~/Software/rust-games/status/`, not your worktree's copy); while you
   work on one, keep your own note there. `status/README.md` has the
   format. If another session's note says it is changing the same API,
   coordinate through the notes (or ask the owner) before going ahead.

## Workspace checks

| What | Command (from the repo root or a worktree root) |
|---|---|
| Fast check (the pre-commit hook) | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` |
| Tests (nextest kills a hung test; plain `cargo test` doesn't) | `cargo nextest run --workspace` |
| Lints | `cargo clippy --workspace --all-targets -- -D warnings` |
| Wasm builds | `cargo check --workspace --target wasm32-unknown-unknown` |
| Crate dependency rules (`deps.toml`) | `cargo xtask check-deps` |
| Everything, then merge to `main` | `scripts/land.sh <branch>` |

`cargo xtask` at the root runs the kit's tooling; inside a game's folder it
runs that game's (`games/<game>/.cargo/config.toml` aliases it), e.g.
`cd games/snowboarding && cargo xtask web --release`. Each game writes its
web build into its own `dist/`.

The git hooks live in `.githooks/` (`git config core.hooksPath .githooks`,
set once per clone; worktrees share it).

## Serving and ports

Output is viewed only through registered servers (`~/scripts/README.md`):
`proj up <name>` or the project's own `./serve.sh`. Never start an ad-hoc
server and never write a port number into a script, config or doc: a
program takes `--port`, then `$PORT`, then `proj port`, then fails loudly.
`rust-games` itself is registered (its `index.html` status page); the games
and the kit have their own registrations.

## Commits

Imperative subject, a prose body that says why. Build output (`target/`,
`dist/`, caches) stays out of git. Each game's and crate's history from
before the move into this repo is intact (`git log --follow <file>`), but
not every hash: before the first push, a machine's private hostname was
scrubbed from Midnight Playground's history, which renumbered its commits
from 2026-10-03 on. The docs here cite the new hashes;
`docs/history-rewrite.md` maps the old ones (as the old repos and GitHub
links have them) to the new.

Never commit a private hostname, address or account detail: write
`<machine>.<tailnet>.ts.net`, and ask for the real one at run time.
