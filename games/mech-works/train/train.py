"""PPO for the Mech Works teacher policy.

    ./build.sh                      # the Rust sim as the mw_sim_py module
    python train.py --run first-walk --steps 100e6

Writes, under ../checkpoints/<run>/:
    metrics.jsonl        one line per update: rewards, episode length, falls,
                         reward terms, curriculum, speed (the viewer charts it)
    ckpt_<steps>.bin     the policy every --export-every updates (+ .json)
    coverage.jsonl       per ended episode: body parameters and whether it fell
    index.json           the checkpoints, for the viewer
    model.pt             the full training state, to resume from
"""
import argparse
import json
import os
import time
from pathlib import Path

import numpy as np
import torch

import mw_sim_py
from policy import Teacher, export

HERE = Path(__file__).resolve().parent


def parse():
    p = argparse.ArgumentParser()
    p.add_argument("--run", required=True)
    p.add_argument("--steps", type=float, default=100e6)
    p.add_argument("--envs", type=int, default=1024)
    p.add_argument("--horizon", type=int, default=24)
    p.add_argument("--epochs", type=int, default=5)
    p.add_argument("--minibatches", type=int, default=4)
    p.add_argument("--lr", type=float, default=3e-4)
    p.add_argument("--gamma", type=float, default=0.99)
    p.add_argument("--lam", type=float, default=0.95)
    p.add_argument("--clip", type=float, default=0.2)
    p.add_argument("--entropy", type=float, default=0.003)
    p.add_argument("--value-coef", type=float, default=1.0)
    p.add_argument("--kl-target", type=float, default=0.01)
    p.add_argument("--export-every", type=int, default=25)
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--curriculum", action="store_true", help="widen randomization as the walk improves")
    p.add_argument("--max-level", type=float, default=1.0)
    p.add_argument("--level", type=float, default=0.0, help="starting curriculum level")
    p.add_argument("--resume", action="store_true")
    p.add_argument("--init", help="start from this run's weights and curriculum level (a fine-tune)")
    p.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    return p.parse_args()


def curriculum(level):
    """The randomization at a curriculum level in [0, 1]: speed first, then
    the body, then pushes, then damage."""
    ramp = lambda a, b: float(np.clip((level - a) / (b - a), 0.0, 1.0))
    return dict(speed=0.3 + 0.7 * ramp(0.0, 0.3), body=ramp(0.1, 0.6), pushes=ramp(0.3, 0.8), damage=ramp(0.5, 1.0))


def register_run(root, run):
    """Lists the run in checkpoints/runs.json, which the viewer reads."""
    path = root / "runs.json"
    runs = json.loads(path.read_text())["runs"] if path.exists() else []
    if run not in runs:
        runs.append(run)
    path.write_text(json.dumps({"runs": runs}, indent=1))


def main():
    a = parse()
    torch.manual_seed(a.seed)
    out = HERE.parent / "checkpoints" / a.run
    out.mkdir(parents=True, exist_ok=True)
    dev = torch.device(a.device)

    env = mw_sim_py.VecEnv(a.envs, seed=a.seed)
    terms_names = list(mw_sim_py.VecEnv.reward_terms)
    level = a.level
    env.set_curriculum(**curriculum(level))
    model = Teacher(mw_sim_py.PROPRIO, mw_sim_py.BODY_PARAMS, mw_sim_py.JOINTS).to(dev)
    opt = torch.optim.Adam(model.parameters(), lr=a.lr)
    lr = a.lr
    update, total_steps = 0, 0
    index = []
    if a.resume and (out / "model.pt").exists():
        st = torch.load(out / "model.pt", map_location=dev)
        model.load_state_dict(st["model"])
        opt.load_state_dict(st["opt"])
        update, total_steps, level, lr = st["update"], st["steps"], st["level"], st["lr"]
        index = json.loads((out / "index.json").read_text())["checkpoints"]
        env.set_curriculum(**curriculum(level))
    elif a.init:
        st = torch.load(HERE.parent / "checkpoints" / a.init / "model.pt", map_location=dev)
        model.load_state_dict(st["model"])
        level = max(level, st["level"])
        env.set_curriculum(**curriculum(level))
    (out / "config.json").write_text(json.dumps(vars(a), indent=1))
    register_run(out.parent, a.run)

    obs = torch.as_tensor(env.reset(), device=dev)
    n, T = a.envs, a.horizon
    od, ad = mw_sim_py.OBS, mw_sim_py.JOINTS
    buf_obs = torch.zeros(T, n, od, device=dev)
    buf_act = torch.zeros(T, n, ad, device=dev)
    buf_logp = torch.zeros(T, n, device=dev)
    buf_rew = torch.zeros(T, n, device=dev)
    buf_done = torch.zeros(T, n, device=dev)
    buf_val = torch.zeros(T, n, device=dev)

    ep_ret = np.zeros(n)
    ep_len = np.zeros(n)
    metrics_f = open(out / "metrics.jsonl", "a")
    coverage_f = open(out / "coverage.jsonl", "a")
    t_start = time.time()
    recent = []  # the last episodes' bodies and outcomes, for the coverage map
    # The curriculum steps on the fall rate over the last 10 updates' episodes,
    # not on one update's handful (teacher-v1 climbed on lucky samples).
    window_falls, window_ends = [], []
    test_obs = env.reset()[:4].copy()
    obs = torch.as_tensor(env.reset(), device=dev)

    while total_steps < a.steps:
        t0 = time.time()
        finished_ret, finished_len, falls, ends = [], [], 0, 0
        term_sums = np.zeros(len(terms_names))
        params_before = None
        with torch.no_grad():
            for t in range(T):
                model.norm.update(obs)
                d = model.dist(obs)
                act = d.sample()
                buf_obs[t], buf_act[t] = obs, act
                buf_logp[t] = d.log_prob(act).sum(-1)
                buf_val[t] = model.value(obs)
                params_before = env.body_params()
                o, r, term, trunc, terms = env.step(act.clamp(-1, 1).cpu().numpy().astype(np.float32))
                term_sums += terms.sum(0)
                rew = torch.as_tensor(r, device=dev)
                # Bootstrap through a time limit: add the value of the last state.
                if trunc.any():
                    fin = torch.as_tensor(env.final_obs()[trunc], device=dev)
                    rew[torch.as_tensor(trunc, device=dev)] += a.gamma * model.value(fin)
                buf_rew[t] = rew
                done = term | trunc
                buf_done[t] = torch.as_tensor(done, device=dev, dtype=torch.float32)
                ep_ret += r
                ep_len += 1
                for i in np.nonzero(done)[0]:
                    finished_ret.append(ep_ret[i])
                    finished_len.append(ep_len[i])
                    ends += 1
                    falls += int(term[i])
                    p = params_before[i]
                    row = {"u": update, "p": [round(float(x), 3) for x in p], "fell": bool(term[i]), "len": int(ep_len[i])}
                    coverage_f.write(json.dumps(row) + "\n")
                    recent.append(row)
                    ep_ret[i] = ep_len[i] = 0
                obs = torch.as_tensor(o, device=dev)
            next_val = model.value(obs)

            # GAE.
            adv = torch.zeros_like(buf_rew)
            last = torch.zeros(n, device=dev)
            for t in reversed(range(T)):
                nv = next_val if t == T - 1 else buf_val[t + 1]
                nonterm = 1.0 - buf_done[t]
                delta = buf_rew[t] + a.gamma * nv * nonterm - buf_val[t]
                last = delta + a.gamma * a.lam * nonterm * last
                adv[t] = last
            ret = adv + buf_val
        t_collect = time.time() - t0

        b_obs, b_act = buf_obs.reshape(-1, od), buf_act.reshape(-1, ad)
        b_logp, b_adv, b_ret, b_val = buf_logp.reshape(-1), adv.reshape(-1), ret.reshape(-1), buf_val.reshape(-1)
        b_adv = (b_adv - b_adv.mean()) / (b_adv.std() + 1e-8)
        bs = b_obs.shape[0]
        mb = bs // a.minibatches
        kls, pls, vls, ents = [], [], [], []
        for _ in range(a.epochs):
            perm = torch.randperm(bs, device=dev)
            for k in range(a.minibatches):
                idx = perm[k * mb : (k + 1) * mb]
                d = model.dist(b_obs[idx])
                logp = d.log_prob(b_act[idx]).sum(-1)
                ratio = (logp - b_logp[idx]).exp()
                pg = -torch.min(ratio * b_adv[idx], ratio.clamp(1 - a.clip, 1 + a.clip) * b_adv[idx]).mean()
                v = model.value(b_obs[idx])
                v_clip = b_val[idx] + (v - b_val[idx]).clamp(-a.clip, a.clip)
                vl = torch.max((v - b_ret[idx]) ** 2, (v_clip - b_ret[idx]) ** 2).mean()
                ent = d.entropy().sum(-1).mean()
                loss = pg + a.value_coef * vl - a.entropy * ent
                opt.zero_grad()
                loss.backward()
                torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
                opt.step()
                with torch.no_grad():
                    kl = ((ratio - 1) - (logp - b_logp[idx])).mean().item()
                kls.append(kl)
                pls.append(pg.item())
                vls.append(vl.item())
                ents.append(ent.item())
                # Adaptive learning rate on the KL.
                if kl > 2 * a.kl_target:
                    lr = max(1e-5, lr / 1.5)
                elif kl < a.kl_target / 2:
                    lr = min(1e-3, lr * 1.5)
                for g in opt.param_groups:
                    g["lr"] = lr
        with torch.no_grad():
            model.log_std.clamp_(-2.5, 0.5)

        update += 1
        total_steps += n * T
        mean_len = float(np.mean(finished_len)) if finished_len else float("nan")
        fall_rate = falls / max(ends, 1)
        terms_mean = {k: float(v) / (n * T) for k, v in zip(terms_names, term_sums)}
        # The lin tracking term's best is 2.0 * POLICY_DT per step.
        track = terms_mean["track_lin"] / (2.0 / mw_sim_py.POLICY_HZ)
        window_falls, window_ends = (window_falls + [falls])[-10:], (window_ends + [ends])[-10:]
        window_rate = sum(window_falls) / max(sum(window_ends), 1)
        if a.curriculum and update % 10 == 0 and sum(window_ends) >= 100 and window_rate < 0.15 and track > 0.7 and level < a.max_level:
            level = min(a.max_level, level + 0.05)
            env.set_curriculum(**curriculum(level))
        m = {
            "update": update,
            "steps": total_steps,
            "time": time.time() - t_start,
            "reward": float(np.mean(finished_ret)) if finished_ret else None,
            "step_reward": float(buf_rew.mean().item()),
            "ep_len": mean_len if finished_len else None,
            "episodes": ends,
            "fall_rate": fall_rate,
            "track": track,
            "terms": terms_mean,
            "level": level,
            "curriculum": curriculum(level),
            "lr": lr,
            "kl": float(np.mean(kls)),
            "policy_loss": float(np.mean(pls)),
            "value_loss": float(np.mean(vls)),
            "entropy": float(np.mean(ents)),
            "std": float(model.log_std.exp().mean().item()),
            "fps": n * T / (time.time() - t0),
            "collect_s": t_collect,
        }
        metrics_f.write(json.dumps(m) + "\n")
        metrics_f.flush()
        coverage_f.flush()
        if update % 5 == 0:
            print(
                f"u {update:5d} steps {total_steps/1e6:7.2f}M rew {m['reward'] or 0:8.2f} len {mean_len:6.1f} "
                f"falls {fall_rate:.2f} track {track:.2f} lvl {level:.2f} std {m['std']:.2f} fps {m['fps']:.0f}",
                flush=True,
            )
        if update % a.export_every == 0 or total_steps >= a.steps or update == 1:
            name = f"ckpt_{total_steps:010d}.bin"
            export(model, out / name, meta={k: m[k] for k in ("update", "steps", "reward", "ep_len", "fall_rate", "track", "level")}, test_obs=test_obs)
            model.to(dev).train()
            index.append({"file": name, "update": update, "steps": total_steps, "reward": m["reward"], "fall_rate": fall_rate, "track": track, "level": level})
            (out / "index.json").write_text(json.dumps({"run": a.run, "checkpoints": index}, indent=1))
            del recent[:-4000]
            (out / "coverage_recent.json").write_text(json.dumps({"update": update, "episodes": recent}))
            torch.save({"model": model.state_dict(), "opt": opt.state_dict(), "update": update, "steps": total_steps, "level": level, "lr": lr}, out / "model.pt")


if __name__ == "__main__":
    main()
