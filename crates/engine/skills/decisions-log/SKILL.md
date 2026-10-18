---
name: decisions-log
description: Record and ship work the way this owner's repositories expect: a numbered DECISIONS.md entry for every choice the spec did not cover, a DEVIATIONS.md row for every behaviour change against a reference, the commit message shape (imperative subject, prose body, the Co-Authored-By line), push straight to main, parallel work on disjoint files with one owner per shared module, and the port rule. Use at the end of any task in Playground Kit, Midnight Playground or a game built on the kit, and whenever asked how to commit, what to write up, or where a decision goes.
---

# Decisions log: how work is recorded and shipped

Applies to Playground Kit (`~/Software/game-engine`, `docs/DECISIONS.md`),
Midnight Playground (`~/Software/midnight-playground`,
`docs/rust-port/DECISIONS.md` and `DEVIATIONS.md`) and any game made from
the kit's template (its `DECISIONS.md` at the root of `docs/`).

## Read first

- The repository's `CLAUDE.md` ("Working on a package", "Parallel work",
  "Viewing output") and, in Midnight, `docs/rust-port/ROADMAP.md`
  "Working rules for every package".
- The last ten entries of the `DECISIONS.md` you are about to add to, for
  the current number and the voice.
- `~/scripts/README.md`: the port registry and the `proj` CLI.

## DECISIONS.md

One entry per choice the plan or spec did not cover, numbered in sequence
(`D1230`, `D3`), newest at the end, never renumbered or deleted. The
heading is `## D<N> (<date>): <what was chosen>` (the older form
`## D<N>. <title>` with the date in the first line is also in use; follow
the file you are in). The body says, in prose or short bullets:

1. **Who asked and in what words.** Quote the owner: "the roads are no
   longer separated from scenery". The quote is the requirement.
2. **What was chosen**, concretely: names of types, files, constants,
   numbers measured before and after.
3. **Why**, including the alternatives rejected and what each would have
   cost.
4. **What it changes** for other work: a protocol version, a golden
   regenerated, a format, a budget, a test added.
5. **What is still open**, named, so the next session starts there.

An entry is written **in the same commit as the change**. Design notes go
in `docs/vision/*.md` or `docs/design/*.md` first; the DECISIONS entry
points at them and records what was built against them. Cross-reference
by number (`D1208's packing choice stands`).

## DEVIATIONS.md

Only where the project has a reference it is measured against (Midnight
against the JS game). A behaviour change on purpose is one row: the
deviation, the reason, where (SPEC section, work package, the DECISIONS
number). An accidental difference is a bug, not a deviation.

## Commit and push

Straight to `main`; no branches, no PRs, no squash. One commit per
coherent change, with the tests, the docs and the DECISIONS entry inside
it. Shape:

```
Imperative subject under about 70 characters, no trailing period

A prose body in sentences: what the owner asked or saw, what was done
and why, what was measured, what is left. Name the DECISIONS number.
Paragraphs, not bullet lists. No "This commit ...".

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
```

The attribution line is the one the owner's harness gives in the session's
system reminder; use that model's name, and do not add any other
attribution. Before pushing:

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check-deps
git status                 # build output (dist/, target/, parity/report/, recordings/) stays out
git push origin main
```

Pages builds on every push to `main`, so a red push is a broken site.
Across two repositories (the kit and a game that depends on it by sibling
path), push the kit first, then the game, and record the move in both
DECISIONS logs naming the crate that took the code.

## Parallel work

Packages run in parallel only on **disjoint files**. A shared module has
**one owner**; everyone else may add exports to it but not change what
exists. Say which files you own when you start, and do not touch a
sibling agent's files; if you must, send them the change instead. Never
modify a repository you were told is read-only (Midnight while the kit is
carved; Snowboarding while its agent is mid-task).

## The port rule

Every server takes its port from `--port`, then `$PORT`, then `proj port`
(which resolves the current directory to its registered project), then
**fails loudly**. No default, no literal in a script, config, compose
file, doc or test. Output is viewed through the registered server only:

```bash
proj up game-engine            # or ./serve.sh inside the project
proj port                      # the number, when a tool needs it
proj add <name> <dir>          # a new project: allocates a port, scaffolds serve.sh
proj doctor                    # after changing how a project serves itself
```

Every URL in a client is relative, because the game is always served under
a sub-path. Never start `python3 -m http.server <n>` or any other ad-hoc
server to look at output.

## Common mistakes

- A DECISIONS entry that says what but not who asked or why.
- Writing the entry in a later commit, or in a chat only.
- Fixing an owner's bug report without first writing the test that fails
  on it; the entry should name that test.
- Hedged reporting. If tests fail, paste the failure. If a step was
  skipped (no GPU, no network), say which, in the entry and the message.
- A commit body as a bullet list, or a subject in the past tense.
- Touching a sibling's module "just a little".
- A port in a `default=` argument.
