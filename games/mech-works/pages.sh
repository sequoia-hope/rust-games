#!/usr/bin/env bash
# Builds Mech Works for the public site and writes it into the folder given
# (.github/workflows/pages.yml; the site serves it at /rust-games/mech-works/):
# the web build, both backends, with the shipped policies in
# assets/policies/, as `cargo xtask web --release` lays it out in dist/web/.
# There is no checkpoints/ on the site, so the viewer shows the shipped
# policy. Pages compresses on the fly, so the .br and .gz copies stay out.
set -euo pipefail
out=${1:?usage: pages.sh <output folder>}
mkdir -p "$out"
out=$(cd "$out" && pwd)
cd "$(dirname "$0")"

cargo xtask web --release
cp -r dist/web/. "$out/"
find "$out" \( -name '*.br' -o -name '*.gz' \) -delete
du -sh "$out"
