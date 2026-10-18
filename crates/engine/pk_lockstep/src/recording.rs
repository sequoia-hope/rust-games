//! A recording: a seed, the game's own setup bytes, every tick's inputs for
//! every player, and the state's hash at checkpoints. With a deterministic
//! simulation this is a replay, a ghost and a bug report in one small file.
//!
//! The byte format (`PKREC`, version 1, little-endian) is generic over the
//! input through [`crate::NetSim`]'s codec:
//!
//! ```text
//! "PKREC\0"  u16 version  u32 seed  u8 players
//! u32 setup_len  setup bytes
//! u32 ticks  u32 runs  runs × ( u32 count, players × INPUT_BYTES )
//! u32 checks  checks × ( u32 tick, u64 hash )
//! ```
//!
//! Inputs are run-length coded by whole ticks: a run is a tick's set of
//! `players` inputs and how many consecutive ticks had exactly that set.
//! Decoding checks every count against the bytes left before it allocates,
//! so a forged file cannot ask for memory it does not carry; a malformed
//! file is a [`DecodeError`], never a panic.

use crate::{NetSim, Simulation};

pub const MAGIC: &[u8; 6] = b"PKREC\0";
pub const VERSION: u16 = 1;

/// One recorded run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recording<I> {
    /// The simulation's seed.
    pub seed: u32,
    /// Inputs per tick.
    pub players: u8,
    /// Whatever else the game needs to rebuild the context and the first
    /// state (a level id, a car, options), in the game's own encoding.
    pub setup: Vec<u8>,
    /// `players` inputs per tick, tick-major, from tick 1.
    pub inputs: Vec<I>,
    /// `(tick, hash of the state after it)`, in tick order.
    pub checks: Vec<(u32, u64)>,
}

impl<I: Copy + Default + PartialEq> Recording<I> {
    pub fn new(seed: u32, players: u8, setup: Vec<u8>) -> Recording<I> {
        assert!(players > 0, "a recording has at least one player");
        Recording {
            seed,
            players,
            setup,
            inputs: Vec::new(),
            checks: Vec::new(),
        }
    }

    /// Ticks recorded.
    pub fn ticks(&self) -> u32 {
        (self.inputs.len() / usize::from(self.players)) as u32
    }

    /// Records a tick's inputs (a short slice is padded with the default,
    /// a long one cut, as the simulation itself would read them).
    pub fn push_tick(&mut self, inputs: &[I]) {
        let n = usize::from(self.players);
        self.inputs
            .extend((0..n).map(|i| inputs.get(i).copied().unwrap_or_default()));
    }

    /// Records the state's hash after tick `tick` (ticks must come in
    /// order; a repeated tick replaces).
    pub fn checkpoint(&mut self, tick: u32, hash: u64) {
        if let Some(last) = self.checks.last_mut()
            && last.0 == tick
        {
            last.1 = hash;
            return;
        }
        debug_assert!(self.checks.last().is_none_or(|c| c.0 < tick));
        self.checks.push((tick, hash));
    }

    /// The inputs of tick `tick` (from 1), if recorded.
    pub fn inputs_at(&self, tick: u32) -> Option<&[I]> {
        let n = usize::from(self.players);
        let i = (tick as usize).checked_sub(1)? * n;
        self.inputs.get(i..i + n)
    }

    /// The ticks as runs of equal input sets: `(first tick's inputs, count)`.
    fn runs(&self) -> Vec<(&[I], u32)> {
        let n = usize::from(self.players);
        let mut out: Vec<(&[I], u32)> = Vec::new();
        for tick in self.inputs.chunks_exact(n) {
            match out.last_mut() {
                Some((prev, count)) if *prev == tick => *count += 1,
                _ => out.push((tick, 1)),
            }
        }
        out
    }

    /// The recording as bytes.
    pub fn encode<S: NetSim<Input = I>>(&self) -> Vec<u8> {
        let runs = self.runs();
        let mut b = Vec::with_capacity(32 + self.setup.len() + runs.len() * (4 + S::INPUT_BYTES));
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&VERSION.to_le_bytes());
        b.extend_from_slice(&self.seed.to_le_bytes());
        b.push(self.players);
        b.extend_from_slice(&(self.setup.len() as u32).to_le_bytes());
        b.extend_from_slice(&self.setup);
        b.extend_from_slice(&self.ticks().to_le_bytes());
        b.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (inputs, count) in runs {
            b.extend_from_slice(&count.to_le_bytes());
            for i in inputs {
                let at = b.len();
                S::encode_input(i, &mut b);
                debug_assert_eq!(
                    b.len() - at,
                    S::INPUT_BYTES,
                    "encode_input wrote INPUT_BYTES"
                );
            }
        }
        b.extend_from_slice(&(self.checks.len() as u32).to_le_bytes());
        for (tick, hash) in &self.checks {
            b.extend_from_slice(&tick.to_le_bytes());
            b.extend_from_slice(&hash.to_le_bytes());
        }
        b
    }

    /// Bytes back into a recording.
    pub fn decode<S: NetSim<Input = I>>(bytes: &[u8]) -> Result<Recording<I>, DecodeError> {
        let mut r = Reader { b: bytes, at: 0 };
        if r.take(MAGIC.len())? != MAGIC {
            return Err(DecodeError("not a recording"));
        }
        let version = r.u16()?;
        if version != VERSION {
            return Err(DecodeError("unknown recording version"));
        }
        let seed = r.u32()?;
        let players = r.u8()?;
        if players == 0 {
            return Err(DecodeError("no players"));
        }
        let setup_len = r.u32()? as usize;
        let setup = r.take(setup_len)?.to_vec();
        let ticks = r.u32()? as usize;
        let runs = r.u32()? as usize;
        let n = usize::from(players);
        // Each run is at least a count and one tick's inputs.
        if runs
            .checked_mul(4 + n * S::INPUT_BYTES)
            .is_none_or(|need| need > r.left())
        {
            return Err(DecodeError("recording ends early"));
        }
        let mut inputs = Vec::with_capacity(ticks.saturating_mul(n).min(r.left()));
        let mut counted = 0usize;
        for _ in 0..runs {
            let count = r.u32()? as usize;
            let mut tick = Vec::with_capacity(n);
            for _ in 0..n {
                let b = r.take(S::INPUT_BYTES)?;
                tick.push(S::decode_input(b).ok_or(DecodeError("bad input"))?);
            }
            counted = counted.checked_add(count).ok_or(DecodeError("bad run"))?;
            if counted > ticks {
                return Err(DecodeError("runs exceed the tick count"));
            }
            for _ in 0..count {
                inputs.extend_from_slice(&tick);
            }
        }
        if counted != ticks {
            return Err(DecodeError("runs do not cover the tick count"));
        }
        let nchecks = r.u32()? as usize;
        if nchecks.checked_mul(12).is_none_or(|need| need > r.left()) {
            return Err(DecodeError("recording ends early"));
        }
        let mut checks = Vec::with_capacity(nchecks);
        for _ in 0..nchecks {
            let tick = r.u32()?;
            let hash = r.u64()?;
            if checks.last().is_some_and(|c: &(u32, u64)| c.0 >= tick) {
                return Err(DecodeError("checkpoints out of order"));
            }
            checks.push((tick, hash));
        }
        if r.left() != 0 {
            return Err(DecodeError("trailing bytes"));
        }
        Ok(Recording {
            seed,
            players,
            setup,
            inputs,
            checks,
        })
    }

    /// Steps `state` through every recorded tick, comparing the hash at each
    /// checkpoint. `state` must be the simulation's first state for this
    /// recording's seed and setup (the game builds it). Returns the state
    /// after the last tick; on a mismatch, the tick and both hashes.
    pub fn replay<S: Simulation<Input = I>>(
        &self,
        ctx: &S::Ctx,
        mut state: S::State,
        events: &mut Vec<S::Event>,
    ) -> Result<S::State, ReplayError> {
        let n = usize::from(self.players);
        let mut checks = self.checks.iter().peekable();
        for inputs in self.inputs.chunks_exact(n) {
            S::step(ctx, &mut state, inputs, events);
            let tick = S::tick(&state);
            while let Some(&&(t, _)) = checks.peek()
                && t < tick
            {
                checks.next();
            }
            if let Some(&&(t, recorded)) = checks.peek()
                && t == tick
            {
                checks.next();
                let got = S::hash(&state);
                if got != recorded {
                    return Err(ReplayError {
                        tick,
                        recorded,
                        got,
                    });
                }
            }
        }
        Ok(state)
    }
}

/// Why bytes are not a recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for DecodeError {}

/// A replay's hash did not match a checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayError {
    pub tick: u32,
    pub recorded: u64,
    pub got: u64,
}

impl core::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "tick {}: recorded hash {:016x}, replay gave {:016x}",
            self.tick, self.recorded, self.got
        )
    }
}

impl std::error::Error for ReplayError {}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

const SHORT: DecodeError = DecodeError("recording ends early");

impl Reader<'_> {
    fn left(&self) -> usize {
        self.b.len() - self.at
    }
    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        let end = self.at.checked_add(n).ok_or(SHORT)?;
        let s = self.b.get(self.at..end).ok_or(SHORT)?;
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}
