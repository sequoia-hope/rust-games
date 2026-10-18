"""How alike the announcers' voices are to each other and to other voices.

Embeds each voice with Qwen3-TTS's own speaker encoder (the embedding its
voice cloning works from) and prints the cosine similarity of every
candidate in voices/ against each other and against the reference voices
in any other directories given, with each voice's median pitch. Used to
keep Dani and Wren unlike midnight-playground's DJs:

    python tools/announcers/compare.py ../midnight-playground/tools/dj-voice/voices

Needs a Python with qwen-tts, a CUDA GPU and librosa.
"""
import argparse
from pathlib import Path

import librosa
import numpy as np
import torch
from qwen_tts import Qwen3TTSModel

HERE = Path(__file__).resolve().parent
MODEL = 'Qwen/Qwen3-TTS-12Hz-1.7B-Base'


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('others', nargs='*', help='directories of other voices (.flac) to compare against')
    args = ap.parse_args()
    tts = Qwen3TTSModel.from_pretrained(MODEL, device_map='cuda:0', dtype=torch.bfloat16)
    sr = tts.model.speaker_encoder_sample_rate

    def embed(path):
        wav, _ = librosa.load(path, sr=sr)
        e = tts.model.extract_speaker_embedding(audio=wav, sr=sr).float().flatten().cpu().numpy()
        f0, _, _ = librosa.pyin(librosa.resample(wav, orig_sr=sr, target_sr=16000), fmin=70, fmax=500, sr=16000)
        return e / np.linalg.norm(e), float(np.nanmedian(f0))

    mine = {p.stem: embed(p) for p in sorted((HERE / 'voices').glob('*.flac'))}
    others = {p.stem: embed(p) for d in args.others for p in sorted(Path(d).glob('*.flac'))}
    # Every embedding shares a large common part (any voice is 0.98 like
    # any other): measure from the pool's mean instead.
    mean = np.mean([e for e, _ in {**mine, **others}.values()], axis=0)

    def centred(e):
        c = e - mean
        return c / np.linalg.norm(c)

    mine = {k: (centred(e), f0) for k, (e, f0) in mine.items()}
    others = {k: (centred(e), f0) for k, (e, f0) in others.items()}
    for name, (e, f0) in mine.items():
        print(f'{name} (median pitch {f0:.0f} Hz)')
        rows = [(float(e @ o), k, of0) for k, (o, of0) in {**mine, **others}.items() if k != name]
        for sim, k, of0 in sorted(rows, reverse=True)[:4]:
            tag = 'ours' if k in mine else 'other'
            print(f'   {sim:5.2f}  {k:16s} ({tag}, {of0:.0f} Hz)')
    if others:
        print('the others:', ', '.join(f'{k} {f0:.0f} Hz' for k, (_, f0) in others.items()))


if __name__ == '__main__':
    main()
