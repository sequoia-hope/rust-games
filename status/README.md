# status/: who is changing what in the shared crates

Parallel sessions coordinate here. The notes live **only in the main
checkout** (`~/Software/rust-games/status/`), so every worktree sees the same
ones: read and write them by that path, not in your worktree's copy. They are
git-ignored; only this README is tracked.

## When

- **Before changing `crates/engine` or `crates/music`:** read every note
  here. If another session is changing the same API, coordinate first.
- **While your branch changes a shared crate:** keep one note,
  `status/<branch>.md` (slashes in the branch name become `-`).
- **When your branch lands** (or is abandoned): delete your note.

A task that touches only one game needs no note.

## Format

```markdown
# <branch>

- **Who:** <session / tmux name / what started it>
- **Updated:** 2026-10-18 17:40
- **Touching:** crates/engine/pk_shell (store), crates/music (radio)
- **API changes:** pk_shell::store::Store::platform gains a `scope` field
- **Breaking:** no  (if yes: who it breaks and how they migrate)
- **Requests:** <asks of other sessions, or "none">
- **Notes:** <anything another session should know first>
```
