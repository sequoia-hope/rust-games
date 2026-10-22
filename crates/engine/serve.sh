#!/usr/bin/env bash
# serve.sh — serve the kit's documentation on its registered port.
#
# The kit is a library: it has no web build of its own the way a game does,
# so what it serves is its docs — `tools/docs.py` renders every tracked .md
# under this folder into dist/docs/ (README, CHANGELOG, docs/PLAN.md,
# docs/DECISIONS.md, each crate's and patch's notes, the skills, the
# template) with the links between them intact.
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

# Cheap (a few dozen markdown files), so it runs on every start and the page
# is never behind the docs.
python3 tools/docs.py dist/docs

exec python3 -m http.server "$PORT" --bind 0.0.0.0 --directory dist/docs
