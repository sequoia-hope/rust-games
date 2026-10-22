# Changelog: the music engine (crates/music)

The music engine is shared by the games here and may be used by games
outside this repo. Every breaking change goes here, with how to migrate;
notable additions too. Newest first.

## Unreleased

### The radio plays the new generator (2026-10-22)

The owner's go-ahead for PLAN.md's Phase 3 gate ("a station switches to
the new generator only when the owner says so").

- **A station's genres may be the new generator's** (`synthwave`,
  `outrun`, `chicago`, `deep`) beside the classic grammars', in any mix;
  `Station::parse_list` accepts both.
- **Breaking: `radio::Slot::track` and `radio::Cue::track` are now
  `piece: radio::Piece`**, either `Piece::Track(Box<Track>)` (a classic track,
  as before) or `Piece::Song { genre, seed, order, bpm, bars }` (a new
  song, planned from its shape and made only when it plays, by
  `Piece::song()`). Migrate `slot.track.bpm` to `slot.piece.bpm()`, and
  `bars()`, `title` and `style` likewise; `piece.track()` gives the classic
  track where there is one. (Midnight Playground's now-playing line was
  the one reader outside the crate; it is migrated.)
- **A new song is fitted to its block** by `radio::fit_song`: whole
  sections repeated or left out, then the tempo nudged (under 8 %).
- `score::Song::rearrange` (a song's sections in another order),
  `generator::shape` (a song's tempo and sections without making it) and
  `song::SongPlayer::cue` (start on a 16th, mid-song) are added for it.
- `Player` runs a `SongPlayer` beside each `Engine`. A new song is made on
  the audio thread when it starts (about 4 ms native), once per song.
- Stations with only classic genres play exactly what they did.

### New lead, pluck and bass patches, and measured unison (2026-10-22)

- **Five base patches added** (`patches.rs`), so every genre can reach them:
  `fmBass` (a 4-operator stack bass), `modernPluck` (a 5-voice Juno pluck),
  `leadGuitar` (a plucked-string lead with bend and delayed vibrato),
  `vocalLead` (a Juno lead through the vowel formant) and `celloPad` (a bowed
  pad). `celloPad` is reachable from the Music Lab but referenced by no
  profile yet, as `flute`, `twang` and `whistle` already are.
- **`JunoVoice` unison is the measured shape, not a straight ramp**
  (`instruments.rs`): 7-voice unison uses the measured JP-8000 / Serum
  asymmetric spread, and other voice counts keep the old power curve with the
  positive side pulled in slightly, so the pairs either side of centre no
  longer beat in phase lock. The oscillator mix is centre-weighted (1.25 at
  the centre, 0.85 either side) and normalised by the RMS of those weights
  rather than `sqrt(n)`, which keeps the patch level where it was.
- **Patches retuned:** `supersaw` (darker and louder, with vibrato and glide),
  `superPadWide`, `tranceBass`, `piano`, `choir` (5-voice, formant softened)
  and `pluckTrance`.
- **Genre pools widened** (`generator/profile.rs`): `SYNTHWAVE`, `OUTRUN`,
  `CHICAGO` and `DEEP` draw on the new patches; `OUTRUN` gets its own bass and
  arp pools instead of inheriting `SYNTHWAVE`'s. Gated reverb is pulled back
  from the full-wet snare (`SYNTHWAVE` `1.0`/`0.1` to `0.55`/`0.08`, `OUTRUN`
  `0.45`/`0.06`) and the `SYNTHWAVE` pad sits 1 LU lower.
- `LAB_SHA256` and the track and render goldens are regenerated. Measured
  after the change: `synthwave` −23.4, `outrun` −21.4, `chicago` −23.1 and
  `deep` −23.7 LUFS integrated, true peak −6.3 to −8.1 dBFS, so the louder
  patches and the new normalisation cost no headroom.

### Engine-wide patch, kit, DSP, and house/synthwave upgrade (2026-10-22)

- **Upgraded base patches (`patches.rs`), kits (`kits.rs`), and DSP defaults
  (`dsp.rs`) across all genres:**
  - `dsp::Chorus` defaults to the hardware-measured Juno-60 BBD timing
    (`±0.5 ms` mode I, `±1.1 ms` mode II, `15 kHz` low-pass) for all genres.
  - `JunoVoice::start` and `FmVoice::start` only pitch-glide on legato notes
    when `p.glide` is explicitly set, fixing unintended pitch slides on FM
    pianos, electric pianos, organs, bells, and Juno plucks; `FmVoice::start`
    preserves operator phase when retriggering active voices to prevent clicks.
  - Base patches (`sawBass`, `pluckBass`, `reese`, `sub`, `darkBass`, `acid`,
    `roundBass`, `superPad`, `warmPad`, `darkPad`, `stab`, `hit`, `ep`, `bell`,
    `glass`, `pluck`, `sqArp`, `brassLead`, `sawLead`, `sqLead`, `organ`,
    `dubChord`, `supersaw`, `superPadWide`, `tranceBass`, `psyBass`,
    `euroLead`, `piano`, `pluckTrance`, `honkyPiano`) and base kits (`tr808`,
    `tr909` with 6-bit sampled cymbals, fuller snares, and natural tom pitch
    sweeps) are upgraded directly, and golden hashes/renders regenerated.
- **House and harmonic generator fixes (`generator/`):**
  - `theory::voice` preserves the chord root (`0`) in 4-note voicings and
    places 9ths (`14`) above the 3rd in the upper octave instead of forming
    1-semitone `[2, 3]` clusters on minor-9th chords; `parts::pool` keeps
    extensions consistent across repeated numerals in a progression.
  - `parts::sparse` retains every second event when downbeat filtering would
    empty offbeat syncopated patterns (`ChordStyle::Stabs`).
  - `CHICAGO` and `DEEP` profiles tuned (`CHICAGO_DRUMS` rimshot, `DEEP_DRUMS`
    subtle low tom, `DEEP` `roundBass`/`sub` and `dubChord`/`stab`/`ep`
    stabs with stereo ping-pong delay and reverb, chord-following `darkPad`
    drone).

### Synthwave and outrun instrument tune-up (2026-10-21)

- **12 dB/oct ladder tap (`slope: 12`):** `dsp::Ladder::run` adds `mode == 2`
  (2-pole low-pass tap for Oberheim SEM / OB-Xa / airy Juno filter roll-off);
  `track::Lab` adds `slope` (`"slope"`), read by `JunoVoice` and `MonoVoice`.
- **Chorus bass high-pass (`chorusHp`):** `dsp::Chorus::set_hp` high-passes
  the BBD delay input while leaving the dry signal full-range (`track::Lab`
  `chorus_hp` / `"chorusHp"`), so bass fundamentals below 180 Hz stay solid
  mono while upper harmonics spread in stereo (`synthwave.md` §4.4).
- **Mono PWM:** `MonoVoice::run` now modulates pulse width from `p.pwm` and
  the instrument LFO.
- **Bus mix per song:** `score::Song` carries `tape`, `glue` and `reverb`
  from `Profile`, applied by `SongPlayer::set_song`.
- **Synthwave and outrun voicings (`profile::sound_patch`, `kit_tweak`,
  `parts`):**
  - Plucked Prophet-5 / Juno-60 8th-note bass (`sawBass`, `darkBass`,
    `pluckBass`; `roundBass` dropped from synthwave/outrun) with saw + PWM
    pulse + upper-octave saw + phase-locked sub, snappy filter decay, low
    keytrack, and high-passed Chorus I.
  - Juno-60 / OB-Xa arpeggio plucks (`pluck`, `sqArp`) with saw + PWM pulse
    unison, 12 dB/oct filter, 400 ms filter envelope, and Chorus II; `parts::arp`
    always spells 5 ascending chord tones in the midrange (`MIDI 55..76`,
    fixing the 4-tone triad modulo wrap and extra `+12` octave shift) in a
    continuous 8-note bar.
  - Lush Juno/OB-Xa pads (`warmPad`, `superPad`, `darkPad`) with saw + PWM
    pulse, no sub-oscillator mud, 180 Hz HPF, 12 dB/oct filter, and Chorus II.
  - Singing solo leads (`sawLead`, `sqLead`, `brassLead`) with stereo Chorus II,
    12 dB/oct filter, delayed vibrato, and legato portamento (`slide = true`)
    with boundary-reflecting scale motion in `parts::draft`.
  - 80s LinnDrum / Simmons drum voicing on `tr808` and `tr909` (`kit_tweak`):
    tight punchy kick, full-bodied gated snare, backbeat clap on 2 & 4
    (replacing the bar-2 16th clap burst), 8-bit sampled cymbals, and
    pitch-swept Simmons toms with accented descending fills.

### Synthwave balance (2026-10-19)

- **`Profile::levels`**: per-role level offsets (dB) on the generator's
  channel levels. Synthwave and outrun raise the arpeggio 12 dB and the
  lead 7 dB: in the chorus, K-weighted, the arpeggio had sat 19 LU under
  the bass and the lead 11; now about 6 and 4. House is unchanged.
- The intro lift is for beds only (pads, atmosphere), no longer the
  arpeggio.
- Struct literals of `Profile` need the new field (`levels: &[]`).

### Synthwave openings (2026-10-19)

- **`profile::Intro`** and **`Profile::intros`**: ways a song can open,
  drawn per song by weight, each replacing the template's first cells per
  role. Synthwave and outrun open on pads and the arpeggio, on the bass
  alone, behind an opening filter, or (least often) on the drums, after the
  sourced reference intros (`research/genres/synthwave.md` §4.8); house
  keeps its DJ intro on the kick. Pads play +12 dB while exposed before
  the kick.
- A run of filtered blocks now opens its filter in one sweep, not one per
  block (no current template had such a run).
- Struct literals of `Profile` need the new field (`intros: &[]` keeps
  the template's opening).

### Sound accuracy for synthwave and house (2026-10-19)

Additions, all opt-in: with none of the new parameters set, every sound is
bit-for-bit what it was (the golden renders pass unchanged), so the games'
stations sound the same. From `research/RULES.md`, "What the engine
lacks" #10, #14 and #18.

- **Gated reverb:** `dsp::GatedVerb` (a hall with no pre-delay, cut a
  hold after each keyed hit); `score::Gated` and `Layer::gated` give a
  layer its own, keyed by its hits, in `SongPlayer`. `dsp::Reverb` has a
  `pre_s` field (18 ms, as before).
- **Sampled cymbals:** kit voices take `bits`, `clock` and `dacLp`: the
  metal and cymbal voices requantised (the 909's and 707's 6-bit
  cymbals), the envelope after the converter, then a low-pass the clock
  also bounds.
- **Juno chorus depth:** patches take `chorusMs` (the sweep, ± ms) and
  `chorusLp` (Hz); `dsp::Chorus::model`. Unset, the chorus keeps its
  ±1.7 ms and 9 kHz.
- **The new generator** uses them: synthwave and outrun put the snare
  wholly into a gated room and the kick a little into its own
  (`Profile::gated`); 909 kits get sampled hats, ride and crash
  (`profile::kit_tweak`); every chorus runs at the measured ±0.5 ms (I)
  or ±1.1 ms (II) through 15 kHz.
- Struct literals of `Layer`, `Profile` and `KitVoice` need the new
  fields (`Layer::new` and `KitVoice::NONE` fill them).

### The new generator (2026-10-19)

Additions only; the classic generator, the radio and the worklet's radio
exports are unchanged.

- **`score`**: a song as per-instrument layers (`Song`, `Layer`, `Clip`,
  `Pattern`, `Note`, `Automation`, `Fx`, `Section`): clips on phrase
  boundaries, patterns of any length, notes at fractional 16ths with their
  own velocity, automation lanes, a sidechain depth and an energy
  threshold per layer. `Song::summary()` is its JSON for a page.
- **`song::SongPlayer`**: plays a `Song` on the lab's instruments, kits and
  mix (per-layer pump, bus filters, risers).
- **`generator`**: `generate(genre, seed, &Settings)` for the profiles in
  `generator::profile` (`synthwave`, `outrun`, `chicago`, `deep`), built
  from `research/RULES.md`; `generator::theory` (modes, numerals,
  voicings) and `generator::surprise` (the PPM expectation model, IC and
  entropy scores, the LHL and Witek syncopation indices).
- **Lab:** the `song` message and `lab::song_json`; worklet export
  `music_song`; `music_tables` lists the new generator's genres as
  `generators`.
- **`examples/render.rs`:** any song, new or classic, to a WAV file.

### The lab on this engine (2026-10-19)

Additions only.

- **`lab`**: a [`lab::Session`] is an engine driven by the Music Lab page's
  messages (JSON: track, play, stop, patch, kit, audition, on, off, hit,
  energy, mix, mute, solo); `lab::generate(genre, seed, dejavu)` and
  `lab::tables_json()` serve the page. The page is `apps/music-lab/`.
- **`Engine`**: the lab keyboard and pads, which the port had left out:
  `set_audition`, `note_on`, `note_off`, `hit`, and `set_kit_tweak`
  (per-lane kit voice edits). A game that never calls them renders as
  before.
- `Instrument::release(midi)`; `Lab::from_val`, `OscSpec::from_val`,
  `Op::from_val` (a patch from its JSON).
- **Worklet ABI:** `music_tables`, `music_generate`, and the lab node's
  `music_lab_new`, `music_lab_msg`, `music_lab_process`, `music_lab_poll`,
  `music_lab_free`.

### Renamed `music`; stations come from the game (2026-10-19)

Breaking, for every consumer; the first step of the redesign (`PLAN.md`).

- **The package is `music`** (it was `mp_music`): `use mp_music::…`
  becomes `use music::…`, the dependency `mp_music.workspace = true`
  becomes `music.workspace = true`, and the worklet wasm is `music.wasm`.
- **Stations are data.** `radio::STATIONS`, `radio::station` and
  `radio::level_station` are gone: they were Midnight Playground's. A
  `radio::Station` now owns its fields (`String`s, `Vec<(String, f64)>`
  genres) and is read from JSON with `Station::parse_list` (written by
  `Station::list_json`). Migrate: put your table in a JSON file, parse it
  once, and keep your own `station(key)` and level map.
- **`Player::new(seed, sample_rate, stations)`** takes the table; the
  `station` param indexes it. `Player::stations()` reads it back.
- **The worklet ABI** is renamed and takes the table first: `mpm_*`
  became `music_*` (`music_new`, `music_params`, `music_process`,
  `music_free`, `music_stations`, `music_schedule`, `music_text_len`), and
  `music_alloc` + `music_set_stations(ptr, len)` hand it the JSON
  (`music_text` holds the error when it returns -1).
- **One shim for every game:** `web/music-worklet.js`, registering
  `music-radio`, with `processorOptions` `{module, seed, stations}`
  (`stations`: the JSON as a `Uint8Array`). Copy it beside your page with
  `music.wasm`; the games' own copies are gone.
- **Added** `json::parse` (and `Val::get`, `as_str`, `as_f64`, `as_arr`).
- Snowboarding's fork, `sb_music`, is deleted: it now plays this crate
  with its own table.

### Moved out of Midnight Playground (2026-10-18)

- **Where:** `rust-games/crates/music/` (it was
  `midnight-playground/crates/mp_music`). The package is still named
  `mp_music`, so `use mp_music::…`, the profile overrides and the worklet
  wasm's file name (`mp_music.wasm`) are unchanged. A rename is left to the
  music engine's redesign.
- **Goldens:** the Music Lab goldens moved from Midnight Playground's
  `parity/golden/music/` to `tests/golden/` here; the lab
  (`games/midnight-playground/tools/music-lab/test/golden.mjs`) writes them
  there. No code change.
