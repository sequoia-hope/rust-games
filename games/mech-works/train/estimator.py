"""The estimator: the second half of Rapid Motor Adaptation. It learns to
infer the teacher's body latent z from the last second of proprioception
alone, so the policy walks without being told its body.

    python estimator.py --teacher teacher-v1 --run estimator-v1 --steps 40e6

The teacher (checkpoints/<teacher>/model.pt) is frozen. Each policy step the
estimator reads every env's history window and regresses the teacher's z
(and, as an auxiliary head for the demo's bars, the true body parameters).
This is DAgger: the estimator is trained online, and after a warm-up the
envs are driven by the actor on the *estimated* z, so the estimator learns
from the states its own mistakes lead to.

Writes, under ../checkpoints/<run>/:
    metrics.jsonl        one line per log interval: latent and parameter
                         errors, falls and tracking with the student driving
    est_<steps>.bin      the estimator (+ .json with test vectors)
    index.json           the exports
    model.pt             the estimator's state, to resume from
"""
import argparse
import json
import struct
import time
from pathlib import Path

import numpy as np
import torch
from torch import nn

import mw_sim_py
from policy import BLOB_VERSION, OBS_CLIP, Teacher, _act_name, _linears, mlp

HERE = Path(__file__).resolve().parent
ARCH = "estimator_v1"
WINDOW = 50  # policy steps: 1 s at 50 Hz
EMBED = 32
# (out channels, kernel, stride): 50 → 11 → 7 → 3 steps.
CONVS = ((32, 8, 4), (32, 5, 1), (32, 5, 1))

# The body parameter groups, for the logs (BodyParams::to_vec order).
GROUPS = {"torso_mass": (0, 1), "com": (1, 4), "leg_scale": (4, 6), "health": (6, 18), "driver": (18, 30), "friction": (30, 31), "latency": (31, 32)}


class Estimator(nn.Module):
    """Per-step embedding, a 1D conv over time, then two heads: the latent
    (tanh, as the teacher's encoder ends) and the body parameters."""

    def __init__(self, proprio, latent, params):
        super().__init__()
        self.proprio, self.latent_dim, self.params = proprio, latent, params
        self.embed = mlp((proprio, EMBED), out_act=nn.ELU())
        convs, ch, t = [], EMBED, WINDOW
        for out, k, s in CONVS:
            convs.append(nn.Conv1d(ch, out, k, s))
            ch, t = out, (t - k) // s + 1
        self.convs = nn.ModuleList(convs)
        self.flat = ch * t
        self.head_z = mlp((self.flat, latent), out_act=nn.Tanh())
        self.head_params = mlp((self.flat, params))

    def features(self, hist):
        """hist: (n, WINDOW, proprio), normalized, oldest first."""
        x = self.embed(hist).transpose(1, 2)  # (n, EMBED, WINDOW)
        for c in self.convs:
            x = nn.functional.elu(c(x))
        # Flattened time-major (step, channel), as the Rust side reads it.
        return x.transpose(1, 2).reshape(x.shape[0], -1)

    def forward(self, hist):
        f = self.features(hist)
        return self.head_z(f), self.head_params(f)


def parse():
    p = argparse.ArgumentParser()
    p.add_argument("--teacher", required=True, help="a run under checkpoints/")
    p.add_argument("--run", required=True)
    p.add_argument("--steps", type=float, default=40e6)
    p.add_argument("--envs", type=int, default=1024)
    p.add_argument("--lr", type=float, default=5e-4)
    p.add_argument("--aux", type=float, default=0.5, help="weight of the body-parameter head")
    p.add_argument("--warmup", type=float, default=2e6, help="steps with the teacher driving")
    p.add_argument("--handover", type=float, default=8e6, help="steps over which the student takes over")
    p.add_argument("--level", type=float, default=None, help="curriculum level (default: the teacher's last)")
    p.add_argument("--log-every", type=int, default=50, help="policy steps")
    p.add_argument("--export-every", type=int, default=1000, help="policy steps")
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    return p.parse_args()


@torch.no_grad()
def export(est: Estimator, norm, path, meta=None, test_hist=None):
    """Writes the estimator blob (the layout of policy.py's, arch
    "estimator_v1") and `path` + ".json" with the header and test vectors.
    The blob carries the teacher's proprioception normalization, so it
    reads raw observations as the policy does."""
    est = est.cpu().eval()
    tensors, data, offset = [], [], 0

    def add(name, t):
        nonlocal offset
        a = t.detach().cpu().numpy().astype("<f4").ravel()
        tensors.append({"name": name, "shape": list(t.shape), "offset": offset})
        data.append(a)
        offset += a.size

    p = est.proprio
    add("norm.mean", norm.mean[:p])
    add("norm.std", norm.std()[:p])
    layers = {}
    for name in ("embed", "head_z", "head_params"):
        seq = getattr(est, name)
        lins = _linears(seq)
        layers[name] = []
        for i, lin in enumerate(lins):
            add(f"{name}.{i}.weight", lin.weight)
            add(f"{name}.{i}.bias", lin.bias)
            layers[name].append({"in": lin.in_features, "out": lin.out_features, "activation": "elu" if name == "embed" else _act_name(seq, i, len(lins))})
    convs = []
    for i, (c, (out, k, s)) in enumerate(zip(est.convs, CONVS)):
        add(f"conv.{i}.weight", c.weight)  # (out, in, kernel)
        add(f"conv.{i}.bias", c.bias)
        convs.append({"in": c.in_channels, "out": out, "kernel": k, "stride": s, "activation": "elu"})
    layers["conv"] = convs
    header = {
        "arch": ARCH,
        "proprio": p,
        "window": WINDOW,
        "latent": est.latent_dim,
        "params": est.params,
        "obs_clip": OBS_CLIP,
        "layers": layers,
        "tensors": tensors,
    }
    hj = json.dumps(header).encode()
    with open(path, "wb") as f:
        f.write(b"MWPOLICY")
        f.write(struct.pack("<II", BLOB_VERSION, len(hj)))
        f.write(hj)
        f.write(b"\0" * ((-(16 + len(hj))) % 4))
        for a in data:
            f.write(a.tobytes())
    side = {"header": header, "meta": meta or {}}
    if test_hist is not None:
        raw = torch.as_tensor(np.asarray(test_hist, dtype=np.float32))
        z, prm = est(normalize(norm, raw, p))
        side["test"] = {"history": raw.tolist(), "latent": z.tolist(), "params": prm.tolist()}
    with open(str(path) + ".json", "w") as f:
        json.dump(side, f)


def normalize(norm, raw, p):
    """The teacher's normalization of the proprioception part."""
    return torch.clamp((raw - norm.mean[:p]) / norm.std()[:p], -OBS_CLIP, OBS_CLIP)


def main():
    a = parse()
    torch.manual_seed(a.seed)
    rng = np.random.default_rng(a.seed)
    dev = torch.device(a.device)
    root = HERE.parent / "checkpoints"
    out = root / a.run
    out.mkdir(parents=True, exist_ok=True)

    P, B, J = mw_sim_py.PROPRIO, mw_sim_py.BODY_PARAMS, mw_sim_py.JOINTS
    teacher = Teacher(P, B, J).to(dev)
    st = torch.load(root / a.teacher / "model.pt", map_location=dev)
    teacher.load_state_dict(st["model"])
    teacher.eval()
    for q in teacher.parameters():
        q.requires_grad_(False)
    level = st["level"] if a.level is None else a.level
    from train import curriculum

    est = Estimator(P, teacher.encoder[-2].out_features, B).to(dev)
    opt = torch.optim.Adam(est.parameters(), lr=a.lr)
    cfg = dict(vars(a), level=level, window=WINDOW, convs=CONVS, embed=EMBED)
    (out / "config.json").write_text(json.dumps(cfg, indent=1))

    env = mw_sim_py.VecEnv(a.envs, seed=a.seed + 100)
    env.set_curriculum(**curriculum(level))
    n = a.envs
    obs = torch.as_tensor(env.reset(), device=dev)
    hist = normalize(teacher.norm, obs[:, :P], P).unsqueeze(1).repeat(1, WINDOW, 1)
    raw_hist = obs[:, :P].unsqueeze(1).repeat(1, WINDOW, 1)
    student = torch.zeros(n, dtype=torch.bool, device=dev)

    metrics_f = open(out / "metrics.jsonl", "a")
    index = []
    steps, t_start = 0, time.time()
    acc = {"z": 0.0, "zvar": 0.0, "p": 0.0, "groups": np.zeros(len(GROUPS)), "n": 0, "falls": 0, "ends": 0, "s_falls": 0, "s_ends": 0, "track": 0.0}
    test_hist = None
    it = 0
    while steps < a.steps:
        with torch.no_grad():
            nobs = teacher.norm(obs)
            z_true = teacher.latent(nobs)
        params = torch.as_tensor(env.body_params(), device=dev)
        z_hat, p_hat = est(hist)
        loss_z = ((z_hat - z_true) ** 2).mean()
        loss_p = ((p_hat - params) ** 2).mean()
        loss = loss_z + a.aux * loss_p
        opt.zero_grad()
        loss.backward()
        nn.utils.clip_grad_norm_(est.parameters(), 1.0)
        opt.step()

        with torch.no_grad():
            z_drive = torch.where(student.unsqueeze(1), z_hat.detach(), z_true)
            act = teacher.actor(torch.cat([nobs[:, :P], z_drive], dim=1)).clamp(-1, 1)
            o, r, term, trunc, terms = env.step(act.cpu().numpy().astype(np.float32))
            done = term | trunc
            obs = torch.as_tensor(o, device=dev)
            nprop = normalize(teacher.norm, obs[:, :P], P)
            hist = torch.cat([hist[:, 1:], nprop.unsqueeze(1)], dim=1)
            raw_hist = torch.cat([raw_hist[:, 1:], obs[:, :P].unsqueeze(1)], dim=1)
            d = torch.as_tensor(done, device=dev)
            if d.any():
                # A new episode's history is its first frame, repeated (the
                # Rust estimator's reset does the same).
                hist[d] = nprop[d].unsqueeze(1).repeat(1, WINDOW, 1)
                raw_hist[d] = obs[d, :P].unsqueeze(1).repeat(1, WINDOW, 1)
                p_student = float(np.clip((steps - a.warmup) / a.handover, 0.0, 1.0))
                student[d] = torch.as_tensor(rng.random(int(d.sum())) < p_student, device=dev)

            acc["z"] += loss_z.item()
            acc["zvar"] += z_true.var(0).mean().item()
            acc["p"] += loss_p.item()
            err = ((p_hat - params) ** 2).mean(0).cpu().numpy()
            acc["groups"] += [err[s:e].mean() for s, e in GROUPS.values()]
            acc["n"] += 1
            stu = student.cpu().numpy()
            acc["ends"] += int(done.sum())
            acc["falls"] += int(term.sum())
            acc["s_ends"] += int((done & stu).sum())
            acc["s_falls"] += int((term & stu).sum())
            acc["track"] += float(terms[:, 0].mean()) / (1.5 / mw_sim_py.POLICY_HZ)
        steps += n
        it += 1
        if it == 200:
            test_hist = raw_hist[:4].cpu().numpy().copy()

        if it % a.log_every == 0:
            k = acc["n"]
            m = {
                "steps": steps,
                "time": time.time() - t_start,
                "latent_mse": acc["z"] / k,
                # What guessing the mean z would score: the bar to beat.
                "latent_var": acc["zvar"] / k,
                "params_mse": acc["p"] / k,
                "groups": {g: float(v) / k for g, v in zip(GROUPS, acc["groups"])},
                "student_share": float(student.float().mean().item()),
                "fall_rate": acc["falls"] / max(acc["ends"], 1),
                "student_fall_rate": acc["s_falls"] / acc["s_ends"] if acc["s_ends"] else None,
                "track": acc["track"] / k,
                "fps": a.log_every * n / (time.time() - t_log) if it > a.log_every else None,
            }
            t_log = time.time()
            metrics_f.write(json.dumps(m) + "\n")
            metrics_f.flush()
            sfr = m["student_fall_rate"]
            print(
                f"{steps/1e6:7.2f}M  z {m['latent_mse']:.4f}/{m['latent_var']:.4f}  p {m['params_mse']:.4f}  student {m['student_share']:.2f}  "
                f"falls {m['fall_rate']:.2f} (student {sfr if sfr is None else round(sfr, 2)})  track {m['track']:.2f}",
                flush=True,
            )
            acc = {"z": 0.0, "zvar": 0.0, "p": 0.0, "groups": np.zeros(len(GROUPS)), "n": 0, "falls": 0, "ends": 0, "s_falls": 0, "s_ends": 0, "track": 0.0}
        elif it == 1:
            t_log = time.time()

        if (it % a.export_every == 0 or steps >= a.steps) and test_hist is not None:
            name = f"est_{steps:010d}.bin"
            export(est, teacher.norm.cpu(), out / name, meta={"teacher": a.teacher, "steps": steps}, test_hist=test_hist)
            teacher.norm.to(dev)
            est.to(dev).train()
            index.append({"file": name, "steps": steps})
            (out / "index.json").write_text(json.dumps({"run": a.run, "teacher": a.teacher, "estimators": index}, indent=1))
            torch.save({"model": est.state_dict(), "opt": opt.state_dict(), "steps": steps}, out / "model.pt")


if __name__ == "__main__":
    main()
