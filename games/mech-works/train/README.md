# Training

PPO for the teacher policy, on the Rust simulation through the `mw_sim_py`
module (SPEC.md, "Training").

Needs a Python with PyTorch (CUDA for speed) and NumPy. Then:

```sh
PYTHON=python3 ./build.sh                 # the Rust sim as mw_sim_py.so, here
python bench.py                           # policy steps per second
python train.py --run teacher-v2 --steps 150e6 --curriculum
```

A run writes `../checkpoints/<run>/` (see `train.py`'s docstring) and lists
itself in `../checkpoints/runs.json`; the demo's training viewer reads both,
live, on the dev server. `--resume` continues a run from its `model.pt`.

To ship a policy with the demos, copy its `ckpt_*.bin` and `.bin.json` into
`../assets/policies/` (the parity test in `mw_policy` checks every one).
