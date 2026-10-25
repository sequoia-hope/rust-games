"""Design the announcers' voices with Qwen3-TTS.

Every announcer in voices.json has a few candidate voices. A candidate is
either a plain-English description (VoiceDesign) or another candidate's
voice cloned (the Base model); either way this speaks the candidate's
ref_text (the announcer's, unless it has its own) and saves
voices/<candidate>.flac: the reference clip render.py clones every line
from, so each announcer's whole script comes out in one consistent voice
and delivery. Set "chosen" in voices.json, then run render.py.

    python tools/announcers/design.py               # every candidate not designed yet
    python tools/announcers/design.py dani-pipe     # just these (re-designs them)
    python tools/announcers/design.py --force       # every candidate again

Qwen3-TTS VoiceDesign & ICL reference tuning notes:
  1. Register vs. exclamation marks: In Qwen3-TTS-12Hz-1.7B-VoiceDesign,
     combining "loud / shouting / excited" in `instruct` with '!' on every
     sentence in `ref_text` causes F0 to jump an octave into a shrill
     380-450 Hz cheerleader archetype (clustering with midnight-playground's
     Kit at 0.5-0.8 similarity). To get an authentic action-sports booth
     voice, anchor the vocal register explicitly in `instruct` (e.g. "deep,
     husky contralto voice, rich chest resonance... speaking at a brisk,
     punchy, confident clip in her lower register, smiling and energetic
     without ever sounding shrill") and use periods/commas in setup clauses,
     reserving '!' for the climax at the end of `ref_text`.
  2. Rising-arc `ref_text` (the Tail-Energy Rule): `render.py` uses Base
     in-context learning (ICL) voice cloning (`x_vector_only_mode=False`),
     which prepends `ref_code` and generates target speech tokens immediately
     after the end of `ref_code`. Due to causal self-attention recency bias,
     the energy, pace, and projection of cloned clips—especially short 1-3 s
     trick and crash calls—are dominated by the final 2-3 seconds of
     `ref_audio`. Always end `ref_text` at peak landing excitement rather
     than trailing off into a calm transition ("Here comes the next rider...").
  3. Pause tightening (`max_pause_s`): VoiceDesign often inserts 350-600 ms
     clause pauses or trailing breaths that ICL cloning copies into game
     clips. Setting `"max_pause_s": 0.14` on a candidate trims leading/trailing
     silence and caps internal pauses to brisk broadcast pacing.
  4. Avoid double-cloning: Cloning a VoiceDesign clip onto a second reference
     clip (`"clone"` in voices.json) and then cloning that clone again in
     `render.py` washes out vocal texture (rasp, chest resonance, accent)
     and pulls different speakers toward the same generic narrator timbre.
     Prefer direct VoiceDesign candidates with rising-arc `ref_text`.

Borrowed from midnight-playground's tools/dj-voice/. Needs a Python with
qwen-tts (https://github.com/QwenLM/Qwen3-TTS), librosa, and a CUDA GPU with
about 5 GB free.
"""
import argparse
import json
import time
from pathlib import Path

import librosa
import numpy as np
import soundfile as sf
import torch
from qwen_tts import Qwen3TTSModel

HERE = Path(__file__).resolve().parent
DESIGN_MODEL = 'Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign'
CLONE_MODEL = 'Qwen/Qwen3-TTS-12Hz-1.7B-Base'


def ref_text(cfg, who, cand):
    """What a candidate's clip says."""
    return cfg['announcers'][who]['candidates'][cand].get('ref_text') or cfg['announcers'][who]['ref_text']


def tighten_pauses(wav, sr, max_pause_s=0.14, top_db=32):
    """Trim leading/trailing silence and cap internal pauses for brisk broadcast pacing."""
    wav = np.asarray(wav, dtype=np.float32)
    _, (a, b) = librosa.effects.trim(wav, top_db=35, frame_length=512, hop_length=128)
    wav = wav[a:b]
    intervals = librosa.effects.split(wav, top_db=top_db, frame_length=1024, hop_length=256)
    if len(intervals) == 0:
        return wav
    pieces = []
    max_gap = int(sr * max_pause_s)
    for idx, (start, end) in enumerate(intervals):
        if idx > 0:
            prev_end = intervals[idx - 1][1]
            gap = start - prev_end
            keep_gap = min(gap, max_gap)
            if keep_gap > 0:
                pieces.append(wav[prev_end:prev_end + keep_gap] * 0.25)
        pieces.append(wav[start:end])
    return np.concatenate(pieces)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('names', nargs='*', help='candidates to (re)design (default: those not designed yet)')
    ap.add_argument('--force', action='store_true', help='re-design every candidate')
    args = ap.parse_args()
    cfg = json.loads((HERE / 'voices.json').read_text())
    cands = {c: (who, d) for who, a in cfg['announcers'].items() for c, d in a['candidates'].items()}
    unknown = [n for n in args.names if n not in cands]
    if unknown:
        raise SystemExit(f'not in voices.json: {", ".join(unknown)}')
    out_dir = HERE / 'voices'
    out_dir.mkdir(exist_ok=True)
    names = args.names or [c for c in cands if args.force or not (out_dir / f'{c}.flac').exists()]
    # The designed ones first: a clone needs its source.
    names.sort(key=lambda n: 'clone' in cands[n][1])
    if not names:
        print('every candidate voice is designed already (--force to redo them)')
        return

    models = {}

    def model(name):
        if name not in models:
            models[name] = Qwen3TTSModel.from_pretrained(name, device_map='cuda:0', dtype=torch.bfloat16)
        return models[name]

    for name in names:
        who, d = cands[name]
        text = ref_text(cfg, who, name)
        t0 = time.time()
        with torch.inference_mode():
            if 'clone' in d:
                src = d['clone']
                if not (out_dir / f'{src}.flac').exists():
                    raise SystemExit(f'{name}: voices/{src}.flac is missing; design {src} first')
                prompt = model(CLONE_MODEL).create_voice_clone_prompt(
                    ref_audio=str(out_dir / f'{src}.flac'), ref_text=ref_text(cfg, cands[src][0], src))
                torch.manual_seed(d.get('seed', 7))
                wavs, sr = model(CLONE_MODEL).generate_voice_clone(
                    text=[text], language=['English'], voice_clone_prompt=prompt)
            else:
                torch.manual_seed(d.get('seed', 7))
                wavs, sr = model(DESIGN_MODEL).generate_voice_design(text=text, language='English', instruct=d['instruct'])
        wav = wavs[0]
        if 'max_pause_s' in d:
            wav = tighten_pauses(wav, sr, max_pause_s=float(d['max_pause_s']))
        out = out_dir / f'{name}.flac'
        sf.write(out, wav, sr)
        print(f'{out.relative_to(HERE.parent.parent)}: {len(wav) / sr:.1f} s of speech in {time.time() - t0:.1f} s', flush=True)


if __name__ == '__main__':
    main()
