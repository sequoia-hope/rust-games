# Mech Works: working rules for agents

A mech game whose mech walks on a learned locomotion policy (RL-trained,
running live in the browser). It lives in the rust-games monorepo at
`games/mech-works/`. Read the root `CLAUDE.md` (two levels up) first: its
rules (branches and worktrees, `scripts/land.sh`, `status/`, ports) override
anything here.

**The spec is `docs/SPEC.md`.** It covers the world, the story arc, the
mechanics, the tech architecture, the locomotion policy, the build plan and
the milestones (M0 to M6). Work goes milestone by milestone, and each one
ends with something visible in the browser. Music and sound have their
own requirements: `docs/AUDIO.md`.

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

| Where | What |
|---|---|
| `crates/mw_sim` | the simulation: the mech (`mech.rs`), its motor, driver and thermal model (`actuator.rs`), the Rapier world (`sim.rs`), the ground (`terrain.rs`), the training environment: observations, rewards, curriculum, pushes, damage (`env.rs`) |
| `crates/mw_sim_py` | `mw_sim` as the Python module `mw_sim_py`: a batch of envs stepped in parallel |
| `crates/mw_policy` | the trained networks in plain Rust, from their weight blobs; the parity test against PyTorch |
| `crates/mw_audio` | simulation-driven sound effects (`docs/AUDIO.md`): footfalls, foot slip, 12-joint actuator drive/strain/heat/derate/trip/fault, projectile hull ring, fall crash, body creak, wind; zero dependencies so it also builds as a standalone `AudioWorklet` wasm |
| `crates/mw_demo` | the Bevy client: the training viewer and the tech demos; `web/` is its page (`index.html` the kit's loader, `ui.js` the controls, curves and inspector) |
| `train/` | PPO (`train.py`), the networks and their export (`policy.py`), `build.sh` |
| `checkpoints/<run>/` | a training run's output (git-ignored): checkpoints, `metrics.jsonl`, coverage; `runs.json` lists the runs |
| `assets/policies/` | the policies the demos ship (committed; the parity test reads them) |
| `devlog/` | **the dev log**: `index.html`, newest entry first, with pictures in `img/` |
| `xtask/` | `cargo xtask` here: the web build (Playground Kit's `pk_xtask`) |

## Commands

| What | Command (from `games/mech-works/`) |
|---|---|
| Tests | `cargo nextest run -p mw_sim -p mw_policy -p mw_audio -p mw_demo` |
| Render SFX traces | `cargo run -p mw_audio --release --example render_traces -- /tmp/mw_audio` |
| The Python module | `PYTHON=<a python with torch> train/build.sh` |
| Train | `cd train && python train.py --run <name> --steps 150e6 --curriculum` |
| Web build into `dist/web/` | `cargo xtask web --release` (`--only webgpu\|webgl2`) |
| Native client | `cargo run --release -p mw_demo -- --mode impact --policy assets/policies/teacher-v1.bin` |
| Serve (the page, and `checkpoints/` live) | `./serve.sh` (registered as `mech-works`) |
| The standing benchmark | `cargo run --release -p mw_sim --example stand` |

The page takes `?mode=viewer|impact|damage|calibration`. On the dev server
the viewer reads the trainer's `checkpoints/` as it writes them, so a run
in progress can be watched.

## Rapier lessons (2026-10-23; the dev log has the details)

- Rapier's **stabilization pass is off** (`num_internal_stabilization_iterations
  = 0`): it corrects penetration without friction and slid a loaded foot
  19 cm in 4 s. `examples/slide.rs` measures it.
- A multibody link's **reported velocity is stale**: foot slip is measured
  from collider positions between policy steps.
- **Impulses on a link are lost**: a push is a force held for one physics
  step (`Sim::push`).
- The PD controllers are Rapier's **implicit, force-based joint motors**;
  the actuator model sets each one's torque limit every step.

## The dev log

Keep `devlog/index.html` current: an entry (newest first) for each thing
that evolves, with pictures (screenshots of the demos through the kit's
`js/shot.mjs`, training curves from `devlog/plot_training.py`) and
diagrams (inline SVG). It is how the owner follows the work.
