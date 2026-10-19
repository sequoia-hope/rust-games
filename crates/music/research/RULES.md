# Rules for the Phase 3 generator

The distilled, implementable rules the new generator (PLAN.md, Phase 3) is
built from: global rules that every genre shares, then one profile per genre
in a fixed field list that maps onto a `GenreProfile` struct, then what the
engine lacks and what the research left open. Every rule cites the research
file it came from. Nothing here is new research: where the generator needs a
number the files do not give, the rule says **engine default: X**.

---

## How to read this

**Citations.** `[house#12]` is reference 12 of `research/genres/house.md`;
`[cognition#4]` is reference 4 of `research/cognition.md`. File keys:
`arrangement`, `cognition`, `sound-design` (in `research/`), and `house`,
`techno`, `dnb`, `trance`, `synthwave`, `ambient` (in `research/genres/`).
Several citations on one rule mean the rule combines them.

**Labels, carried over from the source files.**

| Label | Meaning | Used by |
|---|---|---|
| (unverified) | the source author could not confirm it | all files |
| (engine default) | a design choice in the source file, not backed by a source | house, trance, sound-design |
| (design choice) | the same as (engine default), in ambient.md's wording | ambient |
| (heuristic) | the same, in dnb.md's wording | dnb |
| (proposal) | an engineering proposal with no direct empirical source | cognition |
| (derived) | the source author's own arithmetic on a published number | cognition, sound-design |
| (inference) | the source author's reading, not stated by a source | techno |
| (forum) | practitioner forum only | trance |
| **engine default: X** | a number this file adds because the generator needs one and no research file gives it | this file |

**Units and conventions** (used everywhere below).

- **Grid.** One 4/4 bar = 16 steps numbered **1–16**. Beats are steps 1, 5,
  9, 13; off-beat 8ths are 3, 7, 11, 15; even steps are the "e" and "a"
  16ths. A "2-bar loop" numbers its second bar 1–16 again. 32nds are written
  as half-steps (e.g. 12.5). [house][techno][dnb][trance][synthwave]
- **Tempo** in BPM; one bar lasts 240/BPM s; a 16-bar section 3840/BPM s
  (30 s at 128, 22 s at 174). [arrangement#7]
- **Swing** as the Linn/MPC percentage *s* (50 = straight, 54 = loose
  without sounding swung, 66.7 = triplet). Every even step is delayed by
  **(2s − 1) × one 16th** (55 % = 0.10, 60 % = 0.20, 66.7 % = 0.33, 69 % =
  0.38 of a 16th). Useful range 50–70. DAWs use other scales (FL/Cubase 0 %
  = Linn 50 %; Maschine's scale cannot be converted). [house#11][house#12]
- **Velocity** 0–1 (1 = full or accent). Where a source gives dB it stays in
  dB. Percent velocities from sources are written as fractions (70 % → 0.7).
- **Pitch.** MIDI note numbers or Hz; intervals in semitones (st) or cents.
  Roman numerals relative to the tonic of the stated mode: upper case major,
  lower case minor, `ø` half-diminished; `♭VI` etc. only where the mode
  would not give the chord.
- **Energy** E in 0–1 per phrase block (the planner's input and the
  renderer's `energy`).
- **Probabilities** p in 0–1, "per track" unless stated.
- **Typical tempo** is the source's stated centre or median where one exists
  (cited); otherwise it is the midpoint of the cited range, an engine default.
- **Weights** on modes and progressions are the source's numbers where it
  gives them (GiantSteps minor shares, synthwave.md's mode split); otherwise
  they are engine defaults ordered by how often the research attests each
  item, Zipf-like [cognition#6][cognition#10], and marked so.

**The profile field list** (identical for every genre, in this order; the
names are the struct fields):

```text
GenreProfile {
  scope        // sub-styles covered, variants as modes of one profile
  tempo        // range_bpm (lo, hi), typical_bpm
  swing        // linn_pct (lo, hi), default
  meter_grid   // meter, steps per bar, extra grids (triplet, 32nd, odd cycles), half-time
  drums        // kit; per lane: steps (16-grid), vel, variations, fills
  bass         // pattern grammar, register, articulation, sound
  harmony      // modes [(mode, w)], progressions [(romans, w)], extensions [(ext, p)],
               // bars_per_chord, loop_bars
  melody       // density (onsets/bar), range (st), contour, call/response, repetition
  sounds       // role -> engine instrument + preset recipe; missing instruments
  arrangement  // layer template table (rows = roles, columns = phrase blocks),
               // intro_bars, outro_bars, total_bars
  energy       // E per block
  surprise     // where variation goes and how much
  references   // landmark tracks to listen against
}
```

**Arrangement tables.** Rows are roles, columns are phrase blocks (bar
ranges, 1-based, inclusive). Cell codes: `x` plays its core clip; `v` plays
a variation; `f` filtered or teased (LPF closed / HPF'd / low level); `s`
sparse or sliced (half the notes, a fragment); `+` enters during the block
(the cell names the bar if it isn't the block start); `r` active as a build
device (roll or riser); `1` a single hit on the block's first downbeat; `-`
silent. Role keys follow arrangement.md's legend [arrangement]: **K** kick,
**H** hats/ride, **C** clap/snare, **P** percussion, **B** bass (mid),
**Sub** sub-bass, **Pd** pad/chords/stabs, **A** arp, **L** lead/hook, **V**
vocal or chop, **Atm** atmosphere/FX bed, **R** riser, **Sn** snare roll,
**X** impact. Where a template's bar numbers come from a source, the
citation is on the table; where the source gives only the order, the bar
counts are marked engine default.

**What the RNG may do** (PLAN.md): pick among the options a rule allows,
weighted as the rule says, from its own forked stream per decision. A rule
with a range means "draw uniformly in the range" unless a weight is given.

---

## Global rules

### G1. Tempo and time

- Hold one tempo for the whole track; builds fake acceleration by
  subdividing (quarters → 8ths → 16ths), never by changing BPM.
  [arrangement#2][arrangement#11][techno#27][techno#35]
- Exception: electro may wander ±0.5 BPM, imitating an 808 master clock
  (Robotnick averages 120.7). [synthwave#12]
- Tempo medians from Beatport (crowd-corrected, median and IQR): deep house
  122 (121–123), tech house 127 (123–127), progressive house 127 (127–129),
  electro house 128.5 (127–130), techno 127 (123–132), trance 139, dubstep
  139, breaks 139.5, psytrance 147, DnB 174 (170–174), hardcore/hard techno
  170. [cognition#7][cognition#8]
- Humanise drums in a fixed direction: hits on the 8th-note grid late,
  off-beat 16ths early; the size is a per-profile parameter, not a constant.
  [cognition#23]
- Sync every modulation to the bar: LFO Hz = BPM/60 × divisions per beat (1/4
  → ×1, 1/8 → ×2, 1/8T → ×3, 1/16 → ×4). (derived) [sound-design#43]

### G2. The phrase grid

- Loops are 1, 2, 4 or 8 bars; sections are multiples of 8 bars (16 the
  working default); every layer entry, exit or change falls on a multiple of
  4 bars; at least one layer enters, exits or changes every 16 bars,
  typically every 8. [arrangement#1][arrangement#2][arrangement#8][arrangement#9]
- Evidence: 95.3 % of 160 DnB songs fit "phrases are multiples of 8 bars"
  [arrangement#8]; in 1,557 DJ mixes transition lengths peak every 32 beats (8
  bars) [arrangement#7]; techno arranges in 8- or 16-bar blocks, adding one
  element every 8 or 16 bars [techno#20][techno#26].
- A loop runs at least 16 bars unchanged in techno; the filter moves
  continuously. [techno#20][techno#26][techno#27]
- Deliberate off-grid moments are surprises, rationed:
  - 5–10 % of tracks: end a section 1 bar early (7, 15 or 31 bars) or after
    14 bars. [arrangement#15][arrangement#19][arrangement#20]
  - 10–20 % of tracks: one extra 1-, 2- or 4-bar gap before a major
    transition, never in the first or last 32 bars. [arrangement#10]

### G3. Layer roles, entry and exit

**Roles** (the score model's layer roles): K, H (closed hat, open hat,
ride, shaker as sub-lanes), C, P, B, Sub, Pd, A, L, V, Atm, plus the
transition roles R, Sn, X (G4). [arrangement] PLAN.md adds `fx`.

**Entry order** (four-on-the-floor genres): K → H (off-beat open hat or
ride) → C → P → B → Pd → L/V → FX ear-candy. Each entry lands 8 or 16 bars
after the previous one; B and L may move later. H never enters before K,
except house intros that open on clap and hat (Disclosure).
[arrangement#13][arrangement#14][arrangement#16][arrangement#17]

| Rule | Value | Source |
|---|---|---|
| Kick enters | bar 1 in house, techno, trance (5 of 6 deconstructed 4/4 tracks) | [arrangement#14][arrangement#16][arrangement#18] |
| Second drum layer (off-beat hat or ride) | bar 9 | [arrangement#14][arrangement#16] |
| Clap/snare | by bar 17 | [arrangement#14][arrangement#16] |
| DnB/jungle intro | a processed or filtered break, not a kick | [arrangement#19][arrangement#20] |
| Bass entry | from {9, 17, 33, 41, 65, first drop}; house 17–41, techno 9–65, DnB/dubstep 17 or first drop; held out of bars 1–8 in 6 of 7 tracks, except deep-house/techno "kick + bass" intros | [arrangement#13]–[arrangement#17][arrangement#20] |
| Lead/hook | out of the intro, or a bare motif over the kick | [arrangement#9][arrangement#14][arrangement#18] |
| Pitched material (chords, lead) | out of the first and last 16 bars of club versions | [arrangement#9][arrangement#33] |
| Layer count | intro 1–3; groove 4–6; core/drop 8–12; breakdown 1–3; outro 2–3 (author's count) | [arrangement#14]–[arrangement#17] |
| Outro | remove layers in the reverse order of entry, one per 8 or 16 bars | [techno#20] |
| Anacrusis entries | a new synth or vocal may start just before the downbeat | [arrangement#18] |

**Additive vs subtractive.** Both are valid generation strategies:
additive stacks loops one at a time (Plastikman) [arrangement#1]; subtractive
writes the peak first and mutes parts to carve intro and sections
[arrangement#10]; Timecop1983 builds a 6–7 s peak loop first and derives the
intro and breakdown by subtraction [synthwave#50]. Detroit-school tracks
arrange almost entirely by muting (no programmed fills in Mills "The Bells",
Reese, Âme "Rej"). [arrangement#14][arrangement#15][arrangement#17]

### G4. Transitions

**Breakdown.** Remove kick and bass first, then hats and shakers ("best
saved for when it's really pumping"); optionally keep the clap on 5, 13
(Kronos), or keep kick + bass (Âme). The bass may keep playing with its lows
cut (HPF ≥ 150 Hz). Lower the breakdown 2–4 dB against the core. Disable or
reduce sidechain. In breakdowns change the chord loop every 8 bars: full
progression, then shortened, then one repeated chord in the last 8.
[arrangement#2][arrangement#9][arrangement#12][arrangement#17]

| Genre | Breakdown bars | Build bars | Source |
|---|---|---|---|
| house | 16–64 | 8 (4–16) | [arrangement#12][arrangement#13] |
| techno | 1–16 (Berghain 0–1 breakdowns ≤ 8; melodic one of 16) | rare; filter moves and fake-outs | [arrangement#14][arrangement#17][techno#35][techno#44] |
| trance | 32–64 | 8–16 | [arrangement#21][arrangement#22] |
| psytrance | main ≤ 64 at ~2/3; short breaks ≤ 16 | 8, 16 or 32 | [trance#25] |
| DnB / dubstep | 16–32 | 8–16 | [dnb#36][arrangement#23] |
| pop-EDM riserchorus | 8–16 | in the riserchorus | [arrangement#6] |

**Snare roll** (the build). Osborn's build is "a rhythmic crescendo, usually
on a synthesized snare drum, in which attack onsets become exponentially
faster" with the four-on-the-floor kick absent. Stages: quarters (1, 5, 9,
13) → 8ths (odd steps) → 16ths (every step) → optionally 32nds in the last
bar. Velocity ramps 1 → 127 (0.01 → 1.0) with slight randomisation; accent
the first of each 4 sixteenths. Optional layers: pitch rise across the roll
(3–12 st), a filter opening fully at the drop on a parabolic curve, a 16th
roll layered with a quarter-note snare. [arrangement#6][arrangement#11][arrangement#12][trance#28]

| Source / template | Stage lengths (quarters / 8ths / 16ths / 32nds / silence) |
|---|---|
| arrangement.md house template (8-bar build) | 4 / 2 / 1 / 1 bar, silence on the last 2 beats [arrangement#9][arrangement#12][arrangement#13] |
| arrangement.md trance template (16-bar build) | 8 / 4 / 3 / 1 bars [arrangement#22][arrangement#21] |
| trance.md (last 8 bars) | 2 / 2 / 2 / 1 / 1 bar (engine default) [trance#28] |
| dnb.md build roll (kick or snare) | 4 / 2 / 2 / 0 (heuristic) [dnb#14] |
| Attack "speed-up" roll (1 bar) | 4 quarters → 4 8ths → 8 16ths [arrangement#11][trance#28] |

- Stop early at beat 2.4 (or 3) of the last bar and leave silence: 30 % of
  builds [arrangement#11][arrangement#12]; trance.md says ≈ 20 % [trance#28].
  **engine default: 25 %.**
- No rolls in techno, progressive or deep house (G4 per-genre fills below).
  [arrangement#14][arrangement#15][house#2][trance#16]

**Riser.** White noise plus 1–3 synth layers whose pitch or HPF sweeps up
over 4–16 bars, ending exactly on the drop downbeat. Noise riser recipe:
white noise → BP or LP with Q 2–4, centre 300 Hz → 8–12 kHz over 8 bars (or
7 of 8, holding the last), HP rising 100 Hz → 1 kHz alongside, level +6 dB
across. 8 bars at 128 BPM is the common unit. Osborn's riser keeps the
fundamental and cuts lows progressively. [arrangement#2][arrangement#6][arrangement#12][sound-design#45][sound-design#46][sound-design#47]
(Q, end frequency and level: engine default [sound-design])

**Impact.** Crash, splash or "faller" (falling white-noise hiss) on the drop
downbeat and on every section downbeat that follows a riser.
[arrangement#12][arrangement#17]

**Filters.** Build: HPF on the music bus rising to remove the bottom end
just before the drop. Breakdown: LPF on pads opening over the section (Kronos:
32 bars). Reset both instantly at the drop. [arrangement#9][arrangement#12]

**Pre-drop gap.** 2 beats to 1 bar of silence with reverb tails ringing;
optionally a single snare hit on the last downbeat; psytrance ≈ 4 beats.
Variants: Good Life cuts all but the piano 2 beats before; Madeon's "micro
vacuum" plays a bass slide and kick alone on beat 1 and withholds the groove
until beat 2. House: cut all drums for exactly 1 bar before a drop, p = 0.5
per drop. [arrangement#2][arrangement#9][arrangement#12][arrangement#16][arrangement#21][house#13]

**Drop.** The full bass returns exactly on the drop downbeat; optionally a
1-beat downward bass slide on beat 1, groove on beat 2. The drop is the
statistical peak: loudest and thickest. Sidechain on. [arrangement#2][arrangement#3][arrangement#6]

**Fake-out** (10 % of tracks). The build cuts 1 bar early; then a single-kick
bar or a filtered "false drop" of 8 bars, then the real drop. [arrangement#16]

**Second drop** changes at least one layer against the first (new bass
pattern, drum loop or an added top layer); the second build is longer and
the second drop denser. [arrangement#2][arrangement#13][arrangement#23][arrangement#25]

**Phrase-end fills and markers.**

| Device | Rule | Source |
|---|---|---|
| Kick drop-out | mute K for the last 1–2 bars or 2 beats before a turnaround; at most once per 16 bars | [arrangement#14][arrangement#15] |
| Extra snare | one extra snare hit at the end of each 4-bar block | [arrangement#15] |
| Snare climb | on bar 7 of each 8 (DnB) | [arrangement#20] |
| Off-time roll | at each 16-bar loop end | [arrangement#17] |
| Pattern change | a break-pattern change at the end of a 16-bar section as a "mini fill" | [arrangement#16] |
| Bass drop-out | mute B for 1 bar, return 1 bar before the turnaround | [arrangement#19] |
| Reverse | a full-mix reverse into a section (Tiga 121 → 129); reverse cymbal | [arrangement#9][arrangement#18] |
| Variety | never the same fill type at two consecutive boundaries; the loudest fill goes in the bar before the drop | [arrangement#37] |

**Forecast a peak by subtraction.** Filter-shift the drum loop 8–16 bars
before; remove a top layer 4 bars before and the drum loop 1 bar before;
everything back on the downbeat. [techno#26][arrangement#1]

### G5. Energy

- **Within a track:** rises stepwise across the first ~60 %; dips at about
  50–65 % (main breakdown); peaks at the drop that follows; decays over the
  outro. Dancers move less in the breakdown and more at the drop.
  [arrangement#2][arrangement#9][arrangement#21][arrangement#34]
- Future Music's map of a ~256-bar track: mini breakdown at 65, main
  breakdown at 129, drop to outro beats at 193; "the first three quarters …
  consists of build-ups (with intermittent breaks)". [arrangement#9]
- Peak pattern: intro → groove → main 1 → breakdown → build → drop/main 2 →
  outro; two build/drop pairs, the second more intense.
  [arrangement#2][arrangement#25]
- **Mapping E to the mix** (engine default): E sets layer count through the
  G3 table (E 0.2 ≈ 1–3 layers, 0.5 ≈ 4–6, 0.9–1.0 ≈ 8–12), the music-bus LPF
  (closed below E 0.3), hat/percussion density and the pump depth (0 in
  breakdowns). The per-genre E values below are engine defaults; their
  shapes are cited.
- **Tension curve for checking a plan:** Farbood's trend model (G9.9).
- **Over a set** (radio/playlist mode): move energy by at most one step per
  track on a 1–10 scale (5 → 7 is "too abrupt"; dips like 6 → 4 → 6 are
  fine); open at 50–60 % of maximum; save the top two levels for a climax;
  famous sets average 6.2–7.5. Butler's set shape: relaxed → build →
  calmer after the climax → second climax. [arrangement#8][arrangement#32]

### G6. DJ intros, outros and track-to-track transitions

- **Intro:** 16 or 32 bars of drums plus at most one other non-melodic layer
  (kick, hats/percussion, minimal bass, light FX; no full melody or vocal).
  Deconstructed intros: 8 bars (Reese, Mills, Lone), 16 (Tiga, Disclosure,
  Special Request), 24 (Tessela), 48 bars of groove before the first riser
  (Âme). [arrangement#13]–[arrangement#20][arrangement#33]
- **Outro:** mirror the intro, removing layers down to kick + percussion over
  the last 16–32 bars (psytrance: kick + bass; Âme: kick + two shakers; Lone:
  percussion only; Disclosure: LPF on the kick over the last 4).
  [arrangement#13][arrangement#16][arrangement#17][arrangement#21]
- Never put a gap or off-grid turnaround in the first or last 32 bars (it
  knocks the DJ's intro or outro out of place). [arrangement#10]
- **Extended vs radio edit:** radio edits run 3–5 min and cut intros, outros
  and builds; extended versions are about double. [arrangement#27][arrangement#28][arrangement#35]
- **Between generated tracks** (radio mode): overlap 16 bars (32 for a double
  drop) with a bass swap at the midpoint (only one track's bass at a time).
  Transition type by Markov chain (rows = previous type; columns relaxed /
  rolling / double drop): after relaxed 0 / 0.7 / 0.3; after rolling
  0.2 / 0.8 / 0; after double drop 0.2 / 0.8 / 0. Double drop: both tracks
  cued 16 bars before their drops, 16-bar fade-in, 32-bar fade-out. Rolling:
  outgoing cued 32 bars before its high→low boundary, incoming 16 before its
  drop. Relaxed: outgoing 16 before high→low, incoming from the top.
  [arrangement#8]
- **Key chaining** (Camelot): same code, ±1 number, or A ↔ B at the same
  number; "energy boost" +2 numbers at most every 20–30 minutes. DJs rarely
  re-pitch (2.5 % of tracks, 94.3 % of those by 1 st).
  [arrangement#7][arrangement#30][arrangement#31]

### G7. Mix

**Sidechain pump.** Fastest attack (≤ 6 ms; under 10 ms); up to 6 dB from a
kick key before pumping becomes audible, more for deliberate pumping; a
silent click key allows faster releases. [sound-design#29][sound-design#30]
Release ≈ one 8th note minus the 30 ms hold (200 ms at 128 BPM, 180 ms at
140) so gain is back by the off-beat (formula: engine default
[sound-design]). Engine `pump.depth` d → gain reduction 20·log10(1 − d).

| Genre | Pump depth | Source |
|---|---|---|
| house (Chicago, deep) | −3 to −4.5 dB (0.3–0.4) [sound-design]; 2–6 dB [house] | both engine default; [house#28][sound-design#29] |
| French/filter house | −6 to −9 dB (0.5–0.65) [sound-design]; 6–12 dB [house] | both engine default; [house#28] |
| trance | −4 to −6 dB (0.35–0.5); uplifting pads 4–8 dB on every beat | engine default [sound-design#29][trance#3][trance#17] |
| hard trance (rolling bass) | ≥ 6 dB | engine default [trance#15][trance#43] |
| techno | −2 to −3 dB (0.2–0.3) | engine default [sound-design#29] |
| DnB, dubstep, trap | 0 to −2 dB (≤ 0.2) | engine default [sound-design#29] |
| lo-fi | melodic parts and bass ducked 2–4 dB, "almost universal" | [ambient#29] |

- Sidechain bass and pads to the kick in drop and core sections; reduce or
  disable it in breakdowns; restore at the drop. [arrangement#2][arrangement#29][arrangement#32]
- Alternative key: a separate MIDI pattern on every beat (Darren Tate) rather
  than the kick. [trance#3]

**Glue** (SSL-type bus): 2:1 (4:1 for techno and big room), attack 10–30 ms
(30 common), release 100–200 ms or auto, 1–4 dB gain reduction.
[sound-design#31][sound-design#32]
**Tape:** drive 0.1–0.3 on house, deep house and lo-fi; 0–0.1 on trance and
big room. (engine default) [sound-design]

**Loudness.** Master to a per-genre integrated target with true peak ≤ −1
dBTP (≤ −2 dBTP if louder than −14 LUFS). Medians of 900 released tracks
[sound-design#35]; targets [sound-design#34][sound-design#35]:

| Genre | Median LUFS (LRA) | Target LUFS |
|---|---|---|
| dubstep | −6.0 (6.0) | −6 to −7 |
| DnB | −6.5 (6.8) | −6 to −7 |
| hard techno | −7.3 (4.5) | −7.5 |
| big room | −7.4 (5.4) | −7.5 |
| trance | −7.5 (4.8) | −7.5 |
| psytrance | −7.9 (4.9) | −7.5 |
| breaks / UK bass | −8.1 (4.8) | engine default: −8 |
| techno peak time / deep-hypnotic | −8.6 (5.6) / −8.7 (3.2) | −8.5 |
| tech house / house | −8.9 / −9.2 | −9 |
| progressive / deep / melodic house & techno | −10.1 / −10.3 / −10.5 | −10.5 |

Genres the table does not cover (synthwave, electro, Italo, ambient,
downtempo, lo-fi): engine default −10 LUFS (synthwave, electro, Italo),
−14 LUFS (ambient, downtempo, lo-fi, the streaming reference level
[sound-design#34]). Spotify normalises to −14 LUFS. [sound-design#34]

**Frequency allocation.** Tune the kick to the bass root (or its 5th); when
the bass carries the sub, shorten the kick's sub tail to 150–250 ms; keep the
bass mono below 120 Hz [sound-design#36] (ms and Hz engine default).
High-pass corners: pads and chords 150–250 Hz; leads 200–300 Hz; hats
300–500 Hz; reverb return 200 Hz; delay return 280 Hz. (engine default)
[sound-design]
**Breakdowns:** reverb RT60 3–5 s on pads, pump off. (engine default) [sound-design]
**Richness over dissonance:** when a passage needs "more", raise spectral
richness (β +0.22 on pleasure) rather than sensory dissonance (β −0.25).
[cognition#16]
**Analogue drift:** ±3 cents pitch and ±5 % cutoff variance per voice, fixed
per voice. [sound-design#44]

### G8. Shared sound recipes (referenced by the profiles)

| Key | Recipe | Source |
|---|---|---|
| `kick808` | rest 49–56 Hz; jump ×2.0–2.2 for 6 ms then drop back abruptly; slow sag 1–3 % over 300 ms (unverified size); no noise; click = 1 ms pulse through LP. Decay 0.3–0.6 s (electro, Miami, house), 1–3 s tuned to the bass root (trap, "808 bass") | [sound-design#1][sound-design#2][sound-design#8] (decays engine default) |
| `kick909` | waveshaped saw; pitch env ×3–4 → 50–60 Hz, τ 8–12 ms; decay 0.3–0.5 s (house), 0.2–0.35 s (techno, with distortion); click = LP noise + 2 ms pulse | [sound-design#1] (numbers engine default) |
| `hats808` | six squares 205.3, 304.4, 369.6, 522.7, 540, 800 Hz, 48 % duty; cymbal BP 3440 + 7100 Hz, three envelopes, HP each, resonance 10.5 kHz; closed 40–60 ms, open 300–500 ms, closed chokes open | [sound-design#3][sound-design#5] (decays engine default) |
| `hats909` | dense inharmonic/noise source requantised to 6 bits at a 30 kHz clock, LP after the VCA; closed 30–80 ms, open 250–600 ms | [sound-design#5][sound-design#6] (decays engine default) |
| `snare909` | two waveshaped tonal partials; noise LP'd then split into an HP narrow band and an unfiltered path, e.g. 180 ms and 80 ms decays | [sound-design#4] (times engine default) |
| `lofiPCM` | 707/727/LinnDrum: 8-bit (cymbals 6-bit), 25–35 kHz sample-and-hold, decay applied after quantisation | [sound-design#37]–[sound-design#40] |
| `gatedVerb` | RT60 1.5–2.5 s, gated by the dry snare, hold 400–500 ms, release < 20 ms; compress before the reverb; shorter hold for hats | [sound-design#33][synthwave#25][synthwave#26] (RT60 engine default) |
| `tb303` | saw (square darker); 24 dB diode ladder with soft knee, ~10 Hz HP in the feedback loop; base cutoff 200–800 Hz, max ≈ 2.5 kHz before env; res 0.6–0.95; env mod 0.4–0.9 (floor > 0); normal decay 200–2000 ms, accent decay 200 ms; slide 60 ms; amp: 4 ms delay, 3 ms attack, 8 ms hold + 8 ms linear fade at note end; accent cap charge τ ≈ 150 ms, discharge ≈ 100 ms, cumulative; normal gate 7/12 of the step, slid notes tied | [sound-design#9]–[sound-design#11][techno#7][techno#8] (ranges engine default) |
| `reese` | two saws (or sines) ±27 cents (subtle) to ±55 cents (heavy), mono, sustain 1.0, release 24 ms; LP 24 dB 300–1200 Hz; optional drive; variant: 2nd saw +12 or +19 st, quieter | [sound-design#25][sound-design#42][dnb#11][dnb#17] (filter engine default) |
| `wobble` | saw + square 1 oct down, LP 24 dB base 150–300 Hz, LFO depth +3 to +5 oct, res 0.3–0.5, triangle/sine LFO synced 1/8T or 1/8, 0.5–1 beat LFO delay | [sound-design#43] (values engine default) |
| `moogBass` | 2–3 saws at 0, −5, +5 cents (or one an octave down), 24 dB ladder 250–600 Hz, filter env decay 150–300 ms at +2 to +3 oct, res 0.2–0.4, glide 0–40 ms, low-note priority | [sound-design#19] (values engine default) |
| `junoBass` | saw + sub 0.5–0.8, Juno HPF position 0 (+10 dB below 150 Hz), LP 400–900 Hz, res 0.1–0.3, decay 200–400 ms, chorus off | [sound-design#44] (values but HPF engine default) |
| `organBass` | M1 "Organ 2": additive 16', 8', 5⅓', 4' (0.5, 1, 1.5, 2 × note), decay 150–250 ms to sustain 0.3, key click; or Syntorial: fundamental + strong 6th partial (≈ 1.5 × level), A 0, D 1.8–1.9 s, S 0, R 0.2 s, 1/8 delay 25 % wet 20 % fb | [sound-design#20][sound-design#21][house#22][house#24] |
| `dxBass` | carrier 0.5; modulators 0.5 (FB 7), 5.0 and 9.0, fast-decaying, velocity-sensitive | [sound-design#23] |
| `junoPad` | saw + PWM (LFO 0.3–0.5 Hz, depth 0.3–0.6), attack 300–800 ms, release 600–1200 ms, LP 1.5–3 kHz; chorus I (0.513 Hz ±0.5 ms) wide/soft, II (0.863 Hz ±1.1 ms) moving, centre 3.5 ms | [sound-design#44] (envelopes engine default) |
| `stab` | one fixed voicing (minor triad, m7, m9) moved in parallel; 1/16 long; on off-16ths; cutoff, reverb, pan automated over 4–8 bars. Patch: 2 saws ±7 cents + square, LP 24 dB 1.5–3 kHz, filter decay 120–200 ms, amp decay 150–300 ms, S 0, reverb send 0.2–0.35, ping-pong 0.75 beat | [sound-design#27][sound-design#28] (patch engine default) |
| `supersaw` | 7 saws; spread at max detune: see contradiction C1; non-linear detune knob (fraction of max spread 0.097 at 0.50, 0.243 at 0.75, 0.393 at 0.94, 1.0 at 1.0); mix law: centre −0.55366x + 0.99785, sides −0.73764x² + 1.2841x + 0.044372 (equal at x ≈ 0.75); HP tracking the fundamental (24 dB per [trance#5]); random phase per voice per note. Lead knob 0.75, pad 0.5, big room 0.94; mix 0.75 | [sound-design#13][trance#5][trance#8] |
| `fmEP` | DX7 E.PIANO 1 in 4-op: tine pair 14:1 (velocity 7, mod decay 300–600 ms) + body pair 1:1 with feedback 6–7; a 1:1 pair detuned ±7 DX units for chorus; velocity → modulation index | [sound-design#23][sound-design#24][synthwave#34] |
| `fmBell` | pairs 3.5:1 (or 3.18:1), one fixed-frequency operator near 320 Hz, carrier decay 2–4 s | [sound-design#23][sound-design#24] (decay engine default) |
| `syncLead` | osc 2 synced to osc 1, ratio swept 1 → 3 over 200–400 ms by the filter env; 12 dB LP | [sound-design#17][sound-design#18] (values engine default) |
| `hoover` | PWM-saw on three oscillators at −12, 0, +12 st; PWM LFO 4–6 Hz, depth 0.6–0.9; fast chorus (Juno-6 I+II: 8 Hz, 25 %); bend down 2–12 st over 100–300 ms; mono, glide 50–150 ms | [sound-design#26][sound-design#15] (rates, bend, glide unverified/engine default) |
| `noiseRiser` | see G4 Riser | [sound-design#45]–[sound-design#47] |
| `tapeEcho` | delay 2.8–5.65 s, feedback 0.7–0.9, wet 40–50 %, high cut in the loop; optional 160 ms echo at ~40 % | [ambient#5] |
| `tapeStage` | wow 0.3–1 Hz ±5–20 cents; flutter 6–10 Hz ±2–5 cents; saturation; high cut 8–12 kHz | [ambient#14][ambient#15][ambient#29] (values design choice) |
| `noiseBeds` | rain (dense high-passed noise grains), birds (FM chirps 1–3 kHz, 50–200 ms), train (low noise, AM 0.5–2 Hz), vinyl crackle (sparse impulses through BP + low pink noise), at −25 to −35 dB (crackle −30 to −40 dB under drums) | [ambient#9][ambient#14][ambient#27][ambient#29][ambient#40] (design choice) |

### G9. The surprise and expectation model

The generator proposes candidates per phrase and keeps one whose surprise
profile lands in the genre's band. This section is the whole model: inputs,
formulas, order, smoothing, calibration, scores, targets, search.

#### G9.1 Why: the findings the model rests on

| Finding | Number | Engine reading | Source |
|---|---|---|---|
| Huron ITPRA: correct predictions are rewarded on every event; surprise pleases through contrastive valence (negative reaction, then neutral/positive appraisal) | — | predictable events earn a small reward always; surprises earn a larger one only in a safe context that resolves, so surprise is rationed and placed | [cognition#11][cognition#12] |
| Berlyne: hedonic value peaks at moderate arousal potential (novelty, complexity, surprisingness) | inverted U | neither the argmax sequence nor the random one | [cognition#13] |
| Cheung et al. 2019 (745 Billboard songs, 80,943 chords; 39 raters, 40 fMRI) | β: uncertainty −0.143, surprise −0.327, U×S −0.124, dissonance −0.251, spectral complexity +0.224; marginal R² 0.476; a saddle | slope of pleasure w.r.t. surprise = −0.327 − 0.124·U (U in SD): −0.20 at U = −1, −0.33 at 0, −0.45 at +1; zero only near U ≈ −2.6 (derived). Surprise is cheapest where uncertainty is lowest | [cognition#16] |
| Gold et al. 2019 (55 melodies, 43 raters) | liking vs mean IC: inverted U, quadratic β −0.09 / −0.18; vs mean entropy β −0.06 / −0.25; peaks at IC z ≈ −1.2 (study 1) and ≈ 0 (study 2), entropy z ≈ −0.6 to −0.75 (derived); in low-IC melodies high entropy preferred (p = .001); over 7 repetitions liking declines (R² .81) but the preferred complexity does not move | target at or a little below average complexity | [cognition#17] |
| Witek et al. 2014 (50 two-bar breaks, 66 raters) | wanting to move and pleasure: inverted U over the polyphonic syncopation index; vertices 31–35 | target medium syncopation | [cognition#18] |
| Sioros 2014; Matthews 2019 | moderate syncopation (clave level) grooviest; maximal (every note anticipated) less; high harmonic complexity (b9) dampens the rhythm effect | never anticipate every note; keep harmony simple when rhythm is busy | [cognition#21][cognition#22] |
| Margulis | > 90 % of listening time is on heard-before material; exact refrains produce more tapping and singing than varied ones; added repetition made Berio/Carter more enjoyable | repeat the hook exactly; vary the surroundings | [cognition#24] |
| Zipf | chord rank-frequency is a power law; pitch codeword exponent β ≈ 2.18 ± 0.06, stable 50 years | weight vocabulary by corpus frequency; spend the tail deliberately | [cognition#6][cognition#10] |

#### G9.2 Inputs: alphabets and viewpoints

| Model | Alphabet / viewpoints | Source |
|---|---|---|
| Chords | relative root + quality (optionally extension and inversion) as one symbol; single viewpoint, as Cheung used | [cognition#1][cognition#16] |
| Melody and bass | two viewpoints, each its own PPM: pitch interval ⊗ scale degree (pi⊗sd), and ioi-ratio; a note's probability is the joint probability of pitch and onset | [cognition#15][cognition#17] |
| Drums | one PPM per lane over 16-step on/off tokens, conditioned on step position (step index as a linked viewpoint) (proposal) | [cognition] |
| Kit groove | Witek polyphonic index over K, C, H (G9.6) | [cognition#18] |
| Per-layer syncopation | LHL index (G9.6) | [cognition#19][cognition#20] |

Other viewpoints IDyOM supports, if needed: chromatic pitch, inter-onset
interval, sd⊗ioi-ratio. [cognition#15]

#### G9.3 PPM-C with interpolated smoothing

For event e after context s, with c(e|s) the count of e after s, n(s) = Σ_e
c(e|s), t(s) the number of distinct symbols seen after s, and su(s) = s with
its oldest element dropped [cognition#14]:

```text
p(e | s)    = ( c(e|s) + t(s) · p(e | su(s)) ) / ( n(s) + t(s) )   // recurse to order 0
p(e | ε₋₁)  = 1 / |A|                                               // order −1: uniform
IC(e_i)     = −log2 p(e_i | c)                                      // surprise
H(c)        = −Σ_{e∈A} p(e | c) · log2 p(e | c)                     // uncertainty, before the event
```

- Escape method C (Witten–Bell); interpolated smoothing (blend all orders,
  don't back off). Pearce found consistent gains from escape C, interpolation,
  unbounded order (PPM*, longest matching context) *only together with*
  interpolation, and combining LTM and STM. Update exclusion was
  inconsistent; the best order bound depended on the dataset. [cognition#14]
- **Order:** bound 2–4, or unbounded (PPM*). [cognition#14][cognition#16]
  **engine default: order bound 3 for chords and drums, unbounded (PPM*) for
  melody and bass.**
- **Two models per viewpoint:** the **LTM** is trained on the corpus (here,
  the profile's own grammar plus the seed tables of G9.4) and stands for the
  listener's enculturation; the **STM** starts empty each song and learns as
  it plays, which is how a returning hook becomes predictable.
  [cognition#14][cognition#15]
- **Combination** (LTM with STM, and viewpoints with each other), weighted
  geometric mean:

  ```text
  p(e) ∝ ( Π_m p_m(e)^{w_m} )^{1 / Σ w_m},    w_m = ( H(p_m) / log2|A| )^{−b},   b = 2
  ```

  Lower-entropy models get more weight; Pearce's optimum b was 1–4 (up to 32
  made results worse); geometric beat arithmetic. [cognition#14][cognition#16]
- **Calibration:** raw bits depend on alphabet size, so express every IC and H
  as a z-score (or percentile) against the model's own corpus, computed once
  at start-up. Gold's and Cheung's results are in standardised units, so the
  targets below stay comparable. [cognition#16][cognition#17]
- Typical magnitudes for orientation: 2.34 bits/note pitch-only on chorales,
  1.95 for the best multiple-viewpoint system; a well-fitted melody model
  lands around 2–2.5 bits/note on its own style; melodies in Gold's set ran
  2.34 (folk) to 16.52 bits mean IC (atonal). [cognition#14][cognition#15][cognition#17]

#### G9.4 Seed tables for the chord LTM

No published EDM chord-transition corpus exists; GiantSteps gives only
mode and tempo. [cognition#7] So the LTM is trained on (a) each profile's
progression pool, counted by its weights, plus (b) these priors.

**Rock (de Clercq & Temperley, 100 songs), root distribution:** I .328, IV
.226, V .163, ♭VII .081, VI .072, ♭VI .040, II .036, ♭III .026, III .019, ♭II
.005, VII .004, #IV .003. I + IV + V + ♭VII + VI = 87 %. Quality: 75.8 % major,
23.4 % minor, 0.7 % dim, 0.1 % aug; 94.1 % root position. Unigram entropy 2.70
bits (derived). [cognition#1]

**Rock transitions P(next | current)** (derived from Table 3 of [cognition#1]):

| From | H (bits) | Top continuations |
|---|---|---|
| I | 2.51 | IV .36, V .24, ♭VII .16, VI .10, II .04 |
| IV | 1.97 | I .56, V .25, ♭III .05, ♭VII .04, VI .03 |
| V | 1.82 | I .53, IV .26, VI .13, ♭VII .03, II .02 |
| VI | 2.24 | IV .39, I .21, V .18, II .13, III .05 |
| ♭VII | 1.77 | I .53, IV .26, ♭VI .16, V .04 |
| ♭VI | 1.92 | I .57, ♭VII .21, IV .06, V .06 |
| II | 2.23 | I .36, V .29, IV .17, VI .07 |
| ♭III | 2.32 | ♭VI .28, IV .27, I .21, ♭VII .17 |
| III | 2.23 | VI .34, IV .26, II .22, I .09 |

- Root motion: ascending and descending equally likely; frequency falls with
  circle-of-fifths distance: P4 > M2 > m3/M3 > m2 > tritone. [cognition#1]
- Cadence trigrams ending on I (counts over 100 songs): IV–V–I 352, V–IV–I
  292, ♭VII–IV–I 146, VI–IV–I 126, ♭VII–♭VI–I 103, ♭III–♭VI–I 66, II–V–I 63,
  ♭VI–♭VII–I 60. A pre-tonic V is nearly always approached from IV.
  [cognition#1]
- **Pop (Hooktheory)**: unigram I .176, IV .12, V .116, vi .075, ii .033;
  after IV: I .332, V .266, vi .066; after IV→I: V .407, IV .199, vi .101, ii
  .046 (context matters); after iii choose vi or IV ≥ 90 %. [cognition#3][cognition#4]
- **EDM frame:** default to a minor-tonic alphabet (i, ♭III, iv, v, ♭VI,
  ♭VII …). Minor share by genre (GiantSteps, 604 Beatport tracks, 84.6 %
  minor overall): deep house 95 %, dubstep 91 %, electro house 90 %, tech
  house 88 %, trance 88 %, house 83 %, techno 82 %, DnB 82 %, progressive
  house 76 %, breaks 71 %, chill-out 64 %; minimal 11/11 and psytrance 5/5
  (tiny samples). [cognition#7]
- How to transfer the rock table into the minor frame is not in the
  research. **engine default: use the rock table only for its root-motion
  shape (circle-of-fifths ordering, symmetry around the tonic) as a weak prior
  under each profile's own pool; do not relabel I→i.**

#### G9.5 Scores

For a candidate X with events x₁…xₙ [cognition#16][cognition#17][cognition#18][cognition#20]:

- **Surprise profile:** IC_i and H_i (before each event), as z-scores.
- **Mean complexity:** μ_IC = mean z(IC_i); μ_H = mean z(H_i).
- **Placement (Cheung):** Σ_i z(IC_i) · (−z(H_i)): rewards surprises at low
  uncertainty and expected events at high uncertainty.
- **Syncopation:** LHL per layer per bar; Witek per 2-bar kit loop; fitness
  f = −(S − S*)² around the target S*.
- **Repetition:** fraction of bar or chord 4-grams that repeat earlier
  material, plus the STM-vs-LTM IC drop on a hook's return (proposal).
- **Total:** maximise placement, subtract weighted squared distances to the
  targets (μ_IC, μ_H, syncopation, repetition), and treat hard constraints
  (cadence, key, range, repeat ceilings) as filters. Weights: **engine
  default 1.0 each**, tuned by listening.

#### G9.6 Syncopation indices

**LHL (monophonic, per layer).** Metrical weights on the 16-step grid
[cognition#19][cognition#20]:

| Step | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| w | 0 | −4 | −3 | −4 | −2 | −4 | −3 | −4 | −1 | −4 | −3 | −4 | −2 | −4 | −3 | −4 |

A syncopation is a rest or tie R preceded by a sounded note N of lower
weight; value w(R) − w(N), summed over the pattern with wrap. Grid
implementation (derived): for each onset at step i, scan the silent steps up
to the next onset; if the highest weight among them w_max > w(i), add
w_max − w(i). Fitch & Rosenfeld's worked example scores 5 ("moderate").
[cognition#20]

**Witek (polyphonic, per kit loop).** Weights (Figure S5): step 1 = 0; steps
5, 9, 13 = −1 (all non-downbeat quarters equal, unlike LHL); steps 3, 7, 11,
15 = −2; even steps = −3. A note N on one instrument followed by a note on a
*different* instrument at weight ≥ w(N) is a polyphonic syncopation; followed
by a rest at weight ≥ w(N), a monophonic one. Score S = w(following) − w(N) +
I, with instrument weight I = 2 (kick syncopated while snare and hat land on
the stronger position, "three-stream"), 1 (snare syncopated while kick and
hat land on it), 5 (only one other stream on the stronger position,
"two-stream"). Sum over the 2-bar loop with wrap. Worked: "Impeach the
President" 5, "Lifetime Monologue" 41, "Actual Proof" 68. [cognition#18]

**Bins and targets.**
- Witek's stimuli spanned 0–81: low 0–21 (mean 10.8), medium 22–46 (33.5),
  high 49–81 (60.3); vertices for wanting to move 31–35 (musicians 35.2,
  non-musicians 31.2); the polyphonic index explained more than LHL (R² .357
  vs .273 for wanting to move, .430 vs .306 for pleasure). [cognition#18]
- **Groove kits** (breaks, DnB, garage, funk-house): target Witek 22–46 per
  2-bar loop, centre ≈ 33; reject ≥ 49. [cognition#18]
- **Four-on-the-floor:** kick 1, 5, 9, 13 + clap 5, 13 scores 0 on both
  indices; keep that layer at 0 and put syncopation in hats, percussion, bass
  and stabs instead. [cognition#18]
- **Bass:** LHL 2–7 per bar (proposal); 16th-off-beat-only patterns (LHL 15)
  for at most 1 bar in 8. [cognition#20][cognition#22]
- Prefer moderate over maximal syncopation in any layer; never anticipate
  every note. [cognition#21][cognition#22]

**Worked values** (derived) [cognition]:

| Pattern | LHL | Witek (1 bar) |
|---|---|---|
| four-on-the-floor kick 1, 5, 9, 13 | 0 | — |
| off-beat 8ths 3, 7, 11, 15 (house hat, off-beat bass) | 7 | — |
| tresillo 1, 7, 13 | 2 | — |
| pushed bass 1, 4, 7, 9, 12, 15 | 4 | — |
| son clave 3-2: 1, 4, 7, 11, 13 | 4 | — |
| every 16th off-beat 2, 4, …, 16 | 15 | — |
| rock beat (K 1, 9; S 5, 13; 8th hats) | — | 0 |
| house (K 1, 5, 9, 13; C 5, 13; H 3, 7, 11, 15) | — | 0 |
| K 1, 4, 11; S 5, 13; 8th hats | — | 7 |
| breakbeat (K 1, 3, 11, 12; S 5, 8, 10, 13, 16; 8th hats) | — | 20 |

#### G9.7 Targets and placement rules

| Target | Value | Source |
|---|---|---|
| Melody phrase mean IC | z −1.0 to 0.0 vs the melody LTM's corpus | [cognition#17] |
| Melody phrase mean entropy | z −0.75 to −0.25 | [cognition#17] |
| Too predictable / too complex | mean IC below z −1.5 / above z +1 → reject (proposal from Gold's inverted U) | [cognition#17] |
| Never | pick the argmax-probability sequence | [cognition#17] |
| Melodic surprises | 1–2 events per 8-bar phrase with IC above the 80th percentile, each where H is below the 40th percentile and followed by an event with IC below the 40th percentile (proposal, after Cheung) | [cognition#16] |
| High-IC chord | only where the entropy before it is in the bottom 40 %: after a loop has been heard at least twice, at a section end, or at the 4th/8th chord of a repeated 4-chord cycle | [cognition#16] |
| After a surprise, or anywhere H is in the top 40 % | choose a chord from the bottom 40 % of IC | [cognition#16][cognition#17] |
| Section ends | make them low-entropy contexts (a repeated cadence or loop) so a surprise there pays off | [cognition#16] |
| Voicing | prefer acoustically smooth voicings (dissonance β −0.25 independently) | [cognition#16] |
| Rock/pop presets | I + IV + V + ♭VII + VI ≈ 87 % of chord mass; < 1 % diminished; ≈ 94 % root position | [cognition#1] |

#### G9.8 Repetition and return

- Repetition ceiling: a ≥ 5-chord sub-sequence repeats only after a gap of at
  least 2 chords, and at most 3 times in a row (adapted from Cheung's stimulus
  constraint; a selection rule, not a preference finding). [cognition#16]
- Keep exact repetition of refrains and hooks; vary the surroundings, not the
  hook. [cognition#24]
- Let the STM make repeated hooks cheaper. When a returning hook's mean IC
  falls below z −1.5, vary 1 event in 4–8 (pitch or onset) until it is back in
  band (proposal). [cognition#14][cognition#17] — this conflicts with the rule
  above; see contradiction C9. **engine default: first vary the surroundings
  (accompaniment layers, voicing, drum variation); touch the hook itself only
  if the phrase is still below z −1.5, and never in its first two bars.**
- Weight vocabulary by corpus frequency (Zipf-like); spend rare chords,
  sounds and rhythms sparingly, not uniformly. [cognition#6][cognition#10]
- Hooks as hard constraints during refinement (MorpheuS keeps template
  patterns fixed). [cognition#26]

#### G9.9 Tension curve (checking an arrangement plan)

- **Farbood:** trend = weighted slopes of features within a moving attentional
  window: loudness/dynamics and melody pitch height weight 3; tempo and onset
  frequency 2; harmony, bass pitch height and inner-voice pitch 1; weights
  normalised to sum 1. A trend continuing the previous window's direction is
  multiplied by a memory constant b > 1; a decaying filter integrates it,
  sampled every 0.25 s. Melodic expectation did not correlate with tension.
  [cognition#25] **engine default: b = 1.5, decay time constant 4 s.**
- **MorpheuS:** tension in Chew's spiral array (cloud diameter, cloud
  momentum, tensile strain); objective D(x) = Σ_i a_i · ‖T_i(x) − T_i(template)‖₂
  with all a_i = 1; repeated patterns are hard constraints; variable
  neighbourhood search with change1, swap and changeSlice moves, first-descent,
  backtrack 4 slices after each success. [cognition#26]
- Use: compute the plan's tension curve over sections (loudness, onset
  density, pitch height, harmony) and fit the genre's target energy curve
  with D(x). [cognition#25][cognition#26]

#### G9.10 Search

- **Generate-and-test:** sample K = 32–128 candidates per phrase from the
  model at temperature τ ∈ [0.8, 1.2] (exponent 1/τ on p); score; keep the
  best. This avoids greedy sampling's trap (every step likely, the whole not).
  (proposal for K and τ) [cognition#14] **engine default: K = 48, τ = 1.0.**
- **Beam search** over chords: beam 8–16, scored by the running objective,
  not raw likelihood, so it can look ahead to the cadence. [cognition]
- **Refinement:** Metropolis–Hastings (propose from the model's conditional,
  accept with min(1, p(s′)q(t_i)/p(s)q(t′_i)); or with score, min(1,
  exp(Δscore/T))) or VNS first-descent with change-one and swap moves on the
  best candidate; hooks fixed. [cognition#14][cognition#26]

#### G9.11 Per-genre surprise bands (summary)

No study measures per-genre IC targets; every band below is the cognition
default (G9.7) unless the genre's own research gives a reason to move it,
and every move is **engine default**. [cognition] The per-genre "surprise"
field of each profile says where variation goes.

| Profile group | Chord IC band | Melody IC band | Kit syncopation | Notes |
|---|---|---|---|---|
| house, garage, deep | G9.7 | G9.7 | 4/4 layer Witek 0; UKG/2-step Witek 22–46 | [cognition#18] |
| techno (all) | n/a in 60–70 % (one chord) | engine default: z −1.5 to −0.5 (1–4-note riffs) | 4/4 layer 0; percussion LHL per layer | riffs fixed, variation by filter and mutes [techno#2][techno#29] |
| DnB, jungle | G9.7 (liquid); n/a (neuro pedal) | G9.7 | Witek 22–46, reject ≥ 49 | [cognition#18] |
| trance, eurobeat | G9.7 | engine default: z −1.25 to −0.25 (narrow stepwise hook) | 4/4 layer 0 | [trance#29] |
| synthwave, Italo, electro | G9.7 | G9.7 | electro: groove kit 22–46 | [synthwave#16] |
| ambient | n/a (fixed pitch set) | n/a | none | surprise budget ≈ 0 [ambient#3] |
| downtempo, lo-fi | G9.7 | G9.7 | Witek 22–46 | engine default |

---

## Genre profiles

Every profile uses the field list from "How to read this". Engine instrument
names are the crate's (`tb303`, `juno`, `mono`, `fm`, `string`/`bowed`, kits
`tr808`, `tr909`, `latin`, `country`); existing patch names are in
backticks (`acid`, `reese`, `supersaw`, `warmPad`, `ep`, `piano`, `organ`,
`dubChord`, `rumble`, `psyBass`, `tranceBass`, `euroLead`, …); shared recipes
from G8 are in backticks too (`kick909`, `junoPad`, …). "Missing" names what
the profile needs and the engine cannot make yet (collected in "What the
engine lacks").

Unless a profile says otherwise, it inherits the global rules G1–G9.

### House family

#### P1. Chicago house

- **scope:** 80s Chicago, 21st-century Chicago, "bumpin'" house; garage house
  (NY/NJ) and Italo/piano house as variants (harmony and keys only).
- **tempo:** 118–126, core 120–125, typical 123; garage house 118–124.
  [house#1][house#4][house#5][house#6][house#18]
- **swing:** 80s mode 50–55; modern 55–60; never above 70 in house modes.
  [house#5][house#6][house#11]
- **meter/grid:** 4/4, 16 steps, 2-bar drum loops, 8-bar sections.
  [house#1][house#6]
- **drums** (kit: `tr909`, or a 707 kick tuned −2 st; 909 hats with a short
  open decay choked by the closed hat; 909 + 707 clap layer [house#5][house#19][house#20]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13; bar 1 of 80s loop + 15 | 1.0; 15 at 0.6–0.7 | bar 2: a quiet ghost kick mid-bar (step not given; engine default: 11 at 0.4); turnaround kick on 15 of the last bar of each 4- or 8-bar phrase at 0.6–0.7 | [house#5][house#6] |
  | Snare | 5, 13 | 1.0 | — | [house#6] |
  | C | 5, 13, + 16 with p = 0.5 | 1.0; 16 at engine default 0.6 | clap layered on the snare | [house#6] |
  | Closed H | all 16 | odd 1.0; even 0.55–0.75 of odd, ±10 % | — | [house#5][house#6] |
  | Open H | 3, 7, 11, 15 | 1.0 | choked by the closed hat | [house#5][house#6] |
  | Rim | 2, 4, 6, …, 16 | engine default 0.35 | — | [house#6] |
  | Tambourine | 3, 7, 11, 15 | engine default 0.6 | two detuned copies hard L/R | [house#5] |
  | Toms ("bumpin'") | low tom 15 of odd bars; mid toms 2–3 steps earlier | engine default 0.6 | ride once at the start of the 2-bar loop; syncopated kick at the end of bar 2 | [house#7] |
  | Crash | step 1 of bar 1 of each new section | 1.0 | — | [house#9] |

  Kit Witek target: 0 on K + C; syncopation lives in the rim, tambourine and
  bass. [cognition#18]
- **bass:** grammar: no transcribed Chicago bass rhythm; tracks are built on
  bass-heavy loops [house#1]. engine default: 1-bar cells from {off-beat root
  `..r.`, root + octave, pickup into the next chord's root on beat 4},
  LHL 2–7 per bar [cognition#20]. Register: engine default root in MIDI 29–40,
  kick tuned to the root [sound-design#36]. Articulation: short plucks, the
  bass never muted more than 8 bars outside breakdowns and outros (engine
  default) [house#1][house#13]. Sound: mono pulse, saw off, sub 0, cutoff at
  minimum opened only by an envelope at full depth (decay ~35 % of range,
  sustain 0, release ~33 %), 70 % velocity and key tracking, chorus wet ~90 %
  (u-he on a Juno-60 model) [house#26]; "Move Your Body" used a JX-8P [house#42].
- **harmony:**
  - modes: minor 0.83, major 0.17 (GiantSteps house) [cognition#7].
  - progressions (4-bar loops; weights engine default, Zipf-ordered
    [cognition#6]): i7–v7–iv7 0.25; iv7–VImaj7–i7 0.2; VImaj7–v7 0.2; i7–iv7
    0.2; i7 vamp 0.15 [house#15]. Garage-house variant adds ii7–V7–Imaj7,
    IVmaj7–iii7–vi7 and a passing V7/ii (engine default; the sources only
    establish "gospel-influenced piano") [house#38].
  - extensions: min7 default; 9th p = 0.6, 11th p = 0.3; maj7 on VI and IV;
    keep the 7th in every voicing, drop other notes first; drop-octave 7ths so
    the top voice moves by step. [house#15][house#48][house#49]
  - bars_per_chord: 1 (engine default); loop_bars: 4. [house#15]
- **melody/hooks:** a 1–2-bar piano or synth riff repeated unchanged for
  16–32 bars [house#1][house#42]; early Chicago often used a clave-based riff
  [house#1]. Piano-house variant: M1-style bright piano chords on syncopated
  8ths and 16ths, doubled an octave down (the "16'" layer, inferred from the
  preset name) [house#21][house#41][house#46]. Density engine default 3–6
  onsets/bar; range ≤ 12 st; repetition exact (G9.8).
- **sounds:**

  | Role | Engine | Recipe | Source |
  |---|---|---|---|
  | K, C, H | `tr909` (+ `lofiPCM` for 707 character) | `kick909` house decay 0.3–0.5 s; 8-bit/6-bit crush on hats and cymbals, light tape | [house#6][house#19][house#20] |
  | B | `juno` mono or `mono` | Chicago bass above | [house#26] |
  | Pd (stabs) | `juno` | `junoPad`, `stab`; Juno-style poly for Chicago stabs | [house#2][house#16][house#25] |
  | Piano/organ | `fm` `piano`, `organ` | M1 "Piano 16'" stand-in, non-resonant filter | [house#21][house#22] |
  | Strings | `juno` | half the piano line through a string patch ("Move Your Body") | [house#42] |

  Missing: TR-707/727 sample-character stage; M1 piano and organ stand-ins
  (an FM piano exists, not an M1); a tambourine lane; per-copy detune and pan.
- **arrangement** (club/extended, 124–128 BPM in the source, 192 bars ≈ 6:00)
  [arrangement#9][arrangement#12][arrangement#13][arrangement#33]:

  | Role | 1–16 | 17–32 | 33–48 | 49–64 | 65–80 | 81–104 | 105–112 | 113–144 | 145–160 | 161–176 | 177–192 |
  |---|---|---|---|---|---|---|---|---|---|---|---|
  | K | x | x (out bar 32 beats 3–4) | x | x | x | - | - | x | x | x | x (LPF 189–192) |
  | H | x | x | v (+2nd hat) | x | x, out 73–80 | - | - | x | x | x | x |
  | C | - | + | x | x | x | +89 | x | x | x | x | - |
  | P | +9 | x | x | x | x | - | - | v (+extra P) | x | x | x |
  | B | - | f | x | x | x | - | - | x | x | x | - |
  | Pd | - | - | f (LPF stabs) | x | x | x (LPF opens; loop changes every 8) | x | x | x | - | - |
  | L/V | - | - | - | x | x | V | - | x | x, out 153 | - | - |
  | R | - | - | - | - | r 77–80 | - | r | - | - | - | - |
  | Sn | - | - | fill 48 | - | - | - | r: ¼ 105, ⅛ 109, 1/16 111, 1/32 112, silent last 2 beats | - | - | - | - |
  | X | - | - | - | - | - | - | - | 1 | - | - | - |

  intro 16 (DJ) + 16; outro 16 + 16 (DJ); total 192. Classic Chicago/acid
  option: 6–12 min (engine default 256–384 bars) with slow layer changes, no
  roll, built by adding layers and raising the level. [house#1][house#18]
- **energy** (engine default; shape [arrangement#2][arrangement#9]): 0.3, 0.45,
  0.6, 0.75, 0.8, 0.3, 0.55→0.85, 1.0, 0.9, 0.6, 0.4.
- **surprise targets:**
  - turnaround kick on step 15 of the last bar of each 4- or 8-bar phrase at
    0.6–0.7 [house#5][house#6]; low tom on 15 of odd bars, mid toms 2–3 steps
    earlier ("bumpin'") [house#7];
  - drum variation: hat mutation rate engine default 0.1 per block; rim and
    tambourine swap at 16-bar boundaries;
  - 1 bar of silence before a drop, p = 0.5 [house#13]; crash on bar 1 of each
    section [house#9];
  - hook: exact repeats; chord IC per G9.7.
- **references:** Jesse Saunders "On & On" (1984); Steve "Silk" Hurley "Music
  Is the Key", "Jack Your Body" (1986); Marshall Jefferson "Move Your Body"
  (1986); Frankie Knuckles feat. Jamie Principle "Your Love" / "Baby Wants to
  Ride" (1987); Phuture "Acid Tracks" (1987); Robin S "Show Me Love"
  (StoneBridge remix, garage/organ bass); Black Box "Ride on Time" (Italo
  piano house); Disclosure "Bang That" (modern arrangement reference).
  [house#1][house#18][house#22][house#41][house#43][arrangement#13]

#### P2. Deep house

- **scope:** Chicago/NY/NJ deep house from 1985, "rolling deep house".
- **tempo:** 115–125, typical 122 (Beatport median 122, IQR 121–123); Attack's
  beat 125. [house#2][house#8][cognition#8]
- **swing:** 58–68 (Attack: 60–70). [house#8]
- **meter/grid:** 4/4, 16 steps, 1-bar loops; every hit short, no overlaps.
  [house#8]
- **drums** (kit `tr909` samples, kick tape-saturated [house#8]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | none | [house#8] |
  | C | 5, 13 | engine default 0.8 | — | [house#1] |
  | Closed H | 16ths | engine default 0.5–0.8; step 10 extra quiet (engine default 0.3) | shaker shares the role, may share a choke group | [house#8] |
  | Tom (909) | 3, 9, 11 | engine default 0.6 | "reinforces the shuffle and jacking feel" | [house#8] |
  | Perc (optional, afro) | 2, 10 | engine default 0.5 | tuned against the tom | [house#8] |

- **bass:** grammar: an arpeggiated or repeating 1-bar figure synced to the
  drum clock (the "Can You Feel It" line has been recreated by clocking the
  Juno-60's arpeggiator from the 909) [house#16][house#17]. Register: sub-octave
  (engine default root MIDI 29–40). Articulation: short decay, "hollowed-out",
  "muted" [house#2][house#16]. Sound: `junoBass` (saw + sub, Juno HPF position
  0, LP 400–900 Hz, decay 200–400 ms, chorus off) [sound-design#44], or the
  pulse/square low-cutoff Chicago bass [house#26].
- **harmony:**
  - modes: minor 0.95, major 0.05 (GiantSteps deep house 73/77) [cognition#7].
  - progressions (4-bar loops; weights engine default): i7–v7–iv7 0.25 (Am7
    voiced with its 7th dropped, top line E–D–C); iv7–VImaj7–i7 0.25 (MAW "To
    Be In Love"); VImaj7–v7 0.2 (= IVmaj7–iii7 of the relative major); i7–iv7
    0.15; i7 vamp 0.15. Max 3 chords per loop. [house#15][house#16]
  - extensions: m9 "the one main chord" (stab 0, +3, +7, +10, +14 st, inner
    voices up an octave for brightness); 9th p = 0.6, 11th p = 0.3, maj7 on VI
    and IV; keep the 7th; clustered close voicings. [house#15][house#16][house#48][house#49][house#50]
  - bars_per_chord: 1–2 (engine default); loop_bars: 4. [house#15]
- **melody/hooks:** chords and bass are the hook; melodic lead optional, p ≈
  0.3 (engine default) [house#2]; "smooth three-chord synth line" with
  clustered notes [house#16]. Density engine default ≤ 3 onsets/bar.
- **sounds:** pads `junoPad` (chorus on), `warmPad`; keys `fm` `ep` (`fmEP`);
  stabs `juno` m9; bass `junoBass`; ambient reverb on chords and percussion
  [house#2][house#16][sound-design#44]. Missing: Juno HPF 4-position switch
  and correct chorus depths ("What the engine lacks", #18).
- **arrangement** (deep house: no build-and-release emphasis, at most one
  breakdown, changes by layer muting only [house#2]; template after Âme
  "Rej" [arrangement#17]; bar counts after bar 48 engine default):

  | Role | 1–16 | 17–32 | 33–48 | 49–56 | 57–104 | 105–112 | 113–160 | 161–176 | 177–192 |
  |---|---|---|---|---|---|---|---|---|---|
  | K | x | x | x | x | x | x | x | x | x |
  | B | x | x | x | x | x | x | x | x | - |
  | P (shaker, perc) | x | v (+2nd shaker) | x | x | x | - | x | x | x (two shakers) |
  | H (open) | - | - | + | x | x | - | x | x | - |
  | Atm (drone) | - | + | x | x | x | - | x | - | - |
  | Pd | - | - | s (synth pulses) | x | x | - | x | - | - |
  | L (p 0.3) | - | - | - | - | x | - | x | - | - |
  | R | - | - | - | r | off-time roll at each 16-bar end | r 109–112 | - | - | - |
  | X | - | - | - | - | 1 (57, splash) | - | 1 (113) | - | - |

  intro 48 bars of building groove before the first riser; breakdown ≈ 8 bars
  of kick + bass only; outro last 16 bars kick + two shakers; total 192 (engine
  default). [arrangement#17]
- **energy** (engine default; flat, slowly rising, one shallow dip): 0.4, 0.45,
  0.55, 0.6, 0.7, 0.45, 0.75, 0.55, 0.4.
- **surprise targets:** no fills or percussion dropouts; synths mark changes
  [arrangement#17]; variation goes into chord voicing and hat/perc mutation;
  an off-time roll at each 16-bar loop end [arrangement#17]; melody IC per
  G9.7, chord surprises only at loop ends (G9.7).
- **references:** Mr. Fingers "Mystery of Love" (1985), "Can You Feel It"
  (1986); Masters At Work "To Be In Love (MAW Dub)" (1997); Âme "Rej"
  (deep house/techno arrangement). [house#2][house#15][house#16][arrangement#17]

#### P3. French / filter house

- **scope:** French touch, filter house (Roulé, Daft Punk, Cassius, Stardust).
- **tempo:** 115–128 (sources: 110–130), typical engine default 124. [house#3]
- **swing:** 52–58 (engine default). [house]
- **meter/grid:** 4/4, 16 steps; the "disco loop" is 1 or 2 bars (engine
  default). [house#3]
- **drums** (kit `tr909`/`tr808`; *Homework* used TR-707, 808, 909
  [house#30]): steady four-on-the-floor with a heavy kick [house#3][house#32].

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | keys the bus sidechain | [house#3][house#28] |
  | C | 5, 13 | engine default 0.9 | — | [house#1] |
  | Open H | 3, 7, 11, 15 | engine default 0.8 | — | [house#1] |
  | Closed H | 16ths, even steps 0.55–0.75 | engine default | — | [house#5] |

- **bass:** a syncopated octave bass inside the loop bus (engine default; the
  Stardust loop has a Korg Trident bassline [house#27]). Register engine
  default MIDI 33–45. Sound: `moogBass` with short decay (engine default).
- **harmony:** no progression is transcribed. engine default: 1–2 chords per
  loop (maj7/m7, Rhodes voicing), loop_bars 1–2, modes minor 0.6 / major 0.4;
  the samples come from late-70s/early-80s European disco [house#3].
- **melody/hooks:** the filtered loop bus is the hook; no separate lead in
  ≥ 60 % of tracks (engine default). [house#3][house#32]
- **sounds:** the **loop bus** (engine default, from the cited ingredients):
  `fm` `ep` (Rhodes) chords + syncopated octave bass + muted funk-guitar pluck
  (`string` `rhythmGuitar`/`wahGuitar`) + `juno` string or brass stab, summed to
  a bus → resonant LPF with cutoff automated 200 Hz → 12 kHz → phaser →
  kick-keyed sidechain (ratio ≥ 10:1, release ~1/8 note). [house#3][house#27][house#28][house#29]
  Pump 6–12 dB [house] or −6 to −9 dB [sound-design] (both engine default).
  Missing: bus routing with its own filter automation and sidechain, phaser,
  per-bus compressor (Alesis 3630 character [house#28][house#30]).
- **arrangement** (engine default bar counts; filter opening over 8–16 bars is
  unverified [house#3]):

  | Role | 1–16 | 17–32 | 33–48 | 49–64 | 65–80 | 81–96 | 97–112 | 113–144 | 145–160 | 161–176 |
  |---|---|---|---|---|---|---|---|---|---|---|
  | K | x | x | x | x | x | - | x | x | x | x |
  | H, C | x | x | x | x | x | - | x | x | x | x |
  | Loop bus (B + keys + guitar + stab) | - | f (LPF 200 Hz) | f (opening) | x | x | f (LPF closed, no pump) | f → x (opens 16 bars) | x | f (closing) | - |
  | L (p ≤ 0.4) | - | - | - | - | x | - | - | x | - | - |
  | X | - | - | - | 1 | - | - | - | 1 | - | - |

  intro 16; outro 16–32; total 176 (engine default).
- **energy** (engine default; follows the bus cutoff): 0.35, 0.45, 0.6, 0.85,
  0.9, 0.35, 0.6→0.9, 1.0, 0.6, 0.4.
- **surprise targets:** one "filter dip" (cutoff < 400 Hz) for 1–2 bars before
  a section change, p = 0.5 (engine default) [house#3]; the cutoff sweep is the
  main motion; loop content stays fixed (exact repetition).
- **references:** Daft Punk "Da Funk" (1995), "Indo Silver Club" (1996),
  "Around the World", "Burnin'" (1997), "Revolution 909" (1998); Stardust
  "Music Sounds Better With You" (1998); Cassius "Foxxy", "Cassius 1999",
  "Feeling for You"; Modjo "Lady"; Motorbass *Pansoul*. [house#3][house#27][house#31][house#32][house#33]

#### P4. UK garage / 2-step (speed garage as a mode)

- **scope:** UK garage 4x4, 2-step, rolling 2-step; speed garage as a mode;
  future garage noted in P24's survey.
- **tempo:** UKG/2-step 128–138, typical 132–134 (sources: ~130; 124–132;
  122–128 rolling; 132; 136; > 138 leans speed garage); speed garage 128–134.
  [house#4][house#9][house#10][house#13][house#14][house#36]
- **swing:** 2-step 60–69; UKG 4x4 62–69 (MPC16 swing 68–69). [house#9][house#10]
- **meter/grid:** 4/4, 16 steps, 2-bar drum loops; swung 16ths carry the
  hats. [house#9][house#13]
- **drums** (sounds: MPC, Drumtrax and LinnDrum [house#9]; a deep 808 kick
  with a 909/707 layer [house#10]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K (2-step) | bar 1: 1, 11; bar 2: repeat, or a double kick (1 and 2, second quieter), or drop beat 1 (p = 0.3) | first kick of each bar +10 %; second of a double ≈ 0.6 (engine default) | kick +3 st, short decay; fast double kick before the downbeat (16 + next 1, or 1–2) once per 4 bars | [house#9][house#10][house#13][house#14] |
  | K (4x4 / speed) | 1, 5, 9, 13 | 1.0 | — | [house#4][house#36] |
  | Snare/rim | rim A on 5, 13; rim B (+2 st) and A on 1–3 syncopated steps chosen per 2 bars | velocities alternate between the two rims | clap/rim fill at the end of every 2nd bar at 0.6–0.7 | [house#9][house#14] |
  | Open H | 3, 7, 15 + swung hits after 8 and 10 (NI); or every off-beat (rolling, Producer School) | engine default 0.7 | 50 %: cabasa/shaker on the off-beats in place of the open hat; soft-attack closed hat avoids a sharp off-beat | [house#9][house#10][house#13][house#14] |
  | Closed H | 1, 9 + swung hits after 2, 4, 6 (NI); or syncopated 16ths | engine default 0.5–0.8 | — | [house#13][house#14] |
  | Shaker / 16th tambourine | all 16 | 0.3–0.8, random accents | the "roll" | [house#10] |
  | Crash | bar 1 of a section | 1.0 | — | [house#9] |
  | Speed-garage roll | 16ths, 1 bar before section changes | rising | — | [house#4][house#36] |

  Kit Witek target 22–46 per 2-bar loop, centre 33. [cognition#18]
- **bass:** grammar: a 3-note line over 4 bars looped to 8, with an opening
  LPF, on a "Dred"-style bass (NI, A minor) [house#13]. Speed garage: sub plus
  a warped, sweeping mid layer (filter or pitch LFO over 1/4–1 bar) with
  reverse-envelope "spin-up" notes (engine default from descriptions)
  [house#4][house#36]. Register engine default root MIDI 28–40. LHL 2–7.
- **harmony:** minor keys; chords that "conjure positive emotions": maj7/add9
  colours on VI and III [house#13][house#35]. Progressions engine default:
  i7–VImaj7 0.3; VImaj7–v7 0.25; iv7–VImaj7–i7 0.25; i7–IIImaj7 0.2.
  bars_per_chord 1–2; loop_bars 4. Modes: minor 0.85, major 0.15 (engine
  default).
- **melody/hooks:** vocal-chop hook: 1–3-syllable synthetic vowel grains
  re-pitched to chord tones, on swung 16ths; reverse 1 grain in 8; reverse or
  re-pitch one syllable per 2 bars [house#4][house#34][house#35]. Todd Edwards
  picks syllables that sound good over the piano riff [house#35]. Density
  engine default 4–8 grains/bar; range ≤ 12 st.
- **sounds:** kit `tr909` + `tr808` kick layer, two `rim` voices; bass
  `mono`/`reese`-style warm bass; chords `fm` `ep`/`piano`, `juno` pads;
  vocal chops: `juno` with the `vowel` formant bank as a stand-in (engine
  default). Missing: a vocal-chop grain voice (formant grains with reverse and
  re-pitch), cabasa/tambourine lane, reverse envelopes, spin-back/rewind,
  gunshot and siren FX (speed garage [house#36]).
- **arrangement** (NI: elements added gradually up to the drop at bar 25, the
  beat cut for one bar before the drop, the vocal replacing the arp at the
  drop, a bassless outro from bar 73 [house#13]; bar counts after 40 engine
  default):

  | Role | 1–8 | 9–16 | 17–24 | 25–40 | 41–48 | 49–56 | 57–72 | 73–88 |
  |---|---|---|---|---|---|---|---|---|
  | K | x | x | x (cut bar 24) | x | - | x (cut bar 56) | v | x |
  | H, shaker | x | x | x | x | s | x | x | x |
  | Rims | - | + | x | x | - | x | v | x |
  | B | - | - | f (LPF opening) | x | - | f | x | - |
  | Pd | - | x | x | x | x | x | x | f |
  | A (arp) | - | - | x | - | x | x | - | - |
  | V (chops) | - | - | - | x | s | - | x | s |
  | R / X | - | - | r 21–24 | 1 | - | r | 1 | - |

  intro 16; outro 16 (bassless); total 88 (≈ 2:40 at 132); extended mode
  doubles each 8/16-bar block (engine default).
- **energy** (engine default): 0.35, 0.5, 0.65, 1.0, 0.4, 0.7, 0.95, 0.5.
- **surprise targets:** double kick once per 4 bars; drop kick 1 of bar 2,
  p = 0.3; clap/rim fill every 2nd bar; chop reverse/re-pitch per 2 bars;
  speed garage: one spin-back or reversed-drum transition per track
  [house#4][house#9][house#10][house#14][house#34]; kit Witek 22–46.
- **references:** Todd Edwards "Guide My Soul", "Saved My Life"; MJ Cole
  "Sincere" (1998), "Crazy Love"; Shanks & Bigfoot "Sweet Like Chocolate"
  (1999); Artful Dodger feat. Craig David "Re-Rewind" (1999); Craig David "Fill
  Me In" (2000); speed garage: Armand Van Helden's CJ Bolland "Sugar Is
  Sweeter" remix, 187 Lockdown "Gunman", Double 99 "RipGroove".
  [house#4][house#34][house#36][house#37][house#47]

### Techno family

Shared by P5–P10 unless a profile overrides: kick 1, 5, 9, 13 in every bar
where drums play; open hat 3, 7, 11, 15; snare/clap 5, 13; closed hats on
16ths with ±10 % velocity and steps 4, 8, 12, 16 delayed ≈ 1/64 bar; rimshot
and percussion never on 1, 5, 9, 13 [techno#11][techno#12][techno#13][techno#15][techno#17].
"99.9 % of techno is minor": natural minor default, major < 5 %
[techno#19] (but see contradiction C5). Default: no lead melody; hooks are a
1–4-note riff, a stab or a 303 line [techno#2][techno#29][techno#30].
Structural changes on 4/8/16-bar boundaries, default 16; additive intro; outro
in reverse order of entry; a loop runs ≥ 16 bars; the filter moves
continuously; one long cutoff sweep per section (16–32 bars) on the lead,
chord or 303, and on pads a sweep up from 50 Hz across each 4-bar group
[techno#19][techno#20][techno#26][techno#27][techno#44][techno#48]. No fills:
transitions by mute/unmute only [arrangement#14][arrangement#15][arrangement#17].
Phrygian ♭2 only as a ≤ 10 % colour (unverified). [techno]
Glue 4:1; pump −2 to −3 dB; loudness −8.5 LUFS (hard −7.5).
[sound-design#31][sound-design#32][sound-design#35]

#### P5. Detroit techno

- **scope:** first and second wave Detroit (Belleville Three, UR, Carl Craig).
- **tempo:** 120–125, typical 123. [techno#11]
- **swing:** 55–60. [techno#11]
- **meter/grid:** 4/4, 16 steps; phrases of 8–16 bars. [techno#18]
- **drums** (`tr909` through a 12-bit crush, light distortion, little velocity
  variation; the movement comes from how parts interact [techno#11]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | no variation in the groove; whole-kick dropouts of 1–4 bars, 1–3 per track, or "broken up" | [techno#11][techno#18] |
  | Open H | 3, 7, 11, 15 | engine default 0.8 | — | [techno#11] |
  | Closed H ×2 | 16ths: one sharp, one soft with a slightly longer decay | engine default 0.7 / 0.5 | choke group with the open hat on (tight) or off (loose), toggled per section | [techno#11] |
  | C (909 snare) | 5, 13 | engine default 0.9 | snare-roll fills at section ends (Strings of Life) | [techno#18][techno#22] |
  | Ride | 1, 5, 9, 13 ("four to the bar on the kick beats") | engine default 0.6 | — | [techno#18] |
  | Rim / perc | any step except 1, 5, 9, 13 | engine default 0.5 | 727 bongos enter later (`latin` bongoH/L) | [techno#11][techno#18] |
  | Shaker | 16ths | engine default 0.5 | enters with the kick | [techno#18] |

- **bass:** "no bass" variant p = 0.2–0.3 (the probability is the author's,
  unverified), chords and drums carrying the low mids [techno#18]. Otherwise
  engine default: root 8ths or off-beat roots from the kick's tuning
  [sound-design#36]; Reese-style option (`reese`, two sines ±27 cents; the
  Reese bass is from Saunderson's "Just Want Another Chance", Detroit
  techno/house) [sound-design#25][arrangement#15].
- **harmony:**
  - modes: natural minor 0.95, major 0.05 [techno#19].
  - progressions: a 4-chord minor loop (minor 7ths moved in parallel as one
    "chord memory" shape, forum-sourced, unverified) 0.5; i–v two-chord, 4
    bars each, with III passing in bars 3 and 7 of the 8-bar cycle 0.3; one
    chord 0.2 (weights engine default). [techno#18][techno#19]
  - extensions: m7 (chord-memory shape), m9 stabs [sound-design#27][sound-design#28].
  - bars_per_chord 1 (4-chord loop) or 4 (two-chord); loop_bars 4 or 8.
- **melody/hooks:** orchestral string stabs on the first beat of each phrase
  with quieter sustained notes between; later the synth rhythm shifts
  against the unchanged string pattern for tension (8 bars) [techno#18].
  Piano chord riff (Strings of Life is four piano chords) [techno#18].
- **sounds:** strings: sampled-strings-style pad, short attack (Mirage-ish),
  stabs layered with a synth for attack, sometimes gated → `juno` strings +
  `stab` [techno#18][sound-design#27]; piano `fm` `piano`; kit `tr909` with
  a 12-bit crush, a little chorus and reverb on the hats [techno#11].
  Missing: 12-bit/sampler crush stage; sampled-string character.
- **arrangement** ("Strings of Life" order [techno#18]; bar counts engine
  default):

  | Role | 1–16 | 17–24 | 25–40 | 41–56 | 57–72 | 73–88 | 89–104 | 105–120 | 121–136 |
  |---|---|---|---|---|---|---|---|---|---|
  | Pd (piano chords) | x (solo) | v (up an octave: pre-drum cue) | x | x | x | v (new piano part) | x | x | f (cuts, fade) |
  | Strings (stabs) | - | - | x (beat 1 of each phrase) | x | x | v (new string part) | x | x (synth rhythm shifted) | f |
  | K | - | - | + | x | x (1–4-bar dropout) | x | x | x | f |
  | P (shaker) | - | - | + | x | x + bongos | x | x | x | - |
  | C, H | - | - | - | + (snare roll fill at 56) | x (fill 72) | x (fill 88) | x | x | - |
  | B (p 0.75) | - | - | - | + | x | x | x | x | - |

  intro 16–24 (solo piano); outro: abrupt volume cuts and a fade; total 136
  (engine default; ≈ 4:25 at 123).
- **energy** (engine default): 0.3, 0.4, 0.6, 0.75, 0.8, 0.7, 0.9, 1.0, 0.4.
- **surprise targets:** kick dropout 1–4 bars, 1–3 per track; choke toggle at
  a section change; stab rhythm shifted against the strings for 8 bars;
  snare-roll fills only at section ends [techno#11][techno#18]. Reese-style
  turnarounds: drop the kick before the turnaround, extra snare at the end of
  each 4-bar block [arrangement#15].
- **references:** Cybotron "Alleys of Your Mind" (1981), "Clear" (1983);
  Model 500 "No UFO's" (1985); Rhythim Is Rhythim "Strings of Life" (1987);
  Reese "Just Want Another Chance" (1988); UR "Riot" EP (1991); X-101 "Sonic
  Destroyer" (1991); Paperclip People "Throw" (1994); DJ Rolando "Knights of
  the Jaguar" (1998/99). [techno#1][techno#18][techno#36][techno#37][arrangement#15]

#### P6. Berlin / hard techno

- **scope:** Tresor and Berghain ("rumble", dark), Birmingham industrial,
  grinding analogue, 2020s hard techno; modes of one profile.
- **tempo:** rumble/Berghain 128–135; industrial 126–130; grinding 125–132;
  hard 140–160 (some DJs 150–160; "roughly 160 and up"); techno before 2019
  "rarely exceeded the mid 130s". Typical: 130 / 128 / 128 / 150.
  [techno#12][techno#13][techno#14][techno#23][techno#24]
- **swing:** rumble 50–54; industrial 50–60; grinding 50–65; hard 50.
  [techno#12][techno#13][techno#14]
- **meter/grid:** 4/4, 16 steps, 2-bar loops (grinding ghost kicks); 1/64-bar
  nudges and 2–8 ms offsets need micro-timing. [techno#14][techno#17]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | grinding: two ghost kicks in bar 2 of a 2-bar loop at 0.25–0.4, each 2–8 ms off the grid (offsets unverified) | [techno#12][techno#14] |
  | Open H | 3, 7, 11, 15 | engine default 0.8 | — | [techno#11] |
  | C / snare | 5, 13 (industrial: noise snare) | engine default 0.9 | — | [techno#13] |
  | Shaker ("hat 2") | 7, 15; or every off-beat with alternate hits 20–30 % quieter | engine default 0.6 | — | [techno#12][techno#13] |
  | Ride | odd steps; hits on 3, 7, 11, 15 shorter and −6 dB; HPF 1 kHz | 1.0 / −6 dB | — | [techno#12] |
  | Closed H | 16ths, ±10 %, 4/8/12/16 late ≈ 1/64 bar | engine default 0.6 | — | [techno#17] |
  | Perc growl | 3 (one off-beat near the bar start) | engine default 0.6 | — | [techno#14] |
  | Pre-beat wobble | short LFO glitch at 4 or 5 | engine default 0.5 | — | [techno#14] |
  | Low tom (industrial) | in the gaps between kicks | 0.5–0.7 less when it coincides with K or H | 6-bit woodblock layered on the toms | [techno#13] |

- **bass:** rumble mode: no separate bass; derive the rumble from the kick:
  a second kick with long decay and soft attack → long (3–4 s) wet reverb →
  resonant LPF and drive just under 4 dB, ducked by the main kick; drum bus
  Drive 17 %, Boom 45 % at 55 Hz [techno#12]. Engine has a `rumble` patch.
  Hard/industrial: the distorted kick carries the low end [techno#13][techno#14].
  "Throbbing … rigid beat and determined bassline" fits Berghain (inference)
  [techno#35]. Register: the kick's pitch (engine default 45–55 Hz).
- **harmony:** one chord or one riff in 60–70 % of tracks (share unverified);
  else i–v or i–iv, 4 bars each, with III passing in bars 3 and 7. Modes:
  minor 0.95. [techno#2][techno#15][techno#19] bars_per_chord 4; loop_bars 8.
- **melody/hooks:** none, or a 1–4-note riff; Regis: "wave upon wave of
  drums, percussive clutter and background grit", often without melody;
  Surgeon: one drum pattern plus two differently processed copies of one synth
  line mixed live. [techno#28][techno#41]
- **sounds:** kick: 909 with a strong pitch envelope, short decay, < 50 Hz
  cut, −3 dB at 500 Hz (rumble); industrial: 909 resampled through an SP-1200,
  250–300 Hz cut "making the sub emerge", overdrive, bitcrush 6-bit / 22 kHz;
  hard: distorted saturated 909 with a long release, lo-fi samples; hats and
  cymbals 6-bit (`hats909`); short plate reverbs; noise snare.
  [techno#10][techno#12][techno#13][techno#14][techno#24] Missing:
  bitcrush/requantise, per-lane EQ notches, saturation on the drum bus as a
  module, plate reverb.
- **arrangement** (techno template [arrangement#14][arrangement#15][arrangement#16][arrangement#17];
  Berghain: gradual builds, no individually explosive moments, 0–1 breakdowns
  of ≤ 8 bars [techno#35]):

  | Role | 1–8 | 9–16 | 17–32 | 33–48 | 49–50 | 51–64 | 65–96 | 97–104 | 105–112 | 113–160 | 161–176 | 177–192 |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | K | x | x | x | x | - | x | x | x or - | + (fake-out kick bar at 104, optional) | x (1-bar mute at 128, 144) | x | x |
  | H (open) | - | + | x | x | x | x | x | - | + | x | x | x |
  | C | - | - | + | x | - | x | x | - | + | x | x | - |
  | P (ride, shaker) | - | - | + | x | x | x (ride out until 57) | x | - | + | x | x | x |
  | B / rumble | - | - | - | + | x | x | x | x or - | + | x | - | - |
  | Synth motif | s (optional 1-bar motif) | - | - | + | x | x | x + 2nd synth or modulating bass | f (alone, filter closed) or - | + | x | - | - |

  Short breakdown: K + B only, or the synth alone; layers return one per 2–4
  bars. intro 16–32 drums only (bar count unverified [techno#20][techno#48]);
  outro 32; total 192 (6–10 min overall, ≈ 192–320 bars at 128 [techno#4][techno#47]).
- **energy** (engine default; Berghain: high plateau, subtle progression):
  0.4, 0.5, 0.6, 0.75, 0.6, 0.8, 0.9, 0.55, 0.75, 1.0, 0.7, 0.5.
- **surprise targets:** one layer in or out every 8 bars, no fills; kick HP
  reveal (HPF ~80 Hz for 2–8 bars, open on a downbeat); off-grid ghost kick or
  a delayed 32nd once per 4–8 bars; effects gating on bar boundaries; choke
  toggle at section changes [techno#11][techno#14][techno#17][techno#27];
  "resonance crack": let a filter self-distort at the peak of a long sweep and
  keep it [techno#27].
- **references:** X-101 "Sonic Destroyer" (Tresor 1); Jeff Mills *Waveform
  Transmission Vol. 1* (1992); *Tresor II: Berlin & Detroit* (1993); Ben Klock
  *Berghain 04* (2010); Surgeon (Downwards, early tracks); Regis; Sandwell
  District; Perc; Minimal Violence *InDreams* (≈ 155 BPM).
  [techno#2][techno#23][techno#24][techno#28][techno#33][techno#35][techno#38][techno#41]

#### P7. Minimal

- **scope:** 1990s Detroit/Plus 8 minimal (Hood, Bell, Plastikman, Mills's
  Purpose Maker) and 2000s German minimal/microhouse (Kompakt, Perlon, M-nus).
- **tempo:** 125–130, typical 127 ("Spastik" recreation 127; "Hawtin suggests
  128" is citation-needed). [techno#17][techno#30]
- **swing:** placed by hand; *Consumed*'s 909 shuffle ≈ 3; engine default
  52–56. [techno#17][techno#27]
- **meter/grid:** 4/4, 16 steps; 32nd grid for 808 snare rolls; polymeter
  cycles of 5 and 12 (*Consumed*) or 3, 5, 12, 13, 15 for 1–2 voices, never
  more than 2 at once (cap unverified). [techno#9][techno#17][techno#25][techno#27]
- **drums** (TR-808 percussion for "Spastik"; the 909 also used as a one-note
  sequencer trigger [techno#27][techno#47]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | HPF at 82 Hz for bars 1–2 then open at bar 3 (the "Spastik" reveal compressed into 4 bars) | [techno#17] |
  | Closed H | 16ths, ±10 %, 4/8/12/16 late ≈ 1/64 bar | engine default 0.6 | percussion can be just kick and hi-hat | [techno#17][techno#29] |
  | Snare roll (808) | 32nd grid; the 6th 32nd of a beat delayed in two places; bar 3 adds one 32nd right after the downbeat | engine default 0.5–0.9 | "Spastik"/percussive mode | [techno#17] |
  | Cross-rhythm voice | 1, 4, 7, 10, 13 (3+3+3+3+4) | engine default 0.7 | a 1-16th or 1-8th displaced copy of a part as metrical dissonance | [techno#25] |
  | Perc (polymeter) | cycle 5 or 12 steps | engine default 0.5 | phases against the bar | [techno#27] |
  | Ride/perc one-shot | phrase end | — | pitch falls 4 st right after the hit | [techno#17] |

- **bass:** energy sits in the mid frequencies more than deep bass; keep subs
  sparse [techno#30]. Mills "The Bells": a 16th-note bass enters at bar 65
  [arrangement#14]. Register engine default MIDI 36–48; sound `mono` short
  pluck or a `tb303` with low env (engine default).
- **harmony:** "rhythm and repetition instead of melody and linear
  progression": one chord or none (drone/pedal) [techno#30]; modes minor 1.0
  (GiantSteps minimal 11/11, tiny sample) [cognition#7].
- **melody/hooks:** motifs of 1 bar (Hood) to 4 bars (Plastikman "ethnik"
  flute); a bare 2-note motif over the kick (Mills); "take your time, get into
  a groove and stick to it" (Hood). [techno#26][techno#29][arrangement#14]
- **sounds:** `tr808` percussion, `tr909` kick; Serge-style resonant filters,
  triplet delays, gated reverbs switched by mute automation; a tom or conga
  slowly opened and closed through a resonant filter. [techno#27] Missing:
  32nd grid, polymeter per layer, per-hit micro-timing, effect-send gating as
  automation.
- **arrangement** (Mills "The Bells" [arrangement#14]; Plastikman forecasting
  [techno#26]; bars after 65 engine default):

  | Role | 1–8 | 9–16 | 17–24 | 25–32 | 33–48 | 49–50 | 51–64 | 65–96 | 97–128 | 129–160 | 161–176 | 177–192 |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | Motif (2 notes) | x | x | x | x (alone) | x | x | x | x | v | x | - | - |
  | K | x | x | x | - | x | - | x | x | x | x | x | x |
  | H (open, ride fades in) | - | + | x | - | x | x | x (1-bar break at 64) | x | x | x | x | x |
  | C | - | - | + | - | x | - | x (out at 64) | x | x | x | x | - |
  | B (16ths) | - | - | - | - | - | - | - | + | x | x | - | - |
  | P (polymeter) | - | - | - | - | + | x | x | x | x | x | x | x |

  intro 8–16; outro 32; total 192; "extended" variant 12+ min for minimal and
  acid [techno#4][techno#47]. Hawtin: the loop ran overnight while mixer mute
  automation switched effects; "an hour-long listening experience" [techno#27].
- **energy** (engine default; slow, accumulative): 0.3, 0.4, 0.5, 0.25, 0.6,
  0.5, 0.65, 0.8, 0.85, 0.9, 0.6, 0.45.
- **surprise targets:** subtraction rather than addition [techno#29]; one
  odd-length loop so phase alignment returns every 13 or 15 bars [techno#9];
  effects gating on bar boundaries [techno#27]; a delayed 32nd or off-grid
  ghost once per 4–8 bars [techno#14][techno#17]; kick HP reveal [techno#17].
- **references:** Plastikman "Spastik" (1993, 9:22), *Musik* (1994),
  *Consumed* (1998); Robert Hood *Minimal Nation* (1994); Jeff Mills "The
  Bells" (1996); Kompakt *Total* series; Ricardo Villalobos *Alcachofa* (2003),
  "Fizheuer Zieheuer" (2006). [techno#27][techno#29][techno#30][techno#45][techno#46][techno#47]

#### P8. Acid (acid house, acid techno)

- **scope:** Chicago acid house (TB-303 lineage), UK acid techno, early
  German acid trance as a tempo mode.
- **tempo:** acid house 118–122 ("Acid Tracks" ≈ 130 as made, ≈ 120 as
  released), typical 120 [techno#4][house#18]; acid techno engine default
  130–140; early German/acid trance 128–140 (engine default within 120–150)
  [trance#1][trance#14].
- **swing:** engine default 52–55 for acid house (Chicago 80s range
  [house#6]); 50 for acid techno and acid trance (trance straight [trance]).
- **meter/grid:** 4/4; the 303 plays 16ths; pattern length 16, or cut to 13
  or 15 steps against the drums ("woozy and unstable"). [techno#8][techno#9]
- **drums:** acid house: the Chicago grid (P1) on a TR-707 + TR-727 (the
  "Acid Tracks" jam) [techno#5][house#20]; acid techno: techno grid (shared),
  909, distortion. Lone "Dragonrush" (rave-techno, 132): 909 off-beat ride from
  bar 9, chopped break and tambourine from bar 17 [arrangement#16].
- **bass (the 303 line):**
  - grammar: one line in 16ths; enter 16 notes quickly, random pitches inside
    the key, then fix the riff; accents and slides placed at random: 25–40 %
    of steps accented, 10–25 % slid (percentages unverified); octave jumps ≤ +1
    octave. [techno#8][techno#9]
  - articulation: normal notes gated 7/12 of the step (≈ 58 %); slid notes
    held the full 12 ticks into the next. [techno#8]
  - accent: filter-env decay forced to its minimum (200 ms), filter env added
    into the VCA with a softened attack, cutoff sweep scaled by resonance,
    accumulating across consecutive accents (don't reset the sweep capacitor).
    [techno#7][sound-design#9][sound-design#10]
  - register: engine default MIDI 36–60 (C2 base, +1 oct jumps).
  - sound: `tb303` / `acid` patch with the `tb303` recipe; distortion after
    the 303, dynamics after the distortion (acid techno); stack up to 2 lines
    (Hardfloor used up to six). [techno#9][techno#40][trance#14]
- **harmony:** single-key 303 riffs; no published scale rule for 303 riffs
  [techno#9]; minor 0.9 (engine default), Phrygian ≤ 0.1 (unverified). No
  chord layer by default; optional one chord pad (engine default p 0.3).
- **melody/hooks:** the 303 line is the hook; the motion is the knob moves
  (cutoff, resonance, decay), not a written melody: vary only accent, slide and
  cutoff after the riff is fixed. [techno#5][techno#9]
- **sounds:** `tb303`; drums `tr909`/`tr808`; 707/727 need `lofiPCM` and the
  `latin` kit's congas/bongos for the 727. Missing: 303 accuracy fixes (24 dB
  ladder, HP in the resonance loop, accent τ ≈ 150 ms, gate delay) and a
  distortion stage per part. [sound-design]
- **arrangement** (long-form acid house; engine default bar counts; "Acid
  Tracks" runs 12:15, the motion is the knobs [techno#4][techno#5]; acid line
  entering as the beat drops out, Lone bar 33 [arrangement#16]):

  | Role | 1–16 | 17–32 | 33–64 | 65–96 | 97–112 | 113–128 | 129–192 | 193–224 | 225–256 |
  |---|---|---|---|---|---|---|---|---|---|
  | K | x | x | x | x | - | + (bar 121) | x | x | x |
  | H, C, P | x (H) | + C | + P | x | s (H only) | x | x | x | x |
  | 303 | - | f (cutoff closed, decay short) | x (cutoff opening over 32) | v (accent bursts, res up) | x (alone, resonance peak) | r (303 build macro) | x + 2nd 303 line | f (closing) | - |
  | Pd (p 0.3) | - | - | - | x | x | x | x | - | - |

  intro 16–32; outro 32; total 256 (≈ 8:30 at 120); extended 12+ min
  [techno#4].
- **energy** (engine default; follows the cutoff macro): 0.35, 0.45, 0.65,
  0.8, 0.5, 0.75→0.95, 1.0, 0.6, 0.4.
- **surprise targets:** accent bursts of 3–5 consecutive accents so the sweep
  climbs [techno#7]; every 4–8 bars change the decay knob (200 → 600 ms) or
  cutoff, not the notes [sound-design#9][sound-design#12]; 303 build: cutoff
  +1–2 octaves and resonance +0.1–0.2 over 8–16 bars, accent density 1-in-8 →
  1-in-3 [sound-design#10]; resonance crack [techno#27]; odd-length 303 loop
  [techno#9].
- **references:** Phuture "Acid Tracks" (1987); Hardfloor *Hardtrance
  Acperience* (1992); The Liberator DJs *It's Not Intelligent… And It's Not
  From Detroit…* (1997); Lone "Dragonrush" (rave-techno); 808 State "Cubik"
  (cross-rhythm reference). [techno#4][techno#25][techno#39][trance#14][arrangement#16]

#### P9. Dub techno

- **scope:** Basic Channel / Maurizio / Chain Reaction, Rhythm & Sound,
  Deepchord.
- **tempo:** 112–125 typical (sources: 110–125), typical 120; allow a 140–145
  variant at low weight (Attack's Basic Channel-style recreation runs at 145;
  conflict unresolved, C4). [techno#15][techno#31]
- **swing:** 50. [techno#15]
- **meter/grid:** 4/4, 16 steps; dotted-1/8 echoes define the off-grid feel.
  [techno#15][techno#16]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | ducks the bass and stabs | [techno#15] |
  | Open H | 3, 7, 11, 15 | engine default 0.7 | — | [techno#15] |
  | Snare | 5, 13 ("every other downbeat"), doubled with a clap pitched up 3–7 st (pitch unverified) | engine default 0.8 | — | [techno#15] |
  | Dub perc | short loop (engine default 1 bar) | engine default 0.5 | through amp distortion, a dotted-1/8 echo and bit erosion; vinyl crackle | [techno#15] |

- **bass:** short sub notes on the root and the fourth only, short release,
  sidechained to the kick (example D♯ and G♯) [techno#15]; "chorded layers of
  elaborate basslines" [techno#31]. Register engine default MIDI 26–38.
  Sound `mono` `sub`.
- **harmony:** one minor triad or minor 7th (example D♯ minor), chopped into
  syncopated stabs plus the same chord sustained underneath; chords "tend to
  sound hollow" (scoop the mids). Modes minor 1.0. bars_per_chord: whole
  track (engine default: one chord change at most per 64 bars).
  [techno#15][techno#16]
- **melody/hooks:** none; the stab rhythm and its echoes are the hook.
  [techno#15]
- **sounds:** dub chord chain: saw + square + a little noise (PW ≈ 70) → BP ≈
  450 Hz, 12 dB/oct, slight resonance, slow LFO on cutoff → bit reduction
  (Redux 6) → EQ −7 dB at 600 Hz (wide), HP 100 Hz, LP 4.5 kHz → phaser →
  plate/short reverb; sends: dotted-1/8 tap at 20 % feedback plus a tape echo
  (RE-201 model) with a free 264 ms tap at 80–85 % feedback, filtered on the
  send by a 0.25 Hz LFO; tape hiss [techno#15][techno#16]. Engine: `juno`
  `dubChord` patch as the source. Missing: band-pass mode in the poly filter
  chain, phaser, tape echo with filtered feedback, bit reduction, hiss bed.
- **arrangement:** the techno template (P6) with the stab and its echo
  replacing the synth motif; breakdown = stabs and echo alone; engine default
  total 192.

  | Role | 1–16 | 17–32 | 33–64 | 65–96 | 97–112 | 113–160 | 161–192 |
  |---|---|---|---|---|---|---|---|
  | K | x | x | x | x | - | x | x |
  | H, C | + 9 | x | x | x | - | x | x (C out 177) |
  | Dub perc | - | + | x | x | - | x | x |
  | Sub | - | - | + | x | - | x | - |
  | Stab + echo | f (LPF) | f (opening) | x | v (echo throw) | x (alone, long echo) | x | f |
  | Sustained chord | - | - | - | + | x | x | - |

- **energy** (engine default; shallow): 0.35, 0.45, 0.6, 0.7, 0.4, 0.8, 0.45.
- **surprise targets:** dub echo throw: raise the tape-delay feedback to ≥ 85
  % on one stab, cut it after 1–2 bars (inference from the feedback settings)
  [techno#16]; effects gating on bar boundaries [techno#27]; stab-pattern
  mutation every 16 bars (engine default).
- **references:** Maurizio "Ploy", "Lyot" (1992); Basic Channel
  "Enforcement", "Phylyps Trak" (1993), "Quadrant Dub", "Octagon", "Radiance"
  (1994); *BCD* (1995); Rhythm & Sound; Deepchord. [techno#31][techno#32]

#### P10. Melodic techno

- **scope:** melodic house & techno (Afterlife, Tale Of Us, Âme, Bodzin).
- **tempo:** 120–128 (tutorial 122–125), typical 124. [techno#43][techno#44]
- **swing:** 50–54. [techno]
- **meter/grid:** 4/4, 16 steps; 16-bar blocks. [techno#44]
- **drums:** the techno grid (shared); full drums only in the drop and climax
  [techno#44].
- **bass:** a Moog-style mono on the chord roots in 1/8 and 1/16 pulses,
  sidechained; "deep, continuous basslines". Sound `moogBass`. Register engine
  default MIDI 29–41. [techno#43][techno#44]
- **harmony:** minor keys or modes; progressions i–VI–III–VII 0.5, i–iv–v–III
  0.5 (weights engine default); one chord per bar or per 2 bars; 2–3 pad
  layers; filter automation over 16 bars. loop_bars 4–8. [techno#43][techno#44]
- **melody/hooks:** a minor-key arpeggio in 16ths, with delay and reverb doing
  much of the work; lead in the drop and climax. [techno#43][techno#44]
- **sounds:** pads `junoPad` ×2–3 layers; arp `juno` `pluck`; lead `mono`
  `sawLead` or `supersaw` (engine default); bass `moogBass`. Loudness −10.5
  LUFS. [techno#44][sound-design#35]
- **arrangement** (16-bar blocks, normalised from overlapping bar numbers in
  the source [techno#44]):

  | Role | 1–16 intro | 17–32 build | 33–48 drop | 49–64 breakdown | 65–80 climax | 81–112 outro |
  |---|---|---|---|---|---|---|
  | K, H | x | x | x | - | x | x |
  | C, P | - | s | x | - | x | x (to 96) |
  | B | - | + | x | - | x | - |
  | Pd | - | + (filtered) | x | x | x | f |
  | A | - | + | x | s | x | - |
  | L | - | - | x | s | x | - |
  | Atm / FX | x | x | x | x | x (more automation) | x |

  intro 16; outro 16–32; total 112 (engine default 2× blocks for a 7-min club
  version).
- **energy** (engine default): 0.3, 0.55, 0.9, 0.35, 1.0, 0.5.
- **surprise targets:** filter automation is the motion; one 16-bar breakdown,
  then a 16-bar climax [techno#44]; arp pattern mutation every 16 bars (engine
  default); melody IC per G9.7.
- **references:** no landmark tracks verified by the research [techno#43];
  artists Tale Of Us, Âme, Stephan Bodzin, Mind Against, Adriatique, ARTBAT,
  Anyma; Âme "Rej" as an arrangement reference. [techno#43][techno#44][arrangement#17]

### Drum and bass family

Shared by P11–P13: 16th swing 50–60 [dnb#9][dnb#10]; phrases in multiples of
8 bars, drums in 16-bar phrases with an uplifter every 8 bars, a crash every 8
(on step 1, and at the end of a 32-bar pattern; extra crashes on bars 5 and 7
optional) and a fill at the end of every 16 [dnb#11][arrangement#8]; split the
bass into a mono sine sub (≤ ~100 Hz) and a mid layer, sidechain the sub to
the kick, use one main bass patch at a time and switch patches between drop
1 and drop 2 [dnb#11][dnb#20][dnb#36][dnb#37]; drum bus: saturation, then
parallel compression (threshold −20 dB, 2:1, attack ~40 ms, release ~115 ms)
[dnb#9][dnb#11]; reverbs: short drum room, hall for music, low-cut bass
reverb, snare top and ride sharing the snare's send [dnb#9][dnb#14]; pump ≤
0.2; loudness −6 to −7 LUFS [sound-design#35]. Default form (256 bars ≈ 5:53
at 174, first drop at bar 49 ≈ 66 s): intro 32 – build 16 – drop 64 –
breakdown 32 – build 16 – drop 64 – outro 32; ranges intro/outro 16–32, builds
8–16, drops 32–64, breakdowns 16–32 [dnb#36][dnb#37] (arrangement.md's 160-bar
template differs: C7).

**Two-step skeleton** (all DnB): kick 1, 11; snare 5, 13; hat 8ths on 3, 7,
9, 15 [dnb#9][dnb#11][dnb#12][dnb#14]. The second kick is "pushed later in
time to the eighth-note before the second snare" [dnb#12].

**Break kit (heuristic)** for jungle and break layers [dnb]: six voices: low
kick (808/909, short), snare body, snare-noise top, ghost snare (same snare,
−12 to −18 dB, 40–60 ms decay), ride (8ths), shuffle layer (noise hat, 16ths,
−15 dB, velocity-randomised). Breaks are stored as **cells** of 2 beats (8
steps), each a list of (step, voice, velocity, micro-offset ms). Chop
operations: *reorder* half-bar cells; *repeat* a 1/8 or 1/16 slice 2–8×
(stutter); *pitch* a slice ±1–12 st (ragga squeal = 1/32 repeats pitched up);
*reverse* a snare's envelope; *drop* a slice to leave a gap before a drop.
Keep the kick on 1 and backbeats (5, 13) intact in ≥ 3 bars of 4; concentrate
chops on bar 4 of each 4 and bar 8/16 of each phrase; ghost hits ±5–15 ms off
the grid; break bus: saturation, bracket EQ (HPF ~100 Hz, LPF ~10–12 kHz),
optional 12-bit reduction (S950). [dnb#4][dnb#9][dnb#10][dnb#11][dnb#15][dnb#46][dnb#48][dnb#50]

**Amen template** (reconstructed; bar 3–4 details unverified) [dnb#4][dnb#5][dnb#6]:

| Bars | K | S (accented / ghost) | Other |
|---|---|---|---|
| 1–2 | 1, 3, 11, 12 | 5, 13 / 8, 10, 16 | ride on all 8ths |
| 3 | 1, 3, 11 | 5, 15 (the delayed snare) / 8, 10 | ride on 8ths |
| 4 | 3, 11 (beat 1 empty) | 5, 15 / 2, 8, 10 | early crash on 11 |

#### P11. Jungle (ragga, darkcore, hardcore and breaks as modes)

- **scope:** jungle and ragga jungle (1993–96), darkcore, breakbeat hardcore
  (1989–93), intelligent/atmospheric jungle; big beat and nu skool breaks as
  the same break grammar at 125–140 BPM with house-length phrasing
  (heuristic). [dnb#1][dnb#2][dnb#3][dnb#22][dnb#42][dnb#43]
- **tempo:** jungle/ragga 160–170, typical 165; darkcore ~155–165; breakbeat
  hardcore 130–155; big beat 100–140; nu skool breaks 125–140 (Beatport breaks
  median 139.5). [dnb#1][dnb#24][dnb#36][dnb#42][dnb#43][cognition#8]
- **swing:** 50–60. [dnb#9][dnb#10]
- **meter/grid:** 4/4, 16 steps; 2-beat break cells; 1/32 slices for stutters
  and squeals; bass felt at half time (~80 BPM). [dnb#24][dnb#50]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | Break (kit above) | Amen template, or a chopped Think-style break; or the two-step skeleton overlaid | accents 1.0, ghosts −12 to −18 dB | alternate two break sources each bar; swap break source at each 16-bar boundary; chops per the rules above | [dnb#1][dnb#4][dnb#11][dnb#15] |
  | Kick (programmed) | 1, 11 (+3, 12 Amen) | 1.0 | 808 kicks pitched down or lengthened are central | [dnb#1][dnb#9] |
  | Tambourine/tops | 16ths (Think tambourine layered under other breaks) | engine default 0.4 | — | [dnb#8] |
  | Crash | step 1 every 8 bars | 1.0 | — | [dnb#11] |
  | Fills | Amen fragments/fill at the end of each 16-bar section; snare climbs on bar 7 of each 8; filter-and-flange fills (bars 24, 32); stick fill at each turnaround | — | turnaround on the last bar of each 8-bar loop (transition break, filtered melody, reverse crash) | [dnb#11][dnb#15][arrangement#19][arrangement#20] |

  Kit Witek target 22–46 per 2-bar loop; reject ≥ 49 [cognition#18]; the
  breakbeat bar of G9.6 scores 20 per bar.
- **bass:** grammar: half-time feel, one or two long notes per bar (steps 1
  and 11, or a 2-bar sustained root) (heuristic); "160 bpm breaks and 80 bpm
  basslines" [dnb#24]. Register: sub roots in E–G (≈ 41–49 Hz) [dnb#15].
  Sound: 808 sub (`mono` `sub`), `reese` (Renegade "Terrorist" sampled
  Saunderson's Reese over the Amen) [dnb#1][dnb#18]; ragga "dread bass" with a
  wah filter [dnb#23].
- **harmony:** reggae-derived bass carries the harmony, often a single-key
  vamp under samples (specific progressions unverified) [dnb#24]; heuristic
  pool: i pedal 0.5, i–VI–VII 0.3, i–VII 0.2 (weights engine default; the
  i–VI–VII is unverified [dnb]). Hardcore: major-key piano stab riffs; darkcore:
  minor [dnb#3][dnb#23]. Intelligent: slow minor pads, 2–4 bars per chord
  (rate heuristic) [dnb#22]. Modes: minor 0.82 (GiantSteps DnB) [cognition#7];
  hardcore mode major 0.6.
- **melody/hooks:** short vocal/MC phrase hooks, time-stretched and
  stuttered; dub siren as a call [dnb#23][dnb#24]. Hardcore: hoover lead,
  house piano stabs, sped-up vocals [dnb#3][dnb#34]. Intelligent: ambient pads,
  jazz, celestial atmospheres [dnb#22].
- **sounds:** break kit on `tr808`/`tr909` voices; `reese`; `mono` sub; FX:
  dub siren, gunshot, air-horn, delay throws, tape spin-back, time-stretch
  stutter [dnb#2][dnb#23][dnb#24][dnb#38]. Missing: break-cell player with
  chop operations and per-hit offsets; vocal/MC chop voice; siren, gunshot,
  air-horn; tape stop/spin-back; `hoover`; sampler bit reduction.
- **arrangement** (Special Request "Delicate Limbs" remix, 160 BPM
  [arrangement#20]; Tessela "Hackney Parrot" [arrangement#19]; vocals from the
  third 8-bar block [dnb#15]; bar counts after 112 engine default):

  | Role | 1–16 | 17–32 | 33–48 | 49–64 | 65–80 | 81–96 | 97–112 | 113–144 | 145–160 |
  |---|---|---|---|---|---|---|---|---|---|
  | Break | f (processed) | x (fills 24, 32) | x (climbs 39, 47) | v (2nd break alternating) | x | f (intro break returns) | x (cut at 112) | v | f |
  | Sub + Reese | - | + | x | x | x (1-bar mute, back 1 bar before the turnaround) | - | x (cut at 112) | v (new patch) | - |
  | V (MC, chops) | - | + 17 | x | s | x | - | x | x | - |
  | Atm / siren | x (siren before each turnaround) | - | - | - | - | x | - | - | x |
  | X | - | 1 | - | - | - | - | - | 1 | - |

  intro 16–24, break-led; outro 16; total 160 (≈ 3:50 at 165).
- **energy** (engine default): 0.4, 0.8, 0.85, 0.9, 0.85, 0.45, 0.95, 1.0, 0.45.
- **surprise targets:** rewind: p ≈ 0.1 per track, 8–16 bars into drop 1:
  tape-stop + spin-back 0.5–1.5 s, 1 bar of silence, restart drop 1 from its
  first bar (heuristic) [dnb#38][dnb#39]; stutter: 1/16–1/32 slice repeats of a
  snare or vocal for 1–2 beats at phrase ends, optionally pitched up
  [dnb#23][dnb#50]; break swap at each 16-bar boundary [dnb#1][dnb#11]; cutting
  break and bass one bar before the expected turnaround (Special Request at
  112) and a 7-bar spoken-word break (Tessela) as off-grid surprises
  [arrangement#19][arrangement#20].
- **references:** Lennie De Ice "We Are I.E." (1991); 4hero "Mr Kirk's
  Nightmare" (1990); Metalheads "Terminator" (1992); Rufige Kru "Ghosts of My
  Life", Origin Unknown "Valley of the Shadows", Omni Trio "Renegade Snares"
  (1993); Shy FX & UK Apache "Original Nuttah"; M-Beat feat. General Levy
  "Incredible"; Renegade "Terrorist"; DJ Hype "Roll the Beats"; Remarc "Drum &
  Bass Wize"; Dead Dread "Dread Bass"; Goldie "Inner City Life" (1994); LTJ
  Bukem "Atlantis", "Music", "Horizons"; Special Request's "Delicate Limbs"
  remix; Tessela "Hackney Parrot (Special Request VIP)"; big beat: The
  Prodigy *The Fat of the Land*, Chemical Brothers *Dig Your Own Hole*.
  [dnb#1][dnb#2][dnb#23][dnb#24][dnb#25][dnb#26][dnb#28][dnb#42][arrangement#19][arrangement#20]

#### P12. Liquid drum and bass

- **scope:** liquid funk (1999–), with intelligent jungle as its ancestor.
- **tempo:** 170–176, typical 174. [dnb#1][dnb#14][dnb#36]
- **swing:** 50–60. [dnb#9][dnb#10]
- **meter/grid:** 4/4, 16 steps; 16-bar blocks. [dnb#14]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 11 | 1.0 | extra kick on 16 (p ≈ 0.25 in bar 3 of 4); bar 4 kick 1 → 3; ghost kicks on the last steps of every 8th bar | [dnb#11][dnb#14] |
  | S | 5, 13 | 1.0 | displaced snare 13 → 15 (variation) | [dnb#5][dnb#12] |
  | Ghost S | 8, 10 | −12 to −18 dB, short, ±5–15 ms, ±15 % vel | every 4th bar at minimum | [dnb#10][dnb#13][dnb#14] |
  | H | 3, 7, 9, 15 (8ths where K and S are absent) | engine default 0.6 | phrase-end hat ghost 16ths swung back, quieter | [dnb#9][dnb#14] |
  | Ride | odd steps, HP'd, ducked by K and S | engine default 0.5 | — | [dnb#9][dnb#10] |
  | Tops (Think-style) | 16ths, high-passed, low level | engine default 0.3 | shaker and ride loops | [dnb#14] |

  Kit Witek target 22–46. [cognition#18]
- **bass:** sub around −4 dB; a 4-bar riff looped to 16 bars, an extra bass
  run in every second 4-bar group, the bass muted for the last 2 beats of each
  16-bar phrase [dnb#14]. Register engine default root MIDI 29–41. Sound:
  `mono` `sub` + a warm `reese` (±20–30 cents) mid layer [dnb#17].
- **harmony:**
  - modes: minor 0.9 (default key F minor, the most common), major 0.1
    (engine default). [dnb#14]
  - progressions: i9–IIImaj7–v7–i9–IIImaj7sus4/VI–V (MusicRadar over an A
    minor bassline) 0.5; i9–IIImaj7 two-chord with the voicing held while the
    bass moves i ↔ III (Am9 and Cmaj7 share C-E-G-B) 0.3; i9–iv9–IIImaj7–VImaj7
    0.2 (weights and the third loop engine default). [dnb#16]
  - extensions: m9, maj7, m7 (soul/jazz/R&B vocabulary). [dnb#16][dnb#21]
  - bars_per_chord: 1–2 (heuristic); loop_bars 4–8.
- **melody/hooks:** Rhodes or piano riff, low-passed (~350 Hz) in the build
  and opened at the drop; a bell counter-melody with 1/4 delay in drop 1B,
  second pass an octave up; an optional held tonic drone for 7–8 bars; soul
  vocals. [dnb#14][dnb#21][dnb#22]
- **sounds:** keys `fm` `ep` (`fmEP`), `piano`; bells `fmBell` with 1/4 delay;
  pads `junoPad`; bass `sub` + `reese`; kit `tr909` (kick HPF ~85 Hz on any
  break layer, snare in three layers: body, mid, bright top + hall)
  [dnb#9][dnb#14]. Missing: Think-style top loop (needs the break-cell player),
  vocal voice.
- **arrangement** (EDMProd's six 16-bar blocks [dnb#14]; the 256-bar form
  above is the extended version):

  | Role | 1–16 intro | 17–32 build | 33–48 drop 1A | 49–64 drop 1B | 65–80 drop 1C | 81–96 outro |
  |---|---|---|---|---|---|---|
  | Drums | s (tops, LP'd) | f (LP'd; out 29–32) | x | x (out 49–52) | x (out 79–80) | s |
  | Sub / B | - | - | x | x (out 49–52) | x (out 79–80) | - |
  | Keys (Rhodes) | x | f (LPF ~350 Hz, muted every 2 bars) | x | x | x | f |
  | Pd | x | x | x | x | x | x |
  | Bell counter-melody | - | - | - | + (1/4 delay) | x (octave up) | - |
  | R, reverse crash | - | r (29–32) | 1 (X) | - | - | - |

  intro 16 (32 in the extended form); outro 16–32; total 96 (extended 256).
- **energy** (engine default): 0.35, 0.5, 0.9, 0.95, 1.0, 0.4.
- **surprise targets:** drop 1B mutes drums and bass for 4 bars and adds the
  counter-melody; C/D remove drums and bass for the last 2 bars [dnb#14];
  ghost-snare and kick-variation menu per bar [dnb#11][dnb#12]; halftime
  switch in drop 2 p ≈ 0.3 (heuristic) [dnb#22][dnb#36][dnb#45]; chord IC per
  G9.7.
- **references:** Calibre "Mystic/Feelin" (2000), *Musique Concrète* (2001),
  *Second Sun* (2005); High Contrast *True Colours*, "Return of Forever",
  "Global Love" (2002), "If We Ever" (2007); London Elektricity *Pull the
  Plug* (1999); Marcus Intalex & ST Files "Universe"; LTJ Bukem "Horizons"
  (intelligent ancestor). [dnb#21][dnb#22][dnb#26][dnb#29][dnb#30][dnb#31]

#### P13. Neurofunk and rollers (techstep, jump-up, halftime as modes)

- **scope:** techstep (1995–), neurofunk (1997–), rollers (a drum pattern
  rather than a sub-genre), jump-up, halftime. [dnb#19][dnb#20][dnb#22][dnb#40][dnb#41]
- **tempo:** 170–176, typical 174. [dnb#1][dnb#36]
- **swing:** ≤ 52 for techstep/neuro (heuristic); rollers 50–60. [dnb#9][dnb#19][dnb#20]
- **meter/grid:** 4/4, 16 steps; 1/16 edits; halftime mode = snare on 9 only
  (felt ~87 BPM). [dnb#22][dnb#36]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 11; neuro: 1, 8 ("the last 16th before beat 3") | 1.0 | sections with kicks on every quarter; extra kick 16 in bar 3; bar 4 kick 1 → 3 | [dnb#11][dnb#12] |
  | S | 5, 13; halftime: 9 only | 1.0 | displaced 13 → 15 | [dnb#12][dnb#22] |
  | H / ride | 3, 7, 9, 15 / odd steps | engine default 0.6 | bit-crushed, side-chained ride | [dnb#9] |
  | Ghost S | 8, 10 (rollers); none in techstep/neuro | −12 to −18 dB | — | [dnb#14] |
  | Build roll | K or S: quarters 4 bars → 8ths 2 → 16ths 2 (heuristic split) | ramp | into each drop | [dnb#14] |

  Techstep/neuro: quantised machine drums, no humanised break layer,
  "regimented" [dnb#19][dnb#20][dnb#22]. Rollers: a rolling breakbeat plus "a
  gripping bassline" [dnb#40]. Kit Witek: rollers 22–46; neuro engine default
  15–30.
- **bass:**
  - neuro/techstep: mono with ~250 ms glide; heavy saturation; continuous
    filter/FM/wavefold modulation on the mid layer only, a steady sub beneath;
    one long note per 1–2 bars plus 1/16 "edits" (stutters, gaps) at phrase
    ends (heuristic). [dnb#20][dnb#44]
  - rollers: bass hits locked to the kick (1, 11) with short 16ths between; a
    4-bar riff with a fill run in bar 4 or 8 (heuristic). [dnb]
  - jump-up: a 1–2-bar hooky bass riff is the main hook; wobble (LFO on
    cutoff or volume synced to 1/4, 1/8 or 1/8T). [dnb#22][dnb#45][dnb#47]
  - Reese: 2 saws ±20–30 cents (default ±27), mono, sustain 1.0, release ~25
    ms; distorted variant with the 2nd saw +12 or +19 st, quieter.
    [dnb#11][dnb#17]
  - register: sub ≤ ~100 Hz; mid layer engine default 100–800 Hz.
- **harmony:** no chord progression: a single root pedal with ♭2 and tritone
  inflections in the mid-bass (heuristic; "tonal dissonance") [dnb#1][dnb#22].
  Modes: minor/Phrygian 1.0 (engine default).
- **melody/hooks:** avoid melody; hooks are bass-timbre gestures; techstep
  "eschewed melody" for "nightmarish cyborg sounds". [dnb#22]
- **sounds:** `reese` + `mono` `sub`; `fm` for the mid layer's FM growl
  (engine default); bleeps, squelches, sci-fi noise (techstep); dry machine
  drums `tr909`. [dnb#19][dnb#20][dnb#22] Missing: wavefolder, resampling,
  per-note continuous modulation lanes (FM index, fold, cutoff), wobble LFO
  routed to cutoff/volume, 250 ms portamento on a distorted mono.
- **arrangement** (the 256-bar DnB form [dnb#36][dnb#37]):

  | Role | 1–32 | 33–48 | 49–64 | 65–80 | 81–96 | 97–112 | 113–144 | 145–160 | 161–224 | 225–256 |
  |---|---|---|---|---|---|---|---|---|---|---|
  | Drums | x (no sub; first bass hit at 17) | f (LP'd; out last 4) | x | v | x (out 95–96) | x (out 111–112) | s or - | f | v (new pattern or halftime 8–16 bars) | x (mirror of intro) |
  | Sub | + 17 (s) | - | x | x | x | x | s | - | x | - |
  | Mid bass | - | - | x | v (edits) | x | v | - | f (teased HPF'd) | v (new patch) | - |
  | Atm / Pd | x | x | - | - | - | - | x | x | - | x |
  | R, X | - | r, 1-beat gap | 1 | - | - | - | - | r | 1 | - |

  intro 32; outro 32; total 256 (≈ 5:53 at 174).
- **energy** (engine default): 0.4, 0.55, 0.95, 0.9, 0.95, 0.9, 0.35, 0.6,
  1.0, 0.45.
- **surprise targets:** drop 2 differs by a new bass patch, an extra layer, a
  different drum pattern or a halftime switch, plus slightly more saturation
  [dnb#36][dnb#37]; halftime 8–16 bars of drop 2, p ≈ 0.3 (heuristic);
  bass "edits" at phrase ends; gaps: bass out for the last 2 beats of a
  16-bar phrase, all drums out for 2 bars before a section change [dnb#14].
- **references:** DJ Trace remix of T-Power "Mutant Jazz" (1995); Trace/Ed
  Rush/Nico "Damn Son" (*Torque*, 1997); Bad Company "The Nine"; Ed Rush &
  Optical "Funktion" (1997), *Wormhole* (1998); Noisia *Split the Atom*
  (2010), *Outer Edges* (2016); DJ Zinc "Super Sharp Shooter" (1996, jump-up);
  Om Unit, Dub Phizix, Fracture, Ivy Lab (halftime).
  [dnb#19][dnb#20][dnb#22][dnb#23][dnb#32][dnb#33][dnb#45]

### Trance family

Shared by P14–P17: kick on every quarter (1, 5, 9, 13); open hat on 3, 7, 11,
15, swapped to closed hats in lower-energy sections; clap/snare on 5, 13;
straight 16ths, swing 0 (engine default); 4/4 in 16- or 32-beat phrases;
instruments enter or leave every 4, 8, 16 or 32 bars; change something every
8 bars and never let one layer set run longer than 32 bars.
[trance#1][trance#19][trance#22][trance#40] Loudness −7.5 LUFS; glue 2:1;
tape 0–0.1. [sound-design]

#### P14. Uplifting trance (hard trance as a mode)

- **scope:** uplifting/epic/anthem trance (from 1996); German trance roots;
  hard trance (Belgium, Germany, Netherlands, 1993–97) as a mode.
  [trance#1][trance#15][trance#17]
- **tempo:** 136–140, typical 138 (Beatport trance median 139); "modern
  uplifting" 128–135; hard trance 140–150 (allow 160; the 180 ceiling belongs
  to adjacent hardcore). [trance#1][trance#15][trance#17][trance#29][trance#30][trance#33][cognition#8]
- **swing:** 0. [trance]
- **meter/grid:** 4/4, 16 steps; work starts as 4- or 8-bar sections; 32nd
  grid for the final roll bar. [trance#3][trance#28]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | 909-ish, chunk ~100 Hz (Tate) or 60–70 Hz with a top "tick" (Armin); sub rolled off; de-emphasised vs house/techno; Armin duplicates the drum track to build toward the chorus | [trance#1][trance#3][trance#4] |
  | C | 5, 13 | 1.0 | white noise compressed hard, stereo, reverb | [trance#3] |
  | Open H | 3, 7, 11, 15 | engine default 0.8 | closed hats in low-energy sections | [trance#1][trance#22] |
  | Closed H | engine default 16ths at 0.4–0.6 | — | — | [trance#3] |
  | Crash | step 1 of the first bar of each 16-bar block | 1.0 | long reverb on crashes, short on hats | [trance#3][trance#22] |
  | Sn (build) | G4 roll; trance.md: last 8 bars ¼ 2 → ⅛ 2 → 1/16 2 → 1/32 1 → silence 1 | 0.01 → 1.0 | stop at beat 2.4 of the last bar (≈ 20 %); pitch +3–12 st | [trance#28] |

- **bass:** off-beat bass, one note on each of 3, 7, 11, 15, the kick kept
  short so they don't overlap; a sine layered under it; ducked by the kick or
  by a MIDI-keyed pump on every beat; verse bass "more straight" than the
  theme's. Hard trance: rolling bass on all 16 steps or off-beat, ducked ≥ 6
  dB (engine default amount). Register engine default MIDI 33–45. Sound
  `tranceBass` (`mono`) + sine. [trance#1][trance#3][trance#4][trance#15][trance#43]
- **harmony:**
  - modes: minor 0.88 (GiantSteps trance; trance.md ≥ 0.8 engine default),
    major 0.12 ("Out of the Blue"-style anthems). [trance#1][trance#33][cognition#7]
  - chord pool in minor: i, iv, v, VI, III, VII, weighted toward VI, III, VII
    so most chords are major. [trance#17]
  - progressions (weights engine default; no corpus says which order trance
    prefers): i–VI–III–VII 0.2; i–VI–VII–III (Humoresque in minor) 0.15;
    VI–VII–i 0.15; VI–iv–i ("Children") 0.12; i–v–VII ("For an Angel") 0.12;
    i–iv–III ("9 PM") 0.12; i–III–iv ("Sun and Moon") 0.08; major: I–vi7sus2–IV(add9)
    ("Out of the Blue") 0.06. [trance#29]–[trance#33][trance#45]
  - extensions: sus2, add9, m9 on 25–40 % of chord changes (engine default
    rate). Optional major V (raised 7th) before i at phrase ends (forum).
    [trance#30][trance#33][trance#45]
  - bars_per_chord 1 or 2 (engine default); loop_bars 4 or 8.
- **melody/hooks:** one central hook, 8 bars, reused across the track,
  repeating at intervals from 2 beats to 32 bars; narrow (≤ 7 st; "For an
  Angel" chorus G4–C5, 5 st), stepwise, ~1 note per beat (0.8 beats/note),
  all diatonic, ~60 % chord tones; chords and melody "quite straightforward",
  catchy on first hearing. Before the main breakdown play a sliced, simplified
  hook (every other note, or its first 2 bars); the final drop combines the
  full hook with earlier material. Armin: 8-bar theme, 8-bar verse variation,
  8-bar chorus, "four, five, six different sections". Rapid arps sit behind
  choirs/strings. Call-and-response between two lead phrases: no source
  (unverified). [trance#1][trance#3][trance#4][trance#17][trance#29]
- **sounds:** lead `supersaw` (knob 0.75, mix 0.75, LP 12 dB 4–8 kHz, ping-pong
  1/8 or dotted 1/8 fb 0.3–0.45, reverb RT60 2–4 s; "very clean Super Saw, a
  little detune, a little delay"); pads `superPad`/`superPadWide` (multi-saw
  strings, slow attack, long release, mostly in breakdowns; supersaw knob 0.5,
  24 dB 1–3 kHz); arps `pluckTrance`/`sqArp`; FM bells in breakdowns
  (`fmBell`); trance gate: a 16-step on/off pattern on the pad, 50–75 % of
  steps on, each gate ≤ one 16th (density engine default); off-beat pumping:
  duck pads and strings on every beat 4–8 dB, recovered by the off-beat.
  [trance#3][trance#4][trance#5][trance#17][trance#44][sound-design#13][sound-design#14]
  Missing: supersaw fixes (mix law, tracking HP, asymmetric spread, per-note
  random phase), trance-gate step automation, pump keyed from a pattern.
- **arrangement** (224 bars ≈ 6:30 at 138 [arrangement#22][arrangement#21][arrangement#2][arrangement#11];
  trance.md's own layout differs in counts, C8):

  | Role | 1–16 | 17–32 | 33–48 | 49–64 | 65–96 | 97–128 | 129–144 | 145–176 | 177–192 | 193–208 | 209–224 |
  |---|---|---|---|---|---|---|---|---|---|---|---|
  | K | x | x | x | x | x | - | - | x | x | x | x |
  | H | +9 | x | x | x | x | - | - | x | x | x | x |
  | C, P | - | + | x | x | x | - | r (Sn) | x | x | x | x |
  | B | - | + (off-beat) | x | x | x | - | f (pedal, HPF) | x | x | x | - |
  | Pd | - | - | f | x | x | x (LPF opening) | x | x | x | - | - |
  | A | - | - | + | x | x | - | x | x | x | - | - |
  | L | - | - | - | s (sliced teaser) | - (full minus lead) | x (full melody) | x | x (reprise) | - | - | - |
  | Sn, R | - | - | - | r 61–64 | - | - | r: ¼ 129, ⅛ 137, 1/16 141, 1/32 144 | - | - | - | - |
  | X | - | - | - | - | 1 | - | - | 1 | - | - | - |

  intro 16 (DJ) + 16; outro 16 + 16; total 224. Club mix 7–8 min, radio edit
  ~3.5 min. [trance#3]
- **energy** (engine default): 0.3, 0.45, 0.6, 0.7, 0.85, 0.35, 0.5→0.9,
  1.0, 0.8, 0.55, 0.4.
- **surprise targets:** second breakdown optional (1–2 per track) [trance#3];
  on the second pass of the loop start on a different chord of the same loop
  (forum) [trance#45]; the climax is the hook's return (exact), surprise goes
  into the chord colour and the build; rare (≤ 10 %): a real-instrument hook in
  place of the synth lead (ATB "9 PM" guitar) [trance#10][trance#20]; melody
  band engine default z −1.25 to −0.25 (narrow stepwise hook) (G9.11).
- **references:** Age of Love "The Age of Love (Jam & Spoon remix)" (1992);
  Jam & Spoon "Stella" (1992); Paul van Dyk "For an Angel" (1994/98); Robert
  Miles "Children"; Chicane "Offshore" (1996); Armin van Buuren "Blue Fear"
  (1997); ATB "9 PM (Till I Come)" (1998); System F "Out of the Blue" (1999);
  Darude "Sandstorm" (1999); Gouryella; Above & Beyond "Sun and Moon"; hard
  trance: Jones & Stephenson "The First Rebirth", Yves Deruyter "Rave City".
  [trance#2][trance#7][trance#9][trance#10][trance#11][trance#12][trance#15][trance#29]–[trance#33]

#### P15. Psytrance (Goa, full-on, progressive psy)

- **scope:** Goa trance (golden age 1994–97), psytrance/full-on (2004–09),
  night full-on, dark psy, progressive psy, Hamburg "offbeat", tribal.
- **tempo:** Goa 135–150 (some 110–160); psytrance 138–144; full-on 140–148;
  night full-on 145–150; dark psy 148–155+; progressive psy 130–138; Hamburg
  134–140; typical 142. (Beatport psytrance median 147, n = 5.)
  [trance#18][trance#19][trance#26][cognition#8]
- **swing:** 0; tribal psy may use triplet basslines. [trance#26]
- **meter/grid:** 4/4, 16 steps; 16-bar drop blocks separated by ~4-beat
  gaps. [trance#25]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 | 1.0 | sine, fast downward sweep, ≤ 1/8 note long, volume dip after the click (200–600 Hz thinned); "classic": lower sweep start, slower drop; "modern": faster, layered with clicks/short hats | [trance#22] |
  | C | 5, 13 | engine default 0.9 | 909 snare + clap + stereo snare, no low end | [trance#22] |
  | Open H | 3, 7, 11, 15 | engine default 0.8 | closed hats in low-energy sections | [trance#22] |
  | Top loop | all 16 (closed) | engine default 0.5 | mid/high energy | [trance#22] |
  | Crash | start or end of a 16-bar passage | 1.0 | — | [trance#22] |

- **bass:** KBBB: bass on 2–4, 6–8, 10–12, 14–16, each ≤ 1 step (ideally
  shorter), no overlap; the first B after each K 2–4 dB quieter (direction
  sourced, amount engine default); kick tail overlaps the first bass 16th on
  the same root; saw → LPF, amp attack 0, release ≤ 10 ms, filter env opening
  fully and closing in ~5–30 ms (numbers engine default); classic = free-running
  phase and slower filter env, modern = reset phase. Root in E–G (engine
  default). Full-on: across 2–3 octaves within a bar. Progressive psy /
  Hamburg: off-beat bass, one 8th on 3, 7, 11, 15. Tribal: triplet bass.
  Sound `psyBass` (`mono`). [trance#22][trance#26][trance#27][trance#48]
- **harmony:** modal single-chord or drone harmony; modes Phrygian 0.45
  (Goa "oriental"), Dorian 0.3, Lydian 0.1, Aeolian 0.15 (weights engine
  default; a "Phrygian dominant" default is unverified); minor 5/5 in
  GiantSteps. bars_per_chord: whole section. [trance#18][trance#23][trance#26][cognition#7]
- **melody/hooks:** leads are monotone 16th-note step sequences whose rhythm
  comes from note jumps (octave or 5th), the filter opening slowly over 8–16
  bars; FM leads fast and mostly monotone, shaped by HP movement; "grids":
  short FM shots, chord stabs (2nd osc +7 st; supersaws most common), plucks.
  Full-on changes synth lines regularly (re-roll the lead every 8 bars);
  progressive psy keeps lines and modulates them. [trance#23][trance#24]
- **sounds:** `psyBass`; leads `fm` (4-op FM shots; more index = more
  aggressive) and `tb303`/`acid` for Goa squelch through resonant BP/HP;
  "fractal": sample & hold on a resonant band-pass cutoff at a 16th rate on a
  saw; FX: filter sweeps, pitch dives, long ping-pong delays (dotted quarter or
  8th), white-noise risers with volume and filter modulation layered with
  rising synths; Goa used TB-303, Juno-60/106, SH-101, MS-10.
  [trance#18][trance#23][trance#24] Missing: sample & hold modulation, BP/HP
  modes per voice, pitch-dive FX.
- **arrangement** (Bonedo [trance#25][arrangement#21]; bar counts engine
  default; 288 bars ≈ 8:07 at 142):

  | Role | 1–32 | 33–48 | 49–64 | 65–80 | 81–96 | 97–112 | 113–128 | 129–176 | 177–208 | 209–224 | 225–256 | 257–288 |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | K | x (sparse) | x | x | x | x | - | r (HP/LP sweeps) | x | - | r | x | x |
  | B (KBBB) | + 17 | x | x | x | x | - | f | x | - | f | x | x |
  | Closed H | - | - | + | x | x | - | x | x | - | x | x | - |
  | C | - | - | - | + | x | - | x | x | - | x | x | - |
  | Open H | - | - | - | - | + | - | - | x | - | - | x | - |
  | L (16th seq / FM) | - | - | s | x | v | - | f | v (re-rolled every 8 full-on) | x (with pads) | f | x | - |
  | Pd, Atm | x | - | - | - | - | x | x | - | x (main breakdown) | x | - | - |
  | Gap | - | 4-beat gaps between 16-bar blocks | | | | short break (≤ 16, no K/B) | 4 beats before drop | gaps | | 4 beats | | |

  drop entry: kick + bass only, closed hat at +16, snare at +32, open hat at
  +48 [trance#25]; main breakdown at ~2/3, ≤ 64 bars; 2–3 drops; outro reduces
  to kick + bass [trance#25]; intro sparse, meant to blend with the previous
  track's outro [arrangement#21]; length 7–9 min (Goa 8–12) [trance#18][trance#25].
- **energy** (engine default): 0.4, 0.55, 0.65, 0.75, 0.85, 0.3, 0.6→0.9,
  0.95, 0.35, 0.6→0.95, 1.0, 0.5.
- **surprise targets:** drop variant: a stripped start (kick + bass) or a full
  start that thins out [trance#25]; full-on re-rolls the lead every 8 bars,
  progressive psy modulates filter, phaser or pitch instead [trance#24];
  speeding kick in builds [trance#25]; IC bands per G9.7 on lead pitch
  intervals (few pitches, so z-scores against the profile's own corpus).
- **references:** The KLF "What Time Is Love? (Pure Trance)" (precursor);
  *Project II Trance* (1993, Man With No Name, Hallucinogen); Astral
  Projection *Trust in Trance 2*, *Dancing Galaxy* ("People Can Fly",
  "Mahadeva", "Kabalah"); Infected Mushroom *The Gathering* (1999), "Bust a
  Move" (2000), *IM the Supervisor* (2004), "Becoming Insane" (2007);
  Xenomorph, X-Dream (dark psy roots). [trance#18][trance#19][trance#20][trance#21]

#### P16. Progressive (house and trance)

- **scope:** progressive house/trance from the UK, early 1990s (Sasha &
  Digweed, Renaissance); the progressive house of house.md §7.
- **tempo:** progressive trance 126–132; progressive house 127 (Beatport,
  IQR 127–129); typical 128. [trance#2][cognition#8]
- **swing:** engine default 50–52.
- **meter/grid:** 4/4; phrases of 4, 8 or 16 bars, each often starting with
  a new melody or rhythm. [house#40][trance#16]
- **drums:** four-on-the-floor (P14 grid); no drum rolls, crescendos or
  anthemic choruses; builds by filters and layers. [house#40][trance#16]
- **bass:** unverified in the sources; engine default: off-beat or rolling
  16th bass with filter movement (P14 bass rules), `moogBass`/`tranceBass`.
- **harmony:** minor 0.76, major 0.24 (GiantSteps progressive house)
  [cognition#7]; progressions engine default: the P14 pool without the major V,
  bars_per_chord 2–4, loop_bars 8–16 (long harmonic rhythm to support slow
  builds).
- **melody/hooks:** each 4/8/16-bar phrase may start with a new melody or
  rhythm; no anthem chorus. [house#40][trance#16]
- **sounds:** `junoPad`, `pluck` arps, `supersaw` pads at knob 0.5, `fmBell`;
  loudness −10.5 LUFS. [sound-design#35]
- **arrangement** (layers added or removed every 4, 8 or 16 bars; a build of
  up to ~4 minutes ≈ 128 bars at 128 BPM, then breakdown and climax
  [house#40][trance#16][arrangement#26]; bar counts engine default):

  | Role | 1–32 | 33–48 | 49–64 | 65–80 | 81–96 | 97–112 | 113–128 | 129–160 | 161–192 | 193–224 | 225–256 |
  |---|---|---|---|---|---|---|---|---|---|---|---|
  | K, H | x | x | x | x | x | x | x | x | - | x | x |
  | C, P | - | + | x | v | x | v | x | x | - | x | x (C out 241) |
  | B | - | + (f) | x | x | x | x | x | x | - | x | - |
  | Pd | - | - | f | f (opening) | x | x | x | x | x | x | - |
  | A | - | - | - | + | x | v | x | x | - | x | - |
  | L | - | - | - | - | s | x | v | x | x | x | - |
  | Atm/FX | x | x | x | x | x | x | x | x | x | x | x |

  intro 32; outro 32; total 256 (8:00 at 128).
- **energy** (engine default; one long ramp): 0.3, 0.4, 0.5, 0.58, 0.66,
  0.74, 0.82, 0.9, 0.4, 1.0, 0.45.
- **surprise targets:** a new melody or rhythm at the start of a phrase
  instead of fills; filter moves carry the build [house#40][trance#16]; one
  layer change per 4–16 bars (G2).
- **references:** Leftfield "Not Forgotten" (1990); Gat Decor "Passion";
  Spooky "Don't Panic"; Slam; Sasha "Xpander" (1999); *Renaissance: The Mix
  Collection* (1994); *Northern Exposure* (1996). [trance#2][trance#16]

#### P17. Eurobeat

- **scope:** Italian Eurobeat for Japan (Avex, Super Eurobeat from 1990;
  A-Beat-C, Time, Delta); Hi-NRG roots in P20.
- **tempo:** 150–160, typical 155; Japanese Wikipedia: 120–160 overall, ~125
  for 1980s Eurobeat, ≥ 140 after Super Eurobeat. [trance#36][trance#37][trance#40]
- **swing:** 0. [trance]
- **meter/grid:** 4/4, 16 steps; 8-bar riff units. [trance#36]
- **drums:** four-on-the-floor, "tight percussion"; snare/clap 5, 13 and
  off-beat open hats (engine default; no eurobeat-specific source).
  [trance#35][trance#40]
- **bass:** 8th-note octave bass: low root on 1, 5, 9, 13, the octave above on
  3, 7, 11, 15 (order engine default), following the chord root; "off-beat
  basslines" agree (the high note is on the off-beat); the FM octave bassline
  was established by Dead or Alive's "You Spin Me Round". Sound: `fm` octave
  bass (`dxBass` style). Register engine default MIDI 33–45 low note.
  [trance#35][trance#36][trance#40]
- **harmony:** minor 0.9 (engine default); verse i–VI–VII–i–v; chorus
  VI–VII–i–VII(–VI) ("Deja Vu"); the minor VI–VII–i rise as in trance.
  bars_per_chord 1 (engine default; 2 chords per bar allowed in the chorus).
  **Key change:** modulate the last chorus up 1 or 2 st in 60–80 % of songs
  (engine default rate; direction from a genre blurb and J-pop convention).
  [trance#34][trance#40][trance#42]
- **melody/hooks:** an ~8-bar synth riff after the intro and after every
  chorus, weighed equally with verse, bridge and chorus, stacked up to 20
  tracks; vocal-style top line in verse (A-melo), bridge (B-melo) and chorus;
  English vocals. [trance#35][trance#36]
- **sounds:** lead `euroLead` (`mono`); FM octave bass; brass stabs (`juno`
  or `mono` `brassLead`); harp/bell arps (`fm` `bell`, `string` pluck);
  optional overdriven guitar (`string` with drive); 1990s gear Yamaha SY/EOS,
  Roland JD-800/990. [trance#35][trance#36] Missing: a harp voice (a
  Karplus-Strong pluck could stand in), a vocal-style lead (formant/vowel).
- **arrangement** (order [trance#35][trance#36]; bar counts engine default; 128
  bars ≈ 3:18 at 155; ~3.5–5 min total):

  | Role | 1–16 intro | 17–24 riff | 25–32 A | 33–40 B | 41–56 chorus | 57–64 riff | 65–72 A | 73–80 B | 81–96 chorus | 97–112 chorus (+1/+2 st) | 113–120 riff | 121–128 ending |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | K, C, H | x | x | x | x | x | x | x | x | x | x | x | f |
  | Octave bass | x | x | x | x | x | x | x | x | x | x | x | f |
  | Riff (synth hook) | s | x | - | - | s | v (+1 layer) | - | - | s | s | v (+1 layer) | x |
  | Lead (vocal-style) | - | - | x | x | x | - | x | x | x | x | - | - |
  | Pd, brass | x | x | s | x | x | x | s | x | x | x | x | x |
  | Arp (harp/bell) | - | - | x | x | x | - | x | x | x | x | - | - |

- **energy** (engine default): 0.6, 0.85, 0.6, 0.75, 0.95, 0.85, 0.6, 0.75,
  0.95, 1.0, 0.9, 0.6.
- **surprise targets:** the final-chorus key lift is the main surprise; the
  riff is re-voiced with an extra layer each time it returns (exact notes)
  [trance#35][trance#40][trance#42].
- **references:** Max Coveri "Help Me to Say Goodbye" (1996), "Running in the
  90s"; Dave Rodgers "Space Boy" (1998), "Deja Vu", "Beat of the Rising Sun"
  (1999); Niko "Speedway" (2000), "Night of Fire"; Manuel "Gas Gas Gas"; Dead
  or Alive "You Spin Me Round" (1984, FM octave bass). [trance#35][trance#36][trance#38][trance#49]

### Synthwave family

Shared by P18–P20: hard-quantised drums [synthwave#2][synthwave#3][synthwave#16];
Juno-style poly as the default voice (saw + PWM, sub 0–10, 24 dB LPF, HPF for
layers, chorus I or II or both, always on for pads, arps and leads)
[synthwave#29][synthwave#38][synthwave#50]; chorus on most buses, tape
saturation on drums, bass and synths [synthwave#47][synthwave#52]; gated
reverb on snare, clap and toms (100 % wet send, pre-delay 0, hold 300–500 ms,
release < 50 ms, compress before the reverb, hats < 150 ms hold or none)
[synthwave#25][synthwave#26]; the ♭VI–♭VII (VI–VII) ascent resolving to i is
the family's cadence [synthwave#23]; one chord per bar over 4-bar loops
[synthwave#38][synthwave#39]. Loudness engine default −10 LUFS (no measured
median in the research).

#### P18. Synthwave / outrun (dreamwave and darksynth as modes)

- **scope:** synthwave/outrun/retrowave (mid-2000s–), dreamwave
  (Timecop1983, The Midnight), darksynth (Perturbator, Carpenter Brut); the
  film/TV-score roots (Carpenter, Tangerine Dream, Vangelis, Hammer,
  Faltermeyer) as references.
- **tempo:** bimodal: dreamwave/night-drive 86–116 (Attack's synthwave beat
  95–115; ModeAudio's slow band 80–118), outrun/darksynth 128–147 (ModeAudio
  128–140); typical 110 / 132. [synthwave#2][synthwave#3][synthwave#23][synthwave#24]
- **swing:** 50 (no swing). [synthwave#3]
- **meter/grid:** 4/4, 16 steps; one 32nd hat; half-time feel under 100 BPM
  with p = 0.3 (backbeat on 13 only, or snare on 9 at half time). [synthwave#2][synthwave#3][synthwave#23]
- **drums:**

  | Lane | Steps (synthwave 95–115) | Steps (outrun 128–140) | Vel | Variations / fills | Source |
  |---|---|---|---|---|---|
  | K | 1, 9; bar 2 + 16 | 1, 5, 9, 13 (+ ghost 15 at −20 %) | 1.0, little variation | 5–15 % wet gated reverb on the kick | [synthwave#3][synthwave#11][synthwave#12][synthwave#53] |
  | Snare | 5, 13 | 5, 13 (clap) | 1.0 | gated reverb | [synthwave#3][synthwave#9] |
  | Closed H | all 16, ±10–15 % | 3, 7, 11, 15 or all 16 | 0.6–0.8 | dreamwave mode: no hats or cymbals, p = 0.4 | [synthwave#3][synthwave#11][synthwave#50] |
  | Open H | ~14 (once per bar, impetus into the next bar) | 3, 7, 11, 15 or the last off-beat of a 2-bar loop | engine default 0.7 | — | [synthwave#3][synthwave#11] |
  | Clap | bar 2: 10, 11, 14, 15 | 5, 13 | engine default 0.8 | — | [synthwave#3] |
  | 32nd hat | one, before 13 of bar 2 | — | engine default 0.5 | rogue 32nd hat at a turnaround | [synthwave#3] |
  | Fills | snare 16ths on 13–16, or tom runs high → low on 9–16, every 4th or 8th bar; "big, boomy toms" | same | ramp | extra snare hits at the bar end into the next section | [synthwave#2][synthwave#12] |
  | Cowbell (808) | colour, choruses | — | — | — | [synthwave#2] |

  Kits: DrumTraks-style short digital samples (synthwave), Linn/LM-1 (outrun),
  808 (electro); tune the snare to a related pitch of the kick (e.g. 2 octaves
  up). [synthwave#3][synthwave#19][synthwave#27][synthwave#28]
- **bass:**
  - default: the root repeated in 8ths (1, 3, 5, …, 15), plucked: saw (+ square
    or sub), cutoff ~150–250 Hz, env amount high, filter decay 400–500 ms,
    sustain 0, amp decay ~650 ms, sustain ~0.3, mono unison 5–9 cents
    ("A Real Hero": Prophet-5 mono unison, osc 2 +9 cents, cutoff 171 Hz,
    env 3.4, filter decay 490 ms). [synthwave#2][synthwave#39][synthwave#50]
  - Nightcall chorus bass: two saws an octave apart, LPF ~1.1 kHz, drive; a
    piano doubling the bass in octaves. [synthwave#38]
  - resonant sweep bass: saws an octave apart, res 60–80 %, cutoff ~700 Hz
    with a 2.5 s downward env sweep, delayed vibrato and filter LFO at ~5.3 Hz
    faded in over ~1 s, plus a sub low-passed at ~220 Hz. [synthwave#38]
  - 80s bass stack: saws at −2 oct (+9 cents), −2 oct (−4 cents), −1 oct (+6
    cents), 4-pole LPF, filter decay ~40 %, sustain ~35 %, a few ms attack,
    chorus with low cut. [synthwave#52]
  - Timecop1983 plucked bass: Juno saw + sub (6/10 light, 10/10 heavy),
    cutoff 2–3/10, env 5/10, ADSR ≈ 0/4/3/4, chorus I. [synthwave#50]
  - register engine default root MIDI 33–45; detune 3–10 cents for beating.
    [synthwave#2]
- **harmony:**
  - modes: minor/Aeolian 0.6, Dorian 0.15, Phrygian 0.05, major 0.15,
    Mixolydian 0.05; about three-quarters of the sample is minor or Dorian;
    minor = "brooding, soundtrack", major = "cheesier synth-pop". [synthwave#2][synthwave#23]
  - minor pool (weights engine default, VI–VII ascent weighted high):
    i–VII6–VI–iv ("Nightcall" verse, VII over its 3rd) 0.15; VI–VII–v–VI
    ("Nightcall" chorus) 0.12; VI(add9)–VII–i7 ("Love on a Real Train") 0.12;
    i–VI7–VII–i ("Axel F") 0.1; i–III–VII 0.1; VI–VII–III–VII–i ("Megamagic")
    0.08; III–iv–VI–i–VII ("Venger") 0.08; iv–i–III–V ("Odd Look") 0.05;
    Dorian: i–III–v–IV ("Turbo Killer") 0.08, i–v–VII–IV ("Blinding Lights")
    0.07, i–III–IV 0.05. [synthwave#23][synthwave#38]
  - major pool: IV–V–vi–I (Timecop1983) 0.4; I–♭VII–♭VI–♭VII (Italo) 0.25;
    I–♭III–♭VII–♭VI–IV (Com Truise, Mixolydian) 0.2; I–V–vi–IV–V ("Tarzan
    Boy") 0.15. [synthwave#11][synthwave#23][synthwave#50]
  - extensions: add9, 11, sus2, m7 on 30–50 % of chords in dreamwave/Mitch
    Murder mode; triads in darksynth; slash VII over its 3rd p = 0.3;
    darksynth allows a harmonic-minor V and a ♭II. [synthwave#23][synthwave#38]
  - bars_per_chord 1 (ModeAudio also mentions four chords per bar); loop_bars 4.
    [synthwave#2][synthwave#38]
- **melody/hooks:** the arp is the hook: 8 notes per bar (8ths) spelling the
  chord ("Nightcall") or off-beat-only 8ths (3, 7, 11, 15; "A Real Hero"); a
  second sparser arp layer (2-note pattern, −10 dB) and a dry mono doubling;
  lead monophonic, sparse ("a few well-chosen notes"), mostly stepwise, ≥ 95 %
  diatonic, ~1.4 beats per note, range 12–15 st ("Turbo Killer" F4–A♭5);
  scale runs doubled in octaves; solo section with a sax-like or guitar-like
  lead, pitch bend (±2 st, unverified) and vibrato; vocoder verses, clean
  chorus hooks. [synthwave#2][synthwave#23][synthwave#34][synthwave#38][synthwave#39][synthwave#48][synthwave#51]
- **sounds:**

  | Role | Engine | Recipe | Source |
  |---|---|---|---|
  | Arp | `juno` `pluck`/`sqArp` | saw, no sub, HPF on, cutoff mid, res low–moderate, pluck env with slight attack, chorus I; "A Real Hero" arp: soft-attack unison saw, attack 68 ms, decay 1.2 s, chorus, 25 % delay, 50 % reverb, compressed after the FX | [synthwave#38][synthwave#39] |
  | Pads/strings | `juno` `warmPad` | saw + thin pulse (PW 85 %), osc B +12 cents, LPF ~280 Hz, filter attack 44 ms, decay 5.4 s; or Juno strings ADSR ≈ (0.1, 0.6, 0.8, 0.6), chorus II; warbling detuned pads over LFO-filtered noise | [synthwave#2][synthwave#38][synthwave#50] |
  | Lead | `mono` `sawLead`/`sqLead` | saw or square, vibrato 4.5–5.5 Hz delayed, reverb 30–50 %, delay ~25 %; "Under Your Spell": 3 saws −13/+10 cents, vibrato 4.56 Hz, drive | [synthwave#2][synthwave#39] |
  | Brass stabs | `juno` unison saw | filter-env brass on chord hits (Prophet/OB style) | [synthwave#14][synthwave#43][synthwave#47] |
  | EP, bells | `fm` `ep`, `bell` | velocity → modulation index (Hammer: "upper harmonics pop up") | [synthwave#30][synthwave#34] |
  | Bass | `mono` / `juno` | as above | [synthwave] |
  | Drums | `tr808` / Linn | `gatedVerb`, `lofiPCM` for Linn/DrumTraks | [synthwave#3][synthwave#27] |

  Reverbs: hall ≥ 3 s for dreamwave; compress after delay and reverb; darksynth
  and Kavinsky: hard bus compression, light saturation or bit reduction;
  detune the whole track 0–35 cents flat with p = 0.1 (*Drive*'s "flat tape").
  [synthwave#38][synthwave#39][synthwave#41][synthwave#50] Missing: gated
  reverb, sax/guitar-like lead with bend, vocoder, CS-80-style aftertouch swell,
  osc sync, Linn/DrumTraks sample character, whole-mix detune.
- **arrangement** (instrumental/DJ form: drums only (kick + snare) first,
  hats at 9, bass at 17, one element in or out every 4 or 8 bars, FX sweep at
  each 8- or 16-bar boundary [synthwave#2][synthwave#11]; section order from
  the pop template [synthwave#12][synthwave#24]; bar counts engine default; 128
  bars ≈ 4:39 at 110):

  | Role | 1–8 intro | 9–16 | 17–24 verse | 25–32 | 33–48 chorus | 49–56 verse 2 | 57–72 chorus 2 | 73–80 middle eight | 81–88 solo | 89–96 break | 97–112 chorus ×2 | 113–128 outro |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | K, C | x | x | x | x | x | x | x | x (+3 st option) | x | x (95–96 drums only) | x | f (fade) |
  | H | - | + | x | x | x | x | x | x | x | - | x | - |
  | B | - | - | + | x | x | x | x | x | x | - | x | x (hook bass) |
  | A | - | - | - | + | x | x | x | s | x | - | x | - |
  | Pd | - | - | - | - | x | s | x | x | x | x | x | x |
  | L | - | - | - | - | x | - | x | - | solo (sax/guitar-like) | - | x | x (reverb-drenched) |
  | R, X | - | - | - | r (sweep) | 1 | - | 1 | - | - | r (reversed reverb riser 95–96) | 1 | - |

  Pop/vocal form: intro 4–8, verse 4 or 8, chorus 8, verse, chorus, middle
  eight 8–12, solo 4–8, drum break 2 + riser, chorus ×2, outro fade; target
  3:30–6:30. Design the peak first (a 6–7 s loop with all parts) and derive the
  intro and breakdown by subtraction. [synthwave#12][synthwave#24][synthwave#50]
- **energy** (engine default): 0.3, 0.4, 0.55, 0.6, 0.85, 0.6, 0.9, 0.7, 0.8,
  0.35, 1.0, 0.5.
- **surprise targets:** key change in the middle eight: +3 st (minor to minor a
  third up) then return, or a solo in the major key a 5th above, p = 0.15
  [synthwave#12]; bass-timbre snap: filter open → closed at the midpoint of a
  4-bar phrase after a chorus ("laser gun") [synthwave#51]; delayed vibrato or
  filter-LFO swell on long notes [synthwave#38]; siren synth over 2–4 bars in
  breaks and the outro [synthwave#12]; a rogue 32nd hat or extra kick on 16 at a
  turnaround [synthwave#3]; start off the downbeat (open hat before bar 1) p =
  0.1 [synthwave#12]; whole-mix pitch offset up to −35 cents or slight bit
  reduction as a "tape/VHS" variant [synthwave#38][synthwave#39].
- **references:** Kavinsky "Nightcall" (2010), "Odd Look", "Protovision",
  "Testarossa Autodrive"; College & Electric Youth "A Real Hero"; Desire "Under
  Your Spell"; the *Drive* soundtrack (2011); Perturbator "Future Club",
  "Venger", *Dangerous Days* (2014); Carpenter Brut "Turbo Killer", "Roller
  Mobster", *Trilogy*; The Midnight "Sunset", "Los Angeles", "Deep Blue";
  Timecop1983 "On the Run"; FM-84 "Running in the Night"; Lazerhawk
  "Overdrive"; Gunship "Tech Noir"; Mitch Murder "Megamagic"; roots: John
  Carpenter *Halloween* (1978), Tangerine Dream "Love on a Real Train" (1983),
  Vangelis *Blade Runner* (1982), Jan Hammer "Miami Vice Theme", "Crockett's
  Theme", Harold Faltermeyer "Axel F" (1984), *Out Run* (Sega, 1986), The
  Weeknd "Blinding Lights" (2020). [synthwave#1][synthwave#23][synthwave#24][synthwave#32][synthwave#33][synthwave#35][synthwave#36][synthwave#38][synthwave#39][synthwave#40]

#### P19. Electro

- **scope:** early-80s electro (Tokyo, New York, Detroit), Miami bass,
  Detroit/afrofuturist electro (Drexciya, Dopplereffekt), lo-fi electro and the
  revivals.
- **tempo:** 120–135, typical 127 ("Planet Rock" 126–127, "Wavejumper" ~127,
  "Clear" style 125); Miami bass 130–135; lo-fi electro 130–135.
  [synthwave#17][synthwave#18][synthwave#19][synthwave#23][synthwave#24]
- **swing:** 50; lo-fi electro variant 55–65. [synthwave#17][synthwave#18][synthwave#19]
- **meter/grid:** 4/4, 16 steps, 2-bar loops (bar A / bar B); tempo drift
  ±0.5 BPM allowed. [synthwave#12][synthwave#17]
- **drums** (`tr808`; drum bus driven and glued at ~4 dB [synthwave#17];
  velocity two levels only, normal and accent; snare only on the backbeat, no
  ghost notes; every kick and snare at full velocity [synthwave#16]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K ("Clear") | bar A: 1, 4, 7, 11; bar B: 1, 7, 11 | 1.0 | alternate A/B bars | [synthwave#17] |
  | K (Miami bass) | bar 1: 1, 7; bar 2: 1, 7, 11, 14 | 1.0 | long-decay `kick808` | [synthwave#18] |
  | Snare + clap | 5, 13 (+ rimshot layer in Miami) | 1.0 | last snare of the phrase doubled with a clap | [synthwave#16][synthwave#17][synthwave#18] |
  | Closed H | "Clear": 1–4, 9–12 (four-16th bursts); Miami: all 16ths except 2, 6, 10, 14 | normal / accent | move a hat gap every 4 bars | [synthwave#17][synthwave#18] |
  | Open H | "Clear": 5, 13; Miami: 2 (bar 1), 13 (bar 2); phrase end: 16 of the last bar | accent | — | [synthwave#16][synthwave#17][synthwave#18] |
  | Cowbell | busy, syncopated | normal | only in choruses or breakdowns | [synthwave#16] |
  | Perc / zap | mostly 8ths, avoiding the hat gaps | normal | — | [synthwave#18] |
  | Lo-fi variant | K long on each downbeat + short on the following off-beat; snare tuned 2 octaves above the kick (≈ 52 / 210 Hz, both G♯); 16th hat bursts at the bar start; bell perc band-passed at 1 kHz with a slow LFO | — | TR-606/707 sounds | [synthwave#19] |

  Kit Witek target: groove kit 22–46 (engine default; electro is syncopated
  machine breakbeat, "more mechanical" than live breaks [synthwave#13]).
- **bass:** a 303- or SH-101-style mono line, one pattern per section (a new
  pattern for the bridge, back in the chorus); raise resonance for squelch;
  laser-gun bass: SH-101 saw with sub at 32′, high resonance, decaying VCA,
  cutoff snapping open → closed halfway through a 4-bar loop.
  [synthwave#2][synthwave#12][synthwave#51] Register engine default MIDI 33–48.
  Sound `tb303` or `mono` `acid`-style.
- **harmony:** modes: minor 0.5, Phrygian 0.3 ("Planet Rock" B Phrygian),
  Dorian 0.1, major 0.1 (weights engine default); progressions: i–v vamp
  ("The Model", with a bridge in the parallel major) 0.4; i–III–VII ("Computer
  World") 0.3; one-chord/riff 0.3. bars_per_chord 1–2; loop_bars 2–4.
  [synthwave#23]
- **melody/hooks:** sequenced or arpeggiated riffs; vocoded or
  speech-synthesised vocals; atonal FX, synthetic strings; "Planet Rock"
  combined Kraftwerk's "Trans-Europe Express" melody with "Numbers"' beat.
  [synthwave#13][synthwave#14]
- **sounds:** `tr808` (cowbell, clap, toms, rim; claves only in the `latin`
  kit); `tb303`/`mono` bass; `juno` strings; FX: vocoder or robotic speech,
  orchestra hits, explosion/noise sweeps, sonar pings (Drexciya) → `fm` pings.
  [synthwave#13][synthwave#14][synthwave#21] Missing: vocoder/speech voice,
  orchestra hit, explosion FX, claves in the 808 kit.
- **arrangement** (Tiga "Bugatti" (electro/techno) entries [arrangement#18];
  cowbell held for choruses/breakdowns [synthwave#16]; "Planet Rock"'s
  orchestra-hit and explosion break [synthwave#14]; bars after 81 engine
  default):

  | Role | 1–16 | 17–32 | 33–48 | 49–72 | 73–80 | 81–96 | 97–120 | 121–128 | 129–160 | 161–176 |
  |---|---|---|---|---|---|---|---|---|---|---|
  | K (A/B bars) | x | x | x | x | x | - | x | x | x | x |
  | Snare + clap | x | x | x | x | x | - | x | x | x | x |
  | H | s | x (ride creeps in) | x | x | - (ride out) | - | x | x | x | x |
  | Lead hook | x | x | x | x | - (synths out) | s | x | f | x | - |
  | 2nd synth riff | - | + (anacrusis) | x | x | - | - | x | f | x | - |
  | V (vocoder) | - | - | + (anacrusis) | x | - | x | x | f | x | - |
  | Cowbell, perc | - | - | - | x | - | x | x | - | x | - |
  | B | - | - | - | - | - | - | + (after 3:00) | x | x | - |
  | FX | - | - | - | - | orchestra hit, explosion | x | - | full-mix reverse into 129 | 1 | - |

  intro 16; outro 16; total 176 (≈ 5:30 at 128); 12-inch electro and Italo run
  4:30–9:00. [synthwave#24]
- **energy** (engine default): 0.5, 0.6, 0.7, 0.85, 0.4, 0.55, 0.9, 0.7, 1.0,
  0.5.
- **surprise targets:** alternate kick bars and move a hat gap every 4 bars
  [synthwave#17][synthwave#18]; laser-gun filter snap [synthwave#51];
  cowbell enters only in choruses/breakdowns [synthwave#16]; full-mix reverse
  [arrangement#18]; tempo wander ±0.5 BPM [synthwave#12].
- **references:** YMO/Sakamoto "Riot in Lagos" (1980); Kraftwerk "Numbers",
  "Computer World" (1981), "The Model"; Afrika Bambaataa "Planet Rock" (1982);
  Warp 9 "Nunk"; Hashim "Al-Naafiysh", Herbie Hancock "Rockit", Cybotron
  "Clear" (1983); Drexciya *Bubble Metropolis*, "Wavejumper"; Dopplereffekt
  "Pornoactress"; Tiga "Bugatti". [synthwave#13][synthwave#14][synthwave#20][synthwave#21][synthwave#22][arrangement#18]

#### P20. Italo disco (Moroder / Hi-NRG as modes)

- **scope:** Italo disco (~1978–88), Moroder/Eurodisco ("I Feel Love"),
  Hi-NRG; Italo house and Eurobeat are its descendants (P1, P17).
- **tempo:** Italo 103–122, typical 118 ("Self Control" 107, "I Like Chopin"
  103, "Dolce Vita" 120–122, "Problèmes d'Amour" ~120.7); Moroder 126–128;
  Hi-NRG 120–140 ("high energy" 130–140). [synthwave#9][synthwave#12][synthwave#23][synthwave#24]
- **swing:** 50–53 (Linn LM-1 disco beat 50–53 at 110–120). [synthwave#28]
- **meter/grid:** 4/4, 16 steps, 2-bar loops; Moroder bass = 8-step 8th
  sequence plus a 1/16-offset delayed copy. [synthwave#8][synthwave#11]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1, 5, 9, 13 + 15 of bar 1 | 1.0; 15 at 0.8 | enters "some bars into the first chorus" (Robotnick) | [synthwave#11][synthwave#12] |
  | Clap | 5, 13 | 1.0 | heavy drum-machine clap (Hi-NRG) | [synthwave#9][synthwave#11] |
  | Closed H | 3, 7, 11, 15 (NI) or all 16 (Robotnick) | engine default 0.7 | staccato (Hi-NRG); Moroder's hat was envelope-shaped white noise | [synthwave#4][synthwave#9][synthwave#11][synthwave#12] |
  | Open H | last off-beat of the 2-bar loop (NI) or 3, 7, 11, 15 (Robotnick) | engine default 0.8 | — | [synthwave#11][synthwave#12] |
  | Rim | pattern panned left | engine default 0.6 | — | [synthwave#12] |
  | Fills | snare fills close each 8-bar block | — | 2-bar drums-only break ending in a reversed-reverb riser | [synthwave#12] |
  | Eurodance descendant | tambourine all 16; syncopated conga | — | — | [synthwave#53] |

- **bass:** sequenced or arpeggiated bass; a 303 line that changes pattern per
  section (Robotnick); Moroder: an 8-step 8th-note sequence alternating root
  and octave (Hi-NRG octave alternation), plus a delayed copy offset by one
  16th, hard-panned (L dry / R echo), which makes it sound like 16 steps; Hi-NRG
  octave basslines can take the place of the hi-hat. Register engine default
  MIDI 33–45. Sound `moogBass`, `tb303`. [synthwave#4][synthwave#8][synthwave#9][synthwave#10][synthwave#12]
- **harmony:**
  - modes: minor 0.55, major 0.3, Dorian 0.15 (weights engine default; examples
    span minor, major and F♯ Dorian). [synthwave#23]
  - progressions: i–v ("Self Control") 0.2; I–♭VII–♭VI–♭VII (NI tutorial,
    lead avoids the minor third) 0.2; I–V–vi–IV–V ("Tarzan Boy") 0.15; ii–V–I
    ("Dolce Vita" verse) 0.15; I–♭III–IV–V ("I Feel Love") 0.15; i–III–VII–i–IV
    ("You Spin Me Round", Dorian) 0.15. [synthwave#11][synthwave#23]
  - extensions: triads default; m7 on minor chords p = 0.3 (engine default).
  - bars_per_chord 1–2; loop_bars 2–4; middle eight modulates +3 st (G minor →
    B♭ minor), a synth solo in the major key a 5th above (D major). [synthwave#12]
- **melody/hooks:** a 4-bar hook; vocal-style top line with accented English;
  vocoder; a counter-melody with 68 % delay and 72 % reverb; a siren synth
  rising and falling in the instrumental break and outro.
  [synthwave#10][synthwave#11][synthwave#12]
- **sounds:** drums `tr808` / Linn (`lofiPCM`); bass `moogBass`/`tb303`;
  chords `juno`; lead `mono`; vocoder (missing). [synthwave#10][synthwave#11]
  Missing: vocoder, stereo delay with a 1/16 offset copy panned opposite
  (Moroder), Linn sample character.
- **arrangement** (Robotnick's sections: a 4-bar hook, 4-bar verse, 8-bar
  choruses, 12-bar middle eight, verse/chorus "stretched beyond pop length"
  [synthwave#12]; NI: elements in or out every 4 or 8 bars with an FX sweep at
  each change [synthwave#11]; bar counts engine default; 12-inch versions run
  3:56–8:56 [synthwave#24]):

  | Role | 1–8 intro | 9–12 hook | 13–16 verse | 17–24 chorus | 25–28 hook | 29–32 verse | 33–40 chorus | 41–52 middle eight (+3 st) | 53–60 solo (V major) | 61–62 break | 63–78 chorus ×2 | 79–94 outro |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | Hats, rim | x | x | x | x | x | x | x | x | x | x | x | x |
  | K, clap | - | - | - | + 19 | x | x | x | x | x | x | x | x |
  | B (per-section pattern) | - | x | v | x | x | v | x | v (new) | x | - | x | x |
  | Hook / lead | - | x | - | x | x | - | x | s | solo | - | x | s |
  | Chords | - | x | x | x | x | x | x | x | x | - | x | f |
  | Siren / FX | - | - | - | - | - | - | - | x | - | r (reversed reverb riser) | - | x |

  12-inch mode repeats the verse/chorus pair once more (engine default).
- **energy** (engine default): 0.35, 0.55, 0.5, 0.8, 0.65, 0.6, 0.85, 0.7,
  0.75, 0.4, 1.0, 0.5.
- **surprise targets:** middle-eight key change (+3 st) or a major-key solo,
  p = 0.15 [synthwave#12]; bass pattern change per section [synthwave#12];
  siren synth; ghost kick on 15 [synthwave#11].
- **references:** Donna Summer "I Feel Love" (1977); Klein + M.B.O. "Dirty
  Talk"; Ryan Paris "Dolce Vita"; Laura Branigan "Self Control"; Alexander
  Robotnick "Problèmes d'Amour" (1983); Gazebo "I Like Chopin"; Baltimora
  "Tarzan Boy"; Kano "I'm Ready"; Trans-X "Living On Video"; Dead or Alive "You
  Spin Me Round" (1985), Bananarama "Venus" (1986) (Hi-NRG). [synthwave#4][synthwave#9][synthwave#10][synthwave#12][synthwave#23][synthwave#41][synthwave#51]

### Ambient and downtempo family

#### P21. Ambient (ambient house and ambient techno as modes)

- **scope:** Eno-style ambient and generative music; drone; ambient house and
  chill-out (1989–94); ambient techno and early IDM as modes.
- **tempo:** ambient: no global pulse; schedule events in seconds, not steps.
  Ambient house/techno: 4/4 at 100–120, typical 110 ("Little Fluffy Clouds"
  ~105, "Xtal" ~115). [ambient#1][ambient#4][ambient#7][ambient#8][ambient#37]
- **swing:** n/a (ambient); ambient techno and ambient house 50–54. [ambient#26][ambient#27][ambient#28]
- **meter/grid:** ambient: 3–7 independent loops with periods 17–70 s whose
  pairwise ratios are not small integers (Eno: one note every 23½ s, the next
  every 25⅞ s, the third every 29 15⁄16 s; *Discreet Music* ≈ 63 s and ≈ 69 s,
  realigning after ~15 min by ear, lcm ≈ 24 min (derived); three of Eno's
  periods line up after ≈ 27 days (derived)). Ambient house: 4/4, 16 steps.
  [ambient#1][ambient#4][ambient#5]
- **drums:** ambient: none with p ≥ 0.7; else a sparse slow "tribal" pattern
  at low level ("Grass"). Ambient house: kick 1, 5, 9, 13, soft hats, drums low
  under the pads. Ambient techno: "subtle drum programming that goes beyond
  simple 4/4 patterns", 909. [ambient#7][ambient#8][ambient#12][ambient#13][ambient#16]
- **bass:** ambient: one held pedal tone (tonic, or tonic + 5th) changing at
  most once every 2–5 minutes (rate design choice; MfA 1/1's Rhodes holds a D
  throughout). Ambient techno: optional slow 303 line or sub pulse with the
  filter barely opening. [ambient#4][ambient#16]
- **harmony:** no chord progression: harmony emerges from overlapping
  single-note loops drawn from one 5–7-note pitch set fixed for the whole
  piece; palette: Ionian with an added 6th (A♭–C–E♭–F), Mixolydian ♭7 colour
  (MfA 1/1 moves D major → D Mixolydian), a sus4 clash (4th against 3rd); allow
  one semitone clash between two loops (D♭ against C) at low density; optional
  microtuning ±10–50 cents per voice (SAW II; amount design choice).
  [ambient#1][ambient#4][ambient#5][ambient#12]
- **melody/hooks:** no melodies; each voice plays one note, or a 2–5-note
  phrase, per loop. *Discreet Music* mode: two phrases with loop lengths in a
  ratio ≈ 63:69, panned hard left and right, through a long echo; "two simple
  and mutually compatible melodic lines of different duration". Each voice is
  silent for at least ⅔ of its loop ("a silence at least twice as long as the
  sound"). [ambient#1][ambient#3][ambient#4][ambient#5][ambient#6]
- **sounds:**

  | Role | Engine | Recipe | Source |
  |---|---|---|---|
  | Pad voice | `juno` `warmPad` | attack 1–4 s, release 3–10 s; sine + triangle; slow low-depth vibrato; cutoff just under half (Synthi patch: attack/decay 6–7 of 10) | [ambient#5] |
  | Sung/bowed single notes | `juno` `choir` (vowel), `bowed` | soft attack, formant filter, one note per voice | [ambient#1][ambient#4] |
  | Piano/Rhodes through echo | `fm` `piano`, `ep` | MfA 1/1: piano through echo, looped, then at half speed (−12 st, envelopes ×2) | [ambient#4] |
  | Long echo | — | `tapeEcho`: 2.8–5.65 s, fb 0.7–0.9, wet 40–50 %, high cut in the loop, + 160 ms at 40 % | [ambient#5] |
  | Reverb | FDN | large hall 6–15 s (design choice) | [ambient#16] |
  | Bells | `fm` | `fmBell`, carrier decay 2–4 s | [sound-design#23][sound-design#24] |
  | Beds | — | `noiseBeds` (rain, birds, water, train) at −25 to −35 dB | [ambient#9][ambient#14] |
  | Ambient house | `juno` arps, strings | repeated arps gradually modulated, reverbed snippets of dialogue (radio voice: formant noise bursts band-limited 300–3000 Hz, design choice) | [ambient#8] |

  Missing: a seconds-based scheduler with incommensurable loops; a long tape
  echo with filtered feedback; half-speed transposition of a rendered loop;
  noise beds; a granular freeze (20–100 ms grains, design choice [ambient#19]);
  time-of-day modulation.
- **arrangement** (voices, not bars; Eno MfA 2/1: seven loops of different
  lengths, one note each [ambient#1][ambient#4]; voice entry times engine
  default):

  | Voice | Period (s) | Content | Enters (min) | Source |
  |---|---|---|---|---|
  | V1 | 23.5 | one sung/bowed note | 0:00 | [ambient#1] |
  | V2 | 25.875 | one note | 0:10 | [ambient#1] |
  | V3 | 29.9375 | one note | 0:30 | [ambient#1] |
  | V4 | ≈ 19.6 | high F | 1:00 | [ambient#4] |
  | V5 | ≈ 20.1 | C | 1:30 | [ambient#4] |
  | V6 | engine default 34 | D♭ (clashes a semitone with C; "significantly" longer) | 2:00 | [ambient#4] |
  | Pedal | held | tonic (+5th) | 0:00 | [ambient#4] |

  Density: 1 note onset per 3–8 s across all voices (derived from 3–7 voices
  with 20–30 s loops); the timbre of a pad changes by at most one small EQ or
  filter step every 1–3 minutes, the only intervention allowed; piece length
  8–30 min (MfA 6:00–16:30; *Discreet Music* ~30 min), endless in radio mode;
  long-form mode (KLF *Chill Out*): one continuous piece of scenes lasting
  0:20–10:00 with segues, percussion density rising across the second half.
  [ambient#3][ambient#6][ambient#9][ambient#21] Ambient house: sections of
  16–32 bars, arps whose filter or modulation moves over 32–64 bars (bar
  counts design choice). [ambient#8]
- **energy:** flat, E ≈ 0.15–0.3 (engine default); radio mode lowers
  brightness and density at night (*Reflection* modulates by time of day).
  [ambient#21]
- **surprise targets:** ≈ 0: Eno's "pleasure in 'gradual processes' prevented
  me from attempting to create surprises"; variation comes only from loop
  phase, which never repeats within a session (realignment ≥ 15 min for 2
  loops, days for 3+). Ambient house: every 1–3 minutes drop in one "found"
  event (radio-voice burst, bird or train texture, an echoed foreign melodic
  fragment). [ambient#1][ambient#3][ambient#5][ambient#8][ambient#9]
- **references:** Brian Eno *Discreet Music* (1975), *Ambient 1: Music for
  Airports* (1978/79), *Reflection* (2017); The Orb "A Huge Ever Growing
  Pulsating Brain…" (1989), "Little Fluffy Clouds" (1990); The KLF *Chill Out*
  (1990); Aphex Twin *Selected Ambient Works 85–92* ("Xtal"), *SAW II* (1994);
  Global Communication *76:14* (1994); Biosphere *Substrata* (1997); Carl Craig
  *Elements 1989–1990*; The Black Dog; B12. [ambient#1]–[ambient#13][ambient#16][ambient#17][ambient#21]

#### P22. Downtempo and trip-hop (Balearic and Boards of Canada as modes)

- **scope:** Bristol trip-hop, downtempo/chill-out, Balearic beat, Air-style
  downtempo, Boards of Canada-style ambient hip-hop, Bonobo.
- **tempo:** trip-hop 67–95, weighted toward 75–90 (*Blue Lines* runs 67–90,
  "Teardrop" ~77, "Sour Times" ~94); downtempo/chill-out 85–110, centre 90
  (chill-out under ~120); Balearic 100–118 (under 119); BoC mode 80–95
  ("Roygbiv" ~84). [ambient#23][ambient#31][ambient#32][ambient#34][ambient#37]
- **swing:** downtempo and trip-hop 54–60 (a 90 BPM groove feels looser at 62
  than at 66); Balearic 50–54 ("laid-back swing patterns" on a 909); never
  above 66 except as a triplet grid. [ambient#26][ambient#27][ambient#28][ambient#34]
- **meter/grid:** 4/4, 16 steps, 1- or 2-bar loops; humanise kicks and snares
  ±5–20 ms, keeping step 1 of each bar exact (jitter design choice).
  [ambient#27][ambient#29][ambient#36]
- **drums** (Attack "raw strip-hop" 80–90 BPM and "dusty hip hop" 90–110
  [ambient#26][ambient#27]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1 (exactly on the grid, full velocity); ghost kicks 9 and 16 | 1.0; ghosts 0.3–0.5 | layered (boom + punch + vinyl crunch); a longer attack is "lazier"; other kicks vary in velocity and timing | [ambient#26][ambient#27] |
  | Snare | 5, 13 | 1.0 | a ghost note in the second-to-last bar; hall reverb | [ambient#26] |
  | Closed H 1 | 1, 3, 5, …, 15 | low | — | [ambient#26] |
  | Closed H 2 | random 16ths | per-hit velocity on almost every hit | — | [ambient#26] |
  | Open H | against the kick; two open hats choked by the next closed hat (dusty) | engine default 0.6 | — | [ambient#26][ambient#27] |
  | Boom | step 1 of each bar (or every 2 bars) | engine default 0.7 | heavily reverbed low "bass hit" | [ambient#26] |
  | Conga/tom | start and end of the bar | engine default 0.5 | balances the bar (dusty) | [ambient#27] |
  | Crackle | continuous | −30 to −40 dB | under every downtempo kit | [ambient#27][ambient#29] |
  | Balearic | 909 kick, snare and hats in laid-back swing; four-on-the-floor | — | — | [ambient#34] |

  Drum bus: compression or tape emulation; Portishead mode: saturation and a
  high cut at 6–10 kHz ("homemade breakbeats"; cutoff design choice). Bonobo
  mode: no quantise; every second bar a slightly mutated copy (velocity ±10 %,
  timing ±10 ms). Kit Witek engine default 22–46. [ambient#24][ambient#27][ambient#36]
- **bass:** a deep, round sub or dub bass on root notes, 1–2 notes per bar,
  leaving silence between; Air mode: Moog-style mono bass. Register engine
  default root MIDI 28–40. Sound `mono` `sub`/`roundBass`.
  [ambient#22][ambient#24][ambient#33]
- **harmony:** minor keys (B minor, G♯ minor, E♭ minor in the database
  examples; chill-out 64 % minor in GiantSteps); progressions of 2–4 chords
  looping over 4–8 bars (loop length design choice); engine default pool:
  i–iv 0.3, i–VI 0.25, i–VII–VI–VII 0.2, i–v–iv 0.15, i alone 0.1; m7/m9
  extensions on 40 % (engine default). [ambient#22][ambient#37][cognition#7]
- **melody/hooks:** a sparse lead (theremin-like sine with vibrato, or
  Rhodes) over the loop; at least 50 % of bars without a lead (ratio design
  choice); often female vocals (no vocal voice in the engine). [ambient#22][ambient#24]
- **sounds:** Rhodes `fm` `ep`; Hammond `fm` `organ`; strings `juno` (live
  strings on "Unfinished Sympathy"); theremin `mono` sine + vibrato; horns,
  flute (`mono` `flute`), Mellotron; Air mode: Solina-style string ensemble,
  Moog bass, vocoder-like formant lead, Rhodes or Wurlitzer; BoC mode: Juno
  pads detuned 5–15 cents; `tapeStage` on drums and keys; reverb 2–4 s.
  [ambient#14][ambient#22][ambient#23][ambient#33] Missing: vinyl crackle bed,
  tape stage (wow/flutter), scratch-like sweep, tape stop, vocal voice.
- **arrangement** (intro 8 bars of drums or a pad alone, verse/loop 16,
  breakdown 8 with drums out, outro 8–16; bar counts design choice
  [ambient#22][ambient#26]):

  | Role | 1–8 intro | 9–24 loop A | 25–40 loop A′ | 41–48 breakdown | 49–64 loop B | 65–80 loop A″ | 81–92 outro |
  |---|---|---|---|---|---|---|---|
  | Drums | x (or pad alone) | x | v | - | x | x | s |
  | Boom, crackle | x | x | x | x (crackle) | x | x | x |
  | B | - | + | x | - | x | x | - |
  | Keys / chords | f | x | x | x | v | x | f |
  | Lead (≤ 50 % of bars) | - | s | x | x | s | x | - |
  | Strings / pad | x | - | x | x | x | x | x |

  total 92 bars (≈ 4:20 at 85; engine default).
- **energy** (engine default): 0.3, 0.5, 0.6, 0.3, 0.6, 0.65, 0.3.
- **surprise targets:** once every 16–32 bars a scratch-like pitch-swept noise,
  a tape stop or a full drum dropout of 1–2 bars (rate design choice); Bonobo
  every-second-bar mutation; found sounds (Burial's rain and vinyl hiss).
  [ambient#22][ambient#24][ambient#36][ambient#40]
- **references:** Massive Attack *Blue Lines* (1991, "Unfinished Sympathy"),
  *Mezzanine* (1998, "Teardrop"); Portishead *Dummy* (1994, "Sour Times"); DJ
  Shadow "In/Flux" (1993); Nightmares on Wax; Kruder & Dorfmeister; Thievery
  Corporation; Air *Moon Safari* (1998); Bonobo *Black Sands*, *Migration*
  ("Kiara"); Boards of Canada *Music Has the Right to Children* ("Roygbiv");
  José Padilla's *Café del Mar* compilations (Balearic).
  [ambient#14][ambient#22][ambient#23][ambient#24][ambient#25][ambient#31][ambient#33][ambient#35][ambient#36][ambient#37]

#### P23. Lo-fi hip hop

- **scope:** "beats to relax/study to" (J Dilla, Madlib, Nujabes lineage; the
  24/7 streams).
- **tempo:** 70–85, typical 78. [ambient#29]
- **swing:** 56–62. [ambient#26][ambient#27][ambient#28]
- **meter/grid:** 4/4, 16 steps; one 4- or 8-bar loop; kick and snare loose
  ±5–20 ms with step 1 exact (jitter design choice). [ambient#29]
- **drums** (loose, low-passed kit, no crash cymbals [ambient#29]; kick lane
  engine default from the strip-hop/dusty grids [ambient#26][ambient#27]):

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | K | 1 (exact), 11 (engine default), ghost 9 or 16 | 1.0; ghost 0.3–0.5 | timing ±5–20 ms except step 1 | [ambient#26][ambient#29] |
  | Snare | 5, 13 | 0.85–1.0 | loose | [ambient#29] |
  | Closed H | 8ths (odd steps) + random 16ths | 0.3–0.6 per hit | — | [ambient#26] |
  | Crackle | continuous | −30 to −40 dB | — | [ambient#29] |

- **bass:** soft bass (sine or round FM) under the chords, root plus approach
  notes, ducked 2–4 dB to the kick. Register engine default MIDI 28–40. Sound
  `mono` `roundBass` or `fm`. [ambient#29]
- **harmony:** extended jazz chords (maj7, m7, 9ths) on Rhodes or piano; 2–4-
  chord loops; ii–V–I acceptable. Engine default pool: ii7–V7–Imaj7 0.25;
  Imaj7–vi7–ii7–V7 0.2; i7–iv7 0.2; IVmaj7–iii7–vi7 0.15; i9–VImaj7 0.2; modes
  major 0.5 / minor 0.5 (engine default). bars_per_chord 1–2; loop_bars 4 or 8.
  [ambient#29]
- **melody/hooks:** a short repeating piano, Rhodes or chorused-guitar figure,
  1–2 bars, looped unchanged; "a cycle out of linearly-conceived melody".
  [ambient#29]
- **sounds:** `fm` `piano`/`ep`; nylon-string or hollow-body guitar "often
  processed through a vibrato or chorus" (`string` `acousticGuitar` +
  chorus); occasionally harp, ukulele, sampled strings; highs filtered out
  (high cut 8–12 kHz), `tapeStage`, crackle; sidechain melodic parts to the
  kick ("almost universal"). [ambient#29][ambient#30] Missing: vinyl crackle,
  tape wow/flutter, chorus/vibrato on the string model, vinyl stop.
- **arrangement** (one 4- or 8-bar loop for 1.5–3 min with drums dropping in
  and out; no build or drop [ambient#29]; bar counts design choice):

  | Role | 1–4 | 5–20 | 21–24 | 25–40 | 41–44 | 45–48 |
  |---|---|---|---|---|---|---|
  | Keys loop | x (filtered) | x | x | x | x | f (fade) |
  | Drums | - | + | - (vinyl stop / filter dip at 24) | x | - | - |
  | B | - | + | - | x | - | - |
  | Guitar / counter-line | - | - | x | x | x | - |

  total 48 bars (≈ 2:28 at 78).
- **energy** (engine default): 0.2, 0.45, 0.25, 0.5, 0.25, 0.15.
- **surprise targets:** a vinyl stop or filter dip at the end of every 8th or
  16th bar (design choice); otherwise exact loop repetition. [ambient#29]
- **references:** J Dilla (unquantised MPC beats); Madlib & MF Doom
  *Madvillainy* (2004); Nujabes (*Samurai Champloo*); ChilledCow / Lofi Girl
  streams. [ambient#29][ambient#30]

#### P24. Dubstep (early UK)

- **scope:** early UK dubstep (2002–08): FWD>>, DMZ, Burial; future garage as a
  mode.
- **tempo:** 132–142, typical 140, felt in half time (~70); Beatport median
  139; future garage 130–140. [ambient#38][ambient#39][ambient#41][cognition#8]
- **swing:** built from a triplet grid (Sound on Sound's patterns); engine
  default: hats and percussion on 8th-note triplets, swing8 ≈ 0.67.
  [ambient#39]
- **meter/grid:** 4/4 at 140 felt as half time; triplet grid required.
  [ambient#38][ambient#39]
- **drums:**

  | Lane | Steps | Vel | Variations / fills | Source |
  |---|---|---|---|---|
  | Snare/clap | 9 ("on every third beat of a bar") | 1.0 | — | [ambient#38][arrangement#23] |
  | K | 1 and 13 (beats 1 and 4) + one two-thirds of the way between beats 2 and 3 (triplet position ≈ step 7.67) | 1.0 | half-step kicks are "lazy" off-beat hits | [ambient#39] |
  | H | engine default: triplet 8ths, velocity 0.4–0.7 | — | future garage: off-grid, late, humanised hats between kick and snare | [ambient#41] |
  | Roll | build: G4 snare roll | — | last bar or beat before a drop silent | [arrangement#23] |

- **bass:** a sine sub at about 30–60 Hz (written C0–C1 in DAW octave
  numbering; C1–C2 in scientific pitch); wobble from a usually tempo-synced
  LFO on filter cutoff, volume or distortion: `wobble` recipe, 1/8T = 7.0 Hz,
  1/8 = 4.67 Hz, 1/4 = 2.33 Hz at 140, switching the division every 1–2 bars
  for "talking" wobbles (switching rate engine default); future garage: warm
  filtered Reese. [ambient#38][ambient#39][ambient#41][sound-design#43]
- **harmony:** minor keys, Phrygian, tritones; minor 0.91 (GiantSteps
  dubstep). Engine default: one-chord or two-chord (i–♭II) vamps, loop_bars 8.
  [ambient#38][cognition#7]
- **melody/hooks:** minimal; pitch-shifted R&B vocal fragments cut word by
  word (Burial); pitched vocal chops (future garage). [ambient#40][ambient#41]
- **sounds:** sub `mono` `sub`; wobble (missing LFO-to-cutoff routing; `mono`
  saw + square); atmos pads `darkPad`; rain and vinyl hiss beds (Burial);
  loudness −6 to −7 LUFS; pump ≤ 0.2. [ambient#40][sound-design#35] Missing:
  tempo-synced LFO to cutoff/volume/drive, triplet grid, vocal chop voice.
- **arrangement** (140 BPM half time, 144 bars ≈ 4:07 [arrangement#23]; drops
  usually after bar 16 or 32 [ambient#38]):

  | Role | 1–16 intro | 17–32 build | 33–64 drop 1 | 65–80 mid | 81–96 build 2 | 97–128 drop 2 | 129–144 outro |
  |---|---|---|---|---|---|---|---|
  | Atm / Pd | x | x | - | x | x | - | x |
  | P (half-time, snare 9) | x | x | - | - (paused) | x | - | x |
  | K | - | + | x | - | x | x | - |
  | C (step 9) | - | - | x | - | - | x | - |
  | Sub + wobble | - | - | x | - | - | v (changed pattern) | - |
  | V | - | - | - | x | - | s | - |
  | Sn, R | - | r (last bar silent) | 1 (X) | - | r (last beat or bar silent) | 1 (X) | - |

- **energy** (engine default): 0.3, 0.55→0.85, 1.0, 0.35, 0.6→0.9, 1.0, 0.3.
- **surprise targets:** drop 2 changes the bass pattern or drum loop
  [arrangement#23]; wobble division switches every 1–2 bars (engine default)
  [sound-design#43]; percussion pause, sometimes to silence, before the sub
  returns [arrangement#23].
- **references:** Skream "Midnight Request Line" (~2005); Benga & Coki
  "Night" (2007); Burial *Burial* (2006), *Untrue* (2007, "Archangel").
  [ambient#38][ambient#40]

#### Survey-only genres (not profiled)

These appear in ambient.md's survey with too little detail for a profile; the
numbers below are what a future profile starts from.

| Genre | Tempo | Rules the research gives | Source |
|---|---|---|---|
| Footwork/juke | 150–165, often 160, alternating with half-time | sparse syncopated 808 kicks that "skip beats"; crackling snares/claps; pitched vocal chops; heavy sub; triplets by ear (unverified) | [ambient#42] |
| Hardstyle | ~140 early, 150–160 now | overdriven, pitched kick that doubles as the bassline; off-beat "reverse bass"; screeches | [ambient#43] |
| Big room / electro house | 126–132 | minimal drop: kick or tom and a couple of hats; long builds; short simple supersaw lead in a large hall; two builds and drops, one build much longer | [ambient#44][arrangement#25] |
| Future bass | 130–175 (usually half time, unverified) | LFO- or automation-modulated chords, vocal chops, vocoders, "twinkly" noise risers, trap/dubstep drums | [ambient#45] |
| Vaporwave | from the slowed source | a smooth-jazz/muzak loop pitched and slowed 10–30 % (design choice), chopped, repeated, heavy reverb | [ambient#46] |
| Afro house / amapiano | afro house ~80–122; amapiano ~110–115 (unverified) | log drum = a short tuned FM or pitch-swept sine "bass hit" (design choice); shakers, piano melodies | [ambient#47][ambient#48][ambient#49] |
| Pop-EDM (radio edit) | 126–128 | verse–prechorus–riserchorus–drop rotations; 120 bars ≈ 3:45 (template in arrangement.md) | [arrangement#6][arrangement#28] |

---

## What the engine lacks

Collected from every file's rules and from sound-design.md §6, in priority
order: first what most profiles need to sound like their genre at all, then
accuracy, then colour. "Engine" means the current `crates/music` (classic
generator, `seq.rs`, `engine.rs`, instruments and kits). The per-instrument
clip timeline itself (PLAN.md Phase 3's score model) is assumed.

### Priority 1: sequencer and score features (needed by nearly every profile)

| # | Feature | Why | Profiles | Source |
|---|---|---|---|---|
| 1 | **Per-hit micro-timing (ms offsets) and continuous velocity** (0–1 per hit, ramps). The classic sequencer has a few velocity levels and a ±6 % humanise | ghost notes ±5–15 ms; hats nudged 1/64 bar; ghost kicks 2–8 ms off; lo-fi/Bonobo jitter ±5–20 ms with step 1 exact; 8th-grid late / off-16th early humanising; roll velocity ramps 1 → 127; ±10 % hat velocities | all; essential for P4, P6, P7, P11–P13, P22, P23 | [dnb#10][techno#14][techno#17][ambient#29][ambient#36][cognition#23][arrangement#11] |
| 2 | **Automation lanes per layer and per bus** with curve shapes (linear, exponential, parabolic) over 1–64 bars, and parameter targets beyond cutoff (resonance, decay, send level, delay feedback, detune, pitch) | builds (HPF on the music bus, pitch rise, filter "fully open at the drop"), breakdown LPF opening, 303 cutoff/decay/res macros, French-house bus sweep 200 Hz → 12 kHz, techno 16–32-bar sweeps and 50 Hz-up pad sweeps every 4 bars, supersaw detune sweep, dub echo throws, effects-send gating | all four-on-the-floor profiles; P3, P8, P9 most | [arrangement#9][arrangement#11][arrangement#12][house#3][techno#16][techno#19][techno#27][sound-design#13] |
| 3 | **Finer grids:** a 32nd-note grid, a triplet grid (beyond `swing8`), **odd-length per-layer cycles** (3, 5, 12, 13, 15 steps) and **seconds-based loops** for ambient | roll final bars in 32nds; "Spastik" 32nd rolls; synthwave 32nd hat; dubstep triplet grid and 2/3-beat kick; tribal psy triplet bass; acid 13/15-step 303; Hawtin's 5/12 cycles; Eno's 17–70 s incommensurable loops | P1, P7, P8, P14, P15, P18, P21, P24 | [arrangement#11][techno#9][techno#17][techno#27][synthwave#3][ambient#1][ambient#39][trance#26] |
| 4 | **Bus routing** (sub-mixes with their own insert chain: filter, phaser, saturation, compressor, sidechain) | French-house "loop bus"; DnB drum bus with saturation + parallel compression; break bus with bracket EQ; techno drum bus drive/boom; build HPF on the music bus only | P3, P6, P11–P13, all builds | [house#3][house#28][dnb#9][dnb#10][dnb#11][techno#12][arrangement#9] |
| 5 | **Break-cell player** with chop operations (reorder 2-beat cells, repeat/stutter 1/8–1/32 slices, pitch ±1–12 st, reverse envelope, drop) | jungle and DnB breaks without samples; Think "tops"; ragga squeals; time-stretch stutters | P11, P12, P13, breaks/big beat | [dnb#4][dnb#11][dnb#15][dnb#23][dnb#46][dnb#50] |
| 6 | **Per-layer sidechain depth and alternative keys** (kick, silent click, a MIDI pattern on every beat), release derived from tempo; the classic engine has one global pump | house 3–4.5 dB vs French 6–12 dB vs DnB ≤ 2 dB on different layers; Tate's MIDI-keyed pump; lo-fi melodic ducking | P1–P3, P14, P23, all | [sound-design#29][sound-design#30][trance#3][ambient#29][house#28] |
| 7 | **Mid-song transposition and key change**; whole-mix detune; slow tempo drift | Eurobeat final-chorus lift (+1/+2 st); Italo/synthwave middle eight +3 st and major-key solo; *Drive* "flat tape" −35 cents; electro ±0.5 BPM | P17, P18, P19, P20 | [trance#40][trance#42][synthwave#12][synthwave#39] |
| 8 | **Lane options:** choke groups toggled per section, per-lane pan and EQ, duplicated lanes with detune and opposite pan, trance-gate step patterns on a pitched layer, chord-memory (one shape moved in parallel) | Detroit tight/loose hats, Chicago hats choked; 707 tambourine copies L/R; rim panned left; ride HPF 1 kHz; trance gate; rave/Detroit stabs | P1, P5, P6, P9, P14, P20 | [techno#11][house#5][synthwave#12][techno#12][trance#44][sound-design#27][sound-design#28] |

### Priority 2: mix and master

| # | Feature | Why | Profiles | Source |
|---|---|---|---|---|
| 9 | **Integrated-LUFS meter, per-genre loudness target, true-peak limiter** (the current limiter is a sample-peak ceiling at 0.95 ≈ −0.45 dBFS) | genre loudness spans −6 (dubstep) to −10.5 LUFS (deep/melodic); ≤ −1 dBTP | all | [sound-design#34][sound-design#35] |
| 10 | **Gated reverb** (gate keyed by the dry hit, hold 0.3–0.5 s, release < 20–50 ms) | the 80s snare and tom; synthwave kick | P18, P19, P20 (and 80s revival anywhere) | [sound-design#33][synthwave#25][synthwave#26] |
| 11 | **Tape stage** (wow, flutter, saturation, high cut) and a **long tape echo** with filtered feedback (2.8–5.65 s, fb 0.7–0.9; RE-201-style 264 ms at 85 %) | lo-fi, BoC, downtempo, dub techno echoes, Eno's *Discreet Music* | P9, P21, P22, P23 | [ambient#5][ambient#14][ambient#29][techno#16] |
| 12 | **Phaser, flanger, band-pass mode** in the poly chain; **stereo delay with a 1/16-offset copy** panned opposite | French house filter + phaser; dub chord BP ≈ 450 Hz; jungle filter-and-flange fills; Moroder's bass | P3, P9, P11, P20 | [house#3][techno#15][arrangement#20][synthwave#4][synthwave#8] |
| 13 | Glue attack up to 30 ms; 4:1 ratio preset | punchier techno and big-room buses | P6, survey big room | [sound-design#31][sound-design#32] |

### Priority 3: DSP and instrument accuracy

| # | Feature | Why | Profiles | Source |
|---|---|---|---|---|
| 14 | **Sample-character stage:** requantise to 6–12 bits at 22–35 kHz with no anti-alias, decay applied after the "DAC" | 909 hats/ride/crash are 6-bit samples (the engine's square-cluster hat sounds like an 808); TR-707/727, LM-1/LinnDrum; MPC60 12-bit (Detroit); SP-1200 6-bit/22 kHz (industrial); S950 grit (jungle); lo-fi 8-bit drop | P1, P5, P6, P8, P11, P18–P20, P23 | [sound-design#5][sound-design#6][sound-design#37]–[sound-design#40][techno#11][techno#13][dnb#48] |
| 15 | **Tempo-synced LFO routable to cutoff, volume or drive**, with LFO delay and division switching; **sample & hold**; delayed vibrato/filter-LFO fade-in (the poly LFO reaches only PWM and vowel) | dubstep wobble; jump-up and neuro movement; future bass chords; psy "fractal" S&H; Nightcall's 5.29 Hz delayed LFO | P13, P15, P18, P24 | [sound-design#43][dnb#47][trance#23][synthwave#38] |
| 16 | **Supersaw fixes:** mix law (centre linear, sides parabolic), HP tracking the fundamental, measured asymmetric spread, random phase per note | the trance lead and pads | P14, P16, survey big room | [sound-design#13][trance#5][trance#8] |
| 17 | **TB-303 fixes:** 4-pole diode ladder with a 24 dB asymptote and soft knee (the engine header says 18 dB); ~10 Hz HP in the resonance loop; accent capacitor τ ≈ 150 ms charge / 100 ms discharge (the engine's ≈ 24 ms may be too fast and peaky); 4 ms gate delay + 3 ms attack; base cutoff topping out near 2.5 kHz | acid's "wow" and squelch | P8, P15 (Goa), P19, P20 | [sound-design#9][sound-design#10][sound-design#11][sound-design#41] |
| 18 | **Juno fixes:** chorus I ±0.5 ms and II ±1.1 ms (the engine sweeps ±1.7 ms, ~3× too deep for I); offer Juno-60 I+II (1 Hz, 8 %) beside the Juno-6 (8 Hz, 25 %); the 4-position HPF (+10 dB < 150 Hz / flat / 240 Hz / 720 Hz); 15 kHz pre/post LPF | deep-house bass and pads, synthwave chorus "on everything" | P2, P5, P18, P22 | [sound-design#15][sound-design#44] |
| 19 | **FM additions:** a fixed-frequency operator; check the EP preset uses the 14:1 tine pair; a bass algorithm (carrier 0.5, modulators 5.0 and 9.0, fast decays) | bells (3.5:1, 3.18:1), DX bass, the octave FM bass of eurobeat/Hi-NRG | P2, P12, P17, P18, P21, P23 | [sound-design#23][sound-design#24][trance#36] |
| 20 | **Drum accuracy:** 808 kick's abrupt ×2 jump for 6 ms then a separate slow sag; 808 cymbal in three bands; 909 kick from a waveshaped saw with LP-noise + pulse click; 909 snare noise split into two paths | the two kits most profiles use | all 808/909 profiles | [sound-design#1]–[sound-design#4] |

### Priority 4: missing voices and FX

| # | Voice / FX | Why | Profiles | Source |
|---|---|---|---|---|
| 21 | **Vocal-like voices:** a vocal-chop grain voice (formant grains, re-pitch, reverse); a vocoder/speech voice; a sax-like and a guitar-like distorted lead with pitch bend | UKG/garage chops; jungle MC; Burial and future-garage chops; electro and Italo vocoder; synthwave solos; trip-hop and trance vocals have no stand-in at all | P4, P11, P17–P20, P22, P24 | [house#4][house#34][dnb#23][synthwave#13][synthwave#34][synthwave#48][ambient#40] |
| 22 | **Hoover** (PWM-saw ×3 octaves, fast PWM LFO, fast chorus, downward bend) | hardcore, Belgian techno, rave stabs | P11 (hardcore mode) | [sound-design#26][dnb#34] |
| 23 | **Osc sync / cross-mod; osc 3 as LFO/FM; low-note priority** in `mono` | sync-sweep leads (Jupiter-8, Prophet-5), Minimoog behaviour | P18, P19, P20 | [sound-design#17][sound-design#18][sound-design#19] |
| 24 | **M1 "Organ 2" bass and "Piano 16'" stand-ins** (additive organ; FM/modal piano) | garage house, piano/Italo house, UKG | P1, P4 | [sound-design#20][sound-design#21][house#21][house#22] |
| 25 | **Wavefolder and resampling** on a mono bass, with a clean sub beneath | neurofunk mid-bass | P13 | [dnb#20][dnb#44] |
| 26 | **Transition FX:** full-mix reverse and reversed-reverb riser, tape stop / spin-back (rewind), faller, splash | jungle rewinds, Italo breaks, Tiga's reverse, Âme's splash | P11, P19, P20, P22, P23 | [dnb#38][synthwave#12][arrangement#17][arrangement#18] |
| 27 | **Texture FX:** vinyl crackle, tape hiss, rain, birds, train, radio-voice bursts, dub siren, gunshot, air-horn, sonar ping, orchestra hit, explosion | lo-fi, trip-hop, dub techno, ambient house, jungle, electro | P9, P11, P19, P21–P23 | [ambient#9][ambient#14][ambient#29][techno#15][dnb#23][synthwave#14][synthwave#21] |
| 28 | **Kit lanes:** tambourine, cabasa, woodblock, claves in the 808 kit, bongos as a 727 kit | Chicago tambourine, UKG cabasa, industrial woodblock, Detroit 727 bongos | P1, P4, P5, P6, P19 | [house#5][house#9][techno#13][techno#18] |
| 29 | **Granular freeze** (20–100 ms grains over a held pad) and half-speed rendering of a loop (−12 st, envelopes ×2) | IDM/ambient textures, *Music for Airports* 1/1 | P21 | [ambient#4][ambient#19] |

---

## Contradictions between files

| # | Topic | One file | The other | Resolution used here |
|---|---|---|---|---|
| C1 | Supersaw spread at max detune | sound-design (Szabo): 0.88998, 0.93712, 0.98048, 1, 1.01991, 1.06217, 1.10745 = −202, −112, −34, 0, +34, +104, +177 cents [sound-design#13] | trance (Shore, from Szabo): 0.8900, 0.9371, 0.9698, 1.0, 1.0295, 1.0622, 1.1075 = −202, −113, −53, 0, +50, +104, +177 cents; a forum measure .893/.939/.980/1/1.020/1.064/1.110 [trance#5][trance#8] | the inner pair is in dispute (±34 vs −53/+50); the outer and middle pairs agree. Engine default: Szabo's table (the primary measurement; the forum figure agrees with it) |
| C2 | Juno chorus LFO rates | 0.513 Hz (I), 0.863 Hz (II) from a circuit-informed emulation [sound-design#44] | 0.4 Hz (I), 0.6 Hz (II) from Wikipedia [synthwave#29] | use 0.513 / 0.863 (the circuit model) |
| C3 | Sidechain depth | house: 2–6 dB Chicago/deep, 6–12 dB French [house]; trance: pads 4–8 dB, hard trance ≥ 6 dB [trance] | sound-design: house −3 to −4.5, French −6 to −9, trance −4 to −6 dB [sound-design] | all are engine defaults; ranges overlap; the narrower sound-design ranges as defaults, the wider ones as the RNG's bounds |
| C4 | Dub techno tempo | 110–125 typical (Wikipedia) [techno#31] | Attack's Basic Channel-style recreation at 145, no swing [techno#15] | main band 112–125, a 140–145 variant at low weight (techno.md's own rule) |
| C5 | Techno mode | "99.9 % of techno is minor" (Attack) [techno#19] | GiantSteps: techno 82 % minor (28/34) [cognition#7] | minor 0.95 (between the two); revisit with listening |
| C6 | Build roll staging and early stop | house template 4/2/1/1 bars + 2 silent beats; trance template 8/4/3/1 over 16 bars [arrangement]; trance.md 2/2/2/1 + 1 silent bar [trance#28]; dnb.md 4/2/2 [dnb#14]; stop early in 30 % of builds [arrangement] vs ≈ 20 % [trance] | — | per-profile staging as listed; early stop engine default 25 % |
| C7 | DnB form | arrangement.md: 160 bars, intro 16, build 16, first drop at bar 33 (32 bars) [arrangement#8][arrangement#20][arrangement#24] | dnb.md: 256 bars, intro 32, build 16, first drop at bar 49 (64 bars) [dnb#36][dnb#37] | dnb.md's 256-bar form for P12 (extended) and P13; arrangement.md's shape is within dnb.md's ranges (intro 16–32, drops 32–64) and serves shorter radio versions |
| C8 | Uplifting trance layout | arrangement.md: 224 bars; main 1 at 65–96, 32-bar breakdown, 16-bar build, 32-bar climax [arrangement#22] | trance.md: intro 32 bars drums only with bass at 17 → build 16 → main 32 → breakdown 32–64 → build 8–16 → drop 32 → outro 32 (engine default) [trance] | arrangement.md's bar-numbered table; trance.md's ranges as the RNG's bounds |
| C9 | Hook repetition | exact refrains produce more tapping and singing; "vary the surroundings, not the hook" [cognition#24] | when a returning hook's mean IC falls below z −1.5, vary 1 event in 4–8 (proposal) [cognition#14][cognition#17] | vary the surroundings first; touch the hook only if still below band, never in its first two bars (engine default, G9.8) |
| C10 | TB-303 filter slope | 24 dB 4-pole diode ladder, gentle near cutoff; "18 dB" is a folk reading [sound-design#11]; Wikipedia 24 dB non-self-oscillating [techno#6][sound-design#12] | the engine's `tb303` is "an 18 dB diode-leaning ladder" (`src/instruments.rs` header) | research wins; listed as engine gap #17 |
| C11 | House tempo | Chicago 118–126, deep 115–125 (typical 122) [house]; Beatport deep house 122 [cognition#8] | arrangement.md's house template is written for 124–128 [arrangement] | per-profile tempos from house.md; the template's bar layout is tempo-independent |
| C12 | Bass in the DJ intro | arrangement: bass held out of bars 1–8; intro = drums + at most 1 non-melodic layer [arrangement] | house.md: "drums (plus bass after 8–16 bars)" [house#13][house#44]; Âme: kick + bass from bar 1 [arrangement#17] | compatible: bass is the one allowed non-melodic layer, from bar 9 (bar 1 in deep house) |
| C13 | Deep-house mode | research: minor 95 % (no Dorian source) [cognition#7][house#15] | the classic generator writes deep house in Dorian (`src/genres.rs`) | research wins (natural minor); Dorian is not sourced |
| C14 | Dubstep sub register | "C0–C1 (about 30–60 Hz)" [ambient#39] | 30–60 Hz is C1–C2 in scientific pitch | a DAW octave-numbering difference, not a real conflict; use Hz |
| C15 | Psytrance tempo | 138–144 (psytrance), full-on 140–148 [trance#19][trance#26] | Beatport median 147 [cognition#8] | trance.md's bands (the Beatport sample is 5 tracks) |

---

## Open questions and gaps

**Corpora and cognition.**
- No published corpus of chord transitions or drum-pattern statistics for
  house, techno or any Beatport genre; the LTM must be trained on the
  profiles' own pools (G9.4). J. Park's 2025 dissertation on EDM-pop chord
  loops was found but not read; Hooktheory's full matrix sits behind an
  authenticated API. [cognition]
- No study measures per-genre IC or entropy targets; every band in G9.11 is a
  proposal or engine default. Cheung's chords and Gold's melodies were rated
  in isolation, without production or dance context; Witek's breaks are funk,
  not EDM. [cognition]
- How to move the rock transition table into a minor-tonic EDM frame is not
  in the research (G9.4's engine default uses it only as a root-motion prior).
- The relative weights of the score terms (placement, syncopation,
  repetition, IC band) need listening tests.

**Arrangement.**
- Butler's *Unlocking the Groove* and Snoman's *Dance Music Manual* were read
  only through secondary sources; no Spotify or Beatport corpus of section
  lengths exists; trance and dubstep bar counts come from tutorials and
  Wikipedia. [arrangement]
- The mapping from energy E to concrete layer counts, filter positions and
  pump depth is an engine default (G5) and needs tuning by ear.
- A 64-bar DnB intro is not confirmed by any source. [dnb]

**Genres.**
- House: French-house filter-sweep lengths (8–16 bars) unverified; no
  transcribed garage-house gospel progressions; tech-house and
  progressive-house tempos unverified; Italo-house tempo and swing unverified;
  no French-house progression. [house]
- Techno: Attack's grids are only in screenshots, so many positions are
  derived from descriptions; Phrygian ♭2 as a techno habit unverified; the
  60–70 % one-chord share, acid accent/slide percentages, 303 hardware glide
  time and ghost-kick offsets unverified; no verified melodic-techno landmark
  tracks; the dub techno tempo conflict (C4). [techno]
- DnB: Amen bars 3–4 placements are best-effort; jungle progressions
  unverified; the "Apache" use and E-Z Rollers details unverified. [dnb]
- Trance: classic trance-gate patterns, call-and-response between lead
  phrases, and a "Phrygian dominant" Goa default are unverified; early German
  landmark tempos unverified; eurobeat's key-change habit rests on one blurb
  and a J-pop convention; Hooktheory's "Deja Vu" is internally inconsistent.
  [trance]
- Synthwave: "The Midnight = major" unverified; the i–VI–III–VII order is not
  attested; brass patch settings and solo bend depth unverified; FM-84 and
  Lazerhawk details unverified. [synthwave]
- Ambient survey: amapiano tempo, footwork triplets, future bass half-time and
  vaporwave tempo unverified. [ambient]

**Sound.**
- 808 clap circuit, the size of the 808 kick's pitch sigh, the LinnDrum sample
  rate, the hoover's bend, the dotted-8th trance delay and the Juno HPF
  positions (against the service manual) are unverified; per-instrument HPF
  corners are engine defaults. [sound-design]
- Vocals carry garage, trance, Italo, eurobeat, trip-hop and jungle, and the
  engine has no vocal synthesis; the stand-ins (formant grains, vocoder) are
  design questions, not research results.
- Several genres are sample-based (French house's disco loops, jungle breaks,
  trip-hop, vaporwave); the synthesised stand-ins (loop bus, break kit) are
  design choices nobody has listened to yet.
- Reference audio: the owner has not supplied reference music (PLAN.md,
  answer 3); every profile's "references" list is the research's landmark
  tracks, to be listened against when rating the Phase 3 gate.
