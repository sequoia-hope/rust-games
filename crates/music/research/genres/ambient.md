# Ambient, downtempo, chill-out and a survey of other electronic genres

Research notes for the `mp_music` procedural engine. Scope: ambient (Eno and after), ambient house and chill-out, ambient techno and IDM, downtempo and trip-hop, Balearic, lo-fi hip hop; then a short survey of eight more genres a generator could add. Each section gives what the sources support. Where I propose a number the sources don't give, I label it **(design choice)**. Where a source could not be verified, the text says **(unverified)**.

A note on tempos: individual track BPMs below come from the SongBPM database [37], which measures audio automatically. Such databases often report double or half the felt pulse, so treat them as approximate. Genre-level ranges come from the cited articles and books.

---

## 1. Ambient: Eno's definition and his process music

### 1.1 Origins and definition

- **Precursors.** Erik Satie's *musique d'ameublement* ("furniture music", 1910s–20s) was meant to sit in the background at dinner. Cage's *4'33"* (1952) framed a room's own sound as the piece [7].
- **Eno names it.** In January 1975, bedridden after an accident, Eno heard a harp record playing "almost inaudibly". This suggested "a new way of hearing music – as part of the ambience of the environment just as the colour of the light and the sound of the rain were parts of that ambience." He asked for *Discreet Music* (1975) to be played "at comparatively low levels, even to the extent that it frequently falls below the threshold of audibility" [3].
- **The manifesto.** The liner notes of *Ambient 1: Music for Airports* (dated September 1978; released February 1979) say: "Ambient Music is intended to induce calm and a space to think. Ambient Music must be able to accommodate many levels of listening attention without enforcing one in particular; it must be as ignorable as it is interesting" [2][6].
- **Generative music.** Eno said he began working with SSEYO's Koan software in 1995 and coined "generative music" there. He released *Generative Music 1* on Koan in 1996 [20]. In his 1996 San Francisco talk he defined it: generative music "specifies a set of rules and then lets them make the thing." He called the computer "the way of growing little seeds" [1]. *Reflection* (Warp, 1 January 2017) shipped as a 54-minute fixed excerpt plus an iOS app. The app plays forever and "modulates its output at different times of the day". Its rules were updated each season, and Peter Chilvers is credited with "mutation software" [21].

For a seeded engine this is the most relevant lineage. Eno's ambient works are already small rule systems with fixed inputs. Their character comes from incommensurable loop lengths and very slow change, not from composed form.

### 1.2 *Discreet Music* (1975): two loops and a long echo

Eno's liner notes describe the system [3]:

- He wrote: "I tend towards the roles of the planner and programmer, and then become an audience to the results."
- **Input:** "two simple and mutually compatible melodic lines of different duration stored on a digital recall system". This was the EMS Synthi AKS sequencer [50].
- **Processing:** a "long delay echo system". The signal went through a graphic EQ and an echo unit to tape machine A. The tape travelled on to the take-up reel of machine B, and B's playback was fed back into A [50][5].
- **His only intervention:** "occasionally altering the timbre of the synthesizer's output by means of a graphic equalizer."
- **No surprises, on purpose:** his "pleasure in 'gradual processes' prevented me from attempting to create surprises and less than predictable changes in the piece" [3].

Reverb Machine reverse-engineered the piece [5]:

- **Key:** A♭ major.
- **Right sequence:** outlines A♭–C–E♭ and adds the 6th.
- **Left sequence:** leans on D♭, the 4th, which clashes a semitone with C. It holds "a lot of silence" and ends with a four-note motif.
- **Loop lengths (final recording):** about 63 s on the left and about 69 s on the right. The loops "return near their original alignment after about 15 minutes". By exact arithmetic, lcm(63, 69) = 1449 s ≈ 24 min (my calculation).
- **Delays:** the long tape echo is estimated at about 2.8 s at original speed. Reverb Machine's remake used 5.65 s with high feedback and a mix just under 50%, plus a 160 ms Echoplex-style delay with high feedback at a 40% mix.
- **Synth patch:** a sine plus a triangle oscillator, with a slow low-level vibrato. Cutoff sits just under halfway. Attack and decay are soft (6 and 7 on the Synthi's scales).

### 1.3 *Music for Airports* (1978–79): incommensurable tape loops

| Track | Length | Material | System |
|---|---|---|---|
| 1/1 | 16:30 | Robert Wyatt piano and Rhodes; D major moving to D mixolydian (a C♮ appears); the Rhodes holds a D bass throughout [4] | Piano through echo, looped, then played at half speed (one octave down) [4] |
| 2/1 | 8:20 | Single sung notes from three women (Christa Fast, Christine Gomez, Inge Zeininger) [6] | Seven loops of different lengths, one note each [4] |
| 1/2 | 11:30 | Voices and piano [6] | Loops |
| 2/2 | 6:00 | ARP 2600, slowed down, "three kinds of echo" [6] | — |

The loop lengths come from Eno himself, in the 1996 talk [1]. One note "repeats every 23 1/2 seconds", the next "every 25 7/8 seconds or something like that", and the third "every 29 15/16 seconds or something". The cycles are "incommensurable", so they are "not likely to come back into sync again". What you hear is "the various clusterings and configurations of these six basic elements", and "the basic elements in that particular piece never change."

Reverb Machine's reconstruction adds detail [4]:

- The loops were wrapped around chair legs, so their timing wobbles.
- Named pitches include C at about 20.1 s and a high F at about 19.6 s. A D♭ loop is "significantly" longer and clashes a semitone with C.

Two quotes cited on the Wikipedia page for the album [6]:

- Eno wanted "a silence at least twice as long as the sound" (Tannenbaum, *Musician*, 1985).
- The album was "essentially made by machines" (*Mojo*, 2009).

**How slow it is, in numbers:**

- In 1/16-second units Eno's three periods are 376, 414 and 479. Their lcm is about 3.7 × 10⁷ units ≈ **27 days** before the three line up again (my calculation).
- Each voice sounds for one or two bars of a loop that is 17–30 s long. Each sounds less than a third of the time (the "silence twice as long as the sound" rule).
- Several loops are running, so a new note-onset arrives every few seconds. Any given vertical chord, though, effectively never recurs.

**Precedents.** Reich's *It's Gonna Rain* (1965) used two identical speech loops running at slightly different speeds. Riley's *In C* (1964) has 53 short cells, which performers repeat at will while staying within a few cells of each other [51]. Eno replaced Reich's two machines with many loops of different lengths [51].

### 1.4 Ambient after Eno: textures and techniques

- **Sound vocabulary** [7]:
  - "tone and atmosphere over traditional structure or rhythm";
  - "typically lacks a steady beat and structured melody";
  - "sustained or repeated notes, as in drone music";
  - nature soundscapes.
- **Durations** [7]: Éliane Radigue's three hour-long ARP 2500 works (1988–93) set the scale of drone pieces.
- **Lo-fi production** [11]:
  - Aphex Twin's *Selected Ambient Works 85–92* (Apollo/R&S, 9 Nov 1992) has 13 tracks over 74:40.
  - It was recorded 1985–92 on homemade equipment, partly to cassette; AllMusic notes the sound quality suffered.
  - Database tempo for "Xtal" is about 115 BPM [37]. These are beat-driven pieces with ambient pads.
- **Beatless and microtonal** [12]:
  - *Selected Ambient Works Volume II* (Warp, 7 March 1994; 156:42 on CD) is "largely beatless", with "chilly" textures and "heavy use of microtonal tunings".
  - A few tracks have slow tribal beats, such as "Grass".
  - Instruments include EMS, Moog, ARP and Oberheim Matrix synths and a Yamaha CS-5. James credited lucid dreaming as the source.
- **Rhythm through processing** [5]: *Discreet Music*'s 2.8–5.65 s repeats turn any sparse input into continuous, overlapping texture.
- **Tape, degradation and resampling** [14][15]:
  - Boards of Canada's *Music Has the Right to Children* (Warp/Skam, 20 April 1998) used "intentionally detuned vintage synthesizers, drum machines, and reel-to-reel tape recorders".
  - Birdsong from an open studio window was kept on "Rue the Whirl" [14].
  - The duo describe "feeding those sounds through stacks of destructive hardware and resampling them to make unrecognisable new sounds," which "gives it a really nice untidy energy" [15].
  - Database tempo for "Roygbiv" is about 84 BPM in E minor [37]: a hip-hop pulse under nostalgic, detuned pads.
- **Granular processing** [19]: Academic accounts of IDM and ambient electronica list granular synthesis, slicing, time-stretching and "freezing" as core techniques (Papavassiliou 2010, via [19]).

### 1.5 Ambient house and the chill-out room (1989–1994)

- **Origins** [8][32]:
  - In 1989 Paul Oakenfold ran the Land of Oz acid house night at Heaven, London.
  - Alex Paterson, with Jimmy Cauty and Youth, ran the chill-out counterpart in the White Room, so dancers could "chill out" from the faster main floor.
  - They mixed Eno, Pink Floyd, records and sound effects.
- **Sound** [8]:
  - AllMusic: "midtempo, four-on-the-floor beats; synth pads and strings; soaring vocal samples."
  - The *Bloomsbury Encyclopedia*: "repeated synthesizer arpeggios that are gradually modulated, reverbed snippets of film, radio, or relaxation-tape dialogue, and samples drifting in and out."
- **Landmarks:**
  - The Orb, "A Huge Ever Growing Pulsating Brain That Rules from the Centre of the Ultraworld" (Oct 1989) [8].
  - The Orb, "Little Fluffy Clouds" (16 Nov 1990) [10]:
    - speech from a Rickie Lee Jones interview;
    - Reich's *Electric Counterpoint* played by Pat Metheny;
    - a Morricone harmonica sample;
    - drums from a Jim Gordon solo on Nilsson's "Jump Into the Fire", slowed from 45 to 33 rpm.
    - Database tempo is about 105 BPM [37].
  - The Orb, *Adventures Beyond the Ultraworld* (1991) [8].
  - The Orb, "Blue Room" (1992): about 40 minutes long, the longest single to reach the UK charts [8].
  - Global Communication (Tom Middleton and Mark Pritchard), *76:14* (Dedicated, 1 June 1994). It is "mostly beatless", and each track is titled by its own length "to avoid implying any specific meaning" [13].
- **The KLF, *Chill Out*** (5 Feb 1990) [9]:
  - Recorded at Trancentral as **one 44-minute live take with no edits**. A mistake meant restarting from the top.
  - Concept: an imaginary night-time journey along the US Gulf Coast, as one continuous piece with segues. Percussion builds in the second half.
  - Sources included Elvis, Fleetwood Mac's "Albatross", train sound effects, BBC pips, Tuvan throat singing, birds and water, sheep, and Graham Lee's pedal steel.
  - Track sections run 0:19 to 9:50.

### 1.6 Ambient techno and IDM

- **Ambient techno** [16]:
  - Late 1980s to early 1990s, UK and US; marketed as "electronic listening music".
  - Sound: "heavily reverbed string pads", analogue synths, TB-303 and TR-909, "subtle drum programming that goes beyond simple 4/4 patterns", little sampling, albums rather than 12-inch singles.
  - Landmarks: Carl Craig (*Elements 1989–1990*); Aphex Twin, "Analogue Bubblebath" (1991) and *SAW 85–92* (1992); Biosphere, *Microgravity* (1991); The Black Dog, *Temple of Transparent Balls* (1993); B12, *Electro-Soma* (1993); Pete Namlook's Fax label (1992).
- **Biosphere (Geir Jenssen)** [17]:
  - Albums: *Microgravity* (1991), *Patashnik* (1994), *Substrata* (1997, "often cited as an all-time classic ambient album").
  - Style: Arctic and mountain imagery, sci-fi and nature loops, field recordings, film and radio samples (*Twin Peaks* on *Substrata*), and "drones that change slowly".
- **IDM** [19]:
  - The term traces to Warp's *Artificial Intelligence* compilation (1992), subtitled "electronic listening music", and Alan Parry's 1993 IDM mailing list.
  - Papavassiliou: popular-music structures (binary rhythms, repetition) with rhythmic and timbral complexity.
  - Reynolds: an early house/techno-beat phase, then a jungle-influenced phase.
- **Autechre (Sean Booth, Rob Brown)** [18]:
  - Albums: *Incunabula* (1993), *Amber* (1994), *Tri Repetae* (1995), *Chiastic Slide* (1997), *LP5* (1998), *Confield* (2001).
  - Max/MSP has been a main tool since about 1997. *Confield* "grew out of experiments with Max" (Tingen, *Sound on Sound*, 2004). Booth said *Untilted* (2005) had "no generative work".
  - Early work was melodic techno; *Tri Repetae* moved toward harsher, mechanical textures.

---

## 2. Downtempo and trip-hop

### 2.1 Origins and landmarks

- **Bristol roots** [22]:
  - Trip-hop grew out of Bristol's sound-system culture (dub plus hip hop) and the Wild Bunch crew, who became Massive Attack.
  - Andy Pemberton coined "trip hop" in *Mixmag* in June 1994, for DJ Shadow's "In/Flux" (1993) and similar Mo' Wax releases.
- **Massive Attack, *Blue Lines*** (8 April 1991) [23]:
  - Simon Reynolds: the songs run at "spliff" tempos, from about 90 BPM down to 67 BPM (*Generation Ecstasy*, 1998, p. 321).
  - Daddy G: "dance music for the head rather than the feet".
  - "Unfinished Sympathy" adds a live string section, arranged by Wil Malone and recorded at Abbey Road.
- **Massive Attack, *Mezzanine*** (20 April 1998) [25]: dark and claustrophobic; heavy bass, ambient textures, post-punk and dub. Database tempo for "Teardrop" is about 77 BPM in B minor [37].
- **Portishead, *Dummy*** (22 Aug 1994; won the 1995 Mercury Prize) [22][24]:
  - The band recorded their own music, cut it to vinyl, and sampled it back.
  - Geoff Barrow distressed the records by "putting them on the studio floor and walking across them". He recorded them through a broken amplifier (*NYT*, 2019).
  - Instruments: Rhodes, Hammond, theremin, guitar; drummer Clive Deamer.
  - Pitchfork's description: "boom-bap" with "homemade breakbeats" set against silence.
  - Database tempo for "Sour Times" is about 94 BPM in G♯ minor [37].
- **Genre traits** [22]:
  - bass-heavy drums, often slowed-down breakbeats;
  - slow tempos and a melancholic mood;
  - often female vocals;
  - Rhodes, horns, flute, theremin, Mellotron;
  - turntable scratching and film-soundtrack samples.

### 2.2 Downtempo, chill-out and Balearic

- **Downtempo** ("downbeat") [31]:
  - Close to ambient but rhythm-forward; "usually around 90 BPM".
  - Nightmares on Wax pioneered it in the early 1990s.
  - Kruder & Dorfmeister popularised it in the late 1990s with soul-jazz-influenced remixes. Thievery Corporation added Brazilian music and dub.
- **Chill-out** [32]: Rick Snoman's *Dance Music Manual* (2013) suggests that anything under about 120 BPM with a laid-back groove can be called chill-out.
- **Balearic beat** [34]:
  - Origins: DJ Alfredo at Amnesia, Ibiza, mid-1980s; Oakenfold, Fung and St. Paul brought it to London in 1987.
  - Records were "often slower and R&B-influenced, under 119 BPM". A 909 played kick, snare and hats "in laid-back swing patterns". Influences mixed house, Italo, soul, Latin, African, funk and dub.
- **Café del Mar** [35]: the bar opened in 1980. José Padilla's sunset sets began in 1991, and he compiled the first six albums (1994–99). Artists on the compilations include Pat Metheny, Chicane and Underworld.
- **Air, *Moon Safari*** (16 Jan 1998) [33]:
  - Instruments: Minimoog, Korg MS-20, Solina String Ensemble, Rhodes, Wurlitzer, Mellotron, vocoder, talk box.
  - Live drums on some tracks, a drum machine on others; David Whitaker strings.
  - The album is credited with helping set up downtempo as a style.
- **Bonobo (Simon Green)** [36][37]:
  - *Black Sands* (2010); *Migration* (2017). Database tempo for "Kiara" is about 97 BPM in E♭ minor [37].
  - On timing: "I don't believe in quantize or groove pools." He draws drums by hand, turns the grid off every other bar, and nudges single hits' timing and velocity [36].
  - On sources: re-pitching samples is "one of the biggest tools". "I just build up layers and layers and layers, and then remove the common element that was there to start with" [36].
  - His first two albums were made on Akai samplers with 32 seconds of memory and 8-note polyphony [36].

### 2.3 Downtempo drum grammar (16-step grid, step 1 = downbeat)

Attack Magazine's "Beat Dissected" series by The Groove Criminals gives two specs:

| Spec | "Raw strip-hop" [26] | "Dusty hip hop" [27] |
|---|---|---|
| Tempo | 80–90 BPM | 90–110 BPM |
| Swing | 50–60% | 50–60% |
| Kick | layered (boom + punch + vinyl crunch); a longer attack gives a "lazier" feel; **ghost kicks at steps 9 and 16** | first kick of the bar exactly on the grid at full velocity; other kicks vary in velocity and timing |
| Snare | beats 2 and 4 (**steps 5, 13**); a ghost note in the second-to-last bar; hall reverb | solid on beats 2 and 4, layered |
| Hats | closed hat 1 on straight 8ths (1,3,5,…,15), mixed low; closed hat 2 on sporadic 16ths with velocity changes on almost every hit; open hats against the kick | simple closed pattern; two open hats choked by mute groups |
| Extra | a heavily reverbed "bass hit" on step 1 of each bar | conga/tom balancing the start and end of the bar; vinyl crackle "at a low volume"; drum bus through compression or tape emulation |

**Swing in numbers.** Roger Linn says swing "merely delay[s] the second 16th note within each 8th note" [28]:

- 50% means no swing.
- 54% "loosen[s] up the feel without it sounding like swing".
- 66% is "perfect triplet swing".
- A 90 BPM groove "will feel looser at 62% than at a perfect swing setting of 66%".
- "Between 50% and around 70%" holds most useful settings.

For downtempo this means 54–62%.

---

## 3. Lo-fi hip hop ("beats to relax/study to")

- **Lineage** [29][30]:
  - J Dilla, whom fans call "the godfather of lofi hip hop", made hand-played, unquantized beats on the Akai MPC.
  - Madlib and MF Doom's *Madvillainy* (2004) is a shared touchstone.
  - Nujabes became widely known through the *Samurai Champloo* soundtrack.
  - The Roland SP-303 and SP-404 samplers had a dedicated lo-fi effect.
  - YouTube 24/7 streams (ChilledCow, later Lofi Girl) made the genre a phenomenon from about 2015; by 2018 several channels had millions of followers.
- **Musical profile.** Winston & Saywood's academic study is the best source [29]:
  - **Tempo:** "usually relatively slow, between 70 and 85 BPM, by our own sampling."
  - **Instruments:** acoustic piano most often; Fender Rhodes; nylon-string or hollow-body electric guitar, "often processed through a vibrato or chorus effect"; occasionally harp, ukulele or sampled strings.
  - **Harmony:** "frequently use jazz-influenced extended chords".
  - **Timing and pumping:** "slightly loose, rhythmically relaxed drums, particularly the kick and the snare". Sidechaining melodic parts to the kick makes "a rhythmic rise and fall of volume". Both are "almost universal".
  - **Texture:** samples are aged by "deliberately filtering out the higher frequencies" and "at times, layering a crackling vinyl sound effect". Dilla used cassette-hiss samples for an "amount of imperfection".
  - **Form:** the central melodic element is a loop, "a cycle out of linearly-conceived melody", over drums built from single hits.
- Pitchfork's Philip Sherburne summed it up as slow tempos, repetitive piano or guitar melodies, and simulated vinyl hiss [30].

---

## 4. Synthesis notes for a no-samples engine

The engine cannot sample, so each "found" sound needs a synthesized stand-in. The mappings below are **(design choice)**; the sources they draw on are cited.

| Source texture | Where it appears | Synth stand-in |
|---|---|---|
| Long sung or played single notes on loops | Eno 2/1 [1][4] | Bowed-string or choir-like pad: soft attack (Synthi patch 6/10 [5]), formant filter, one note per voice |
| Long tape echo | *Discreet Music* [3][5] | Delay of 2.8–5.65 s, feedback 0.7–0.9, mix 40–50%, gentle low-pass in the loop |
| Half-speed tape | MfA 1/1 [4] | Transpose −12 semitones and lengthen envelopes ×2 |
| Detuned vintage synths, tape wobble | BoC [14][15]; lo-fi guitar chorus/vibrato [29] | Juno-style poly with a slow pitch LFO (wow) plus a fast small one (flutter) on the tape stage |
| Vinyl crackle, cassette hiss | Portishead [24], lo-fi [29], Attack [27] | Sparse random impulses through a band-pass, plus pink noise at a low level |
| Rain, birds, water, trains | KLF [9], BoC [14], Burial [40] | Filtered-noise "rain" (dense tiny grains); FM chirps (birds); low noise with slow AM (train) |
| Radio and dialogue snippets | Ambient house [8] | Formant-filtered noise bursts band-limited to 300–3000 Hz as "radio voice" (design choice) |
| Microtonal pads | SAW II [12] | Optional detune of ±10–50 cents per voice (design choice) |
| Granular freeze/stretch | IDM/ambient [19] | Grain cloud over a held pad, 20–100 ms grains (design choice) |

---

## 5. Survey: other electronic genres a generator might add

**Dubstep (early UK, about 2002–2008).**

- *Origins:* South London and Croydon, out of UK garage, 2-step, dub, grime and drum and bass. Hubs: the FWD>> night, Rinse FM, Big Apple Records, and DMZ in Brixton [38].
- *Landmarks:* Skream's "Midnight Request Line" (about 2005); Benga & Coki's "Night" (2007); Burial's *Burial* (2006) and *Untrue* (Hyperdub, 5 Nov 2007) [38][40].
- *Tempo:* 132–142 BPM, mostly 140, felt in half time (about 70) [38][39].
- *Drums:* a clap or snare "on every third beat of a bar", so a half-step snare on step 9 [38]. Sound on Sound's patterns build swing from a triplet grid. One example has kicks on beats 1 and 4 and an extra kick two-thirds of the way between beats 2 and 3. Half-step kicks are "lazy" off-beat hits [39].
- *Bass:* a sine sub at C0–C1 (about 30–60 Hz) [39]. Wobble comes from an LFO, usually tempo-synced, on filter cutoff, volume or distortion [38][39].
- *Harmony and form:* minor keys, Phrygian, tritones. Drops usually come after bar 16 or 32 [38].
- *Burial's method:* he worked in Sound Forge, a plain audio editor, not a sequencer. He used rain and vinyl hiss, sounds from *Metal Gear Solid* and keys, and pitch-shifted R&B vocals, cut word by word [40]. Database tempo for "Archangel" is about 135 BPM [37].

**Future garage (mid-2000s on).**

- *Tempo:* 130–140 BPM. *Drums:* 2-step or four-on-the-floor, with "off-grid hi-hats"; the swing comes from late, humanized hats between kick and snare [41].
- *Sound:* pitched vocal chops, warm filtered Reese bass, dark pads, field recordings and vinyl crackle [41].
- *Name:* coined by Whistla; the Future Garage Forum opened in November 2009 [41].
- It is Burial's sound, stretched toward ambient.

**Footwork/juke (Chicago, late 1990s on).**

- *Tempo:* about 150–165 BPM, often 160, alternating with half-time sections [42].
- *Drums:* sparse, syncopated TR-808 kicks that "skip beats"; crackling snares and claps [42].
- *Samples and bass:* rap and pop vocal chops pitched up or down, heavy sub, lo-fi production [42].
- *Landmarks:* RP Boo's "Baby Come On" (1997), often called the founding track; Planet Mu's *Bangs & Works* Vol. 1 (2010); DJ Rashad's *Double Cup* (2014) [42].
- Triplet subdivisions are common by ear but **unverified** in the sources reached.

**Hardstyle (Netherlands, Belgium and Italy, late 1990s to 2000).**

- *Tempo:* about 140 BPM early; 150 or more now (150–160) [43].
- *Kick:* overdriven, tuned ("pitched") kicks, paired with an off-beat "reverse bass" taken from hard trance [43].
- *Sound:* vocal samples, distorted "screeches", synth leads [43].
- *History:* Qlubtempo (2000) was the first event billed as hardstyle; Q-dance trademarked the name in 2002 [43].
- *Engine note:* the kick doubles as the bassline (design implication).

**Electro house / big room (about 2010–2014).**

- *Tempo:* 126–132 BPM [44].
- *Drums:* four-on-the-floor and minimal in the drop: a kick or tom and a couple of hats [44].
- *Form and sound:* long build-ups into a drop with a short, simple melody, often a supersaw lead in large-hall reverb [44].
- *Landmark:* Martin Garrix's "Animals" (2013) [44].

**Future bass (2010s).**

- *Tempo:* 130–175 BPM [45], in practice usually half-time **(unverified)**.
- *Sound:* chords whose filter or volume is modulated by LFO or automation ("wobbly"); vocal chops; vocoders; "twinkly" noise risers; trap- and dubstep-derived drums [45].
- *Landmarks:* Rustie's *Glass Swords* (2011); Flume's remix of "You & Me" (2013); 2016 was the breakout year [45].

**Vaporwave (2010–2011).**

- *Landmarks:* Daniel Lopatin's *Chuck Person's Eccojams Vol. 1* (2010); Macintosh Plus's *Floral Shoppe* (Dec 2011) set the template [46].
- *Method:* almost entirely slowed-down samples of 1980s–90s smooth jazz, muzak, R&B and lounge, chopped and screwed, repeated, with heavy reverb [46].
- *Tempo:* not stated in sources; it follows from slowing the source.
- *Engine translation:* render a smooth-jazz or "Muzak" loop, then pitch and time it down 10–30% **(design choice)**.

**Afro house and amapiano (South Africa).**

- *Afro house:* house fused with South African rhythms, from kwaito and deep house; tempo listed as about 80–122 BPM [49]. Landmarks: Black Coffee's debut (2005); Master KG's "Jerusalema" (2020) [49].
- *Amapiano:* mid-2010s, Johannesburg townships or Pretoria (disputed). It mixes kwaito, deep house, jazz and lounge, with "piano melodies" and the **log drum**: "a wide percussive bassline", credited by Kabza De Small to MDU aka TRP [47][48].
- *Soulful amapiano ("private school piano"):* shakers, mellow log drums, progressive chords [47].
- *Landmark:* Tyla's "Water" (2023) [48].
- *Tempo:* commonly quoted as about 110–115 BPM **(unverified)**; no source reached gives a BPM.
- *Engine translation:* the log drum as a short tuned FM or pitch-swept sine "bass hit" **(design choice)**.

---

## Rules and parameters

### Tempo & feel

- Ambient (Eno-style): run no global pulse. Schedule events in seconds, not steps. [1][4][7]
- Ambient techno and ambient house: 4/4 at 100–120 BPM; database examples are 105 ("Little Fluffy Clouds") and 115 ("Xtal"). [8][37]
- Ambient-hip-hop (Boards of Canada mode): 80–95 BPM; "Roygbiv" is about 84. [37]
- Trip-hop: 67–95 BPM (*Blue Lines* runs 67–90 [23]; "Teardrop" 77, "Sour Times" 94 [37]). Weight the draw toward 75–90.
- Downtempo/chill-out: 85–110 BPM; centre on 90. Cap chill-out below 120. [31][32][27]
- Balearic: 100–118 BPM, four-on-the-floor with a laid-back swing. [34]
- Lo-fi hip hop: 70–85 BPM. [29]
- Swing amount (Linn %): ambient techno and Balearic 50–54; downtempo and trip-hop 54–60; lo-fi 56–62; never above 66 except as a triplet grid. [26][27][28]
- Humanize lo-fi and Bonobo-mode kicks and snares with ±5–20 ms timing jitter. Keep step 1 of each bar exact. (Jitter range is a design choice.) [27][29][36]

### Drums

- Ambient: no drums at all with probability ≥ 0.7; if drums are present, use a sparse slow "tribal" pattern at low level. [7][12][13]
- Ambient house: four-on-the-floor kick (1,5,9,13), soft hats, low drum level under pads. [8]
- Trip-hop/downtempo: kick on step 1; snare on 5 and 13; ghost kicks at 9 and 16 at 30–50% velocity; closed hat on the 8ths (odd steps) mixed low; a second hat on random 16ths with per-hit velocity. [26]
- Trip-hop: add one reverbed low "bass hit" or boom on step 1 of each bar, or every 2 bars. [26]
- Dusty hip-hop mode: open hats choked by the next closed hat; optional conga or tom at the start and end of the bar. [27]
- Trip-hop/Portishead mode: run the drum bus through saturation and a high cut at 6–10 kHz to suggest "homemade breakbeats". (Cutoff is a design choice.) [24]
- Lo-fi: kick and snare loose; snare on 5 and 13; low-passed kit; no crash cymbals. [29]
- Every downtempo kit gets vinyl crackle at −30 to −40 dB under the drums. (Level is a design choice.) [27][29]

### Bass

- Ambient: one held pedal tone (tonic, or tonic plus 5th) that changes at most once every 2–5 minutes; MfA 1/1 holds a D bass throughout. (Rate is a design choice.) [4]
- Ambient techno: optional slow 303 line or sub pulse, filter barely opening. [16]
- Trip-hop: deep, round sub or dub bass on root notes, 1–2 notes per bar, leaving silence between. [22][24]
- Lo-fi: soft bass (sine or round FM) under the chords, root plus approach notes; sidechain-duck it 2–4 dB to the kick. [29]

### Harmony

- Ambient key/mode palette: Ionian with an added 6th (A♭–C–E♭–F); Mixolydian ♭7 colour; sus4 clash (4th against 3rd). [4][5]
- Allow one semitone clash between two loops (e.g. D♭ against C) at low density; it is a feature. [4][5]
- Ambient: no chord progression. Harmony emerges from overlapping single-note loops drawn from one 5–7-note pitch set that stays fixed for the whole piece. [1][4]
- SAW II mode: optional microtuning, ±10–50 cents per voice. (Amount is a design choice.) [12]
- Trip-hop/downtempo: minor keys (database examples B minor, G♯ minor, E♭ minor); progressions of 2–4 chords looping over 4–8 bars. (Loop length is a design choice.) [37][22]
- Lo-fi: extended jazz chords (maj7, m7, 9ths) on Rhodes or piano; 2–4-chord loops; ii–V–I is acceptable. [29]
- Dubstep (survey): minor/Phrygian, optional tritone. [38]

### Melody/hooks

- Ambient: no melodies. Each voice plays one note, or a 2–5-note phrase, per loop. [1][3][4]
- *Discreet Music* mode: two phrases with different loop lengths (ratio about 63:69), panned hard left and right, through a long echo. [5]
- Lo-fi/downtempo hook: a short repeating piano, Rhodes or chorused-guitar figure, 1–2 bars, looped unchanged. [29]
- Trip-hop: a sparse lead (theremin-like sine with vibrato, or Rhodes) over the loop; leave at least 50% of bars without a lead. (Ratio is a design choice.) [22][24]

### Sound design

- Ambient pads: soft attack of 1–4 s and release of 3–10 s (design choice, after the Synthi patch's soft settings); sine plus triangle mix; slow low-depth vibrato. [5]
- Long echo: delay 2.8–5.65 s, feedback 0.7–0.9, wet 40–50%, high cut in the feedback path; optional second short echo of 160 ms at about 40% wet. [5]
- Half-speed variant: −12 semitones and ×2 envelope times on any ambient voice. [4]
- Reverb: large hall, decay 6–15 s for ambient, 2–4 s for downtempo. (Values are design choices; the sources say "heavily reverbed".) [16][26]
- Tape stage on downtempo, BoC and lo-fi: wow at 0.3–1 Hz, ±5–20 cents; flutter at 6–10 Hz, ±2–5 cents; saturation; high cut at 8–12 kHz. (Values are design choices; the techniques are sourced.) [14][15][29]
- Detune a Juno-style poly's oscillators by 5–15 cents for BoC and Air-style pads. [14][33]
- Noise beds: "rain" (dense grains of high-passed noise), "birds" (FM chirps of 1–3 kHz, 50–200 ms), "train" (low noise with AM at 0.5–2 Hz), at −25 to −35 dB. (Synthesis is a design choice.) [9][14][40]
- Air mode: Solina-style string ensemble, Moog-style mono bass, vocoder-like formant lead, Rhodes or Wurlitzer. [33]

### Arrangement & energy

- Ambient loop lengths: draw 3–7 voices with periods of 17–70 s and pairwise ratios that are not small integers (e.g. 23.5, 25.875, 29.9375 s). [1][4][5]
- Duty cycle: each ambient voice is silent for at least ⅔ of its loop ("a silence at least twice as long as the sound"). [6]
- Ambient total density: 1 note-onset per 3–8 s across all voices. (Derived from 3–7 voices with 20–30 s loops.) [1][4]
- Ambient timbre drift: change a pad's EQ or filter by at most one small step every 1–3 minutes; that is the only intervention allowed. [3]
- Ambient piece length: 8–30 minutes per piece (MfA tracks run 6:00–16:30; *Discreet Music* about 30 minutes); endless in radio mode. [3][6][21]
- Long-form mode: one continuous piece with segues between scenes of 0:20–10:00; percussion density rises across the second half. [9]
- Ambient house: layer sections 16–32 bars long; arpeggios whose filter or modulation moves gradually over 32–64 bars. (Bar counts are a design choice.) [8]
- Trip-hop/downtempo: intro 8 bars (drums or a pad alone), verse/loop 16 bars, breakdown 8 bars (drums out), outro 8–16 bars. (Bar counts are a design choice; the sources describe loops without fixed bar counts.) [22][26]
- Lo-fi: one 4- or 8-bar loop for 1.5–3 minutes with drums dropping in and out; no build or drop. (Durations are a design choice.) [29]
- Time-of-day modulation: let the ambient radio lower brightness and density at night. [21]

### Variation/surprise

- Ambient surprise budget: about 0. No sudden changes; "gradual processes" only. [3]
- Ambient variation comes only from loop phase, which never repeats within a session (realignment ≥ 15 minutes for 2 loops; days for 3 or more). [1][5]
- Ambient house surprise: every 1–3 minutes, drop in a "found" event (radio-voice burst, bird or train texture, an echoed foreign melodic fragment). [8][9]
- Trip-hop surprise: once every 16–32 bars, a scratch-like pitch-swept noise, a tape-stop or a full drum dropout of 1–2 bars. (Rate is a design choice.) [22][24]
- Lo-fi surprise: a vinyl-stop or filter dip at the end of every 8th or 16th bar. (Design choice, after the vinyl aesthetic.) [29]
- Bonobo mode: don't quantize; make every second bar's drums a slightly mutated copy (velocity ±10%, timing ±10 ms). [36]

---

## References

1. Eno, Brian. "Generative Music." Talk at the Imagination Conference, San Francisco, 8 June 1996. *In Motion Magazine*, 7 July 1996. https://www.inmotionmagazine.com/eno1.html
2. Eno, Brian. "Ambient Music." Liner notes, *Ambient 1: Music for Airports*, PVC 7908 (AMB 001), September 1978. http://music.hyperreal.org/artists/brian_eno/MFA-txt.html
3. Eno, Brian. Liner notes, *Discreet Music*, Obscure/EG Records, 1975. http://music.hyperreal.org/artists/brian_eno/discreet-txt.html
4. Reverb Machine. "How Brian Eno Created Ambient 1: Music for Airports." 11 July 2019. https://reverbmachine.com/blog/deconstructing-brian-eno-music-for-airports/
5. Reverb Machine. "How Brian Eno Created Discreet Music." 3 September 2019. https://reverbmachine.com/blog/deconstructing-brian-enos-discreet-music
6. Wikipedia. "Ambient 1: Music for Airports." Citing Tannenbaum, R., "John Cage & Brian Eno," *Musician*, 1985; Barnes, M., *Mojo*, March 2009; Hewett, I., *The Telegraph*, 5 Jan 2016. https://en.wikipedia.org/wiki/Ambient_1:_Music_for_Airports
7. Wikipedia. "Ambient music." Citing Prendergast, M., *The Ambient Century*, Bloomsbury, 2003, among others. https://en.wikipedia.org/wiki/Ambient_music
8. Wikipedia. "Ambient house." Citing Bush, J., "Ambient House," AllMusic (https://www.allmusic.com/style/ma0000012031), and Albiez, S., *Bloomsbury Encyclopedia of Popular Music of the World*, Vol. 11, 2017, p. 26. https://en.wikipedia.org/wiki/Ambient_house
9. Wikipedia. "Chill Out (KLF album)." Citing Sherburne, P., *Pitchfork*, 16 Feb 2020; Bush, J., AllMusic. https://en.wikipedia.org/wiki/Chill_Out_(KLF_album)
10. Wikipedia. "Little Fluffy Clouds." Citing Simpson, D., "How we made the Orb's Little Fluffy Clouds," *The Guardian*, 7 June 2016. https://en.wikipedia.org/wiki/Little_Fluffy_Clouds
11. Wikipedia. "Selected Ambient Works 85–92." Citing Tognazzini, A., AllMusic; Carr, D., MusicRadar, 2022. https://en.wikipedia.org/wiki/Selected_Ambient_Works_85–92
12. Wikipedia. "Selected Ambient Works Volume II." Citing Richardson, M., *Pitchfork*, 2014; Toop, D., *The Face*, March 1994. https://en.wikipedia.org/wiki/Selected_Ambient_Works_Volume_II
13. Wikipedia. "76:14." Citing Burgess, J., *The Guardian*, 16 Sept 2005; Bush, J., AllMusic. https://en.wikipedia.org/wiki/76:14
14. Wikipedia. "Music Has the Right to Children." Citing Passett, R., "A Bunker Full of Memories," *Forcefield*, 1998; Richardson, M., *Pitchfork*, 2004. https://en.wikipedia.org/wiki/Music_Has_the_Right_to_Children
15. FACT. "Boards of Canada reveal studio processes behind Tomorrow's Harvest." 12 June 2013 (reporting the duo's email interview with *The New York Times*). https://www.factmag.com/2013/06/12/boards-of-canada-reveal-studio-processes-behind-tomorrows-harvest-hint-at-collection-of-early-material/
16. Wikipedia. "Ambient techno." Citing Reynolds, S., *Energy Flash*, 2012; AllMusic "Ambient Techno". https://en.wikipedia.org/wiki/Ambient_techno
17. Wikipedia. "Biosphere (musician)." Citing Thompson, D., *Alternative Rock*, 2000; Fischer, T., *Tokafi*, 2011. https://en.wikipedia.org/wiki/Biosphere_(musician)
18. Wikipedia. "Autechre." Citing Tingen, P., "Autechre: Recording Electronica," *Sound on Sound*, April 2004; Daniel, D., *Pitchfork*, 9 April 2005. https://en.wikipedia.org/wiki/Autechre
19. Wikipedia. "Intelligent dance music." Citing Papavassiliou, A., *Intersections* 30(2), 2010; Ramsay, B., *eContact!* 14(4), 2013; Cardew, B., *The Guardian*, 2017. https://en.wikipedia.org/wiki/Intelligent_dance_music
20. Wikipedia. "Generative music." Citing Eno 1996 [1]; Collins, N., "The analysis of generative music programs," *Organised Sound* 13(3), 2008. https://en.wikipedia.org/wiki/Generative_music
21. Wikipedia. "Reflection (Brian Eno album)." Citing Hilton, R., NPR, 15 Nov 2016; Empire, K., *The Guardian*, 1 Jan 2017. https://en.wikipedia.org/wiki/Reflection_(Brian_Eno_album)
22. Wikipedia. "Trip hop." Citing Pemberton, A., "Trip Hop," *Mixmag*, June 1994; Reynolds, S., *NYT*, 28 May 1995; Wheaton, R.J., *Portishead's Dummy*, Continuum, 2001. https://en.wikipedia.org/wiki/Trip_hop
23. Wikipedia. "Blue Lines." Citing Reynolds, S., *Generation Ecstasy*, Little, Brown, 1998, p. 321. https://en.wikipedia.org/wiki/Blue_Lines
24. Wikipedia. "Dummy (album)." Citing Weingarten, C., *The New York Times*, 21 Aug 2019 (https://www.nytimes.com/2019/08/21/arts/music/portishead-dummy-anniversary.html); *Pitchfork*, 28 Sep 2022. https://en.wikipedia.org/wiki/Dummy_(album)
25. Wikipedia. "Mezzanine (album)." Citing Humberstone, N., *Sound on Sound*, April 2003. https://en.wikipedia.org/wiki/Mezzanine_(album)
26. Mr Hoodee (The Groove Criminals). "Chillout: Raw strip-hop." *Attack Magazine*, 26 June 2012. https://www.attackmagazine.com/technique/beat-dissected/chillout-beat-raw-strip-hop/
27. Mr Hoodee (The Groove Criminals). "Chillout: Dusty Hip Hop." *Attack Magazine*, 2 October 2012. https://www.attackmagazine.com/technique/beat-dissected/chillout-dusty-hip-hop/
28. Scarth, Greg & Linn, Roger. "Roger Linn on Swing, Groove & the Magic of the MPC's Timing." *Attack Magazine*, 2 July 2013. https://www.attackmagazine.com/features/interview/roger-linn-swing-groove-magic-mpc-timing/
29. Winston, Emma & Saywood, Laurence. "Beats to Relax/Study To: Contradiction and Paradox in Lofi Hip Hop." *IASPM Journal* 9(2), 2019. https://iaspmjournal.net/index.php/IASPM_Journal/article/view/949
30. Wikipedia. "Lo-fi hip hop." Citing Sherburne, P., *Pitchfork*, 7 Oct 2021; Winkie, L., *Vice*, 13 July 2018. https://en.wikipedia.org/wiki/Lo-fi_hip_hop
31. Wikipedia. "Downtempo." Citing AllMusic "Downtempo"; Cooper, S., "Nightmares on Wax" biography, AllMusic. https://en.wikipedia.org/wiki/Downtempo
32. Wikipedia. "Chill-out music." Citing Snoman, R., *Dance Music Manual*, Taylor & Francis, 2013; Reynolds, S., *Energy Flash*, 2012. https://en.wikipedia.org/wiki/Chill-out_music
33. Wikipedia. "Moon Safari." Citing Simpson, D., "How we made Air's Moon Safari," *The Guardian*, 31 May 2016. https://en.wikipedia.org/wiki/Moon_Safari
34. Wikipedia. "Balearic beat." Citing Anderson, J., "The Birth of Balearic," *i-D*, 29 May 2014. https://en.wikipedia.org/wiki/Balearic_beat
35. Wikipedia. "Café del Mar." Citing Hunter-Tilney, L., *Financial Times*, 23 Oct 2020. https://en.wikipedia.org/wiki/Café_del_Mar
36. Ableton. "Bonobo: The Path to Migration." Ableton blog, 31 January 2017. https://www.ableton.com/en/blog/bonobo-path-to-migration/
37. SongBPM (automated tempo/key database), accessed 19 Oct 2026:
    - Massive Attack, "Teardrop": https://songbpm.com/@massive-attack/teardrop
    - Portishead, "Sour Times": https://songbpm.com/@portishead/sour-times
    - Bonobo, "Kiara": https://songbpm.com/@bonobo/kiara
    - The Orb, "Little Fluffy Clouds": https://songbpm.com/@the-orb/little-fluffy-clouds
    - Boards of Canada, "Roygbiv": https://songbpm.com/@boards-of-canada/roygbiv
    - Aphex Twin, "Xtal": https://songbpm.com/@aphex-twin/xtal
    - Burial, "Archangel": https://songbpm.com/@burial/archangel
38. Wikipedia. "Dubstep." Citing Sande, K., "Dubstep 101," *Resident Advisor*, 2007; Mugan, C., *The Independent*, 28 July 2006; Reynolds, S., *Energy Flash*, 2012. https://en.wikipedia.org/wiki/Dubstep
39. Jenkins, Peter. "Dubstep Basics." *Sound on Sound*, July 2010. https://www.soundonsound.com/techniques/dubstep-basics
40. Wikipedia. "Untrue (album)." Citing Hancox, D., *The Guardian*, 26 Oct 2007; Fisher, M., "Burial: Unedited transcript," *The Wire*, 2012. https://en.wikipedia.org/wiki/Untrue_(album)
41. Wikipedia. "Future garage." Citing *MusicTech* production feature, 2012/2013. https://en.wikipedia.org/wiki/Future_garage
42. Wikipedia. "Footwork (genre)." Citing Quam, D., *Bangs & Works Vol. 1* liner notes, Planet Mu, 2010; "How Footwork Began: An Interview With DJ Clent," *The Quietus*, 16 May 2017. https://en.wikipedia.org/wiki/Footwork_(genre)
43. Wikipedia. "Hardstyle." Citing Q-dance, "Hardstyle"; Bevacqua, J., *LA Weekly*, 16 Dec 2015. https://en.wikipedia.org/wiki/Hardstyle
44. Wikipedia. "Big room house." Citing Rovito, M., "Layering Synths for Big-Room EDM Tracks," *Electronic Musician* 30(12), 2014. https://en.wikipedia.org/wiki/Big_room_house
45. Wikipedia. "Future bass." Citing Turner, D., *Rolling Stone*, 14 Feb 2017; Garber, D., *Thump*, 19 Nov 2015. https://en.wikipedia.org/wiki/Future_bass
46. Wikipedia. "Vaporwave." Citing Harper, A., "Vaporwave and the pop-art of the virtual plaza," *Dummy*, 7 Dec 2012. https://en.wikipedia.org/wiki/Vaporwave
47. Wikipedia. "Amapiano." https://en.wikipedia.org/wiki/Amapiano
48. Kriger, Themba. "Amapiano History: South Africa's Unique Music Genre." *Red Bull*, 25 April 2024. https://www.redbull.com/za-en/amapiano-history
49. Wikipedia. "Afro house." Citing Guttridge-Hewitt, M., "Gauteng style: a history of house music in South Africa," *DJ Mag*, 23 Feb 2023. https://en.wikipedia.org/wiki/Afro_house
50. Wikipedia. "Discreet Music." Citing O'Brien 2016 and Carr 2019. https://en.wikipedia.org/wiki/Discreet_Music
51. Parviainen, Tero. "How Generative Music Works: A Perspective" (interactive essay). https://teropa.info/loop/
