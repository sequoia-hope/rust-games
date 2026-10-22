# Playground Kit (crates/engine): conventions

The shared engine every game here builds on, and that games outside this
repo use by path or git URL. Read the root `CLAUDE.md` first (the rules on
outside consumers, breaking changes and `CHANGELOG.md` apply in full here).
`README.md` is the overview, `docs/PLAN.md` the crate map, seams and
phases, and `docs/DECISIONS.md` every choice the plan did not cover
(numbered D1, D2, …; add one for a choice you make here).

- **Layers.** Layer 0 (`pk_math`, `pk_lockstep`, `pk_canvas`) has no
  engine, browser, ambient randomness or threads; it builds for native and
  wasm alike with `#![forbid(unsafe_code)]`, and every inexact float
  function goes through `pk_math`'s kernel (`check-deps` rejects
  `f64::sin` and friends in `exact_math` crates). Layer 1 (`pk_shell`, and
  the planned `pk_input`, `pk_ui`, `pk_play`, `pk_render`, `pk_audio`) may
  use Bevy. The rules are the `pk_*` entries in the root `deps.toml`.
- **No game in the kit.** A crate here never names a car, a road, a course,
  a station or a level. A game's type becomes a trait or an open data
  type that the game implements.
- **Bevy is pinned exactly** (0.19.1, root `Cargo.toml`) and moves between
  milestones; no third-party Bevy plugin without a DECISIONS entry.
- **Patches** (`patches/*/PATCHED.md` says what and why): wgpu, wgpu-hal
  and bevy_ui_render as every game patches them, until upstream lands
  (`docs/upstream/` holds the drafts). The root `[patch.crates-io]` points
  here; `render` outside this repo does too. `patches/wgpu-30` is for Bevy
  0.20 consumers. Keep each patch minimal and its PATCHED.md current.
- **Two game xtasks call `pk_xtask`** with paths relative to their own
  folder (`set_root`); keep `pk_xtask` free of any one game's layout.
- **`template/`** is a new game's skeleton (its own workspace, excluded
  from this one) and **`js/`** the headless-Chrome tools the games wrap;
  both are part of the kit's public surface.
- **`skills/`** are Claude Code skills for the sub-crafts (engine sound,
  music genre, level viewer, …). Their paths assume the old standalone
  layout in places; fix what you use.
- **Its page is its docs.** `proj up game-engine` (`./serve.sh`) renders every
  tracked `.md` under this folder into `dist/docs/` with `tools/docs.py` and
  serves that: the overview, the changelog, `docs/`, each crate's and patch's
  notes, the skills, the template. A library has no web build to show, so the
  documentation is what the page is for; a doc added here appears on the next
  start, and no list in `tools/docs.py` needs touching (`git ls-files` is the
  list). There is no status-page `index.html` any more.
