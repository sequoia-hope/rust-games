# Game Workspace Reorganization — Kickoff Prompt

Oct 18, 2026 · @Sequoia

## Before you start (manual steps)

- In each existing Claude Code session (Midnight Playground, snowboarding game, game engine, any others), say: "I'm about to move this project into a shared monorepo workspace. Commit or stash all work, and write a short HANDOFF.md covering current state, in-progress work, known issues, and any concerns about the move."
- Review the handoffs, then close those sessions.
- Create the new parent folder and start one fresh Claude Code session there. Paste the prompt below.

## Prompt

I'm consolidating my hobby game projects into one Rust monorepo. Currently they live in parallel folders: the Midnight Playground racing game, a 1080 Snowboarding-style game, a shared game engine that was extracted from Midnight Playground, and a procedural music generator that still lives inside Midnight Playground and is used by multiple games. Each old folder has a HANDOFF.md describing its state — read those first.

### Target layout

- One git repo, one Cargo workspace at the root.
- `crates/engine` — shared game engine.
- `crates/music` — music engine (for now, just create the crate and move the existing music code with minimal changes; a separate agent will redesign it later using `docs/music-engine-kickoff.md`).
- `games/midnight-playground`, `games/snowboard` — games depending on the crates via path dependencies.
- `docs/` — architecture notes and project kickoff docs.
- `status/` — a lightweight coordination folder (see below).
- Adjust this layout if you find a good reason, but explain why before doing it.

**External consumers:** some games live outside this repo and depend on the engine crates by path or git URL. Treat `crates/engine` and `crates/music` as libraries with stable, documented public APIs: avoid needless breaking changes, note any breaking change in a CHANGELOG, and never add references from this repo to code outside it. Include this rule in the root CLAUDE.md.

### Migration

- Preserve git history from each existing repo if practical (e.g. subtree merge or `git filter-repo`); tell me the options and tradeoffs before choosing.
- Move everything into place, fix paths and dependencies, and get the whole workspace building and all existing tests passing before any other changes.
- Do the work on a branch in its own git worktree, not directly on main.
- Don't delete the old folders; I'll remove them once I've verified things.

### Root CLAUDE.md

Write a root CLAUDE.md that every future session will read. It should cover:

- The layout and what each crate and game is for.
- **One agent follows a task wherever it goes**: if a bug reported in a game is really in the engine or music crate, fix it there directly. No handing off to other sessions.
- **Shared crates must not break other consumers**: any change to `crates/` must keep the entire workspace building and passing tests.
- **Worktrees**: each task gets its own branch and git worktree; merge to main only when workspace checks pass.
- **Coordination**: when running parallel sessions, note in-progress work on shared crates in `status/` (who's touching what, open requests, breaking changes) and check it before changing a shared crate.
- Short per-crate CLAUDE.md files where useful, for crate-specific conventions.

### Enforcement

- A fast git pre-commit hook: `cargo check --workspace` (plus fmt/clippy if quick).
- A pre-push hook, or a required step before merging to main: `cargo test --workspace`.
- Suggest whether Claude Code hooks (e.g. running checks when an agent finishes a turn) are worth adding, and set them up if I agree.

### Working style

- Start by writing the plan to `docs/REORG_PLAN.md` and check in with me before moving files.
- When finished, summarize what moved, what changed, and anything I should verify by hand (e.g. running each game).
