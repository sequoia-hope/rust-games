"""Design the announcers' voices with Qwen3-TTS.

Every announcer in voices.json has a few candidate voices. A candidate is
either a plain-English description (VoiceDesign) or another candidate's
voice cloned (the Base model); either way this speaks the candidate's
ref_text (the announcer's, unless it has its own) and saves
voices/<candidate>.flac: the reference clip render.py clones every line
from, so each announcer's whole script comes out in one consistent voice
and delivery. Set "chosen" in voices.json, then run render.py.

    python tools/announcers/design.py               # every candidate not designed yet
    python tools/announcers/design.py dani-booth    # just these (re-designs them)
    python tools/announcers/design.py --force       # every candidate again

Borrowed from midnight-playground's tools/dj-voice/. Needs a Python with
qwen-tts (https://github.com/QwenLM/Qwen3-TTS) and a CUDA GPU with about
5 GB free.
"""
import argparse
import json
import time
from pathlib import Path

import soundfile as sf
import torch
from qwen_tts import Qwen3TTSModel

HERE = Path(__file__).resolve().parent
DESIGN_MODEL = 'Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign'
CLONE_MODEL = 'Qwen/Qwen3-TTS-12Hz-1.7B-Base'


def ref_text(cfg, who, cand):
    """What a candidate's clip says."""
    return cfg['announcers'][who]['candidates'][cand].get('ref_text') or cfg['announcers'][who]['ref_text']


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
        out = out_dir / f'{name}.flac'
        sf.write(out, wavs[0], sr)
        print(f'{out.relative_to(HERE.parent.parent)}: {len(wavs[0]) / sr:.1f} s of speech in {time.time() - t0:.1f} s', flush=True)


if __name__ == '__main__':
    main()
