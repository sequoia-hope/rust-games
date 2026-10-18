#!/usr/bin/env bash
# serve.sh — serve this project on its registered port.
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

# An https front on the tailnet, at https://<machine>.<tailnet>.ts.net/snowboarding/
# (mind the trailing slash), as midnight-playground does: phones only hand
# WebGPU to https pages, and this server speaks plain http. Pointed at
# $PORT on every start, so the proxy follows the registry. Best-effort:
# without tailscale the plain address still works.
tailscale serve --bg --set-path /snowboarding "http://127.0.0.1:$PORT" >/dev/null 2>&1 \
  || echo "serve.sh: no tailnet https front (tailscale serve failed); plain http only" >&2

# tools/serve.py: dist/web/ over the root, no-store caching, precompressed files.
exec python3 tools/serve.py --port "$PORT" --bind 0.0.0.0
