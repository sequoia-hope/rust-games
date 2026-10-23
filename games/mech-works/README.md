# Mech Works

A mech game about staying: fighting for a place in a broken world, then
rebuilding it until people can live there again. The mech walks on a
learned, damage-tolerant, self-calibrating locomotion policy that runs live
in the browser. Nothing about its stumbling, limping or recovering is
animated.

The first thing to build is not the game. It is a set of tech demos for that
policy, with the training pipeline and a training visualizer built first.

**Status:** M0 and M1. The simulation, the trainer, the policy in Rust and
the browser client with the training viewer and the impact, damage and
calibration demos exist; a teacher policy is training. The spec is
[`docs/SPEC.md`](docs/SPEC.md), the audio requirements
[`docs/AUDIO.md`](docs/AUDIO.md), and the progress, with pictures, is the
dev log, [`devlog/index.html`](devlog/index.html) (on the dev server at
`/devlog/`). `CLAUDE.md` has the layout and the commands.
