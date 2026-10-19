#!/usr/bin/env bash
# Claude Code Stop hook (.claude/settings.json): when a session's branch
# changes a shared crate (crates/) and it has no note in the main checkout's
# status/, remind it once to write one (CLAUDE.md, "Coordination"). Cheap:
# git only, no build. Never blocks twice in a row (stop_hook_active).
set -euo pipefail
input="$(cat)"
[[ "$(jq -r '.stop_hook_active // false' <<<"$input")" == "true" ]] && exit 0
cd "$(jq -r '.cwd // empty' <<<"$input" || true)" 2>/dev/null || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

branch="$(git symbolic-ref --short -q HEAD || true)"
[[ -n "$branch" && "$branch" != main ]] || exit 0

# Shared crates changed on this branch, committed or not.
changed="$( { git diff --name-only main...HEAD -- crates/ 2>/dev/null; git status --porcelain -- crates/ | cut -c4-; } | sort -u)"
[[ -n "$changed" ]] || exit 0

# The main checkout: the first entry of `git worktree list`.
main="$(git worktree list --porcelain | sed -n '1s/^worktree //p')"
note="$main/status/${branch//\//-}.md"
[[ -f "$note" ]] && exit 0

jq -n --arg note "$note" --arg files "$(head -5 <<<"$changed" | tr '\n' ' ')" '{
  decision: "block",
  reason: ("This branch changes shared crates (" + $files + ") but has no coordination note. Per CLAUDE.md (Coordination), write " + $note + " in the format of status/README.md (who, touching, API changes, breaking, requests), then finish.")
}'
