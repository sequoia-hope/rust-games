//! The `Simulation` seam of Playground Kit: what a fixed-tick, deterministic
//! game simulation looks like from the outside, so one frame loop, one
//! rollback netcode, one recorder and one set of determinism checks serve
//! every game. The design note is `docs/design/simulation.md` in the kit.
//!
//! A simulation is plain data stepped by a pure function:
//!
//! - [`Simulation::Ctx`] is everything built once and never changed (a
//!   track, a course, a level's tables). It is passed by reference to every
//!   step and never cloned.
//! - [`Simulation::State`] is everything that changes. It is `Clone` because
//!   the frame loop keeps the previous tick beside the current one and the
//!   rollback client keeps a ring of them.
//! - [`Simulation::Input`] is one player's controls for one tick, already
//!   quantised, so the local simulation, the network and a replay feed the
//!   integrator identical values.
//! - [`Simulation::Event`] is what the simulation tells the client instead
//!   of calling it (sounds, toasts, rumble, camera kicks).
//!
//! Every method is a static function taking the context and the state
//! explicitly rather than `&self`: the implementor is a marker type, the
//! caller owns the context (an `Arc` in the netcode, a plain field in a
//! session) and the state (a ring, a pair, a vector), and nothing needs to
//! be constructed to use the trait. The note's section 2 says why.
//!
//! The crate has no clock, no threads, no hash maps, no randomness of its
//! own beyond `pk_math`'s `Mulberry32`, and `#![forbid(unsafe_code)]`; the
//! kit's `deps.toml` holds it to that.

#![forbid(unsafe_code)]

pub mod check;
pub mod hash;
pub mod pacing;
pub mod recording;
pub mod rng;

pub use check::{Divergence, Fuzz, FuzzFailure, assert_deterministic, check_determinism, fuzz};
pub use hash::{Bytes, fnv1a64};
pub use pacing::{Pacing, advance, alpha};
pub use recording::{DecodeError, Recording, ReplayError};
pub use rng::{RngStreams, Stream};

/// A fixed-tick, deterministic simulation.
///
/// The contract, which every helper in this crate and the kit's frame loop
/// and netcode lean on:
///
/// - `step` advances `state` by exactly one tick of `DT` seconds and raises
///   [`Simulation::tick`] by one. It is a pure function of `(ctx, state,
///   inputs)`: no clock, no global state, every inexact operation through
///   `pk_math::kernel`, so the same call gives the same bits on every
///   platform.
/// - `inputs[i]` is player `i`'s controls. A missing entry (a shorter slice)
///   is no input: the simulation substitutes [`Default`].
/// - `hash` covers every field that affects a later tick, and nothing that
///   does not. Two states with equal hashes must continue identically; the
///   netcode compares hashes to detect a desync and the recorder stores
///   them as checkpoints.
/// - `Clone` of the state is faithful: a clone steps exactly as the
///   original would. [`check_determinism`] tests this.
pub trait Simulation {
    /// Built once, shared, immutable: what a level or a course is made of.
    type Ctx;
    /// Everything that changes. Plain data; cloned by the frame loop every
    /// tick and by the rollback client up to the window's depth, so it
    /// should be small and own no heavy tables (those belong in `Ctx`).
    type State: Clone;
    /// One player's controls for one tick, already quantised.
    type Input: Copy + Default + PartialEq;
    /// What a tick reports to the client.
    type Event;

    /// The tick length in seconds (1/120 in both of the kit's games).
    const DT: f64;

    /// One tick.
    fn step(
        ctx: &Self::Ctx,
        state: &mut Self::State,
        inputs: &[Self::Input],
        events: &mut Vec<Self::Event>,
    );

    /// The state's fingerprint: everything that affects later ticks.
    fn hash(state: &Self::State) -> u64;

    /// Ticks stepped since the state was built (0 for a fresh state).
    fn tick(state: &Self::State) -> u32;
}

/// What the rollback netcode and the recorder need beyond [`Simulation`]:
/// inputs as bytes, and the two guesses the host and the clients make when
/// a player's input is missing.
///
/// - The host steps a tick when it is due, with each player's input if it
///   has arrived and otherwise [`NetSim::predict`] of that player's last
///   one. A client steps ahead of the host with its own input and
///   [`NetSim::predict`] of every other player's last confirmed input.
/// - A player who has disconnected is driven by [`NetSim::absent`], given
///   their last input, until they return. The simulation decides what that
///   means (an autopilot, a parked car, a rider who stops).
/// - [`NetSim::accept`] is the host's filter on what a client may claim in
///   its own input: flags only the host sets (such as "driven by the
///   autopilot") are stripped here. It defaults to the identity.
pub trait NetSim: Simulation {
    /// Bytes of one encoded input on the wire and in a recording; fixed, so
    /// a message's frame count can be checked against its length before
    /// anything is allocated.
    const INPUT_BYTES: usize;

    /// Appends exactly [`NetSim::INPUT_BYTES`] bytes.
    fn encode_input(input: &Self::Input, out: &mut Vec<u8>);

    /// Reads one input from exactly [`NetSim::INPUT_BYTES`] bytes; `None`
    /// if they do not decode (a malformed message is an error, never a
    /// panic).
    fn decode_input(bytes: &[u8]) -> Option<Self::Input>;

    /// The guess for a player whose input for this tick has not arrived:
    /// their last input with its edge-triggered parts (a reset, a gear
    /// change) removed, so an edge is never repeated.
    fn predict(last: Self::Input) -> Self::Input;

    /// The input of a player who has dropped out, given their last one.
    fn absent(last: Self::Input) -> Self::Input;

    /// What the host accepts from a client's own input.
    fn accept(input: Self::Input) -> Self::Input {
        input
    }
}

/// A simulation whose state can be written out and read back, so a client
/// that has desynced or joined late can adopt the host's state instead of
/// re-simulating from tick 0. Dependency-free: a game writes the bytes by
/// hand (the [`Bytes`] writer helps) or with serde behind its own feature.
/// `restore` must give a state whose [`Simulation::hash`] equals the one
/// `snapshot` was taken from.
pub trait SnapshotSim: Simulation {
    fn snapshot(state: &Self::State, out: &mut Vec<u8>);
    fn restore(ctx: &Self::Ctx, bytes: &[u8]) -> Option<Self::State>;
}
