"""Check every recorded clip against its line with Qwen3-ASR.

Transcribes audio/announcers/<id>.mp3 and scores how much of the line's
words came back (0..1). Prints the clips under --min, worst first, and
writes them to check.txt; re-record those with
`render.py --seed <n> --only <id> ...`.

    python tools/announcers/check.py [--min 0.8] [--only <id-part>]

Needs a Python with qwen-asr (https://github.com/QwenLM/Qwen3-ASR), a CUDA
GPU and librosa.
"""
import argparse
import difflib
import json
import re
from pathlib import Path

import librosa
import torch
from qwen_asr import Qwen3ASRModel

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
OUT = ROOT / 'audio' / 'announcers'
MODEL = 'Qwen/Qwen3-ASR-1.7B'

# How the recogniser writes what the script spells out.
SAME = {'1080': 'ten eighty', 'danny': 'dani', 'ren': 'wren', 'okay': 'okay', 'ok': 'okay'}


def words(s):
    s = s.lower().replace('-', ' ')
    out = []
    for w in re.findall(r"[a-z0-9']+", s):
        out += SAME.get(w, w).split()
    return out


def score(want, got):
    a, b = words(want), words(got)
    if not a:
        return 1.0
    m = difflib.SequenceMatcher(a=a, b=b)
    return sum(x.size for x in m.get_matching_blocks()) / len(a)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--min', type=float, default=0.8)
    ap.add_argument('--only', default='')
    ap.add_argument('--model', default=MODEL)
    args = ap.parse_args()
    clips = [c for c in json.loads((HERE / 'lines.json').read_text())['clips'] if args.only in c['id']]
    asr = Qwen3ASRModel.from_pretrained(args.model, dtype=torch.bfloat16, device_map='cuda:0', max_new_tokens=128)
    bad = []
    for i in range(0, len(clips), 8):
        batch = [c for c in clips[i:i + 8] if (OUT / f"{c['id']}.mp3").exists()]
        audio = [(librosa.load(OUT / f"{c['id']}.mp3", sr=16000)[0], 16000) for c in batch]
        for c, r in zip(batch, asr.transcribe(audio=audio, language=['English'] * len(batch))):
            got = getattr(r, 'text', str(r))
            s = score(c['text'], got)
            if s < args.min:
                bad.append((s, c['id'], c['text'], got))
    bad.sort()
    for s, i, want, got in bad:
        print(f'{s:.2f} {i}\n     want: {want}\n     got:  {got}')
    (HERE / 'check.txt').write_text(''.join(f'{i}\n' for _, i, _, _ in bad))
    print(f'{len(clips) - len(bad)} of {len(clips)} clips match their lines (>= {args.min})')


if __name__ == '__main__':
    main()
