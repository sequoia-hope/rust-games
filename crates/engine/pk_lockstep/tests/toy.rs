//! A toy simulation that exercises the whole seam: a few bodies on a torus
//! pushed by two players, a per-tick draw from a named stream, an event
//! when a body wraps, an edge-triggered flag, and a hash over the state.
//! Everything it uses is exact arithmetic, so the tests hold on native and
//! on `wasm32-unknown-unknown` alike.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use pk_lockstep::rng::RngStreams;
use pk_lockstep::{
    Bytes, Fuzz, NetSim, Pacing, Recording, Simulation, SnapshotSim, advance, alpha,
    check_determinism, fuzz,
};
use pk_math::Rng;

/// A player's controls: a push along x and y, and flags. Bit 1 is an edge
/// (a "reset" that teleports the player's body to the origin), bit 2 is
/// "away" (hands off; the simulation ignores it), bit 4 is "absent" (the
/// host-set autopilot bit a client may not claim).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct In {
    ax: i8,
    ay: i8,
    flags: u8,
}

const RESET: u8 = 1;
const AWAY: u8 = 2;
const ABSENT: u8 = 4;

#[derive(Clone, Debug, PartialEq)]
struct Ctx {
    wrap: f64,
    push: f64,
    bodies: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct St {
    tick: u32,
    pos: Vec<[f64; 2]>,
    vel: Vec<[f64; 2]>,
    rng: RngStreams<2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev {
    Wrapped(usize),
    Reset(usize),
}

struct Toy;

impl Toy {
    fn start(ctx: &Ctx, seed: u32) -> St {
        St {
            tick: 0,
            pos: vec![[0.0, 0.0]; ctx.bodies],
            vel: vec![[0.0, 0.0]; ctx.bodies],
            rng: RngStreams::new(seed),
        }
    }
}

impl Simulation for Toy {
    type Ctx = Ctx;
    type State = St;
    type Input = In;
    type Event = Ev;
    const DT: f64 = 1.0 / 120.0;

    fn step(ctx: &Ctx, st: &mut St, inputs: &[In], events: &mut Vec<Ev>) {
        st.tick += 1;
        for b in 0..ctx.bodies {
            // Players push the first bodies; the rest drift on the jitter
            // stream (stream 1), so the stream's position is state.
            let inp = inputs.get(b).copied().unwrap_or_default();
            let driven = b < 2;
            let (ax, ay) = if driven {
                if inp.flags & ABSENT != 0 {
                    (0.0, 0.0)
                } else {
                    (f64::from(inp.ax), f64::from(inp.ay))
                }
            } else {
                let j = st.rng[1usize].next_f64() * 2.0 - 1.0;
                (j, -j)
            };
            if driven && inp.flags & RESET != 0 {
                st.pos[b] = [0.0, 0.0];
                st.vel[b] = [0.0, 0.0];
                events.push(Ev::Reset(b));
                continue;
            }
            let v = &mut st.vel[b];
            v[0] = (v[0] + ax * ctx.push) * 0.99;
            v[1] = (v[1] + ay * ctx.push) * 0.99;
            let p = &mut st.pos[b];
            let mut wrapped = false;
            for k in 0..2 {
                p[k] += v[k] * Toy::DT;
                let w = ctx.wrap;
                let q = p[k] - (p[k] / w).floor() * w;
                if q != p[k] {
                    wrapped = true;
                }
                p[k] = q;
            }
            if wrapped {
                events.push(Ev::Wrapped(b));
            }
        }
    }

    fn hash(st: &St) -> u64 {
        let mut b = Bytes::new();
        b.u32(st.tick);
        for (p, v) in st.pos.iter().zip(&st.vel) {
            b.f64(p[0]);
            b.f64(p[1]);
            b.f64(v[0]);
            b.f64(v[1]);
        }
        for s in st.rng.as_slice() {
            b.u32(s.rng.a);
            b.u32(s.draws);
        }
        b.hash()
    }

    fn tick(st: &St) -> u32 {
        st.tick
    }
}

impl NetSim for Toy {
    const INPUT_BYTES: usize = 3;
    fn encode_input(i: &In, out: &mut Vec<u8>) {
        out.extend_from_slice(&[i.ax as u8, i.ay as u8, i.flags]);
    }
    fn decode_input(b: &[u8]) -> Option<In> {
        if b.len() != 3 || b[2] & !(RESET | AWAY | ABSENT) != 0 {
            return None;
        }
        Some(In {
            ax: b[0] as i8,
            ay: b[1] as i8,
            flags: b[2],
        })
    }
    fn predict(last: In) -> In {
        In {
            flags: last.flags & !RESET,
            ..last
        }
    }
    fn absent(last: In) -> In {
        In {
            flags: ABSENT | (last.flags & AWAY),
            ..In::default()
        }
    }
    fn accept(i: In) -> In {
        In {
            flags: i.flags & !ABSENT,
            ..i
        }
    }
}

impl SnapshotSim for Toy {
    fn snapshot(st: &St, out: &mut Vec<u8>) {
        let mut b = Bytes::new();
        b.u32(st.tick);
        b.u32(st.pos.len() as u32);
        for (p, v) in st.pos.iter().zip(&st.vel) {
            b.f64(p[0]);
            b.f64(p[1]);
            b.f64(v[0]);
            b.f64(v[1]);
        }
        for s in st.rng.as_slice() {
            b.u32(s.rng.a);
            b.u32(s.draws);
        }
        out.extend_from_slice(&b.buf);
    }
    fn restore(_ctx: &Ctx, bytes: &[u8]) -> Option<St> {
        let u32at = |i: usize| Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?));
        let f64at = |i: usize| Some(f64::from_le_bytes(bytes.get(i..i + 8)?.try_into().ok()?));
        let tick = u32at(0)?;
        let n = u32at(4)? as usize;
        let mut pos = Vec::with_capacity(n);
        let mut vel = Vec::with_capacity(n);
        let mut at = 8;
        for _ in 0..n {
            pos.push([f64at(at)?, f64at(at + 8)?]);
            vel.push([f64at(at + 16)?, f64at(at + 24)?]);
            at += 32;
        }
        let mut rng = RngStreams::from_seeds([u32at(at)?, u32at(at + 8)?]);
        rng[0usize].draws = u32at(at + 4)?;
        rng[1usize].draws = u32at(at + 12)?;
        Some(St {
            tick,
            pos,
            vel,
            rng,
        })
    }
}

const CTX: Ctx = Ctx {
    wrap: 10.0,
    push: 40.0,
    bodies: 4,
};

/// Two players' inputs for `ticks` ticks: a deterministic pattern with
/// pushes, holds, a reset edge and an away period.
fn script(ticks: u32) -> Vec<In> {
    let mut v = Vec::with_capacity(ticks as usize * 2);
    for t in 0..ticks {
        let p0 = In {
            ax: if t % 97 < 60 { 3 } else { -2 },
            ay: 1,
            flags: if t == 150 { RESET } else { 0 },
        };
        let p1 = In {
            ax: -1,
            ay: if t % 40 < 20 { 2 } else { -3 },
            flags: if (300..400).contains(&t) { AWAY } else { 0 },
        };
        v.push(p0);
        v.push(p1);
    }
    v
}

#[test]
fn the_toy_is_deterministic_and_its_hash_moves() {
    let st = Toy::start(&CTX, 7);
    let inputs = script(600);
    let h = check_determinism::<Toy>(&CTX, &st, &inputs, 2).unwrap();
    assert_eq!(
        h,
        pk_lockstep::assert_deterministic::<Toy>(&CTX, &st, &inputs, 2)
    );
    assert_ne!(h, Toy::hash(&st), "600 ticks changed the state");
    // A different seed is a different race (the jitter stream is state).
    let other = Toy::start(&CTX, 8);
    assert_ne!(
        check_determinism::<Toy>(&CTX, &other, &inputs, 2).unwrap(),
        h
    );
    // A different input is a different race.
    let mut bent = inputs.clone();
    bent[2 * 500].ax = 0;
    assert_ne!(check_determinism::<Toy>(&CTX, &st, &bent, 2).unwrap(), h);
}

/// A broken simulation (one that reads something outside its state) is
/// caught at the first tick where the two runs part.
#[test]
fn a_divergence_names_the_first_bad_tick() {
    struct Broken;
    impl Simulation for Broken {
        type Ctx = core::cell::Cell<u32>;
        type State = u64;
        type Input = ();
        type Event = ();
        const DT: f64 = 1.0 / 60.0;
        fn step(ctx: &Self::Ctx, st: &mut u64, _: &[()], _: &mut Vec<()>) {
            // A hidden counter outside the state: the second run sees a
            // different world from tick 3 on.
            let calls = ctx.get();
            ctx.set(calls + 1);
            *st += 1 + u64::from(calls >= 2 * 3 - 1 && calls % 2 == 1);
        }
        fn hash(st: &u64) -> u64 {
            *st
        }
        fn tick(st: &u64) -> u32 {
            *st as u32
        }
    }
    let ctx = core::cell::Cell::new(0);
    let err = check_determinism::<Broken>(&ctx, &0, &[(); 10], 1).unwrap_err();
    assert_eq!(err.tick, 3);
    assert_eq!((err.first, err.second), (3, 4));
}

#[test]
fn a_recording_round_trips_and_replays_to_its_checkpoints() {
    let st = Toy::start(&CTX, 11);
    let inputs = script(500);
    let mut rec: Recording<In> = Recording::new(11, 2, b"toy:v1".to_vec());
    let mut run = st.clone();
    let mut ev = Vec::new();
    for (k, tick) in inputs.as_chunks::<2>().0.iter().enumerate() {
        Toy::step(&CTX, &mut run, tick, &mut ev);
        rec.push_tick(tick);
        if (k + 1) % 120 == 0 {
            rec.checkpoint(run.tick, Toy::hash(&run));
        }
    }
    rec.checkpoint(run.tick, Toy::hash(&run));
    assert_eq!(rec.ticks(), 500);
    assert_eq!(rec.checks.len(), 5);
    assert_eq!(rec.inputs_at(151), Some(&inputs[300..302]));
    assert_eq!(rec.inputs_at(0), None);
    assert_eq!(rec.inputs_at(501), None);

    let bytes = rec.encode::<Toy>();
    // Runs: the script holds p0 for up to 60 ticks but p1 flips every 20,
    // so the file is far smaller than one input per tick.
    assert!(bytes.len() < 500 * 2 * 3 / 4, "{} bytes", bytes.len());
    let back = Recording::<In>::decode::<Toy>(&bytes).unwrap();
    assert_eq!(back, rec);

    let end = back.replay::<Toy>(&CTX, st.clone(), &mut ev).unwrap();
    assert_eq!(Toy::hash(&end), Toy::hash(&run));
    assert!(ev.contains(&Ev::Reset(0)), "the reset edge replayed");

    // A wrong first state fails at the first checkpoint, naming it.
    let wrong = Toy::start(&CTX, 12);
    let err = back.replay::<Toy>(&CTX, wrong, &mut ev).unwrap_err();
    assert_eq!(err.tick, 120);
    assert_eq!(err.recorded, rec.checks[0].1);
    assert_ne!(err.got, err.recorded);
}

#[test]
fn a_malformed_recording_is_an_error_not_a_panic() {
    let mut rec: Recording<In> = Recording::new(1, 2, Vec::new());
    for tick in script(50).as_chunks::<2>().0 {
        rec.push_tick(tick);
    }
    rec.checkpoint(50, 0xabc);
    let bytes = rec.encode::<Toy>();
    for n in 0..bytes.len() {
        assert!(
            Recording::<In>::decode::<Toy>(&bytes[..n]).is_err(),
            "cut at {n}"
        );
    }
    let mut long = bytes.clone();
    long.push(0);
    assert!(Recording::<In>::decode::<Toy>(&long).is_err());
    // A forged run count cannot make the decoder allocate: it is checked
    // against the bytes left.
    let mut forged = bytes.clone();
    let runs_at = 6 + 2 + 4 + 1 + 4 + 4;
    forged[runs_at..runs_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Recording::<In>::decode::<Toy>(&forged).is_err());
    // Random bytes never panic.
    let mut rng = pk_math::Mulberry32::new(3);
    for _ in 0..5000 {
        let n = (rng.next_f64() * 64.0) as usize;
        let b: Vec<u8> = (0..n).map(|_| (rng.next_f64() * 256.0) as u8).collect();
        let _ = Recording::<In>::decode::<Toy>(&b);
    }
    // An input the game rejects is an error too.
    let mut bad = bytes;
    bad[runs_at + 4 + 4 + 2] = 0xff; // flags byte of the first input
    assert_eq!(
        Recording::<In>::decode::<Toy>(&bad),
        Err(pk_lockstep::DecodeError("bad input"))
    );
}

#[test]
fn the_net_hooks_strip_edges_and_claims() {
    let last = In {
        ax: 3,
        ay: -1,
        flags: RESET | AWAY,
    };
    assert_eq!(Toy::predict(last).flags, AWAY);
    assert_eq!(
        Toy::absent(last),
        In {
            ax: 0,
            ay: 0,
            flags: ABSENT | AWAY
        }
    );
    assert_eq!(
        Toy::accept(In {
            flags: ABSENT | RESET,
            ..last
        })
        .flags,
        RESET
    );
    let mut b = Vec::new();
    Toy::encode_input(&last, &mut b);
    assert_eq!(b.len(), Toy::INPUT_BYTES);
    assert_eq!(Toy::decode_input(&b), Some(last));
}

#[test]
fn a_snapshot_restores_to_the_same_hash_and_future() {
    let st = Toy::start(&CTX, 5);
    let inputs = script(300);
    let mut ev = Vec::new();
    let mut a = st;
    for tick in inputs.as_chunks::<2>().0 {
        Toy::step(&CTX, &mut a, tick, &mut ev);
    }
    let mut bytes = Vec::new();
    Toy::snapshot(&a, &mut bytes);
    let mut b = Toy::restore(&CTX, &bytes).unwrap();
    assert_eq!(Toy::hash(&a), Toy::hash(&b));
    for tick in inputs.as_chunks::<2>().0 {
        Toy::step(&CTX, &mut a, tick, &mut ev);
        Toy::step(&CTX, &mut b, tick, &mut ev);
    }
    assert_eq!(Toy::hash(&a), Toy::hash(&b));
    assert!(Toy::restore(&CTX, &bytes[..bytes.len() - 1]).is_none());
}

#[test]
fn fuzzing_keeps_the_toy_finite_and_on_the_torus() {
    let st = Toy::start(&CTX, 99);
    let end = fuzz::<Toy>(
        &CTX,
        st,
        Fuzz {
            players: 2,
            ticks: 3000,
            seed: 4,
            hold: 30,
        },
        |rng, _player| In {
            ax: (rng.next_f64() * 255.0 - 128.0) as i8,
            ay: (rng.next_f64() * 255.0 - 128.0) as i8,
            flags: if rng.next_f64() < 0.05 { RESET } else { 0 },
        },
        |st| {
            pk_lockstep::check::all_finite(st.pos.iter().chain(&st.vel).flatten())
                && st
                    .pos
                    .iter()
                    .all(|p| (0.0..CTX.wrap).contains(&p[0]) && (0.0..CTX.wrap).contains(&p[1]))
                && st.pos.len() == CTX.bodies
        },
    )
    .unwrap();
    assert_eq!(end.tick, 3000);
    // The predicate's failure is reported at its tick.
    let err = fuzz::<Toy>(
        &CTX,
        Toy::start(&CTX, 1),
        Fuzz {
            players: 2,
            ticks: 100,
            seed: 1,
            hold: 0,
        },
        |_, _| In::default(),
        |st| st.tick < 42,
    )
    .unwrap_err();
    assert_eq!((err.tick, err.seed), (42, 1));
}

#[test]
fn advance_steps_the_ticks_that_came_due() {
    let p = Pacing::of::<Toy>(6);
    assert_eq!(p.max_ticks, 6);
    assert!((p.max_frame - 1.0 / 20.0).abs() < 1e-12);
    let dt = Toy::DT;
    let mut acc = 0.0;
    // 60 Hz frames: two ticks a frame, never a drift.
    let mut total = 0;
    for _ in 0..600 {
        total += p.advance(&mut acc, 1.0 / 60.0);
    }
    assert_eq!(total, 1200);
    assert!(acc.abs() < 1e-6);
    // Ragged frames still step exactly the time that passed.
    let frames = [1.0 / 60.0, 1.0 / 144.0, 1.0 / 30.0, 0.0, 1.0 / 120.0, 0.011];
    let mut acc = 0.0;
    let mut time = 0.0;
    let mut ticks = 0;
    for k in 0..900 {
        let f = frames[k % frames.len()];
        time += f;
        ticks += p.advance(&mut acc, f);
        assert!((0.0..=1.0).contains(&p.alpha(acc)));
    }
    assert!((f64::from(ticks) * dt + acc - time).abs() < 1e-9);
    // Half a tick: nothing yet, alpha one half; then a whole tick: one.
    let mut acc = 0.0;
    assert_eq!(p.advance(&mut acc, dt / 2.0), 0);
    assert!((p.alpha(acc) - 0.5).abs() < 1e-9);
    assert_eq!(p.advance(&mut acc, dt), 1);
    assert!((p.alpha(acc) - 0.5).abs() < 1e-9);
}

#[test]
fn advance_clamps_a_long_frame_and_caps_the_ticks() {
    let dt = 1.0 / 120.0;
    // A one-second stall is credited as 1/20 s: six ticks, and nothing
    // carried over to the next frame beyond the tolerance.
    let mut acc = 0.0;
    assert_eq!(advance(&mut acc, 1.0, dt, 6, 1.0 / 20.0), 6);
    assert!(acc.abs() < 1e-9, "{acc}");
    assert_eq!(advance(&mut acc, 0.0, dt, 6, 1.0 / 20.0), 0);
    // Exactly 1/20 s is six ticks whatever the rounding.
    let mut acc = 0.0;
    assert_eq!(advance(&mut acc, 1.0 / 20.0, dt, 6, 1.0 / 20.0), 6);
    // The tick cap binds on its own when the clamp is looser: the rest
    // waits in the accumulator for the next frame.
    let mut acc = 0.0;
    assert_eq!(advance(&mut acc, 0.1, dt, 6, 1.0), 6);
    assert!((acc - (0.1 - 6.0 * dt)).abs() < 1e-9);
    assert_eq!(advance(&mut acc, 0.0, dt, 6, 1.0), 6);
    // A clock that went back, or a NaN, adds nothing.
    let mut acc = 0.4 * dt;
    assert_eq!(advance(&mut acc, -1.0, dt, 6, 1.0 / 20.0), 0);
    assert_eq!(advance(&mut acc, f64::NAN, dt, 6, 1.0 / 20.0), 0);
    assert!((acc - 0.4 * dt).abs() < 1e-12);
    assert!((alpha(acc, dt) - 0.4).abs() < 1e-9);
    assert_eq!(alpha(f64::NAN, dt), 0.0);
}

/// The frame loop the kit's session will run, in miniature: prev and curr
/// states, the ticks from `advance`, one input per tick, events per tick.
#[test]
fn a_session_loop_on_the_helpers_matches_a_direct_run() {
    let p = Pacing::of::<Toy>(6);
    let inputs = script(240);
    let mut prev = Toy::start(&CTX, 2);
    let mut curr = prev.clone();
    let mut direct = prev.clone();
    let mut acc = 0.0;
    let mut ev = Vec::new();
    let mut tick = 0usize;
    for frame in 0..200 {
        let dt = if frame % 7 == 0 { 0.3 } else { 1.0 / 60.0 };
        let n = p.advance(&mut acc, dt);
        assert!(n <= 6);
        for _ in 0..n {
            if tick >= 240 {
                break;
            }
            let inp = &inputs[tick * 2..tick * 2 + 2];
            prev.clone_from(&curr);
            Toy::step(&CTX, &mut curr, inp, &mut ev);
            Toy::step(&CTX, &mut direct, inp, &mut ev);
            tick += 1;
            assert_eq!(Toy::tick(&prev) + 1, Toy::tick(&curr));
        }
        assert_eq!(Toy::hash(&curr), Toy::hash(&direct));
    }
    assert_eq!(tick, 240);
}
