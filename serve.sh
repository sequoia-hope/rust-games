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
# and from then on the mirror tracks the local main branch. Each build
# records the commit it was built from in $SITE/.built-from; while the server
# runs, a watcher checks main every minute and, when it has moved, builds the
# new site beside the old ($SITE.next) and swaps it in, so the page is never
# half built. A build that fails leaves the old site up and is not retried
# until main moves again; one is skipped while the disk is nearly full.
#
#   REBUILD=1 ./serve.sh              rebuild, then serve
#   scripts/pages.py build dist/local refresh it by hand
#
# `pages.sh` reads the games' sources from `git archive HEAD` in places, so the
# mirror shows what is committed — like the deployed site, and unlike the
# games' own servers, which serve the working tree.
STAMP="$SITE/.built-from"
MIN_FREE_GB=15

main_commit() { git rev-parse --verify --quiet main || true; }

# Builds the site for main into $SITE.next and swaps it in.
build_swap() {
  local commit=$1
  local free
  free=$(df --output=avail -BG . | tail -1 | tr -dc 0-9)
  if [ "$free" -lt "$MIN_FREE_GB" ]; then
    echo "serve.sh: ${free} GB free, under ${MIN_FREE_GB}: not building ${commit:0:7}" >&2
    return 1
  fi
  echo "serve.sh: main is at ${commit:0:7}; building the site (minutes)" >&2
  rm -rf "$SITE.next"
  if python3 scripts/pages.py build "$SITE.next"; then
    echo "$commit" > "$SITE.next/.built-from"
    rm -rf "$SITE.old"
    if [ -d "$SITE" ]; then mv "$SITE" "$SITE.old"; fi
    mv "$SITE.next" "$SITE"
    rm -rf "$SITE.old"
    echo "serve.sh: now serving ${commit:0:7}" >&2
  else
    echo "serve.sh: the build of ${commit:0:7} failed; still serving the last one" >&2
    rm -rf "$SITE.next"
    return 1
  fi
}

if [ "${REBUILD:-}" = 1 ] || [ ! -f "$SITE/index.html" ]; then
  build_swap "$(main_commit)" || [ -f "$SITE/index.html" ]
else
  echo "serve.sh: serving the site already built in $SITE ($(cut -c1-7 "$STAMP" 2>/dev/null || echo 'commit unknown'))" >&2
fi

# The watcher: rebuild whenever main moves (also right away when the site on
# disk is older than main). It ends with the server. (http.server reads
# $SITE by name on every request, so the swap takes effect at once.)
(
  tried=""
  while sleep 60; do
    kill -0 $$ 2>/dev/null || exit 0
    want=$(main_commit)
    have=$(cat "$STAMP" 2>/dev/null || true)
    if [ -n "$want" ] && [ "$want" != "$have" ] && [ "$want" != "$tried" ]; then
      tried=$want
      build_swap "$want" || true
    fi
  done
) &

# exec: the server keeps this script's pid, which the watcher checks, and
# stopping the server stops the watcher a minute later at most.
exec python3 -m http.server "$PORT" --bind 0.0.0.0 --directory "$SITE"
