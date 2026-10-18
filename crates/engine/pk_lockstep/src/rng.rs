//! Named, draw-counted random streams inside a simulation state.
//!
//! A lockstep simulation must never share a random generator with the
//! presentation, and when two devices disagree it helps to know which
//! stream drew how often. So the state holds a fixed set of [`Stream`]s,
//! each a `Mulberry32` with a draw counter, and the trace or the hash can
//! record the counters. The seeding rule is Midnight Playground's (its SPEC
//! 4.3): stream `k` starts at `seed + 0x9E3779B9 * (k + 1)` in wrapping
//! `u32` arithmetic, which spreads one race seed over the streams without
//! correlating them. A stream a game seeds differently (Midnight's traffic
//! stream has always been seed 99) is reseeded after construction with
//! [`RngStreams::reseed`].
//!
//! The streams are indexed by anything that converts to `usize`, so a game
//! names them with a small enum:
//!
//! ```
//! use pk_lockstep::rng::RngStreams;
//! use pk_math::Rng;
//!
//! #[derive(Clone, Copy)]
//! enum Draw { Ai, Weather }
//! impl From<Draw> for usize {
//!     fn from(d: Draw) -> usize { d as usize }
//! }
//!
//! let mut streams: RngStreams<2> = RngStreams::new(7);
//! let x = streams[Draw::Ai].next_f64();
//! assert!((0.0..1.0).contains(&x));
//! assert_eq!(streams.draws(), [1, 0]);
//! # let _ = Draw::Weather;
//! ```

use core::ops::{Index, IndexMut};

use pk_math::{Mulberry32, Rng};

/// The seeding constant (the golden ratio's fraction in 32 bits).
pub const STREAM_SPREAD: u32 = 0x9E37_79B9;

/// One stream: its generator and how often it has been drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stream {
    pub rng: Mulberry32,
    pub draws: u32,
}

impl Stream {
    pub fn new(seed: u32) -> Stream {
        Stream {
            rng: Mulberry32::new(seed),
            draws: 0,
        }
    }
}

impl Rng for Stream {
    fn next_f64(&mut self) -> f64 {
        self.draws = self.draws.wrapping_add(1);
        self.rng.next_f64()
    }
}

/// The seed of stream `k` under the seeding rule.
pub fn stream_seed(seed: u32, k: u32) -> u32 {
    seed.wrapping_add(STREAM_SPREAD.wrapping_mul(k.wrapping_add(1)))
}

/// `N` named streams. `Copy` and `Hash` so it sits in any plain-data state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RngStreams<const N: usize> {
    streams: [Stream; N],
}

impl<const N: usize> RngStreams<N> {
    /// Every stream from one seed, by the seeding rule.
    pub fn new(seed: u32) -> RngStreams<N> {
        let mut k = 0u32;
        RngStreams {
            streams: [(); N].map(|()| {
                let s = Stream::new(stream_seed(seed, k));
                k += 1;
                s
            }),
        }
    }

    /// Every stream from its own seed.
    pub fn from_seeds(seeds: [u32; N]) -> RngStreams<N> {
        RngStreams {
            streams: seeds.map(Stream::new),
        }
    }

    /// Restarts stream `k` at `seed` with its draw count at zero.
    pub fn reseed(&mut self, k: impl Into<usize>, seed: u32) {
        self.streams[k.into()] = Stream::new(seed);
    }

    /// The draw counts, in stream order (what a trace record carries).
    pub fn draws(&self) -> [u32; N] {
        self.streams.map(|s| s.draws)
    }

    pub fn as_slice(&self) -> &[Stream] {
        &self.streams
    }
}

impl<const N: usize, K: Into<usize>> Index<K> for RngStreams<N> {
    type Output = Stream;
    fn index(&self, k: K) -> &Stream {
        &self.streams[k.into()]
    }
}

impl<const N: usize, K: Into<usize>> IndexMut<K> for RngStreams<N> {
    fn index_mut(&mut self, k: K) -> &mut Stream {
        &mut self.streams[k.into()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Midnight's `RngStreams::new(seed)`: ai, police, pursuit at k = 0, 1, 2.
    #[test]
    fn seeds_follow_the_spread_rule() {
        assert_eq!(stream_seed(1, 0), 0x9E37_79BA);
        assert_eq!(
            stream_seed(1, 1),
            1u32.wrapping_add(0x9E37_79B9u32.wrapping_mul(2))
        );
        let s: RngStreams<4> = RngStreams::new(12345);
        for k in 0..4usize {
            assert_eq!(s[k].rng, Mulberry32::new(stream_seed(12345, k as u32)));
        }
    }

    #[test]
    fn draws_are_counted_per_stream_and_a_reseed_restarts() {
        let mut s: RngStreams<3> = RngStreams::new(9);
        let a = s[2usize].next_f64();
        let _ = s[2usize].next_f64();
        let _ = s[0usize].next_f64();
        assert_eq!(s.draws(), [1, 0, 2]);
        let mut fresh = Mulberry32::new(stream_seed(9, 2));
        assert_eq!(a, fresh.next_f64());
        s.reseed(2usize, 99);
        assert_eq!(s.draws(), [1, 0, 0]);
        assert_eq!(s[2usize].rng, Mulberry32::new(99));
        let t = RngStreams::from_seeds([1, 2, 3]);
        assert_eq!(t[1usize].rng, Mulberry32::new(2));
    }
}
