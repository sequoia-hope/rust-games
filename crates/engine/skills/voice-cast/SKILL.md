---
name: voice-cast
description: Build a spoken-word character for a game (a radio DJ, a dispatcher, a narrator) with the Playground Kit voice pipeline: write the character sheet, design three TTS voice candidates, record the lines, check them with Whisper, audition on the page, and wire the in-game director. Use when asked to add or re-record a DJ, a station voice, radio chatter, voice lines, or to fix lines that "don't land".
---

# Voice cast: a character who talks on the radio

Today the pipeline lives in Midnight Playground (`~/Software/midnight-playground`,
read paths below relative to it). In a later kit phase it moves to
`tools/voice` here with `design`, `render`, `verify` (Whisper) and `audition`
commands and a `cast.json` + `script.json` schema (PLAN.md `pk_voice`).
Until then, use the game's scripts; the procedure is the same.

## Read first

- `tools/dj-voice/README.md`: the side quest, the setup, what is committed.
- `tools/dj-voice/lines.json` (the script) and `tools/dj-voice/voices.json`
  (candidates and `chosen`), then `design.py` and `render.py` headers.
- `docs/vision/radio.md` sections 2 (rules for talking), 3 (reacting to the
  player), 4 (data and files), 8 (favourites: the DJs know the records).
- `docs/vision/WORLD.md` 1.1: "depth from memory, not from script".
- `docs/rust-port/DECISIONS.md` D1184 (Tess) and D1221 (Dez): two characters
  built end to end, the model for the next one.
- `crates/mp_game/src/play/station.rs`: the director (`BREAK_GAP`,
  `BREAK_CHANCE`, `IDENT_CHANCE`, `HISTORY`, the quiet rule in `frame`).
- `tools/radio-voice/` for the handset variant (police dispatch).

## The data shapes

`voices.json`: per character, `ref_text` (a 2 to 4 sentence sample the
candidate speaks and the clone is prompted with), `candidates` (name to
plain-English description), and `chosen`. `lines.json`: `topics` (key to
label, in order) and `djs.<key>` with `name`, `station`, `about` (the
character sheet) and `lines.<topic>`: a list of strings, or objects with
`text` and `template`. A clip id is `<dj>-<topic>-<n>`. A blank line
(`\n\n`) inside a text splits it into parts recorded separately and joined
with a 0.45 s pause. `audio/dj/index.json` records `voices` and, per clip,
`takes`, `text`, `voice`: that is what makes re-runs incremental.

## Procedure

1. **Character sheet first**, from the owner's brief (D1184 is the shape:
   age, work, where they broadcast from, who they love, what they fix,
   what they will not explain, their edge). Write it as `about` in
   `lines.json`. The owner's brief is often a paragraph of fragments;
   turn it into one grounded life, named people and places included.
2. **The lines**: about 70 per character across the fifteen topics in
   `lines.json` (ident, music, morning, weather, places, scene, player,
   raves, tired, camping, offair, callers, ads, banter, night). Idents
   say the station and frequency as words ("one-oh-one point seven").
   `player` lines are templates with slots in brackets and one filled
   example as `text`. Dez's `songs` lines about favourites (radio.md 8) are
   the same pattern per `(genre, seed)`.
3. **Three voice candidates** in `voices.json`, each a one-paragraph
   description (age, register, pace, accent, where they are speaking from,
   the mood). Pick a `chosen` yourself when nobody can listen, and say so;
   the owner picks on the page and a change of `chosen` re-records only
   that character.
4. **Design and render** (needs a Python with `qwen-tts`, a CUDA GPU with
   about 5 GB free, ffmpeg; run from the repo root):

   ```bash
   python tools/dj-voice/design.py                 # candidates not designed yet
   python tools/dj-voice/design.py dez-smooth      # redo one
   python tools/dj-voice/render.py --only dez --takes 1
   python tools/dj-voice/render.py --only kit-morning --seed 12   # a different take
   python tools/dj-voice/render.py --takes 3        # a third take of every line
   python tools/dj-voice/render.py --force          # everything again
   ```

   `render.py` records only what is stale: a new or edited `text`, or a
   character whose `chosen` changed (compared against `index.json`), plus
   any take file missing on disk. Clips whose id left the script are
   deleted. Levelling: trim at 38 dB, speech to -18 dBFS RMS, peaks capped
   at -1 dBFS, 24 kHz mono 64 kbit/s MP3 for the DJ (a car stereo plays it
   full range). The police radio (`tools/radio-voice/render.py`) encodes
   16 kHz 32 kbit/s because the game's radio bus keeps only 300 to 3000 Hz;
   its script comes from `cargo run -p mp_audio --example radio_lines`.
5. **Verify with Whisper by hand** (today there is no `verify` command):
   transcribe every new clip with `faster-whisper` and compare to its
   `text` (a token match ratio). Expect a mean near 0.96; low scores that
   are digits ("94.9") or US spellings are fine; a garbled long line is
   split further at a blank line and re-rendered.
6. **Audition** through the registered server, never another one:

   ```bash
   proj up midnight-playground     # then open /tools/dj-voice.html
   ```

   The page plays takes side by side, has **Car stereo** mode (a 7 kHz
   low-pass, forward mids, a little squash: what the game sounds like),
   **A random radio break**, and 👍/👎 per line kept in the browser; **Copy
   my notes** gives the owner a paste for the rewrite round. Tell the owner
   it is ready and which candidates to compare.
7. **Rewrite from the notes.** Every 👎 line is edited in `lines.json`
   (the id keeps its number if you keep its position) and `render.py`
   records just those. Commit only the takes the owner kept (the committed
   set is one take per line, `takes: 1` in `index.json`).
8. **Wire the station**: `"dj": "dez"` on its entry in the game's
   station table (Midnight Playground: `crates/mp_audio/stations.json`); the director handles any DJ with clips
   in `index.json`. A station with no clips plays music only.
9. **DECISIONS entry** (see the `decisions-log` skill): the brief, the
   sheet in one paragraph, the three candidates and which was chosen and
   why, what was recorded and what the owner has not heard yet.

## Tone rules the owner set

- **PG, always.** Partners, nightlife, the fetish fair: spoken of plainly
  and kept PG. The life is adult; the words are not.
- **Defended by craft, not slogans.** A character's identity is carried by
  what they make and do (harnesses sewn for the crew, a well pump rebuilt
  twice, nine hundred photographs of one heron), never by announcements.
- **Kind, with one edge.** Each character has one thing they are short
  with (being second-guessed; someone hurting their friends), stated dry
  and never cruel: "he's not coming back and the bouncer was very polite".
- **A licensed station does not cheer a street race.** The DJ nods at the
  driver ("That's a compliment here"); the night lines circle what they
  will not say on air.
- **Banter places the dial**: each DJ names the others as the harbour, the
  hill, the farm, the bar, the loft.

## Does the talk land? (WORLD 1.1)

- **Sparse.** The director speaks at a song change with probability
  `BREAK_CHANCE` (0.35), at least `BREAK_GAP` (150 s) apart, an ident
  within 2 s of tuning in half the time, never in the menus, never during
  a countdown, and not within 4 s of the police radio (`quiet`). A break
  is one line: `music` 70 %, `ident` otherwise. The last `HISTORY` (8)
  ids are not repeated. Keep these numbers; make a setting if the owner
  wants more or less.
- **Reactive.** `player` lines are about what this player did (the car,
  the colour, the road, the record). Pre-render every slot combination
  (radio.md 3); stitching fragments is the fallback and sounds like one.
- **Little and good.** The test is the owner listening on the page before
  anything is wired. Chatter that tries to be deep shows no depth.

## Qwen3-TTS voice design & ICL cloning notes

When designing reference clips (`Qwen3-TTS-12Hz-1.7B-VoiceDesign`) and cloning lines from them (`Qwen3-TTS-12Hz-1.7B-Base` in `x_vector_only_mode=False` ICL mode, as used in `games/snowboarding/tools/announcers/` and `games/midnight-playground/tools/dj-voice/`):

- **Anchor vocal register in `instruct`, not with exclamation marks.** In `VoiceDesign`, combining words like `"loud, shouting, excited, high-energy"` in `instruct` with `!` on every sentence of `ref_text` causes fundamental frequency (F0) to jump up an octave into a shrill 380–450 Hz cheerleader archetype (which clusters with Midnight Playground's `kit-bright` at 0.50–0.80 cosine similarity). To get an energetic broadcaster who stays grounded, specify the chest/alto/contralto register and anti-shrill constraint explicitly in `instruct` (e.g., `"deep, husky contralto voice, rich chest resonance... speaking at a brisk, punchy, confident clip in her lower register, smiling and energetic without ever sounding shrill or high-pitched"`) and use periods/commas in the setup sentences of `ref_text`, reserving `!` for the climax at the end.
- **Structure `ref_text` as a rising arc (the Tail-Energy Rule).** In `Base` ICL mode (`x_vector_only_mode=False`), `generate_voice_clone` prepends `ref_code` (the 12 Hz codec tokens of `ref_audio`) and generates target speech tokens immediately after the end of `ref_code`. Because of causal self-attention recency bias, the energy, projection, and pace of generated clips—especially short 1–3 s action calls—are dominated by the **final 2–3 seconds** of `ref_audio`. If `ref_text` puts the excitement in the middle and trails off with a calm transition (`"Here comes the next rider out of the gate, let's see what she's got"`), short action lines will come out flat and conversational. End `ref_text` at peak broadcast excitement (`"...and she stomps the landing clean! Yes, what a run!"`).
- **Avoid double-cloning.** Cloning a `VoiceDesign` reference clip onto a second reference clip (`"clone"` in `voices.json`) and then cloning that clone a second time in `render.py` washes out vocal texture (rasp, chest resonance, regional accent) and pulls distinct speakers toward the same generic audiobook narrator timbre. Design the chosen reference clip directly with `VoiceDesign` on the rising-arc `ref_text`.
- **Tighten internal pauses (`max_pause_s`) and avoid `"breathy / never raises her voice"`.** Prompting `VoiceDesign` with `"a little breathy, deadpan, never raises her voice"` produces sleepy delivery with 350–600 ms pauses and audible sighs that ICL cloning copies into game clips. Instead, ask for `"clear, crisp, agile voice with a bright, dry, articulate edge—no huskiness and no breathiness"`, and use `max_pause_s` (e.g. `0.14` in `snowboarding/tools/announcers/design.py`) to trim leading/trailing dead air and cap internal pauses to brisk broadcast pacing.
- **Keep `ref_text` aligned with `ref_audio` and cap `max_new_tokens`.** In ICL mode, `ref_text` must exactly match the words spoken in `ref_audio`; if `ref_audio` is sliced or differs from `ref_text`, text-to-codec alignment breaks and the model may babble or fail to emit `eos_token_id` (running up to the default `max_new_tokens=2048`, ~170 s at 12 Hz, which stalls the batch and can OOM a 12 GB GPU). Keep `max_new_tokens=240` (~20 s) in `render.py`.
- **Check speaker contrast with `compare.py`.** Center speaker embeddings on the pool mean and verify that new voices stay $\le 0.35$ cosine similarity against Midnight Playground's DJs (`tools/announcers/compare.py ../midnight-playground/tools/dj-voice/voices`) and separate clearly from co-hosts in both embedding space and median pitch.

## Common mistakes

- Starting a second HTTP server to hear the page. Use `proj up`.
- Editing a line's position: ids are positional, so inserting mid-topic
  renumbers and re-records everything after it. Append instead.
- Forgetting `--takes 1` and recording two takes of 70 lines on a 5 GB GPU.
- Writing digits or abbreviations in a text: the TTS says them oddly.
  Spell "ninety-four point nine", "Highway Eighty-Four".
- A bilingual voice: test the three candidates' Spanish before rendering
  seventy lines (Teo's `ref_text` mixes both on purpose).
- Committing `voices/*.flac` for candidates nobody chose; commit the three
  and the clips the owner picked, with `index.json`.
- Claiming the owner heard it. Say what was checked (Whisper) and what was
  not (ears).
