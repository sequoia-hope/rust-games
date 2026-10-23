//! Keys, modes, Roman numerals and voicings for the new generator.
//!
//! Numerals follow research/RULES.md ("Units"): relative to the tonic of
//! the stated mode, upper case major, lower case minor, `ø` half-diminished,
//! `°` diminished, and `♭`/`b`/`#` only where the mode would not give the
//! chord (so `VI` in minor is the mode's sixth, `♭VI` in major is borrowed).
//! A suffix adds the extension (`7`, `maj7`, `9`, `m9`, `11`, `add9`,
//! `sus2`, `sus4`); `6` is a first inversion (the third in the bass, as
//! RULES.md's "VII6" over its 3rd).

/// A mode: its name and its scale in semitones from the tonic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    pub name: &'static str,
    pub scale: [i32; 7],
    /// Minor-family (Aeolian, Dorian, Phrygian, harmonic minor).
    pub minor: bool,
}

pub const AEOLIAN: Mode = Mode {
    name: "minor",
    scale: [0, 2, 3, 5, 7, 8, 10],
    minor: true,
};
pub const DORIAN: Mode = Mode {
    name: "dorian",
    scale: [0, 2, 3, 5, 7, 9, 10],
    minor: true,
};
pub const PHRYGIAN: Mode = Mode {
    name: "phrygian",
    scale: [0, 1, 3, 5, 7, 8, 10],
    minor: true,
};
pub const HARMONIC_MINOR: Mode = Mode {
    name: "harmonic minor",
    scale: [0, 2, 3, 5, 7, 8, 11],
    minor: true,
};
pub const IONIAN: Mode = Mode {
    name: "major",
    scale: [0, 2, 4, 5, 7, 9, 11],
    minor: false,
};
pub const MIXOLYDIAN: Mode = Mode {
    name: "mixolydian",
    scale: [0, 2, 4, 5, 7, 9, 10],
    minor: false,
};

pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// A chord: its root (semitones above the tonic, 0..12) and its tones as
/// intervals above the root, the bass's interval first.
#[derive(Clone, Debug, PartialEq)]
pub struct Chord {
    /// The numeral it was read from.
    pub name: String,
    pub root: i32,
    /// Intervals above the root, ascending (0 first).
    pub tones: Vec<i32>,
    /// The tone in the bass (an interval above the root; 0 in root
    /// position).
    pub bass: i32,
    pub minor: bool,
}

impl Chord {
    /// The chord's pitch classes relative to the tonic.
    pub fn pcs(&self) -> Vec<i32> {
        self.tones
            .iter()
            .map(|t| (self.root + t).rem_euclid(12))
            .collect()
    }

    /// The symbol a statistical model sees: relative root and quality
    /// (RULES.md G9.2, as Cheung et al. used).
    pub fn symbol(&self) -> u32 {
        let q = if self.tones.contains(&6) && !self.tones.contains(&7) {
            2
        } else if self.minor {
            1
        } else {
            0
        };
        (self.root.rem_euclid(12) as u32) * 3 + q
    }
}

/// Reads a numeral in `mode` (see the module docs); `None` if it is not
/// one.
pub fn numeral(s: &str, mode: &Mode) -> Option<Chord> {
    let mut rest = s.trim();
    let mut acc = 0;
    loop {
        if let Some(r) = rest.strip_prefix('♭').or_else(|| rest.strip_prefix('b')) {
            acc -= 1;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('#').or_else(|| rest.strip_prefix('♯')) {
            acc += 1;
            rest = r;
        } else {
            break;
        }
    }
    const NUMS: [(&str, usize); 7] = [
        ("vii", 7),
        ("iii", 3),
        ("vi", 6),
        ("iv", 4),
        ("ii", 2),
        ("v", 5),
        ("i", 1),
    ];
    let lower = rest.to_ascii_lowercase();
    let (n, deg) = NUMS.iter().find(|(n, _)| lower.starts_with(n))?;
    let upper = rest.as_bytes()[0].is_ascii_uppercase();
    let mut suffix = &rest[n.len()..];
    let mut dim = false;
    let mut half_dim = false;
    if let Some(r) = suffix
        .strip_prefix('°')
        .or_else(|| suffix.strip_prefix("dim"))
    {
        dim = true;
        suffix = r;
    } else if let Some(r) = suffix.strip_prefix('ø') {
        half_dim = true;
        suffix = r;
    }
    let root = if acc != 0 {
        IONIAN.scale[deg - 1] + acc
    } else {
        mode.scale[deg - 1]
    };
    let minor = !upper || dim || half_dim;
    let mut tones = if dim || half_dim {
        vec![0, 3, 6]
    } else if minor {
        vec![0, 3, 7]
    } else {
        vec![0, 4, 7]
    };
    let mut bass = 0;
    // Strip "(…)" around an extension: "VI(add9)".
    let suffix: String = suffix.chars().filter(|c| *c != '(' && *c != ')').collect();
    let mut sfx = suffix.as_str();
    while !sfx.is_empty() {
        let (add, len): (&[i32], usize) = if sfx.starts_with("maj7") {
            (&[11], 4)
        } else if sfx.starts_with("maj9") {
            (&[11, 14], 4)
        } else if sfx.starts_with("add9") {
            (&[14], 4)
        } else if sfx.starts_with("sus2") {
            tones.retain(|t| *t != 3 && *t != 4);
            (&[2], 4)
        } else if sfx.starts_with("sus4") {
            tones.retain(|t| *t != 3 && *t != 4);
            (&[5], 4)
        } else if sfx.starts_with("m9") {
            (&[10, 14], 2)
        } else if sfx.starts_with("11") {
            (&[10, 14, 17], 2)
        } else if sfx.starts_with('9') {
            (&[10, 14], 1)
        } else if sfx.starts_with('7') {
            if dim { (&[9], 1) } else { (&[10], 1) }
        } else if sfx.starts_with('6') {
            bass = if minor { 3 } else { 4 };
            (&[], 1)
        } else if sfx.starts_with('m') {
            // "Vm": a minor v written in upper case.
            tones.retain(|t| *t != 4);
            tones.push(3);
            (&[], 1)
        } else {
            return None;
        };
        for a in add {
            if !tones.contains(a) {
                tones.push(*a);
            }
        }
        sfx = &sfx[len..];
    }
    if half_dim && !tones.contains(&10) {
        tones.push(10);
    }
    tones.sort();
    Some(Chord {
        name: s.to_owned(),
        root,
        tones,
        bass,
        minor,
    })
}

/// A progression from numerals separated by `–`, `-` or spaces.
pub fn progression(s: &str, mode: &Mode) -> Option<Vec<Chord>> {
    s.split(['–', '-', ' '])
        .filter(|x| !x.is_empty())
        .map(|x| numeral(x, mode))
        .collect()
}

/// Voices `chord` (in the key whose tonic is MIDI `tonic`) as `n` notes
/// between `lo` and `hi`, as close as possible to `prev` (least total
/// motion; RULES.md: "drop other notes first", "the top voice moves by
/// step"). Keeps the 7th and the 3rd when it has to drop tones.
pub fn voice(
    chord: &Chord,
    tonic: i32,
    n: usize,
    lo: i32,
    hi: i32,
    prev: Option<&[i32]>,
) -> Vec<i32> {
    // Which tones to keep: the root, 3rd, 7th, and 5th first so 4-voice
    // seventh chords form a complete closed triad+7th, then extensions.
    let mut tones = chord.tones.clone();
    let rank = |t: i32| match t {
        0 => 0,
        3 | 4 => 1,
        10 | 11 => 2,
        6 | 7 => 3,
        14 | 17 | 2 | 5 | 9 => 4,
        _ => 5,
    };
    tones.sort_by_key(|t| rank(*t));
    tones.truncate(n.max(1));
    let pcs: Vec<i32> = tones
        .iter()
        .map(|t| (tonic + chord.root + t).rem_euclid(12))
        .collect();
    let third_pc = (tonic + chord.root + if chord.minor { 3 } else { 4 }).rem_euclid(12);
    let ninth_pc = chord
        .tones
        .contains(&14)
        .then_some((tonic + chord.root + 2).rem_euclid(12));
    // Every way of placing those pitch classes in range, closest-packed:
    // start the lowest at each candidate and stack upward (placing a 9th
    // above the 3rd so minor-9th chords voice as root–3rd–7th–9th instead
    // of a 2nd–3rd semitone cluster).
    let mut best: Option<(i32, Vec<i32>)> = None;
    for start in lo..lo + 12 {
        let pc0 = start.rem_euclid(12);
        if !pcs.contains(&pc0) || ninth_pc == Some(pc0) {
            continue;
        }
        let mut notes = vec![start];
        let mut used = vec![pc0];
        let mut cur = start;
        while notes.len() < pcs.len() {
            cur += 1;
            let pc = cur.rem_euclid(12);
            if pcs.contains(&pc) && !used.contains(&pc) {
                if ninth_pc == Some(pc) && !used.contains(&third_pc) && cur + 12 <= hi {
                    continue;
                }
                notes.push(cur);
                used.push(pc);
            }
        }
        if *notes.last().unwrap() > hi {
            continue;
        }
        let cost = match prev {
            Some(p) if p.len() == notes.len() => {
                notes.iter().zip(p).map(|(a, b)| (a - b).abs()).sum()
            }
            Some(p) => {
                let c = |v: &[i32]| v.iter().sum::<i32>() as f64 / v.len().max(1) as f64;
                ((c(&notes) - c(p)).abs() * 2.0) as i32
            }
            // No previous chord: the middle of the range, avoiding low 2nd
            // clusters (e.g. 7th–root a whole step apart in the bottom voices)
            // and wide internal holes.
            None => {
                let root_pc = (tonic + chord.root + chord.bass).rem_euclid(12);
                let mut c = (notes[0] + notes[notes.len() - 1] - lo - hi).abs();
                if notes.len() >= 2 && notes[1] - notes[0] <= 2 {
                    c += 8;
                }
                for w in notes.windows(2) {
                    if w[1] - w[0] > 7 {
                        c += (w[1] - w[0] - 7) * 2;
                    }
                }
                if pc0 != root_pc {
                    c += 3;
                }
                c
            }
        };
        if best.as_ref().is_none_or(|(b, _)| cost < *b) {
            best = Some((cost, notes));
        }
    }
    best.map(|(_, v)| v)
        .unwrap_or_else(|| vec![lo + (tonic + chord.root).rem_euclid(12)])
}

/// The chord's bass note at or above MIDI `lo` (its root, or the inverted
/// bass tone).
pub fn bass_note(chord: &Chord, tonic: i32, lo: i32) -> i32 {
    let pc = (tonic + chord.root + chord.bass).rem_euclid(12);
    lo + (pc - lo).rem_euclid(12)
}

/// The scale's notes from MIDI `lo` to `hi`.
pub fn scale_notes(mode: &Mode, tonic: i32, lo: i32, hi: i32) -> Vec<i32> {
    (lo..=hi)
        .filter(|m| mode.scale.contains(&(m - tonic).rem_euclid(12)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numerals_read_in_their_mode() {
        let vi = numeral("VI", &AEOLIAN).unwrap();
        assert_eq!(
            (vi.root, vi.tones.clone(), vi.minor),
            (8, vec![0, 4, 7], false)
        );
        let flat6 = numeral("♭VI", &IONIAN).unwrap();
        assert_eq!(flat6.root, 8, "borrowed in major");
        let i7 = numeral("i7", &AEOLIAN).unwrap();
        assert_eq!(i7.tones, vec![0, 3, 7, 10]);
        let vimaj7 = numeral("VImaj7", &AEOLIAN).unwrap();
        assert_eq!(vimaj7.tones, vec![0, 4, 7, 11]);
        let vii6 = numeral("VII6", &AEOLIAN).unwrap();
        assert_eq!((vii6.root, vii6.bass), (10, 4), "VII over its 3rd");
        let add9 = numeral("VI(add9)", &AEOLIAN).unwrap();
        assert_eq!(add9.tones, vec![0, 4, 7, 14]);
        let m9 = numeral("im9", &AEOLIAN).unwrap();
        assert_eq!(m9.tones, vec![0, 3, 7, 10, 14]);
        assert!(numeral("xyz", &AEOLIAN).is_none());
        let p = progression("i–VI–III–VII", &AEOLIAN).unwrap();
        assert_eq!(
            p.iter().map(|c| c.root).collect::<Vec<_>>(),
            vec![0, 8, 3, 10]
        );
        assert!(progression("IV–V–vi–I", &IONIAN).unwrap()[2].minor);
    }

    #[test]
    fn voicings_move_little() {
        let p = progression("i7–VImaj7–III–VII", &AEOLIAN).unwrap();
        let mut prev: Option<Vec<i32>> = None;
        for c in &p {
            let v = voice(c, 57, 4, 55, 76, prev.as_deref());
            assert_eq!(v.len(), if c.tones.len() >= 4 { 4 } else { 3 });
            assert!(v.iter().all(|m| (55..=76).contains(m)), "{v:?}");
            if let Some(pv) = &prev
                && pv.len() == v.len()
            {
                let moved: i32 = v.iter().zip(pv).map(|(a, b)| (a - b).abs()).sum();
                assert!(moved <= 8, "{} moved {moved}", c.name);
            }
            prev = Some(v);
        }
        assert_eq!(
            bass_note(&numeral("VII6", &AEOLIAN).unwrap(), 57, 33) % 12,
            (57 + 10 + 4) % 12
        );
    }
}
