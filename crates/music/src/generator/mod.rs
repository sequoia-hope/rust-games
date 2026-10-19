//! The new generator (PLAN.md Phase 3): genre profiles from
//! research/RULES.md, an arrangement planner that gives every instrument
//! its own timeline, and candidates scored for surprise. It writes a
//! [`crate::score::Song`], which [`crate::song::SongPlayer`] plays.
//!
//! [`generate`] goes: tempo, swing, mode and key from the profile; a verse
//! and a chorus progression chosen by the chord model; one layer per role
//! of the profile's arrangement table, each cell of the table becoming
//! clips on that layer's own timeline (drum loops with a fill in the last
//! bar of a phrase, the bass and chords over the block's progression, the
//! hook stated exactly wherever it plays, a roll and a riser into the
//! drops, a crash on the downbeats after); filtered cells as filter
//! automation; the energy curve as the sections' energies and each layer's
//! energy threshold (G5).

pub mod parts;
pub mod profile;
pub mod surprise;
pub mod theory;

use crate::patches::bpatch;
use crate::score::{
    Automation, Clip, Curve, Fx, Gated, Layer, Mark, Pattern, Role, Section, Song, Sound,
};
use crate::track::Ch;
use parts::Dice;
use profile::{PROFILES, Profile};
use surprise::Ppm;
use theory::{Chord, NOTE_NAMES};

/// The new generator's genres: `(key, name)`, in the lab's order.
pub fn genres() -> Vec<(&'static str, &'static str)> {
    PROFILES.iter().map(|p| (p.key, p.name)).collect()
}

/// What the player can set beside the seed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settings {
    /// A tempo inside the profile's range (else drawn).
    pub bpm: Option<f64>,
    /// The tonic's pitch class, 0 = C (else drawn).
    pub key: Option<i32>,
}

/// One cell of an arrangement table (RULES.md's codes).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cell {
    Off,
    /// `x`, `+` (from the block's start or from bar `N`), `V`.
    On(Mark, Option<u32>),
    /// `r` or `rA-B`: a build over the block or over bars A..B.
    Build(Option<(u32, u32)>),
    /// `1` or `1N`: one hit at the block's start or at bar N.
    Hit(Option<u32>),
}

fn cell(s: &str) -> Cell {
    let num = |t: &str| t.parse::<u32>().ok().map(|n| n.saturating_sub(1));
    match s {
        "-" => Cell::Off,
        "x" | "V" | "+" => Cell::On(Mark::Core, None),
        "v" => Cell::On(Mark::Var, None),
        "f" => Cell::On(Mark::Filtered, None),
        "s" => Cell::On(Mark::Sparse, None),
        "r" => Cell::Build(None),
        "1" => Cell::Hit(None),
        _ if s.starts_with('+') => Cell::On(Mark::Core, num(&s[1..])),
        _ if s.starts_with('r') => {
            let (a, b) = s[1..].split_once('-').unwrap_or(("", ""));
            Cell::Build(num(a).zip(num(b).map(|b| b + 1)))
        }
        _ if s.starts_with('1') => Cell::Hit(num(&s[1..])),
        _ => Cell::Off,
    }
}

/// A role's layer, its energy threshold (G5: E 0.2 ≈ 1–3 layers, 0.5 ≈
/// 4–6, 0.9 ≈ 8–12; engine default) and whether it is pumped.
fn role_of(key: &str) -> Option<(Role, f64, bool)> {
    Some(match key {
        "K" => (Role::Kick, 0.0, false),
        "C" => (Role::Clap, 0.1, false),
        "H" => (Role::Hats, 0.2, false),
        "B" => (Role::Bass, 0.25, true),
        "Atm" => (Role::Atmos, 0.3, true),
        "Pd" => (Role::Pad, 0.35, true),
        "P" => (Role::Perc, 0.45, false),
        "Sn" => (Role::Clap, 0.5, false),
        "X" => (Role::Fx, 0.5, false),
        "A" => (Role::Arp, 0.55, true),
        "L" => (Role::Lead, 0.6, false),
        _ => return None,
    })
}

/// The level of a bed (pads, atmosphere) while it plays exposed in an
/// intro, before the kick: +12 dB (engine default).
const EXPOSED_LIFT: f64 = 4.0;

/// A song from a profile and a seed; `None` for an unknown genre.
pub fn generate(genre: &str, seed: u32, settings: &Settings) -> Option<Song> {
    let p = profile::profile(genre)?;
    Some(plan(p, seed, settings))
}

fn transpose(p: &Pattern, st: f64) -> Pattern {
    let mut q = p.clone();
    for n in &mut q.notes {
        for m in &mut n.midis {
            *m += st;
        }
    }
    q
}

fn plan(p: &Profile, seed: u32, settings: &Settings) -> Song {
    let dice = Dice::new(seed).fork(p.key);
    // Tempo near the typical (a triangle over the range), swing in range.
    let mut dt = dice.fork("tempo");
    let (lo, hi, typ) = p.tempo;
    let bpm = settings.bpm.unwrap_or_else(|| {
        let u = (dt.f() + dt.f()) / 2.0;
        if u < 0.5 {
            lo + (typ - lo) * u * 2.0
        } else {
            typ + (hi - typ) * (u - 0.5) * 2.0
        }
    });
    let bpm = bpm.clamp(lo, hi).round();
    let swing = dt.range(p.swing.0, p.swing.1).round();

    // Mode and key.
    let mut dk = dice.fork("key");
    let mode = p.modes[dk.weighted(&p.modes.iter().map(|m| m.1).collect::<Vec<_>>())].0;
    let pc = settings.key.unwrap_or_else(|| dk.int(0, 12)).rem_euclid(12);
    let (blo, _) = p.bass_range;
    let bass_tonic = blo + (pc - blo).rem_euclid(12);
    // The pitch reference for chords and lines: the bass tonic two octaves up.
    let tonic = bass_tonic + 24;

    // Progressions: the verse's, then the chorus's (the chord model's STM
    // has heard the verse by then).
    let mut dh = dice.fork("harmony");
    let pool = parts::pool(p, &mode, &mut dh);
    let mut chord_stm = Ppm::new(3, 36);
    let a = parts::choose_progression(&pool, &chord_stm, &mut dh);
    let seq_a: Vec<u32> = pool[a]
        .0
        .iter()
        .chain(&pool[a].0)
        .map(Chord::symbol)
        .collect();
    chord_stm.train(&seq_a);
    let b = if p.arp || dh.chance(0.4) {
        parts::choose_progression(&pool, &chord_stm, &mut dh)
    } else {
        a
    };
    let progs = [pool[a].0.clone(), pool[b].0.clone()];
    // The middle eight a minor third up, then back (p = 0.15
    // [synthwave#12]).
    let key_change = p.form.iter().any(|f| f.name == "middle eight") && dh.chance(0.15);

    let bars: u32 = p.form.iter().map(|b| b.bars).sum();
    let mut song = Song::new(p.key, seed, bpm, bars);
    song.swing = swing;
    let key_name = format!("{} {}", NOTE_NAMES[pc as usize], mode.name);
    let prog_name = |i: usize| {
        progs[i]
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
            .join(" ")
    };
    song.title = format!("{} {}", p.name, seed);
    song.style = format!("{} · {} · {}", p.name, key_name, prog_name(0));
    song.pump_release = (60.0 / bpm / 2.0 - 0.03).max(0.1);
    // Block starts.
    let mut starts = Vec::new();
    let mut at = 0;
    for b in p.form {
        starts.push(at);
        song.sections.push(Section {
            name: b.name.to_owned(),
            bar: at,
            bars: b.bars,
            energy: b.energy,
        });
        at += b.bars;
    }
    let chorus = |i: usize| matches!(p.form[i].name, "chorus" | "drop" | "main" | "solo");

    // The lead's hook: a call and an answer over the chorus progression
    // (G9.7, G9.8: stated exactly wherever it plays).
    let mut dl = dice.fork("lead");
    let line = parts::Line {
        mode: &mode,
        tonic,
        lo: tonic + 12 - 5,
        range: p.lead.1,
        density: p.lead.0,
    };
    let (mel_ltm, mel_calib) = parts::melody_ltm(&pool, &line, seed);
    let mut mel_stm = Ppm::new(3, 15);
    let has_lead = dl.chance(p.lead.2);
    let hook = |dl: &mut Dice, stm: &mut Ppm| {
        let (call, s1, syms1) =
            parts::phrase(&progs[1], &line, false, &mel_ltm, &mel_calib, stm, 48, dl);
        stm.train(&syms1);
        let (answer, s2, syms2) =
            parts::phrase(&progs[1], &line, true, &mel_ltm, &mel_calib, stm, 48, dl);
        stm.train(&syms2);
        let mut notes = call.notes;
        notes.extend(answer.notes.into_iter().map(|mut n| {
            n.at += 64.0;
            n
        }));
        (
            Pattern {
                name: "hook".into(),
                len: 128.0,
                notes,
            },
            (s1.ic + s2.ic) / 2.0,
            (s1.placement + s2.placement) / 2.0,
        )
    };
    let (hook_pat, hook_ic, hook_place) = hook(&mut dl, &mut mel_stm);
    let (solo_pat, _, _) = hook(&mut dl, &mut mel_stm);

    let mut notes: Vec<String> = p.notes.iter().map(|s| s.to_string()).collect();
    notes.push(format!(
        "{bpm} BPM, in the profile's range of {lo}–{hi} (typical {typ}); swing {swing} %."
    ));
    notes.push(format!(
        "{key_name}; verse {}, chorus {}: chosen from the profile's pool by the chord model, the chorus heard after the verse.",
        prog_name(0),
        prog_name(1)
    ));
    if has_lead {
        notes.push(format!(
            "The hook was the best of 48 candidates per phrase: mean surprise z = {hook_ic:.2} (target −1 to 0, Gold et al. 2019), placement {hook_place:.2} (Cheung et al. 2019: surprises where the listener is sure). It returns exactly (Margulis)."
        ));
    }
    if key_change {
        notes.push("The middle eight goes up a minor third and comes back, as Italo and synthwave middle eights do [synthwave#12].".into());
    }

    // How the song opens: one of the profile's intros, by weight.
    let intro = (!p.intros.is_empty()).then(|| {
        let mut di = dice.fork("intro");
        &p.intros[di.weighted(&p.intros.iter().map(|i| i.p).collect::<Vec<_>>())]
    });
    if let Some(i) = intro {
        notes.push(i.note.to_owned());
    }
    let row = |rk: &str, cells: &str| -> Vec<Cell> {
        let mut cells: Vec<Cell> = cells.split_whitespace().map(cell).collect();
        if let Some((_, over)) = intro.and_then(|i| i.cells.iter().find(|(k, _)| *k == rk)) {
            for (c, o) in cells.iter_mut().zip(over.split_whitespace()) {
                *c = cell(o);
            }
        }
        cells
    };

    // The bar the kick first plays.
    let kick_in = p
        .template
        .iter()
        .find(|(k, _)| *k == "K")
        .and_then(|(rk, cells)| {
            row(rk, cells)
                .iter()
                .position(|c| !matches!(c, Cell::Off))
                .map(|bi| starts[bi])
        })
        .unwrap_or(0);

    let mut dd = dice.fork("drums");
    let kit = |kit: &str| Sound::Drums {
        kit: kit.to_owned(),
        tweak: profile::kit_tweak(kit),
    };
    for (rk, cells) in p.template {
        let Some((role, min_energy, pumped)) = role_of(rk) else {
            continue;
        };
        let cells = row(rk, cells);
        let mut layer = match *rk {
            "K" | "H" | "C" | "P" | "Sn" | "X" => Layer::new(&rk.to_lowercase(), role, kit(p.kit)),
            _ => {
                let sounds = match *rk {
                    "B" => p.bass_sounds,
                    "Pd" | "Atm" => p.pad_sounds,
                    "A" => p.arp_sounds,
                    _ => p.lead_sounds,
                };
                let name = *dice.fork(&format!("sound {rk}")).pick(sounds);
                let Some(mut lab) = bpatch(name) else {
                    continue;
                };
                // The Juno chorus at its measured depth: ±0.5 ms (I),
                // ±1.1 ms (II), through 15 kHz (sound-design.md §6).
                match lab.chorus {
                    Some(1.0) => lab.chorus_ms = Some(0.5),
                    Some(2.0) => lab.chorus_ms = Some(1.1),
                    _ => {}
                }
                if lab.chorus.is_some_and(|c| c > 0.0) {
                    lab.chorus_lp = Some(15000.0);
                }
                Layer::new(
                    match *rk {
                        "B" => "bass",
                        "Pd" => "chords",
                        "A" => "arp",
                        "Atm" => "drone",
                        _ => "lead",
                    },
                    role,
                    Sound::Synth(Box::new(lab)),
                )
            }
        };
        layer.min_energy = min_energy;
        layer.pump = if pumped { p.pump } else { 0.0 };
        layer.ch = match *rk {
            "B" => Ch {
                level: Some(0.95),
                ..Ch::default()
            },
            "Pd" => Ch {
                level: Some(0.55),
                rev: Some(0.3),
                pan: Some(0.0),
                ..Ch::default()
            },
            "Atm" => Ch {
                level: Some(0.35),
                rev: Some(0.5),
                hp: Some(200.0),
                ..Ch::default()
            },
            "A" => Ch {
                level: Some(0.45),
                rev: Some(0.2),
                dly: Some(0.25),
                pan: Some(0.15),
                hp: Some(150.0),
                ..Ch::default()
            },
            "L" => Ch {
                level: Some(0.5),
                rev: Some(0.25),
                dly: Some(0.2),
                hp: Some(200.0),
                ..Ch::default()
            },
            _ => Ch::default(),
        };
        if let Some((_, db)) = p.levels.iter().find(|(k, _)| k == rk) {
            layer.ch.level = Some(layer.ch.level.unwrap_or(1.0) * 10f64.powf(db / 20.0));
        }
        if *rk == "K" {
            layer.keys_pump = true;
        }
        let gated = |send: f64, hold: f64| {
            (send > 0.0).then_some(Gated {
                send,
                hold,
                rt60: 2.0,
            })
        };
        match *rk {
            // A long hold on the snare, a short one on the kick: under the
            // next beat [synthwave#26].
            "C" => layer.gated = gated(p.gated.0, 0.45),
            "K" => layer.gated = gated(p.gated.1, 0.25),
            _ => {}
        }

        // The layer's patterns.
        let mut pat_of: Vec<(Mark, usize, usize)> = Vec::new();
        let mut core_fill: Option<(usize, usize, usize, usize)> = None;
        match *rk {
            "K" | "H" | "C" | "P" => {
                let lanes: Vec<_> = p
                    .drums
                    .iter()
                    .filter(|l| l.role == *rk && dd.chance(l.p))
                    .copied()
                    .collect();
                if lanes.is_empty() {
                    continue;
                }
                let core = parts::drum_loop(rk, &lanes, &mut dd);
                let var = parts::drum_var(&core, &mut dd);
                let sp = parts::sparse(&core);
                let fl = parts::fill(&core, rk, p.tom_fills, &mut dd);
                let ci = layer.pattern(core);
                let vi = layer.pattern(var);
                let si = layer.pattern(sp);
                let fi = layer.pattern(fl);
                core_fill = Some((ci, vi, si, fi));
            }
            "B" | "Pd" | "A" | "Atm" | "L" => {
                let mut dp = dice.fork(&format!("part {rk}"));
                for prog in 0..2 {
                    for tr in [0.0, 3.0] {
                        if tr != 0.0 && !key_change {
                            continue;
                        }
                        let pr = &progs[prog];
                        let voiced = parts::voicings(pr, tonic, 4, tonic - 3, tonic + 16);
                        let core = match *rk {
                            "B" => parts::bass(p.bass, pr, tonic, blo, &mut dp),
                            "Pd" => parts::chords(p.chords, &voiced, &mut dp),
                            "A" => parts::arp(&voiced, &mut dp),
                            "Atm" => parts::drone(tonic - 12, pr.len()),
                            _ => hook_pat.clone(),
                        };
                        let core = transpose(&core, tr);
                        let var = if *rk == "L" {
                            transpose(&solo_pat, tr)
                        } else {
                            core.clone()
                        };
                        let sp = parts::sparse(&core);
                        let ci = layer.pattern(core);
                        let vi = layer.pattern(var);
                        let si = layer.pattern(sp);
                        pat_of.push((Mark::Core, prog, ci));
                        pat_of.push((Mark::Var, prog, vi));
                        pat_of.push((Mark::Sparse, prog, si));
                    }
                }
            }
            "Sn" | "X" => {}
            _ => {}
        }
        if *rk == "L" && !has_lead {
            continue;
        }

        let mut lp = Automation::new("lp");
        let mut filtered = false;
        for (bi, c) in cells.iter().enumerate() {
            let Some(&b0) = starts.get(bi) else { break };
            let blen = p.form[bi].bars;
            let b1 = b0 + blen;
            match *c {
                Cell::Off => {}
                Cell::On(mark, from) => {
                    let from = from.unwrap_or(b0).clamp(b0, b1 - 1);
                    if let Some((ci, vi, si, fi)) = core_fill {
                        // Drums: phrases of 8 bars, the last bar a fill
                        // (synthwave every 4th or 8th bar on the snare;
                        // house a turnaround kick).
                        let pat = match mark {
                            Mark::Var => vi,
                            Mark::Sparse => si,
                            _ => ci,
                        };
                        let fills = (p.tom_fills && *rk == "C")
                            || (!p.tom_fills && (*rk == "K" || *rk == "C"));
                        let mut ph = from;
                        while ph < b1 {
                            let len = (b1 - ph).min(8);
                            if fills && len == 8 && mark != Mark::Sparse && dd.chance(0.6) {
                                layer.clips.push(Clip {
                                    bar: ph,
                                    bars: 7,
                                    pattern: pat,
                                    mark,
                                    gain: 1.0,
                                });
                                layer.clips.push(Clip {
                                    bar: ph + 7,
                                    bars: 1,
                                    pattern: fi,
                                    mark: Mark::Var,
                                    gain: 1.0,
                                });
                            } else {
                                layer.clips.push(Clip {
                                    bar: ph,
                                    bars: len,
                                    pattern: pat,
                                    mark,
                                    gain: 1.0,
                                });
                            }
                            ph += len;
                        }
                    } else if !pat_of.is_empty() {
                        let prog = if chorus(bi) { 1 } else { 0 };
                        let tr_block = key_change && p.form[bi].name == "middle eight";
                        // Patterns were pushed per (prog, transpose) in
                        // groups of three: core, var, sparse.
                        let group = prog * if key_change { 2 } else { 1 } + usize::from(tr_block);
                        let want = match mark {
                            Mark::Var => Mark::Var,
                            Mark::Sparse => Mark::Sparse,
                            _ => Mark::Core,
                        };
                        let pi = pat_of[group * 3
                            + match want {
                                Mark::Var => 1,
                                Mark::Sparse => 2,
                                _ => 0,
                            }]
                        .2;
                        // A pattern restarts with its block, so the hook's
                        // call lands on the chorus's downbeat.
                        layer.clips.push(Clip {
                            bar: from,
                            bars: b1 - from,
                            pattern: pi,
                            mark,
                            gain: 1.0,
                        });
                    }
                    if mark == Mark::Filtered {
                        // One sweep over a run of filtered blocks.
                        let filt = |c: &Cell| matches!(c, Cell::On(Mark::Filtered, _));
                        if bi == 0 || !filt(&cells[bi - 1]) {
                            filtered = true;
                            let run = cells[bi..].iter().take_while(|c| filt(c)).count();
                            let end: u32 = p.form[bi..bi + run].iter().map(|b| b.bars).sum();
                            lp = lp.to(b0 as f64, 400.0, Curve::Step).to(
                                (b0 + end) as f64 - 0.01,
                                2400.0,
                                Curve::Exp,
                            );
                        }
                    } else {
                        lp = lp.to(b0 as f64, 20000.0, Curve::Step);
                    }
                }
                Cell::Build(range) => {
                    let (a, b) = range.unwrap_or((b0, b1));
                    match *rk {
                        "R" => {}
                        _ => {
                            let pi = layer.pattern(parts::roll(b - a, &mut dd));
                            layer.clips.push(Clip {
                                bar: a,
                                bars: b - a,
                                pattern: pi,
                                mark: Mark::Build,
                                gain: 1.0,
                            });
                        }
                    }
                }
                Cell::Hit(bar) => {
                    let at = bar.unwrap_or(b0);
                    let pi = layer.pattern(parts::one_hit("crash"));
                    layer.clips.push(Clip {
                        bar: at,
                        bars: 1,
                        pattern: pi,
                        mark: Mark::Hit,
                        gain: 1.0,
                    });
                }
            }
        }
        if filtered {
            layer.autos.push(lp);
        }
        // Beds mixed under the drums play up while they are exposed,
        // before the kick comes in (engine default).
        if matches!(*rk, "Pd" | "Atm") && kick_in > 0 && layer.clips.iter().any(|c| c.bar < kick_in)
        {
            layer.autos.push(
                Automation::new("level")
                    .to(0.0, EXPOSED_LIFT, Curve::Step)
                    .to(kick_in as f64, 1.0, Curve::Step),
            );
        }
        if !layer.clips.is_empty() {
            song.layers.push(layer);
        }
    }

    // Risers into the drops, and the bus high-pass rising under them
    // (G4: "HPF on the music bus rising to remove the bottom end just
    // before the drop", reset at the drop).
    if let Some((rk, cells)) = p.template.iter().find(|(k, _)| *k == "R") {
        let mut hp = Automation::new("hp").to(0.0, 0.0, Curve::Step);
        for (bi, c) in row(rk, cells).into_iter().enumerate() {
            if let Cell::Build(range) = c {
                let (a, b) = range.unwrap_or((starts[bi], starts[bi] + p.form[bi].bars));
                song.fx.push(Fx::Riser {
                    bar: a as f64,
                    bars: (b - a) as f64,
                    level: 1.0,
                });
                hp = hp
                    .to(a as f64, 20.0, Curve::Step)
                    .to(b as f64 - 0.05, 380.0, Curve::Parabolic)
                    .to(b as f64, 0.0, Curve::Step);
            }
        }
        song.bus.push(hp);
    }
    song.notes = notes;
    song
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_fits_its_form() {
        for p in PROFILES {
            for (k, cells) in p.template {
                assert_eq!(
                    cells.split_whitespace().count(),
                    p.form.len(),
                    "{} row {k}",
                    p.key
                );
                assert!(role_of(k).is_some() || *k == "R", "{} row {k}", p.key);
            }
            for i in p.intros {
                for (k, cells) in i.cells {
                    assert!(
                        p.template.iter().any(|(t, _)| t == k),
                        "{} intro {} row {k}",
                        p.key,
                        i.name
                    );
                    assert!(
                        cells.split_whitespace().count() <= p.form.len(),
                        "{} intro {} row {k}",
                        p.key,
                        i.name
                    );
                }
            }
            for (s, _) in p.minor_progs.iter().chain(p.major_progs) {
                assert!(
                    theory::progression(s, &theory::AEOLIAN).is_some(),
                    "{} {s}",
                    p.key
                );
            }
            for n in p
                .bass_sounds
                .iter()
                .chain(p.pad_sounds)
                .chain(p.arp_sounds)
                .chain(p.lead_sounds)
            {
                assert!(bpatch(n).is_some(), "{} sound {n}", p.key);
            }
        }
    }

    #[test]
    fn songs_are_deterministic_and_layered() {
        for p in PROFILES {
            let a = generate(p.key, 7, &Settings::default()).unwrap();
            let b = generate(p.key, 7, &Settings::default()).unwrap();
            assert_eq!(a, b, "{}", p.key);
            let c = generate(p.key, 8, &Settings::default()).unwrap();
            assert_ne!(a, c, "{}: another seed, another song", p.key);
            assert!(a.layers.len() >= 6, "{}: {} layers", p.key, a.layers.len());
            assert!(a.bpm >= p.tempo.0 && a.bpm <= p.tempo.1);
            // Layers enter at different times (no whole-song blocks).
            let entries: std::collections::BTreeSet<u32> = a
                .layers
                .iter()
                .filter_map(|l| l.clips.iter().map(|c| c.bar).min())
                .collect();
            assert!(entries.len() >= 3, "{}: entries {entries:?}", p.key);
            // Every entry falls on a 4-bar boundary (G2), or a named bar.
            for l in &a.layers {
                for c in &l.clips {
                    assert!(
                        c.bar + c.bars <= a.bars,
                        "{} {}: clip past the end",
                        p.key,
                        l.name
                    );
                }
            }
        }
    }

    #[test]
    fn synthwave_opens_in_more_than_one_way() {
        // Where the kick and the bass first play, per seed.
        let first = |s: &Song, name: &str| {
            s.layers
                .iter()
                .find(|l| l.name == name)
                .and_then(|l| l.clips.iter().map(|c| c.bar).min())
        };
        for genre in ["synthwave", "outrun"] {
            let (mut drums_first, mut drums_later, mut bass_first) = (0, 0, 0);
            for seed in 0..60 {
                let s = generate(genre, seed, &Settings::default()).unwrap();
                match first(&s, "k") {
                    Some(0) => drums_first += 1,
                    _ => drums_later += 1,
                }
                if first(&s, "bass") == Some(0) {
                    bass_first += 1;
                }
                // Pads exposed before the kick play up until it comes in.
                if first(&s, "chords") == Some(0) && first(&s, "k").is_some_and(|b| b > 0) {
                    let pad = s.layers.iter().find(|l| l.name == "chords").unwrap();
                    let lv = pad.autos.iter().find(|a| a.param == "level").unwrap();
                    let kick = first(&s, "k").unwrap() as f64;
                    assert!(lv.at(0.0).unwrap() > 1.0 && lv.at(kick).unwrap() == 1.0);
                }
            }
            assert!(drums_first > 0, "{genre}: {drums_first} open on the kick");
            assert!(drums_later > 0, "{genre}: {drums_later} hold it back");
            assert!(bass_first > 0, "{genre}: {bass_first} open on the bass");
        }
    }

    #[test]
    fn a_filtered_run_is_one_sweep() {
        // Two filtered blocks in a row open the filter once, not twice.
        let s = (0..60)
            .map(|seed| generate("synthwave", seed, &Settings::default()).unwrap())
            .find(|s| s.notes.iter().any(|n| n.contains("filter")))
            .expect("a filtered intro");
        let pad = s.layers.iter().find(|l| l.name == "chords").unwrap();
        let lp = pad.autos.iter().find(|a| a.param == "lp").unwrap();
        let hz: Vec<f64> = (0..32).map(|h| lp.at(h as f64 / 2.0).unwrap()).collect();
        assert!(hz[0] < 500.0 && hz[31] > 1500.0, "{hz:?}");
        assert!(
            hz.windows(2).all(|w| w[1] >= w[0]),
            "never closes again: {hz:?}"
        );
    }

    #[test]
    fn a_song_renders() {
        let s = generate("synthwave", 3, &Settings::default()).unwrap();
        let mut pl = crate::song::SongPlayer::new(5, 48000.0);
        pl.set_song(s);
        pl.play(32, 0.0);
        let (mut l, mut r) = (vec![0f32; 128], vec![0f32; 128]);
        let mut sum = 0.0;
        let mut peak = 0f32;
        for _ in 0..(6.0 * 48000.0 / 128.0) as usize {
            pl.process(&mut l, &mut r);
            for x in l.iter().chain(&r) {
                assert!(x.is_finite());
                sum += (*x as f64).powi(2);
                peak = peak.max(x.abs());
            }
        }
        let rms = (sum / (6.0 * 48000.0 * 2.0)).sqrt();
        assert!(rms > 0.02, "the chorus is heard: {rms}");
        assert!(peak <= 1.0);
    }
}
