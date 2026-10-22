#!/usr/bin/env bash
# Render a song from crates/music and extract an MP3 clip for listening via read_file.
# Usage: render-clip.sh <genre> <seed> [start_sec] [duration_sec] [out.mp3]
set -euo pipefail

GENRE="${1:?usage: render-clip.sh <genre> <seed> [start_sec] [duration_sec] [out.mp3]}"
SEED="${2:?usage: render-clip.sh <genre> <seed> [start_sec] [duration_sec] [out.mp3]}"
START="${3:-50}"
DUR="${4:-35}"
OUT="${5:-/tmp/${GENRE}-${SEED}.mp3}"
SECS=$(( START + DUR + 5 ))
WAV="${OUT%.mp3}.wav"

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

cargo run --release -p music --example render -- "$GENRE" "$SEED" "$WAV" --secs "$SECS"
ffmpeg -y -ss "$START" -t "$DUR" -i "$WAV" -b:a 128k "$OUT" 2>/dev/null
echo "Rendered clip: $OUT (${START}s..$((START + DUR))s)"
