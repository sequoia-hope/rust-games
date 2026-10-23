#!/usr/bin/env bash
# Builds the Rust simulation as the Python module `mw_sim_py` into this
# folder (train/mw_sim_py.so), for the Python that will import it:
# $PYTHON, else python3. Release build: training steps it millions of times.
set -euo pipefail
cd "$(dirname "$0")"
PYTHON="${PYTHON:-python3}"
PYO3_PYTHON="$(command -v "$PYTHON")" cargo build --release -p mw_sim_py --features extension-module
target="$(cargo metadata --format-version 1 --no-deps | "$PYTHON" -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
cp "$target/release/libmw_sim_py.so" mw_sim_py.so
echo "built $(pwd)/mw_sim_py.so"
