#!/usr/bin/env bash
# land.sh: the only way onto main (CLAUDE.md, "Landing on main").
#
#   scripts/land.sh <branch>       check <branch> in its worktree, then merge it into main
#   scripts/land.sh --check-only   run the checks in the current checkout, merge nothing
#
# The checks are the whole workspace's, whichever crate or game the branch
# touched: formatting, clippy with warnings denied, the wasm builds, every
# test (nextest), and the crate dependency rules (deps.toml). The merge is
# done in the main checkout, as a merge commit (--no-ff) so a task stays one
# unit in the history, and only if main has no uncommitted changes.
set -euo pipefail

checks() {
  local dir="$1"
  cd "$dir"
  run() { echo "land: $*" >&2; "$@"; }
  run cargo fmt --all --check
  run cargo clippy --workspace --all-targets --quiet -- -D warnings
  # The web clients and everything they build on, for the browser target.
  run cargo clippy --target wasm32-unknown-unknown --quiet \
    -p mp_game -p sb_game -p pk_shell -p pk_math -p pk_lockstep -p pk_canvas -p mp_music -p sb_music \
    -- -D warnings
  run cargo clippy -p mp_audio --target wasm32-unknown-unknown --features web --quiet -- -D warnings
  run cargo nextest run --workspace
  run cargo xtask check-deps
}

if [[ "${1:-}" == "--check-only" ]]; then
  checks "$(git rev-parse --show-toplevel)"
  echo "land: checks passed" >&2
  exit 0
fi

branch="${1:?usage: scripts/land.sh <branch> | --check-only}"
[[ "$branch" != main ]] || { echo "land: give the task's branch, not main" >&2; exit 1; }

# The worktree that has the branch checked out (`git worktree list --porcelain`).
tree="$(git worktree list --porcelain | awk -v b="refs/heads/$branch" '
  /^worktree /{ w = substr($0, 10) } $0 == "branch " b { print w }')"
[[ -n "$tree" ]] || { echo "land: no worktree has $branch checked out" >&2; exit 1; }
[[ -z "$(git -C "$tree" status --porcelain)" ]] || {
  echo "land: $tree has uncommitted changes; commit or stash them first" >&2; exit 1; }

# The main checkout: the worktree with main checked out.
main="$(git worktree list --porcelain | awk '
  /^worktree /{ w = substr($0, 10) } $0 == "branch refs/heads/main" { print w }')"
[[ -n "$main" ]] || { echo "land: no checkout has main" >&2; exit 1; }
[[ -z "$(git -C "$main" status --porcelain --untracked-files=no)" ]] || {
  echo "land: $main has uncommitted changes" >&2; exit 1; }

# Check what will actually land: the branch with main merged in. If main has
# moved on, merge it into the branch first so the checks see the result.
if ! git -C "$tree" merge-base --is-ancestor main "$branch"; then
  echo "land: main has moved on; merging it into $branch first" >&2
  git -C "$tree" merge --no-edit main
fi
( checks "$tree" )

git -C "$main" merge --no-ff --no-edit "$branch"
echo "land: $branch is on main. Remove its worktree when done:" >&2
echo "      git worktree remove $tree && git branch -d $branch" >&2
echo "      and delete its note in $main/status/, if it had one." >&2
