# Sound design: the signature machines of electronic dance music, and how to synthesise them

Research notes for the music engine's sound-design presets. Every factual claim
cites the numbered references at the end. Values the sources do not give, which
are design choices for this engine, are marked **(engine default)**. Values I
could not verify are marked **(unverified)**. Where a number is my own
arithmetic on a sourced value (for example, cents from a measured frequency
ratio, or an RC time constant from component values), it is marked
**(derived)**.

The engine code read for the comparison in section 6 is `src/instruments.rs`,
`src/kits.rs`, `src/dsp.rs` and `src/engine.rs` in this crate.

---

## 1. Drum machines

### 1.1 Roland TR-808 (1980–83, about 12,000 built) [8]

**Bass drum.** The 808 kick has no oscillator in the usual sense: a
*bridged-T network* rings when a trigger hits it and decays to silence on its
own. Positive feedback sets how long it rings; with too much feedback it never
stops and becomes an oscillator [1]. Werner, Abel and Smith's circuit analysis
gives the details that matter for synthesis [2]:

- The trigger logic makes a **1 ms pulse**. A pulse shaper passes its high
  frequencies and "shaves off" the falling edge, and the result strikes the
  bridged-T [2].
- The network's resting centre frequency is **≈ 49.5 Hz**, close to Roland's
  "typical" tuning chart value of **56 Hz** [2].
- **Attack pitch jump:** for about **6 ms** after the trigger an envelope
  transistor grounds part of the network. That raises its centre frequency
  "by more than an octave" and raises its Q. Six milliseconds is less than one
  period at the higher frequency, so it isn't heard as pitch. It makes the
  attack "punchier" and "crisper" [2].
- **Retrigger pulse:** when the centre frequency drops back, most of the
  energy at the resting frequency has already gone. A second, smaller pulse
  re-excites the network so the amplitude does not jump [2].
- **Pitch sigh:** a slower, subtle downward drift over the note's first few
  hundred milliseconds, caused by leakage through R161. It is a separate
  effect from the attack jump, and the two are often conflated [2]. Reid hears
  the same drift: at long decays the drum goes slightly flat [1].
- The **decay knob** sets the feedback buffer's high-shelf gain: the less it
  attenuates, the longer the note [2]. The **tone** stage is a passive
  low-pass filter, and accent raises the strike pulse and with it the click [1][2].
- There is no noise generator and no contour generator in the 808 kick [1].

Hip-hop producers made the long-decay, retuned 808 kick into a bassline (Rick
Rubin is credited with popularising this), and it is the bedrock of trap [8].

**Snare.** Two bridged-T resonators, their pitches and decays fixed by
component values, mixed in fixed proportion. The trigger also fires a contour
generator whose "snappy" output gates high-passed white noise, which is mixed
with the tones [4].

**Hats, cymbal, cowbell.** All four come from one bank of **six Schmitt-trigger
square-wave oscillators** (one HD14584 chip) at **205.3, 304.4, 369.6, 522.7,
540 and 800 Hz** (the last two factory-trimmed). Their duty cycle is 47.98 %,
not 50 % [3]. For the cymbal, two band-pass filters at about **3440 Hz** and
**7100 Hz** split the summed squares into bands with their own envelopes and
"swing-type" VCAs. Each band is then high-passed, and the output stage has a
resonant corner near **10.5 kHz**. The tone knob mainly changes the level of
the third band [3]. Reid gives the band structure: the low band has an AR
envelope set by the decay knob, and the upper band splits into a
shortest-decay path and a mid-decay path [5]. The cowbell uses two of the six
oscillators [3].

**Clap.** I found no circuit-level primary source. The usual description is
band-passed white noise through one VCA with two envelopes: a repeating
sawtooth for the multi-clapper flam, then a longer decay for the "reverb" tail
**(unverified)**.

### 1.2 Roland TR-909 (1983, about 10,000 built) [7]

The 909 is a hybrid. **Kick, snare, toms, rim and clap are analogue. Hats,
ride and crash are 6-bit samples** [6][7]. Atsushi Hoshiai sampled his own
cymbals: a 14" Paiste Sound Edge top over a 14" A. Zildjian New Beat bottom
for the hat, and one 18" Paiste Thin Crash, hit on the edge for the crash and
on the bow for the ride. He recorded through a Sony C-38 microphone to a
Pioneer reel-to-reel, with no EQ or compression [6]. In playback, a 30 kHz
clock steps an address counter through the 6-bit table, and tuning changes
that clock. A second DAC drives an anti-log VCA envelope that tracks the
counter, and a low-pass after the VCA reduces quantisation noise and aliasing [5].

**909 kick** [1]: a **sawtooth VCO** whose pitch follows EG3 (instant attack,
slow decay), with a **waveshaper** that removes most overtones to leave a
near-sine, then a VCA on EG1. A second path mixes **low-passed noise with a
short pulse** (the click) through its own VCA and EG2. Accent changes EG1's
attack and decay. The 909 panel has attack (click) and decay controls [7].

**909 snare** [4]: two oscillators with waveshapers give only the lowest drum
modes, with separate VCAs and envelopes so the two partials decay at different
rates. Noise is low-passed first, then split. One path is high-passed into a
narrow band with its own VCA and envelope; the other goes through a VCA
unfiltered. A last VCA shapes the sum.

### 1.3 TR-707 / TR-727, LinnDrum

- **TR-707** (1985): PCM samples, **8-bit**, with **crash and ride at 6-bit**
  [37]. It was used in early house, for example Marshall Jefferson's "Move
  Your Body" [37].
- **TR-727** (1985): the same engine, with 15 Latin percussion sounds. Roland
  gives **25 kHz / 8-bit** playback, some tones 6-bit. The PCM waveforms do not
  decay; analogue envelope circuits after the DAC add the decay and mask the
  quantisation noise in the tail [38].
- **Linn LM-1** (1980): twelve **8-bit companded** samples at **28 kHz**, each
  tunable [39]. **LinnDrum**: fifteen sounds, which a museum catalogue gives as
  **35 kHz / 8-bit** [40]. Other sources quote about 27 kHz for the family, so
  the LinnDrum rate is **(unverified)**.

To synthesise these machines without samples, the engine needs a
*sample-character stage*: requantise to 6–8 bits at 25–35 kHz with no
anti-alias filtering, then an analogue-style decay envelope after the "DAC".

---

## 2. Bass machines

### 2.1 Roland TB-303 (1981–84) [12]

Robin Whittle's Devil Fish manual measures the stock 303 [9]:

| Parameter | Stock TB-303 |
|---|---|
| Slide | **60 ms**, fixed |
| Gate to sound | **1–5 ms delay (typically 4 ms)**, then a **3 ms** attack |
| VEG (amp) decay | fixed, **≈ 3–4 s** (so a held note barely droops) |
| End of an unaccented note | **≈ 16 ms**: 8 ms at full level, then 8 ms linear fade |
| MEG (filter env) decay, normal notes | **200 ms – 2 s** (the Decay knob) |
| MEG decay, accented notes | **fixed 200 ms** |
| Max filter frequency with no env mod | an octave below the Devil Fish's ≈ 5 kHz, i.e. **≈ 2.5 kHz** |
| Bass response | **32 Hz is down 5 dB** |
| Env Mod | cannot be turned fully to zero |
| Key tracking of the filter | none (the Devil Fish adds it) |

**Accent** does two things [10]. (1) The MEG voltage reaches the VCA through
47 kΩ and 0.033 µF (τ ≈ 1.6 ms, **derived**), which softens the attack and
makes the note louder. (2) The *accent sweep circuit*: a diode and 47 kΩ
charge a **1 µF** capacitor through the Resonance pot's second gang (100 kΩ),
and the wiper feeds the filter frequency. Charge τ ≈ 147 ms and discharge
τ ≈ 100 ms at the clockwise end (**derived** from those values). A single
accent gives a smooth rise and fall in cutoff. Accents in quick succession
stack higher because the capacitor has not discharged: this is the "wow", and
it grows with resonance [10]. Whittle calls it the distinctive "acidee wow"
[9].

**Filter: 18 or 24 dB?** Roland and EMS sold their diode ladders as "18 dB,
3-pole". Tim Stinchcombe shows the 303 core is a **4-pole ladder with a
24 dB/oct asymptote** (SPICE: −24.1 dB/oct at the fourth stage). Its roll-off
near cutoff is gentler because the stages load each other and are not
buffered, which is probably where the "18 dB" comes from [11]. Halving the
bottom capacitor shifts the whole response up by 2^0.25 (≈ 1.19×), not one
pole by an octave. Coupling capacitors add about five poles near **10 Hz**,
mostly inside the feedback loop, which gives a second, low resonant peak [11].
An open-source emulation that measured a real unit reports a high-pass in the
resonance feedback loop [41]. The 303 is a 24 dB, non-self-oscillating
low-pass in Roland's own spec [12]. The oscillator is saw or square [12].
Phuture's "Acid Tracks" (1987) came from twisting the knobs while it played
[12].

### 2.2 Minimoog

Three VCOs (oscillator 3 can double as an LFO), white/pink noise, a 4-pole
**24 dB/oct ladder** with key tracking and its own contour, low-note
priority, and a pitch wheel. It works as both a lead and a "propulsive bass"
instrument [19]. A two- or three-oscillator Moog bass in the engine's `mono`
is architecturally right.

### 2.3 The Reese

Kevin Saunderson's "Just Want Another Chance" (as Reese, 1988) was made on a
**Casio CZ** phase-distortion synth. Renegade's "Terrorist" (1994) sampled it
for jungle, and "Reese" now means any sustained, full-range, beating bass [25].
Attack's minimal recipe: **two sines at +27 and −27 cents**, mono, sustain
1.0, release 24 ms. Changing either fine-tune changes the beat rate [25].
MusicRadar's version uses two oscillators at **±55 cents** [42]. The
characteristic beating is the difference frequency: at A1 (55 Hz), ±27 cents
gives about **1.7 Hz** (**derived**).

### 2.4 Wobble (dubstep)

A low-pass filter whose cutoff is swept by a **tempo-synced LFO**. MusicRadar
builds it with a triangle LFO at **1/8 triplet** into two cutoff inputs, with
an LFO delay so the wobble fades in [43]. At 140 BPM, 1/8T is 7 Hz and 1/8 is
4.67 Hz (**derived**: rate = BPM/60 × divisions per beat).

### 2.5 Korg M1 organ bass and piano (1988)

The M1 is sample-plus-synthesis: 4 MB of 16-bit PCM, 100 multisounds and 44
drums, 16 voices, about 250,000 sold [20]. Its piano and organ presets made
1990s house: Madonna's "Vogue" and Robin S's "Show Me Love" (1990) [20].
MusicRadar identifies **"Organ 2"** (preset 17) as the "Show Me Love" bass
organ and **"Piano 16'"** (patch 01) as the house piano. Beyoncé's "Break My
Soul" reuses both [21][22]. Since these are samples, the engine has to
approximate them: an additive organ (drawbar-like partials) with a fast
percussive decay for "Organ 2", and FM or modal synthesis for the piano
**(engine default)**.

---

## 3. Polysynths and leads

### 3.1 Roland Juno-60 / Juno-106

The Juno-106 has six voices, one DCO each, with saw, pulse (PWM), sub-square
and noise. It has a **24 dB resonant low-pass** on Roland's 80017A module
(IR3109 filter + BA662 VCA), a **non-resonant high-pass**, one triangle LFO
with delay, one ADSR, and the chorus [16][44]. Its classic dance users include
Leftfield, Underworld, Reel 2 Real, Daft Punk and Calvin Harris [16].

**Chorus.** The chorus is a BBD (MN3009) [15]. The best numeric description I
found is the circuit-informed KR-106 emulation [44]:

| Mode | LFO (triangle) | Delay swing | Centre delay |
|---|---|---|---|
| I | **0.513 Hz** | **±0.5 ms** | 3.5 ms |
| II | **0.863 Hz** | **±1.1 ms** | 3.5 ms |
| I+II (106) | both rates on separate taps | both depths | 3.5 ms |

It also uses 15 kHz pre/post low-pass filters, 256 BBD stages and a
0.4 dry / 0.6 wet mix [44]. The I+II mode differs by model. SoundForce's
clone, whose driver waveforms were sampled from a Juno-106, gives the
**Juno-60's** I+II as **1 Hz at 8 % depth** and the **Juno-6's** as
**8 Hz at 25 %**: a fast, shallow vibrato-chorus [15].

**HPF switch** (per KR-106, **(unverified)** against the service manual):
position 0 is a **+10 dB low-shelf boost below 150 Hz**, 1 is flat, 2 is a
1-pole HP at **240 Hz**, and 3 is a 1-pole HP at **720 Hz** [44]. The same
model gives per-voice pitch variance of ±3 cents and cutoff variance of ±5 %
[44].

### 3.2 Roland Jupiter-8 and Sequential Prophet-5

- **Jupiter-8:** 8 voices, 2 VCOs per voice with **cross-mod and sync**, an
  IR3109 resonant LPF switchable **12 or 24 dB/oct**, a non-resonant HPF, two
  envelopes (IR3R01, about 1 ms attack), a 4-wave LFO, polyphonic unison with
  all 16 oscillators on one note, and an arpeggiator. Giorgio Moroder and
  Vince Clarke used it [17].
- **Prophet-5:** 5 voices, 2 VCOs per voice, a 4-pole resonant LPF, and
  **Poly-Mod** (analogue FM routings). It used SSM chips in Rev 1–2 and Curtis
  CEM chips from Rev 3, which Reid finds "colder" [18].

What these add over a Juno is **oscillator sync and cross-modulation** (the
sync-sweep lead) and a **12 dB filter mode**.

### 3.3 Roland JP-8000 supersaw (1996/97) [14]

Adam Szabo measured the JP-8000 Super Saw [13]:

- **Seven saws**: a centre oscillator plus three below and three above. At
  maximum detune they sit at these ratios to the centre: **0.88998, 0.93712,
  0.98048, 1, 1.01991, 1.06217, 1.10745** [13]. In cents (**derived**):
  **−202, −112, −34, 0, +34, +104, +177**. The spacing is not symmetric, and
  the outer pair is about 5.6× the inner pair.
- **The detune knob is non-linear.** Szabo fits an 11th-degree polynomial to
  sampled points. The fraction of maximum spread is **0.097 at knob 0.50,
  0.243 at 0.75, 0.393 at 0.94, and 1.0 at 1.0** [13]. Cents per knob position
  (**derived**):

  | Knob (0–1) | Inner pair | Middle pair | Outer pair |
  |---|---|---|---|
  | 0.25 | ±1.6 | ±5.1 | ±8.6 |
  | 0.50 | ±3.3 | ±10.5 | ±18 |
  | 0.75 | ±8.2 | ±26 | ±46 |
  | 0.94 | ±13 | ±43 | ±74 |
  | 1.00 | ±34 | −112/+104 | −202/+177 |

- **The mix knob** fades the centre down and the sides up. The centre is
  `−0.55366·x + 0.99785` and the six sides are
  `−0.73764·x² + 1.2841·x + 0.044372`, which are equal at x ≈ 0.75 [13].
- The summed saws pass through a **high-pass that tracks the fundamental**,
  which removes aliasing below the note and shapes the wave [13].
- Oscillators are **free-running**: each note starts with random phases [13].
- The JP-8000 has 8 voices and a 12/24 dB LP/HP/BP filter. The Super Saw led
  "anthem" trance, and the JP-8080 is on Darude's "Sandstorm" [14].

### 3.4 Hoover (Alpha Juno "What the")

Eric Persing made the "What the" patch for the Roland Alpha Juno-1/-2 and
MKS-50. It is a **PWM sawtooth**, which on these Rolands inserts flat
segments of variable width into a saw, with **heavy PWM**, a **thick chorus**,
and a **fast LFO** on both the PWM and the chorus. It is built from three
oscillators an octave apart [26]. It is believed to have first appeared on a
record in Second Phase's "Mentasm" (Joey Beltram and Mundo Muzique, 1991). It
spread through Belgian techno, and Human Resource's "Dominator" (1991) was a
top-10 hit [26]. The downward pitch bend or portamento usually heard on
hoover riffs is **(unverified)** in these sources.

### 3.5 Yamaha DX7 (FM)

These are decoded from the factory ROM1A bank (32 voices) [23]. Levels and
rates are on DX7 0–99 scales, and "vel" is key-velocity sensitivity 0–7.

| Patch | Algorithm | Carriers | Modulator → carrier ratios | Notes |
|---|---|---|---|---|
| **E.PIANO 1** | 5 (three 2-op pairs), FB 6 | op1, op3, op5 at 1.00 | op2 **14.00** (out 58, vel 7) → op1; op4 1.00 (out 89, vel 6) → op3; op6 1.00 (out 79, vel 6) → op5 | op5 and op6 detuned −7/+7 (chorus beating); all ops attack rate 95; carriers decay to 0 |
| **BASS 1** | 16, FB 7 | op1 at **0.50** | op2 0.50, op3 0.50, op4 5.00, op5 0.50, op6 9.00 | high-ratio mods (5, 9) are velocity-sensitive (vel 5 and 7), decaying fast: the "pluck" |
| **TUB BELLS** | 5, FB 7 | op1 1.00, op3 1.00, op5 **fixed 323.6 Hz** | op2 **3.50** → op1, op4 **3.50** → op3, op6 2.00 → op5 | inharmonic 3.5:1 pairs give the bell; slow carrier decays |
| **E.ORGAN 1** | 32 (all six carriers), FB 0 | 0.50, 1.01, 1.50, 0.50, 1.00, 3.00 | none (additive) | drawbar-like partials, slight detune |

Paul Colbert's 1983 review gives the same reading of E.PIANO 1: the 14:1 pair
is the tine "tinkle", and the 1:1 pairs make the body. He also notes the
"Orchestra and Chimes" bells pair carrier and modulator at **3.18** [24].

---

## 4. Effects, stabs and transitions

**Stabs.** A rave or techno chord stab keeps **one chord shape** and moves it
as a block. Chord-memory or "Chord" MIDI tools trigger the whole shape from
one root, typically a **minor 7th** [28]. Attack's techno chapter adds that
stabs are often parallel minor triads, cut to **one 16th** and placed **off
the downbeats**, with filter cutoff, reverb and auto-pan automated live [27].

**Risers and noise sweeps.** White noise into a **low-pass with a small
resonance bump**, cutoff automated up [45]. Alternatives are band-passed noise
with some resonance, centre swept up over **7 of 8 bars** [46], or a high-pass
starting near **100 Hz** that rises to strip the lows; an 8-bar riser at
128 BPM is a common unit [47].

**Gated reverb.** A strong reverb, cut by a noise gate keyed from the dry
drum. In the live-room method the gate holds about **0.5 s** and then releases
fast [33]. Padgham and Collins found it on Peter Gabriel's "Intruder" (1979)
through the SSL talkback mic's compressor and gate. AMS turned it into the
RMX16 **NonLin2** algorithm, which is the version on most 1980s records [33].

**Delay.** The engine's ping-pong defaults to 0.75 beats, a dotted 8th. I
found no authoritative source pinning dotted-8th delay to a genre (trance
production lore), so that pairing is **(unverified)**.

---

## 5. The mix and master

**Sidechain ducking.** Attack Magazine [29]: use the fastest attack (watch
for clicks); set release by the kick's decay and the BPM so the duck recovers
in time with the groove; **up to 6 dB** of gain reduction from a kick key
before pumping becomes audible. Deliberate "pumping" uses more. A dedicated
silent click as the key allows faster releases. Starting points from another
guide: attack under 10 ms, release 20–75 ms for subtle ducking, tempo-timed
releases for obvious pumping [30].

**Bus "glue" compression (SSL-type).** Ratio steps of **2:1** (gentle soft
knee), **4:1** and **10:1** (limiter-like) [32]. Waves' starting point is
attack **10–30 ms** (30 ms common), 2:1, threshold set for **1–4 dB** of gain
reduction, release 0.1 s or auto [31].

**Loudness.** Spotify normalises to **−14 LUFS** integrated (Loud −11, Quiet
−19). It turns louder tracks down, and turns quieter tracks up only as far as
1 dB of headroom allows. It recommends **≤ −1 dBTP**, or **≤ −2 dBTP** for
masters louder than −14 LUFS [34]. Club masters are louder because clubs do
not normalise. Mixgraph's measurement of 900 released tracks (the loud
2-minute preview window) gives a median of **−8.8 LUFS**, with these genre
medians [35]:

| Genre | Median LUFS | LRA (LU) |
|---|---|---|
| Dubstep | −6.0 | 6.0 |
| Drum & bass | −6.5 | 6.8 |
| Hard techno | −7.3 | 4.5 |
| Mainstage / big room | −7.4 | 5.4 |
| Trance | −7.5 | 4.8 |
| Psytrance | −7.9 | 4.9 |
| Breaks / UK bass | −8.1 | 4.8 |
| Techno (peak time) | −8.6 | 5.6 |
| Techno (deep / hypnotic) | −8.7 | 3.2 |
| Tech house | −8.9 | 4.6 |
| House | −9.2 | 4.8 |
| Progressive house | −10.1 | 5.4 |
| Deep house | −10.3 | 4.3 |
| Melodic house & techno | −10.5 | 5.4 |

**Frequency allocation.** The 808 kick's resting resonance is about
**50–56 Hz** [2]. The kick's pitch sets the key of the bassline. When the bass
carries the sub, trim the kick's sub tail (Attack shortens its sustain with a
transient shaper) [36]. I found no authoritative numbers for per-instrument
high-pass corners; the values in the rules are **(engine default)**.

---

## 6. The engine against the sources

| Engine component | Matches the sources | Missing or likely inaccurate |
|---|---|---|
| **808 kick** (`kick808`: tune 49 Hz, sweep ×1.7, τ 14 ms, decay 0.75 s, click) | Tune 49 Hz matches the circuit's 49.5 Hz [2]; no noise, matches [1] | The circuit jumps **more than an octave for about 6 ms**, then drops back almost as a step, and then has a slow, subtle *pitch sigh* [2]. An exponential ×1.7 sweep with τ = 14 ms merges the two. Use a ×2.0–2.2 jump that is abrupt at 6 ms, plus a separate small, slow sag of a few percent over 200–300 ms (the size is **(unverified)**; Werner's Fig. 11 plots it). Decay should lower the pitch slightly at long settings [1]. |
| **909 kick** (sine + tanh, sweep ×3.6, τ 9 ms, noise click) | Waveshaped oscillator plus a noise-and-pulse click path [1] | The real one is a waveshaped **saw**, and the click path is **low-passed noise plus a pulse**, not band-passed noise [1]. Minor. |
| **808 hats, cymbal, cowbell** (6 squares at 205.3–800 Hz) | Frequencies match the circuit exactly [3] | Duty cycle 47.98 % [3] (trivial). The cymbal should be **three bands** (BP ≈ 3.44 kHz and ≈ 7.1 kHz, each high-passed, with three envelopes: long, mid, shortest) [3][5]; the engine has two. |
| **909 hats, ride, crash** (same `Metal` 6-square model as the 808) | none | **Inaccurate**: the 909's are **6-bit samples of real cymbals** [6][7]. A square-cluster hat sounds like an 808. Approximate with a denser inharmonic source (many partials or filtered noise), requantised to 6 bits at a 30 kHz clock, with a low-pass after the VCA [5]. |
| **Snares** (two sines + filtered noise) | Two tonal partials plus noise, 909 with a pitch bend [4] | The 909's noise should be split: one high-passed narrow band and one unfiltered path, each with its own envelope [4]. |
| **Clap** (3–4 bursts, BP noise, tail) | Matches the common description | Circuit unverified. |
| **TB-303** | Slide 60 ms ✓, accented MEG 200 ms ✓, 8 ms end fade ✓, accent capacitor "wow" ✓ [9][10] | (1) The filter is a **4-pole diode ladder, 24 dB asymptotic** with a soft knee [11]; an "18 dB tap mix" is a folk reading. (2) No high-pass coupling in the resonance loop (poles near 10 Hz) [11][41]. (3) The engine's accent capacitor charges with τ ≈ 24 ms at 48 kHz (**derived** from 0.014 per 16 samples). The circuit's charge τ is ≈ 147 ms at full resonance (**derived**) [10], so the engine's wow may be too fast and peaky. (4) The default normal decay of 0.4 s is in range (0.2–2 s) [9]. (5) Missing: the 4 ms gate-to-sound delay and 3 ms attack [9]. (6) The base cutoff should top out near 2.5 kHz before env mod [9]. |
| **Juno poly + chorus** | I 0.513 Hz, II 0.863 Hz ✓ [44] | (1) Both modes sweep **±1.7 ms**; the model gives **±0.5 ms (I)** and **±1.1 ms (II)** [44], so the engine's chorus I is about 3× too deep. (2) I+II (9.75 Hz, 25 % depth) is the **Juno-6** behaviour; the Juno-60 is 1 Hz at 8 % [15]. Offer both. (3) Missing: the **4-position HPF** (boost / flat / 240 Hz / 720 Hz) [44]; the engine has a free `hpf` corner, so add a preset table. (4) The pre/post low-pass is 9 kHz in the engine and 15 kHz in the model [44]. |
| **Supersaw** (7 osc, spread ∝ \|u\|^1.3) | Spread shape: inner/middle/outer = 0.24/0.59/1.0 vs measured 0.18/0.57/1.0 [13], close | Missing: (1) the **mix law** (centre falls linearly while the sides rise on a parabola) [13]; (2) the **high-pass tracking the fundamental** [13]; (3) the measured **asymmetry** (−202/+177 cents at maximum) [13]; (4) random phase **per note** (the engine draws phases once per voice) [13]. |
| **Moog-style mono** | 24 dB ladder, up to 3 osc, glide [19] | Missing: **osc 3 as LFO / FM source** and low-note priority [19]. |
| **4-op FM** (EP: 2→1, 4→3; bell; stack; organ) | EP pair structure matches DX7 algorithm 5's pairs [23][24] | (1) E.PIANO 1 needs a **14:1** tine pair with velocity 7 and a 1:1 body pair with feedback [23]; check that the EP preset uses 14:1. (2) Bells need **3.5:1** pairs (or 3.18) and a **fixed-frequency** operator [23][24]; the engine has no fixed-frequency mode. (3) Missing: a bass algorithm with high-ratio (5:1, 9:1) fast-decaying modulators on a 0.5 carrier [23]. |
| **Pump** (down in 6 ms, hold to 30 ms, then release; depth 0.08–0.5) | Depth 0.5 = −6 dB, the subtle ceiling [29] | Fine. The release is a free number; it should be derived from the tempo (rules below). |
| **Glue** (thr −10 to −20 dB, ratio 1–3.5, att 12 ms, rel 180 ms) | Inside SSL-type ranges [31][32] | Attack could go to 30 ms for more punch [31]. |
| **Limiter** (sample peak, ceiling 0.95 ≈ −0.45 dBFS) | none | **No loudness target, and no true peak.** Add an integrated-LUFS meter and a per-genre target (section 5) [34][35], and a true-peak ceiling of −1 dBTP (−2 if louder than −14 LUFS) [34]. |
| **FDN reverb, ping-pong** | Dotted-8th default (0.75 beat) | Missing: a **gated reverb** (gate keyed by the dry snare, 0.5 s hold, fast release) [33]. |
| **Missing instruments** | none | **Hoover** (PWM-saw, 3 octaves, fast LFO on PWM and chorus) [26]; **wobble** (tempo-synced LFO → cutoff; the poly LFO only reaches PWM and vowel) [43]; **Reese** preset (2 detuned saws or sines in `mono`) [25]; **M1 organ bass / Piano 16'** stand-ins [20][21]; **sample-character stage** for 707/727/Linn/909 cymbals (6–8 bit, 25–35 kHz) [37][38][39][40]; **osc sync / cross-mod** (Jupiter-8, Prophet-5) [17][18]; **chord-memory stabs** (sequencer side) [27][28]. |

---

## Rules and parameters

Units: Hz, ms, cents, dB. "Knob" values are 0–1. The engine's `pump.depth` is
the linear dip, so gain reduction = 20·log10(1 − depth).

### Tempo & feel
- Sync every modulation to the bar: LFO Hz = BPM/60 × (divisions per beat): 1/4 → ×1, 1/8 → ×2, 1/8T → ×3, 1/16 → ×4. **(derived)** [43]
- Wobble at 140 BPM: 1/8T = 7.0 Hz, 1/8 = 4.67 Hz, 1/4 = 2.33 Hz; switch the division every 1–2 bars for "talking" wobbles. [43] (switching rate **(engine default)**)
- Pump release ≈ one 8th note minus the 30 ms hold, e.g. 200 ms at 128 BPM (8th = 234 ms) and 180 ms at 140 BPM, so gain is back by the off-beat. [29] (formula **(engine default)**)

### Drums
- 808 kick: rest 49–56 Hz; jump to ×2.0–2.2 for 6 ms, then drop back abruptly; add a slow sag of 1–3 % over 300 ms; no noise; click = 1 ms pulse through a low-pass. [1][2] (sag size **(unverified)**)
- 808 kick decay: 0.3–0.6 s (electro, Miami, house); 1.0–3.0 s and tuned to the bass root (trap, hip-hop "808 bass"). [8] (times **(engine default)**)
- 909 kick: waveshaped saw; pitch env from ×3–4 down to 50–60 Hz with τ 8–12 ms; amp decay 0.3–0.5 s (house) or 0.2–0.35 s (techno, with distortion); click = low-passed noise + 2 ms pulse. [1] (numbers **(engine default)**)
- 808 metallic voices: six squares at 205.3, 304.4, 369.6, 522.7, 540, 800 Hz, 48 % duty. Cymbal = BP 3440 Hz + BP 7100 Hz, three envelopes (long, mid, short), HP each, output resonance 10.5 kHz. Closed hat decay 40–60 ms, open hat 300–500 ms, closed chokes open. [3][5] (decay times **(engine default)**)
- 909 hats and cymbals: not square clusters. Use a dense inharmonic or noise source, requantised to 6 bits at a 30 kHz clock, with an LP after the VCA. Hat decay 30–80 ms closed, 250–600 ms open. [5][6] (decays **(engine default)**)
- 707/727/LinnDrum kits: 8-bit (cymbals 6-bit), 25–35 kHz sample-and-hold, the decay applied *after* quantisation. [37][38][39][40]
- 909 snare noise: LP the noise, then split it into an HP narrow band and an unfiltered path with separate decays (e.g. 180 ms and 80 ms). [4] (times **(engine default)**)
- Gated reverb on snares (80s, synth-pop, italo, electro revival): reverb of 1.5–2.5 s RT60, gated by the dry snare, hold 400–500 ms, release under 20 ms. [33] (RT60 **(engine default)**)

### Bass
- **Acid (303):** saw (square for darker lines); 24 dB diode ladder with a soft knee and a ~10 Hz HP in the feedback loop; base cutoff 200–800 Hz, max 2.5 kHz before env; res 0.6–0.95; env mod 0.4–0.9; normal decay 200–2000 ms, accent decay 200 ms; slide 60 ms; amp: 4 ms delay, 3 ms attack, hold, then 8 ms hold and 8 ms linear fade at note end. Accent cap: charge τ ≈ 150 ms, discharge τ ≈ 100 ms, cumulative. [9][10][11] (cutoff, res and env ranges **(engine default)**)
- **Reese (techno, jungle/DnB):** two saws (or sines for the minimal version) at ±27 cents (subtle) to ±55 cents (heavy), mono, sustain 1.0, release 24 ms; LP 24 dB at 300–1200 Hz; optional drive. [25][42] (filter **(engine default)**)
- **Wobble (dubstep):** saw + square one octave down, LP 24 dB, base cutoff 150–300 Hz, LFO depth +3 to +5 octaves, res 0.3–0.5, triangle or sine LFO synced at 1/8T or 1/8, 0.5–1 beat LFO delay at note start. [43] (values **(engine default)**)
- **Moog bass (disco, Italo, electro):** 2–3 saws at 0, −5 and +5 cents (or one an octave down), 24 dB ladder at 250–600 Hz, filter env decay 150–300 ms at +2 to +3 octaves, res 0.2–0.4, glide 0–40 ms, low-note priority. [19] (values **(engine default)**)
- **Juno bass (deep house):** saw + sub-square at 0.5–0.8, HPF position 0 (the +10 dB boost below 150 Hz), LP at 400–900 Hz, res 0.1–0.3, decay 200–400 ms, chorus off. [44] (values except the HPF **(engine default)**)
- **M1 "Organ 2" bass (90s house, garage):** additive organ at 16', 8', 5⅓' and 4' (ratios 0.5, 1, 1.5, 2 to the played note, the way DX7 E.ORGAN 1 uses 0.5/1/1.5/3), percussive decay 150–250 ms to sustain 0.3, key click. [20][21][23] (recipe **(engine default)**)
- **DX bass:** carrier 0.5; modulators 0.5 (FB 7), 5.0 and 9.0, fast-decaying and velocity-sensitive. [23]
- Kick and bass: tune the kick to the bass root (or its 5th); when the bass carries the sub, shorten the kick's sub tail to 150–250 ms; keep the bass mono below 120 Hz. [36] (ms and Hz **(engine default)**)

### Harmony
- Stabs: one fixed voicing (minor triad, minor 7th, minor 9th), moved in parallel; 1/16-note long; on off-16ths; cutoff, reverb and pan automated over 4–8 bars. [27][28]
- Stab patch (Detroit/techno): 2 saws ±7 cents + square, LP 24 dB at 1.5–3 kHz, filter env decay 120–200 ms, amp decay 150–300 ms, sustain 0, reverb send 0.2–0.35, ping-pong 0.75 beat. **(engine default)**
- Rave stab or M1 piano stab (hardcore, piano house): FM EP or piano voicing of minor 7th / major 7th chords, 1 bar of 8th or dotted-8th hits. [20][21][28] (rhythm **(engine default)**)
- Juno pads and chords (deep house, synth-pop): saw + PWM at 0.3–0.5 Hz LFO and PWM depth 0.3–0.6, attack 300–800 ms, release 600–1200 ms, LP 1.5–3 kHz, chorus I for wide and soft, chorus II for moving. Chorus I = 0.513 Hz ±0.5 ms; II = 0.863 Hz ±1.1 ms; centre 3.5 ms. [44] (envelope values **(engine default)**)

### Melody / hooks
- **Supersaw (trance):** 7 saws at the measured spread. Uplifting lead: knob 0.75 (inner ±8, middle ±26, outer ±46 cents). Pad: knob 0.5 (±3, ±10, ±18). Hardstyle/big room: knob 0.94 (±13, ±43, ±74). Mix 0.75 (centre 0.58, sides 0.59). HP at the fundamental; random phase per note. [13][14]
- Supersaw filter: LP 12 dB, cutoff 4–8 kHz for leads; 24 dB, 1–3 kHz for pads; ping-pong delay 0.75 beat, fb 0.3–0.45, reverb RT60 2–4 s. [14] (values **(engine default)**; dotted 8th **(unverified)**)
- **Hoover (hardcore, Belgian techno, rave revival):** PWM-saw on three oscillators at −12, 0 and +12 semitones; PWM LFO 4–6 Hz at depth 0.6–0.9; chorus fast (I+II, Juno-6 style: 8 Hz, 25 %); bend down 2–12 semitones over 100–300 ms at note start; mono, glide 50–150 ms. [26][15] (rates, bend and glide **(unverified)/(engine default)**)
- **FM EP (deep house, garage, lo-fi):** algorithm 5 analogue in 4-op: tine pair 14:1 (mod level moderate, velocity 7, mod decay 300–600 ms) + body pair 1:1 with feedback 6/7; a 1:1 pair detuned ±7 DX units for chorus. [23][24]
- **FM bells (trance breakdowns, ambient):** pairs at 3.5:1 (or 3.18:1), one fixed-frequency operator near 320 Hz, carrier decay 2–4 s. [23][24] (decay **(engine default)**)
- **Sync lead (electro, synthwave):** osc 2 synced to osc 1, sync ratio swept 1→3 over 200–400 ms by the filter envelope; 12 dB LP. [17][18] (values **(engine default)**)

### Sound design (mix & master)
- Sidechain pump, by genre: house −3 to −4.5 dB (depth 0.3–0.4); French/filter house −6 to −9 dB (0.5–0.65); trance −4 to −6 dB (0.35–0.5); techno −2 to −3 dB (0.2–0.3); DnB, dubstep, trap 0 to −2 dB (≤ 0.2). Attack ≤ 6 ms. [29][30] (the per-genre split **(engine default)**; the 6 dB "subtle" ceiling [29])
- Glue: ratio 2:1 (4:1 for techno and big room), attack 10–30 ms, release 100–200 ms, 1–4 dB gain reduction. [31][32]
- Tape: drive 0.1–0.3 on house, deep house and lo-fi; 0 to 0.1 on trance and big room. **(engine default)**
- Master loudness targets (integrated): dubstep and DnB −6 to −7 LUFS; hard techno, trance and big room −7.5; techno −8.5; tech house and house −9; deep, progressive and melodic house −10.5. True peak ≤ −1 dBTP. [34][35]
- High-pass corners: pads and chords 150–250 Hz; leads 200–300 Hz; hats 300–500 Hz; reverb return 200 Hz; delay return 280 Hz (as now). **(engine default)**
- Juno HPF as a 4-step switch: boost <150 Hz +10 dB / flat / 240 Hz / 720 Hz, 1-pole. [44]

### Arrangement & energy
- Noise riser: white noise → BP or LP with Q 2–4, centre swept 300 Hz → 8–12 kHz over 8 bars (or 7 of 8, holding the last bar), HP rising from 100 Hz to 1 kHz alongside, level +6 dB across the riser. [45][46][47] (Q, end frequency and level **(engine default)**)
- 303 build: raise cutoff by 1–2 octaves and resonance by 0.1–0.2 over 8–16 bars, and accent density from 1-in-8 to 1-in-3 steps (accent stacking raises the peaks). [10] (rates **(engine default)**)
- Breakdown pads: reverb RT60 3–5 s, pump off; restore the pump at the drop. **(engine default)**
- Gated-reverb fill: gate the room on the last snare hits of a phrase. [33] (placement **(engine default)**)

### Variation / surprise
- Detune drift: give every analogue voice ±3 cents of pitch and ±5 % of cutoff variance per voice, fixed per voice. [44]
- Supersaw knob automation: sweep detune 0.5 → 0.94 into a drop. The non-linear curve keeps it smooth until about 0.75, then it opens fast. [13]
- Every 4–8 bars on acid lines, change the decay knob (200 → 600 ms) or cutoff, not the notes. [9][12] (period **(engine default)**)
- Swap the chorus mode between sections (I in verses, II in choruses, I+II for one fill bar). [15][44] (placement **(engine default)**)
- Wobble: change the LFO division (1/4 → 1/8 → 1/8T → 1/16) on bar or half-bar boundaries. [43] (boundaries **(engine default)**)
- Lo-fi drop: for one bar before a section, requantise the drum bus to 8 bits at 25 kHz (707/727 character). [38] (use **(engine default)**)

---

## References

1. Gordon Reid, "Practical Bass Drum Synthesis", *Sound On Sound* (Synth Secrets), February 2002. https://www.soundonsound.com/techniques/practical-bass-drum-synthesis
2. Kurt James Werner, Jonathan S. Abel, Julius O. Smith III, "A Physically-Informed, Circuit-Bendable, Digital Model of the Roland TR-808 Bass Drum Circuit", *Proc. DAFx-14*, Erlangen, 2014, pp. 159–166. https://dafx14.fau.de/papers/dafx14_kurt_james_werner_a_physically_informed,_ci.pdf
3. Kurt James Werner, Jonathan S. Abel, Julius O. Smith, "The TR-808 Cymbal: a Physically-Informed, Circuit-Bendable, Digital Model", *Proc. ICMC|SMC 2014*, Athens, 2014. https://speech.di.uoa.gr/ICMC-SMC-2014/images/VOL_2/1453.pdf
4. Gordon Reid, "Practical Snare Drum Synthesis", *Sound On Sound* (Synth Secrets), April 2002. https://www.soundonsound.com/techniques/practical-snare-drum-synthesis
5. Gordon Reid, "Practical Cymbal Synthesis", *Sound On Sound* (Synth Secrets), July 2002. https://www.soundonsound.com/techniques/practical-cymbal-synthesis
6. Adam Douglas, "Atsushi Hoshiai: the man behind the TR-909", *Attack Magazine*, 20 August 2024. https://www.attackmagazine.com/features/interview/atsushi-hoshiai-the-man-behind-the-tr-909/
7. Wikipedia, "Roland TR-909" (accessed 2026-10-19; cites Reid, *Sound On Sound*, 2014; Butler, *Unlocking the Groove*, 2006). https://en.wikipedia.org/wiki/Roland_TR-909
8. Wikipedia, "Roland TR-808" (accessed 2026-10-19; cites Reid, *SOS* 2002; Leight, *Rolling Stone* 2016; Norris, *The New Yorker* 2015). https://en.wikipedia.org/wiki/Roland_TR-808
9. Robin Whittle, "User Manual for the Devil Fish mods to the TB-303", 9 February 2019. https://www.firstpr.com.au/rwi/dfish/Devil-Fish-Manual.pdf
10. Robin Whittle, "What makes the TB-303 unique" (Devil Fish site; accent and accent sweep circuit). https://www.firstpr.com.au/rwi/dfish/303-unique.html
11. Tim Stinchcombe, "Diode Ladder Filters (including the pretension to 18dB)". http://www.timstinchcombe.co.uk/index.php?pge=diode
12. Wikipedia, "Roland TB-303" (accessed 2026-10-19; cites Sholomove, *Attack Magazine* 2020; Reid, *SOS* 2004). https://en.wikipedia.org/wiki/Roland_TB-303
13. Adam Szabo, *How to Emulate the Super Saw*, BSc thesis, KTH Royal Institute of Technology, 2010 (TRITA-CSC-E 2010:131). https://www.nada.kth.se/utbildning/grukth/exjobb/rapportlistor/2010/rapporter10/szabo_adam_10131.pdf
14. Wikipedia, "Roland JP-8000" (accessed 2026-10-19; cites Ward, *SOS* 1998; Shore 2013; Szabo 2010). https://en.wikipedia.org/wiki/Roland_JP-8000
15. SoundForce, "Chorus 6" module description (Juno chorus clone; I+II settings per model), ModularGrid. https://modulargrid.net/e/modules/view/43993
16. Wikipedia, "Roland Juno-106" (accessed 2026-10-19; cites "The History of Roland: Part 2", *SOS*; Roland Icon Series). https://en.wikipedia.org/wiki/Roland_Juno-106
17. Wikipedia, "Roland Jupiter-8" (accessed 2026-10-19; cites *SOS* Feb 1998). https://en.wikipedia.org/wiki/Roland_Jupiter-8
18. Wikipedia, "Sequential Circuits Prophet-5" (accessed 2026-10-19; cites Reid, *SOS* March 1999). https://en.wikipedia.org/wiki/Sequential_Circuits_Prophet-5
19. Wikipedia, "Minimoog" (accessed 2026-10-19; cites Magnus, *SOS* 1998; Pinch & Trocco, *Analog Days*, 2002). https://en.wikipedia.org/wiki/Minimoog
20. Wikipedia, "Korg M1" (accessed 2026-10-19; cites Vail, "Korg M1 (Retrozone)", *SOS* Feb 2002; Korg M1 Super Guide). https://en.wikipedia.org/wiki/Korg_M1
21. MusicRadar, "How to create classic house sounds with the Korg M1 and when to use them" (preset numbers as quoted in search results; article body not retrieved). https://www.musicradar.com/news/how-to-create-classic-house-sounds-with-the-korg-m1-and-when-to-use-them
22. MusicRadar, "Beyonce's Break My Soul sounds like a love letter to the Korg M1 synth" (as quoted in search results). https://www.musicradar.com/news/beyonce-break-my-soul-korg-m1-synth
23. Yamaha DX7 factory ROM1A voice bank (1983), as the `ROM1A.SYX` dump distributed in Matt Montag, *dx7-synth-js*, decoded with the DX7 32-voice packed format. https://github.com/mmontag/dx7-synth-js
24. Paul Colbert, "Yamaha DX7", *One Two Testing*, November 1983 (Muzines archive). https://www.muzines.co.uk/articles/yamaha-dx7/4733
25. Attack Magazine, "Reese Bass Redux", 4 September 2014. https://www.attackmagazine.com/technique/tutorials/reese-bass-redux/
26. Wikipedia, "Hoover sound" (accessed 2026-10-19; cites Vail, *The Synthesizer*, OUP 2014; Roland, *Alpha Juno-2 Owner's Manual*, 1985; Moffa, "Key Tracks: Mundo Muzique on Second Phase's 'Mentasm'", Red Bull Music Academy 2014). https://en.wikipedia.org/wiki/Hoover_sound
27. Aykan Esen, "The Theory of Techno: Parallel Chord Stabs", *Attack Magazine*, 20 February 2025 (excerpt from *The Secrets of Techno Production*). https://www.attackmagazine.com/technique/tutorials/the-theory-of-techno-parallel-chord-stabs/
28. MusicRadar, "How to create a classic rave chord stab" (as quoted in search results: constant chord shape, Ableton Chord set to minor 7). https://musicradar.com/how-to/rave-chord-stab
29. Bruce Aisher, "Ten Production Tips for Better Basslines", p. 4 "Sidechain Compression", *Attack Magazine*, 21 May 2013. https://attackmagazine.com/technique/ten-production-tips-for-better-basslines/4
30. Sonarworks, "Sidechain compression" (as quoted in search results). https://www.sonarworks.com/blog/learn/sidechain-compression
31. Waves, "How to Use an SSL Buss Compressor: Complete Guide to the Classic Mix Glue" (as quoted in search results). https://www.waves.com/how-to-use-ssl-bus-compressor-classic-mix-glue
32. *Sound On Sound*, review of Cytomic The Glue (as quoted in search results). https://www.soundonsound.com/reviews/cytomic-glue
33. Wikipedia, "Gated reverb" (accessed 2026-10-19; cites *Sound On Sound*, "Canyons of the Mind", 1996; Flans, *Mix* 2005; *SOS* RMX16 review). https://en.wikipedia.org/wiki/Gated_reverb
34. Spotify for Artists, "Loudness normalization". https://support.spotify.com/us/artists/article/loudness-normalization/
35. Mixgraph (NC Digital Ltd), "LUFS meter": measurements of 900 released tracks, 2026. https://www.mixgraph.io/tools/lufs-meter
36. Attack Magazine, rolling techno bass tutorial (kick tuning sets the bass key; trimming the kick's sub tail) (as quoted in search results). https://attackmagazine.com/?p=85913
37. Wikipedia, "Roland TR-707" (as mirrored; 8-bit samples, 6-bit cymbals; "Move Your Body"). https://en.wikipedia.org/wiki/Roland_TR-707
38. Roland, "TR-727 Software Rhythm Composer" articles (25 kHz/8-bit, some 6-bit; non-decaying PCM with analogue decay). https://www.roland.com/us/products/rc_tr-727/articles/
39. Wikipedia, "Linn LM-1" (8-bit companded samples at 28 kHz). https://en.wikipedia.org/wiki/Linn_LM-1
40. National Music Centre (Calgary) collection, "Linn LinnDrum" (35 kHz, 8-bit). https://nmc.emuseum.com/objects/271/linn-linndrum
41. KVR Audio forum, "Open303 – open source 303 emulation project" (author's measurement: diode ladder matches theory except for a high-pass in the resonance feedback loop) (as quoted in search results). https://www.kvraudio.com/forum/viewtopic.php?p=3931725
42. MusicRadar, "How to make a classic reese bass like Renegade's Terrorist" (±55 cents, as quoted in search results). https://musicradar.com/how-to/classic-reese-bass-renegade-terrorist
43. MusicRadar, "'This is a sound that captured the production psyche': How to build an LFO wobble bass" (LFO 1/8 triplet, triangle, LFO delay, cutoff modulation, as quoted in search results). https://musicradar.com/how-to/lfo-wobble-bass
44. Ultramaster KR-106, "DSP Architecture" (a circuit-informed Juno-106 emulation's model parameters; not Roland data). https://raw.githubusercontent.com/kayrockscreenprinting/ultramaster_kr106/main/docs/DSP_ARCHITECTURE.md
45. MusicRadar, "How to make synth sweep FX for EDM drops" (as quoted in search results). https://www.musicradar.com/how-to/synth-sweep-fx
46. Soundbridge, "8-bar EDM buildup" (as quoted in search results). https://soundbridge.io/8-bar-edm-buildup
47. Unison Audio, "How to create risers" (as quoted in search results). https://unison.audio/how-to-create-risers/
