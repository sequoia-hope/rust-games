#!/usr/bin/env bash
# Builds Midnight Playground for the public site and writes it into the
# folder given (.github/workflows/pages.yml; the site serves it at
# /rust-games/midnight-playground/, D1235). As on the old site (D1112): the
# Rust build at the folder's root; the tools beside it (the labs, benches and
# voice studios count as part of the game) with what their pages load; and
# the JS game, Midnight Racer, as its tracked files under legacy/. Pages
# compresses on the fly, so the .br and .gz copies stay out.
#
# MP_SIGNAL_URL, if set, is the signalling server built in (D1124).
set -euo pipefail
out=${1:?usage: pages.sh <output folder>}
mkdir -p "$out/legacy"
out=$(cd "$out" && pwd)
cd "$(dirname "$0")"
# This folder's tree in HEAD; git archive runs from the top, where its
# paths are read relative to that tree.
here=$(git rev-parse --show-prefix)
top=$(git rev-parse --show-toplevel)

cargo xtask web --release
git -C "$top" archive "HEAD:${here%/}" tools src vendor audio assets music.html crates/mp_audio/stations.json | tar -x -C "$out"
cp -r dist/next/. "$out/"
find "$out" \( -name '*.br' -o -name '*.gz' \) -delete
git -C "$top" archive "HEAD:${here%/}" | tar -x -C "$out/legacy"
rm -rf "$out/legacy/dist"
du -sh "$out" "$out/legacy"
