//! The new generator's song description (PLAN.md Phase 3): a song is a set
//! of **layers**, one per instrument, each with its own timeline of clips,
//! instead of the classic [`crate::track::Track`]'s whole-song section
//! blocks. A layer enters, drops out and varies on its own, on phrase
//! boundaries (research/RULES.md G2, G3); sections are labels for the
//! viewer and the energy curve, not the unit of playback.
//!
//! - A [`Layer`] has a [`Sound`] (a lab patch, or a drum kit), a mixer
//!   channel, its own sidechain depth (RULES.md G7, "What the engine lacks"
//!   #6) and the energy below which it drops out (G5).
//! - A [`Clip`] places one of the layer's [`Pattern`]s over a run of bars.
//!   The pattern loops inside the clip from the clip's start, at its own
//!   length (any number of 16ths: odd cycles phase against the bar, #3).
//! - A [`Note`] sits at a fractional 16th (32nds, triplets and micro-timing
//!   are fractions, #1, #3) with a continuous velocity.
//! - [`Automation`] moves a layer's patch parameter, its level or filters,
//!   or the music bus's filters, along curves over bars (#2).
//! - [`Fx`] are the transition sounds that are not notes: noise risers and
//!   downlifters (G4).
//!
//! Times are in bars and 16th steps (16 to a bar: RULES.md "Units");
//! [`crate::song`] turns them into samples at the song's tempo and swing.

use crate::json::Val;
use crate::track::{Ch, Lab};

/// A layer's role in the arrangement (RULES.md G3's keys), which the viewer
/// groups by and the generator's rules speak of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Kick,
    Hats,
    Clap,
    Perc,
    Bass,
    Sub,
    Pad,
    Arp,
    Lead,
    Vocal,
    Atmos,
    Fx,
}

impl Role {
    pub fn key(self) -> &'static str {
        match self {
            Role::Kick => "K",
            Role::Hats => "H",
            Role::Clap => "C",
            Role::Perc => "P",
            Role::Bass => "B",
            Role::Sub => "Sub",
            Role::Pad => "Pd",
            Role::Arp => "A",
            Role::Lead => "L",
            Role::Vocal => "V",
            Role::Atmos => "Atm",
            Role::Fx => "X",
        }
    }
}

/// What a layer plays on.
#[derive(Clone, Debug, PartialEq)]
pub enum Sound {
    /// A lab instrument (`patches::bpatch` with overrides).
    Synth(Box<Lab>),
    /// A drum machine: notes name its lanes; per-lane voice parameters by
    /// their JS names lay over the kit's (`kitTweak`).
    Drums {
        kit: String,
        tweak: Vec<(String, Vec<(String, f64)>)>,
    },
}

/// A note: pitched (`midis`, a chord when several) or a drum hit (`lane`).
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// 16ths from the pattern's start (fractions allowed).
    pub at: f64,
    /// Length in 16ths (gate time; a drum hit's choke length, 0 for the
    /// voice's own decay).
    pub len: f64,
    pub midis: Vec<f64>,
    pub lane: Option<String>,
    /// 0..1.
    pub vel: f64,
    pub accent: bool,
    /// Slides from the previous note (303, mono glide).
    pub slide: bool,
}

impl Note {
    pub fn hit(at: f64, lane: &str, vel: f64) -> Note {
        Note {
            at,
            len: 0.0,
            midis: Vec::new(),
            lane: Some(lane.to_owned()),
            vel,
            accent: false,
            slide: false,
        }
    }

    pub fn pitched(at: f64, len: f64, midis: Vec<f64>, vel: f64) -> Note {
        Note {
            at,
            len,
            midis,
            lane: None,
            vel,
            accent: false,
            slide: false,
        }
    }
}

/// A loop of notes `len` 16ths long (any length; a 4-bar loop is 64).
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub name: String,
    pub len: f64,
    pub notes: Vec<Note>,
}

/// How a clip plays its pattern, for the viewer (RULES.md's arrangement
/// cell codes); the sound of it is in the pattern and the automation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// `x`: the core pattern.
    Core,
    /// `v`: a variation.
    Var,
    /// `f`: filtered or teased.
    Filtered,
    /// `s`: sparse or sliced.
    Sparse,
    /// `r`: a build device (roll, riser).
    Build,
    /// `1`: one hit.
    Hit,
}

impl Mark {
    pub fn key(self) -> &'static str {
        match self {
            Mark::Core => "x",
            Mark::Var => "v",
            Mark::Filtered => "f",
            Mark::Sparse => "s",
            Mark::Build => "r",
            Mark::Hit => "1",
        }
    }
}

/// A pattern placed over `bars` bars from `bar` (0-based).
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub bar: u32,
    pub bars: u32,
    /// An index into the layer's patterns.
    pub pattern: usize,
    pub mark: Mark,
    /// A gain on every note's velocity (a teased entry, a quieter return).
    pub gain: f64,
}

/// An automation curve's shape between two points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    Linear,
    /// Equal ratios per unit of time (filter cutoffs, gains in dB).
    Exp,
    /// Slow then fast (a build's filter "fully open at the drop", G4).
    Parabolic,
    /// Holds the first value, jumps at the end.
    Step,
}

/// One automated parameter: `(bar, value)` points (bars fractional) with
/// the curve into each point. Before the first point and after the last,
/// the nearest point's value holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Automation {
    /// A patch parameter by its JS name (`cutoff`, `res`, `decay`, …), or
    /// the channel's `level`, `hp` or `lp` (Hz); on the song's bus
    /// automation, the music bus's `hp` or `lp`.
    pub param: String,
    pub points: Vec<(f64, f64, Curve)>,
}

impl Automation {
    pub fn new(param: &str) -> Automation {
        Automation {
            param: param.to_owned(),
            points: Vec::new(),
        }
    }

    /// Adds a point (keep them in bar order).
    pub fn to(mut self, bar: f64, v: f64, c: Curve) -> Automation {
        self.points.push((bar, v, c));
        self
    }

    /// The value at `bar`; `None` with no points.
    pub fn at(&self, bar: f64) -> Option<f64> {
        let p = &self.points;
        let first = p.first()?;
        if bar <= first.0 {
            return Some(first.1);
        }
        for w in p.windows(2) {
            let (b0, v0, _) = w[0];
            let (b1, v1, c) = w[1];
            if bar < b1 {
                let u = if b1 > b0 { (bar - b0) / (b1 - b0) } else { 1.0 };
                return Some(match c {
                    Curve::Linear => v0 + (v1 - v0) * u,
                    Curve::Exp if v0 > 0.0 && v1 > 0.0 => v0 * (v1 / v0).powf(u),
                    Curve::Exp => v0 + (v1 - v0) * u,
                    Curve::Parabolic => v0 + (v1 - v0) * u * u,
                    Curve::Step => v0,
                });
            }
        }
        Some(p.last().map(|x| x.1).unwrap_or(first.1))
    }
}

/// A layer: one instrument's timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub role: Role,
    pub sound: Sound,
    pub ch: Ch,
    /// Sidechain depth from the kick, 0..1 (gain falls to 1 − depth).
    pub pump: f64,
    /// Below this energy the layer drops out (G5).
    pub min_energy: f64,
    /// Its swing, when it differs from the song's (Linn %).
    pub swing: Option<f64>,
    /// Whether its hits key the sidechain (the kick).
    pub keys_pump: bool,
    /// Its own gated reverb, keyed by its hits.
    pub gated: Option<Gated>,
    pub patterns: Vec<Pattern>,
    pub clips: Vec<Clip>,
    pub autos: Vec<Automation>,
}

impl Layer {
    pub fn new(name: &str, role: Role, sound: Sound) -> Layer {
        Layer {
            name: name.to_owned(),
            role,
            sound,
            ch: Ch::default(),
            pump: 0.0,
            min_energy: 0.0,
            swing: None,
            keys_pump: false,
            gated: None,
            patterns: Vec::new(),
            clips: Vec::new(),
            autos: Vec::new(),
        }
    }

    /// Adds a pattern; returns its index.
    pub fn pattern(&mut self, p: Pattern) -> usize {
        self.patterns.push(p);
        self.patterns.len() - 1
    }

    /// Places pattern `pattern` over `bars` bars from `bar`.
    pub fn clip(&mut self, bar: u32, bars: u32, pattern: usize, mark: Mark) {
        self.clips.push(Clip {
            bar,
            bars,
            pattern,
            mark,
            gain: 1.0,
        });
    }
}

/// A gated reverb on one layer (RULES.md G8 `gatedVerb`): a hall the
/// layer sends to, cut off `hold` seconds after each of its hits. One per
/// drum, as the records had: a long hold on the snare, a short one and a
/// little send on the kick [synthwave#3][synthwave#25][synthwave#26].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gated {
    /// The send, 0..1 (1: the whole dry signal).
    pub send: f64,
    /// Seconds the gate holds open after a hit (0.3–0.5).
    pub hold: f64,
    /// The hall's decay time, seconds (1.5–2.5).
    pub rt60: f64,
}

/// A transition sound that is not a note (G4).
#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    /// Noise swept up over `bars` bars, ending on `bar + bars`.
    Riser { bar: f64, bars: f64, level: f64 },
    /// Noise falling after a peak.
    Downlifter { bar: f64, bars: f64, level: f64 },
}

/// A section label: where it starts, how long, its name and the energy the
/// plan gave it.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub name: String,
    pub bar: u32,
    pub bars: u32,
    pub energy: f64,
}

/// A whole song.
#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub title: String,
    /// The line the radio shows: sub-style, key, progression.
    pub style: String,
    pub genre: String,
    pub seed: u32,
    pub bpm: f64,
    /// Linn/MPC swing percentage (50 straight, 66.7 triplet; RULES.md).
    pub swing: f64,
    pub bars: u32,
    pub sections: Vec<Section>,
    pub layers: Vec<Layer>,
    pub fx: Vec<Fx>,
    /// The music bus's filters (`hp`, `lp`).
    pub bus: Vec<Automation>,
    /// Sidechain release in seconds.
    pub pump_release: f64,
    /// Ping-pong delay time in beats, and its feedback.
    pub delay_beats: f64,
    pub delay_fb: f64,
    /// Bus tape saturation, glue compression and reverb return level
    /// (RULES.md G7).
    pub tape: f64,
    pub glue: f64,
    pub reverb: f64,
    /// Master gain into the bus.
    pub gain: f64,
    /// Why the song is the way it is: the generator's choices and the rule
    /// behind each, for the lab's notes (Phase 4).
    pub notes: Vec<String>,
}

impl Song {
    pub fn new(genre: &str, seed: u32, bpm: f64, bars: u32) -> Song {
        Song {
            title: String::new(),
            style: String::new(),
            genre: genre.to_owned(),
            seed,
            bpm,
            swing: 50.0,
            bars,
            sections: Vec::new(),
            layers: Vec::new(),
            fx: Vec::new(),
            bus: Vec::new(),
            pump_release: 0.2,
            delay_beats: 0.75,
            delay_fb: 0.35,
            tape: 0.35,
            glue: 0.5,
            reverb: 1.0,
            gain: 1.0,
            notes: Vec::new(),
        }
    }

    /// Seconds per 16th.
    pub fn step_secs(&self) -> f64 {
        60.0 / self.bpm / 4.0
    }

    /// The song's length in seconds.
    pub fn secs(&self) -> f64 {
        self.bars as f64 * 16.0 * self.step_secs()
    }

    /// The section a bar is in.
    pub fn section_at(&self, bar: u32) -> Option<usize> {
        self.sections.iter().rposition(|s| s.bar <= bar)
    }

    /// The song as the viewer and the radio page need it, as JSON: the
    /// metadata, the sections, and every layer's clips (not its notes).
    pub fn summary(&self) -> Val {
        let layers = self
            .layers
            .iter()
            .map(|l| {
                let sound = match &l.sound {
                    Sound::Synth(lab) => {
                        format!("{} · {}", lab.kind, lab.name.as_deref().unwrap_or(""))
                    }
                    Sound::Drums { kit, .. } => kit.clone(),
                };
                Val::obj(vec![
                    ("name", Val::str(&l.name)),
                    ("role", Val::str(l.role.key())),
                    ("sound", Val::str(&sound)),
                    (
                        "lab",
                        match &l.sound {
                            Sound::Synth(lab) => Some(lab.to_val()),
                            Sound::Drums { .. } => None,
                        },
                    ),
                    ("pump", Val::num(l.pump)),
                    ("minEnergy", Val::num(l.min_energy)),
                    (
                        "gated",
                        l.gated.map(|g| {
                            Val::obj(vec![
                                ("send", Val::num(g.send)),
                                ("hold", Val::num(g.hold)),
                                ("rt60", Val::num(g.rt60)),
                            ])
                        }),
                    ),
                    (
                        "clips",
                        Some(Val::Arr(
                            l.clips
                                .iter()
                                .map(|c| {
                                    Val::obj(vec![
                                        ("bar", Val::num(c.bar as f64)),
                                        ("bars", Val::num(c.bars as f64)),
                                        ("mark", Val::str(c.mark.key())),
                                        ("pattern", Val::str(&l.patterns[c.pattern].name)),
                                    ])
                                })
                                .collect(),
                        )),
                    ),
                    (
                        "autos",
                        Some(Val::Arr(
                            l.autos.iter().map(|a| Val::Str(a.param.clone())).collect(),
                        )),
                    ),
                ])
            })
            .collect();
        let fx = self
            .fx
            .iter()
            .map(|f| match f {
                Fx::Riser { bar, bars, .. } => Val::obj(vec![
                    ("type", Val::str("riser")),
                    ("bar", Val::num(*bar)),
                    ("bars", Val::num(*bars)),
                ]),
                Fx::Downlifter { bar, bars, .. } => Val::obj(vec![
                    ("type", Val::str("downlifter")),
                    ("bar", Val::num(*bar)),
                    ("bars", Val::num(*bars)),
                ]),
            })
            .collect();
        Val::obj(vec![
            ("title", Val::str(&self.title)),
            ("style", Val::str(&self.style)),
            ("genre", Val::str(&self.genre)),
            ("seed", Val::num(self.seed as f64)),
            ("bpm", Val::num(self.bpm)),
            ("swing", Val::num(self.swing)),
            ("bars", Val::num(self.bars as f64)),
            (
                "sections",
                Some(Val::Arr(
                    self.sections
                        .iter()
                        .map(|s| {
                            Val::obj(vec![
                                ("name", Val::str(&s.name)),
                                ("bar", Val::num(s.bar as f64)),
                                ("bars", Val::num(s.bars as f64)),
                                ("energy", Val::num(s.energy)),
                            ])
                        })
                        .collect(),
                )),
            ),
            ("layers", Some(Val::Arr(layers))),
            ("fx", Some(Val::Arr(fx))),
            (
                "notes",
                Some(Val::Arr(
                    self.notes.iter().map(|n| Val::Str(n.clone())).collect(),
                )),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_curves() {
        let a = Automation::new("cutoff")
            .to(0.0, 100.0, Curve::Linear)
            .to(8.0, 1600.0, Curve::Exp)
            .to(16.0, 0.0, Curve::Step);
        assert_eq!(a.at(-1.0), Some(100.0));
        assert!(
            (a.at(4.0).unwrap() - 400.0).abs() < 1e-6,
            "halfway up in ratio"
        );
        assert_eq!(a.at(12.0), Some(1600.0), "a step holds");
        assert_eq!(a.at(20.0), Some(0.0));
        let p = Automation::new("hp")
            .to(0.0, 0.0, Curve::Linear)
            .to(4.0, 1.0, Curve::Parabolic);
        assert!((p.at(2.0).unwrap() - 0.25).abs() < 1e-9);
    }
}
