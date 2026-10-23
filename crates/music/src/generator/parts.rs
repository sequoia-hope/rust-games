//! The pattern writers: drums, fills and rolls, bass, chords, arpeggio,
//! drone and lead. Each writes [`Pattern`]s; the planner places them.
//!
//! Every choice is drawn from the [`Dice`] it is handed, among the options
//! a rule allows (PLAN.md: the RNG chooses inside the structure).

use super::profile::{BassStyle, ChordStyle, DrumLane, Profile};
use super::surprise::{Calib, Ppm, Score, lhl, profile as ic_profile};
use super::theory::{Chord, Mode, bass_note, scale_notes, voice};
use crate::dsp::Rng;
use crate::score::{Note, Pattern};

/// A seeded stream of choices; [`Dice::fork`] gives an independent stream
/// per decision, so changing one choice does not reshuffle the rest.
#[derive(Clone, Debug)]
pub struct Dice {
    seed: u32,
    r: Rng,
}

impl Dice {
    pub fn new(seed: u32) -> Dice {
        Dice {
            seed,
            r: Rng::new(if seed == 0 { 1 } else { seed }),
        }
    }

    /// An independent stream named `name` (FNV-1a of the name, mixed with
    /// the seed).
    pub fn fork(&self, name: &str) -> Dice {
        let mut h: u32 = 0x811C_9DC5;
        for b in name.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
        Dice::new(self.seed ^ h.wrapping_mul(0x9E37_79B9).rotate_left(7))
    }

    /// 0..1.
    pub fn f(&mut self) -> f64 {
        self.r.next()
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f() < p
    }

    /// `lo..hi` (`hi` excluded).
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + ((self.f() * (hi - lo) as f64) as i32).min(hi - lo - 1)
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }

    /// An index by weight.
    pub fn weighted(&mut self, ws: &[f64]) -> usize {
        let total: f64 = ws.iter().sum();
        let mut u = self.f() * total;
        for (i, w) in ws.iter().enumerate() {
            u -= w;
            if u < 0.0 {
                return i;
            }
        }
        ws.len() - 1
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.int(0, xs.len() as i32) as usize]
    }
}

// ── Drums ────────────────────────────────────────────────────────────

/// The core 2-bar loop of one drum layer (role `K`, `H`, `C` or `P`) from
/// the lanes the song uses.
pub fn drum_loop(name: &str, lanes: &[DrumLane], d: &mut Dice) -> Pattern {
    let mut notes = Vec::new();
    for bar in 0..2 {
        for l in lanes {
            let steps = if bar == 1 {
                l.steps2.unwrap_or(l.steps)
            } else {
                l.steps
            };
            for &s in steps {
                let st = (s - 1) as f64;
                let even = (s - 1) % 2 == 1;
                let base = if even {
                    l.vel_even.unwrap_or(l.vel)
                } else {
                    l.vel
                };
                let vel = (base * (1.0 + l.jitter * (d.f() * 2.0 - 1.0))).clamp(0.05, 1.0);
                notes.push(Note::hit(bar as f64 * 16.0 + st, l.lane, vel));
            }
        }
    }
    notes.sort_by(|a, b| a.at.total_cmp(&b.at));
    Pattern {
        name: name.to_owned(),
        len: 32.0,
        notes,
    }
}

/// A variation of a drum loop (RULES.md "surprise targets": hat and
/// percussion mutation): a few hits moved to a neighbouring 16th or
/// dropped, one ghost added; the downbeats stay.
pub fn drum_var(p: &Pattern, d: &mut Dice) -> Pattern {
    let mut notes: Vec<Note> = Vec::new();
    for n in &p.notes {
        let strong = (n.at as i64) % 4 == 0;
        if !strong && d.chance(0.12) {
            continue;
        }
        let mut n = n.clone();
        if !strong && d.chance(0.1) {
            n.at = (n.at + if d.chance(0.5) { 1.0 } else { -1.0 }).rem_euclid(32.0);
        }
        notes.push(n);
    }
    if let Some(src) = p.notes.first() {
        let at = d.int(0, 16) as f64 * 2.0 + 1.0;
        let mut g = src.clone();
        g.at = at;
        g.vel = (g.vel * 0.45).max(0.15);
        notes.push(g);
    }
    notes.sort_by(|a, b| a.at.total_cmp(&b.at));
    Pattern {
        name: format!("{} var", p.name),
        len: p.len,
        notes,
    }
}

/// Half the hits of a loop: the strong ones (`s`, sparse), or every second
/// note when the pattern is wholly syncopated off the half-bar grid.
pub fn sparse(p: &Pattern) -> Pattern {
    let mut notes: Vec<Note> = p
        .notes
        .iter()
        .filter(|n| (n.at as i64) % 8 == 0)
        .cloned()
        .collect();
    let min_notes = (p.len / 16.0).round().max(1.0) as usize;
    if notes.len() < min_notes {
        notes = p
            .notes
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, n)| n.clone())
            .collect();
    }
    Pattern {
        name: format!("{} sparse", p.name),
        len: p.len,
        notes,
    }
}

/// A tired, sparse statement of a melodic hook (for the `decline` movement
/// of an orchestral score): keeps the phrase's downbeat and half-bar anchor
/// notes, stretches their duration so each note lingers and breathes across
/// rests, and softens velocities.
pub fn sparse_melody(p: &Pattern) -> Pattern {
    let mut notes: Vec<Note> = p
        .notes
        .iter()
        .enumerate()
        .filter(|(i, n)| (n.at as i64) % 8 == 0 || *i == 0 || *i + 1 == p.notes.len())
        .map(|(_, n)| {
            let mut m = n.clone();
            m.vel = (m.vel * 0.72).clamp(0.35, 0.75);
            m.slide = false;
            m
        })
        .collect();
    for i in 0..notes.len() {
        let next_at = notes.get(i + 1).map_or(p.len, |n| n.at);
        let gap = (next_at - notes[i].at).max(2.0);
        notes[i].len = (gap * 0.65).clamp(3.0, 12.0);
    }
    Pattern {
        name: format!("{} sparse", p.name),
        len: p.len,
        notes,
    }
}

/// The loop's second bar with a fill on top, a 1-bar pattern for the last
/// bar of a phrase. Synthwave: snare 16ths on 13–16, or a tom run high to
/// low on 9–16 [synthwave#2][synthwave#12]; house: the turnaround kick on
/// 15 [house#5][house#6] and an extra snare at the end [arrangement#15].
pub fn fill(p: &Pattern, role: &str, toms: bool, d: &mut Dice) -> Pattern {
    let mut notes: Vec<Note> = p
        .notes
        .iter()
        .filter(|n| n.at >= 16.0)
        .map(|n| {
            let mut n = n.clone();
            n.at -= 16.0;
            n
        })
        .collect();
    match (role, toms) {
        ("C", true) => {
            notes.retain(|n| n.at < 8.0);
            if d.chance(0.45) {
                for (i, s) in (12..16).enumerate() {
                    notes.push(Note::hit(s as f64, "snare", 0.55 + 0.12 * i as f64));
                }
            } else {
                for (s, lane, v) in [
                    (8, "tomH", 0.85),
                    (9, "tomH", 0.65),
                    (10, "tomM", 0.85),
                    (12, "tomM", 0.8),
                    (13, "tomL", 0.7),
                    (14, "tomL", 0.9),
                ] {
                    notes.push(Note::hit(s as f64, lane, v));
                }
            }
        }
        ("K", false) => notes.push(Note::hit(14.0, "kick", 0.65)),
        ("C", false) => notes.push(Note::hit(15.0, "snare", 0.6)),
        _ => {}
    }
    notes.sort_by(|a, b| a.at.total_cmp(&b.at));
    Pattern {
        name: format!("{} fill", p.name),
        len: 16.0,
        notes,
    }
}

/// A build's snare roll over `bars` bars (RULES.md G4): quarters, then
/// 8ths, 16ths, and 32nds in the last bar, the velocity ramping up and the
/// first of each four accented; a quarter of builds stop early and leave
/// the last two beats silent [arrangement#11][arrangement#12].
pub fn roll(bars: u32, d: &mut Dice) -> Pattern {
    let total = bars as f64 * 16.0;
    // Stage lengths in bars (the house template: 4 / 2 / 1 / 1 of 8).
    let b = bars as f64;
    let stages = [
        (b * 0.5, 4.0),
        (b * 0.25, 2.0),
        (b * 0.125, 1.0),
        (b * 0.125, 0.5),
    ];
    let early = d.chance(0.25);
    let mut notes = Vec::new();
    let mut at = 0.0;
    for (len, step) in stages {
        let end = at + len * 16.0;
        let mut t = at;
        let mut k = 0;
        while t < end - 1e-9 {
            if !(early && t >= total - 8.0) {
                let u = t / total;
                let mut n = Note::hit(
                    t,
                    "snare",
                    (0.15 + 0.85 * u * u).min(1.0) * if k % 4 == 0 { 1.0 } else { 0.85 },
                );
                n.accent = k % 4 == 0;
                notes.push(n);
            }
            t += step;
            k += 1;
        }
        at = end;
    }
    Pattern {
        name: "roll".into(),
        len: total,
        notes,
    }
}

/// One hit (a crash on a section's downbeat [house#9][arrangement#12]).
pub fn one_hit(lane: &str) -> Pattern {
    Pattern {
        name: lane.to_owned(),
        len: 1024.0,
        notes: vec![Note::hit(0.0, lane, 0.9)],
    }
}

// ── Pitched parts over a progression (one chord a bar) ───────────────

/// The bass over a progression (a pattern as long as the progression).
pub fn bass(style: BassStyle, prog: &[Chord], tonic: i32, lo: i32, d: &mut Dice) -> Pattern {
    let mut notes = Vec::new();
    let n = prog.len();
    // House cells: one rhythm a song, scored by LHL (2–7 a bar
    // [cognition#20]), so the bass syncopates moderately.
    let cells: [&[(u8, i32)]; 5] = [
        // (1-based step, interval: 0 root, 12 octave, -1 next chord's root)
        &[(3, 0), (7, 0), (11, 0), (15, 0)],
        &[(1, 0), (4, 12), (7, 0), (9, 0), (12, 12), (15, 0)],
        &[(1, 0), (7, 0), (11, 0), (15, -1)],
        &[(1, 0), (4, 0), (7, 0), (11, 12), (14, 0)],
        &[(3, 0), (6, 12), (11, 0), (16, -1)],
    ];
    let cell = {
        let ok: Vec<usize> = (0..cells.len())
            .filter(|i| {
                let mut on = [false; 16];
                for (s, _) in cells[*i] {
                    on[(*s - 1) as usize] = true;
                }
                (2..=7).contains(&lhl(&on))
            })
            .collect();
        ok[d.int(0, ok.len() as i32) as usize]
    };
    let roll_figure = [0, 0, 0, 0, 7, -2];
    let octave_jumps = d.chance(0.4);
    for (bar, c) in prog.iter().enumerate() {
        let root = bass_note(c, tonic, lo) as f64;
        let next = bass_note(&prog[(bar + 1) % n], tonic, lo) as f64;
        let b0 = bar as f64 * 16.0;
        match style {
            BassStyle::Eighths => {
                for k in 0..8 {
                    let up = octave_jumps && k % 2 == 1;
                    notes.push(Note::pitched(
                        b0 + k as f64 * 2.0,
                        1.4,
                        vec![root + if up { 12.0 } else { 0.0 }],
                        if k % 2 == 0 { 0.9 } else { 0.75 },
                    ));
                }
            }
            BassStyle::HouseCells => {
                for (s, iv) in cells[cell] {
                    let m = if *iv < 0 { next } else { root + *iv as f64 };
                    notes.push(Note::pitched(b0 + (*s - 1) as f64, 1.8, vec![m], 0.85));
                }
            }
            BassStyle::Rolling => {
                let third = if c.minor { 3.0 } else { 4.0 };
                for (k, s) in [0, 3, 6, 8, 11, 14].iter().enumerate() {
                    let iv = if k == 2 { third } else { roll_figure[k] as f64 };
                    notes.push(Note::pitched(
                        b0 + *s as f64,
                        1.6,
                        vec![root + iv],
                        if k == 0 { 0.92 } else { 0.72 },
                    ));
                }
            }
            BassStyle::Orchestral => {
                // Symphonic contrabass & tuba: resonant half-note root on
                // beat 1, fifth (or root octave) on beat 3, and a martial
                // pickup on step 15 every second bar for the walker's stride.
                let fifth = if root + 7.0 <= (lo + 16) as f64 {
                    root + 7.0
                } else {
                    root - 5.0
                };
                notes.push(Note::pitched(b0, 7.2, vec![root], 0.90));
                notes.push(Note::pitched(
                    b0 + 8.0,
                    if bar % 2 == 1 { 5.5 } else { 7.2 },
                    vec![if bar % 2 == 0 { fifth } else { root }],
                    0.80,
                ));
                if bar % 2 == 1 {
                    notes.push(Note::pitched(b0 + 14.0, 1.8, vec![next], 0.76));
                }
            }
        }
    }
    Pattern {
        name: "bass".into(),
        len: n as f64 * 16.0,
        notes,
    }
}

/// The chords voiced with least motion (RULES.md G9.7: smooth voicings).
pub fn voicings(prog: &[Chord], tonic: i32, n: usize, lo: i32, hi: i32) -> Vec<Vec<i32>> {
    let mut prev: Option<Vec<i32>> = None;
    prog.iter()
        .map(|c| {
            let v = voice(c, tonic, n, lo, hi, prev.as_deref());
            prev = Some(v.clone());
            v
        })
        .collect()
}

/// Chords: held a bar each, or stabs on a syncopated rhythm of off-16ths
/// chosen once a song ([sound-design#27]).
pub fn chords(style: ChordStyle, voiced: &[Vec<i32>], d: &mut Dice) -> Pattern {
    let rhythms: [(&[u8], &[u8]); 4] = [
        (&[4, 11], &[4, 14]),
        (&[3, 12], &[3, 8, 15]),
        (&[1, 7, 14], &[4, 12]),
        (&[3, 8, 14], &[3, 11]),
    ];
    let (r1, r2) = *d.pick(&rhythms);
    let mut notes = Vec::new();
    for (bar, v) in voiced.iter().enumerate() {
        let b0 = bar as f64 * 16.0;
        let midis: Vec<f64> = v.iter().map(|m| *m as f64).collect();
        match style {
            ChordStyle::Sustain => notes.push(Note::pitched(b0, 15.5, midis, 0.7)),
            ChordStyle::Stabs => {
                let rhythm = if bar % 2 == 0 { r1 } else { r2 };
                for (i, s) in rhythm.iter().enumerate() {
                    notes.push(Note::pitched(
                        b0 + (*s - 1) as f64,
                        2.0,
                        midis.clone(),
                        if i == 0 { 0.78 } else { 0.68 },
                    ));
                }
            }
        }
    }
    Pattern {
        name: "chords".into(),
        len: voiced.len() as f64 * 16.0,
        notes,
    }
}

/// The arpeggio: eight 8ths a bar spelling the chord ("Nightcall"), with
/// downbeat or off-beat emphasis ("A Real Hero") [synthwave#38][synthwave#39].
pub fn arp(voiced: &[Vec<i32>], d: &mut Dice) -> Pattern {
    let offbeat = d.chance(0.3);
    let shape: &[usize] = d.pick(&[
        &[0, 1, 2, 3, 4, 3, 2, 1][..],
        &[0, 2, 1, 3, 2, 4, 3, 1][..],
        &[0, 1, 2, 4, 0, 1, 2, 4][..],
    ]);
    let mut notes = Vec::new();
    for (bar, v) in voiced.iter().enumerate() {
        let mut tones: Vec<i32> = v.clone();
        let mut oct = 0;
        while tones.len() < 5 {
            let src = v[oct % v.len()] + 12 * (oct / v.len() + 1) as i32;
            tones.push(src);
            oct += 1;
        }
        let b0 = bar as f64 * 16.0;
        for k in 0..8 {
            let m = tones[shape[k] % tones.len()];
            let vel = if (k % 2 == 1) == offbeat { 0.82 } else { 0.68 };
            notes.push(Note::pitched(b0 + k as f64 * 2.0, 1.6, vec![m as f64], vel));
        }
    }
    Pattern {
        name: "arp".into(),
        len: voiced.len() as f64 * 16.0,
        notes,
    }
}

/// An atmospheric bed: each bar's chord root and fifth held softly across
/// the bar.
pub fn drone(voiced: &[Vec<i32>]) -> Pattern {
    let mut notes: Vec<Note> = Vec::new();
    for (bar, v) in voiced.iter().enumerate() {
        let r = v.first().copied().unwrap_or(60) as f64;
        notes.push(Note::pitched(
            bar as f64 * 16.0,
            15.8,
            vec![r, r + 7.0],
            0.55,
        ));
    }
    Pattern {
        name: "drone".into(),
        len: voiced.len() as f64 * 16.0,
        notes,
    }
}

// ── Melody: candidates scored for surprise (RULES.md G9) ─────────────

/// Symbols of the melody model: the interval in scale steps, −7..7.
const IV: usize = 15;

fn iv_sym(steps: i32) -> u32 {
    (steps.clamp(-7, 7) + 7) as u32
}

/// Where a melodic line lives: its mode and key, its lowest note and range,
/// and its density in notes a bar.
#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    pub mode: &'a Mode,
    pub tonic: i32,
    pub lo: i32,
    pub range: i32,
    pub density: f64,
}

/// One draft of a phrase over `prog` (one chord a bar), built as hooks
/// are: a one-bar motif (onsets on the 8th grid at the line's density,
/// intervals stepping through the scale) restated over each chord, its
/// strong beats snapped to the chord's tones, departing from it now and
/// then (a bar re-drawn with probability `0.6 · tau`), the step weights
/// raised to `1/tau`; `close` ends it on the last chord's root, else on its
/// 3rd or 5th (question and answer). Repetition is what makes a hook
/// predictable to the short-term model (G9.8, Margulis). Returns the notes
/// and the interval symbols (the first note has no interval).
fn draft(
    prog: &[Chord],
    line: &Line,
    close: bool,
    tau: f64,
    d: &mut Dice,
) -> (Vec<Note>, Vec<u32>) {
    let scale = scale_notes(line.mode, line.tonic, line.lo, line.lo + line.range);
    let base = [0.05, 0.12, 0.25, 0.1, 0.25, 0.12, 0.05, 0.06];
    let ws: Vec<f64> = base.iter().map(|w: &f64| w.powf(1.0 / tau)).collect();
    // A bar's figure: (8th, step) pairs.
    let figure = |d: &mut Dice, last: bool| -> Vec<(usize, i32)> {
        let mut f = Vec::new();
        for e in 0..8 {
            let strong = e % 4 == 0;
            let p = line.density / 8.0
                * if strong {
                    1.6
                } else if e % 2 == 0 {
                    1.0
                } else {
                    0.6
                };
            if (d.chance(p) || e == 0) && !(last && e > 4) {
                f.push((e, d.weighted(&ws) as i32 - 3));
            }
        }
        f
    };
    let motif = figure(d, false);
    let mut notes: Vec<Note> = Vec::new();
    let mut ds: Vec<i32> = Vec::new();
    let mut cur = scale.len() as i32 / 2;
    for (bar, c) in prog.iter().enumerate() {
        let pcs = c.pcs();
        let last = bar + 1 == prog.len();
        let fig = if bar == 0 || (!last && !d.chance(0.6 * tau)) {
            motif
                .iter()
                .copied()
                .filter(|(e, _)| !(last && *e > 4))
                .collect()
        } else {
            figure(d, last)
        };
        for (e, step) in fig {
            let strong = e % 4 == 0;
            let n_sc = scale.len() as i32;
            let raw = cur + step;
            let mut next = if raw < 0 || raw >= n_sc {
                (cur - step).clamp(0, n_sc - 1)
            } else {
                raw
            };
            if strong {
                // Snap to the nearest chord tone.
                let mut bestd = i32::MAX;
                for (i, m) in scale.iter().enumerate() {
                    let rel = (m - line.tonic).rem_euclid(12);
                    if pcs.contains(&rel) && (i as i32 - next).abs() < bestd {
                        bestd = (i as i32 - next).abs();
                        next = i as i32;
                    }
                }
            }
            if !notes.is_empty() {
                ds.push(next - cur);
            }
            cur = next;
            notes.push(Note::pitched(
                bar as f64 * 16.0 + e as f64 * 2.0,
                2.0,
                vec![scale[cur as usize] as f64],
                if strong { 0.85 } else { 0.7 },
            ));
        }
    }
    // The ending: the root (closed) or the 3rd or 5th (open).
    let c = &prog[prog.len() - 1];
    let want: Vec<i32> = if close {
        vec![0]
    } else {
        vec![if c.minor { 3 } else { 4 }, 7]
    };
    let target = scale
        .iter()
        .enumerate()
        .filter(|(_, m)| want.contains(&((**m - line.tonic - c.root).rem_euclid(12))))
        .min_by_key(|(i, _)| (*i as i32 - cur).abs())
        .map(|(i, _)| i as i32)
        .unwrap_or(cur);
    ds.push(target - cur);
    notes.push(Note::pitched(
        (prog.len() - 1) as f64 * 16.0 + 8.0,
        6.0,
        vec![scale[target as usize] as f64],
        0.8,
    ));
    // Legato: consecutive notes within a beat overlap slightly and glide;
    // repeated notes or notes before a rest articulate cleanly.
    for i in 0..notes.len() {
        let next_at = notes.get(i + 1).map_or(prog.len() as f64 * 16.0, |n| n.at);
        let gap = next_at - notes[i].at;
        let step_dist = notes
            .get(i + 1)
            .and_then(|n| n.midis.first().zip(notes[i].midis.first()))
            .map_or(0.0, |(a, b)| (a - b).abs());
        if gap <= 4.0 && (0.5..=7.5).contains(&step_dist) {
            notes[i].len = (gap + 0.15).clamp(0.8, 8.0);
            if let Some(n1) = notes.get_mut(i + 1) {
                n1.slide = true;
            }
        } else {
            notes[i].len = (gap - 0.25).clamp(0.8, 8.0);
        }
    }
    (notes, ds.iter().map(|x| iv_sym(*x)).collect())
}

/// The long-term melody model of a style (G9.3: "trained on the profile's
/// own grammar"): drafts of the line over the profile's progressions, so a
/// typical draft scores near z = 0 and the targets pick the more singable
/// ones; with its calibration.
pub fn melody_ltm(pool: &[(Vec<Chord>, f64)], line: &Line, seed: u32) -> (Ppm, Calib) {
    let mut d = Dice::new(seed ^ 0x51ED);
    let ws: Vec<f64> = pool.iter().map(|p| p.1).collect();
    let drafts = |n: usize, d: &mut Dice| -> Vec<Vec<u32>> {
        (0..n)
            .map(|i| draft(&pool[d.weighted(&ws)].0, line, i % 2 == 0, 1.0, d).1)
            .collect()
    };
    let corpus = drafts(60, &mut d);
    let mut m = Ppm::new(3, IV);
    for s in &corpus {
        m.train(s);
    }
    // Calibrated on drafts it has not learned, as a listener meets a new
    // song of a style they know.
    let held_out = drafts(40, &mut d);
    let c = Calib::of(&m, &held_out);
    (m, c)
}

/// A phrase: of `k` drafts, from stepwise to leapy (τ spread over 0.4..1.6,
/// G9.10's temperature), the one with the best fitness is kept: mean z(IC)
/// in −1..0, mean z(H) in −0.75..−0.25, Cheung's placement, rejecting
/// below −1.5 or above +1 (G9.7).
#[allow(clippy::too_many_arguments)]
pub fn phrase(
    prog: &[Chord],
    line: &Line,
    close: bool,
    ltm: &Ppm,
    calib: &Calib,
    stm: &Ppm,
    k: usize,
    d: &mut Dice,
) -> (Pattern, Score, Vec<u32>) {
    let mut best: Option<(f64, Pattern, Score, Vec<u32>)> = None;
    for ki in 0..k {
        let tau = 0.4 + 1.2 * (ki as f64 + 0.5) / k as f64;
        let (notes, syms) = draft(prog, line, close, tau, d);
        let z: Vec<(f64, f64)> = ic_profile(ltm, stm, &syms, &[])
            .into_iter()
            .map(|x| calib.z(x))
            .collect();
        let score = Score::of(&z);
        let fit = score.fitness((-1.0, 0.0), (-0.75, -0.25), (-1.5, 1.0));
        if best.as_ref().is_none_or(|b| fit > b.0) {
            best = Some((
                fit,
                Pattern {
                    name: if close {
                        "answer".into()
                    } else {
                        "call".into()
                    },
                    len: prog.len() as f64 * 16.0,
                    notes,
                },
                score,
                syms,
            ));
        }
    }
    let (_, p, s, syms) = best.expect("k > 0");
    (p, s, syms)
}

/// Picks a progression from a pool: every candidate is scored by the chord
/// model (its pool's own statistics, G9.4) for the song so far (`stm`);
/// those within one unit of the best fitness are kept, and the pool's
/// weights choose among them.
pub fn choose_progression(pool: &[(Vec<Chord>, f64)], stm: &Ppm, d: &mut Dice) -> usize {
    let mut ltm = Ppm::new(3, 36);
    let corpus: Vec<Vec<u32>> = pool
        .iter()
        .flat_map(|(p, w)| {
            let seq: Vec<u32> = p.iter().chain(p.iter()).map(Chord::symbol).collect();
            std::iter::repeat_n(seq, (w * 20.0).round().max(1.0) as usize)
        })
        .collect();
    for s in &corpus {
        ltm.train(s);
    }
    let calib = Calib::of(&ltm, &corpus);
    let fits: Vec<f64> = pool
        .iter()
        .map(|(p, _)| {
            let seq: Vec<u32> = p.iter().chain(p.iter()).map(Chord::symbol).collect();
            let z: Vec<(f64, f64)> = ic_profile(&ltm, stm, &seq, &[])
                .into_iter()
                .map(|x| calib.z(x))
                .collect();
            Score::of(&z).fitness(
                (-1.0, 0.0),
                (-0.75, -0.25),
                (f64::NEG_INFINITY, f64::INFINITY),
            )
        })
        .collect();
    let best = fits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let ws: Vec<f64> = pool
        .iter()
        .zip(&fits)
        .map(|((_, w), f)| if *f >= best - 1.0 { *w } else { 0.0 })
        .collect();
    d.weighted(&ws)
}

/// The profile's progression pool for a mode, with extensions drawn per
/// chord (7th, 9th, 11th by the profile's probabilities).
pub fn pool(p: &Profile, mode: &Mode, d: &mut Dice) -> Vec<(Vec<Chord>, f64)> {
    let src = if mode.minor {
        p.minor_progs
    } else {
        p.major_progs
    };
    src.iter()
        .filter_map(|(s, w)| {
            let mut prog = super::theory::progression(s, mode)?;
            let mut seen: Vec<(String, Vec<i32>)> = Vec::new();
            for c in &mut prog {
                if let Some((_, prev_tones)) = seen.iter().find(|(n, _)| *n == c.name) {
                    c.tones = prev_tones.clone();
                    continue;
                }
                let has7 = c.tones.iter().any(|t| *t == 10 || *t == 11);
                let diatonic7 = if c.minor {
                    10
                } else if mode.scale.contains(&((c.root + 11).rem_euclid(12))) {
                    11
                } else {
                    10
                };
                if !has7 && d.chance(p.ext.0) {
                    c.tones.push(diatonic7);
                }
                if d.chance(p.ext.1) && !c.tones.contains(&14) {
                    c.tones.push(14);
                } else if d.chance(p.ext.2) && c.minor && !c.tones.contains(&17) {
                    c.tones.push(17);
                }
                c.tones.sort();
                seen.push((c.name.clone(), c.tones.clone()));
            }
            Some((prog, *w))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::theory::{AEOLIAN, progression};

    #[test]
    fn a_roll_speeds_up_and_rises() {
        let mut d = Dice::new(3);
        let r = roll(8, &mut d);
        let gaps: Vec<f64> = r.notes.windows(2).map(|w| w[1].at - w[0].at).collect();
        assert_eq!(gaps[0], 4.0, "quarters first");
        assert!(
            gaps.contains(&0.5) || r.notes.last().unwrap().at < 120.0,
            "32nds at the end, unless it stopped early"
        );
        assert!(r.notes.last().unwrap().vel > r.notes[0].vel * 3.0);
    }

    #[test]
    fn melodies_land_in_the_target_band() {
        let prog = progression("i–VI–III–VII", &AEOLIAN).unwrap();
        let line = Line {
            mode: &AEOLIAN,
            tonic: 57,
            lo: 69,
            range: 14,
            density: 3.0,
        };
        let (ltm, calib) = melody_ltm(&[(prog.clone(), 1.0)], &line, 7);
        let stm = Ppm::new(3, IV);
        let mut d = Dice::new(9);
        let (p, s, _) = phrase(&prog, &line, true, &ltm, &calib, &stm, 48, &mut d);
        assert!(p.notes.len() >= 6, "{} notes", p.notes.len());
        assert!(s.ic >= -1.0 && s.ic <= 0.0, "in the band: {s:?}");
        let last = p.notes.last().unwrap().midis[0] as i32;
        assert_eq!(
            (last - 57 - 10).rem_euclid(12),
            0,
            "closes on the last chord's root"
        );
    }

    #[test]
    fn forks_are_independent() {
        let d = Dice::new(42);
        let mut a = d.fork("bass");
        let mut b = d.fork("drums");
        assert_ne!(a.f(), b.f());
        assert_eq!(Dice::new(42).fork("bass").f(), d.fork("bass").f());
    }
}
