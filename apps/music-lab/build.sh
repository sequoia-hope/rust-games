#!/usr/bin/env bash
# build.sh [--release] [<out>]: the Music Lab's site in <out> (default
# dist/): the page, its scripts, and the music engine as music.wasm (the
# `music` crate's worklet build). Optimised even without --release (the
# `release` profile): the engine runs per sample. --release is the
# `web-release` profile and wasm-opt, as the games' web builds.
set -euo pipefail
cd "$(dirname "$0")"
profile=release
if [[ "${1:-}" == --release ]]; then profile=web-release; shift; fi
out=${1:-dist}

cargo build -p music --lib --features worklet --target wasm32-unknown-unknown --profile "$profile"
target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
mkdir -p "$out"
cp index.html main.js knobs.js measure.js lab-worklet.js "$out/"
cp "$target/wasm32-unknown-unknown/$profile/music.wasm" "$out/"
if [[ $profile == web-release ]]; then
  # The features the games' builds pass (crates/engine/pk_xtask).
  wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "$out/music.wasm" -o "$out/music.wasm"
fi
du -sh "$out/music.wasm"
