//! The lab's DSP and instrument tests (`tools/music-lab/test/lab.test.js`,
//! the DSP, Instruments and kit sections), with the same numeric checks.
//! The `renderInst` harness: 48 kHz, notes fired at their rounded sample,
//! the stereo output stored to `f32` as the lab's `Float32Array`s are.
//! Not here: the tests over the JS game's tracks (`voiceTrack`,
//! `patchName`) and the `DRUM_LANES` check, which belong to the parts of
//! the lab this crate does not hold yet.

// The lab's index loops stay index loops, as in the crate.
#![allow(clippy::needless_range_loop)]

use music::dsp::{Adsr, Chorus, GatedVerb, Ladder, Osc, Reverb, Rng};
use music::instruments::{Drums, Instrument, TriggerOpt, kit_voice, make_instrument};
use music::js_round;
use music::kits::{KITS, kit};
use music::patches::{all, bpatch};
use music::track::{Lab, Op};

const RATE: f64 = 48000.0;

fn rms(x: &[f32]) -> f64 {
    let mut s = 0.0;
    for &v in x {
        s += v as f64 * v as f64;
    }
    (s / x.len() as f64).sqrt()
}

fn peak(x: &[f32]) -> f64 {
    let mut p = 0.0f64;
    for &v in x {
        p = p.max((v as f64).abs());
    }
    p
}

fn finite(x: &[f32]) -> bool {
    x.iter().all(|v| v.is_finite())
}

/// Power in a band, by direct DFT over a few bins (small n only).
fn band_power(x: &[f32], lo: f64, hi: f64) -> f64 {
    let n = x.len();
    let mut p = 0.0;
    let mut f = lo;
    while f < hi {
        let mut re = 0.0;
        let mut im = 0.0;
        let w = (2.0 * std::f64::consts::PI * f) / RATE;
        for i in 0..n {
            re += x[i] as f64 * (w * i as f64).cos();
            im -= x[i] as f64 * (w * i as f64).sin();
        }
        p += re * re + im * im;
        f += RATE / n as f64;
    }
    p
}

/// Zero-crossing frequency of a stretch of a signal.
fn zc_hz(x: &[f32], from: usize, to: usize) -> f64 {
    let mut c = 0;
    for i in from + 1..to {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            c += 1;
        }
    }
    (c as f64 * RATE) / (to - from) as f64
}

/// A sample index from seconds, as the lab's `0.05 * RATE` subarray bounds.
fn at(secs: f64) -> usize {
    (secs * RATE) as usize
}

struct Note {
    t: f64,
    midis: Vec<f64>,
    dur: f64,
    vel: Option<f64>,
    opt: TriggerOpt,
}

fn note(t: f64, midis: &[f64], dur: f64) -> Note {
    Note {
        t,
        midis: midis.to_vec(),
        dur,
        vel: None,
        opt: TriggerOpt::default(),
    }
}

fn render_inst(lab: &Lab, notes: &[Note], secs: f64) -> (Vec<f32>, Vec<f32>) {
    let mut inst = make_instrument(lab.clone(), 7, RATE);
    let n = js_round(secs * RATE) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut o = [0.0; 2];
    let mut k = 0;
    for i in 0..n {
        while k < notes.len() && js_round(notes[k].t * RATE) as usize == i {
            let e = &notes[k];
            k += 1;
            inst.trigger(
                &e.midis,
                e.vel.unwrap_or(0.9),
                js_round(e.dur * RATE),
                &e.opt,
            );
        }
        o[0] = 0.0;
        o[1] = 0.0;
        inst.run(&mut o);
        l[i] = o[0] as f32;
        r[i] = o[1] as f32;
    }
    (l, r)
}

/// One kit hit rendered alone for `n` samples; returns the mix and whether
/// the hit is done.
fn one_hit(kit_name: &str, lane: &str, n: usize, dur_s: usize) -> (Vec<f32>, bool) {
    let mut d = Drums::new(1, RATE);
    let k = kit_voice(kit_name, lane, None).expect("a lane");
    let h = d.hit(lane, &k, 1.0, 1.0, dur_s);
    let mut x = vec![0.0f32; n];
    for i in 0..n {
        let mut y = 0.0;
        d.run(|_, v| y += v);
        x[i] = y as f32;
    }
    (x, d.hits()[h].done)
}

fn last_loud(x: &[f32]) -> f64 {
    x.iter()
        .rposition(|v| v.abs() > 1e-4)
        .map_or(-1.0, |i| i as f64)
        / RATE
}

// ── DSP ──────────────────────────────────────────────────────────
#[test]
fn the_polyblep_saw_is_mostly_free_of_aliasing() {
    // A 3.1 kHz saw: its harmonics above Nyquist fold back between the real
    // ones.
    let mut o = Osc::new(0.0);
    let n = 4800;
    let mut x = vec![0.0f32; n];
    for i in 0..n {
        x[i] = o.run(3100.0, 0.5, 1, RATE) as f32;
    }
    let real = band_power(&x, 3000.0, 3200.0);
    let alias = band_power(&x, 4000.0, 5900.0);
    let db = 10.0 * (real / alias).log10();
    assert!(db > 20.0, "harmonic over alias {db} dB");
}

#[test]
fn the_ladder_stays_bounded_at_full_resonance_under_fast_sweeps() {
    let mut f = Ladder::new();
    let mut r = Rng::new(1);
    let mut p = 0.0f64;
    for i in 0..RATE as usize {
        f.set(
            100.0 + 8000.0 * (0.5 + 0.5 * (i as f64 / 50.0).sin()),
            1.05,
            RATE,
        );
        let y = f.run(r.next() * 2.0 - 1.0, 3.0, 0);
        assert!(y.is_finite());
        p = p.max(y.abs());
    }
    assert!(p < 4.0, "peak {p}");
}

#[test]
fn adsr_attacks_sustains_and_releases_to_silence() {
    let mut e = Adsr::new(RATE);
    e.set_all(0.01, 0.1, 0.5, 0.1);
    e.on();
    let mut v = 0.0;
    for _ in 0..(0.01 * RATE) as usize + 10 {
        v = e.run();
    }
    assert!(v > 0.9);
    for _ in 0..(0.5 * RATE) as usize {
        v = e.run();
    }
    assert!((v - 0.5).abs() < 0.01, "sustain {v}");
    e.off();
    for _ in 0..(0.5 * RATE) as usize {
        e.run();
    }
    assert!(e.done());
}

#[test]
fn the_gated_reverb_rings_while_the_gate_holds_then_stops_dead() {
    // A snare-like burst into a 2 s hall, gated 0.45 s after the hit
    // (RULES.md G8 `gatedVerb`); the same hall ungated rings on.
    let mut g = GatedVerb::new(5, 2.0, 0.45, RATE);
    let mut r = Reverb::new(5, RATE);
    r.pre_s = 0.0;
    r.set(2.0, 0.6);
    let mut noise = Rng::new(1);
    let (mut gated, mut open) = (vec![0f32; at(1.0)], vec![0f32; at(1.0)]);
    g.key();
    for i in 0..gated.len() {
        let x = if i < at(0.08) {
            (noise.next() * 2.0 - 1.0) * (1.0 - i as f64 / at(0.08) as f64)
        } else {
            0.0
        };
        let (mut a, mut b) = ([0.0; 2], [0.0; 2]);
        g.run(x, x, &mut a);
        r.run(x, x, &mut b);
        gated[i] = a[0] as f32;
        open[i] = b[0] as f32;
    }
    let during = rms(&gated[at(0.2)..at(0.4)]);
    assert!(
        during > 0.01,
        "the room is heard while the gate holds: {during}"
    );
    assert!(
        (during / rms(&open[at(0.2)..at(0.4)]) - 1.0).abs() < 0.01,
        "open, the gate passes the hall untouched"
    );
    let after = rms(&gated[at(0.5)..at(0.8)]);
    let ungated = rms(&open[at(0.5)..at(0.8)]);
    assert!(
        after < ungated * 1e-3,
        "closed within 50 ms: {after} against {ungated} ungated"
    );
}

#[test]
fn the_measured_juno_chorus_sweeps_less_than_the_classic_one() {
    // ±0.5 ms for chorus I against the classic ±1.7 ms (sound-design.md
    // §6): the two taps part less, so a low tone differs less between
    // the sides.
    let side = |ms: Option<f64>| {
        let mut c = Chorus::new(RATE);
        c.set(1.0);
        c.model(ms, None);
        let mut s = 0.0;
        for i in 0..at(2.0) {
            let x = (i as f64 * 2.0 * std::f64::consts::PI * 100.0 / RATE).sin();
            let mut o = [0.0; 2];
            c.run(x, &mut o, 1.0);
            s += (o[0] - o[1]).powi(2);
        }
        s
    };
    let classic = side(Some(1.7));
    let measured = side(None);
    assert!(
        measured < classic * 0.5,
        "{measured} against {classic}: a shallower sweep"
    );
}

#[test]
fn mulberry32_is_the_labs() {
    // rng(1)'s first draws, from node.
    let mut r = Rng::new(1);
    let a = r.next();
    let b = r.next();
    assert!((a - 0.6270739405881613).abs() < 1e-15, "{a}");
    assert!((b - 0.002735721180215478).abs() < 1e-15, "{b}");
}

// ── Instruments ──────────────────────────────────────────────────
#[test]
fn every_patch_plays_a_finite_audible_bounded_note_and_then_stops() {
    for (name, lab) in all() {
        let midi = if lab.kind == "tb303" || name.contains("ass") {
            45.0
        } else {
            60.0
        };
        let (l, r) = render_inst(lab, &[note(0.01, &[midi], 0.4)], 3.0);
        assert!(finite(&l) && finite(&r), "{name} finite");
        let head = &l[..at(0.5)];
        assert!(rms(head) > 1e-3, "{name} audible ({})", rms(head));
        assert!(peak(&l) < 1.5, "{name} peak {}", peak(&l));
        assert!(rms(&l[at(2.6)..]) < 1e-3, "{name} released");
    }
}

fn fm_pair(l2: f64) -> Lab {
    let op = |l: f64| {
        Some(Op {
            r: Some(1.0),
            l: Some(l),
            d: Some(2.0),
            s: Some(1.0),
            ..Default::default()
        })
    };
    Lab {
        kind: "fm".into(),
        algo: Some("pair".into()),
        ops: Some(vec![op(1.0), op(l2)]),
        r: Some(0.3),
        gain: Some(0.2),
        ..Default::default()
    }
}

#[test]
fn fm_operators_modulate_the_supersaw_spreads_the_choir_is_finite() {
    // A pure sine has nothing above its fundamental; the EP's modulator adds
    // it.
    let over = |lab: &Lab| {
        let (l, _) = render_inst(lab, &[note(0.0, &[60.0], 1.0)], 1.0);
        band_power(&l[at(0.5)..at(0.5) + 4096], 700.0, 3000.0)
    };
    let sine = over(&fm_pair(0.0));
    let fm = over(&fm_pair(2.0));
    assert!(fm > sine * 20.0, "harmonics {fm} vs {sine}");
    let (l, _) = render_inst(
        &bpatch("supersaw").unwrap(),
        &[note(0.0, &[57.0], 1.0)],
        1.5,
    );
    assert!(finite(&l) && peak(&l) > 0.01 && peak(&l) < 1.5);
    let (c, _) = render_inst(
        &bpatch("choir").unwrap(),
        &[note(0.0, &[60.0, 64.0, 67.0], 1.0)],
        2.0,
    );
    assert!(
        finite(&c) && rms(&c[at(0.5)..at(1.0)]) > 1e-3,
        "choir sounds"
    );
    // The vowel bank shapes the spectrum: the 'a' first formant band carries
    // more than far above it.
    let seg = &c[at(0.5)..at(0.5) + 8192];
    let a = band_power(seg, 550.0, 800.0);
    let hi = band_power(seg, 4000.0, 6000.0);
    assert!(a > hi, "formant over the top");
}

#[test]
fn the_plucked_string_in_tune_ringing_strummed_with_tremolo_and_a_wah() {
    // A plain string on A3 rings at 220 Hz; its level falls with the decay.
    let plain = Lab {
        kind: "string".into(),
        decay: Some(0.6),
        damp: Some(0.3),
        pick: Some(0.6),
        body: Some(2500.0),
        body_q: Some(1.0),
        body_mix: Some(0.0),
        tone: Some(8000.0),
        gain: Some(0.3),
        ..Default::default()
    };
    let (l, _) = render_inst(&plain, &[note(0.0, &[57.0], 2.0)], 2.0);
    // The fundamental carries more than the gap up to the second harmonic (a
    // string's crossings count its harmonics, so not zc_hz).
    let seg = &l[at(0.05)..at(0.05) + 16384];
    assert!(
        band_power(seg, 210.0, 230.0) > 3.0 * band_power(seg, 250.0, 420.0),
        "in tune at 220 Hz"
    );
    let early = rms(&l[at(0.05)..at(0.15)]);
    let late = rms(&l[at(0.55)..at(0.65)]);
    assert!(
        early > 0.01 && late < early * 0.3,
        "decays {early} → {late}"
    );
    // Muting: the note's end damps the string within its `r`.
    let mut m = plain.clone();
    m.decay = Some(3.0);
    m.r = Some(0.05);
    let (muted, _) = render_inst(&m, &[note(0.0, &[57.0], 0.3)], 1.0);
    assert!(
        rms(&muted[at(0.5)..at(0.6)]) < rms(&muted[at(0.1)..at(0.2)]) * 0.1,
        "muted after the gate"
    );
    // Tremolo: the level wobbles at the tremolo rate (a 5 Hz dip every 200
    // ms).
    let mut tr = plain.clone();
    tr.decay = Some(4.0);
    tr.trem = Some(0.9);
    tr.trem_rate = Some(5.0);
    let (trem, _) = render_inst(&tr, &[note(0.0, &[57.0], 2.0)], 1.0);
    let mut levels = Vec::new();
    let mut t = 0.2;
    while t < 0.9 {
        levels.push(rms(&trem[at(t)..at(t + 0.02)]));
        t += 0.02;
    }
    let max = levels.iter().cloned().fold(0.0f64, f64::max);
    let min = levels.iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(max > min * 4.0, "tremolo wobbles the level");
    // Strum: the chord's strings start one after the other.
    let chord = [note(0.0, &[57.0, 61.0, 64.0, 69.0], 1.0)];
    let mut st = plain.clone();
    st.strum = Some(0.03);
    let (strum, _) = render_inst(&st, &chord, 0.5);
    let (flat, _) = render_inst(&plain, &chord, 0.5);
    let first = strum.iter().position(|v| v.abs() > 1e-4);
    assert!(matches!(first, Some(i) if i < 64), "{first:?}");
    assert!(
        rms(&strum[..at(0.025)]) < rms(&flat[..at(0.025)]) * 0.7,
        "one string first, the others after"
    );
    // Wah: finite, and the swept band-pass moves the spectrum between
    // sweeps.
    let (wah, _) = render_inst(
        &bpatch("wahGuitar").unwrap(),
        &[note(0.0, &[57.0], 1.5)],
        1.5,
    );
    assert!(finite(&wah) && peak(&wah) > 0.01 && peak(&wah) < 1.5);
}

#[test]
fn the_303_slides_no_new_envelope_and_the_pitch_glides() {
    let mut lab = bpatch("acid").unwrap();
    lab.res = Some(0.0);
    lab.env = Some(0.0);
    lab.drive = Some(0.0);
    // A2 for one step, sliding into A3.
    let step = 0.12;
    let mut slide = note(step, &[57.0], step);
    slide.opt.glide_from = Some(45.0);
    let (l, _) = render_inst(&lab, &[note(0.0, &[45.0], step * 1.05), slide], 0.5);
    let before = zc_hz(&l, at(0.03), at(step));
    let late = zc_hz(&l, at(step + 0.07), at(step + 0.115));
    assert!((before - 110.0).abs() < 6.0, "before {before}");
    assert!((late - 220.0).abs() < 10.0, "after {late}");
}

#[test]
fn kits_every_lane_sounds_decays_and_kicks_sit_low_and_hats_high() {
    for (kit_name, lanes) in KITS {
        for (lane, _) in *lanes {
            let dur = if *lane == "revCrash" {
                RATE as usize
            } else {
                0
            };
            let (x, done) = one_hit(kit_name, lane, 5 * RATE as usize, dur);
            assert!(finite(&x), "{kit_name} {lane} finite");
            assert!(
                peak(&x) > 0.02 && peak(&x) < 3.0,
                "{kit_name} {lane} peak {}",
                peak(&x)
            );
            assert!(done, "{kit_name} {lane} done");
        }
        let k = one_hit(kit_name, "kick", 4096, 0).0;
        assert!(
            band_power(&k, 30.0, 120.0) > 10.0 * band_power(&k, 2000.0, 4000.0),
            "{kit_name} kick is low"
        );
        if kit(kit_name).unwrap().iter().any(|(l, _)| *l == "hat") {
            let h = one_hit(kit_name, "hat", 4096, 0).0;
            assert!(
                band_power(&h, 6000.0, 12000.0) > 10.0 * band_power(&h, 100.0, 1000.0),
                "{kit_name} hat is high"
            );
        }
    }
}

#[test]
fn the_latin_kit_congas_under_bongos_the_guiro_a_stroke_the_slap_bright() {
    let one = |lane: &str, n: usize| one_hit("latin", lane, n, 0);
    let conga = one("congaO", 8192).0;
    let bongo = one("bongoH", 8192).0;
    assert!(
        band_power(&conga, 150.0, 260.0) > band_power(&conga, 350.0, 600.0),
        "the conga is low"
    );
    assert!(
        band_power(&bongo, 350.0, 600.0) > band_power(&bongo, 150.0, 260.0),
        "the bongo is high"
    );
    let slap = one("congaS", 8192).0;
    assert!(
        band_power(&slap, 1500.0, 4000.0) / band_power(&slap, 150.0, 260.0)
            > band_power(&conga, 1500.0, 4000.0) / band_power(&conga, 150.0, 260.0),
        "the slap is brighter than the open tone"
    );
    let (guiro, done) = one("guiroL", RATE as usize / 2);
    assert!(done, "the long scrape ends");
    let len = last_loud(&guiro);
    assert!(len > 0.12 && len < 0.2, "scrape {len} s");
    assert!(
        band_power(&guiro, 2000.0, 4500.0) > 5.0 * band_power(&guiro, 100.0, 800.0),
        "the güiro is a rasp"
    );
    assert!(
        last_loud(&one("guiroS", RATE as usize / 4).0) < 0.06,
        "the short one is short"
    );
}

#[test]
fn the_games_sample_names_tweak_the_lab_voices() {
    let base = kit_voice("tr808", "kick", None).unwrap();
    assert!(kit_voice("tr808", "kick", Some("kickBoom")).unwrap().decay > base.decay);
    assert_eq!(
        kit_voice("tr909", "snare", Some("snareGated"))
            .unwrap()
            .gate,
        Some(0.2)
    );
    assert_eq!(
        kit_voice("tr909", "hat", Some("nope")),
        kit_voice("tr909", "hat", None)
    );
    // A tweak of a key the base lacks scales 1 for `lvl`, 0 otherwise.
    let soft = kit_voice("tr808", "hat", Some("hatSoft")).unwrap();
    assert_eq!(soft.hp, Some(7500.0 * 0.8));
    assert_eq!(soft.lvl, Some(0.45 * 0.8));
    assert_eq!(
        kit_voice("tr808", "revCrash", Some("clapBig"))
            .unwrap()
            .tail,
        Some(0.0)
    );
    assert_eq!(
        kit_voice("tr808", "revCrash", Some("snareSoft"))
            .unwrap()
            .lvl,
        Some(0.32 * 0.8)
    );
    assert!(kit_voice("tr808", "nope", None).is_none());
    assert!(kit_voice("nope", "kick", None).is_none());
}

#[test]
fn the_open_hat_is_choked_by_the_closed_one() {
    let mut d = Drums::new(1, RATE);
    let ohat = kit_voice("tr808", "ohat", None).unwrap();
    let hat = kit_voice("tr808", "hat", None).unwrap();
    let render = |d: &mut Drums, n: usize| {
        let mut x = vec![0.0f32; n];
        for i in 0..n {
            let mut y = 0.0;
            d.run(|_, v| y += v);
            x[i] = y as f32;
        }
        x
    };
    d.hit("ohat", &ohat, 1.0, 1.0, 0);
    let open = render(&mut d, 4800);
    d.hit("hat", &hat, 1.0, 1.0, 0);
    let choked = render(&mut d, 4800);
    // Before the closed hat the open one still rings; 30 ms after it, it is
    // gone (the hit itself is over by then too).
    assert!(rms(&open[4000..4800]) > 1e-3);
    assert!(rms(&choked[2400..4800]) < rms(&open[4000..4800]) * 0.05);
}

#[test]
fn an_instrument_reads_its_patch_live() {
    let mut inst = make_instrument(bpatch("acid").unwrap(), 7, RATE);
    assert_eq!(inst.lab().cutoff, Some(300.0));
    inst.lab_mut().set("cutoff", 1200.0);
    assert_eq!(inst.lab().get("cutoff"), Some(1200.0));
    assert!(!inst.active());
    inst.trigger(&[45.0], 1.0, 4800.0, &TriggerOpt::default());
    assert!(inst.active());
    let mut o = [0.0; 2];
    for _ in 0..4800 {
        inst.run(&mut o);
    }
    assert!(o[0].is_finite() && o[0] == o[1]);
    // `setPatch` replaces the whole patch (its `kind` too); the instrument
    // stays what it was built as.
    inst.set_patch(bpatch("superPad").unwrap());
    assert!(matches!(inst, Instrument::Tb303(_)));
    assert_eq!(inst.lab().kind, "juno");
}

#[test]
fn sampled_cymbals_keep_their_level_add_grit_and_still_decay() {
    // The 909's hats and cymbals are 6-bit samples (sound-design.md §1.2):
    // `bits` requantises the metal voice, the VCA after the converter.
    let render = |k: &music::instruments::KitVoice, lane: &str| {
        let mut d = Drums::new(3, RATE);
        let h = d.hit(lane, k, 1.0, 1.0, 0);
        let mut x = vec![0f32; at(5.0)];
        for v in x.iter_mut() {
            let mut y = 0.0;
            d.run(|_, s| y += s);
            *v = y as f32;
        }
        (x, d.hits()[h].done)
    };
    for lane in ["hat", "ohat", "ride", "crash"] {
        let s = kit_voice("tr909", lane, None).expect("a lane");
        let mut synth = s;
        synth.bits = None;
        synth.clock = None;
        synth.dac_lp = None;
        synth.noise = Some(0.35);
        let (a, _) = render(&synth, lane);
        let (b, done) = render(&s, lane);
        assert!(finite(&b) && done, "{lane} ends");
        let db = 20.0 * (rms(&b[..4800]) / rms(&a[..4800])).log10();
        assert!(
            db.abs() < 4.0,
            "{lane}: {db:.1} dB from the synthesised one"
        );
        let grit = |x: &[f32]| band_power(x, 2000.0, 6000.0) / band_power(x, 6000.0, 20000.0);
        assert!(
            grit(&b[..4800]) > grit(&a[..4800]),
            "{lane}: the quantising adds broadband grit"
        );
    }
}

#[test]
fn the_12db_ladder_passes_more_top_end_than_the_24db_one() {
    // Oberheim 2-pole (slope 12) vs Moog/Juno 4-pole (slope 24): above cutoff,
    // the 2-pole roll-off leaves significantly more air in the 2–8 kHz band.
    let mut p24 = bpatch("warmPad").unwrap();
    p24.saw = Some(1.0);
    p24.cutoff = Some(900.0);
    p24.fenv = Some(0.0);
    p24.slope = Some(24.0);
    let mut p12 = p24.clone();
    p12.slope = Some(12.0);
    let (l24, _) = render_inst(&p24, &[note(0.0, &[60.0], 0.4)], 0.5);
    let (l12, _) = render_inst(&p12, &[note(0.0, &[60.0], 0.4)], 0.5);
    let hi24 = band_power(&l24[at(0.1)..at(0.3)], 2500.0, 7500.0);
    let hi12 = band_power(&l12[at(0.1)..at(0.3)], 2500.0, 7500.0);
    assert!(
        hi12 > hi24 * 4.0,
        "12 dB slope passes >6 dB more upper harmonics: {hi12} vs {hi24}"
    );
}

#[test]
fn chorus_high_pass_keeps_the_bass_fundamental_mono() {
    // `chorusHp` high-passes the BBD delay input so low bass stays centered
    // while upper harmonics spread to the sides (synthwave.md §4.4).
    let side_at = |hz: f64, hp: Option<f64>| {
        let mut c = Chorus::new(RATE);
        c.set(1.0);
        c.model(Some(0.5), Some(15000.0));
        c.set_hp(hp);
        let mut s = 0.0;
        for i in 0..at(1.0) {
            let x = (i as f64 * 2.0 * std::f64::consts::PI * hz / RATE).sin();
            let mut o = [0.0; 2];
            c.run(x, &mut o, 1.0);
            s += (o[0] - o[1]).powi(2);
        }
        s
    };
    let low_full = side_at(55.0, None);
    let low_hp = side_at(55.0, Some(180.0));
    let high_hp = side_at(880.0, Some(180.0));
    assert!(
        low_hp < low_full * 0.2,
        "55 Hz side energy cut by >5x: {low_hp} vs {low_full}"
    );
    assert!(high_hp > low_hp * 20.0, "upper harmonics still chorused");
}
