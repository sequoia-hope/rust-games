# Mech Works

A mech game about staying: fighting for a place in a broken world, then
rebuilding it until people can live there again. The mech walks on a
learned, damage-tolerant, self-calibrating locomotion policy that runs live
in the browser. Nothing about its stumbling, limping or recovering is
animated.

The first thing to build is not the game. It is a set of tech demos for that
policy, with the training pipeline and a training visualizer built first.

**Status:** spec only. No code yet. The spec is [`docs/SPEC.md`](docs/SPEC.md).
Its first milestone is M0 (sim parity: the mech in Rapier, a PyO3 env, and
matching native and WASM replays).
