#!/usr/bin/env bash
# serve.sh — build the Music Lab and serve it on its registered port.
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

./build.sh

# An https front on the tailnet, at https://<machine>.<tailnet>.ts.net/music-lab/
# (mind the trailing slash), as the games do: an AudioWorklet needs a secure
# page, and this server speaks plain http. Best-effort: without tailscale
# the plain address works on this machine (localhost is secure).
tailscale serve --bg --set-path /music-lab "http://127.0.0.1:$PORT" >/dev/null 2>&1 \
  || echo "serve.sh: no tailnet https front (tailscale serve failed); plain http only" >&2

exec python3 -m http.server "$PORT" --bind 0.0.0.0 --directory dist
