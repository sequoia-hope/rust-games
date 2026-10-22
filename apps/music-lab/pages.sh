#!/usr/bin/env bash
# pages.sh <out>: the Music Lab's part of the public site
# (https://sequoia-hope.github.io/rust-games/music-lab/), for
# .github/workflows/pages.yml (site/games.toml lists it).
set -euo pipefail
out=${1:?usage: pages.sh <output folder>}
# Absolute before handing it on, as the games' pages.sh do: build.sh changes
# directory to this folder, so a relative path would land under it instead.
mkdir -p "$out"
out=$(cd "$out" && pwd)
"$(dirname "$0")/build.sh" --release "$out"
