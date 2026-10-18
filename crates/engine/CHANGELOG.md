# Changelog: Playground Kit (crates/engine)

The `pk_*` crates are used by the games in this repo and by games outside
it (by path or git URL). Every breaking change goes here, with how to
migrate; notable additions too. Newest first.

## Unreleased

### Moved into the rust-games monorepo (2026-10-18)

- **Where:** the kit now lives at `rust-games/crates/engine/` (it was the
  standalone repo `sequoia-hope/playground-kit`, checked out as
  `~/Software/game-engine`). Its crates sit directly under it
  (`crates/engine/pk_math`, not `crates/engine/crates/pk_math`); `patches/`,
  `js/`, `template/`, `skills/` and `docs/` are where they were relative to
  the kit. Its history came with it, commit hashes unchanged.
- **Migrating an outside consumer:** repoint path dependencies from
  `../game-engine/crates/pk_*` to `<rust-games>/crates/engine/pk_*`, and
  `[patch.crates-io]` entries from `../game-engine/patches/*` to
  `<rust-games>/crates/engine/patches/*`. A git dependency on
  `playground-kit` keeps working until that repo stops being updated; the
  new git source is not decided yet.
- **Not a workspace of its own any more:** the kit's `Cargo.toml`,
  `Cargo.lock`, `rust-toolchain.toml` and `deps.toml` were merged into the
  rust-games root. Its `xtask` package is now `engine_xtask` (`cargo xtask`
  in `crates/engine/` still runs it).
- **Known stale:** `template/` still assumes a sibling `../game-engine` and
  clones `playground-kit` in CI. It is excluded from the workspace build;
  bringing it up to date is follow-up work.

### pk_xtask: a game's root apart from the workspace root (additive)

- `pk_xtask::set_root(dir)`: a game in a workspace of several games names
  its own folder, and `root()` (its page, `dist/`, tools) is relative to
  that. Without the call, `root()` is the workspace root as before.
- `pk_xtask::workspace_root()`: where `deps.toml` and `Cargo.lock` are
  read from (`check-deps`, the wasm-bindgen version check).
- `pk_xtask::target_dir()`: Cargo's build directory from `cargo metadata`
  (honours `CARGO_TARGET_DIR`); the web build reads its wasm from there
  instead of `root()/target`.
- Not breaking: a game that is its own workspace and does not call
  `set_root` sees the same paths as before.
