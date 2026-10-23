"""Policy steps per second of the vector env, for a few batch sizes."""
import time

import numpy as np

import mw_sim_py

for n in (64, 256, 1024):
    env = mw_sim_py.VecEnv(n, seed=0)
    env.reset()
    rng = np.random.default_rng(0)
    steps = 50
    t = time.perf_counter()
    for _ in range(steps):
        env.step(rng.uniform(-0.3, 0.3, (n, mw_sim_py.JOINTS)).astype(np.float32))
    dt = time.perf_counter() - t
    print(f"n={n:5d}: {n * steps / dt:9.0f} policy steps/s")
