//! Genre profiles: research/RULES.md's profiles as data. Every number
//! carries its citation (`[file#n]` is reference n of `research/…/file.md`)
//! or is marked an engine default; `notes` are the short, sourced reasons
//! the lab shows beside a song.
//!
//! Steps are written 1–16 as in RULES.md (1 is the downbeat) and turned
//! 0-based by the writers.

use super::theory::{AEOLIAN, DORIAN, IONIAN, MIXOLYDIAN, Mode, PHRYGIAN};
use crate::patches::bpatch;
use crate::track::{Lab, OscSpec};

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
}

/// How the chords are played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChordStyle {
    /// Held a bar each (pads [synthwave#2]).
    Sustain,
    /// Short stabs on off-16ths ([sound-design#27][sound-design#28]).
    Stabs,
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
    bass_sounds: &["sawBass", "darkBass", "pluckBass"],
    pad_sounds: &["warmPad", "superPad", "darkPad"],
    arp_sounds: &["sqArp", "pluck"],
    lead_sounds: &["sawLead", "sqLead", "brassLead"],
    form: SYNTH_FORM,
    template: SYNTH_TEMPLATE,
    intros: SYNTH_INTROS,
    // The arp is the hook [synthwave#38]: in the chorus, K-weighted, the
    // arp about 5–7 LU under the bass and the lead 3–5 (engine default).
    levels: &[("A", 9.5), ("L", 4.0), ("Pd", 4.0)],
    pump: 0.3,
    tom_fills: true,
    tape: 0.4,
    glue: 0.5,
    reverb: 1.2,
    // 100 % on the snare; 5–15 % on the kick [synthwave#3][synthwave#26].
    gated: (1.0, 0.1),
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
    pad_sounds: &["darkPad", "superPad", "warmPad"],
    lead_sounds: &["sawLead", "brassLead"],
    intros: OUTRUN_INTROS,
    pump: 0.4,
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
        jitter: 0.1,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            0.75,
        )
    },
    lane("ohat", "H", &[3, 7, 11, 15], 0.8),
    DrumLane {
        p: 0.6,
        ..lane("rim", "P", &[2, 4, 6, 8, 10, 12, 14, 16], 0.35)
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
        vel_even: Some(0.5),
        jitter: 0.1,
        ..lane(
            "hat",
            "H",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16],
            0.65,
        )
    },
    DrumLane {
        p: 0.8,
        ..lane("hat", "H", &[10], 0.3)
    },
    lane("ohat", "H", &[3, 7, 11, 15], 0.6),
    DrumLane {
        p: 0.7,
        ..lane("tomM", "P", &[3, 9, 11], 0.6)
    },
    DrumLane {
        p: 0.5,
        ..lane("shaker", "P", &[2, 10], 0.5)
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
    bass_sounds: &["roundBass", "pluckBass", "sawBass"],
    pad_sounds: &["stab", "dubChord", "organ"],
    arp_sounds: &["pluck"],
    lead_sounds: &["piano", "ep", "organ"],
    form: CHICAGO_FORM,
    template: CHICAGO_TEMPLATE,
    // DJ intros open on the kick (G6) [arrangement#14][arrangement#16].
    intros: &[],
    levels: &[],
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
    pad_sounds: &["warmPad", "dubChord", "ep"],
    lead_sounds: &["ep", "bell", "glass"],
    form: DEEP_FORM,
    template: DEEP_TEMPLATE,
    pump: 0.3,
    tape: 0.3,
    glue: 0.45,
    reverb: 1.3,
    ..CHICAGO
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
        "tr909" => {
            let mut v: Vec<_> = ["hat", "ohat", "ride", "crash"]
                .iter()
                .map(|l| {
                    lane(
                        l,
                        &[
                            ("bits", 6.0),
                            ("clock", 30000.0),
                            ("dacLp", 12000.0),
                            ("noise", 0.6),
                        ],
                    )
                })
                .collect();
            for (l, tune, dec) in [
                ("tomH", 170.0, 0.24),
                ("tomM", 125.0, 0.28),
                ("tomL", 90.0, 0.32),
            ] {
                v.push(lane(
                    l,
                    &[
                        ("tune", tune),
                        ("decay", dec),
                        ("pitch", 1.9),
                        ("pt", 0.035),
                        ("click", 0.3),
                        ("drive", 1.35),
                        ("noise", 0.05),
                    ],
                ));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// Loads a base patch from [`crate::patches::bpatch`], sets its `name` and
/// measured Juno chorus parameters (sound-design.md §6), and applies the
/// genre's instrument voicing (synthwave.md §4.4–4.7).
pub fn sound_patch(genre: &str, name: &str) -> Option<Lab> {
    let mut lab = bpatch(name)?;
    lab.name = Some(name.to_owned());
    if matches!(genre, "synthwave" | "outrun") {
        let osc =
            |w: &str, det: Option<f64>, oct: Option<f64>, lvl: Option<f64>, pw: Option<f64>| {
                OscSpec {
                    w: w.to_owned(),
                    det,
                    oct,
                    lvl,
                    pw,
                }
            };
        match name {
            // Prophet-5 / Juno-60 plucked 8th-note bass ("A Real Hero" /
            // "Nightcall"): saw + PWM pulse + upper-octave saw + phase-locked
            // sub, snappy filter decay, and Juno Chorus I high-passed above
            // 180 Hz so the bottom stays mono [synthwave#2][synthwave#38][synthwave#39].
            "sawBass" => {
                lab.osc = Some(vec![
                    osc("saw", None, None, Some(1.0), None),
                    osc("pulse", Some(7.0), None, Some(0.55), Some(0.42)),
                    osc("saw", Some(-6.0), Some(1.0), Some(0.35), None),
                ]);
                lab.sub = Some(0.6);
                lab.pwm = Some(0.25);
                lab.lfo_rate = Some(0.45);
                lab.cutoff = Some(360.0);
                lab.res = Some(0.22);
                lab.fenv = Some(2.85);
                lab.keytrack = Some(0.15);
                lab.drive = Some(1.65);
                lab.a = Some(0.003);
                lab.d = Some(0.42);
                lab.s = Some(0.30);
                lab.r = Some(0.14);
                lab.fa = Some(0.002);
                lab.fd = Some(0.40);
                lab.fs = Some(0.14);
                lab.fr = Some(0.14);
                lab.chorus = Some(1.0);
                lab.chorus_hp = Some(180.0);
                lab.gain = Some(0.22);
            }
            "darkBass" => {
                lab.osc = Some(vec![
                    osc("saw", None, None, Some(1.0), None),
                    osc("pulse", Some(9.0), None, Some(0.65), Some(0.48)),
                    osc("saw", Some(-8.0), Some(1.0), Some(0.28), None),
                ]);
                lab.sub = Some(0.65);
                lab.pwm = Some(0.2);
                lab.lfo_rate = Some(0.4);
                lab.cutoff = Some(310.0);
                lab.res = Some(0.25);
                lab.fenv = Some(2.65);
                lab.keytrack = Some(0.15);
                lab.drive = Some(2.1);
                lab.a = Some(0.003);
                lab.d = Some(0.44);
                lab.s = Some(0.32);
                lab.r = Some(0.14);
                lab.fa = Some(0.002);
                lab.fd = Some(0.42);
                lab.fs = Some(0.15);
                lab.fr = Some(0.14);
                lab.chorus = Some(1.0);
                lab.chorus_hp = Some(180.0);
                lab.gain = Some(0.22);
            }
            "pluckBass" => {
                lab.osc = Some(vec![
                    osc("saw", None, None, Some(0.9), None),
                    osc("pulse", Some(6.0), None, Some(0.7), Some(0.38)),
                ]);
                lab.sub = Some(0.65);
                lab.pwm = Some(0.3);
                lab.lfo_rate = Some(0.5);
                lab.cutoff = Some(340.0);
                lab.res = Some(0.26);
                lab.fenv = Some(3.0);
                lab.keytrack = Some(0.15);
                lab.drive = Some(1.75);
                lab.a = Some(0.003);
                lab.d = Some(0.38);
                lab.s = Some(0.24);
                lab.r = Some(0.14);
                lab.fa = Some(0.002);
                lab.fd = Some(0.36);
                lab.fs = Some(0.10);
                lab.fr = Some(0.14);
                lab.chorus = Some(1.0);
                lab.chorus_hp = Some(180.0);
                lab.gain = Some(0.22);
            }
            // Juno-60 / OB-Xa arpeggio plucks: saw + PWM pulse unison into a
            // 12 dB/oct filter with a 400 ms envelope and Chorus II
            // [synthwave#38][synthwave#45].
            "pluck" => {
                lab.saw = Some(1.0);
                lab.pulse = Some(0.55);
                lab.pw = Some(0.45);
                lab.pwm = Some(0.45);
                lab.lfo_rate = Some(0.65);
                lab.sub = Some(0.0);
                lab.unison = Some(2.0);
                lab.detune = Some(10.0);
                lab.hpf = Some(140.0);
                lab.cutoff = Some(1050.0);
                lab.res = Some(0.20);
                lab.fenv = Some(1.95);
                lab.keytrack = Some(0.6);
                lab.slope = Some(12.0);
                lab.drive = Some(0.3);
                lab.a = Some(0.005);
                lab.d = Some(0.45);
                lab.s = Some(0.22);
                lab.r = Some(0.28);
                lab.fa = Some(0.004);
                lab.fd = Some(0.40);
                lab.fs = Some(0.18);
                lab.fr = Some(0.28);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.22);
            }
            "sqArp" => {
                lab.saw = Some(0.55);
                lab.pulse = Some(0.95);
                lab.pw = Some(0.38);
                lab.pwm = Some(0.55);
                lab.lfo_rate = Some(0.75);
                lab.sub = Some(0.0);
                lab.unison = Some(2.0);
                lab.detune = Some(11.0);
                lab.hpf = Some(140.0);
                lab.cutoff = Some(1100.0);
                lab.res = Some(0.22);
                lab.fenv = Some(1.85);
                lab.keytrack = Some(0.6);
                lab.slope = Some(12.0);
                lab.drive = Some(0.3);
                lab.a = Some(0.005);
                lab.d = Some(0.45);
                lab.s = Some(0.25);
                lab.r = Some(0.28);
                lab.fa = Some(0.004);
                lab.fd = Some(0.42);
                lab.fs = Some(0.20);
                lab.fr = Some(0.28);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.22);
            }
            // Juno-106 / OB-Xa pads: saw + PWM pulse, no sub-oscillator mud,
            // 180 Hz HPF, open 12 dB/oct filter, and Chorus II [synthwave#43].
            "warmPad" => {
                lab.saw = Some(0.9);
                lab.pulse = Some(0.7);
                lab.pw = Some(0.45);
                lab.pwm = Some(0.55);
                lab.lfo_rate = Some(0.45);
                lab.sub = Some(0.0);
                lab.unison = Some(2.0);
                lab.detune = Some(12.0);
                lab.hpf = Some(180.0);
                lab.cutoff = Some(1800.0);
                lab.res = Some(0.15);
                lab.fenv = Some(0.6);
                lab.slope = Some(12.0);
                lab.drive = Some(0.25);
                lab.a = Some(0.18);
                lab.d = Some(1.2);
                lab.s = Some(0.82);
                lab.r = Some(0.52);
                lab.fa = Some(0.22);
                lab.fd = Some(1.4);
                lab.fs = Some(0.65);
                lab.fr = Some(0.52);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.22);
            }
            "superPad" => {
                lab.saw = Some(1.0);
                lab.pulse = Some(0.45);
                lab.pw = Some(0.5);
                lab.pwm = Some(0.45);
                lab.lfo_rate = Some(0.5);
                lab.sub = Some(0.0);
                lab.unison = Some(4.0);
                lab.detune = Some(16.0);
                lab.hpf = Some(180.0);
                lab.cutoff = Some(2100.0);
                lab.res = Some(0.14);
                lab.fenv = Some(0.7);
                lab.slope = Some(12.0);
                lab.drive = Some(0.25);
                lab.a = Some(0.16);
                lab.d = Some(1.2);
                lab.s = Some(0.8);
                lab.r = Some(0.52);
                lab.fa = Some(0.20);
                lab.fd = Some(1.4);
                lab.fs = Some(0.65);
                lab.fr = Some(0.52);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.21);
            }
            "darkPad" => {
                lab.saw = Some(0.95);
                lab.pulse = Some(0.55);
                lab.pw = Some(0.42);
                lab.pwm = Some(0.5);
                lab.lfo_rate = Some(0.38);
                lab.sub = Some(0.0);
                lab.unison = Some(3.0);
                lab.detune = Some(14.0);
                lab.hpf = Some(180.0);
                lab.cutoff = Some(1450.0);
                lab.res = Some(0.18);
                lab.fenv = Some(0.7);
                lab.slope = Some(12.0);
                lab.drive = Some(0.3);
                lab.a = Some(0.20);
                lab.d = Some(1.4);
                lab.s = Some(0.78);
                lab.r = Some(0.52);
                lab.fa = Some(0.24);
                lab.fd = Some(1.5);
                lab.fs = Some(0.55);
                lab.fr = Some(0.52);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.22);
            }
            // Prophet-5 / OB-Xa / CS-80 solo leads: detuned saws + PWM pulse,
            // 12 dB/oct filter, portamento glide, delayed vibrato, and
            // stereo Chorus II [synthwave#48].
            "sawLead" => {
                lab.osc = Some(vec![
                    osc("saw", Some(-7.0), None, Some(1.0), None),
                    osc("saw", Some(7.0), None, Some(0.85), None),
                    osc("pulse", Some(3.0), None, Some(0.45), Some(0.45)),
                ]);
                lab.pwm = Some(0.35);
                lab.lfo_rate = Some(0.6);
                lab.sub = Some(0.12);
                lab.cutoff = Some(2300.0);
                lab.res = Some(0.20);
                lab.fenv = Some(1.35);
                lab.slope = Some(12.0);
                lab.drive = Some(0.6);
                lab.a = Some(0.012);
                lab.d = Some(0.45);
                lab.s = Some(0.82);
                lab.r = Some(0.24);
                lab.fa = Some(0.012);
                lab.fd = Some(0.45);
                lab.fs = Some(0.45);
                lab.fr = Some(0.24);
                lab.glide = Some(0.055);
                lab.vib = Some(8.5);
                lab.vib_rate = Some(5.5);
                lab.vib_delay = Some(0.28);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.24);
            }
            "sqLead" => {
                lab.osc = Some(vec![
                    osc("pulse", Some(-6.0), None, Some(1.0), Some(0.42)),
                    osc("saw", Some(6.0), None, Some(0.75), None),
                ]);
                lab.pwm = Some(0.45);
                lab.lfo_rate = Some(0.65);
                lab.sub = Some(0.15);
                lab.cutoff = Some(2150.0);
                lab.res = Some(0.22);
                lab.fenv = Some(1.35);
                lab.slope = Some(12.0);
                lab.drive = Some(0.55);
                lab.a = Some(0.012);
                lab.d = Some(0.42);
                lab.s = Some(0.80);
                lab.r = Some(0.24);
                lab.fa = Some(0.012);
                lab.fd = Some(0.42);
                lab.fs = Some(0.42);
                lab.fr = Some(0.24);
                lab.glide = Some(0.055);
                lab.vib = Some(8.5);
                lab.vib_rate = Some(5.5);
                lab.vib_delay = Some(0.28);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.15);
            }
            "brassLead" => {
                lab.saw = Some(1.0);
                lab.pulse = Some(0.55);
                lab.pw = Some(0.45);
                lab.pwm = Some(0.4);
                lab.lfo_rate = Some(0.55);
                lab.sub = Some(0.0);
                lab.unison = Some(3.0);
                lab.detune = Some(12.0);
                lab.hpf = Some(140.0);
                lab.cutoff = Some(1850.0);
                lab.res = Some(0.18);
                lab.fenv = Some(1.55);
                lab.slope = Some(12.0);
                lab.drive = Some(0.45);
                lab.a = Some(0.028);
                lab.d = Some(0.48);
                lab.s = Some(0.78);
                lab.r = Some(0.26);
                lab.fa = Some(0.038);
                lab.fd = Some(0.48);
                lab.fs = Some(0.48);
                lab.fr = Some(0.26);
                lab.bend = Some(0.22);
                lab.bend_t = Some(0.04);
                lab.glide = Some(0.055);
                lab.vib = Some(9.0);
                lab.vib_rate = Some(5.4);
                lab.vib_delay = Some(0.32);
                lab.chorus = Some(2.0);
                lab.gain = Some(0.22);
            }
            _ => {}
        }
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
pub const PROFILES: &[Profile] = &[SYNTHWAVE, OUTRUN, CHICAGO, DEEP];

pub fn profile(key: &str) -> Option<&'static Profile> {
    PROFILES.iter().find(|p| p.key == key)
}
