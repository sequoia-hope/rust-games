//! The two tests every simulation owes its players: it is deterministic,
//! and no input can break it.
//!
//! [`check_determinism`] is Midnight Playground's SPEC 4.6 test 4 made
//! generic: clone the state, step both copies through the same inputs, and
//! require equal hashes at every tick (the first differing tick is the
//! answer, not just "they differ"). It also catches an unfaithful `Clone`
//! (a cached value, an identity) and a `hash` that misses state, in so far
//! as the missed state changes a later hash.
//!
//! [`fuzz`] is test 5: seeded random inputs, held for a quarter second at
//! a time as a thumb would, and a predicate the game writes (`no NaN
//! anywhere, every body inside the world, the state no larger than it
//! began`) checked after every tick.

use pk_math::Mulberry32;

use crate::Simulation;

/// Two runs of the same inputs disagreed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The first tick after which the hashes differ.
    pub tick: u32,
    pub first: u64,
    pub second: u64,
}

impl core::fmt::Display for Divergence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "tick {}: hashes {:016x} and {:016x}",
            self.tick, self.first, self.second
        )
    }
}

impl std::error::Error for Divergence {}

/// Steps two clones of `state` through `inputs` (`players` per tick,
/// tick-major; a tail shorter than a tick is ignored) and compares their
/// hashes after every tick. `Ok` carries the final hash.
pub fn check_determinism<S: Simulation>(
    ctx: &S::Ctx,
    state: &S::State,
    inputs: &[S::Input],
    players: usize,
) -> Result<u64, Divergence> {
    assert!(players > 0, "at least one player");
    let mut a = state.clone();
    let mut b = state.clone();
    let mut h = S::hash(&a);
    let hb = S::hash(&b);
    if h != hb {
        return Err(Divergence {
            tick: S::tick(state),
            first: h,
            second: hb,
        });
    }
    let mut ea = Vec::new();
    let mut eb = Vec::new();
    for tick in inputs.chunks_exact(players) {
        S::step(ctx, &mut a, tick, &mut ea);
        S::step(ctx, &mut b, tick, &mut eb);
        ea.clear();
        eb.clear();
        h = S::hash(&a);
        let hb = S::hash(&b);
        if h != hb {
            return Err(Divergence {
                tick: S::tick(&a),
                first: h,
                second: hb,
            });
        }
    }
    Ok(h)
}

/// [`check_determinism`], panicking with the first differing tick.
pub fn assert_deterministic<S: Simulation>(
    ctx: &S::Ctx,
    state: &S::State,
    inputs: &[S::Input],
    players: usize,
) -> u64 {
    match check_determinism::<S>(ctx, state, inputs, players) {
        Ok(h) => h,
        Err(d) => panic!("not deterministic: {d}"),
    }
}

/// The fuzz predicate failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FuzzFailure {
    /// The tick after which the predicate was false.
    pub tick: u32,
    pub seed: u32,
}

impl core::fmt::Display for FuzzFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "fuzz seed {} broke the state at tick {}",
            self.seed, self.tick
        )
    }
}

impl std::error::Error for FuzzFailure {}

/// How a fuzz run is set up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuzz {
    /// Inputs per tick.
    pub players: usize,
    /// Ticks to run.
    pub ticks: u32,
    /// The input generator's seed (reported with a failure).
    pub seed: u32,
    /// Ticks an input is held before the next draw (0 or 1: every tick).
    /// Midnight's fuzzer holds for 30, a quarter second.
    pub hold: u32,
}

/// Steps `state` for `f.ticks` ticks on random inputs and checks `ok`
/// after each. `draw` makes one player's input from the generator; it is
/// called for every player every `f.hold` ticks and the inputs are held in
/// between, as a thumb holds a stick. Returns the final state.
pub fn fuzz<S: Simulation>(
    ctx: &S::Ctx,
    mut state: S::State,
    f: Fuzz,
    mut draw: impl FnMut(&mut Mulberry32, usize) -> S::Input,
    mut ok: impl FnMut(&S::State) -> bool,
) -> Result<S::State, FuzzFailure> {
    let Fuzz {
        players,
        ticks,
        seed,
        hold,
    } = f;
    assert!(players > 0, "at least one player");
    let mut rng = Mulberry32::new(seed);
    let mut inputs: Vec<S::Input> = vec![S::Input::default(); players];
    let mut events = Vec::new();
    let hold = hold.max(1);
    for k in 0..ticks {
        if k % hold == 0 {
            for (p, inp) in inputs.iter_mut().enumerate() {
                *inp = draw(&mut rng, p);
            }
        }
        S::step(ctx, &mut state, &inputs, &mut events);
        events.clear();
        if !ok(&state) {
            return Err(FuzzFailure {
                tick: S::tick(&state),
                seed,
            });
        }
    }
    Ok(state)
}

/// Every value is a finite number: the usual first clause of a fuzz
/// predicate.
pub fn all_finite<'a>(xs: impl IntoIterator<Item = &'a f64>) -> bool {
    xs.into_iter().all(|x| x.is_finite())
}
