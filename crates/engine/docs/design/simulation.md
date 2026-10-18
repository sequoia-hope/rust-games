# Design note: the `Simulation` seam (`pk_lockstep`)

Status: **written 2026-10-16, for the owner's yes before phase 2 wires
Midnight Playground onto it** (PLAN.md section 5 item 1; DECISIONS D6).
The crate exists and is tested (`crates/pk_lockstep`); no game uses it
yet. Read with Midnight's `docs/rust-port/SPEC.md` 3.3, 4.1, 4.3, 4.6 and
9.1 to 9.3, and `MULTIPLAYER.md`.

## 1. What the seam is for

Three pieces of every game in the kit sit on top of "a simulation" and
know nothing else about it:

- the frame loop (`mp_game::play::session` today, `pk_play::Session`
  later): accumulate frame time, run 0 to 6 ticks, keep the state before
  the last tick beside the current one, hand out an alpha and the ticks'
  events;
- the rollback netcode (`mp_net::{client, host}`, `pk_net` later): the
  host steps the authoritative copy and relays final inputs; a client
  predicts ahead, keeps a ring of states, rolls back when a guess was
  wrong, compares hashes, rebuilds from tick 0 when they differ;
- the recorder and the checks (`mp_game::recording`, `mp_sim::replay`,
  `mp_sim::fuzz`, the determinism tests): inputs and hash checkpoints in,
  the same race out.

Each of those is written once in the kit against a trait, and each game
implements the trait. The trait is deliberately the smallest thing those
three consumers need; everything else about a simulation stays the game's.

## 2. The traits

```rust
pub trait Simulation {
    type Ctx;                                   // built once, immutable
    type State: Clone;                          // everything that changes
    type Input: Copy + Default + PartialEq;     // one player, one tick, quantised
    type Event;                                 // what a tick tells the client
    const DT: f64;
    fn step(ctx: &Ctx, state: &mut State, inputs: &[Input], events: &mut Vec<Event>);
    fn hash(state: &State) -> u64;
    fn tick(state: &State) -> u32;
}

pub trait NetSim: Simulation {
    const INPUT_BYTES: usize;
    fn encode_input(input: &Input, out: &mut Vec<u8>);
    fn decode_input(bytes: &[u8]) -> Option<Input>;
    fn predict(last: Input) -> Input;
    fn absent(last: Input) -> Input;
    fn accept(input: Input) -> Input { input }
}

pub trait SnapshotSim: Simulation {
    fn snapshot(state: &State, out: &mut Vec<u8>);
    fn restore(ctx: &Ctx, bytes: &[u8]) -> Option<State>;
}
```

Why each part exists, with the consumer that needs it:

| Item | Needed by |
|---|---|
| `Ctx` separate from `State` | the client ring clones the state up to 32 deep (`ClientRace::ring`) and the session clones it every tick (`prev.clone_from(&curr)`); the context is shared behind an `Arc` and never cloned |
| `State: Clone` | the ring, `prev`/`curr`, the rebuild from `init`, `check_determinism` |
| `Input: Copy + Default + PartialEq` | `Default` is "no input" (`inputs.get(i).copied().unwrap_or_default()` in `step` and `frames_for`); `PartialEq` is the rollback test (`used[k] != confirmed_at(tick)`) and the recording's run-length coding; `Copy` because frames are passed around by value everywhere |
| `Event` | the session's `events`, the client's `record_events`, the host's end-of-race test |
| `DT` | the session's accumulator, the host's `TICK_MS`, the client's clock |
| `step(ctx, state, &[Input], &mut Vec<Event>)` | the one call all three consumers make; the out-vec is appended to, so a frame's ticks accumulate in one vector and the client can slice a tick's events (`events[from..]`) |
| `hash` | the host's `Msg::Hash` every 30 ticks, the client's `our_hashes` and the rebuild's `unrepaired` count, the recorder's `ticks` line, the determinism tests |
| `tick` | `ClientRace::base/local` read `.tick` off the ring's ends; the host's `step_race` loop, `input_log` and the rejoin replay index by it; the recorder's `from`/`tick` fields |
| `INPUT_BYTES` fixed | `proto::R::frames` checks `n * FRAME_BYTES` against the bytes left before allocating; a generic decoder needs the same guard |
| `predict(last)` | `ClientRace::frames_for` and `Host::step_race` both do `flags & !RESET` on a repeated input |
| `absent(last)` | `Host::step_race`'s disconnected branch (`AUTOPILOT | (f.flags & AWAY)`, controls zeroed); given the last input so a flag the others should keep seeing survives |
| `accept(input)` | the host's connected branch strips `AUTOPILOT` from what a client sent, since only the host may set it |
| `SnapshotSim` | nothing today (section 7) |

**Not on the trait.** A `players(state) -> usize`: the host knows the
human count from the lobby (`r.slots.len()`), the client from
`RaceStart::humans`, the recording from its own header; nothing reads it
off the state, so the trait does not ask. `new`/`new_multi`: building the
first state takes game options (a car, a grid, a mode) that the kit cannot
name; the netcode will carry an opaque lobby payload and a game-supplied
`start(ctx, payload) -> State` when phase 3 makes `Host`/`Client` generic
(open question 1). `results`: the host's points table reads
`race::results`; that is a lobby concern and goes with the payload.

**Static methods, not `&self`.** Both games already pass the immutable
part explicitly: Midnight's `step(&lr, &mut st, ...)` with `lr:
&LevelRuntime`, and snowboarding will once `Course` leaves `Session`
(section 4). The netcode owns the context (`Arc<LevelRuntime>`) and the
states (a ring); the session owns a pair. With `&self` there would be an
object with nothing in it, or worse, an object that holds the context and
then has to be shared with the ring and the rebuild. A marker type
(`struct Racing;`) implementing static functions is the shape of what the
code does today, and it keeps `Client<S: NetSim, T: Transport>` generic
over a type rather than over a value. The cost is that a game cannot swap
behaviour at runtime by picking an instance; every such choice (physics
model, mode, assists) is already in `Ctx` or `State` in both games, where
it belongs for determinism anyway.

**`PartialEq`, not `Eq`, on `Input`.** Midnight's `InputFrame` is `Eq`;
snowboarding's `Input` holds `f32`s. A quantised input should be `Eq`, and
the kit's recorder and netcode are only correct when equal inputs are
equal bytes, so the games should quantise. But the trait does not force it:
the checks work on `PartialEq`, and snowboarding can adopt the trait before
it quantises (section 4).

## 3. How Midnight Playground implements it

`SimState` and `InputFrame` are unchanged. A marker type in `mp_sim`:

```rust
pub struct Racing;
impl Simulation for Racing {
    type Ctx = LevelRuntime; type State = SimState;
    type Input = InputFrame; type Event = SimEvent;
    const DT: f64 = race::DT;
    fn step(lr, st, inputs, events) { race::step(lr, st, inputs, events) }
    fn hash(st) { race::hash(st) }
    fn tick(st) { st.tick }
}
```

`race::step` already takes `&[InputFrame]` with a missing entry meaning
"no input", so it is the trait's `step` as it stands. `race::hash` is the
trait's `hash`; it keeps using `trace::fnv1a64`, which becomes `pub use
pk_lockstep::fnv1a64` (same bytes; the published test vectors are pinned
in the kit). `RngStreams` becomes `pk_lockstep::RngStreams<4>` with an enum
`{Ai, Police, Pursuit, Traffic}` for the index, `new(seed)` for the first
three and `reseed(Traffic, 99)` for the fourth; `draws()` returns `[u32;
4]`, which the trace writer maps to its `i32`s. Every golden still
matches: the seeds, the draw order and the hash function are the same.

**The flags become the three `NetSim` hooks.** Today the semantics of
`RESET`, `AUTOPILOT` and `AWAY` are spread over `mp_net::client`,
`mp_net::host` and `mp_sim::race::step`. With the trait:

- `predict(last) = InputFrame { flags: last.flags & !(RESET | SHIFT_UP |
  SHIFT_DOWN), ..last }`. The client's `frames_for` and the host's late
  branch both call it. (`SHIFT_UP`/`SHIFT_DOWN` are edges too; today they
  are repeated when an input is late, which a sim car would feel as a
  double shift. The hook fixes that in one place; open question 3.)
- `absent(last) = InputFrame { flags: AUTOPILOT | (last.flags & AWAY),
  ..default() }`: the host's disconnected branch verbatim. The `AUTOPILOT`
  bit is what `race::step` reads to drive the car with `autopilot()`
  (MULTIPLAYER 2.7), so the takeover stays inside the simulation and every
  device simulates the same car.
- `accept(input) = InputFrame { flags: input.flags & !AUTOPILOT, ..input
  }`: the host's connected branch.
- **"Paused online means an AWAY input" needs nothing from the host.** The
  menu does not stop an online race; the game's per-tick sampler produces a
  frame with `AWAY` set and controls zeroed while the menu is open
  (MULTIPLAYER 2.9). The host relays it like any other input (`accept`
  leaves `AWAY` alone), the simulation ignores the bit, the other clients
  show the marker, and `absent` keeps the bit if that player then drops.
  The only requirement is on the sampler: it is the game's.

`INPUT_BYTES = 6`, and `encode_input`/`decode_input` are `proto::W::frame`
and `proto::R::frame` moved into the impl, so the wire bytes of an input
do not change; the protocol version does not bump for this.

The trace record (`mp_sim::trace`) stays in the game: its field order is
the JS parity format and names cars, rivals, traffic and pursuit. Only
`fnv1a64` moves. The kit's `Bytes` writer is the same shape as
`trace::Writer` (`f64`, `i32`, `bool`, an option) for games that write a
hash by hand; Midnight may keep its own `Writer` or alias it.

The per-tick sampler stays in the game too: `ActionState<CarAction>` to
`Input` with the assists (D1080 to D1084) and the autopilot applied, then
`InputFrame::quantise`. The seam is below it. The kit's `pk_input` (seam 2)
gives the sampler its devices; `pk_lockstep` gives it the type to produce.

**Determinism and fuzz tests.** `check_determinism::<Racing>(&lr, &st,
&inputs, 1)` replaces the hand-written clone-and-step test (SPEC 4.6 test
4) and names the first differing tick. `fuzz::<Racing>` takes
`mp_sim::fuzz::Fuzzer`'s draw (steer, 70 % throttle, 20 % brake, 10 %
handbrake, 20 % nitro) as the closure, `hold: 30`, and the predicate "no
NaN in any body, no body further outside the walls than the JS reaches";
the `Fuzzer` struct itself can stay for the JS-oracle parity runs, which
need its exact draw order.

## 4. What snowboarding's `Session` needs

Three changes, each small, none of them changing how the game plays:

1. **Step over `&[Input]` into an out-vec.** `Session::step(&mut self,
   input: Input)` pushes events onto `self.events` and drives entrant 0
   with `input` and the others with their `Brain`. The trait form is
   `step(course, state, inputs, events)`: entrant `i` without a brain takes
   `inputs[i]` (a missing one is `Input::default()`), and events go to the
   caller's vector. Today's `self.events` field goes; the client already
   drains it once a frame, which is what the out-vec is. This is also what
   makes a second human possible later.
2. **`Course` out of the session into `Ctx`.** `Session` owns `course:
   Course` (848 lines of mountain, kickers, trees, flags). Cloning the
   session clones the mountain, which is why the game keeps one state and
   draws it with no interpolation. With `Course` as `Ctx` and `Session`
   minus the course as `State`, the state is a few riders and a clock,
   cheap to clone every tick for `prev`/`curr` and for a ring. `Rider::step`
   and `Brain::think` already take `&Course`.
3. **A `hash()`.** None exists. One `Bytes` writer over `time`, `state`,
   each rider's position, velocity, orientation, phase and timers, each
   entrant's finish, score and splits, in a fixed order, then `.hash()`.
   With it the game gets the determinism check, the recorder's
   checkpoints and, later, multiplayer's desync detection.

Two notes that are not changes the trait needs: `TICK` is an `f32`
(`1.0 / 120.0`) and the trait's `DT` is an `f64`; the integrator keeps its
`f32`, `DT = 1.0 / 120.0` is for the frame loop. And `Input` has `f32`
axes: it should quantise to a byte or two per axis before the netcode
uses it (section 2's `PartialEq` note), and `noise.rs`'s platform trig
calls must go through `pk_math::kernel` before "same on every platform"
is true (PLAN.md section 7).

## 5. Recording, and the game's replay format

`pk_lockstep::Recording<I>` is `{ seed, players, setup: Vec<u8>, inputs
(players per tick), checks: (tick, hash) }` with a binary encoding
(`PKREC`, run-length by whole ticks, through the `NetSim` codec; a
malformed file is an error, never a panic) and `replay(ctx, first_state,
events)` that steps and compares each checkpoint, reporting the first
mismatch. `setup` is the game's own bytes for whatever rebuilds the
context and the first state (level, car, options); the kit does not read
it.

**Midnight's format is kept, byte-identical; a converter is written.**
`mp-sim replay` reads the client's JSON-lines recording
(`recordings/*.jsonl`), which is a session log (frames, settings, focus,
gamepads, log lines) that happens to contain the replay (`race_start`,
`ticks` with `in` as `steer,throttle,brake,flags[,clutch]*n` and `hash`).
That text coding is `InputFrame`-specific and the file serves an agent
reading the session afterwards, so it is not the kit's format and nothing
in it changes: `mp_sim::replay::{encode, decode, parse}` and the recorder
stay as they are, and `mp-sim replay` keeps working. When Midnight is
wired (phase 2), a function `Recorded -> Recording<InputFrame>` (seed,
`players: 1`, the options as `setup`, `inputs`, `checks`) lets `mp-sim
replay` also accept a `.pkrec`, and the recorder gains a second output: a
`Recording` per race for RECOMMENDATIONS item 1 (replays and ghosts as a
save format). The JSON-lines log stays the agent's view; the binary file
is the player's.

For multiplayer the host's `input_log` (`humans` frames per tick) is
already a `Recording` with `players > 1`; `Host` can emit one per race.

## 6. What stays game-side

- The per-tick sampler: devices to `ActionState<A>` (`pk_input`), assists
  and autopilot, `quantise` to `S::Input`. The seam is below it.
- The trace record format and its `Writer`: JS parity, game names.
- `Input` (what physics integrates) versus `InputFrame` (what the wire and
  the replay carry): the trait sees only the latter.
- Building the first state (`new`, `new_multi`, the grid, the field): game
  options the kit cannot name. The netcode gets an opaque payload
  (section 2, open question 1).
- Ending a race and scoring it (`SimEvent::Results`, `results()`): read by
  the host's points table, a lobby concern.

## 7. The snapshot extension

MULTIPLAYER 6.2 records that there are no snapshots on the wire: a client
that desyncs rebuilds from tick 0 and the confirmed inputs, and a rejoining
player is sent every input so far (`Host::hello` sends the whole log in
chunks of 600 ticks). That is correct and cheap for a race of a few
minutes; it cannot serve a late joiner into a long cruise or a spectator
(RECOMMENDATIONS item 2).

`SnapshotSim` is the hook: `snapshot(state, out)` and `restore(ctx,
bytes) -> Option<State>` with the contract that the restored state hashes
and steps as the original. It is dependency-free: the toy in the tests
writes its bytes with `Bytes`; Midnight would do the same or derive serde
on `SimState` behind an `mp_sim` feature. A kit `serde` feature that
implements `SnapshotSim` for any `State: Serialize + DeserializeOwned` is
the next step, deferred because it needs a byte format crate (postcard or
bincode) and a coherence decision (a blanket impl would stop a game
hand-writing its own), both better taken in phase 3 when `pk_net` adds the
`Snapshot` message and bumps the protocol version anyway.

## 8. An adapter to ggrs, later

`ggrs::Config` wants `type Input: Copy + Clone + PartialEq + bytemuck::Pod
+ Zeroable`, `type State: Clone`, `type Address`. A kit `Simulation` has
`Input: Copy + Default + PartialEq` with a byte codec and `State: Clone`;
an adapter wraps the input in a `#[repr(C)]` fixed array of `INPUT_BYTES`
(`Pod`), calls `S::step` in the `AdvanceFrame` request with the inputs in
player order, uses `S::State` for `SaveGameState`/`LoadGameState` with
`S::hash` as the checksum, and feeds `predict` where ggrs asks for an
input prediction (its default is "repeat the last", without stripping
edges, which is why the hook exists). About a hundred lines in a
`pk_ggrs` crate that depends on ggrs; nothing in `pk_lockstep` changes.
The kit's own netcode stays the host-authoritative star (ECOSYSTEM.md row
6): `absent`, `accept`, the sealed join link and the rebuild have no ggrs
equivalent.

## 9. Open questions for the owner

Marked **Q**; each has a default the work proceeds on if unanswered.

- **Q1. Where does "build the first state" live?** Default: the netcode
  carries an opaque lobby payload (`Vec<u8>` the game encodes) and a
  game-supplied `start(ctx: &Ctx, payload: &[u8]) -> Option<State>` plus
  `players(payload) -> usize`, added to `NetSim` in phase 3, not now.
  Alternative: a `type Setup` associated type with its own codec, which is
  typed but puts lobby shapes into the simulation trait.
- **Q2. One `NetSim` or two?** `accept` is only the host's; `predict` and
  `absent` are both sides'. Default: one trait with `accept` defaulting
  to the identity, so a game that has no host-only flags writes nothing.
- **Q3. Should `predict` strip `SHIFT_UP`/`SHIFT_DOWN` as well as
  `RESET`?** Today a late input repeats a gear change (a sim car would
  double-shift). Default: yes, the hook strips every edge; it is a
  behaviour change for sim cars online, recorded in Midnight's DECISIONS
  when wired.
- **Q4. Recording file extension and where the recorder writes it.**
  Default: `.pkrec` beside the JSON-lines log, one per race, natively and
  as a second download on the web.
- **Q5. Snapshots: serde in the kit (phase 3) or hand-written bytes per
  game?** Default: hand-written now (nothing uses it), serde behind a
  feature in phase 3 with the protocol bump.
- **Q6. Snowboarding's input quantisation.** The trait does not force it;
  the recorder and netcode are only exact with it. Default: quantise when
  snowboarding adopts the seam (phase 6), to `i8` axes and a flag byte.
