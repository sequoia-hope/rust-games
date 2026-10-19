#!/usr/bin/env bash
# Builds Snowboarding for the public site and writes it into the folder
# given (.github/workflows/pages.yml; the site serves it at
# /rust-games/snowboarding/): the web build, both backends, as
# `cargo xtask web --release` lays it out in dist/web/. Pages compresses on
# the fly, so the .br and .gz copies stay out. (The page's crash reports go
# to `log`, which only the local server takes; on Pages they fail quietly.)
set -euo pipefail
out=${1:?usage: pages.sh <output folder>}
mkdir -p "$out"
out=$(cd "$out" && pwd)
cd "$(dirname "$0")"

cargo xtask web --release
cp -r dist/web/. "$out/"
find "$out" \( -name '*.br' -o -name '*.gz' \) -delete
du -sh "$out"
