"""Training curves of a run, for the dev log: python devlog/plot_training.py <run> <out.png>"""
import json
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

run, out = sys.argv[1], sys.argv[2]
rows = [json.loads(l) for l in open(f"checkpoints/{run}/metrics.jsonl") if l.strip()]
steps = [r["steps"] / 1e6 for r in rows]
fig, ax = plt.subplots(1, 4, figsize=(13, 2.8), facecolor="#161617")
series = [
    ("episode length (s)", [(r["ep_len"] or float("nan")) / 50 for r in rows], "#f6c27a", (0, 21)),
    ("fall rate", [r["fall_rate"] for r in rows], "#e5603b", (0, 1.02)),
    ("speed tracking", [r["track"] for r in rows], "#7fc97f", (0, 1.02)),
    ("curriculum level", [r["level"] for r in rows], "#9ab8e0", (0, 1.02)),
]
for a, (title, ys, c, lim) in zip(ax, series):
    a.set_facecolor("#1d1d1e")
    a.plot(steps, ys, color=c, lw=1.2)
    a.set_title(title, color="#ece9e4", fontsize=10)
    a.set_ylim(*lim)
    a.set_xlabel("million steps", color="#a39d94", fontsize=8)
    a.tick_params(colors="#a39d94", labelsize=8)
    for s in a.spines.values():
        s.set_color("#444")
fig.tight_layout()
fig.savefig(out, dpi=110, facecolor=fig.get_facecolor())
