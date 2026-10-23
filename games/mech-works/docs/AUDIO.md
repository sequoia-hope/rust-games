# Mech Works: music and sound requirements

For the agent doing Mech Works' audio (music and sound effects). Read the
root `CLAUDE.md` first: its rules (a worktree and branch per task,
`scripts/land.sh`, the `status/` notes, no ports in code) apply in full.
The game's vision is `SPEC.md` beside this file; this file is what the
audio must do and where it fits.

## The one principle

**Sound comes from the simulation, as the walk does.** Nothing about the
mech's motion is animated: every footstep, stagger and limp is the physics
and a learned policy. Its sound must be driven the same way: from the
simulation's state each step (contact forces, joint torques and currents,
motor temperatures, impacts), never from canned cues timed to an
animation. A heavier body, a hot knee or a weak driver should *sound*
different because the numbers are different.

Audio is presentation: it reads the simulation and never writes to it, and
it draws its own random numbers, never the simulation's (`mw_sim::rng`), so
a run with sound and one without step identically.

## What already exists

- **The music engine**, `crates/music` (package `music`): procedural music,
  per sample, also built as its own AudioWorklet wasm
  (`crates/music/web/music-worklet.js`). Both other games' radios play it.
  It is a **shared crate**: read `crates/music/CLAUDE.md`, `PLAN.md`, and
  every note in the main checkout's `status/` (`music-redesign.md` guards
  it: its generator, `genres.rs` and `compose.rs`, is being redesigned).
  Add rather than change; log additions in `crates/music/CHANGELOG.md`.
- **Generated voices**: the Qwen3-TTS tooling in Midnight Playground
  (`games/midnight-playground/tools/dj-voice/`, `radio-voice/`) and
  Snowboarding's announcers (`games/snowboarding/tools/announcers/`). For
  this game the same approach makes the radio chatter and the mech's voice
  (later; see below).
- **The simulation**, `games/mech-works/crates/mw_sim`, and the demo client
  `crates/mw_demo` (Bevy, web and native). These are being written now
  (branch `mech-works/m0-sim`); see "Hooks" for what audio can read.

## Sound effects (first: the tech demos need them)

The tech demos (SPEC.md, "Tech demos") are the first thing shipped. Each
effect below says what drives it.

| Effect | Driven by | Notes |
|---|---|---|
| Footfall | a foot's contact turning on, and its normal force (N; the mech weighs ~4 t, so ~20 kN standing per foot, peaks higher) | Heavy: steel foot on gravel. Weight and impact speed set loudness and low end. A shuffle (contact flickering, small force) must not sound like a stomp. |
| Foot scrape and slide | a foot in contact moving sideways | Gravel crunch under load. |
| Actuator drive | each joint's current (A) and speed (rad/s), 12 joints | Hydraulic/electric servo: whine or hum rising with speed, strain with current. Twelve at once must mix to something, not a wall of noise: think one or two layered beds modulated by totals, plus detail from the loudest joints. |
| Strain at the limit | a joint's torque at its current limit | The motor working as hard as it can: a groan. Damaged joints (lower limit) hit it more. |
| Heat | each motor's winding temperature (°C; 30 ambient, derate from 90, trip at 120) | Coolant/fan rising with temperature; a ticking or hiss near the trip. |
| Derate and trip | a driver derating (above 90 °C) and tripping to zero torque (at 120 °C, back at 85 °C) | A warning tone as it derates, a hard clunk and an alarm when it trips. This is gameplay information: the pilot hears a knee about to quit. |
| Overcurrent fault | a weak driver faulting (zero torque for 0.25 s) | A relay clack and a brief drop of that joint's drive sound. |
| Projectile hit | an impact: impulse (N·s), the link struck, the point | Demo 1. Metal on armour, sized by impulse; a heavy round should ring the hull. |
| Fall | the episode ending in a fall; the torso or a knee striking the ground | A big, heavy, unglamorous crash. |
| Body creak | torso angular speed, total joint torque | The structure flexing under load: subtle. |
| Ambience | none (the scene) | Open gravel lot at the foot of a hill: wind, distant nothing. A dust storm later (SPEC "Hazards"). |
| Interface | the page's controls | Quiet, mechanical clicks for buttons, damage and part swaps. |

Scale and weight matter more than variety: this is a four-tonne machine
that sounds like an engineering object (SPEC's inspirations: MechWarrior 2's
weight, Avatar's AMP suit, Boston Dynamics videos). No sci-fi laser sounds.

## Music

From SPEC.md ("Audio"):

- **An orchestral mode for the music engine**, in the spirit of a John
  Williams score: brassy leitmotifs for the mechs and the big walkers,
  strings and percussion. *In the spirit of*: every melody original, no
  quoted themes (IP guardrails in SPEC.md apply to music too).
- **A leitmotif for the mech companion** that can be stated in different
  moods: heroic (defence), warm (rebuilding), tired and sparse (decline).
  Its decline is "felt through movement before anyone says it"; the music
  should follow, not lead.
- **How the score evolves across the arc** (defence → civil works →
  community → decline) is an open question in the spec. A proposal with
  sketches is welcome; it is the owner's to decide.
- For the tech demos only a short, restrained loop is needed, or none: the
  demos are about the machine's sound.

Being part of `crates/music`, the orchestral mode must work for the other
games' consumers too (a mode, not a change to the existing genres), and
must build into the existing worklet wasm without breaking its size or
real-time budget on a phone.

## Voice (later)

Radio chatter (mission calls, warnings, banter) and the companion mech's
voice, with the Qwen3-TTS approach the other games use. The cast and their
functions are in SPEC.md ("Characters"); names and scripts are not written
yet, so this waits for them. The mech's voice changes as it wears: "its
words fewer but warmer".

## Where the code goes

- Sound effects: a new crate, `games/mech-works/crates/mw_audio` (prefix
  `mw_`, add it to the root `Cargo.toml` members and `[workspace.dependencies]`
  and give it a `deps.toml` entry: no Bevy, so it can also run in an
  AudioWorklet like the music engine does). It reads a per-step **audio
  frame** (below) and writes samples.
- Music: the orchestral mode in `crates/music` (shared; see above).
- Wiring into the client: `games/mech-works/crates/mw_demo`. That crate is
  being written in another branch right now; coordinate before editing it
  (a note in `status/`, or ask the owner).
- Recorded clips (if any are rendered offline): `games/mech-works/audio/`.
  Keep what is committed small; the web build copies it beside the page.

## Hooks: what audio can read

From `mw_sim` today, per physics step (200 Hz) or per policy step (50 Hz):

- `Sim::actuators[j]` (12 joints, order: per leg from the hip down, left
  leg first: hip yaw, hip roll, hip pitch, knee, ankle pitch, ankle roll):
  `torque` (N·m), `current` (A), `spec.max_current`, `temperature` (°C),
  `tripped`, `fault_left` (s; > 0 while faulted), `health`,
  `torque_limit()`.
- `Sim::joint_angles()`, `Sim::joint_velocities()` (rad, rad/s).
- `Sim::foot_contacts()`: per foot, (touching, normal force N).
- `Sim::link_poses()`: each link's position and rotation (for 3D panning).
- `Sim::torso()`: the torso body (velocity, angular velocity).
- `Env::step` returns whether the step ended in a fall (`terminated`).

The demo client also publishes the selected mech's telemetry as JSON on the
page's status global (`lanes::telemetry` in `mw_demo`), ten times a second.

Planned, to make this easy (ask in `status/` if you need it sooner): an
`mw_sim` **audio frame**, one small struct per policy step with the above
plus discrete events (foot strike with force, impact with impulse and link,
trip, fault, fall), so `mw_audio` depends only on that struct; and recorded
**traces** of real runs (walking, a limp, a push, a fall) as files, so sound
can be designed and listened to offline against real data.

## Done means

- Each effect above responds audibly to its driver: louder steps from a
  heavier body, a hotter motor sounds hotter, a trip is unmistakable.
- It runs on the web (Chrome, and Safari on an iPhone, the owner's phone)
  without dropouts, alongside the simulation.
- Offline renders of the traces, for the owner to listen to before it is
  wired in.
