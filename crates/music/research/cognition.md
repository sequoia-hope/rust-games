# Corpus and cognition research for a surprise-aware generator

Scope: what corpus studies say about chord and rhythm statistics, what music-cognition research says about predictability, surprise, syncopation and repetition, and how to turn that into a generate-and-score loop. The loop proposes candidate phrases and progressions, scores them with a variable-order statistical model (information content and entropy), plus a syncopation index and a repetition measure, and keeps the candidates that land in a target band.

Conventions: 16-step bars, step 1 = downbeat, steps 1/5/9/13 are the quarter-note beats. Roman numerals are relative to the tonic. Where a number comes from my own arithmetic on a published table or dataset (not stated by the authors), it is marked **(derived)**. Where a parameter is an engineering proposal with no direct empirical source, it is marked **[proposal]**.

---

## 1. Harmony corpora: actual numbers

### 1.1 Rock: de Clercq & Temperley (2011), "RS 5×20" corpus

The 2011 corpus is **100 songs**: the top 20 per decade (1950s–1990s) from Rolling Stone's "500 Greatest Songs", analysed independently by both authors. They agreed on 92.4% of relative roots and 94.4% of absolute roots. Repeated adjacent chords with the same root are merged into one chord. [1] (The later 200-song version with melodies is Temperley & de Clercq 2013 [2].)

**Root distribution (Table 2 of [1]):**

| Root | Proportion | Songs containing (of 100) |
|---|---|---|
| I | 0.328 | 99 |
| IV | 0.226 | 90 |
| V | 0.163 | 88 |
| bVII | 0.081 | 37 |
| VI | 0.072 | 39 |
| bVI | 0.040 | 21 |
| II | 0.036 | 39 |
| bIII | 0.026 | 18 |
| III | 0.019 | 23 |
| bII | 0.005 | 5 |
| VII | 0.004 | 7 |
| #IV | 0.003 | 4 |

I, IV, V, bVII and VI make up 87% of all harmonies. Chord quality: 75.8% major, 23.4% minor, 0.7% diminished, 0.1% augmented. 94.1% of chords are in root position, against about 60% in common-practice music. [1]

**Transition probabilities, P(next | current)**, row-normalised from the raw counts in Table 3 of [1] **(derived)**. IC = −log2 p.

| From | n | H (bits) | Top continuations: p (IC in bits) |
|---|---|---|---|
| I | 2951 | 2.51 | IV .36 (1.5), V .24 (2.1), bVII .16 (2.7), VI .10 (3.3), II .04 (4.5) |
| IV | 2090 | 1.97 | I .56 (0.8), V .25 (2.0), bIII .05 (4.4), bVII .04 (4.5), VI .03 (4.9) |
| V | 1488 | 1.82 | I .53 (0.9), IV .26 (1.9), VI .13 (3.0), bVII .03 (5.0), II .02 (5.4) |
| VI | 671 | 2.24 | IV .39 (1.4), I .21 (2.2), V .18 (2.4), II .13 (2.9), III .05 (4.4) |
| bVII | 735 | 1.77 | I .53 (0.9), IV .26 (2.0), bVI .16 (2.7), V .04 (4.8) |
| bVI | 367 | 1.92 | I .57 (0.8), bVII .21 (2.2), IV .06 (4.1), V .06 (4.1) |
| II | 332 | 2.23 | I .36 (1.5), V .29 (1.8), IV .17 (2.5), VI .07 (3.8) |
| bIII | 238 | 2.32 | bVI .28 (1.8), IV .27 (1.9), I .21 (2.3), bVII .17 (2.5) |
| III | 174 | 2.23 | VI .34 (1.5), IV .26 (1.9), II .22 (2.2), I .09 (3.4) |

The zero-order (unigram) entropy of the root distribution is 2.70 bits **(derived)**. So a first-order model removes about 0.2–0.9 bits per chord, depending on context.

Other findings from [1]:
- **Movement around the tonic is symmetric.** The chords that most often lead into I and the chords that most often follow I are the same three, in the same order: IV, V, bVII. IV is 39.6% of pre-tonic chords and 35.6% of post-tonic chords, against 33.6% of non-tonic chords overall. VI is under-represented before the tonic (5.0% vs 10.7%).
- **Root motion.** Ascending and descending motions of each interval are about equally common: +P4 2,266 vs −P4 2,220, +M2 1,384 vs −M2 1,386. Frequency falls steadily with distance on the circle of fifths: P4 > M2 > m3/M3 > m2 > tritone (20).
- **Most common trigrams ending on I:** IV–V–I 352, V–IV–I 292, bVII–IV–I 146, VI–IV–I 126, bVII–bVI–I 103, bIII–bVI–I 66, II–V–I 63, bVI–bVII–I 60, V–VI–I 42, IV–bVII–I 39. A pre-tonic V is nearly always approached from IV.
- **By decade.** In the 1950s, I+IV+V make up more than 95% of harmonies. From the 1960s on the distribution widens, and bVII and VI each reach 6–11%.

### 1.2 Pop: Hooktheory TheoryTab / Trends

The Hooktheory Trends API returns next-chord probabilities from the TheoryTab database. Its documentation gives these example values [3]:

| Context | Next chord probabilities |
|---|---|
| (none, all chords) | I .176, IV .12, V .116, vi .075, ii .033 |
| after IV | I .332, V .266, vi .066, I⁶ .04, iii .035 |
| after IV → I | V .407, IV .199, vi .101, ii .046, iii .033 |

The second-order row shows that context matters: after IV→I, V becomes the favourite (.41), while after IV alone, I is the favourite (.33). Hooktheory's 2012 analysis of about 1,300 song sections (mostly Billboard material) found that iii is followed by vi or IV 93% of the time [4]. The Trends page now draws on more than 40,000 songs; I could not verify an exact count beyond that.

### 1.3 McGill Billboard corpus

The corpus contains more than 1,000 expert transcriptions of songs sampled at random from Billboard Hot 100 charts, 1958–1991. It holds 414,059 labelled beats, 638 distinct chords and 99 chord classes. A song has 11.8 unique chords on average (minimum 1, maximum 84), and late-1970s songs are the most varied. [5] Cheung et al. trained IDyOM on 745 of these songs, 80,943 chords [16] (section 2.4).

### 1.4 Classical, for contrast: Moss et al. (2019)

In Beethoven's string quartets, chord rank-frequency follows a power law (Zipf–Mandelbrot): a few chords cover most of the data. Chords on roots I and V make up more than 60% of chords in major and more than 50% in minor. To measure predictability per chord, the authors use normalised conditional entropy H(B|a)/log2|B|. [6] Pop is flatter around the tonic and more plagal (1.1). Classical harmony is dominated by V→I.

### 1.5 Electronic dance music: mode and tempo (Beatport data)

I found no published corpus of chord-to-chord transitions for Beatport EDM, so that gap remains. What is available is the **GiantSteps** datasets: Beatport 2-minute previews whose keys and tempos were annotated from user corrections [7]. I counted the published annotations myself **(derived)**:

- **Mode:** 511 of 604 tracks (84.6%) are minor. By genre: deep house 95% (73/77), electro house 90%, dubstep 91%, tech house 88%, trance 88%, house 83%, techno 82% (28/34), drum & bass 82%, progressive house 76%, breaks 71%, chill-out 64%. Minimal (11/11) and psy-trance (5/5) are all minor, but those samples are tiny.
- **Tempo** (crowd-corrected v2 labels [8], median and IQR): deep house 122 (121–123), tech house 127 (123–127), progressive house 127 (127–129), electro house 128.5 (127–130), techno 127 (123–132), trance 139, dubstep 139, breaks 139.5, psy-trance 147, drum & bass 174 (170–174), hardcore/hard techno 170.

The pop and rock tables above are major-key-centric. For EDM, the language model should therefore default to a minor-tonic alphabet: i, bIII, iv, v, bVI, bVII, and so on.

### 1.6 Long-run evolution of popular music

- **Mauch, MacCallum, Levy & Leroi (2015)** analysed about 17,000 Billboard Hot 100 recordings, 1960–2010, using 8 harmonic and 8 timbral "topics" [9]. Findings:
  - The most common harmonic topic is H8, "major chords, no changes" (mean frequency 0.236). It was the top topic in 43 of 50 years.
  - Dominant-7th usage (H1) fell by about 75% between 1960 and 2009.
  - Minor-7th "colour" chords (H3, typical of funk and disco) more than doubled between 1967 and 1977.
  - "No identifiable chords" (H5) grew from the late 1980s and peaked in 1993 with hip hop.
  - The timbral topic "drums, aggressive, percussive" (T1) peaked in the 1980s drum-machine era and declined after 1990.
  - Three stylistic revolutions stand out, around 1964, 1983 and 1991.
- **Serrà et al. (2012)** studied the Million Song Dataset [10]. Pitch "codeword" frequencies follow a power law with exponent β ≈ 2.18 ± 0.06, the same as Zipf's law in language, and this was stable for 50 years. Over time, pitch transitions became more restricted (fewer transition options, more defined paths) and the timbral palette became more homogeneous. Median loudness rose from −22 to −13 dBFS, a slope of 0.13 dB/year, while the loudness spread stayed around 9.5 dB.

Implication: real music uses a Zipf-shaped vocabulary, a small number of very common moves plus a long tail. If the generator samples uniformly from "allowed" chords, it will sound unidiomatic. Weight choices by corpus frequency, and spend the tail deliberately (section 6).

---

## 2. Expectation, surprise and pleasure

### 2.1 Huron, *Sweet Anticipation* (2006): ITPRA

Huron splits expectation-driven emotion into five systems ordered in time around an event [11][12]:

- **Imagination**: imagining outcomes long before the event.
- **Tension**: arousal and attention rise just before the expected moment.
- **Prediction**: an immediate reward or penalty for how *accurate* the prediction was. Correct predictions feel good even when the outcome is bad, and Huron argues this pleasure is misattributed to the stimulus, which explains the mere-exposure effect.
- **Reaction**: a fast, automatic, worst-case response starting under about 150 ms.
- **Appraisal**: a slower, conscious evaluation that can overrule the reaction.

Musical surprise is pleasant through **contrastive valence**: the negative reaction and prediction responses are followed by a neutral or positive appraisal, and the contrast amplifies the positive feeling. Huron links surprise to frisson, laughter and awe [12] (summarising Huron 2006, pp. 17, 21–25, 138).

Engine reading: predictable events earn a small reward on every event. Surprises earn a larger reward only if the context is safe and the surprise resolves. This is why surprise has to be rationed and placed, not spread evenly.

### 2.2 Berlyne's inverted U

Berlyne proposed that hedonic value is highest at moderate "arousal potential", driven by collative variables: novelty, complexity, surprisingness, incongruity [13]. Cheung et al. cite this as the "inverted-U relationship between pleasure and variables such as exposure and complexity" and then show that, for chords, the picture involves two variables (2.4) [16].

### 2.3 IDyOM: how expectation is computed (Pearce 2005, 2018)

IDyOM is a **multiple-viewpoint, variable-order Markov model** built on **PPM** (Prediction by Partial Match) from data compression [14][15].

**Core quantities**, for event e_i with context c = e_{i−n+1..i−1} over alphabet A [16]:
- Information content (surprise): **IC(e_i) = −log2 p(e_i | c)**
- Entropy (uncertainty), computed *before* the event: **H(c) = −Σ_{e∈A} p(e | c) log2 p(e | c)**
- Mean IC over a piece is its cross-entropy, the standard measure of model quality.

**PPM with escape method C and interpolated smoothing** [14]. For context s, let c(e|s) be the count of e after s, n(s) = Σ_e c(e|s) the total, t(s) the number of distinct symbols seen after s, and su(s) s with its oldest element dropped:

```
p(e | s) = ( c(e|s) + t(s) · p(e | su(s)) ) / ( n(s) + t(s) )      // recurse down to order 0
p(e | ε-1) = 1 / |A|                                                // order −1: uniform
```

This is the Witten–Bell form of method C: the weight on the lower order, t/(n+t), grows when many different continuations have been seen after s. Pearce's experiments on 8 melodic datasets found consistent gains from four things [14]:
- escape method C (AX was also good for the short-term model);
- interpolated smoothing, blending all orders rather than backing off;
- unbounded order (PPM*, which uses the longest matching context), but only together with interpolated smoothing;
- combining a long-term model and a short-term model.

Update exclusion was inconsistent. Where an order bound is used, the best bound depended on the dataset.

**Long-term and short-term models.**
- The **LTM** is trained on a corpus and represents the listener's enculturation.
- The **STM** starts empty for each piece and learns as the piece unfolds. This is how within-piece repetition becomes expected.

The two are combined with a **weighted geometric mean** [14]:

```
p(e) ∝ ( Π_m p_m(e)^{w_m} )^{1 / Σ w_m},       w_m = ( H(p_m) / log2|A| )^{−b}
```

Lower-entropy (more confident) models get more weight. The bias b tunes how strongly. The optimal b was **1 to 4** depending on the dataset, and values up to 32 made results worse. The geometric mean beat the arithmetic mean, especially when combining viewpoints. [14]

**Viewpoints** are derived features, each with its own model, whose predictions are combined the same way [15]:
- chromatic pitch;
- pitch interval (pi);
- scale degree (sd);
- inter-onset interval (ioi) and ioi-ratio;
- linked pairs such as pi⊗sd (melody and tonality) or sd⊗ioi-ratio (tonality and rhythm).

Pitch and onset are predicted in parallel, and the probability of a note is the joint probability of its pitch and onset [15].

Typical magnitudes on chorales: about 2.34 bits/note for a pitch-only model, about 1.95 bits/note for the best multiple-viewpoint system [14]. In Gold et al.'s stimulus set, mean IC per melody ran from 2.34 bits (a folk tune) to 16.52 bits (atonal) [17].

For harmony, Cheung et al. used a **single viewpoint**: the alphabet was chord symbols, including scale degree, chord type and inversion, as found in the Billboard corpus. Their model was LTM+STM ("BOTH") with 10-fold cross-validation [16].

**Generating from such a model.** Sampling one event at a time greedily favours high-probability events and can still produce a low-probability whole. Pearce describes Metropolis–Hastings as an alternative [14]:
1. Start from a sequence.
2. Pick an index and propose a replacement event from q (the model's conditional distribution).
3. Accept with probability min(1, p(s′)q(t_i)/p(s)q(t′_i)).
4. Repeat for N iterations.

### 2.4 Cheung et al. (2019): uncertainty × surprise in chords

**Design** [16]:
- IDyOM computed entropy (uncertainty, before each chord) and IC (surprise) for 80,943 chords from 745 Billboard songs.
- 30 progressions were selected: 1,039 chords, 30–38 chords each, transposed to C major, chords lasting 2.4 s each, with a quadruple-metre drum backing.
- Selection criteria for the progressions:
  - each starts on tonic root position and ends on a perfect or plagal cadence;
  - each contains chords from all four high/low entropy × high/low IC quadrants (cut-offs at the upper and lower 40th percentiles);
  - no 5+ chord sub-sequence repeats without a gap of at least 2 chords, or more than 3 times in a row.
- 39 subjects gave continuous pleasantness ratings. Another 40 were scanned with fMRI.

**Result: standardised fixed effects, linear mixed model** [16]:

| Predictor | β | 95% CI |
|---|---|---|
| Uncertainty (entropy) | −0.143 | [−0.186, −0.099] |
| Surprise (IC) | −0.327 | [−0.418, −0.236] |
| Uncertainty × surprise | −0.124 | [−0.183, −0.064] |
| Sensory dissonance | −0.251 | [−0.345, −0.158] |
| Spectral centroid | +0.072 | [0.024, 0.119] |
| Spectral complexity | +0.224 | [0.147, 0.300] |

Marginal R² = 0.476. The fitted surface is a **saddle**. The authors' summary: "When the uncertainty of the harmonic context was low (e.g., toward the end of a musical section), chords with higher surprise ... were rated as more pleasant." When uncertainty was high, less surprising chords were preferred. At mean uncertainty, surprise mattered 30% more than sensory dissonance, and twice as much at +1.5 SD of uncertainty. In the brain, amygdala, hippocampus and auditory cortex tracked the interaction, while the nucleus accumbens tracked only uncertainty. [16]

**Reading the coefficients (derived):**
- The slope of pleasure with respect to surprise is −0.327 − 0.124·U, with U in SD units.
- At U = 0 it is −0.33. At U = +1 it is −0.45. At U = −1 it is −0.20. It reaches 0 only near U ≈ −2.6.
- So surprise is cheapest where uncertainty is lowest, and costs most where uncertainty is high.

Rules for chord choice:
- **Surprise only after confident contexts.** Place a high-IC chord where the model is confident: after a well-established loop, at the end of a section, or at the 4th/8th chord of a repeated 4-chord cycle.
- **Resolve uncertainty with the expected chord.** After an unusual chord, or anywhere entropy is high, choose a low-IC continuation.
- **Keep the sound smooth.** Sensory dissonance costs pleasure independently (β −0.25), while spectral richness helps (β +0.22).

### 2.5 Gold et al. (2019): inverted U for melodic IC and entropy

**Design** [17]:
- 55 real monophonic melodies, about 30 s each, played on flute at 96, 120 or 144 bpm.
- IDyOM was trained on 903 folk and chorale melodies. The best viewpoint set was ioi-ratio plus chromatic pitch, with duration weighting.
- 43 subjects rated liking.

**Results** [17]:
- Liking against mean IC was an inverted U. Quadratic β: −0.09 in Study 1 and −0.18 in Study 2.
- Liking against mean entropy was also an inverted U. Quadratic β: −0.06 in Study 1 and −0.25 in Study 2.
- Peak positions, computed from the standardised coefficients **(derived)**:
  - IC: about z −1.2 in Study 1 and about z 0 in Study 2.
  - Entropy: about z −0.6 to −0.75.
  - In other words, liking peaks at or a little below average complexity.
- In low-IC melodies, high entropy was preferred over low entropy (p = .001). Predictable outcomes are most welcome in uncertain contexts, which agrees with 2.4.
- Over 7 repetitions, liking declined (quadratic; R² = .81), but the IC curve's shape did not change significantly.

---

## 3. Rhythm: syncopation and groove

### 3.1 Longuet-Higgins & Lee (LHL) index on a 16-step grid

Metrical weights, following Fitch & Rosenfeld [19][20]:

| Step | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| w | 0 | −4 | −3 | −4 | −2 | −4 | −3 | −4 | −1 | −4 | −3 | −4 | −2 | −4 | −3 | −4 |

A syncopation is a rest or tied note R preceded by a sounded note N of **lower** weight. Its value is w(R) − w(N), and values are summed over the pattern, wrapping when the pattern loops [20].

Grid implementation **(derived operationalisation)**: for each onset at step i, scan the silent steps up to the next onset. If the highest weight among them, w_max, is greater than w(i), add w_max − w(i).

Fitch & Rosenfeld's worked example scores 5, which they call moderate. Their highly syncopated rhythms caused listeners to re-hear the beat; less syncopated rhythms were remembered better [20].

### 3.2 Witek et al. (2014): polyphonic drum index and inverted U

**Stimuli** [18]: 50 two-bar drum breaks (kick, snare, hi-hat on constant 8ths) at 120 bpm, each looped four times. 36 were taken from real funk breaks. 66 participants rated them.

**Metric weights** (Figure S5 of [18]):
- step 1 = 0;
- steps 5, 9, 13 = −1 (all quarter notes except the downbeat are treated as equal);
- steps 3, 7, 11, 15 = −2;
- even steps = −3.

**Definition** (Text S2 of [18]):
- A note N on one instrument followed by a note on a *different* instrument at weight ≥ w(N) is a polyphonic syncopation.
- A note followed by a rest at weight ≥ w(N) is a monophonic syncopation.

**Score:** S = w(following) − w(N) + I, where the instrument weight I is:
- kick syncopated while snare and hat land on the stronger position ("three-stream"): I = 2;
- snare syncopated while kick and hat land on it: I = 1;
- only one other stream lands on the stronger position ("two-stream"): I = 5.

Instances are summed over the 2-bar loop, including the wrap. Worked examples: "Impeach the President" = 5, "Lifetime Monologue" = 41, "Actual Proof" = 68. [18]

**Findings** [18]:
- Wanting to move and pleasure followed an **inverted U**, with medium syncopation highest.
- The polyphonic index explained more variance than plain LHL: R² 0.357 vs 0.273 for wanting to move, 0.430 vs 0.306 for pleasure.
- The stimuli spanned 0–81, binned as low 0–21 (mean 10.8), medium 22–46 (mean 33.5) and high 49–81 (mean 60.3).
- The individual vertices for wanting to move had means of 31–35 (musicians 35.2, non-musicians 31.2). Groove enthusiasts peaked slightly higher (35.3 vs 30.0, p = .052).
- Audio entropy predicted ratings poorly.
- A 2015 correction fixed stimulus-indexing errors and states that the conclusions are unchanged.

### 3.3 Other groove studies

- **Sioros et al. (2014)** made simple piano melodies at 120 bpm syncopated by anticipating the last strong note of each phrase by an 8th or a 16th, and by shifting weak-beat quarters. **Moderate** syncopation was rated grooviest: higher than none, and higher than "maximum" (every note anticipated by an 8th). The relationship was not linear [21].
- **Matthews et al. (2019)** used a one-bar 5-onset 3+2 piano rhythm at 96 bpm over constant 8th hats [22]:
  - Low complexity: all 5 onsets on strong beats.
  - Medium: son or rumba clave.
  - High: only the first onset on a strong beat.
  - Results: an inverted U for rhythm, with medium (clave-level) syncopation highest for pleasure and wanting to move. Low- and medium-complexity chords were rated similarly. High-complexity chords (with a b9) dampened the rhythm effect on pleasure.
  - Syncopation was scored with the Fitch–Rosenfeld LHL index.

### 3.4 Worked values on the 16-step grid (derived, using 3.1 and 3.2)

| Pattern | LHL (mono) |
|---|---|
| Four-on-the-floor kick 1,5,9,13 | 0 |
| Off-beat 8ths 3,7,11,15 (house hat or off-beat bass) | 7 |
| Tresillo 3+3+2: 1,7,13 | 2 |
| Pushed bass 1,4,7,9,12,15 | 4 |
| Son clave 3-2: 1,4,7,11,13 | 4 |
| Every 16th off-beat 2,4,…,16 | 15 |

Witek index, one bar:

| Pattern | Witek |
|---|---|
| Rock beat (kick 1,9; snare 5,13; 8th hats) | 0 |
| House (kick 1,5,9,13; clap 5,13; hats 3,7,11,15) | 0 |
| Kick 1,4,11; snare 5,13; 8th hats | 7 |
| Breakbeat (kick 1,3,11,12; snare 5,8,10,13,16; 8th hats) | 20 |

Key consequence: a four-on-the-floor kick-and-clap grid scores **zero** on both indices. Dance genres therefore get their syncopation from other layers: off-beat hats and bass, 16th percussion, pushed chord stabs, and breakbeat kicks. Score each layer separately with LHL, and score the kick/snare/hat "kit" with the Witek index.

### 3.5 Microtiming

In the Groove MIDI Dataset (10 drummers, 13.6 h, 1,150 files, about 22,000 bars on a Roland TD-11), notes on the 8th-note grid tend to be played **late** and off-beat 16ths tend to be played **early** [23]. This is a direction to humanise in, not a genre constant.

---

## 4. Repetition

- Margulis summarises the field as follows [24]:
  - Huron estimates that more than 90% of listening time is spent on passages the listener has heard before.
  - Looping a spoken phrase makes it sound sung (Deutsch's speech-to-song illusion).
  - Random note sequences heard 6 times are later rated more musical, even when listeners don't remember them.
  - Excerpts of Berio and Carter edited to *add* repetition were rated more enjoyable and more likely to be human-composed.
  - Exact refrain repetition in rondos produced more tapping and singing along than varied refrains.
- Over 7 exposures, liking declines but the preferred complexity level stays put (2.5) [17].
- In IDyOM, the STM gives repetition its effect: when a hook returns, the STM has learned it, so its IC falls. The model therefore *measures* the cost of repetition, and a returning hook automatically reads as "predictable". [14][15]
- Cheung et al. limited repetition in their stimuli: a 5+ chord sub-sequence could not repeat without a gap of at least 2 chords, nor more than 3 times in a row [16]. This was a selection constraint, not a preference finding, but it is a usable ceiling.

---

## 5. Tension models for arrangement

- **Farbood (2012)** [25] models tension as integrated trend:
  - **Trend:** the slopes of features within a moving attentional window are combined with weights. Loudness/dynamics and melody pitch height get 3; tempo and onset frequency get 2; harmony, bass pitch height and inner-voice pitch get 1. Weights are normalised to sum to 1.
  - **Memory:** the trend's magnitude is multiplied by a memory constant b > 1 when it continues the direction of the preceding memory window.
  - **Integration:** a decaying filter integrates the trend over time, sampled every 0.25 s.
  - **Features left out:** melodic expectation did not correlate directionally with tension, and purely linear regression fitted poorly.
- **MorpheuS** (Herremans & Chew 2017) [26]:
  - **Tension measures** are computed in Chew's spiral array: cloud diameter (dispersion of the notes in a time slice), cloud momentum (distance between consecutive centres of effect), and tensile strain (distance from the key's centre).
  - **Objective:** D(x) = Σ_i a_i · ‖T_i(x) − T_i(template)‖₂, with all a_i = 1. Repeated patterns from a template are hard constraints.
  - **Search:** variable neighbourhood search with three moves: change1 (re-pitch one note), swap (exchange two notes), changeSlice (re-pitch two notes in a time slice). It uses first-descent acceptance and backtracks 4 slices after each success.

---

## 6. Turning this into an algorithm

### 6.1 Models to build

1. **Chord model.** PPM-C with interpolated smoothing.
   - Alphabet: relative-root + quality symbols (and optionally extensions).
   - Order bound 2–4, or unbounded.
   - LTM: per-genre seed tables built from 1.1–1.5. Start from the counts in Table 3 of [1], and switch the tonic frame to minor for EDM [7].
   - STM: trained on the current song.
   - Combine LTM and STM by weighted geometric mean, b = 2 (inside Pearce's 1–4 range). [1][3][14][16]
2. **Melody and bass model.** Two viewpoints, each its own PPM: pitch interval ⊗ scale degree, and ioi-ratio. Combine them geometrically, and take the joint probability of pitch and onset. [15][17]
3. **Drum model.** One PPM per instrument lane over 16-step on/off tokens, conditioned on step position (step index as a linked viewpoint) [proposal]. Plus the Witek and LHL indices.
4. **Calibration.** Raw bits depend on alphabet size, so express every IC and H as a **z-score or percentile against the model's own corpus**, computed over the training corpus once at start-up. Gold's and Cheung's results are in standardised units, so targets stay comparable. [16][17]

### 6.2 Scores

For a candidate X with events x_1..x_n:
- **Surprise profile:** IC_i = −log2 p(x_i | context), and H_i before each event.
- **Mean complexity:** μ_IC = mean IC_i, and μ_H = mean H_i.
- **Placement score (Cheung):** Σ_i z(IC_i)·(−z(H_i)). This rewards surprises at low uncertainty and expected events at high uncertainty. [16]
- **Syncopation:**
  - LHL per layer, per bar.
  - Witek per 2-bar kit loop.
  - Fitness peaks at a target and falls off quadratically: f = −(S − S*)². [18][20][22]
- **Repetition:** fraction of the bar or chord n-grams (n = 4) that repeat earlier material, plus the STM-vs-LTM IC drop on a hook's return [proposal].
- **Total:** maximise the placement score, subtract weighted squared distances to the targets, and treat hard constraints (cadence, key, range, repeat ceilings) as filters.

### 6.3 Search

- **Generate-and-test:**
  - Sample K = 32–128 candidates per phrase from the model at temperature τ ∈ [0.8, 1.2], applying the exponent 1/τ to p.
  - Score them and keep the best.
  - This avoids the greedy-sampling trap Pearce describes, where every step is likely but the whole is not [14] [proposal for K, τ].
- **Beam search** over chords: beam 8–16, scored by the running objective (not raw likelihood). This allows look-ahead to the cadence.
- **Refinement:**
  - Metropolis–Hastings or VNS moves (change one event, swap two) on the best candidate.
  - Accept improvements. Accept MH moves with min(1, exp(Δscore/T)).
  - Keep hook n-grams fixed as hard constraints, as MorpheuS does. [14][26]
- **Arrangement:** compute a Farbood-style tension curve over sections (loudness, onset density, pitch height, harmony). Match a per-genre target curve with MorpheuS's objective. [25][26]

---

## 7. Gaps

- No verified Beatport corpus of **chord transitions** or **drum-pattern statistics** for house or techno. Search hits pointed to J. Park's 2025 Indiana dissertation on EDM-pop chord loops, which I did not read. The web-search budget ran out before I could look further, for example at Butler's *Unlocking the Groove*.
- No study measures **per-genre IC targets**. The genre bands below are proposals, built on standardised peaks from melodic and chord studies with Western listeners.
- Cheung's chords and Gold's melodies were rated in isolation, without production or dance context. Witek's breaks are funk breaks, not EDM.
- Hooktheory's full transition matrix sits behind an authenticated API. Only the documentation examples are cited.

---

## Rules and parameters

### Tempo & feel
- Default tempo medians from Beatport (crowd-corrected): deep house 122, tech/prog house 127, techno 127 (IQR 123–132), electro house 128.5, trance/dubstep/breaks 139, psy-trance 147, D&B 174, hard techno 170. [7][8]
- Humanise drums with a lag on 8th-note-grid hits and a lead on off-beat 16ths. Treat the size as a preset parameter, not a constant. [23]

### Drums
- Score the kit with the Witek index: weights step1 = 0; 5, 9, 13 = −1; 3, 7, 11, 15 = −2; even steps = −3. S = w(next) − w(note) + I, with I = 2 (kick syncopated against snare + hat), 1 (snare against kick + hat), 5 (two-stream). Sum over the 2-bar loop with wrap. [18]
- For groove-oriented kits (breaks, D&B, garage, funk-house), target a Witek index of 22–46 per 2-bar loop (Witek's "medium" bin), centre about 33. Reject loops above 49 (the "high" bin). [18]
- For four-on-the-floor genres, keep the kick + clap layer at Witek 0 (kick 1,5,9,13; clap 5,13). Put syncopation in the hats, percussion and bass layers instead (3.4). [1][18]
- Score each monophonic layer with LHL: weights 0,−4,−3,−4,−2,−4,−3,−4,−1,−4,−3,−4,−2,−4,−3,−4. Add w(silent max) − w(onset) when a silence of higher weight follows an onset. [19][20]
- Prefer moderate over maximal syncopation in any layer. Never anticipate every note. [21][22]

### Bass
- Default bass rhythm targets **[proposal]**: LHL 2–7 per bar. Off-beat 8ths = 7, tresillo 1,7,13 = 2, pushed 1,4,7,9,12,15 = 4. Use 16th-off-beat-only patterns (LHL 15) for at most 1 bar in 8. [20][22]
- Predict bass pitch with a pi⊗sd viewpoint PPM and onsets with an ioi-ratio PPM. Score with joint IC. [15]

### Harmony
- Train the chord LTM from the row-normalised rock table for rock and pop presets. Rock: I→IV .36, I→V .24, I→bVII .16, IV→I .56, V→I .53, bVII→I .53, VI→IV .39. [1]
- For pop presets, mix in Hooktheory's second-order priors: after IV→I, V .41, IV .20, vi .10. After iii, choose vi or IV at least 90% of the time. [3][4]
- Use a minor tonic for EDM presets by default, at 85% of seeds overall: deep house 95%, tech house and trance 88%, techno 82%, progressive house 76%, chill-out 64%. [7]
- Give I + IV + V + bVII + VI about 87% of chord mass in rock/pop presets. Use fewer than 1% diminished chords and keep about 94% of chords in root position. [1]
- Weight root motion by circle-of-fifths distance: P4 (either direction) > M2 > thirds > m2 > tritone. Make ascending and descending equally likely. [1]
- Use these trigram templates for cadences: IV–V–I, V–IV–I, bVII–IV–I, VI–IV–I, bVII–bVI–I (rates 352/292/146/126/103 of 100 songs). [1]
- Model chords with PPM-C plus interpolated smoothing, order ≤ 4 or unbounded. Combine LTM and STM by weighted geometric mean, w = (H/Hmax)^−b, with b = 2. [14][16]
- Place a high-IC chord (top 40% of corpus IC) only where entropy before it is in the bottom 40%: after a loop has been heard at least twice, or at a section end. [16]
- Where entropy is in the top 40%, pick a chord from the bottom 40% of IC. [16][17]
- Prefer acoustically smooth voicings over dissonant ones, since dissonance has an independent pleasure penalty of β −0.25 in Cheung's model. [16]

### Melody/hooks
- Target a phrase mean IC of z −1.0 to 0.0, and mean entropy of z −0.75 to −0.25, relative to the melody LTM's own corpus distribution. [17]
- Allow 1–2 events per 8-bar phrase with IC above the 80th percentile, each placed where H is below the 40th percentile and followed by an event with IC below the 40th percentile **[proposal, after 16]**. [16]
- Use viewpoints pi⊗sd plus ioi-ratio. Expect a well-fitted model to land around 2–2.5 bits/note on its own style. [14][15]

### Sound design
- Raise spectral richness rather than dissonance when a passage needs "more": in Cheung's model, spectral complexity has β +0.22 on pleasure and sensory dissonance β −0.25. [16]
- Master to modern loudness. The median rose from −22 to −13 dBFS from the 1950s to the 2000s, with about 9.5 dB inter-quartile spread. [10]

### Arrangement & energy
- Build the section tension curve from slopes of loudness and lead pitch height (weight 3), onset density and tempo (2), and harmony and bass/inner-voice pitch (1). Integrate over time, and boost a trend by a memory factor b > 1 while it continues. [25]
- Fit sections to a target tension curve with D(x) = Σ a_i‖T_i(x) − T_i*‖₂, with all a_i = 1. Keep hook patterns as hard constraints. [26]
- Make section ends low-entropy contexts (a repeated cadence or loop) so a surprise there pays off. [16]

### Variation/surprise
- Generate K = 32–128 candidates per phrase at temperature 0.8–1.2, score them, keep the best, then refine with change-one and swap moves (MH or first-descent VNS) **[proposal for K and τ]**. [14][26]
- Never pick the argmax-probability sequence. Mean IC below z −1.5 is "too predictable" and above z +1 is "too complex" **[proposal from 17's inverted U]**. [17]
- Let the STM make repeated hooks cheaper. When a returning hook's mean IC falls below z −1.5, vary 1 event in 4–8 (pitch or onset) until it is back in band **[proposal]**. [14][17]
- Repetition ceiling: a ≥5-chord sub-sequence repeats only after a gap of at least 2 chords, and at most 3 times in a row **(adapted from Cheung's stimulus constraint)**. [16]
- Keep exact repetition of refrains and hooks: listeners move and sing along more to exact refrains than to varied ones. Vary the surroundings, not the hook. [24]
- Weight vocabulary by corpus frequency (Zipf-like). Spend rare chords, sounds and rhythms sparingly, not uniformly. [6][10]

---

## References

1. de Clercq, T. & Temperley, D. (2011). "A corpus analysis of rock harmony." *Popular Music* 30(1). https://rockcorpus.midside.com/2011_paper/declercq_temperley_2011.pdf
2. Temperley, D. & de Clercq, T. (2013). "Statistical analysis of harmony and melody in rock music." *Journal of New Music Research* 42(3). https://www.midside.com/publications/temperley_declercq_2013.pdf
3. Hooktheory. "Trends API documentation" (example responses for trends/nodes). https://hooktheory.com/api/trends/docs
4. Hooktheory (2012). "I analyzed the chords of 1300 popular songs for patterns. This is what I found." Hooktheory blog. https://www.hooktheory.com/blog/i-analyzed-the-chords-of-1300-popular-songs-for-patterns-this-is-what-i-found/
5. Burgoyne, J. A., Wild, J. & Fujinaga, I. (2011). "An expert ground-truth set for audio chord recognition and music analysis." *Proc. ISMIR 2011*. http://ismir2011.ismir.net/papers/OS8-1.pdf
6. Moss, F. C., Neuwirth, M., Harasim, D. & Rohrmeier, M. (2019). "Statistical characteristics of tonal harmony: A corpus study of Beethoven's string quartets." *PLOS ONE* 14(6): e0217242. https://doi.org/10.1371/journal.pone.0217242
7. Knees, P., Faraldo, Á., Herrera, P., Vogl, R., Böck, S., Hörschläger, F. & Le Goff, M. (2015). "Two data sets for tempo estimation and key detection in electronic dance music annotated from user corrections." *Proc. ISMIR 2015*. Data: https://github.com/GiantSteps/giantsteps-key-dataset and https://github.com/GiantSteps/giantsteps-tempo-dataset (minor/tempo statistics computed for this document).
8. Schreiber, H. & Müller, M. (2018). "A crowdsourced experiment for tempo estimation of electronic dance music." *Proc. ISMIR 2018* (annotations_v2 in the GiantSteps tempo repository above).
9. Mauch, M., MacCallum, R. M., Levy, M. & Leroi, A. M. (2015). "The evolution of popular music: USA 1960–2010." *Royal Society Open Science* 2: 150081. https://arxiv.org/abs/1502.05417
10. Serrà, J., Corral, Á., Boguñá, M., Haro, M. & Arcos, J. L. (2012). "Measuring the evolution of contemporary western popular music." *Scientific Reports* 2: 521. https://arxiv.org/abs/1205.5651
11. Huron, D. (2006). *Sweet Anticipation: Music and the Psychology of Expectation.* MIT Press. (ITPRA: p. 17, Fig. 1.1.)
12. Schwab-Felisch, O. (2013). Review of Huron, *Sweet Anticipation*. *ZGMTH* 10/2, 389–399. https://doi.org/10.31751/731
13. "Daniel Berlyne." Wikipedia (citing Weiner & Craighead, *Corsini Encyclopedia of Psychology*, 2010). https://en.wikipedia.org/wiki/Daniel_Berlyne
14. Pearce, M. T. (2005). *The construction and evaluation of statistical models of melodic structure in music perception and composition.* PhD thesis, City University London. https://openaccess.city.ac.uk/id/eprint/8459/
15. Pearce, M. T. (2018). "Statistical learning and probabilistic prediction in music cognition: mechanisms of stylistic enculturation." *Annals of the New York Academy of Sciences* 1423: 378–395. https://doi.org/10.1111/nyas.13654
16. Cheung, V. K. M., Harrison, P. M. C., Meyer, L., Pearce, M. T., Haynes, J.-D. & Koelsch, S. (2019). "Uncertainty and surprise jointly predict musical pleasure and amygdala, hippocampus, and auditory cortex activity." *Current Biology* 29. https://doi.org/10.1016/j.cub.2019.09.067 (PDF: https://www.stefan-koelsch.de/papers/cheung_harrison_meyer_pearce_haynes_koelsch_2019_uncertainty_and_surprise_jointly_predict_musical_pleasure_and_amygdala_hippocampus_and_auditory_cortex_activity_current_biology_29.pdf)
17. Gold, B. P., Pearce, M. T., Mas-Herrero, E., Dagher, A. & Zatorre, R. J. (2019). "Predictability and uncertainty in the pleasure of music: a reward for learning?" *Journal of Neuroscience* 39(47): 9397–9409. https://doi.org/10.1523/JNEUROSCI.0428-19.2019
18. Witek, M. A. G., Clarke, E. F., Wallentin, M., Kringelbach, M. L. & Vuust, P. (2014). "Syncopation, body-movement and pleasure in groove music." *PLOS ONE* 9(4): e94446, incl. supplementary Text S2, S4, S6, Table S1, Figs S5–S6; correction (2015) doi:10.1371/journal.pone.0139409. https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0094446
19. Longuet-Higgins, H. C. & Lee, C. S. (1984). "The rhythmic interpretation of monophonic music." *Music Perception* 1(4): 424–440 (as described in [18], [20]).
20. Fitch, W. T. & Rosenfeld, A. J. (2007). "Perception and production of syncopated rhythms." *Music Perception* 25(1): 43ff. https://web.uvic.ca/~aschloss/course_mat/MUS%20511/ARTICLES%20AND%20REFS%20FOR%20320/FitchRosenfeld20071.pdf
21. Sioros, G., Miron, M., Davies, M., Gouyon, F. & Madison, G. (2014). "Syncopation creates the sensation of groove in synthesized music examples." *Frontiers in Psychology* 5: 1036. https://doi.org/10.3389/fpsyg.2014.01036
22. Matthews, T. E., Witek, M. A. G., Heggli, O. A., Penhune, V. B. & Vuust, P. (2019). "The sensation of groove is affected by the interaction of rhythmic and harmonic complexity." *PLOS ONE* 14(1): e0204539. https://doi.org/10.1371/journal.pone.0204539
23. Gillick, J., Roberts, A., Engel, J., Eck, D. & Bamman, D. (2019). "Learning to groove with inverse sequence transformations." *Proc. ICML 2019*. https://arxiv.org/abs/1905.06118
24. Margulis, E. H. (2014). "One more time." *Aeon*, 7 March 2014. https://aeon.co/essays/why-we-love-repetition-in-music (see also Margulis, *On Repeat: How Music Plays the Mind*, Oxford University Press, 2014).
25. Farbood, M. M. (2012). "A parametric, temporal model of musical tension." *Music Perception* 29(4): 387ff. https://bpb-us-e1.wpmucdn.com/wp.nyu.edu/dist/f/11865/files/2020/08/Farbood_2012_Musical_Tension.pdf
26. Herremans, D. & Chew, E. (2017). "MorpheuS: generating structured music with constrained patterns and tension." *IEEE Transactions on Affective Computing*, doi:10.1109/TAFFC.2017.2737984. https://arxiv.org/abs/1812.04832
