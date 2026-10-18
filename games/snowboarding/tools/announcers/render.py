"""Record the announcers' lines in their chosen voices (Qwen3-TTS voice clone).

Reads every clip in lines.json (generated from crates/sb_radio/src/
script.rs by `cargo run -p sb_radio --example script`), clones each
announcer's chosen voice from voices.json (voices/<candidate>.flac, made by
design.py) with the Qwen3-TTS Base model, and writes
audio/announcers/<id>.mp3: trimmed, levelled, 24 kHz mono, 64 kbit/s.

audio/announcers/index.json records which voice and text each clip was
made from, so only what changed is recorded again: a new or edited line, or
an announcer whose chosen voice changed. Clips no longer in the script are
removed. The game reads the index: a clip whose text differs from the
script's is not played (the caption still shows).

    python tools/announcers/render.py               # record what's missing or changed
    python tools/announcers/render.py --only big-   # re-record ids containing this
    python tools/announcers/render.py --force       # re-record everything

Borrowed from midnight-playground's tools/dj-voice/render.py. Needs a
Python with qwen-tts, a CUDA GPU with about 5 GB free, and ffmpeg.
"""
import argparse
import json
import os
import subprocess
import time
from pathlib import Path

os.environ.setdefault('PYTORCH_ALLOC_CONF', 'expandable_segments:True')
import librosa
import numpy as np
import torch
from qwen_tts import Qwen3TTSModel

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
OUT = ROOT / 'audio' / 'announcers'
CLONE_MODEL = 'Qwen/Qwen3-TTS-12Hz-1.7B-Base'
BATCH = 4
RATE = 24000


def trim(wav):
    wav = np.asarray(wav, dtype=np.float32)
    _, (a, b) = librosa.effects.trim(wav, top_db=38, frame_length=512, hop_length=128)
    return wav[a:b]


def level(wav, sr):
    """Pad, set the speech to -16 dBFS RMS and cap the peaks at -1 dBFS."""
    wav = np.concatenate([np.zeros(int(sr * 0.02), np.float32), wav, np.zeros(int(sr * 0.06), np.float32)])
    frames = librosa.util.frame(np.pad(wav, (0, 512)), frame_length=512, hop_length=256)
    rms = np.sqrt((frames ** 2).mean(axis=0))
    voiced = rms[rms > rms.max() * 0.1]
    lvl = np.sqrt((voiced ** 2).mean()) if voiced.size else 1e-3
    wav = wav * (10 ** (-16 / 20) / lvl)
    peak = np.abs(wav).max()
    if peak > 0.89:
        wav *= 0.89 / peak
    return wav.astype(np.float32)


def encode(wav, sr, path):
    subprocess.run(
        ['ffmpeg', '-v', 'error', '-y', '-f', 'f32le', '-ar', str(sr), '-ac', '1', '-i', '-',
         '-ar', str(RATE), '-c:a', 'libmp3lame', '-b:a', '64k', '-map_metadata', '-1', str(path)],
        input=wav.tobytes(), check=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--only', action='append', default=[], help='re-record clips whose id contains this (repeatable)')
    ap.add_argument('--force', action='store_true', help='re-record every clip')
    ap.add_argument('--seed', type=int, default=11, help='change it to get different takes')
    args = ap.parse_args()

    cfg = json.loads((HERE / 'voices.json').read_text())
    chosen = {who: d['chosen'] for who, d in cfg['announcers'].items()}
    for who, cand in chosen.items():
        if not (HERE / 'voices' / f'{cand}.flac').exists():
            raise SystemExit(f'{who}: voices/{cand}.flac is missing; run design.py first')
    clips = json.loads((HERE / 'lines.json').read_text())['clips']
    OUT.mkdir(parents=True, exist_ok=True)
    index_path = OUT / 'index.json'
    old = json.loads(index_path.read_text()) if index_path.exists() else {'clips': {}}

    def stale(c):
        o = old['clips'].get(c['id'])
        return (not o or o.get('text') != c['text'] or o.get('voice') != chosen[c['who']]
                or not (OUT / f"{c['id']}.mp3").exists())

    jobs = [c for c in clips if args.force or any(o in c['id'] for o in args.only) or stale(c)]
    print(f'{len(clips)} lines, {len(jobs)} to record', flush=True)
    made = {}
    if jobs:
        model = Qwen3TTSModel.from_pretrained(CLONE_MODEL, device_map='cuda:0', dtype=torch.bfloat16)
        t0 = time.time()
        done = 0
        for who in chosen:
            mine = [c for c in jobs if c['who'] == who]
            if not mine:
                continue
            prompt = model.create_voice_clone_prompt(
                ref_audio=str(HERE / 'voices' / f'{chosen[who]}.flac'),
                ref_text=cfg['announcers'][who]['candidates'][chosen[who]].get('ref_text') or cfg['announcers'][who]['ref_text'])
            print(f'{who}: {len(mine)} lines in the "{chosen[who]}" voice', flush=True)
            for i in range(0, len(mine), BATCH):
                batch = mine[i:i + BATCH]
                torch.manual_seed(args.seed + i)
                wavs, sr = model.generate_voice_clone(
                    text=[c['text'] for c in batch], language=['English'] * len(batch),
                    voice_clone_prompt=prompt * len(batch))
                for c, wav in zip(batch, wavs):
                    wav = level(trim(wav), sr)
                    encode(wav, sr, OUT / f"{c['id']}.mp3")
                    made[c['id']] = {'text': c['text'], 'voice': chosen[who], 'secs': round(len(wav) / sr, 2)}
                    done += 1
                    print(f"  {c['id']}.mp3: {len(wav) / sr:.1f} s  ({done}/{len(jobs)}, {time.time() - t0:.0f} s)", flush=True)

    ids = {c['id'] for c in clips}
    for f in OUT.glob('*.mp3'):
        if f.stem not in ids:
            f.unlink()
            print(f'  removed {f.name} (no longer in the script)')
    index = {'voices': chosen, 'clips': {}}
    for c in clips:
        if not (OUT / f"{c['id']}.mp3").exists():
            continue
        index['clips'][c['id']] = made.get(c['id']) or old['clips'].get(c['id'])
    index_path.write_text(json.dumps(index, indent=1, sort_keys=True) + '\n')
    print(f'{index_path.relative_to(ROOT)}: {len(index["clips"])} of {len(clips)} lines recorded')


if __name__ == '__main__':
    main()
