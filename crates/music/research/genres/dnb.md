# Jungle, drum and bass, and breakbeat

Research notes for the procedural music engine. Scope: breakbeat hardcore, jungle and ragga jungle, intelligent/atmospheric jungle, drum and bass and its main sub-styles (jump-up, techstep, neurofunk, liquid, rollers, halftime), plus big beat and nu skool breaks as slower relatives. Each claim carries a numbered reference. Statements marked **(heuristic)** are engine design choices drawn from the sourced material but not stated by any source. Statements marked **(unverified)** could not be confirmed.

Grid convention used throughout: one 4/4 bar = 16 sixteenth-note steps numbered **1–16**. Quarter-note beats fall on steps 1, 5, 9, 13; the off-beat 8ths ("ands") are 3, 7, 11, 15; the remaining steps are the "e" and "a" 16ths. At 174 BPM one bar lasts 240/174 = 1.379 s, so 8 bars ≈ 11.0 s, 16 ≈ 22.1 s, 32 ≈ 44.1 s, 64 ≈ 88 s [36].

---

## 1. Lineage and timeline

| Period | Style | Tempo | What changed | Refs |
|---|---|---|---|---|
| 1989–92 | Breakbeat hardcore | ~130 BPM (1990–91) rising | 4/4 house rhythms fused with sped-up hip-hop breaks; hoover, piano stabs, sped-up vocals | [1][3] |
| late 1992–93 | Darkcore / "hardcore jungle" | 155–165 BPM by 1993 | Hardcore splits into darkcore, hardcore jungle (reggae bass prominent), and happy hardcore | [1][3] |
| 1993–95 | Jungle, ragga jungle | ~160 BPM breaks, ~80 BPM basslines | Chopped, time-stretched breaks; half-time reggae basslines; dancehall vocals; peak 1994–95 | [2][24] |
| 1993–96 | Intelligent / atmospheric | ~160–170 | Ambient pads, jazz, Detroit-techno influence (LTJ Bukem, Good Looking) | [2][22][26] |
| 1995–98 | Drum and bass, hardstep, jump-up, techstep | 170–180 BPM from ~1996 | Producers drop ragga; drum-machine-tight two-step; distorted Reese basses | [1][19] |
| 1997–2000s | Neurofunk | 170–176 | Cleaner, more regular two-step; heavily modulated mid-range bass | [20] |
| 1999–2000s | Liquid funk | 170–176 (174 typical) | Melodic, soul/jazz harmony, vocals, Rhodes | [14][21][22] |
| 2000s– | Halftime | DnB tempo, snare on beat 3 | Hip-hop lope at full-tempo bass | [22][36] |
| 1995–2001 | Big beat (relative) | 100–140 BPM | Distorted breaks, 303 lines, funk/rock loops | [42] |
| 1996–2002 | Nu skool breaks (relative) | 125–140 BPM | Technical, bass-led breaks; less hip-hop sampling than big beat | [43] |

Today "most drum and bass falls between 170 and 176 BPM, and 174 is the most common modern tempo"; classic jungle ran around 160–170 [36]. Wikipedia gives 160–180 (body) and 165–185 (lead) for DnB as a whole and notes some recent producers returning to 150–170 [1].

---

## 2. The source breaks

### 2.1 Amen break
- From "Amen, Brother", B-side of The Winstons' 1969 single "Color Him Father"; drummer Gregory C. Coleman; a ~7-second, four-bar solo starting ~1:26 [4]. Its original tempo is given as ~136 BPM (external link on [4]) or "about 140 BPM" [6] — sources disagree.
- Structure: bars 1–2 are a conventional funk pattern; in bar 3 Coleman delays a snare; in bar 4 he leaves beat 1 empty, plays a syncopated figure and hits an early crash [4]. The AudioLabs Erlangen analysis says the same: "in the second half of the third bar, the expected repetition is broken, and syncopation shifts accented snare hits to the offbeat" [6].
- Ethan Hein's layer-by-layer reconstruction gives the specific moves: double the kick on the "and" of 3 in bars 1–2; snare pickups on the 16th before beat 3 and the 16th before the "and" of 3; in the last two bars "delay the snares on beat four by an eighth note", which he calls the most crucial element; and he stresses that Coleman's microtiming cannot be captured in MIDI or notation [5].
- Generally considered the most-used break in DnB [1]; at least 5,378 samplings counted by WhoSampled as of 2021 [8]. Producers altered pitch and speed and reordered its components [4].

**Approximate Amen on the 16-step grid** (reconstructed from [4][5][6]; ride on all 8ths 1,3,5,…,15 unless noted; capitals = accented, lowercase = ghost):

```
step     1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
bar 1-2  K . K . S . . s . s  K  K  S  .  .  s
bar 3    K . K . S . . s . s  K  .  .  .  S  .
bar 4    . s K . S . . s . s  K  .  .  .  S  .   (crash/open cymbal on 11)
```

Bar 3 and 4 placements other than the delayed snare (step 15) and the empty downbeat in bar 4 are a best-effort reconstruction **(unverified in detail)**.

### 2.2 Think break
- From Lyn Collins, "Think (About It)" (1972), written and produced by James Brown; drums by John "Jabo" Starks [7][8]. Second-most sampled record (≥3,017 samplings) [8].
- It has several usable sections: the "yeah! woo!" break at 1:21, a one-bar break at 1:34, a clean one-bar drum section at 2:15 and a tambourine loop at 2:21 [8]. A non-shout part of the break is the one prominent in jungle, DnB and breakcore, usually sped up [7].
- In jungle it was sped up, chopped, resampled; its tambourine was often layered under other breaks [8]. Uses include Origin Unknown "Valley of the Shadows" (1993), Omni Trio "Renegade Snares" (Foul Play remix, 1993), Goldie "Ghosts of My Life" (1993), Leviticus "Burial", DJ Hype "Roll the Beats", Remarc "Sound Murderer" [8]. Modern liquid producers still lay a high-passed "Think tops" loop under programmed drums [14].

### 2.3 Apache and others
- The Incredible Bongo Band's "Apache" (1973) drum-and-bongo break, used in Goldie's "Inner City Life" (1994) (search excerpt of the Attack/Mixdown features; **(unverified)** beyond that). Wikipedia lists Amen, Apache and Funky Drummer as DnB sample sources [1]; Polyend's history names Amen, Think, Apache and Hot Pants [49].
- "Many tracks switch between two breaks each bar" [1]. The "Tramen" combines Amen, a James Brown break and an Alex Reece break [1].

### 2.4 Time-stretching and samplers
- Time-stretching let producers raise a break's tempo without raising its pitch, which made hip-hop breaks usable at rave tempos and gave jungle its "machine-processed" feel [23]. Simon Reynolds credits Goldie with pioneering time-stretching on "Terminator" (Metalheads, 1992) [25].
- Typical tools: Akai S950 (gritty, limited memory), ReCycle-style slicing; breaks were "chopped, stretched, pitched, and rearranged" [48]. Early producers also used Amiga trackers [49].
- Time-stretch artefacts were used as effects: DJ Hype "Roll the Beats" (1994) stretches MC GQ's rolled r's into a "digital stutter"; The Dreamteam "Yeah Man" (1995) stretches voices until metallic [23]. In ragga jungle, very tight beat repetitions turn into "high pitched squeals" (practitioner forum, [50]).

---

## 3. Styles in detail

### 3.1 Breakbeat hardcore (c. 1989–1993)
- Blended four-on-the-floor house rhythms with breakbeats sampled from hip-hop; drew on new beat, Belgian techno, acid house, hip-hop and reggae; enabled by cheap Atari ST/Cubase studios [3].
- Sounds: looped breaks, shuffled drum-machine patterns, hoover and Belgian-techno noises, acid and bleep sounds, house piano riffs and vocals, intensified sub-bass, riff "stabs", bursts of noise [3].
- Artists: Shut Up and Dance, Rebel MC, Fabio, Grooverider, Slipmatt, Liam Howlett (The Prodigy), 4hero [3][28]. 4hero's "Mr Kirk's Nightmare" (1990, Reinforced) is built on the Isley Brothers' "Get Into Something" break [28]. Lennie De Ice "We Are I.E." (1991) is the usual proto-jungle citation [1][2]. Urban Shakedown "Some Justice" (1991) chopped Run DMC breaks and added heavy sub bass [49].
- The **hoover**: Roland Alpha Juno "What the" preset by Eric Persing; PWM sawtooth, three oscillators an octave apart, thick chorus, a fast LFO on PWM and chorus [34]; first commercial use in Second Phase "Mentasm" (1991, Joey Beltram & Mundo Muzique), sampled through a Casio FZ-1 [34][35].
- Darkcore turn (1992–93): Metalheads "Terminator" (1992), Rufige Kru "Ghosts of My Life" (1993), Origin Unknown "Valley of the Shadows" (1993), Doc Scott "Here Come the Drums" (1993), Omni Trio "Renegade Snares" (1993) — Rob Haigh built that track's breaks from individual hits [23].

### 3.2 Jungle and ragga jungle (c. 1992–1996)
- From breakbeat hardcore plus Jamaican sound-system culture: rapid breakbeats, syncopated loops, deep basslines from dub, reggae, dancehall, hip-hop and funk [2]. Peaked 1994–95; pirate radio, then Kiss 100 (1994) and Radio 1 [2].
- Martin James: jungle paired "160 bpm breaks and 80 bpm basslines", so ragga vocals — already at the right tempo — fit naturally; the sound was "militant breaks" with "half-time reggae basslines" and dub sound-system effects [24]. Ragga licks were "synonymous with hardcore" by 1993, often lifted from cheap sample CDs [24].
- Landmarks: Shy FX & UK Apache "Original Nuttah" (white label end 1993; 1994, UK #39) [24][27]; M-Beat feat. General Levy "Incredible" (1994, Renk) [24]; Remarc "Drum & Bass Wize" (1994; heavily processed Amen, dancehall crowd chat) [23]; Marvellous Cain "Hitman" (1994; Cutty Ranks sample) [23]; Dead Dread "Dread Bass" (1994; wah-wah "dread bass", gunshot FX, stop-start edits) [23]; Leviticus "Burial" [2][24]; Renegade "Terrorist" (1994, Ray Keith) sampled Kevin Saunderson's Reese bass over the Amen [1]; Goldie "Inner City Life" (1994) and *Timeless* (1995, UK #7) [25]; Metalheadz founded 1994 [25].
- Identity elements: ragga vocals, dub sirens, gunshots, barks, chopped hooks (genre guides; [2][24] confirm vocals and FX broadly).
- The **rewind/reload** came from Jamaican selectors (crowd-demanded pull-ups), moved to London pirate radio with the MC, and became central to jungle; the MC chant "I say RELOAD, you say now!" [38][39].

### 3.3 Intelligent / atmospheric jungle (c. 1993–1997)
- "Dreamy, ambient alternative … splintered breakbeats, warm bass, celestial atmospheres, and melody", influenced by Detroit techno and UK house [22]. LTJ Bukem is the usual originator; Good Looking Records [2].
- LTJ Bukem: "Logical Progression" (1991), "Demon's Theme" (1992), "Atlantis" and "Music" (1993), "Horizons" (1995) [26]. 4hero *Parallel Universe* (1994) and *Two Pages* (1998, Mercury shortlist) [28].

### 3.4 Drum and bass core, hardstep and rollers (1995–)
- By 1995 some producers moved away from ragga; the result became "drum and bass" [1]. Roni Size & Reprazent *New Forms* (1997) won the Mercury Prize [1].
- Rollers are a rolling breakbeat plus "a gripping bassline" — "perhaps the only ingredients a roller needs" [40]; producers call it a drum pattern rather than a sub-genre, found across most styles [40][41]. E-Z Rollers are associated with Moving Shadow's late-1990s two-step **(search excerpt; unverified detail)**.

### 3.5 Jump-up (1995–)
- Came out of hardstep around 1995 and is bassline-driven [2]. "Raw and dancefloor-focused … a stepping beat with hooky electronic bass riffs", "spring-loaded, fast and fun" [22]. Artists: DJ Hype, DJ Zinc, Twisted Individual, Turno, Voltage [22].
- Landmark: DJ Zinc "Super Sharp Shooter" (1996, Parousia) — a slowed, reversed, sped-up bullet sound, cut-up Wu-Tang rap [23]; a "wobbling bass line and an innovative half-time drop" [45]. DJ Hype "Roll the Beats" (1994) [23].

### 3.6 Techstep (1995–)
- Developed from jungle and hardstep ~1995; Ed Rush and Trace coined the name; "tech" referred to harsh early-1990s Belgian hardcore rather than Detroit [19]. Reaction against pop and virtuoso elements, "eschewed melody" for "nightmarish cyborg sounds … regimented, dark beat" [22].
- Sound: almost entirely synthesised or sampled; quantised drum-machine sounds preferred over human breaks; distorted "twisted" bass whose focus moved from depth to timbre; the Reese became a signature; strong emphasis on "the drop" [19]. Described academically as "harsh noise, tonal dissonance and a discourse of sonic violence" [1].
- Landmarks: DJ Trace remix of T-Power "Mutant Jazz" (1995) [19]; Trace/Ed Rush/Nico "Damn Son" from *Torque* (1997, No U Turn; dry looping breaks, high electronics, heavy bass) [23]; Bad Company "The Nine" [22].

### 3.7 Neurofunk (1997–)
- Term coined by Simon Reynolds in *The Wire* (Dec 1997) as a diversion from techstep [20]. Ed Rush & Optical "Funktion" (1997) and *Wormhole* (16 Nov 1998, Virus), often called the first neurofunk album [20][33]. Later: Noisia (formed 2003; *Split the Atom* 2010, *Outer Edges* 2016; Vision Recordings) [32], Phace, Mefjus, Current Value; Critical and Eatbrain labels [20][22].
- Cleaner than techstep; "more regular two-step rhythms" replacing dense jungle polyrhythm; bass built with wavetable, FM, filtering, distortion and resampling, where modulation moves the mid-range harmonics "while a steady low-frequency component provides weight" [20]. "Immaculately produced … mechanical quality … oppressive, dystopian vibe" [22].

### 3.8 Liquid funk (1999–)
- Fabio and Sarah Sandy's Creative Source compilation "Liquid Funk" (1999); grew sharply 2003–04; among best-selling DnB styles by 2005 [21]. Alex Reece and Wax Doctor cited as originators; High Contrast, Marcus Intalex, Calibre; Hospital Records [22].
- "Crisp beats, funky riffs, house influences, Rhodes keyboards, and jazz samples" [22]; more instrumental layers, harmony, melody, vocals; jazz, soul, sometimes blues, Latin and disco influences [21].
- Landmarks: Calibre *Musique Concrète* (2001), *Second Sun* (2005), "Mystic/Feelin" (2000) [30]; High Contrast *True Colours* (2002, Hospital), "Return of Forever", "Global Love" (2002), *Tough Guys Don't Dance* (2007; "If We Ever") [21][29]; London Elektricity *Pull the Plug* (1999; session double bass, guitar, brass, flute, strings) and *Billion Dollar Gravy* (2003) [31]; Marcus Intalex & ST Files "Universe" [22].

### 3.9 Halftime
- Same tempo as DnB but a single snare on beat 3, so 174 BPM feels like ~87 [36]; "halves the tempo to a hip-hop-like lope while keeping the bass and electronics at full speed" [22]. Artists: Om Unit, Dub Phizix, Fracture, Ivy Lab [22].

### 3.10 Big beat and nu skool breaks (slower relatives)
- **Big beat**: ~100–140 BPM; heavy distorted, compressed breakbeats, TB-303-style acid lines, 1960s–70s funk/soul/rock loops, builds, drops, extended drum rolls, sirens and air-horns, filters and flanging. Peak 1995–99 (Skint, Big Beat Boutique). The Prodigy *The Fat of the Land* (1997), The Chemical Brothers *Dig Your Own Hole* (1997), Fatboy Slim *You've Come a Long Way, Baby* (1998) [42].
- **Nu skool breaks**: 125–140 BPM; named at Rennie Pilgrem and Adam Freeland's club Friction (from 1996); *Nu Skool Breakz* vol. 1–2 (1998); dominant bass lines, electro/garage/DnB sounds, less hip-hop sampling than big beat. Artists: Adam Freeland, Stanton Warriors, Plump DJs, Meat Katie, Hybrid [43].
- For the engine these are best treated as the same break grammar at 125–140 BPM with house-length phrasing **(heuristic)**.

---

## 4. Drum grammar at 170–175 BPM

### 4.1 The two-step skeleton
Bonedo's basic two-step, in 8th positions 1–8: kick 1 and 6, snare 3 and 7, hat 2, 4, 5, 8 [11]. On the 16-step grid:

```
step   1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
kick   X . . . . . . . . .  X  .  .  .  .  .
snare  . . . . X . . . . .  .  .  X  .  .  .
hat    . . X . . . X . X .  .  .  .  .  X  .
```

Attack Magazine confirms kick on steps 1 and 11 and snare on 5 and 13 [9]. MusicRadar: the second kick is "pushed later in time to the eighth-note before the second snare", which gives the stepping feel [12]. EDMProd's liquid template states the same in beats: kicks on beat 1 and the "and" of 3 ("bar 1 beat 1 and bar 1 beat 3" in its two-step), snares on 2 and 4, hats on every 8th where kick and snare are absent [14].

- **Swing**: Attack's DnB beats specify 50–60% swing at 168–178 BPM [9][10].
- **Ghost snares**: MusicRadar places the first ghost on the last 16th before beat 3 (step 8) [13]; EDMProd places ghost snares at "2.4 and 3.2" (steps 8 and 10), copied every 4th bar [14]. These match the Amen's pickups (§2.1). Keep ghosts well below the main snare and short — sustain "muddies the beat" — and nudge them off-grid with varied velocities [10]. Hat ghost 16ths at the end of a phrase are swung back and played quieter [9].
- **Ride/hats**: an 8th-note ride, high-passed, bit-crushed, side-chained to kick and snare [9]; or open/pedal hat on the 8ths plus a quiet off-beat open hat [10]. Bonedo: closed hat on 8ths, open hat on quarters with alternate hits quieter, ride mostly on steps 3 and 9 [11].
- **Crashes**: on step 1 every 8 bars and at the end of a 32-bar pattern; extra crashes on bars 5 and 7 of a section [11].
- **Kick sound**: 909-style with "plenty of thump", high-passed ~85 Hz to leave room for the bass [9]; TR-808 kicks pitched down or lengthened are also central [1].
- **Snare sound**: three layers — low punch with boosted fundamental, mid "live" body, fast-attack top through subtle hall reverb [9]; kept short at high tempo [11].

### 4.2 Variants
| Variant | Change from skeleton | Ref |
|---|---|---|
| Displaced snare | Main snare moved back or forward an 8th (e.g. 13→15, as in Amen bar 3) | [5][12] |
| Neurofunk kick | Second kick moved to "the last 16th before beat 3" (step 8) | [12] |
| Halftime | Kick 1, single snare on step 9 | [12][22][36] |
| Kick density | Extra kick on step 16 of bar 3; bar 4 kick moved from step 1 to step 3; sections with kicks on every quarter | [11] |
| Jungle | Full chopped break over/instead of skeleton; two breaks alternating per bar | [1][15] |
| Techstep/neuro | Quantised machine drums, fewer human breaks; "regimented" | [19][20][22] |
| Liquid | Programmed two-step + Think "tops", shaker, ride loops at low level | [14] |

### 4.3 Fills and turnarounds
- Amen fragments and an Amen fill at the end of each 16-bar section; uplifter at the end of each 8-bar pattern [11].
- Turnaround on the last bar of each 8-bar loop (transition break, filtered/phased melody, reverse crash) [15].
- Ghost kicks at the end of every 8th bar leading into the next [14]; end the 8-bar phrase with a "slinky downward roll" [9].
- Build-up kick roll: quarters → 8ths → 16ths into the drop [14].

---

## 5. Approximating break chops with synthesised drums

The engine has no samples, so a "break" must be a synthesised multi-voice kit played with break-like timing and timbre. What the sources say about how real chops work:

- Slice the break into hits, put kick and snare on the grid, leave other hits "loose" for groove [15]; one practitioner reorders 2–4-bar loops in 8th or quarter chunks with only 3–5 slice markers [46]; tracker users sort slices into kick, snare and hat banks, then step through and reorder them with repeats [46].
- Pitch and speed of slices were changed to mimic ghost notes [4]; very tight repetitions become pitched squeals in ragga jungle [50]; time-stretch stutter [23].
- Layer 3+ "dirty" breaks under a programmed skeleton, bracket-EQ'd (cut lows and highs) and bussed through saturation and compression [10]; group programmed drums into a saturator before adding the break [9].

**Engine recipe (heuristic)** derived from the above:
1. Define a "break kit" of 6 voices: low kick (808/909, short), snare-body, snare-noise top, ghost-snare (same snare, −12 to −18 dB, 40–60 ms decay), ride (8ths), and a shuffle layer (noise hat, 16ths, −15 dB, velocity-randomised).
2. Store breaks as **cells** of 2 beats (8 steps) — e.g. the four Amen half-bars and two Think half-bars — each a list of (step, voice, velocity, micro-offset ms).
3. A chop is an operation on cells: *reorder* (permute half-bar cells), *repeat* (play a 1/8 or 1/16 slice 2–8× — a stutter), *pitch* (±1–12 semitones on a slice; ragga squeal = 1/32 repeats pitched up), *reverse* (reverse the envelope of a snare hit), *drop* (mute a slice to leave a gap before a drop).
4. Keep the primary kick on step 1 and backbeat snares (5, 13) intact in 3 bars out of 4; apply heavier chops on bar 4 of each 4-bar group and bar 8/16 of each phrase (matches the fill placements in §4.3).
5. Micro-timing: ghost hits ±5–15 ms off-grid; swing 50–60% on 16ths [9][10].
6. Send the whole break bus through saturation and bracket EQ (HPF ~100 Hz on the break layer so the kick and sub own the low end; LPF ~10–12 kHz for "sampled" grit, optional bit-reduction to emulate the S950) [9][10][48].

---

## 6. Bass

### 6.1 Sub bass
- DnB emphasises deep sub-bass "felt more than heard" on sound systems [1]. Sub is ducked by the kick via sidechain [11]. EDMProd's liquid sub sits around −4 dB, a 4-bar pattern looped to 16 bars, with an extra bass run in every second 4-bar section and the bass muted for the last 2 beats of the 16-bar phrase [14].
- Jungle sub notes are pitched so they land in the E–G range [15]; the jungle bass feel is half-time, ~80 BPM against 160 BPM breaks [24].
- Bonedo's sub: three oscillators an octave apart, sidechained to the kick [11]. Bonedo also uses six different bass patches one at a time, never stacked, to keep the mix clean [11].

### 6.2 Reese bass
- Origin: Kevin Saunderson as Reese, "Just Want Another Chance" (1988, 121 BPM, B♭ minor), played on a Casio CZ-1000/5000 (phase distortion) [18]. Sampled by Renegade (Ray Keith) on "Terrorist" (1994) and from there became the "Reese" of jungle [1][18]. Saunderson: "I was thinking dark, deep" [18].
- Synthesis: two oscillators detuned against each other; Attack's basic patch sets the voice to mono with sustain 1.0 and 24 ms release, and detunes Osc 1 +0.27 semitones and Osc 2 −0.27 semitones (±27 cents); the fine-tune sets the rate of the "rhythmic harmonic movement" [17]. Bonedo's distorted bass uses two saws, the second an octave (or octave-plus-fifth) higher and quieter [11].
- Movement: in techstep the Reese became a signature and bass design moved from depth to timbre [19]; in neurofunk the mid-range is modulated while a steady low component holds weight [20] — i.e. split the bass into a clean sine sub and a processed mid layer.
- Neurofunk patch: Computer Music's Serum walkthrough at 174 BPM sets mono with 250 ms portamento, then heavy saturation and filter movement [44]; FM, wavetable, distortion and resampling are the toolset [20].

### 6.3 Wobble and other movement
- Wobble: an LFO rhythmically modulating volume, distortion or filter cutoff on a held bass note [47]. Jump-up's "Super Sharp Shooter" is cited for its "wobbling bass line" [45]; jump-up's character is "hooky electronic bass riffs" [22].
- Ragga/dub "dread bass" with a wah-wah filter [23]. Bonedo's palette includes a sample-and-hold "talking" bass and an LFO wobble patch [11].

### 6.4 Bass rhythm (heuristic, informed by [14][24])
- Ragga/jungle: one or two long notes per bar at half-time feel (notes on step 1 and step 11, or a 2-bar sustained root), roots following a reggae-style i–VI–VII or pedal movement **(unverified progression)**.
- DnB/rollers: bass hits locked to the kick (steps 1 and 11) with short notes on 16ths in between; a 4-bar riff with a fill run in bar 4 or bar 8.
- Neuro: one long note per 1–2 bars with continuous timbral modulation (filter, FM index, wavefolding), plus 1/16 "edits" (stutters, gaps) at phrase ends.

---

## 7. Sound design palette (per style)

| Style | Signature sounds | Refs |
|---|---|---|
| Hardcore | Hoover (PWM saw ×3 octaves, chorus, fast LFO), house piano stabs, sped-up diva vocals, bleeps, acid | [3][34] |
| Jungle/ragga | Chopped Amen/Think, 808 sub, Reese, dub siren, gunshot, air-horn, MC/ragga phrases, time-stretch stutter, rewind | [2][23][24][38] |
| Intelligent | Ambient pads, celestial atmospheres, jazz samples, warm bass | [22][26] |
| Liquid | Rhodes, piano riffs (low-passed ~350 Hz in the build then opened), pads, bells with 1/4 delay, held drone, soul vocals | [14][21][22] |
| Jump-up | Short hooky bass riffs, rap samples, half-time drops | [22][23][45] |
| Techstep | Reese, distorted twisted bass, bleeps, squelches, sci-fi noise, dry machine drums | [19][22] |
| Neurofunk | Resampled FM/wavetable mid-bass, mechanical drums, minimal melody | [20][22] |
| Big beat | Distorted breaks, 303, sirens, air-horns, flangers | [42] |

Effects chain notes: snare-top through subtle hall reverb, ride shares the snare's reverb send [9]; three reverb sends (short drum room, music hall, bass reverb with low cut) [14]; parallel "New York" drum compression (threshold −20 dB, 2:1, attack ~40 ms, release ~115 ms) [11]; liquid build: piano low-passed at ~347 Hz with cutoff automated open toward the drop, drums low-passed, last 4 drum bars removed [14].

---

## 8. Harmony

- **Liquid**: extended chords — major 7ths and minor 9ths. MusicRadar's example over an A-minor bassline: **Am9 – Cmaj7 – Em7 – Am9 – Cmaj7sus4/F – E**, i.e. i9 – IIImaj7 – v7 – i9 – IIImaj7sus4/VI – V; Am9 and Cmaj7 share C-E-G-B, so moving the bass between A and C re-colours the same voicing [16]. Most liquid is in a minor key, F minor the most common [14]. Soul/jazz/R&B chord vocabulary and funk guitar [21]. Chord changes every 1–2 bars **(heuristic)**.
- **Intelligent**: Detroit-techno and UK-house harmony — sustained pads, slow-moving minor chords [22].
- **Techstep / neurofunk**: melody deliberately avoided [22]; "tonal dissonance" [1]; harmony reduced to a single root pedal with dissonant inflections (semitone and tritone intervals against the root) **(heuristic; no source gives a progression)**.
- **Jungle / ragga**: reggae-derived bass carries the harmony; often a single-key vamp under samples [24] (specific progressions **unverified**).
- **Hardcore**: house piano riffs and stabs, typically major-key rave euphoria vs. darkcore's minor turn [3][23].

---

## 9. Arrangement and energy

### 9.1 DJ-oriented template (174 BPM)
| Section | Bars | Time | Contents | Refs |
|---|---|---|---|---|
| Intro | 32 (16–32) | 0:00–0:44 | Drums/percussion, light atmosphere, hint of hook, little/no sub; first real bass hit ~bar 17 | [36][37] |
| Build 1 | 16 (8–16) | 0:44–1:06 | Noise riser, accelerating snare/kick roll, opening filter, short gap before drop | [14][36][37] |
| Drop 1 | 64 (32–64) | 1:06–2:34 | Full drums, main bass and hook; four 16-bar phrases or two 32-bar halves, second half varied | [36][37] |
| Breakdown | 32 (16–32) | 2:34–3:18 | Drums removed or reduced; chords, pads, vocal, sub | [36][37] |
| Build 2 | 16 (8–16) | 3:18–3:40 | As build 1, often with a twist | [36][37] |
| Drop 2 | 64 (32–64) | 3:40–5:09 | Switch-up: new bass patch, extra layer, new drum pattern or halftime | [36][37] |
| Outro | 32 (16–32) | 5:09–5:53 | Drums/percussion mirror of the intro for mixing out | [36][37] |

Total 256 bars ≈ 5:53 [36]. The first drop arrives after ~48 bars ≈ 66 s ("~1 minute"). A 64-bar DJ intro is mentioned in the brief but **no source found confirms 64 bars as standard**; the sources give 16–32, "often 32" [36][37].

EDMProd's liquid example uses six 16-bar blocks: intro/breakdown, build (piano teased by muting every 2 bars, drums low-passed, bars 29–32 drumless, riser and reverse crash), Drop 1A (core loop), Drop 1B (drums and bass muted for its first 4 bars, counter-melody added), Drop 1C (layers combined, drums and bass removed for its last 2 bars), outro [14]. Wikipedia: the drop is a switch in rhythm or bassline, often after ambient breaks fade out, and the returning beats are typically more complex with heavier bass [1].

### 9.2 Phrase rules
- Drums in 16-bar phrases with an uplifter every 8 bars, crash every 8, fills at the end of 16 [11].
- Turnaround on bar 8 of each 8-bar loop; in jungle the first 16 bars may be drums and bass only, vocals/horns from the third 8-bar block [15].
- Drop 2 differs from drop 1 by a new bass sound, extra layer, different drum pattern or halftime switch, and slightly more saturation/limiting [36][37].

---

## 10. Variation and surprise targets

- **Rewind / reload**: the selector stops the record and spins it back to replay from the start (or the drop), on crowd demand [38][39]. Engine: a rare event after the first 8–16 bars of drop 1 — tape-stop/spin-back (pitch sweep down then reverse, ~0.5–1.5 s), MC-style gap, restart at the drop **(heuristic)**.
- **Halftime drop**: switch drums to snare-on-step-9 for 8–16 bars of a drop [22][45].
- **Break switch**: alternate two breaks per bar, or swap break source at the 16-bar boundary [1][11].
- **Time-stretch stutter**: 1/16–1/32 repeats of a vocal or snare slice, pitched [23][50].
- **Bass patch switch** in drop 2 [36][37]; six-bass rotation [11].
- **Drop-out gaps**: bass muted on the last 2 beats of a 16-bar phrase; drums and bass removed for 2 bars before a section change [14].
- **Dub FX**: siren, gunshot, delay throw on a vocal [14][23].

---

## Rules and parameters

### Tempo & feel
- Set DnB/liquid/neuro/jump-up tempo to 170–176 BPM, default 174. [1][36]
- Set jungle/ragga tempo to 160–170 BPM; breakbeat hardcore 130–155 BPM; darkcore ~155–165 BPM. [1][24][36]
- Set big beat to 100–140 BPM and nu skool breaks to 125–140 BPM. [42][43]
- Apply 16th swing of 50–60%. [9][10]
- In jungle/ragga, write the bass at half-time feel (one bass event per 2 beats or slower, ~80 BPM). [24]
- Halftime mode: keep tempo, move the snare to step 9 only (felt as ~87 BPM). [22][36]

### Drums
- Two-step skeleton: kick steps 1, 11; snare steps 5, 13; hat 8ths on 3, 7, 9, 15. [9][11][12]
- Ghost snares at steps 8 and 10 at −12 to −18 dB with short decay (heuristic level), every 4th bar at minimum. [10][13][14]
- Offset ghost notes by ±5–15 ms and randomise their velocity ±15% (heuristic amounts). [10]
- Ride on all 8ths (odd steps), high-passed, ducked by kick and snare. [9][10]
- Crash on step 1 every 8 bars and at bar 32 of a section; extra crash on bars 5 and 7 optional. [11]
- Variation menu per bar: extra kick on step 16 (p≈0.25 in bar 3 of 4); kick moved 1→3 in bar 4; snare 13→15 displacement; neuro kick on step 8 instead of 11. [5][11][12]
- Jungle: replace or overlay the skeleton with a chopped break; alternate two break sources each bar. [1][15]
- Amen template (bars 1–2): K 1,3,11,12; S 5,8,10,13,16; bar 3: K 1,3,11; S 5,8,10,15; bar 4: K 3,11; S 2,5,8,10,15; crash 11. [4][5]
- Chop operations: reorder 2-beat cells, repeat 1/8–1/32 slices 2–8×, pitch slices ±1–12 st, mute a slice before a drop; concentrate chops on bar 4 of each 4 and bar 8/16 of each phrase (heuristic placement). [4][11][15][46]
- Keep kick on step 1 and backbeat snares intact in ≥3 of 4 bars (heuristic). [15]
- Fill at the end of every 16-bar section; uplifter at the end of every 8 bars; ghost kicks on the last steps of every 8th bar. [11][14]
- Build roll: kick or snare in quarters for 4 bars, 8ths for 2, 16ths for 2 (heuristic split) into the drop. [14]
- Kick: 909/808 short, HPF ~85 Hz on any break-layer kick; snare = 3 layers (body, mid, bright top + hall reverb), short decay. [1][9][11]
- Techstep/neuro: no humanised break layer, tight quantise, swing ≤52% (heuristic). [19][20]

### Bass
- Always split bass into a mono sine sub (≤~100 Hz) and an upper/mid layer; sidechain the sub to the kick. [11][20]
- Reese: 2 saws (or sines) detuned ±20–30 cents (default ±27), mono, sustain 1.0, release ~25 ms. [17]
- Reese variant: second saw +12 or +19 st, quieter, for a distorted bass. [11]
- Neuro/techstep: mono with ~250 ms glide; heavy saturation, then continuous filter/FM/wavefold modulation on the mid layer only. [20][44]
- Wobble: LFO on cutoff or volume synced to 1/4, 1/8 or 1/8T (rate values heuristic). [45][47]
- Liquid bass: 4-bar riff looped to 16 bars; extra run in every second 4-bar group; mute the last 2 beats of each 16-bar phrase. [14]
- Jungle sub roots in the E–G range (≈41–49 Hz). [15]
- Use one main bass patch at a time; switch patches between drop 1 and drop 2. [11][36][37]

### Harmony
- Liquid: minor key (default F minor); extended chords m9, maj7, m7; template i9–IIImaj7–v7–i9–IIImaj7sus4/VI–V, one chord per 1–2 bars (rate heuristic). [14][16]
- Liquid voicing trick: hold the IIImaj7 voicing and move the bass between i and III. [16]
- Intelligent: slow minor pads, 2–4 bars per chord (rate heuristic). [22]
- Techstep/neuro: no chord progression; single root pedal, allow ♭2 and tritone inflections in the mid-bass (heuristic). [1][22]
- Hardcore: major-key piano stab riffs; darkcore: minor. [3][23]

### Melody/hooks
- Liquid: Rhodes or piano riff, low-passed (~350 Hz) during the build and opened at the drop; bell counter-melody with 1/4 delay in drop 1B, second pass an octave up. [14][22]
- Liquid: optional held drone on the tonic for 7–8 bars. [14]
- Jump-up: 1–2-bar hooky bass riff is the main hook. [22]
- Jungle: short vocal/MC phrase hooks, time-stretched and stuttered; dub siren as a call. [23][24]
- Hardcore: hoover lead = PWM saw ×3 octaves + chorus, fast LFO on PW. [34]
- Techstep/neuro: avoid melody; hooks are bass-timbre gestures. [22]

### Sound design
- Drum bus: saturation, then parallel compression (−20 dB threshold, 2:1, ~40 ms attack, ~115 ms release). [9][11]
- Break layer: bracket EQ (HPF ~100 Hz, LPF ~10–12 kHz), optional 12-bit reduction for the sampler sound (values heuristic). [10][48]
- Reverbs: short drum room; hall for music; low-cut bass reverb; snare top and ride share the snare reverb. [9][14]
- Liquid: Rhodes, pads, piano, soul vocal, ride/shaker/Think-style top loops. [14][22]
- Jungle FX: dub siren, gunshot, delay throws, tape spin-back. [23][24][38]

### Arrangement & energy
- Default form at 174 BPM: intro 32 – build 16 – drop 64 – breakdown 32 – build 16 – drop 64 – outro 32 (256 bars, ~5:53). [36][37]
- Ranges: intro/outro 16–32, builds 8–16, drops 32–64, breakdown 16–32 bars; every section a multiple of 8. [36]
- First drop at bar 49 (~66 s at 174 BPM). [36][37]
- Intro: drums and atmosphere only for bars 1–16; first bass hit at bar 17; no full hook. [36][37]
- Outro mirrors the intro (drums + atmosphere) for mixing out. [36][37]
- Build: filter opens, drums low-passed, remove drums for the last 4 bars, riser + reverse crash. [14][37]
- Drop 1 split into 16-bar phrases A/B/C/D; B mutes drums and bass for 4 bars and adds a counter-melody; C/D remove drums and bass for the last 2 bars. [14]
- Breakdown: remove drums (and usually sub), keep pads/chords/vocal. [1][36][37]
- Drop 2: new bass patch or extra layer or new drum pattern or halftime, plus slightly more saturation. [36][37]

### Variation/surprise
- Rewind: with p≈0.1 per track (heuristic), 8–16 bars into drop 1, tape-stop + spin-back 0.5–1.5 s, 1-bar silence, restart drop 1 from bar 1. [38][39]
- Halftime switch: 8–16 bars of halftime in drop 2 (p≈0.3, heuristic). [22][36][45]
- Break swap at each 16-bar boundary; within a bar alternate two break sources. [1][11]
- Stutter: 1/16–1/32 slice repeats of snare or vocal for 1–2 beats at phrase ends, optionally pitched up (ragga squeal). [23][50]
- Gaps: drop bass on the last 2 beats of a 16-bar phrase; drop all drums for 2 bars before a section change. [14]

---

## References

1. Wikipedia contributors. "Drum and bass." *Wikipedia* (accessed 19 Oct 2026); cites Reynolds, *Energy Flash* (2013); Christodoulou, "Rumble in the Jungle," *Transformations* 3 (2002); Reynolds, "Slipping into Darkness," *The Wire* (June 1996). https://en.wikipedia.org/wiki/Drum_and_bass
2. Wikipedia contributors. "Jungle music." *Wikipedia* (accessed 19 Oct 2026); cites Reynolds, *Energy Flash* (2008); Belle-Fortune, *All Crews* (2004); James, *State of Bass* (1997). https://en.wikipedia.org/wiki/Jungle_music
3. Wikipedia contributors. "Breakbeat hardcore." *Wikipedia* (accessed 19 Oct 2026); cites Reynolds, *Energy Flash* (1998). https://en.wikipedia.org/wiki/Breakbeat_hardcore
4. Wikipedia contributors. "Amen break." *Wikipedia* (accessed 19 Oct 2026); cites "Seven seconds of fire," *The Economist* (2011); Otzen, "Six seconds that shaped 1,500 songs," BBC News (2015); Butler, *Unlocking the Groove* (2006). https://en.wikipedia.org/wiki/Amen_break
5. Hein, Ethan. "Building the Amen break." Substack, 8 May 2023. https://ethanhein.substack.com/p/building-the-amen-break
6. International Audio Laboratories Erlangen. "Reverse Engineering the Amen Break – Score-informed Separation and Restoration applied to Drum Recordings" (companion page to a 2016 IEEE/ACM TASLP paper). https://audiolabs-erlangen.com/resources/MIR/2016-IEEE-TASLP-DrumSeparation/AmenBreak
7. Wikipedia contributors. "Think break." *Wikipedia* (accessed 19 Oct 2026); cites Read, "A Brief History of 'The Think Break'," WhoSampled (2017). https://en.wikipedia.org/wiki/Think_break
8. Attack Magazine. "The 2nd Most Sampled Track In History." 27 Oct 2021. https://www.attackmagazine.com/features/long-read/the-2nd-most-sampled-track-in-history/
9. Attack Magazine (Beat Dissected). "Raw Drum & Bass." c. 2012. https://www.attackmagazine.com/technique/beat-dissected/raw-drum-bass/
10. Attack Magazine (Beat Dissected). "Incessant Drum & Bass Beat." c. 2012. https://www.attackmagazine.com/technique/beat-dissected/incessant-drum-bass-beat/
11. Harta, Thomas. "Drum & Bass Basics" (workshop, pp. 1–2). *Bonedo*, 20 Apr 2012. https://www.bonedo.de/artikel/drum-bass-basics and https://www.bonedo.de/artikel/drum-bass-basics/2/
12. Computer Music / MusicRadar. "How to program 6 different jungle and drum 'n' bass grooves." n.d. (body not retrievable; cited from search excerpt). https://www.musicradar.com/how-to/how-to-program-6-different-jungle-and-drum-n-bass-grooves
13. MusicRadar. "12 tips, tricks and ideas for making better beats." n.d. (cited from search excerpt). https://musicradar.com/news/12-drum-production-tips
14. Russell, Aden. "How To Make Liquid Drum & Bass: The Ultimate Guide (2025)." *EDMProd*, 23 May 2025. https://www.edmprod.com/how-to-make-liquid-drum-and-bass/
15. Haven, Simon. "How to Make Jungle Music in 2025: The Complete Producer's Guide." *EDMProd*, 23 May 2025. https://www.edmprod.com/how-to-make-jungle-music/
16. MusicRadar. "How to create uplifting liquid DnB chords." n.d. (body not retrievable; cited from search excerpt). https://musicradar.com/how-to/how-to-create-uplifting-liquid-dnb-chords
17. Attack Magazine. "Reese Bass Redux." 4 Sep 2014. https://www.attackmagazine.com/technique/tutorials/reese-bass-redux/
18. Douglas, Adam. "Deconstructed: Reese – Just Want Another Chance." *Attack Magazine*, 24 Aug 2020 (quotes Saunderson's 2018 RBMA interview). https://www.attackmagazine.com/technique/deconstructed/reese-just-want-another-chance/
19. Wikipedia contributors. "Techstep." *Wikipedia* (accessed 19 Oct 2026); cites Shapiro, *Drum 'n' Bass: The Rough Guide* (1999); Reynolds, *Energy Flash* (2012). https://en.wikipedia.org/wiki/Techstep
20. Wikipedia contributors. "Neurofunk." *Wikipedia* (accessed 19 Oct 2026); cites Reynolds, "2 Steps Back," *The Wire* 166 (Dec 1997); Computer Music, "How to create a neurofunk bass sound in Xfer Records Serum," MusicRadar (26 Jul 2016). https://en.wikipedia.org/wiki/Neurofunk
21. Wikipedia contributors. "Liquid funk." *Wikipedia* (accessed 19 Oct 2026; article flagged for more citations). https://en.wikipedia.org/wiki/Liquid_funk
22. Murphy, Ben. "An expert guide to drum 'n' bass sub-genres." *Red Bull*, 2 Dec 2020. https://redbull.com/ca-en/an-expert-guide-to-drum-n-bass-sub-genres
23. FACT (text by K-Punk). "The 20 best jungle tracks." *FACT Magazine*, 12 Feb 2011. https://factmag.com/2011/02/12/20-best-jungle
24. James, Martin. *State of Bass*, ch. 4 "Armed and Dangerous," excerpted on UKF (Black History Month series), 2024. https://ukf.com/read/bhm-state-of-bass/
25. Wikipedia contributors. "Goldie." *Wikipedia* (accessed 19 Oct 2026); cites Reynolds, *The Wire* 127 (Sep 1994); Roberts, *British Hit Singles & Albums* (2006). https://en.wikipedia.org/wiki/Goldie
26. Wikipedia contributors. "LTJ Bukem." *Wikipedia* (accessed 19 Oct 2026); cites Larkin, *Virgin Encyclopedia of Nineties Music* (2000). https://en.wikipedia.org/wiki/LTJ_Bukem
27. Wikipedia contributors. "Shy FX." *Wikipedia* (accessed 19 Oct 2026). https://en.wikipedia.org/wiki/Shy_FX
28. Wikipedia contributors. "4hero." *Wikipedia* (accessed 19 Oct 2026); cites Shapiro, *Rough Guide to Drum 'n' Bass* (1999); Bächer, "Reinforced Records," RBMA Daily (2016). https://en.wikipedia.org/wiki/4hero
29. Wikipedia contributors. "High Contrast." *Wikipedia* (accessed 19 Oct 2026). https://en.wikipedia.org/wiki/High_Contrast
30. Wikipedia contributors. "Calibre (musician)." *Wikipedia* (accessed 19 Oct 2026). https://en.wikipedia.org/wiki/Calibre_(musician)
31. Wikipedia contributors. "London Elektricity." *Wikipedia* (accessed 19 Oct 2026). https://en.wikipedia.org/wiki/London_Elektricity
32. Wikipedia contributors. "Noisia." *Wikipedia* (accessed 19 Oct 2026). https://en.wikipedia.org/wiki/Noisia
33. Wikipedia contributors. "Wormhole (album)." *Wikipedia* (accessed 19 Oct 2026; cited from search excerpt). https://en.wikipedia.org/wiki/Wormhole_(album)
34. Wikipedia contributors. "Hoover sound." *Wikipedia* (accessed 19 Oct 2026); cites Vail, *The Synthesizer* (2014); Roland Alpha Juno-2 manual (1985). https://en.wikipedia.org/wiki/Hoover_sound
35. Red Bull Music Academy Daily. "Key Tracks: Mundo Muzique on Second Phase's 'Mentasm'." May 2014 (cited from search excerpt). https://daily.redbullmusicacademy.com/2014/05/key-tracks-mundo-muzique-on-mentasm
36. DNB Academy. "Drum and Bass BPM & Song Structure Guide." Updated 13 Oct 2026. https://app.dnbacademy.net/learn/drum-and-bass-bpm-and-structure
37. KAN Samples. "Drum and Bass Track Structure & Arrangement – Complete Guide." 31 May 2026 (vendor blog). https://kansamples.com/blogs/learn/dnb-track-arrangement
38. DJ TechTools. "Reload: A History of the Rewind." 31 Aug 2018 (cited from search excerpt). https://djtechtools.com/2018/08/31/reload-a-history-of-the-rewind
39. Fintoni, Laurent. "Wheel It Up: A History of the Rewind." Dub-Stuy (orig. Cuepoint, 2015). https://www.dub-stuy.com/wheel-it-up-history-of-the-rewind/
40. Jenkins, Dave. "Best classic drum and bass rollers." *Red Bull* (cited from search excerpt). https://www.redbull.com/gb-en/best-classic-drum-and-bass-rollers
41. Marino, Layla. "Rollers are not a subgenre, you muppets." *UKF* (cited from search excerpt). https://ukf.com/read/rollers-are-not-a-subgenre-you-muppets/
42. Wikipedia contributors. "Big beat." *Wikipedia* (accessed 19 Oct 2026; flagged for more citations). https://en.wikipedia.org/wiki/Big_beat
43. Wikipedia contributors. "Nu skool breaks." *Wikipedia* (accessed 19 Oct 2026); cites "Breaks Pilgrem-age," Resident Advisor (2002). https://en.wikipedia.org/wiki/Nu_skool_breaks
44. Computer Music. Neurofunk bass step-by-step in Xfer Serum (two parts). *Computer Music*, 15 Jun 2016 issue (cited from search excerpt; full text paywalled). https://www.pressreader.com/australia/computer-music/20160615/283201557570027
45. Line Noise. "Go DJ: DJ Zinc, bingo beats and ceaseless innovation." (cited from search excerpt). https://buttondown.com/Linenoise/archive/go-dj-dj-zinc-bingo-beats-and-ceaseless-innovation/
46. Elektronauts forum. "Chopping drum breaks" (practitioner discussion; cited from search excerpt). https://www.elektronauts.com/t/chopping-drum-breaks/175570?page=2
47. Wikipedia contributors. "Dubstep" (section "Wobble bass"); cites Clark, *Pitchfork* (2009). https://en.wikipedia.org/wiki/Dubstep
48. Mountford, Saul. "Jungle 101: The Beat." *Reason Studios*, 31 Aug 2025. https://www.reasonstudios.com/news/post/jungle-101-the-beat
49. Polyend. "A Brief History of Jungle: From Amiga to Amen." 7 Nov 2025. https://polyend.com/whats-up/amiga-to-amen/
50. Renoise forum. "Doing crazy jungle drum edits" (practitioner discussion; cited from search excerpt). https://forum.renoise.com/t/doing-crazy-jungle-drum-edits/15408
