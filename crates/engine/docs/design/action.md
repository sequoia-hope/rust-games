# Design note: the `Action` seam (`pk_input`)

Status: **written 2026-10-16, for the owner's yes before phase 2 wires
Midnight Playground onto it** (PLAN.md section 5 item 2; D1 keeps own code
over bevy_enhanced_input, leafwing-input-manager, virtual_joystick and
bevy_input_focus, ECOSYSTEM.md rows 2 to 5). Nothing is written yet. Read
with Midnight's `docs/rust-port/SPEC.md` 8.2 and its DECISIONS D433 and
D780 to D784, and with `docs/design/simulation.md`, whose per-tick sampler
this seam feeds.

## 1. What the seam is for

Every game in the kit has the same five devices and the same two readers:

- devices: a keyboard, gamepads (the browser's Gamepad API on the web,
  gilrs natively, with rumble on both), on-screen touch controls, a tilt
  sensor, and the mouse standing in for a finger;
- readers: the per-tick sampler, which turns "what the player asks for"
  into the simulation's quantised input (`Simulation::Input`), and the
  menus, which want edges (up, down, confirm, back) with repeat timing.

Today Midnight has 3,900 lines doing this with the car's ten actions
hard-coded in five enums (`gamepad::Action`, `touch::Hold`, the key
tables, `Nav`, the tap list), and snowboarding has 236 lines doing a
quarter of it with a flat `Controls` struct that mixes play axes, menu
edges and touch drawing state. The seam is one trait the game's action
enum implements, one merged state the sampler reads, and devices that
know only the trait. Everything that names a car or a snowboard stays in
the game.

What the kit deliberately borrows from the crates it does not use:
bevy_enhanced_input's **contexts** (menu input and play input are
different maps, active at different times; here that is a kit-owned `Nav`
beside the game's `A`, section 7), leafwing's **`Actionlike` shape**
(a `Copy` enum with an iterable `ALL` and a stable string key, so maps
and the test bridge name actions by text), virtual_joystick's
**floating-versus-fixed anchor** as a per-stick choice (snowboarding's
stick anchors under the thumb, Midnight's stays home, D1206), and
bevy_input_focus's **`NavigationInput`-shaped** edge set so `MenuNav` can
sit on the official directional navigation when bevy #21260 lands.

## 2. The traits and types

```rust
pub trait Action: Copy + Eq + Hash + Send + Sync + 'static {
    const ALL: &'static [Self];
    /// Stable id: the key in saved maps and the test bridge ("lookBack").
    fn key(self) -> &'static str;
    /// The Controller screen's name for it ("Look back").
    fn label(self) -> &'static str;
    /// Fires once per press (`consume`) rather than reading while held.
    fn one_shot(self) -> bool;
    /// What a fresh install binds, per device.
    fn default_keys(self) -> &'static [KeyCode];
    fn default_pad(self) -> &'static [Binding];
    fn from_key(k: &str) -> Option<Self> { Self::ALL.iter().copied().find(|a| a.key() == k) }
}
```

**Axis pairs are neither a trait method nor a `KeyMap` concern.** They
are declared once, where the state is built, as `Axis<A> { neg: A, pos:
A, ramp: DigitalRamp }` (section 2.1). The trait stays flat because a
one-shot action has no partner and a pad binding, a key or a touch pad
each bind one direction (the saved map format already has `left` and
`right` as separate keys, and stays byte-compatible). The `KeyMap` does
not know pairs because it only answers "which actions does this key
hold". The ramp needs per-pair state (the current ramped value), and that
is what `ActionState` owns.

### 2.1 `ActionState<A>`: what the player asks for this tick

```rust
pub struct ActionState<A: Action> { /* axes, values, held, pressed, enabled */ }

impl<A: Action> ActionState<A> {
    pub fn new(axes: Vec<Axis<A>>) -> Self;
    /// Runs once per simulation tick with the tick's `dt`.
    pub fn update(&mut self, dt: f64, frame: &Frame<'_, A>);
    pub fn value(&self, a: A) -> f64;          // 0..1
    pub fn held(&self, a: A) -> bool;          // past half travel
    pub fn axis(&self, i: usize) -> f64;       // -1..1, ramped or analogue
    pub fn axis_analog(&self, i: usize) -> bool;
    /// A one-shot since the last call; posted once a frame, not once a tick.
    pub fn consume(&mut self, a: A) -> bool;
    pub fn post(&mut self, a: A);              // a touch tap, a pad edge
    pub enabled: bool;                         // off: held reads zero, one-shots pass
}

/// This frame's device snapshots, taken by the kit's plugin, read by each tick.
pub struct Frame<'a, A: Action> {
    pub keys: &'a KeySet, pub keymap: &'a KeyMap<A>,
    pub touch: Option<&'a TouchState<A>>,
    pub tilt: Option<&'a TiltSteer>,
    pub pad: &'a PadState<A>,
}
```

**The merge order is a rule per kind of control, not a list of devices.**
The existing `Input::update` is the specification; written generically:

| Control | Rule |
|---|---|
| held scalar (a trigger) | `max` over sources: a key reads 1, a touch pad its amount, a pad binding its value (sticks through the dead zone, triggers raw) |
| held boolean | OR over sources |
| an `Axis` with a **digital** source down (a key, a touch pad, a pad **button** binding) | target is −1, 0 or +1 from the two sides and runs through the `DigitalRamp`; `analog = false`; a held digital side wins over every analogue reading |
| an `Axis` with no digital source | the analogue reading of the largest magnitude wins (touch stick, tilt, pad **axis** binding, each through its own `Curve`); the ramp's state is set to it so a later key ramps from there; `analog = true` when the reading is non-zero |
| one-shot | a rising edge on any source posts it once per frame; `consume` clears it |
| `enabled == false` | every held value and axis reads zero; posted one-shots still get through (pause works on the menu) |

That is today's order (keys → touch → tilt → pad → gate) with one
difference: today a pad stick past its dead zone overrides a touch
stick that is also deflected, where this picks the larger. The two are
both deflected only with a thumb on the screen and a pad in hand; it is
recorded as Q5.

```rust
pub struct Axis<A> { pub neg: A, pub pos: A, pub ramp: DigitalRamp }
/// Units per second: turn in 3.6, back to centre 7.0, counter-steer 9.0.
pub struct DigitalRamp { pub turn_in: f64, pub back: f64, pub counter: f64 }
/// dead zone, end zone, exponent: pad stick (0.12, 0, 1.4), touch stick
/// (0.06, 0.04, 1.25), tilt (2° of roll, 0, 1.3).
pub struct Curve { pub dead: f64, pub end: f64, pub exponent: f64 }
impl Curve { pub fn apply(&self, x: f64) -> f64; }
```

The three curves in the code today are the same shape with different numbers;
one `Curve` in the kit replaces `stick_steer`, `roll_to_steer` and the
inline pad expression, each game passing its constants. The arithmetic
goes through `pk_math::kernel::pow`, as today, so a recording made on the
web replays natively.

### 2.2 `KeyMap<A>`: keyed by `KeyCode`, saved as DOM codes

```rust
pub struct KeyMap<A: Action> { /* KeyCode -> Vec<A>, insertion ordered */ }
impl<A: Action> KeyMap<A> {
    pub fn defaults() -> Self;                       // from `A::default_keys`
    pub fn actions(&self, k: KeyCode) -> &[A];
    pub fn is_bound(&self, k: KeyCode) -> bool;      // `isGameKey`: keep it from the page
    pub fn to_json(&self) -> Value;                  // {"KeyW": ["throttle"], ...}
    pub fn from_json(v: &Value) -> Self;
}
/// Complete both ways (winit's enum is named after the W3C `code` values).
pub fn dom_code(k: KeyCode) -> Option<&'static str>;
pub fn key_code(dom: &str) -> Option<KeyCode>;

/// Keys down in the order they went down, with blur.
pub struct KeySet { /* ... */ }
impl KeySet {
    pub fn down(&mut self, k: KeyCode, repeat: bool) -> bool; // true: a new press
    pub fn up(&mut self, k: KeyCode);
    pub fn blur(&mut self);
}
```

**Decision: `KeyCode` in memory, DOM code strings on disk and in the test
bridge.** Both platforms deliver `KeyCode` (bevy_winit already translates
the browser's `code` to it), so matching on it costs no table at all.
The 24-entry `dom_code` table exists only because the key lists
are strings; it becomes a complete generated table used in two places: a
saved key map (not a feature today, but the shape allows one) and the
page-side `isGameKey`, which stops the arrow keys scrolling. The second is
served by Bevy's `Window::prevent_default_event_handling` for the canvas;
the kit's web glue keeps a `KeyMap::is_bound` hook for a page that wants
the finer rule.

### 2.3 `Pads<A>`: bindings, maps, capture, mute, menu buttons, rumble

```rust
pub enum Binding { Button(usize), Axis { axis: usize, dir: f64, rest: Option<f64> } }
pub struct Pad { pub id: String, pub index: usize, pub mapping: String,
                 pub axes: Vec<f64>, pub buttons: Vec<Button> }          // one frame's snapshot

pub struct PadMap<A> { /* A -> Vec<Binding>, insertion ordered */ }
pub type Maps<A> = Vec<(String /* pad id */, PadMap<A>)>;

pub struct Pads<A: Action> { /* maps, active pad, capture, mute set, rumble queue */ }
impl<A: Action> Pads<A> {
    pub fn new(maps: Maps<A>) -> Self;                   // the default map from `A::default_pad`
    pub fn poll(&mut self, now_ms: f64, pads: &[Pad], rumble: &mut dyn Rumble) -> &PadState<A>;
    pub fn label(&self, a: A) -> String;                 // "RT", "Left stick ←", "Button 7", "—"
    pub fn start_capture(&mut self, a: A);               // 8 s timeout, arms once every control is let go
    pub fn cancel_capture(&mut self);
    pub fn done(&mut self) -> Vec<(A, Option<Binding>)>; // one binding does one thing
    pub fn reset_map(&mut self, pad: Option<&Pad>);
    pub fn hush(&mut self);                              // quiet what is held until let go
    pub fn kick(&mut self, strong: f64, weak: f64, ms: f64);
    pub fn feel(&mut self, strong: f64, weak: f64);
    pub maps_changed: bool;
    pub rumble_on: bool;
}

pub struct PadState<A> { /* per action: value, held; edges; nav: [bool; 7]; connected */ }
impl<A: Action> PadState<A> {
    pub fn value(&self, a: A) -> f64;
    pub fn held(&self, a: A) -> bool;
    pub fn edges(&self) -> &[A];
    /// Per `Axis` side: the sum of axis bindings (analogue) and whether a button binding is held (digital).
    pub fn axis_reading(&self, a: A) -> (f64, bool);
    pub fn nav(&self, n: Nav) -> bool;
}
```

Everything inside `Pads` lifts as it stands, with `Action` replaced by
`A` and the `[f64; 10]` arrays by `A::ALL`-indexed vectors: the
`same_binding` rule, the stick dead zone on bindings resting at zero, the
activity tracker that picks the pad in hand, capture (arm when still, take
the first button or axis past 0.6, trigger-as-axis detection on
non-standard pads), the mute set that keeps the A that picked Resume from
firing the nitro, and the rumble queue (kicks that fade, a 150 ms
continuous buzz, the 80 ms / 0.08 resend rule). The one generalisation is
in the axis reading: today `Left` and `Right` are special-cased to build
`steer_axis` and `digital_left/right`; with `axis_reading(a)` per side,
any `Axis<A>` in the `ActionState` gets the same treatment.

**`Nav` stays kit-owned and pinned to the standard layout** (D-pad and
left stick for the directions, 0 confirm, 1 back, 9 start), whatever the
pad's map says, so a bad map can always be put right from the pad. Its
keyboard side (arrows, Enter, Escape, Tab) is a fixed `KeyMap<Nav>`.

**`PadMapStore`** replaces the poll system's direct `Store` reads:

```rust
pub trait PadMapStore { fn load(&self) -> Value; fn save(&mut self, maps: &Value); }
```

`pk_shell::Store` implements it under the key `padMaps` with the same
JSON (`{"<pad id>": {"throttle": [{"button": 7}], "left": [{"axis": 0,
"dir": -1, "rest": 0}]}}`), so a map saved by the JS game or the Rust
build today still reads. The JSON's action keys are `A::key()`, and an
unknown key is kept and written back untouched, so two games sharing an
origin do not erase each other's rows.

**`Rumble`** is the trait that exists today, with both backends moving into
`pk_input` behind `cfg(target_arch)`: `WebRumble` (`vibrationActuator.
playEffect("dual-rumble")`, else `hapticActuators[0].pulse`, promises
swallowed, D780) and `GilrsRumble` (Bevy's `GamepadRumbleRequest`, Stop
then Add for 140 ms, D781). The game no longer reaches into the poll:

```rust
#[derive(Message)] pub struct RumbleKick { pub strong: f64, pub weak: f64, pub ms: f64 }
#[derive(Message)] pub struct RumbleFeel { pub strong: f64, pub weak: f64 }   // this frame's buzz
#[derive(Message)] pub struct RumbleHush;                                      // a screen opened
```

The kit's `PreUpdate` poll drains the three before it polls, where the
poll system reads the session's kick list, buzz and hush flag directly
today. `feel` is dropped when no pad is connected, as now.

### 2.4 `TouchLayout<A>`: declarative, drawn by one renderer

```rust
pub struct TouchLayout<A> {
    pub stick: Option<StickSpec<A>>,
    pub sliders: Vec<SliderSpec<A>>,
    pub buttons: Vec<ButtonSpec<A>>,     // held pads and one-shot taps
    pub slop: f64,                       // px of forgiveness round each button (14)
    pub insets: Insets,
}
pub struct StickSpec<A> {
    pub anchor: Anchor,                  // Fixed(home) | UnderThumb
    pub zone: Rect,                      // where a thumb may start it
    pub range: f64,                      // px to full lock
    pub curve: Curve,
    pub x: Option<usize>, pub y: Option<usize>,   // which `Axis` each direction drives
    pub knob: f64,
}
pub struct Band<A> { pub from: f64, pub to: f64, pub action: A, pub at_from: f64, pub at_to: f64 }
pub struct SliderSpec<A> { pub track: Rect, pub bands: Vec<Band<A>> }   // fractions of the height, bottom up
pub struct ButtonSpec<A> { pub rect: Rect, pub action: A, pub round: bool }

pub struct TouchState<A: Action> { /* pointers, stick thumb, sliding thumbs, held, amounts, lit, buzz, seen */ }
impl<A: Action> TouchState<A> {
    pub fn down(&mut self, id: u64, x: f64, y: f64) -> Option<A>;   // the one-shot it tapped
    pub fn moved(&mut self, id: u64, x: f64, y: f64);
    pub fn up(&mut self, id: u64);
    pub fn release(&mut self);                                      // blur, hide
    pub fn relayout(&mut self, layout: TouchLayout<A>);             // a thumb on a pad that hid lets go
    pub fn value(&self, a: A) -> f64;
    pub fn held(&self, a: A) -> bool;
    pub fn analog(&self, axis: usize) -> Option<f64>;               // the stick, or None when that axis is on pads
    pub fn stick_offset(&self) -> (f64, f64);                       // for drawing
    pub seen: bool;                                                 // a finger has been used
}
```

The engine lifts from `touch.rs` whole: the pointer map in insertion
order, the nearest-centre pick among the rects the slop admits (so
neighbours do not both light), a thumb that slides from one pad to the
next, the slider read as the hardest of its thumbs, the stick's
dead/end zones, the 140 ms highlight and the `navigator.vibrate(8)` tick.
What leaves the kit is `Layout::new`: the 120 lines of CSS arithmetic
(`--b`, `--pedal-h`, the portrait rules) are a game's taste and become
the game's `fn layout(w, h, insets, Steering, PedalKind) -> TouchLayout<A>`.
The three steering modes and two pedal modes are likewise game-side:
each is a different `TouchLayout` handed to `relayout`.

The renderer is `TouchUiPlugin<A>`: Bevy UI nodes for the stick track
and knob, each slider's bands, fill and knob, each button, positioned
from the layout in CSS px times the page scale (D1194). Colours, band
labels and icons come from a `TouchSkin<A>` the game supplies (label and
icon per action, the accent). Icons drawn once with `pk_canvas` from
SVG path data are one such skin.

**The mouse is a finger** while the touch UI shows: the kit's pointer
adapter turns the left button into pointer `MOUSE_ID` down/move/up, as
the game's event reader does by hand today.

### 2.5 Tilt

`TiltSteer`, `TiltState` (off, waiting, live, none, insecure, ask,
denied), `Answer`, the `TiltWindow` trait with `NoSensor` natively and
the web module (`DeviceOrientationEvent` through `Reflect`, the queued
events and answers, the single listener, the `PENDING`/`GRANTED` latches)
move as they are; `screen_roll` is already pure. `roll_to_steer` becomes
a `Curve` with `full_lock_for(sensitivity)` setting its range. Tilt feeds
the merge as one more analogue reading on an axis the game names
(`TiltSource { axis: usize }`), smoothed by its own 50 ms filter at tick
rate as today.

**Permission inside a gesture by marker, not by control id.** Today the
web gesture bridge keeps two static rect lists filled from named
controls (the start button, the Tilt option) and a `set_want_ask` latch carries
"tilt is chosen on a touch screen". With `pk_shell`'s markers, a game puts
`PermissionOnTap::Motion` on any control whose tap should ask (and
`FullscreenOnTap` beside it where it wants fullscreen); the kit collects
their rects each frame into the gesture bridge, asks motion first and
fullscreen second (D843), and the game toggles the marker on Race as its
steering setting changes. The ids disappear from the web glue.

## 3. How Midnight Playground implements it

```rust
pub enum CarAction { Left, Right, Throttle, Brake, Nitro, Handbrake,
                     LookBack, Camera, Reset, Pause, Music }
impl Action for CarAction { /* key(): "left" .. "pause", "music"; one_shot: Camera, Reset, Pause, Music */ }
```

One enum replaces five: `gamepad::Action`, `touch::Hold`, the `KEYMAP`
and `PRESS` tables, and `touch::TAPS`. `Music` joins because it is a key
one-shot today with no pad row; `PadSetupView::new(&CarAction::ALL[..10])`
shows the ten rows the Controller screen has now. `default_keys` is the
`KEYMAP`/`PRESS` tables, `default_pad` is `default_map()`, so a fresh
install binds exactly what it binds today and every saved `padMaps` entry
reads unchanged. `ActionState::new(vec![Axis { neg: Left, pos: Right,
ramp: DigitalRamp { 3.6, 7.0, 9.0 } }])`.

**The sampler is unchanged in shape and sits where it sits**, in
`Race::frame_with`: per tick, `state.update(DT, &frame)`, then the game's
`quantise`:

```rust
fn quantise(s: &ActionState<CarAction>) -> mp_sim::input::Input   // then the assists, then InputFrame::quantise
```

reading `s.axis(0)`, `s.axis_analog(0)` (the `analog` flag physics
scales by `ANALOG_LOCK`), `s.value(Throttle)`, `s.value(Brake)`,
`s.held(Handbrake | Nitro | LookBack)`. The autopilot and `steer_assist`
(D1080 to D1084) run between it and `InputFrame::quantise` as now; the
`AWAY` frame while paused online, `RESET` from `consume(Reset)` on the
first tick of the frame, `consume(Pause)`, `consume(Camera)` and
`consume(Music)` once a frame are the game's lines they are today. Auto
gas (throttle 1 unless braking, no thumb on the slider, controls shown)
moves from `TouchControls::throttle` into `quantise`, reading
`touch.held(Brake)` and whether a thumb is on the slider; it is a car
rule, not a touch rule.

`gamepad_io::poll` becomes the kit's `InputPlugin<CarAction>` poll:
Midnight's `play::flow` writes `RumbleKick`/`RumbleFeel`/`RumbleHush`
instead of filling `race.kicks`, `race.feel`, `race.hush_pads`; the
Rumble setting sets `pads.rumble_on` and the confirming 300 ms buzz stays
a settings-screen line. `PadsRes.pads.label(Reset)` for the stuck hint is
`pads.label(CarAction::Reset)`. The `__mp.pads` and `__mp.padNav` test
objects become `__game.pads` and `__game.padNav` through `pk_shell`'s
bridge with the same field names; the parity suites change one prefix.

Touch: `Layout::new` and the `Steering`/`PedalKind` settings stay in
`mp_game` and produce a `TouchLayout<CarAction>`: a fixed-anchor stick
on the left 45 % driving axis 0 only, a slider with bands BRAKE (0.06 to
0.30, 1.0 to 0.15), GAS (0.36 to 0.62, 0.15 to 1.0), N₂O (0.82 up), or
the four pedal pads, the ◂ ▸ pads, and the reset/camera/pause taps. The
wheel that tilt turns is a skin element driven by `axis(0)`.

The only behaviour change: `LookBack` is zeroed by the `enabled` gate like
every other held action (today it slips through, which nothing relies on).

## 4. What snowboarding needs

Its `Controls` becomes an enum and loses its drawing fields:

```rust
pub enum RideAction { SteerLeft, SteerRight, PitchUp, PitchDown, Jump, Grab, Pause }
```

with two axes, `Axis { SteerLeft, SteerRight }` and `Axis { PitchUp,
PitchDown }`, both with a ramp (today keys snap to ±1 with no ramp;
`DigitalRamp { turn_in: 1e9, .. }` keeps that if the feel matters, Q6).
`Jump` is held (hold to charge). Its menu edges (`up`, `down`, `left`,
`right`, `confirm`, `back`) are `Nav`, not actions (section 7); `pause`
is both, as in Midnight. Its touch controls are one `TouchLayout`: an
`UnderThumb` stick on the left half driving axes 0 and 1 (range 12 % of
the short side, at least 40 px), a JUMP button on the right 26 % and a
GRAB button between, with the top 16 % left to the HUD. `touch_seen`,
`stick` for drawing and `any` (the audio wake) are `TouchState::seen`,
`stick_offset` and a kit `AnyInput` message. Its 0.18 pad dead zone with
no exponent is `Curve { 0.18, 0.0, 1.0 }`. "Turn the assist on the first
time a finger is seen" stays its line, reading `seen`.

What it gains for free: a Controller screen with remapping and saved
maps, pad rumble, the mute set (its B held from a menu does not grab on
the first frame of a run), tilt steering, and a menu highlight that
repeats while the stick is held.

## 5. Menu navigation: two modes over one `NavInput`

```rust
pub enum Nav { Up, Down, Left, Right, Confirm, Back, Start }
pub struct NavInput { pub fired: [bool; 7], pub held: [bool; 7] }   // 380 ms then every 110 ms on the directions
pub enum NavMode { Spatial, List }
```

`pk_input` produces `NavInput` each frame from the pads' `nav` and the
`KeyMap<Nav>`, with the repeat timing. `pk_ui::MenuNav` consumes it in
one of two modes over the same `Control<A>` list: **spatial** (Midnight's
`nearest` scorer over laid-out rects, in-line first, memory per screen,
edit mode for sliders and selects) and **list** (snowboarding's
`navigate`: index cycling with wrap, left/right adjust the focused row,
confirm activates). The mouse or a finger hides the highlight in both.
Exposing `NavInput` in bevy_input_focus's shape is what keeps row 2 of
ECOSYSTEM.md cheap later.

## 6. What stays game-side

- The action enum, its labels, its default bindings and which rows the
  Controller screen shows.
- `quantise(&ActionState<A>) -> S::Input` and everything between it and
  the wire: assists, autopilot, away/reset flags, auto gas.
- The touch layout function (the CSS arithmetic), the steering and pedal
  modes as settings, and the skin (labels, icons, colours).
- Routing order among the game's own widgets: the radio widget's taps are
  checked before `TouchState::down` in the game's `read_input`, because
  the stick's zone covers the whole left side (D1160). The kit offers
  the touch functions; it does not own the HUD (Q3).
- Which controls carry `PermissionOnTap`/`FullscreenOnTap`, and when.
- The settings themselves (rumble on, tilt sensitivity, steering mode)
  and the confirming buzz when rumble is switched on.

Not game-side, for the record: the debug line of what the pads asked for
(`RUST_LOG=mp_game::play::gamepad_io=debug` today) names actions by
`key()` and moves to the kit as `pk_input::pads=debug`.

## 7. Open questions for the owner

Marked **Q**; each has a default the work proceeds on if unanswered.

- **Q1. Menu actions in the game's enum, or a kit-owned `Nav`?** Default:
  kit-owned `Nav`, pinned to the standard pad layout and a fixed key set,
  separate from `A`. Pause appears in both (`Nav::Start` on menus,
  `A::Pause` in play), as it does in Midnight today. Alternative: fold
  `Nav` into `A` with a `menu()` predicate, which makes menu buttons
  remappable but lets a bad map lock the player out of the Controller
  screen.
- **Q2. Is `ActionState<A>` a Bevy resource or data the session owns?**
  Default: plain data owned by the game's session (Midnight's `Race.input`),
  updated inside the per-tick sampler with `S::DT`, because the ramp is a
  tick-rate integrator and the pause/away logic reads it in the same
  closure. The kit's `InputPlugin<A>` owns the devices as resources
  (`PadsRes<A>`, `TouchRes<A>`, `TiltRes`, `KeysRes`) and a `Devices<A>`
  `SystemParam` snapshots them into a `Frame` once per rendered frame.
- **Q3. Where does "radio widget before touch controls" live?** Default:
  in the game's event routing, as now; the kit exposes `down/moved/up` and
  does not read Bevy's touch messages itself. Alternative: a `TapFirst`
  marker on UI nodes the kit hit-tests before the layout, worth adding
  when a second game needs it.
- **Q4. `KeyCode` or DOM code strings as the key identity?** Default:
  `KeyCode` in memory, DOM strings in JSON and the test bridge, with a
  complete two-way table (section 2.2).
- **Q5. Pad stick versus touch stick, both deflected.** Default: the
  larger magnitude wins (a rule); Midnight lets the pad win (a device
  order). Unreachable in practice; the default is the simpler rule.
- **Q6. Should snowboarding's keys ramp?** Default: yes, with Midnight's
  numbers, when it adopts the seam (phase 6); its agent can set the ramp
  to snap if the feel changes.
- **Q7. Bevy's native rumble sends the weak share to the strong motor**
  (D781). Default: leave it; `GilrsRumble` is where a fix lands once the
  owner has judged it with a real pad.
