#!/usr/bin/env bash
# pages.sh <out>: the Music Lab's part of the public site
# (https://sequoia-hope.github.io/rust-games/music-lab/), for
# .github/workflows/pages.yml (site/games.toml lists it).
set -euo pipefail
out=${1:?usage: pages.sh <output folder>}
"$(dirname "$0")/build.sh" --release "$out"
