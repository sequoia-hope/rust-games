#!/usr/bin/env bash
# serve.sh — serve the site this repo publishes, built locally.
#
# What this serves is a local mirror of https://sequoia-hope.github.io/rust-games/:
# `scripts/pages.py build` runs each entry's `pages.sh` (the same scripts
# .github/workflows/pages.yml runs) into dist/local/ and writes the front page
# over them, so the front page and every game underneath it are the real thing
# rather than a status card. The games also have registrations of their own
# (`proj up midnight-playground`, …) which serve from their own dist/ with
# their own cache rules and https front; this is the whole site in one place,
# as it goes out.
#
# The number lives in the registry (~/scripts/launcher.toml), never in this
# file: $PORT when the hub started us, otherwise `proj port`, which resolves
# this directory to the project registered for it. There is deliberately no
# fallback default — a project that cannot find its port should say so rather
# than quietly bind whatever looked free.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

PROJ="$(command -v proj || echo "$HOME/scripts/proj")"
PORT="${PORT:-$("$PROJ" port)}"

SITE=dist/local

# Building is two Bevy release wasm builds and takes minutes, far longer than
# the hub waits before it calls the port down. So the build is not on the
# start path once there is something to serve: the first start pays for it,
# later starts serve what is there, and a rebuild is asked for explicitly.
#
#   REBUILD=1 ./serve.sh              rebuild, then serve
#   scripts/pages.py build dist/local refresh it without touching the server
#
# `pages.sh` reads the games' sources from `git archive HEAD` in places, so the
# mirror shows what is committed — like the deployed site, and unlike the
# games' own servers, which serve the working tree.
if [ "${REBUILD:-}" = 1 ] || [ ! -f "$SITE/index.html" ]; then
  echo "serve.sh: building the site into $SITE (minutes: two release wasm builds)" >&2
  python3 scripts/pages.py build "$SITE"
else
  echo "serve.sh: serving the site already built in $SITE" >&2
  echo "serve.sh: REBUILD=1 ./serve.sh to rebuild it first" >&2
fi

exec python3 -m http.server "$PORT" --bind 0.0.0.0 --directory "$SITE"
