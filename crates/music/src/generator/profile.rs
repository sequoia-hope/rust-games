//! Genre profiles: research/RULES.md's profiles as data. Every number
//! carries its citation (`[file#n]` is reference n of `research/…/file.md`)
//! or is marked an engine default; `notes` are the short, sourced reasons
//! the lab shows beside a song.
//!
//! Steps are written 1–16 as in RULES.md (1 is the downbeat) and turned
//! 0-based by the writers.

use super::theory::{AEOLIAN, DORIAN, IONIAN, MIXOLYDIAN, Mode, PHRYGIAN};
use crate::patches::bpatch;
use crate::track::Lab;

/// A drum lane of a profile: which kit lane, which layer (role key), its
/// steps on the 16-grid of a 1- or 2-bar loop, its velocity, and how
/// likely the profile is to use it at all.
#[derive(Clone, Copy, Debug)]
pub struct DrumLane {
    pub lane: &'static str,
    /// `K`, `H`, `C` or `P`.
    pub role: &'static str,
    /// Steps of bar 1 (1–16).
    pub steps: &'static [u8],
    /// Steps of bar 2 of a 2-bar loop (`None`: as bar 1).
    pub steps2: Option<&'static [u8]>,
    pub vel: f64,
    /// Velocity of the even steps (the "e" and "a"), when it differs.
    pub vel_even: Option<f64>,
    /// ± random velocity (humanising).
    pub jitter: f64,
    /// Probability the song uses this lane.
    pub p: f64,
}

const fn lane(lane: &'static str, role: &'static str, steps: &'static [u8], vel: f64) -> DrumLane {
    DrumLane {
        lane,
        role,
        steps,
        steps2: None,
        vel,
        vel_even: None,
        jitter: 0.0,
        p: 1.0,
    }
}

/// How the bass moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BassStyle {
    /// The root in 8ths, plucked (synthwave [synthwave#2][synthwave#39]).
    Eighths,
    /// 1-bar cells: off-beat root, root + octave, pickup into the next
    /// chord (Chicago, engine default from [house#1], LHL 2–7
    /// [cognition#20]).
    HouseCells,
    /// A 1-bar arpeggiated figure synced to the drums (deep house
    /// [house#16][house#17]).
    Rolling,
    /// Low strings and tuba striding in half-notes and fifths, with a
    /// dotted-quarter push into the next bar (orchestral walker score).
    Orchestral,
    /// 16th-note TB-303 acid line: 7/12 step gate on normal notes, full-step
    /// ties with `slide` on 15–25 % of steps, and `accent` on 25–35 % of steps
    /// so the C13 sweep capacitor builds up (RULES.md P8 [techno#7][techno#8]).
    Acid303,
    /// Uplifting trance off-beat & rolling 16th bass: strong off-beat 8ths on
    /// 3, 7, 11, 15 with 16th pedal pulses between the kicks (RULES.md P14
    /// [trance#1][trance#3]).
    Offbeat16,
    /// Drum & bass two-step roller bass: sustained sub/Reese notes locked to
    /// steps 1 and 11 with a bar-end 16th inflection (RULES.md P12, P13
    /// [dnb#11][dnb#20]).
    DnbRoller,
    /// Dubstep half-time wobble bass: long LFO-modulated notes on beat 1 and
    /// after the beat-3 snare with a syncopated tail (RULES.md P24
    /// [ambient#38][ambient#39]).
    DubstepWobble,
}

impl BassStyle {
    pub const fn key(self) -> &'static str {
        match self {
            BassStyle::Eighths => "eighths",
            BassStyle::HouseCells => "house",
            BassStyle::Rolling => "rolling",
            BassStyle::Orchestral => "orchestral",
            BassStyle::Acid303 => "acid",
            BassStyle::Offbeat16 => "trance",
            BassStyle::DnbRoller => "dnb",
            BassStyle::DubstepWobble => "wobble",
        }
    }

    pub fn from_key(s: &str) -> Option<BassStyle> {
        Some(match s {
            "eighths" => BassStyle::Eighths,
            "house" => BassStyle::HouseCells,
            "rolling" => BassStyle::Rolling,
            "orchestral" => BassStyle::Orchestral,
            "acid" => BassStyle::Acid303,
            "trance" => BassStyle::Offbeat16,
            "dnb" => BassStyle::DnbRoller,
            "wobble" => BassStyle::DubstepWobble,
            _ => return None,
        })
    }
}

/// How the chords are played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChordStyle {
    /// Held a bar each (pads [synthwave#2]).
    Sustain,
    /// Short stabs on off-16ths ([sound-design#27][sound-design#28]).
    Stabs,
}

impl ChordStyle {
    pub const fn key(self) -> &'static str {
        match self {
            ChordStyle::Sustain => "sustain",
            ChordStyle::Stabs => "stabs",
        }
    }

    pub fn from_key(s: &str) -> Option<ChordStyle> {
        Some(match s {
            "sustain" => ChordStyle::Sustain,
            "stabs" => ChordStyle::Stabs,
            _ => return None,
        })
    }
}

/// A block of the form: its name and length in bars, and the energy the
/// plan gives it (RULES.md G5).
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub name: &'static str,
    pub bars: u32,
    pub energy: f64,
}

const fn block(name: &'static str, bars: u32, energy: f64) -> Block {
    Block { name, bars, energy }
}

/// One way a song of the profile can open: replacement cells for the
/// first blocks of the arrangement table, per role. A row's cells replace
/// the template's from the first block on; rows and blocks it leaves out
/// keep the template's. The song draws one by weight.
#[derive(Clone, Copy, Debug)]
pub struct Intro {
    pub name: &'static str,
    pub p: f64,
    pub cells: &'static [(&'static str, &'static str)],
    /// The sourced note the lab shows when a song opens this way.
    pub note: &'static str,
}

/// A genre profile (RULES.md's field list, as far as the generator uses
/// it).
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub key: &'static str,
    pub name: &'static str,
    /// Lowest, highest, typical BPM.
    pub tempo: (f64, f64, f64),
    /// Linn swing range (%).
    pub swing: (f64, f64),
    pub modes: &'static [(Mode, f64)],
    /// Progressions (numerals) with weights, for minor-family modes and for
    /// major ones.
    pub minor_progs: &'static [(&'static str, f64)],
    pub major_progs: &'static [(&'static str, f64)],
    /// Probabilities of a 7th, a 9th, an 11th on a chord that has none.
    pub ext: (f64, f64, f64),
    pub kit: &'static str,
    pub drums: &'static [DrumLane],
    pub bass: BassStyle,
    pub chords: ChordStyle,
    /// Has an arpeggio layer (synthwave's hook [synthwave#38]).
    pub arp: bool,
    /// Lead notes per bar, its range in semitones, and the chance a song
    /// has a lead at all.
    pub lead: (f64, i32, f64),
    /// Tonic register: the bass's root lies in this MIDI range.
    pub bass_range: (i32, i32),
    /// Patches per role, picked from (lab patch names).
    pub bass_sounds: &'static [&'static str],
    pub pad_sounds: &'static [&'static str],
    pub arp_sounds: &'static [&'static str],
    pub lead_sounds: &'static [&'static str],
    /// The form.
    pub form: &'static [Block],
    /// The arrangement: per role key, one cell per block (RULES.md's cell
    /// codes; `+N`, `1N`, `rA-B` name bars, 1-based).
    pub template: &'static [(&'static str, &'static str)],
    /// Ways to open, drawn by weight (empty: the template's own).
    pub intros: &'static [Intro],
    /// Level offsets per role key in dB, on the generator's channel levels
    /// (which suit house's patches).
    pub levels: &'static [(&'static str, f64)],
    /// Sidechain depth on the pumped layers (RULES.md G7).
    pub pump: f64,
    /// The drum fills: `true` for synthwave's snare or tom runs every 4th
    /// or 8th bar, `false` for house's turnaround kick.
    pub tom_fills: bool,
    /// Bus: tape and glue (RULES.md G7), reverb.
    pub tape: f64,
    pub glue: f64,
    pub reverb: f64,
    /// Gated-reverb sends on the snare or clap layer and on the kick layer
    /// (0: none; RULES.md G8 `gatedVerb`).
    pub gated: (f64, f64),
    /// Short, sourced notes on why it sounds as it does.
    pub notes: &'static [&'static str],
}

// ── Synthwave (RULES.md P18) ─────────────────────────────────────────

const SYNTH_MINOR: &[(&str, f64)] = &[
    // [synthwave#23][synthwave#38]; weights engine default, the VI–VII
    // ascent weighted high. Fitted to 4-bar loops (engine default):
    // "i–III–VII" holds its VII, "VI–VII–III–VII–i" drops a VII; the axis
    // i–VI–III–VII and the bare VI–VII cadence i–VI–VII–i are engine
    // defaults built on the family's cadence [synthwave#23].
    ("i–VII6–VI–iv", 0.15),
    ("VI–VII–v–VI", 0.12),
    ("VI(add9)–VII–i7", 0.12),
    ("i–VI7–VII–i", 0.1),
    ("i–III–VII–VII", 0.1),
    ("VI–VII–III–i", 0.08),
    ("i–VI–III–VII", 0.13),
    ("iv–i–III–V", 0.05),
    ("i–VI–VII–i", 0.15),
];
const SYNTH_MAJOR: &[(&str, f64)] = &[
    // [synthwave#11][synthwave#23][synthwave#50]
    ("IV–V–vi–I", 0.4),
    ("I–♭VII–♭VI–♭VII", 0.25),
    ("I–V–vi–IV", 0.2),
    ("vi–IV–I–V", 0.15),
];

/// The synthwave grid at 95–115 BPM [synthwave#3]: kick 1, 9 (bar 2 +16),
/// snare 5, 13, 16th hats, an open hat on 14.
const SYNTH_DRUMS: &[DrumLane] = &[
    DrumLane {
        steps2: Some(&[1, 9, 16]),
        ..lane("kick", "K", &[1, 9], 1.0)
    },
    lane("snare", "C", &[5, 13], 1.0),
    DrumLane {
        vel_even: Some(0.55),
        jitter: 0.12,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            0.7,
        )
    },
    lane("ohat", "H", &[14], 0.6),
    DrumLane {
        p: 0.6,
        ..lane("clap", "C", &[5, 13], 0.5)
    },
];

/// Outrun at 128–140 [synthwave#3][synthwave#11]: four on the floor with a
/// ghost on 15, clap 5, 13, off-beat open hats.
const OUTRUN_DRUMS: &[DrumLane] = &[
    lane("kick", "K", &[1, 5, 9, 13], 1.0),
    DrumLane {
        p: 0.5,
        ..lane("kick", "K", &[15], 0.8)
    },
    lane("clap", "C", &[5, 13], 0.9),
    lane("snare", "C", &[5, 13], 0.8),
    DrumLane {
        jitter: 0.1,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            0.55,
        )
    },
    lane("ohat", "H", &[3, 7, 11, 15], 0.65),
];

/// The pop/DJ form of RULES.md P18 (bar counts engine default; 128 bars).
const SYNTH_FORM: &[Block] = &[
    block("intro", 8, 0.3),
    block("intro", 8, 0.4),
    block("verse", 8, 0.55),
    block("verse", 8, 0.6),
    block("chorus", 16, 0.85),
    block("verse", 8, 0.6),
    block("chorus", 16, 0.9),
    block("middle eight", 8, 0.7),
    block("solo", 8, 0.8),
    block("break", 8, 0.35),
    block("chorus", 16, 1.0),
    block("outro", 16, 0.5),
];
const SYNTH_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x x x x x x x x x - x f"),
    ("C", "x x x x x x x x x - x f"),
    ("H", "- + x x x x x x x - x -"),
    ("B", "- - + x x x x x x - x x"),
    ("A", "- - - + x x x s x - x -"),
    ("Pd", "- - - - x s x x x x x x"),
    ("L", "- - - - x - x - v - x f"),
    ("R", "- - - r - - - - - r - -"),
    ("X", "- - 1 - 1 - 1 - - - 1 -"),
];

/// Synthwave openings over the first four blocks (bars 1–8, 9–16, 17–24,
/// 25–32). Of the reference tracks with a sourced intro, none opens on the
/// drums: two on pads or the arpeggio, two on the bass
/// [synthwave#59][synthwave#60][synthwave#62]; the guides build "pads and
/// arp first, then bass and stripped drums, then the full kit"
/// [synthwave#55][synthwave#56], one tutorial from the drums
/// [synthwave#58]. Weights follow that count (author's estimate).
const PADS_FIRST: Intro = Intro {
    name: "pads first",
    p: 0.4,
    cells: &[
        ("K", "- - x x"),
        ("C", "- r15-16 x x"),
        ("H", "- - x x"),
        ("B", "- + x x"),
        ("A", "- x x x"),
        ("Pd", "x x s s"),
        ("R", "- r13-16 - r"),
    ],
    note: "It opens on pads, the arpeggio and then the bass, and the drums come in at the verse after a two-bar snare roll: the guides' \"pads and arp first, then bass and stripped drums\", and the 8 bars of synths before Blinding Lights' drums [synthwave#55][synthwave#56][synthwave#60][synthwave#61][synthwave#62].",
};
const BASS_FIRST: Intro = Intro {
    name: "bass first",
    p: 0.3,
    cells: &[
        ("K", "- x x x"),
        ("C", "r7-8 x x x"),
        ("H", "- + x x"),
        ("B", "x x x x"),
        ("X", "- 1 1 -"),
    ],
    note: "It opens on the bass alone, as \"A Real Hero\" and \"Under Your Spell\" do, and the drums come in at bar 9 after a snare roll [synthwave#59].",
};
const FILTERED: Intro = Intro {
    name: "filtered",
    p: 0.15,
    cells: &[
        ("K", "f f x x"),
        ("C", "f f x x"),
        ("H", "f f x x"),
        ("B", "f f x x"),
        ("A", "f f - +"),
        ("Pd", "f f - -"),
        ("R", "- r13-16 - r"),
    ],
    note: "It opens on the whole band behind a low-pass filter that opens over 16 bars: the peak loop with its top subtracted, as Timecop1983 derives an intro from the peak [synthwave#50] (the filter sweep is an engine default).",
};
const DRUMS_FIRST: Intro = Intro {
    name: "drums first",
    p: 0.15,
    cells: &[],
    note: "It opens on kick and snare alone, the instrumental form's intro [synthwave#2][synthwave#58]; no sourced reference track opens this way, so it is drawn least.",
};
const SYNTH_INTROS: &[Intro] = &[PADS_FIRST, BASS_FIRST, FILTERED, DRUMS_FIRST];
/// Outrun: the same openings, drums first drawn more often for its
/// four-on-the-floor (engine default: no source compares the styles).
const OUTRUN_INTROS: &[Intro] = &[
    Intro {
        p: 0.3,
        ..PADS_FIRST
    },
    Intro {
        p: 0.25,
        ..BASS_FIRST
    },
    Intro {
        p: 0.15,
        ..FILTERED
    },
    Intro {
        p: 0.3,
        ..DRUMS_FIRST
    },
];

const SYNTH_NOTES: &[&str] = &[
    "Synthwave grew from 1980s film and TV scores (John Carpenter, Tangerine Dream, Vangelis, Jan Hammer) and Italo disco into a 2000s internet genre: Kavinsky, College, Lazerhawk, Perturbator [synthwave#1][synthwave#23].",
    "The Juno-style poly with its chorus is the default voice on pads, arps and leads, the chorus at the measured depth (±0.5 ms for I, ±1.1 ms for II); tape saturation sits on drums, bass and synths [synthwave#29][synthwave#38][synthwave#47][sound-design#44].",
    "The VI–VII ascent resolving to i is the family's cadence, one chord a bar over 4-bar loops [synthwave#23][synthwave#38].",
    "The arpeggio is the hook: eight notes a bar spelling the chord, as in Kavinsky's \"Nightcall\" [synthwave#38].",
    "Drums are hard-quantised, with no swing [synthwave#3]. The snare goes wholly into a gated reverb (a 2 s hall cut 0.45 s after each hit), the kick a little into one of its own, as the 80s records had [synthwave#3][synthwave#25][synthwave#26].",
];

pub const SYNTHWAVE: Profile = Profile {
    key: "synthwave",
    name: "Synthwave",
    // Dreamwave / night drive [synthwave#2][synthwave#3][synthwave#23].
    tempo: (95.0, 115.0, 108.0),
    swing: (50.0, 50.0),
    modes: &[
        (AEOLIAN, 0.6),
        (DORIAN, 0.15),
        (PHRYGIAN, 0.05),
        (IONIAN, 0.15),
        (MIXOLYDIAN, 0.05),
    ],
    minor_progs: SYNTH_MINOR,
    major_progs: SYNTH_MAJOR,
    // add9, 11, sus2, m7 on 30–50 % of chords in dreamwave [synthwave#23].
    ext: (0.3, 0.15, 0.0),
    kit: "tr808",
    drums: SYNTH_DRUMS,
    bass: BassStyle::Eighths,
    chords: ChordStyle::Sustain,
    arp: true,
    // ~1.4 beats a note, range 12–15 st [synthwave#23][synthwave#48].
    lead: (2.8, 14, 0.85),
    bass_range: (33, 45),
    bass_sounds: &[
        "sawBass",
        "moogBass",
        "pluckBass",
        "darkBass",
        "tranceBass",
        "dxBass",
        "fmBass",
    ],
    pad_sounds: &["warmPad", "superPad", "superPadWide", "darkPad", "choir"],
    arp_sounds: &[
        "pluck",
        "modernPluck",
        "sqArp",
        "pluckTrance",
        "acousticGuitar",
    ],
    lead_sounds: &[
        "sawLead",
        "moogLead",
        "syncLead",
        "supersaw",
        "leadGuitar",
        "nylonGuitar",
        "brassLead",
        "vocalLead",
        "sqLead",
    ],
    form: SYNTH_FORM,
    template: SYNTH_TEMPLATE,
    intros: SYNTH_INTROS,
    // The arp is the hook [synthwave#38]: in the chorus, K-weighted, the
    // arp about 5–7 LU under the bass and the lead 3–5 (engine default).
    levels: &[("A", 5.5), ("L", 3.2), ("Pd", 3.5)],
    pump: 0.3,
    tom_fills: true,
    tape: 0.4,
    glue: 0.5,
    reverb: 1.2,
    // Moderate gated verb on the snare; subtle on the kick [synthwave#3][synthwave#26].
    gated: (0.55, 0.08),
    notes: SYNTH_NOTES,
};

pub const OUTRUN: Profile = Profile {
    key: "outrun",
    name: "Outrun",
    // Outrun / darksynth [synthwave#3][synthwave#23][synthwave#24].
    tempo: (128.0, 140.0, 132.0),
    drums: OUTRUN_DRUMS,
    kit: "tr909",
    // Triads in darksynth [synthwave#23].
    ext: (0.15, 0.0, 0.0),
    modes: &[
        (AEOLIAN, 0.65),
        (PHRYGIAN, 0.15),
        (DORIAN, 0.1),
        (IONIAN, 0.1),
    ],
    bass_sounds: &[
        "sawBass",
        "moogBass",
        "darkBass",
        "reese",
        "tranceBass",
        "pluckBass",
    ],
    pad_sounds: &["darkPad", "superPadWide", "superPad", "warmPad"],
    arp_sounds: &["modernPluck", "pluckTrance", "sqArp", "pluck"],
    lead_sounds: &[
        "sawLead",
        "syncLead",
        "moogLead",
        "supersaw",
        "euroLead",
        "leadGuitar",
        "brassLead",
    ],
    intros: OUTRUN_INTROS,
    levels: &[("A", 5.5), ("L", 2.5), ("Pd", 3.5)],
    pump: 0.4,
    gated: (0.45, 0.06),
    ..SYNTHWAVE
};

// ── House (RULES.md P1, P2) ──────────────────────────────────────────

const HOUSE_MINOR: &[(&str, f64)] = &[
    // [house#15], weights engine default, Zipf-ordered [cognition#6];
    // the 3- and 2-chord vamps fitted to 4-bar loops (engine default).
    ("i7–v7–iv7–iv7", 0.25),
    ("iv7–VImaj7–i7–i7", 0.2),
    ("VImaj7–v7–VImaj7–v7", 0.2),
    ("i7–iv7–i7–iv7", 0.2),
    ("i7–i7–i7–i7", 0.15),
];
const HOUSE_MAJOR: &[(&str, f64)] = &[
    // The garage-house variant (engine default; "gospel-influenced
    // piano" [house#38]).
    ("ii7–V7–Imaj7–Imaj7", 0.4),
    ("IVmaj7–iii7–vi7–vi7", 0.35),
    ("Imaj7–IVmaj7–Imaj7–IVmaj7", 0.25),
];

/// Chicago house [house#5][house#6]: four on the floor, snare and clap on
/// 5, 13, 16th hats with quieter evens, open hats on the off-beats, a rim
/// on the evens, a shaker for the tambourine.
const CHICAGO_DRUMS: &[DrumLane] = &[
    lane("kick", "K", &[1, 5, 9, 13], 1.0),
    lane("snare", "C", &[5, 13], 0.85),
    lane("clap", "C", &[5, 13], 1.0),
    DrumLane {
        p: 0.5,
        ..lane("clap", "C", &[16], 0.6)
    },
    DrumLane {
        vel_even: Some(0.45),
        jitter: 0.12,
        ..lane("hat", "H", &[1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, 16], 0.72)
    },
    DrumLane {
        jitter: 0.08,
        ..lane("ohat", "H", &[3, 7, 11, 15], 0.78)
    },
    DrumLane {
        p: 0.5,
        ..lane("rim", "P", &[4, 10, 14], 0.25)
    },
    DrumLane {
        p: 0.7,
        ..lane("shaker", "P", &[3, 7, 11, 15], 0.6)
    },
    DrumLane {
        steps: &[],
        steps2: Some(&[15]),
        p: 0.5,
        ..lane("tomL", "P", &[], 0.6)
    },
];

/// Deep house [house#8]: four on the floor, clap 5, 13, 16th hats with a
/// quiet 10, 909 toms on 3, 9, 11.
const DEEP_DRUMS: &[DrumLane] = &[
    lane("kick", "K", &[1, 5, 9, 13], 1.0),
    lane("clap", "C", &[5, 13], 0.8),
    DrumLane {
        vel_even: Some(0.46),
        jitter: 0.14,
        ..lane("hat", "H", &[1, 2, 4, 5, 6, 8, 9, 12, 13, 14, 16], 0.62)
    },
    DrumLane {
        p: 0.8,
        jitter: 0.12,
        ..lane("hat", "H", &[10], 0.32)
    },
    DrumLane {
        jitter: 0.10,
        ..lane("ohat", "H", &[3, 7, 11, 15], 0.58)
    },
    DrumLane {
        p: 0.45,
        jitter: 0.08,
        ..lane("tomL", "P", &[4, 12], 0.28)
    },
    DrumLane {
        p: 0.5,
        jitter: 0.10,
        ..lane("shaker", "P", &[2, 10], 0.45)
    },
];

/// Chicago's club form and layer table [arrangement#9][arrangement#12]
/// [arrangement#13][arrangement#33] (192 bars ≈ 6:00).
const CHICAGO_FORM: &[Block] = &[
    block("intro", 16, 0.3),
    block("intro", 16, 0.45),
    block("groove", 16, 0.6),
    block("groove", 16, 0.75),
    block("main", 16, 0.8),
    block("breakdown", 24, 0.3),
    block("build", 8, 0.7),
    block("drop", 32, 1.0),
    block("main", 16, 0.9),
    block("outro", 16, 0.6),
    block("outro", 16, 0.4),
];
const CHICAGO_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x x x x x - - x x x f"),
    ("H", "x x v x x - - x x x x"),
    ("C", "- + x x x +89 x x x x -"),
    ("P", "+9 x x x x - - v x x x"),
    ("B", "- f x x x - - x x x -"),
    ("Pd", "- - f x x x x x x - -"),
    ("L", "- - - x x x - x x - -"),
    ("R", "- - - - r77-80 - r - - - -"),
    ("Sn", "- - - - - - r - - - -"),
    ("X", "- - 1 - - - - 1 - - -"),
];

/// Deep house after Âme "Rej" [arrangement#17] (bar counts after 48
/// engine default; 192 bars).
const DEEP_FORM: &[Block] = &[
    block("intro", 16, 0.4),
    block("intro", 16, 0.45),
    block("groove", 16, 0.55),
    block("lift", 8, 0.6),
    block("main", 48, 0.7),
    block("breakdown", 8, 0.45),
    block("main", 48, 0.75),
    block("outro", 16, 0.55),
    block("outro", 16, 0.4),
];
const DEEP_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x x x x x x x x x"),
    ("B", "x x x x x x x x -"),
    ("P", "x v x x x - x x x"),
    ("H", "- - + x x - x x -"),
    ("C", "- + x x x - x x -"),
    ("Atm", "- + x x x - x - -"),
    ("Pd", "- - s x x - x - -"),
    ("L", "- - - - x - x - -"),
    ("R", "- - - r - r - - -"),
    ("X", "- - - - 1 - 1 - -"),
];

const HOUSE_NOTES: &[&str] = &[
    "House began in Chicago in the early 1980s: DJs like Frankie Knuckles at the Warehouse played disco and re-edits over drum machines; Jesse Saunders, Marshall Jefferson and Phuture made it on cheap Roland gear [house#1][house#18].",
    "The four-on-the-floor kick with claps on 2 and 4 has no syncopation of its own; the groove lives in the hats, the shuffle and the bass [cognition#18].",
    "Swing is the MPC's: 50 % straight, 54 % loose, 66 % a triplet feel; house sits at 55–60 % [house#11][house#12].",
    "Minor-seventh vamps of one to three chords carry the harmony, keeping the 7th in every voicing [house#15][house#48].",
    "Tracks are built for DJs: 16 or 32 bars of drums to mix in and out, layers entering every 8 or 16 bars [arrangement#13][arrangement#33].",
    "The 909's hats, ride and crash are 6-bit samples of real cymbals, not the 808's square-wave cluster: the kit's metal voices are requantised to 6 bits, the envelope after the converter [sound-design#5][sound-design#6].",
];

pub const CHICAGO: Profile = Profile {
    key: "chicago",
    name: "Chicago house",
    tempo: (118.0, 126.0, 123.0),
    // Modern 55–60 [house#5][house#6][house#11].
    swing: (54.0, 60.0),
    modes: &[(AEOLIAN, 0.65), (DORIAN, 0.18), (IONIAN, 0.17)],
    minor_progs: HOUSE_MINOR,
    major_progs: HOUSE_MAJOR,
    // min7 default; 9th p = 0.6, 11th p = 0.3 [house#15][house#48].
    ext: (1.0, 0.6, 0.3),
    kit: "tr909",
    drums: CHICAGO_DRUMS,
    bass: BassStyle::HouseCells,
    chords: ChordStyle::Stabs,
    arp: false,
    // A 1–2-bar riff, 3–6 onsets a bar, ≤ 12 st [house#1][house#42].
    lead: (4.0, 12, 0.7),
    bass_range: (29, 40),
    bass_sounds: &[
        "roundBass",
        "organBass",
        "moogBass",
        "pluckBass",
        "dxBass",
        "fmBass",
        "sawBass",
    ],
    pad_sounds: &["stab", "dubChord", "piano", "organ", "modernPluck"],
    arp_sounds: &["pluck", "modernPluck", "pluckTrance"],
    lead_sounds: &["piano", "ep", "organ", "moogLead", "vocalLead", "brassLead"],
    form: CHICAGO_FORM,
    template: CHICAGO_TEMPLATE,
    // DJ intros open on the kick (G6) [arrangement#14][arrangement#16].
    intros: &[],
    levels: &[("Pd", 1.0), ("L", -3.0)],
    // −3 to −4.5 dB [sound-design#29][house#28].
    pump: 0.35,
    tom_fills: false,
    tape: 0.25,
    glue: 0.55,
    reverb: 0.9,
    gated: (0.0, 0.0),
    notes: HOUSE_NOTES,
};

pub const DEEP: Profile = Profile {
    key: "deep",
    name: "Deep house",
    // Beatport median 122 [house#2][house#8][cognition#8].
    tempo: (118.0, 124.0, 122.0),
    swing: (56.0, 64.0),
    // Minor 0.95 (GiantSteps deep house) [cognition#7].
    modes: &[(AEOLIAN, 0.75), (DORIAN, 0.2), (IONIAN, 0.05)],
    drums: DEEP_DRUMS,
    bass: BassStyle::Rolling,
    // m9 "the one main chord" [house#15][house#16].
    ext: (1.0, 0.8, 0.3),
    lead: (2.5, 12, 0.3),
    bass_sounds: &[
        "roundBass",
        "organBass",
        "moogBass",
        "sub",
        "dxBass",
        "fmBass",
        "fingerBass",
    ],
    pad_sounds: &[
        "dubChord",
        "stab",
        "ep",
        "modernPluck",
        "nylonGuitar",
        "acousticGuitar",
    ],
    arp_sounds: &[
        "nylonGuitar",
        "acousticGuitar",
        "modernPluck",
        "pluck",
        "glass",
    ],
    lead_sounds: &[
        "nylonGuitar",
        "acousticGuitar",
        "ep",
        "moogLead",
        "modernPluck",
        "glass",
        "bell",
        "vocalLead",
    ],
    form: DEEP_FORM,
    template: DEEP_TEMPLATE,
    levels: &[("Pd", 0.5), ("L", -3.0)],
    pump: 0.3,
    tape: 0.3,
    glue: 0.45,
    reverb: 1.3,
    ..CHICAGO
};

// ── Orchestral (Mech Works SPEC.md / AUDIO.md) ───────────────────────

const ORCH_MINOR: &[(&str, f64)] = &[
    // Heroic minor with modal mixture, flat-side mediants, and raised V
    // dominant (Lehman 2018, *Hollywood Harmony*; Cohn 2012).
    ("i–♭VI–iv–V", 0.24),
    ("i–VI–♭VII–V", 0.20),
    ("i–iv–♭VI–V", 0.18),
    ("i–III–♭VI–V", 0.16),
    ("i–♭II–iv–V", 0.12),
    ("i–VII–VI–V", 0.10),
];
const ORCH_MAJOR: &[(&str, f64)] = &[
    // Noble Mixolydian, Lydian II, and borrowed flat-side cadences for
    // rebuilding and community (Lehman 2018; Murphy 2006).
    ("I–♭VII–IV–I", 0.28),
    ("I–♭VI–♭VII–I", 0.24),
    ("I–II–IV–I", 0.20),
    ("I–V–vi–IV", 0.16),
    ("I–IV–♭VII–I", 0.12),
];

/// Concert percussion: Gran Cassa on 1 and 9 (with a pickup on 15 in bar 2
/// for the walker's stride), orchestral field snare on 5 and 13 with martial
/// dotted figures, timpani fifths on `tomL` / `tomM`, and gentle cymbal
/// pulses.
const ORCH_DRUMS: &[DrumLane] = &[
    DrumLane {
        steps2: Some(&[1, 9, 15]),
        ..lane("kick", "K", &[1, 9], 0.92)
    },
    DrumLane {
        steps2: Some(&[5, 11, 13, 15]),
        jitter: 0.08,
        ..lane("snare", "C", &[5, 13], 0.78)
    },
    DrumLane {
        steps2: Some(&[1, 7, 9, 13]),
        ..lane("tomL", "P", &[1, 9], 0.85)
    },
    DrumLane {
        p: 0.85,
        ..lane("tomM", "P", &[5, 15], 0.72)
    },
    DrumLane {
        vel_even: Some(0.45),
        jitter: 0.10,
        p: 0.65,
        ..lane("hat", "H", &[1, 5, 9, 13], 0.42)
    },
    DrumLane {
        p: 0.5,
        ..lane("ohat", "H", &[9], 0.45)
    },
];

/// Narrative form across the Mech Works arc (96 bars):
/// prelude → defence (heroic brass leitmotif) → walker march (full tutti
/// with ostinato) → rebuilding (warm strings and horn variation) →
/// community (grand tutti return) → decline (tired, sparse solo statement)
/// → epilogue.
const ORCH_FORM: &[Block] = &[
    block("prelude", 8, 0.35),
    block("defence", 16, 0.85),
    block("walker march", 16, 0.95),
    block("rebuilding", 16, 0.60),
    block("community", 16, 1.00),
    block("decline", 16, 0.30),
    block("epilogue", 8, 0.25),
];
const ORCH_TEMPLATE: &[(&str, &str)] = &[
    ("K", "- x x s x - -"),
    ("C", "- x x - x - -"),
    ("P", "s x x s x - -"),
    ("H", "- - x - x - -"),
    ("B", "s x x x x s -"),
    ("Atm", "x x x x x x x"),
    ("Pd", "x x x x x s f"),
    ("A", "- s x s x - -"),
    ("L", "- x x v x s -"),
    ("Sn", "r7-8 - - r55-56 - - -"),
    ("X", "- 1 1 - 1 - -"),
];

const STRINGS_FIRST: Intro = Intro {
    name: "strings first",
    p: 0.45,
    cells: &[
        ("K", "- x"),
        ("P", "s x"),
        ("Pd", "x x"),
        ("Atm", "x x"),
        ("Sn", "r7-8 -"),
    ],
    note: "It opens on low string fifths and bowed harmonies before a two-bar timpani and snare roll launches the brass leitmotif at bar 9.",
};
const HORN_CALL: Intro = Intro {
    name: "horn call",
    p: 0.35,
    cells: &[
        ("K", "- x"),
        ("P", "- x"),
        ("Atm", "x x"),
        ("Pd", "s x"),
        ("L", "s x"),
        ("Sn", "r7-8 -"),
    ],
    note: "It opens with a solitary horn stating the companion's leitmotif over a low string pedal before the full brass and percussion enter at bar 9.",
};
const WALKER_STRIDE: Intro = Intro {
    name: "walker stride",
    p: 0.20,
    cells: &[
        ("K", "x x"),
        ("P", "x x"),
        ("B", "x x"),
        ("Atm", "x x"),
        ("Pd", "- x"),
        ("Sn", "r7-8 -"),
    ],
    note: "It opens on the concert bass drum, timpani and contrabass alone — the four-tonne walker's stride — before the brass theme enters at bar 9.",
};
const ORCH_INTROS: &[Intro] = &[STRINGS_FIRST, HORN_CALL, WALKER_STRIDE];

const ORCH_NOTES: &[&str] = &[
    "Written in the spirit of a John Williams symphonic score for Mech Works: brassy leitmotifs for the mechs and big walkers over bowed string sections, contrabass and concert percussion (SPEC.md \"Audio\", AUDIO.md).",
    "Harmony uses modal mixture, flat-side chromatic mediants (♭VI, ♭VII, ♭II) and Lydian II cadences, giving the brass fanfares their heroic lift and tragic weight.",
    "The companion mech's leitmotif is stated across the game's arc within the score: heroic in 'defence' and 'walker march', warm and lyrical in 'rebuilding', full tutti in 'community', and finally tired, slow and sparse in 'decline' after the percussion drops away.",
    "No sidechain pump is used (`pump = 0.0`); the ensemble breathes in a large concert hall (`reverb = 1.5`) with tuned timpani and Gran Cassa concert bass drum.",
];

pub const ORCHESTRAL: Profile = Profile {
    key: "orchestral",
    name: "Orchestral",
    tempo: (84.0, 108.0, 96.0),
    swing: (50.0, 50.0),
    modes: &[
        (AEOLIAN, 0.45),
        (DORIAN, 0.25),
        (IONIAN, 0.20),
        (MIXOLYDIAN, 0.10),
    ],
    minor_progs: ORCH_MINOR,
    major_progs: ORCH_MAJOR,
    ext: (0.2, 0.15, 0.0),
    kit: "tr808",
    drums: ORCH_DRUMS,
    bass: BassStyle::Orchestral,
    chords: ChordStyle::Sustain,
    arp: true,
    // Every orchestral piece states its leitmotif (`p = 1.0`), 3.2 notes/bar
    // across a wide 16-semitone heroic compass.
    lead: (3.2, 16, 1.0),
    bass_range: (31, 43),
    bass_sounds: &["contrabass"],
    pad_sounds: &["stringSection", "celloPad", "choir"],
    arp_sounds: &[
        "nylonGuitar",
        "acousticGuitar",
        "stringSection",
        "celloPad",
        "glass",
        "bell",
    ],
    lead_sounds: &["brassSection", "frenchHorn"],
    form: ORCH_FORM,
    template: ORCH_TEMPLATE,
    intros: ORCH_INTROS,
    levels: &[("L", 7.0), ("Pd", 1.5), ("A", 3.5), ("Atm", 1.0)],
    pump: 0.0,
    tom_fills: true,
    tape: 0.2,
    glue: 0.35,
    reverb: 1.5,
    gated: (0.0, 0.0),
    notes: ORCH_NOTES,
};

// ── Acid Techno (RULES.md P8) ────────────────────────────────────────

const TECHNO_MINOR: &[(&str, f64)] = &[
    // [techno#2][techno#18][techno#19][techno#43][techno#44].
    ("i–i–i–i", 0.30),
    ("i–v–i–v", 0.25),
    ("i–VI–III–VII", 0.25),
    ("i–iv–v–III", 0.20),
];
const TECHNO_MAJOR: &[(&str, f64)] = &[("I–V–vi–IV", 0.5), ("I–♭VII–IV–I", 0.5)];

const ACID_DRUMS: &[DrumLane] = &[
    DrumLane {
        steps2: Some(&[1, 5, 9, 13, 15]),
        ..lane("kick", "K", &[1, 5, 9, 13], 1.0)
    },
    lane("clap", "C", &[5, 13], 0.90),
    lane("snare", "C", &[5, 13], 0.80),
    DrumLane {
        vel_even: Some(0.50),
        jitter: 0.12,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            0.70,
        )
    },
    lane("ohat", "H", &[3, 7, 11, 15], 0.82),
    DrumLane {
        p: 0.65,
        ..lane("ride", "P", &[3, 7, 11, 15], 0.55)
    },
    DrumLane {
        p: 0.55,
        ..lane("rim", "P", &[2, 8, 12, 16], 0.45)
    },
];

const ACID_FORM: &[Block] = &[
    block("intro", 16, 0.35),
    block("groove", 16, 0.55),
    block("main", 32, 0.80),
    block("breakdown", 16, 0.45),
    block("build", 8, 0.75),
    block("drop", 32, 1.00),
    block("outro", 16, 0.45),
];
const ACID_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x x x - - x x"),
    ("H", "x x x s x x x"),
    ("C", "- x x - - x -"),
    ("P", "- + x - x v x"),
    ("B", "f x x x x v f"),
    ("Pd", "- - x x x x -"),
    ("L", "- - s - - x -"),
    ("Sn", "- - - - r - -"),
    ("R", "- - - - r - -"),
    ("X", "- 1 1 - - 1 -"),
];

const ACID_NOTES: &[&str] = &[
    "Acid house and acid techno centre on the Roland TB-303 bassline: a 16th-note riff with normal notes gated to 7/12 of a step, tied slides, and accented hits that charge the C13 sweep capacitor [techno#7][techno#8][sound-design#9].",
    "The motion is in the knobs: filter cutoff, resonance and envelope decay sweep across breakdowns, builds and drops while the 16th-note pitch pattern stays locked [techno#5][techno#9].",
    "Driven by a 909 four-on-the-floor kick, off-beat open hats and ride cymbals, and rave/chord stabs (`hoover`, `stab`, `dubChord`) [techno#11][sound-design#26].",
];

pub const ACID: Profile = Profile {
    key: "acid",
    name: "Acid techno",
    tempo: (126.0, 138.0, 132.0),
    swing: (50.0, 53.0),
    modes: &[(AEOLIAN, 0.75), (PHRYGIAN, 0.15), (DORIAN, 0.10)],
    minor_progs: TECHNO_MINOR,
    major_progs: TECHNO_MAJOR,
    ext: (0.4, 0.2, 0.0),
    kit: "tr909",
    drums: ACID_DRUMS,
    bass: BassStyle::Acid303,
    chords: ChordStyle::Stabs,
    arp: false,
    lead: (3.2, 12, 0.65),
    bass_range: (36, 46),
    bass_sounds: &["acid"],
    pad_sounds: &["stab", "dubChord", "hoover", "modernPluck"],
    arp_sounds: &["sqArp", "modernPluck"],
    lead_sounds: &["hoover", "syncLead", "moogLead", "sawLead"],
    form: ACID_FORM,
    template: ACID_TEMPLATE,
    intros: &[],
    levels: &[("B", 2.5), ("Pd", 1.5), ("L", 0.5)],
    pump: 0.30,
    tom_fills: false,
    tape: 0.35,
    glue: 0.65,
    reverb: 0.95,
    gated: (0.0, 0.0),
    notes: ACID_NOTES,
};

// ── Detroit / Melodic Techno (RULES.md P5, P10) ──────────────────────

const DETROIT_DRUMS: &[DrumLane] = &[
    lane("kick", "K", &[1, 5, 9, 13], 1.0),
    lane("snare", "C", &[5, 13], 0.90),
    lane("clap", "C", &[5, 13], 0.85),
    DrumLane {
        vel_even: Some(0.50),
        jitter: 0.10,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            0.68,
        )
    },
    lane("ohat", "H", &[3, 7, 11, 15], 0.80),
    DrumLane {
        p: 0.70,
        ..lane("shaker", "P", &[1, 3, 5, 7, 9, 11, 13, 15], 0.50)
    },
    DrumLane {
        p: 0.60,
        ..lane("rim", "P", &[4, 8, 11, 15], 0.45)
    },
];

const DETROIT_FORM: &[Block] = &[
    block("intro", 16, 0.35),
    block("groove", 16, 0.60),
    block("main", 32, 0.85),
    block("breakdown", 16, 0.35),
    block("build", 8, 0.70),
    block("drop", 32, 1.00),
    block("outro", 16, 0.45),
];
const DETROIT_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x x x - - x x"),
    ("H", "- x x - x x x"),
    ("C", "- x x - - x -"),
    ("P", "+9 x x - x x -"),
    ("B", "- x x - f x -"),
    ("Pd", "f x x x x x f"),
    ("A", "- + x s x x -"),
    ("L", "- - x s - x -"),
    ("Sn", "- - - - r - -"),
    ("R", "- - - - r - -"),
    ("X", "- - 1 - - 1 -"),
];

const DETROIT_NOTES: &[&str] = &[
    "Detroit and melodic techno pair machine-funk 909 drums with warm analogue Moog/Reese basslines, minor-7th chord stabs, and hypnotic arpeggios (Belleville Three, Underground Resistance, Tale Of Us, Stephan Bodzin) [techno#11][techno#18][techno#43].",
    "Filter cutoff opens across 16-bar phrases and builds, while minor-key arpeggios and leads carry the emotional tension over the driving 4/4 pulse [techno#44].",
];

pub const DETROIT: Profile = Profile {
    key: "detroit",
    name: "Detroit techno",
    tempo: (120.0, 128.0, 124.0),
    swing: (52.0, 57.0),
    modes: &[(AEOLIAN, 0.85), (DORIAN, 0.15)],
    minor_progs: TECHNO_MINOR,
    major_progs: TECHNO_MAJOR,
    ext: (0.8, 0.4, 0.2),
    kit: "tr909",
    drums: DETROIT_DRUMS,
    bass: BassStyle::Rolling,
    chords: ChordStyle::Stabs,
    arp: true,
    lead: (3.0, 12, 0.80),
    bass_range: (31, 43),
    bass_sounds: &["moogBass", "reese", "darkBass", "sawBass"],
    pad_sounds: &["stab", "piano", "dubChord", "superPad"],
    arp_sounds: &["pluck", "modernPluck", "pluckTrance"],
    lead_sounds: &["moogLead", "syncLead", "sawLead", "supersaw", "brassLead"],
    form: DETROIT_FORM,
    template: DETROIT_TEMPLATE,
    intros: &[],
    levels: &[("A", 3.5), ("Pd", 0.5), ("L", 1.5)],
    pump: 0.30,
    tom_fills: false,
    tape: 0.25,
    glue: 0.60,
    reverb: 1.15,
    gated: (0.0, 0.0),
    notes: DETROIT_NOTES,
};

// ── Uplifting Trance (RULES.md P14) ──────────────────────────────────

const TRANCE_MINOR: &[(&str, f64)] = &[
    // [trance#29]–[trance#33][trance#45].
    ("i–VI–III–VII", 0.24),
    ("i–VI–VII–III", 0.18),
    ("VI–VII–i–i", 0.18),
    ("VI–iv–i–i", 0.14),
    ("i–v–VII–IV", 0.14),
    ("i–iv–III–VII", 0.12),
];
const TRANCE_MAJOR: &[(&str, f64)] = &[
    ("I–V–vi–IV", 0.45),
    ("IV–V–vi–I", 0.35),
    ("I–vi–IV–V", 0.20),
];

const TRANCE_DRUMS: &[DrumLane] = &[
    lane("kick", "K", &[1, 5, 9, 13], 1.0),
    lane("clap", "C", &[5, 13], 0.95),
    lane("snare", "C", &[5, 13], 0.85),
    DrumLane {
        vel_even: Some(0.48),
        jitter: 0.08,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            0.62,
        )
    },
    lane("ohat", "H", &[3, 7, 11, 15], 0.82),
    DrumLane {
        p: 0.60,
        ..lane("ride", "P", &[3, 7, 11, 15], 0.55)
    },
];

const TRANCE_FORM: &[Block] = &[
    block("intro", 16, 0.35),
    block("groove", 16, 0.55),
    block("main", 32, 0.85),
    block("breakdown", 24, 0.35),
    block("build", 8, 0.75),
    block("drop", 32, 1.00),
    block("outro", 16, 0.45),
];
const TRANCE_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x x x - - x x"),
    ("H", "+9 x x - - x x"),
    ("C", "- x x - - x -"),
    ("P", "- - x - - x -"),
    ("B", "- x x - f x -"),
    ("Pd", "- f x x x x -"),
    ("A", "- + x s x x -"),
    ("L", "- - s x x x -"),
    ("Sn", "- - - - r - -"),
    ("R", "- - r61-64 - r - -"),
    ("X", "- - 1 - - 1 -"),
];

const TRANCE_NOTES: &[&str] = &[
    "Uplifting trance builds around a central 8-bar supersaw hook over minor progressions weighted toward VI, III and VII (Paul van Dyk, Armin van Buuren, System F, Gouryella) [trance#1][trance#17][trance#29].",
    "The bass rolls on the 16ths between the four-on-the-floor kicks with strong off-beat 8ths on steps 3, 7, 11 and 15, ducked by the sidechain pump [trance#1][trance#3][trance#15].",
    "In the breakdown the drums drop away and the pads and hook open before an accelerating snare roll and riser launch the anthem drop [trance#28][arrangement#22].",
];

pub const UPLIFTING: Profile = Profile {
    key: "uplifting",
    name: "Uplifting trance",
    tempo: (134.0, 140.0, 138.0),
    swing: (50.0, 50.0),
    modes: &[(AEOLIAN, 0.85), (IONIAN, 0.15)],
    minor_progs: TRANCE_MINOR,
    major_progs: TRANCE_MAJOR,
    ext: (0.35, 0.25, 0.0),
    kit: "tr909",
    drums: TRANCE_DRUMS,
    bass: BassStyle::Offbeat16,
    chords: ChordStyle::Sustain,
    arp: true,
    lead: (3.6, 12, 0.95),
    bass_range: (33, 45),
    bass_sounds: &["tranceBass", "moogBass", "sawBass", "psyBass"],
    pad_sounds: &["superPadWide", "superPad", "warmPad", "choir"],
    arp_sounds: &["pluckTrance", "modernPluck", "sqArp"],
    lead_sounds: &["supersaw", "syncLead", "euroLead", "sawLead"],
    form: TRANCE_FORM,
    template: TRANCE_TEMPLATE,
    intros: &[],
    levels: &[("A", 4.5), ("Pd", 3.0), ("L", 2.0)],
    pump: 0.42,
    tom_fills: false,
    tape: 0.15,
    glue: 0.55,
    reverb: 1.35,
    gated: (0.0, 0.0),
    notes: TRANCE_NOTES,
};

// ── French / Filter House (RULES.md P3) ──────────────────────────────

const FRENCH_FORM: &[Block] = &[
    block("intro", 16, 0.35),
    block("groove", 16, 0.55),
    block("main", 32, 0.90),
    block("breakdown", 16, 0.40),
    block("build", 8, 0.70),
    block("drop", 32, 1.00),
    block("outro", 16, 0.45),
];
const FRENCH_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x x x - - x x"),
    ("H", "x x x - x x x"),
    ("C", "- x x - - x -"),
    ("P", "- x x - - x x"),
    ("B", "f f x f f x -"),
    ("Pd", "f f x f f x f"),
    ("L", "- - x - - x -"),
    ("Sn", "- - - - r - -"),
    ("R", "- r29-32 - - r - -"),
    ("X", "- - 1 - - 1 -"),
];

const FRENCH_NOTES: &[&str] = &[
    "French touch / filter house (Daft Punk, Stardust, Cassius, Roulé) runs Rhodes/disco chords and a syncopated Moog/octave bass through sweeping low-pass filters and heavy kick-keyed sidechain pumping [house#3][house#27][house#28].",
    "The filter sweep is the primary structural motion: the loop opens from a muffled 400 Hz intro into the full-spectrum drop, dips in the breakdown, and opens again for the second peak [house#3].",
];

pub const FRENCH: Profile = Profile {
    key: "french",
    name: "French house",
    tempo: (118.0, 128.0, 124.0),
    swing: (53.0, 58.0),
    modes: &[(DORIAN, 0.45), (AEOLIAN, 0.35), (MIXOLYDIAN, 0.20)],
    minor_progs: HOUSE_MINOR,
    major_progs: HOUSE_MAJOR,
    ext: (1.0, 0.65, 0.25),
    kit: "tr909",
    drums: CHICAGO_DRUMS,
    bass: BassStyle::HouseCells,
    chords: ChordStyle::Stabs,
    arp: false,
    lead: (3.5, 10, 0.55),
    bass_range: (31, 43),
    bass_sounds: &[
        "fingerBass",
        "moogBass",
        "dxBass",
        "organBass",
        "roundBass",
        "pluckBass",
    ],
    pad_sounds: &[
        "ep",
        "rhythmGuitar",
        "acousticGuitar",
        "stab",
        "piano",
        "organ",
        "dubChord",
    ],
    arp_sounds: &["nylonGuitar", "acousticGuitar", "modernPluck", "pluck"],
    lead_sounds: &[
        "nylonGuitar",
        "leadGuitar",
        "ep",
        "syncLead",
        "moogLead",
        "brassLead",
    ],
    form: FRENCH_FORM,
    template: FRENCH_TEMPLATE,
    intros: &[],
    levels: &[("Pd", 0.0), ("L", -2.5)],
    pump: 0.52,
    tom_fills: false,
    tape: 0.30,
    glue: 0.65,
    reverb: 0.85,
    gated: (0.0, 0.0),
    notes: FRENCH_NOTES,
};

// ── Dubstep (RULES.md P24) ───────────────────────────────────────────

const DUBSTEP_MINOR: &[(&str, f64)] = &[
    // [ambient#38][cognition#7].
    ("i–i–i–i", 0.35),
    ("i–VI–i–VI", 0.30),
    ("i–iv–i–v", 0.35),
];

/// 140 BPM half-time dubstep beat: kick on 1 and 13 plus a lazy off-beat kick
/// on 8, heavy snare/clap on step 9 (beat 3), and swung hats [ambient#38][ambient#39].
const DUBSTEP_DRUMS: &[DrumLane] = &[
    DrumLane {
        steps2: Some(&[1, 8, 12]),
        ..lane("kick", "K", &[1, 8, 13], 1.0)
    },
    lane("snare", "C", &[9], 1.0),
    lane("clap", "C", &[9], 0.85),
    DrumLane {
        vel_even: Some(0.45),
        jitter: 0.14,
        ..lane("hat", "H", &[1, 3, 5, 7, 9, 11, 13, 15], 0.65)
    },
    lane("ohat", "H", &[7, 15], 0.65),
    DrumLane {
        p: 0.55,
        ..lane("rim", "P", &[4, 12, 16], 0.45)
    },
];

const DUBSTEP_FORM: &[Block] = &[
    block("intro", 16, 0.30),
    block("build", 8, 0.65),
    block("drop", 32, 1.00),
    block("breakdown", 16, 0.35),
    block("build", 8, 0.75),
    block("drop", 32, 1.00),
    block("outro", 16, 0.35),
];
const DUBSTEP_TEMPLATE: &[(&str, &str)] = &[
    ("K", "s x x - x x -"),
    ("C", "s x x - x x s"),
    ("H", "- x x s x x s"),
    ("P", "x x x - x x x"),
    ("B", "- f x - f v -"),
    ("Atm", "x x - x x - x"),
    ("Pd", "x f x x f x x"),
    ("L", "- - x s - x -"),
    ("Sn", "- r - - r - -"),
    ("R", "- r - - r - -"),
    ("X", "- - 1 - - 1 -"),
];

const DUBSTEP_NOTES: &[&str] = &[
    "Early UK dubstep (Skream, Benga, DMZ, Burial) runs at 140 BPM but feels like 70 BPM half-time, with the snare cracking on beat 3 (step 9) and swung triplet-feel hats [ambient#38][ambient#39].",
    "The bass pairs deep sub weight with an LFO-modulated `wobble` / `reese` mid-bass whose LFO sweeps filter cutoff across the half-time groove [sound-design#43].",
];

pub const DUBSTEP: Profile = Profile {
    key: "dubstep",
    name: "Dubstep",
    tempo: (136.0, 142.0, 140.0),
    swing: (60.0, 66.0),
    modes: &[(AEOLIAN, 0.70), (PHRYGIAN, 0.20), (DORIAN, 0.10)],
    minor_progs: DUBSTEP_MINOR,
    major_progs: TECHNO_MAJOR,
    ext: (0.3, 0.1, 0.0),
    kit: "tr808",
    drums: DUBSTEP_DRUMS,
    bass: BassStyle::DubstepWobble,
    chords: ChordStyle::Stabs,
    arp: false,
    lead: (2.8, 12, 0.65),
    bass_range: (29, 39),
    bass_sounds: &["wobble", "reese", "sub"],
    pad_sounds: &["dubChord", "stab", "modernPluck"],
    arp_sounds: &["glass", "bell"],
    lead_sounds: &["hoover", "vocalLead", "syncLead", "bell"],
    form: DUBSTEP_FORM,
    template: DUBSTEP_TEMPLATE,
    intros: &[],
    levels: &[("B", 1.0), ("Pd", 3.5), ("L", 1.0)],
    pump: 0.20,
    tom_fills: false,
    tape: 0.35,
    glue: 0.60,
    reverb: 1.25,
    gated: (0.0, 0.0),
    notes: DUBSTEP_NOTES,
};

// ── Liquid / Roller Drum & Bass (RULES.md P12, P13) ──────────────────

/// Two-step DnB break skeleton at 174 BPM: kick on 1 and 11, snare on 5 and 13,
/// ghost snares on 8 and 10, 8th rides and 16th hats [dnb#9][dnb#11][dnb#12].
const LIQUID_DRUMS: &[DrumLane] = &[
    DrumLane {
        steps2: Some(&[1, 11, 16]),
        ..lane("kick", "K", &[1, 11], 1.0)
    },
    lane("snare", "C", &[5, 13], 1.0),
    DrumLane {
        p: 0.85,
        ..lane("snare", "P", &[8, 10], 0.32)
    },
    DrumLane {
        vel_even: Some(0.52),
        jitter: 0.10,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            0.62,
        )
    },
    lane("ride", "H", &[1, 3, 5, 7, 9, 11, 13, 15], 0.50),
    DrumLane {
        p: 0.65,
        ..lane("shaker", "P", &[3, 7, 11, 15], 0.48)
    },
];

const LIQUID_FORM: &[Block] = &[
    block("intro", 16, 0.40),
    block("build", 8, 0.60),
    block("drop", 32, 0.95),
    block("breakdown", 16, 0.40),
    block("build", 8, 0.70),
    block("drop", 32, 1.00),
    block("outro", 16, 0.45),
];
const LIQUID_TEMPLATE: &[(&str, &str)] = &[
    ("K", "x f x - f x x"),
    ("C", "x f x - f x -"),
    ("H", "+9 x x - x x x"),
    ("P", "- + x - - x -"),
    ("B", "- - x s f v -"),
    ("Atm", "x x - x x - x"),
    ("Pd", "x x x x x x f"),
    ("A", "s x x s x x -"),
    ("L", "- - x s - x -"),
    ("Sn", "- r - - r - -"),
    ("R", "- r - - r - -"),
    ("X", "- - 1 - - 1 -"),
];

const LIQUID_NOTES: &[&str] = &[
    "Liquid and roller Drum & Bass runs at 170–176 BPM (typical 174) over the classic two-step breakbeat skeleton: kicks on steps 1 and 11 (the 8th before the second snare), backbeat snares on 5 and 13, and ghost snares on 8 and 10 [dnb#1][dnb#11][dnb#12].",
    "Beneath lush Rhodes/pad progressions (`ep`, `warmPad`, `superPad`), a sustained Reese or sub bass locks to the two-step kick with phrase-end slides and edits [dnb#11][dnb#17][dnb#21].",
];

pub const LIQUID: Profile = Profile {
    key: "liquid",
    name: "Liquid DnB",
    tempo: (170.0, 176.0, 174.0),
    swing: (51.0, 55.0),
    modes: &[(AEOLIAN, 0.75), (DORIAN, 0.20), (IONIAN, 0.05)],
    minor_progs: HOUSE_MINOR,
    major_progs: HOUSE_MAJOR,
    ext: (1.0, 0.70, 0.30),
    kit: "tr909",
    drums: LIQUID_DRUMS,
    bass: BassStyle::DnbRoller,
    chords: ChordStyle::Sustain,
    arp: true,
    lead: (2.6, 12, 0.75),
    bass_range: (29, 40),
    bass_sounds: &["reese", "sub", "wobble", "roundBass", "fingerBass"],
    pad_sounds: &["warmPad", "superPad", "ep", "darkPad"],
    arp_sounds: &[
        "nylonGuitar",
        "acousticGuitar",
        "ep",
        "glass",
        "pluck",
        "modernPluck",
    ],
    lead_sounds: &[
        "nylonGuitar",
        "acousticGuitar",
        "ep",
        "vocalLead",
        "moogLead",
        "flute",
        "glass",
    ],
    form: LIQUID_FORM,
    template: LIQUID_TEMPLATE,
    intros: &[],
    levels: &[("A", 3.5), ("Pd", 2.5), ("L", 2.5)],
    pump: 0.20,
    tom_fills: false,
    tape: 0.25,
    glue: 0.55,
    reverb: 1.25,
    gated: (0.0, 0.0),
    notes: LIQUID_NOTES,
};

// ── Acoustic & Nylon Guitar (Spanish / Fingerstyle / Folk) ───────────

const ACOUSTIC_MINOR: &[(&str, f64)] = &[
    ("i–VII–VI–V", 0.28),
    ("i–VI–III–VII", 0.22),
    ("i7–iv7–VII–III", 0.18),
    ("i–iv–v–i", 0.16),
    ("VI–VII–i–i", 0.16),
];
const ACOUSTIC_MAJOR: &[(&str, f64)] = &[
    ("I–V–vi–IV", 0.32),
    ("I–IV–I–V", 0.26),
    ("vi–IV–I–V", 0.24),
    ("I–♭VII–IV–I", 0.18),
];

const ACOUSTIC_DRUMS: &[DrumLane] = &[
    DrumLane {
        steps2: Some(&[1, 7, 9, 15]),
        ..lane("kick", "K", &[1, 9], 0.85)
    },
    lane("sidestick", "C", &[5, 13], 0.78),
    DrumLane {
        p: 0.55,
        ..lane("snare", "C", &[5, 13], 0.55)
    },
    DrumLane {
        vel_even: Some(0.50),
        jitter: 0.14,
        ..lane("brush", "H", &[1, 3, 5, 7, 9, 11, 13, 15], 0.48)
    },
    DrumLane {
        p: 0.75,
        jitter: 0.12,
        ..lane("shaker", "P", &[3, 7, 11, 15], 0.42)
    },
    DrumLane {
        p: 0.50,
        vel_even: Some(0.45),
        jitter: 0.12,
        ..lane("hat", "H", &[1, 5, 9, 13], 0.40)
    },
];

const ACOUSTIC_FORM: &[Block] = &[
    block("intro", 8, 0.35),
    block("verse", 16, 0.55),
    block("chorus", 16, 0.85),
    block("verse", 16, 0.65),
    block("middle eight", 8, 0.50),
    block("solo", 16, 0.95),
    block("chorus", 16, 1.00),
    block("outro", 8, 0.40),
];
const ACOUSTIC_TEMPLATE: &[(&str, &str)] = &[
    ("K", "- x x x - x x -"),
    ("C", "- x x x - x x -"),
    ("H", "s x x x s x x s"),
    ("P", "- + x x - x x -"),
    ("B", "s x x x s x v s"),
    ("Pd", "s x x x x x x s"),
    ("A", "x x x x x v x s"),
    ("L", "- s x s x v x s"),
    ("Sn", "- - - - r - - -"),
    ("X", "- - 1 - - 1 1 -"),
];

const GUITAR_SOLO_INTRO: Intro = Intro {
    name: "solo guitar",
    p: 0.60,
    cells: &[
        ("K", "- x"),
        ("C", "- x"),
        ("B", "- x"),
        ("Pd", "s x"),
        ("A", "x x"),
        ("L", "s s"),
    ],
    note: "It opens with fingerpicked nylon and steel-string acoustic guitar alone before the upright bass and brushed percussion enter at bar 9.",
};
const DUET_INTRO: Intro = Intro {
    name: "bass and guitar duet",
    p: 0.40,
    cells: &[("K", "- x"), ("B", "s x"), ("Pd", "x x"), ("A", "x x")],
    note: "It opens as an intimate duet between acoustic guitar and upright/fingerstyle bass before the rhythm section joins at bar 9.",
};
const ACOUSTIC_INTROS: &[Intro] = &[GUITAR_SOLO_INTRO, DUET_INTRO];

const ACOUSTIC_NOTES: &[&str] = &[
    "Centred on physical-modelled nylon-string (`nylonGuitar`) and phosphor-bronze steel-string (`acousticGuitar`) guitars with 3-mode wood body resonators, bridge knock, fret/finger transients, and per-note human pluck-position and pick-angle variation.",
    "Chord strums alternate downstrokes and upstrokes with an accelerating human strum curve, while the arpeggio layer weaves fingerstyle patterns with overlapping `let ring` sustain.",
    "Backed by upright or fingerstyle bass and brushed acoustic percussion (`country` kit) in a warm room (`reverb = 1.15`, gentle `pump = 0.08`).",
];

pub const ACOUSTIC: Profile = Profile {
    key: "acoustic",
    name: "Acoustic & nylon",
    tempo: (92.0, 116.0, 104.0),
    swing: (52.0, 57.0),
    modes: &[
        (AEOLIAN, 0.45),
        (DORIAN, 0.25),
        (IONIAN, 0.20),
        (MIXOLYDIAN, 0.10),
    ],
    minor_progs: ACOUSTIC_MINOR,
    major_progs: ACOUSTIC_MAJOR,
    ext: (0.45, 0.25, 0.0),
    kit: "country",
    drums: ACOUSTIC_DRUMS,
    bass: BassStyle::HouseCells,
    chords: ChordStyle::Stabs,
    arp: true,
    lead: (3.2, 14, 1.0),
    bass_range: (33, 45),
    bass_sounds: &["uprightBass", "fingerBass"],
    pad_sounds: &["acousticGuitar", "nylonGuitar"],
    arp_sounds: &["nylonGuitar", "acousticGuitar"],
    lead_sounds: &[
        "nylonGuitar",
        "acousticGuitar",
        "flute",
        "fiddle",
        "pedalSteel",
    ],
    form: ACOUSTIC_FORM,
    template: ACOUSTIC_TEMPLATE,
    intros: ACOUSTIC_INTROS,
    levels: &[("A", 5.5), ("Pd", 3.5), ("L", 4.5)],
    pump: 0.06,
    tom_fills: true,
    tape: 0.20,
    glue: 0.38,
    reverb: 1.15,
    gated: (0.0, 0.0),
    notes: ACOUSTIC_NOTES,
};

/// What the generator changes in a kit, per lane, as the research has it:
/// the 909's hats, ride and crash are 6-bit samples read at about 30 kHz,
/// with a low-pass after the VCA, and a denser source than the 808's six
/// squares (sound-design.md §1.2, §6) [sound-design#5][sound-design#6];
/// synthwave's 80s kit (`tr808`) tightens the kick and snare into a
/// LinnDrum / 707 punch, adds Simmons-style pitch-bend toms, and 8-bit
/// sampled cymbals (synthwave.md §3.1, §3.2).
pub fn kit_tweak(kit: &str) -> Vec<(String, Vec<(String, f64)>)> {
    let lane = |name: &str, kv: &[(&str, f64)]| -> (String, Vec<(String, f64)>) {
        (
            name.to_owned(),
            kv.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
        )
    };
    match kit {
        "tr808" => vec![
            lane(
                "kick",
                &[
                    ("tune", 52.0),
                    ("decay", 0.32),
                    ("pitch", 2.6),
                    ("pt", 0.012),
                    ("click", 0.45),
                    ("drive", 1.5),
                    ("level", 0.95),
                ],
            ),
            lane(
                "snare",
                &[
                    ("tune", 165.0),
                    ("decay", 0.20),
                    ("snappy", 0.22),
                    ("tone", 0.58),
                    ("nhp", 520.0),
                    ("nlp", 6500.0),
                    ("drive", 1.3),
                    ("gate", 0.18),
                    ("level", 0.78),
                ],
            ),
            lane(
                "clap",
                &[
                    ("decay", 0.20),
                    ("freq", 1150.0),
                    ("q", 2.0),
                    ("level", 0.45),
                ],
            ),
            lane(
                "hat",
                &[
                    ("bits", 8.0),
                    ("clock", 28000.0),
                    ("dacLp", 11000.0),
                    ("noise", 0.65),
                    ("hp", 5500.0),
                    ("decay", 0.05),
                    ("level", 0.42),
                ],
            ),
            lane(
                "ohat",
                &[
                    ("bits", 8.0),
                    ("clock", 28000.0),
                    ("dacLp", 11000.0),
                    ("noise", 0.65),
                    ("hp", 5000.0),
                    ("decay", 0.26),
                    ("level", 0.38),
                ],
            ),
            lane(
                "crash",
                &[
                    ("bits", 8.0),
                    ("clock", 28000.0),
                    ("dacLp", 11000.0),
                    ("noise", 0.65),
                    ("hp", 3200.0),
                    ("decay", 1.5),
                    ("level", 0.35),
                ],
            ),
            lane(
                "tomH",
                &[
                    ("tune", 165.0),
                    ("decay", 0.26),
                    ("pitch", 2.1),
                    ("pt", 0.04),
                    ("click", 0.32),
                    ("drive", 1.35),
                    ("noise", 0.06),
                    ("level", 0.65),
                ],
            ),
            lane(
                "tomM",
                &[
                    ("tune", 125.0),
                    ("decay", 0.30),
                    ("pitch", 2.1),
                    ("pt", 0.045),
                    ("click", 0.32),
                    ("drive", 1.35),
                    ("noise", 0.06),
                    ("level", 0.65),
                ],
            ),
            lane(
                "tomL",
                &[
                    ("tune", 92.0),
                    ("decay", 0.34),
                    ("pitch", 2.1),
                    ("pt", 0.05),
                    ("click", 0.32),
                    ("drive", 1.35),
                    ("noise", 0.06),
                    ("level", 0.65),
                ],
            ),
        ],
        "tr909" => vec![
            lane(
                "hat",
                &[
                    ("bits", 6.0),
                    ("clock", 30000.0),
                    ("dacLp", 12000.0),
                    ("hp", 9200.0),
                    ("noise", 0.90),
                ],
            ),
            lane(
                "ohat",
                &[
                    ("bits", 6.0),
                    ("clock", 30000.0),
                    ("dacLp", 12000.0),
                    ("hp", 8400.0),
                    ("noise", 0.90),
                ],
            ),
            lane(
                "ride",
                &[
                    ("bits", 6.0),
                    ("clock", 30000.0),
                    ("dacLp", 12000.0),
                    ("hp", 6200.0),
                    ("noise", 0.86),
                ],
            ),
            lane(
                "crash",
                &[
                    ("bits", 6.0),
                    ("clock", 30000.0),
                    ("dacLp", 12000.0),
                    ("hp", 5400.0),
                    ("noise", 0.86),
                ],
            ),
        ],
        "orchestral" => vec![
            // Gran Cassa (concert bass drum): deep, warm felt-mallet resonance.
            lane(
                "kick",
                &[
                    ("tune", 44.0),
                    ("decay", 0.62),
                    ("pitch", 1.38),
                    ("pt", 0.024),
                    ("click", 0.14),
                    ("drive", 1.15),
                    ("lvl", 0.78),
                ],
            ),
            // Symphonic field / concert snare.
            lane(
                "snare",
                &[
                    ("tune", 178.0),
                    ("decay", 0.24),
                    ("snappy", 0.30),
                    ("tone", 0.62),
                    ("nhp", 480.0),
                    ("nlp", 7200.0),
                    ("drive", 1.15),
                    ("lvl", 0.58),
                ],
            ),
            // Tuned orchestral timpani (kettle drums) with long tonal ring.
            lane(
                "tomL",
                &[
                    ("tune", 73.4),
                    ("decay", 0.65),
                    ("pitch", 1.015),
                    ("pt", 0.028),
                    ("click", 0.22),
                    ("drive", 1.2),
                    ("noise", 0.02),
                    ("lvl", 0.72),
                ],
            ),
            lane(
                "tomM",
                &[
                    ("tune", 98.0),
                    ("decay", 0.58),
                    ("pitch", 1.015),
                    ("pt", 0.025),
                    ("click", 0.22),
                    ("drive", 1.2),
                    ("noise", 0.02),
                    ("lvl", 0.68),
                ],
            ),
            lane(
                "tomH",
                &[
                    ("tune", 110.0),
                    ("decay", 0.54),
                    ("pitch", 1.015),
                    ("pt", 0.022),
                    ("click", 0.22),
                    ("drive", 1.2),
                    ("noise", 0.02),
                    ("lvl", 0.65),
                ],
            ),
            // Piatti crash and suspended cymbal pulses.
            lane(
                "crash",
                &[
                    ("noise", 0.94),
                    ("hp", 4600.0),
                    ("attack", 0.012),
                    ("decay", 2.2),
                    ("lvl", 0.42),
                ],
            ),
            lane(
                "hat",
                &[
                    ("noise", 0.96),
                    ("hp", 6800.0),
                    ("attack", 0.008),
                    ("decay", 0.14),
                    ("lvl", 0.30),
                ],
            ),
            lane(
                "ohat",
                &[
                    ("noise", 0.96),
                    ("hp", 6200.0),
                    ("attack", 0.015),
                    ("decay", 0.48),
                    ("lvl", 0.32),
                ],
            ),
        ],
        "country" => vec![
            lane(
                "kick",
                &[
                    ("tune", 56.0),
                    ("decay", 0.32),
                    ("pitch", 1.9),
                    ("pt", 0.024),
                    ("click", 0.18),
                    ("drive", 1.1),
                    ("lvl", 0.56),
                ],
            ),
            lane(
                "sidestick",
                &[
                    ("tune", 360.0),
                    ("decay", 0.048),
                    ("tone", 0.65),
                    ("snappy", 0.14),
                    ("lvl", 0.44),
                ],
            ),
            lane(
                "snare",
                &[
                    ("tune", 195.0),
                    ("decay", 0.15),
                    ("snappy", 0.36),
                    ("tone", 0.52),
                    ("nlp", 5400.0),
                    ("lvl", 0.36),
                ],
            ),
            lane(
                "brush",
                &[
                    ("decay", 0.14),
                    ("snappy", 0.88),
                    ("tone", 0.08),
                    ("nhp", 1600.0),
                    ("nlp", 5200.0),
                    ("lvl", 0.20),
                ],
            ),
            lane(
                "shaker",
                &[
                    ("hp", 5200.0),
                    ("lp", 9200.0),
                    ("attack", 0.008),
                    ("decay", 0.045),
                    ("lvl", 0.16),
                ],
            ),
            lane(
                "hat",
                &[
                    ("hp", 5800.0),
                    ("lp", 10500.0),
                    ("decay", 0.045),
                    ("lvl", 0.18),
                ],
            ),
            lane("crash", &[("hp", 4200.0), ("decay", 1.3), ("lvl", 0.24)]),
        ],
        _ => Vec::new(),
    }
}

/// Loads a base patch from [`crate::patches::bpatch`] and sets its `name`
/// and measured Juno chorus parameters (sound-design.md §6).
pub fn sound_patch(genre: &str, name: &str) -> Option<Lab> {
    let mut lab = bpatch(name)?;
    lab.name = Some(name.to_owned());
    if matches!(name, "acousticGuitar" | "nylonGuitar")
        && !matches!(genre, "acoustic" | "orchestral")
    {
        lab.gain = lab.gain.map(|g| g * 0.68);
    }
    // The Juno chorus at its measured depth: ±0.5 ms (I), ±1.1 ms (II),
    // through 15 kHz (sound-design.md §6).
    match lab.chorus {
        Some(1.0) => lab.chorus_ms = Some(0.5),
        Some(2.0) => lab.chorus_ms = Some(1.1),
        _ => {}
    }
    if lab.chorus.is_some_and(|c| c > 0.0) {
        lab.chorus_lp = Some(15000.0);
    }
    Some(lab)
}

/// The new generator's genres, in the lab's order.
pub const PROFILES: &[Profile] = &[
    SYNTHWAVE, OUTRUN, CHICAGO, DEEP, FRENCH, ACID, DETROIT, UPLIFTING, LIQUID, DUBSTEP, ACOUSTIC,
    ORCHESTRAL,
];

pub fn profile(key: &str) -> Option<&'static Profile> {
    PROFILES.iter().find(|p| p.key == key)
}
