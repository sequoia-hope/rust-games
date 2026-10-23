# Mech Works: working rules for agents

A mech game whose mech walks on a learned locomotion policy (RL-trained,
running live in the browser). It lives in the rust-games monorepo at
`games/mech-works/`. Read the root `CLAUDE.md` (two levels up) first: its
rules (branches and worktrees, `scripts/land.sh`, `status/`, ports) override
anything here.

**The spec is `docs/SPEC.md`.** It covers the world, the story arc, the
mechanics, the tech architecture, the locomotion policy, the build plan and
the milestones (M0 to M6). Work goes milestone by milestone, and each one
ends with something visible in the browser.

## Principles (from the spec)

1. **One simulation crate, `mw_sim`, is the only source of physics truth**
   for both training (through the `mw_sim_py` PyO3 bindings) and the game.
   The policy must never cross an engine gap.
2. **Determinism**: a fixed timestep, fixed solver iterations, and the same
   Rapier version and features in training and in the browser. A replay
   parity test (native vs WASM) guards it.
3. **No canned locomotion animation.** Stumbles, limps and recoveries come
   from body-parameter mismatch and the actuator, thermal and damage model.
4. **IP guardrails**: original mech designs only. No MechWarrior, BattleTech
   or BattleMech names, designs or lore.

## Layout

The crates use the `mw_` prefix (`mw_sim`, `mw_sim_py`, `mw_policy`,
`mw_demo`) under `crates/`. Python training lives in `train/`, and exported
weights go in `checkpoints/`. None of these exist yet. When the first crate
lands, add it to the root `Cargo.toml` members and `[workspace.dependencies]`,
and give it a `deps.toml` entry.
