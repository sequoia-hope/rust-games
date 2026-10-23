"""An estimator run's curves, for the dev log: python devlog/plot_estimator.py <run> <out.png>"""
import json
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

run, out = sys.argv[1], sys.argv[2]
rows = [json.loads(l) for l in open(f"checkpoints/{run}/metrics.jsonl") if l.strip()]
steps = [r["steps"] / 1e6 for r in rows]
fig, ax = plt.subplots(1, 3, figsize=(13, 2.9), facecolor="#161617")


def style(a, title):
    a.set_facecolor("#1d1d1e")
    a.set_title(title, color="#ece9e4", fontsize=10)
    a.set_xlabel("million steps", color="#a39d94", fontsize=8)
    a.tick_params(colors="#a39d94", labelsize=8)
    for s in a.spines.values():
        s.set_color("#444")


a = ax[0]
a.plot(steps, [r["latent_var"] for r in rows], color="#777", lw=1, label="guessing the mean")
a.plot(steps, [r["latent_mse"] for r in rows], color="#5fb3e8", lw=1.2, label="estimator")
a.set_ylim(0, None)
a.legend(fontsize=7, facecolor="#1d1d1e", labelcolor="#ece9e4", edgecolor="#444")
style(a, "latent z error (MSE)")
a = ax[1]
colors = ["#f6c27a", "#ea851f", "#9ab8e0", "#e5603b", "#b48ead", "#7fc97f", "#ece9e4"]
for (g, c) in zip(rows[0]["groups"], colors):
    a.plot(steps, [r["groups"][g] for r in rows], color=c, lw=1, label=g)
a.set_ylim(0, 0.4)
a.legend(fontsize=7, ncol=2, facecolor="#1d1d1e", labelcolor="#ece9e4", edgecolor="#444")
style(a, "body parameter error (MSE; 0.33 is a blind guess)")
a = ax[2]
a.plot(steps, [r["student_share"] for r in rows], color="#5fb3e8", lw=1.2, label="envs the estimator drives")
a.set_ylim(0, 1.05)
a.legend(fontsize=7, facecolor="#1d1d1e", labelcolor="#ece9e4", edgecolor="#444")
style(a, "DAgger hand-over")
fig.tight_layout()
fig.savefig(out, dpi=110, facecolor=fig.get_facecolor())
