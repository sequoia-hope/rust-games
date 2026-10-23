# Mech Works: World & Tech Demo Spec

Oct 23, 2026 · @Sequoia

## Vision

**Mech Works** (working name) is a mech game about staying: fighting for a place inside a broken world and rebuilding it until people can live there again. The mech is a real adaptive robot, a neural locomotion policy running live in the browser, wearing a game costume.

The first build target is not the game. It is a tech demo that shows a learned, damage-tolerant, self-calibrating walking policy reacting to the world in real time, which is the part nobody has shipped in a game.

**Inspirations (feel, not assets)**

- **MechWarrior 2 (1995):** cockpit view, independent torso and legs, heat management, limbs destroyed piece by piece, weight and momentum.
- **Blast Corps (N64):** a machine doing demolition and civil works as the main verb, not just fighting.
- **Avatar AMP suits:** exposed cockpit, hydraulic, industrial, high-quality modern robotics look instead of 90s blocks.
- **Halo: Reach ending:** the machine left behind in a world the war no longer needs.
- **The Martian:** engineering detail as story, not set dressing.
- **Boston Dynamics push-recovery videos:** the tech demo's tone.

## World and setting

A wasteland that is a possible future of ours, after a political crisis. The crisis stays in the background; the story is about rebuilding, rebirth and renewal.

- **Place:** possibly a compressed future California, potentially sharing the world atlas with Midnight Playground once that matures. The first playable space is a small hand-built patch.
- **Mobility:** no cars in the story. Rebuilt roads are for bicycles. Rail is the backbone that reconnects communities.
- **Visible healing:** the home base is a little green spot that spreads across the wasteland as you play. Rivers run again, plants spread, rail and bike routes reconnect.
- **Hazards:** dust storms, collapsing ground, scarce water.
- **Old cars** appear only as wreckage and obstacles.

**IP guardrails.** "Mech" is a generic word and fine to use. Avoid MechWarrior, BattleTech and BattleMech names, specific mech designs and setting lore. Game mechanics are fair game. All mech designs are original; Avatar's AMP suit is a style reference, not a model to copy. The old Sanctuary story world is not used.

## Game arc

The game moves from defense to civil works to community, while the mech that made it possible slowly wears out.

1. **Defense.** You build defenses and fight off raiders and leftover automated war machines to protect a fragile settlement.
2. **Loss.** Early on, enemies steal or burn a crop you had been saving. The settlement regrows a little and barely gets by.
3. **Civil works.** Once your space is largely safe, missions shift to earthworks: clearing ruins, wells, pipelines, river reroutes, bike routes and rail repair.
4. **Rail and trade.** Reconnecting rail lines links other communities. A surplus that piled up earlier with no use becomes tradeable, and trade brings the parts and supplies you lacked. Strength comes from connection.
5. **Farming.** You build and run a solar farming robot, and food production stabilizes.
6. **Life beyond the war.** More time out of the mech, in the workshop and wider community. Real human connection grows.
7. **Decline.** No production capacity exists for new mech parts; everything is salvage. By the end the mech is barely functional, but you have everything you need. How its story ends is left open until the story is written.

**Underlying idea.** Owning the means of production means running production. The workshop, farm and rail network are that production, and the player runs them: scheduling, maintenance, deciding what goes out on the next train.

## Mechanics

The game mixes several distinct modes on purpose.

| Mode | What you do | Ties to |
| --- | --- | --- |
| Mech combat | Cockpit view, independent torso twist, heat management, limb-by-limb damage | MechWarrior 2 feel |
| Heat and water | Water is coolant and also what people need to live; every shot costs survival | Core tension |
| Civil works | Demolition, rubble clearing, wells, pipelines, river reroutes, rail and bridge rebuilding | Blast Corps feel |
| Workshop | Strip salvage into actuators, pumps and armor with wear and quirks; limited hardpoints force tool vs weapon choices | Calibration system |
| Calibration | After a body change, the mech walks miscalibrated, then calibrates to its true parameters | Locomotion policy |
| Base building | Spiral of growth: gardens, homes, workshop; defenses early, civil infrastructure later | Green spot spreading |
| Rail and trade | Reconnect lines, trade surplus with other communities for parts | Supply chain arc |
| Farming robot | Build a solar farming robot in a simple workshop flow, monitor it remotely, ride a small autonomous vehicle out for field repairs | Acorn experience |

Damage and wear use the same adaptive system as calibration: a hurt actuator makes the mech limp for real, with no canned animation.

## Characters

Each character carries a gameplay function. Names are not chosen yet; this is a starting cast to revise.

- **The mech (companion).** Has a voice and persona all game; the relationship is meaningful. As parts fail, its walk becomes careful and its words fewer but warmer. Its decline is felt through movement before anyone says it.
- **The mechanic.** Older tinkerer who runs the workshop, explains parts and wear, first notices the decline.
- **The radio operator.** Mission voice, sends jobs and warnings. Calm under fire. Possible slow romance over the radio.
- **The wingmate.** Another pilot, more scarred, who struggles to let go of the war as peace arrives.
- **The engineer.** Plans civil works, rail and farmland. Voice of the rebuilding era.
- **The elder.** Remembers the world before; knows where the old wells and lines were.
- **The kids.** Afraid of the mech, then devoted to it, then climbing on it. They make progress visible.
- **The scavenger.** Gray-area trader of salvage and rumors; may betray you.
- **The raider leader.** Desperate rather than evil; could eventually join the settlement.
- **Others to cast:** base defenders, friends, lovers, enemies, operators of other communities on the rail line.

## Audio

Both audio systems already exist in rust-games and get extended, not rebuilt.

- **Music engine.** Currently EDM-focused. Add an orchestral mode in the spirit of a John Williams score: brassy leitmotifs for the mechs and big walkers. How the score evolves as the settlement grows is an open question.
- **Voice.** The existing Qwen-based generated voice system produces scripted radio chatter: mission calls, warnings, banter, and the companion mech's voice. Scripts are written per character.

## Tech architecture

One Rust simulation crate is the single source of truth for physics in both training and the game, so the policy never crosses an engine gap.

| Component | Choice | Why |
| --- | --- | --- |
| Engine | Existing rust-games engine, WASM + WebGPU in the browser | Shared with the other games |
| Physics | Rapier (multibody joints for the mech) | Pure Rust; identical in training and browser. MuJoCo is more accurate for robots but is C and awkward to link into a Rust WASM build |
| Training bridge | PyO3 bindings exposing the Rapier mech sim as a Gymnasium-style vector env | Train with standard RL tooling in Python |
| Training | PPO, many parallel envs on the desktop GPU box | Well understood for legged locomotion |
| Inference | Hand-rolled MLP in Rust, weights as a flat binary blob | A few hundred thousand weights runs in microseconds on CPU; no ONNX Runtime, no WebGPU round trip |
| Weight format | Small header (layer sizes, activation, obs/action normalization) + little-endian f32 | Loadable by both Python export and Rust |

**Determinism.** Fixed timestep (e.g. 200 Hz physics, 50 Hz policy), fixed solver iterations, same Rapier version and feature flags in training and game. Add a parity test: replay a recorded action sequence in native and WASM builds and compare trajectories within tolerance.

**WebGPU for inference** is deferred until many mechs run at once; batching dozens of policies into one dispatch could then pay off.

## Locomotion policy

One policy, conditioned on the mech's real body parameters and trained across wide randomized ranges of them, walks every body, damaged or not. Nothing about stumbling, limping or recovering is animated; it is all real mismatch and real adaptation.

### Body parameter vector

The parameters that the workshop and damage system change, normalized to \[-1, 1\]:

- Total mass and payload; center-of-mass offset (x, y, z)
- Limb segment length scale per leg
- Per-actuator torque limit scale (actuator health)
- Per-actuator driver current limit and thermal state
- Ground friction, control latency (randomized only, for robustness)

### Actuator and damage model

Model each joint as a motor plus driver, so that damage creates physical limits rather than scripted effects:

- **Torque from current.** Torque = Kt × current, clamped by the driver's current limit.
- **Thermal model.** Winding temperature rises with I²R and cools toward ambient. Above a threshold, the driver derates; above a hard limit, it trips to zero torque for a cooldown.
- **Overcurrent fault.** A weak driver that is asked for too much current faults briefly, so heavy loading on that leg causes a drop.
- **Health.** Damage lowers the torque and current limits of one actuator.

Motor temperatures are observed, as on a real robot. A limp then emerges naturally: the cheapest way to keep a hot or weak actuator alive is a short, light stance on that leg. That matches what a limp is, a quick step off the side that hurts.

### Observations and actions

- **Proprioception:** joint positions and velocities, previous action, motor temperatures.
- **IMU:** gravity vector in body frame, angular velocity.
- **Command:** target forward velocity, yaw rate.
- **Terrain:** a height scan sampled ahead of and around the feet (needed for branches, boulders and wrecks).
- **Body:** the parameter vector, or its estimate (below).
- **Actions:** joint position targets to per-joint PD controllers that run at the physics rate, through the actuator model.

### Training (teacher, then estimator)

This follows the Rapid Motor Adaptation pattern.

1. **Teacher.** PPO with privileged true body parameters, passed through a small encoder to a latent z. Domain randomization covers parameters, terrain, random impulses on random links and actuator damage events mid-episode.
2. **Estimator.** A small 1D conv over the last \~0.5–1 s of proprioception and actions, trained to regress z (and the explicit parameters as an auxiliary head, for display).
3. **Deploy.** The policy plus estimator runs in the game. The explicit parameter input remains available for the calibration demo.

**Reward:** velocity command tracking, upright posture, low energy and torque, smooth actions, low foot slip, a sensible air time, a thermal penalty, and survival. Episodes end on a fall.

**Curriculum:** start on flat ground at nominal parameters, then widen parameter ranges, impulse magnitude, terrain roughness and damage frequency as tracking reward improves. Sample the parameter space evenly so interpolation between bodies stays smooth.

### Two calibration modes

- **Explicit calibration.** After a workshop change, feed the old parameter vector to the new body, so the stumble is real mismatch. Then interpolate linearly toward the true values over a few seconds, and the gait settles as inputs approach the true set point.
- **Autonomous calibration.** The estimator infers the new body from its own motion history, which is true system identification. The demo can show estimated vs true parameters live.

## Build plan: training and visualizer

Build the training pipeline and a browser view of training progress before any game content.

### Layout in rust-games

The game lives at `games/mech-works/`, with the `mw_` crate prefix (as
Midnight Playground has `mp_` and Snowboarding `sb_`):

```
games/mech-works/
  crates/
    mw_sim/        # Rust: mech model, Rapier world, actuator + thermal model, terrain, rewards
    mw_sim_py/     # PyO3 bindings: vectorized env, reset/step, param + damage sampling
    mw_policy/     # Rust: MLP + estimator inference, weight blob loader
    mw_demo/       # Browser tech demos on the shared engine (crates/engine)
  train/           # Python: PPO teacher, estimator distillation, curriculum, export
  checkpoints/     # Exported weight blobs + metadata JSON (large ones stay out of git)
```

### Steps

1. **Mech model.** An original bipedal mech defined in code or URDF-like data: torso, two legs with 5–6 DoF each (hip yaw, roll and pitch, knee, ankle pitch and roll). Primitive collision shapes; a separate high-quality visual mesh comes later.
2. **Sim core in Rust.** Rapier world, fixed timestep, PD plus actuator plus thermal model, height-field terrain, impulse injector, observation and reward functions. Unit tests for actuator limits and thermal trips.
3. **PyO3 vector env.** Batched step over N envs in one call, using rayon inside Rust. Expose parameter ranges, damage events and a deterministic seed.
4. **Teacher training.** PPO in PyTorch on the GPU box. Log rewards, episode length, tracking error, fall rate and parameter coverage.
5. **Estimator training.** Roll out the teacher, record histories, regress z. Then fine-tune by running the student in the loop.
6. **Export.** Write weight blobs plus normalization stats. A parity test runs the same observations through PyTorch and the Rust MLP and asserts outputs match within 1e-5.
7. **Native and WASM parity.** Replay recorded episodes in native and browser builds and compare trajectories.

### Training visualizer

The aim is to watch learning happen, not just read curves.

- **Checkpoint gallery.** The trainer exports a weight blob every N updates. The browser viewer loads any checkpoint into the same Rust sim, so you can scrub through training and watch the gait go from flailing to walking.
- **Side-by-side.** Several checkpoints walking in parallel lanes on the same seed.
- **Live curves.** Reward, tracking error, fall rate and curriculum stage, served as JSON from the trainer and charted in the viewer.
- **Parameter coverage map.** A 2D scatter of sampled mass and COM values colored by success, to catch dead zones in the parameter space.
- **Inspector.** Per-joint torque, current, temperature and trip state bars; foot contacts; the estimator's guessed parameters vs the true ones.

## Tech demos

Four browser demos, all running the same policy live with no animation. Each has an overlay toggle for the inspector.

### 1. Impact range

- **Scene:** a gravel lot at the foot of a hillside. Bare like a game test map, but it should read as real.
- **Action:** the mech stands or walks slowly while projectiles hit random parts of its body. It staggers, shifts stance and catches itself.
- **Controls:** projectile mass and speed, fire rate, target body part, walk command.

### 2. Endless path

- **Scene:** a dirt path through an endless void. Terrain streams in ahead and dissolves behind.
- **Action:** the mech walks the path on its own while obstacles spawn ahead: irregular tree branches, boulders and old car wrecks. The gait adapts from the height scan.
- **Controls:** obstacle density and size, walk speed, damage events mid-walk.

### 3. Damage and limp

- **Action:** click an actuator to damage it (lower torque, weak driver, overheat). Watch the limp emerge and the motor temperature hover below trip.
- **Overlay:** per-joint temperature and current, trip events.

### 4. Calibration

- **Action:** swap a part (heavier torso, shifted COM, longer shin). The mech starts walking with the old parameters and wobbles for real.
- **Modes:** explicit (linear interpolation toward true parameters, with a slider for duration) or autonomous (the estimator converges on its own, with live estimated vs true bars).

### Procedural environment

All scenery is generated rather than captured: a simulacrum.

- **Hillside:** layered noise, then GPU hydraulic and thermal erosion for channels, slumps and sediment fans. Erosion is the single biggest jump in realism.
- **Gravel:** a few hundred noise-displaced, chipped convex rock meshes, settled with a physics drop, then instanced. Real geometry near the camera.
- **Materials:** procedural PBR shaders with color variation, dust and wetness; triplanar mapping on slopes.
- **Integration:** erosion output decides where gravel collects, so the lot's edge washes out naturally into the hill.
- **Light:** one strong sun, soft shadows, physical sky and haze. Most realism comes from light.
- **Physics vs visuals:** obstacles have simplified collision shapes in Rapier and detailed procedural visual meshes.

## Milestones and open questions

Each milestone ends with something visible in the browser.

1. **M0 Sim parity:** mech model in Rust Rapier, PyO3 env stepping, native and WASM replay match.
2. **M1 First walk:** teacher walks on flat ground at nominal parameters; the checkpoint viewer shows the learning progression.
3. **M2 Robust walk:** randomized body parameters plus impulses; Demo 1 (impact range) on a placeholder ground.
4. **M3 Damage:** actuator, thermal and current model in training; Demo 3 shows a limp emerging.
5. **M4 Calibration:** explicit interpolation demo, then estimator distillation and autonomous calibration; Demo 4.
6. **M5 Terrain:** height scan, obstacle curriculum; Demo 2 (endless path).
7. **M6 Simulacrum:** procedural eroded hillside, gravel, lighting; visual mech mesh in the AMP-inspired style.

**Open questions**

- [ ] Biped or more legs for the first mech? A biped is the most impressive demo, and also the hardest to train.
- [ ] Is Rapier multibody contact good enough for stable legged training, or does a fallback need to be tested early in M0?
- [ ] Visual mech design: who models it, and how the high-quality mesh maps to collision primitives.
- [ ] Does the world reuse the Midnight Playground atlas?
- [ ] Names, the mech companion's personality and voice, and the ending, all to be decided as the story is written.
- [ ] How the orchestral score evolves across the arc.
- [ ] Dual-stick controller: a custom high-quality twin stick (Hall effect, Pico-based USB HID) as a side project.
