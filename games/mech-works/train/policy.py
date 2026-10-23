"""The teacher policy and its export to the weight blob that the Rust
policy (`mw_policy`) loads.

The teacher sees proprioception and the true body parameters; the
parameters go through a small encoder to a latent z (the estimator later
learns to infer z from motion alone). Observations are normalized with
running statistics that are frozen into the export.

Blob layout (little-endian):
    b"MWPOLICY"  u32 version  u32 header_len  header (JSON, utf-8)
    zero padding to a multiple of 4, then the f32 tensors the header lists
    (each row-major, at its "offset" in floats from the start of the data).
"""
import json
import struct

import numpy as np
import torch
from torch import nn

BLOB_VERSION = 1
ARCH = "teacher_v1"
ENCODER_HIDDEN = (64,)
LATENT = 16
ACTOR_HIDDEN = (256, 256, 128)
CRITIC_HIDDEN = (256, 256, 128)
OBS_CLIP = 5.0


def mlp(sizes, out_act=None):
    layers = []
    for i in range(len(sizes) - 1):
        layers.append(nn.Linear(sizes[i], sizes[i + 1]))
        if i < len(sizes) - 2:
            layers.append(nn.ELU())
        elif out_act is not None:
            layers.append(out_act)
    return nn.Sequential(*layers)


class RunningNorm(nn.Module):
    """Observation normalization from running mean and variance."""

    def __init__(self, dim):
        super().__init__()
        self.register_buffer("mean", torch.zeros(dim))
        self.register_buffer("var", torch.ones(dim))
        self.register_buffer("count", torch.tensor(1e-4))

    @torch.no_grad()
    def update(self, x):
        b_mean, b_var, b_count = x.mean(0), x.var(0, unbiased=False), x.shape[0]
        delta = b_mean - self.mean
        total = self.count + b_count
        self.mean += delta * b_count / total
        m2 = self.var * self.count + b_var * b_count + delta**2 * self.count * b_count / total
        self.var = m2 / total
        self.count = total

    def std(self):
        return torch.sqrt(self.var + 1e-8)

    def forward(self, x):
        return torch.clamp((x - self.mean) / self.std(), -OBS_CLIP, OBS_CLIP)


class Teacher(nn.Module):
    def __init__(self, proprio, params, act_dim, init_log_std=-0.7):
        super().__init__()
        self.proprio, self.params, self.act_dim = proprio, params, act_dim
        obs = proprio + params
        self.norm = RunningNorm(obs)
        self.encoder = mlp((params, *ENCODER_HIDDEN, LATENT), out_act=nn.Tanh())
        self.actor = mlp((proprio + LATENT, *ACTOR_HIDDEN, act_dim))
        self.critic = mlp((obs, *CRITIC_HIDDEN, 1))
        self.log_std = nn.Parameter(torch.full((act_dim,), init_log_std))
        # Small last layer: the first actions stay near the standing pose.
        with torch.no_grad():
            self.actor[-1].weight.mul_(0.01)
            self.actor[-1].bias.zero_()

    def latent(self, nobs):
        return self.encoder(nobs[:, self.proprio :])

    def action_mean(self, obs):
        n = self.norm(obs)
        return self.actor(torch.cat([n[:, : self.proprio], self.latent(n)], dim=1))

    def value(self, obs):
        return self.critic(self.norm(obs)).squeeze(-1)

    def dist(self, obs):
        mean = self.action_mean(obs)
        return torch.distributions.Normal(mean, self.log_std.exp().expand_as(mean))


def _linears(seq):
    return [m for m in seq if isinstance(m, nn.Linear)]


def _act_name(seq, i, n):
    """Activation after linear i of n in an mlp() Sequential."""
    if i < n - 1:
        return "elu"
    last = seq[-1]
    return "tanh" if isinstance(last, nn.Tanh) else "none"


@torch.no_grad()
def export(model: Teacher, path, meta=None, test_obs=None):
    """Writes the deployable part (normalization, encoder, actor) to `path`,
    and `path` + ".json" with the header, `meta`, and test vectors."""
    model = model.cpu().eval()
    tensors, data, offset = [], [], 0

    def add(name, t):
        nonlocal offset
        a = t.detach().cpu().numpy().astype("<f4").ravel()
        tensors.append({"name": name, "shape": list(t.shape), "offset": offset})
        data.append(a)
        offset += a.size

    add("norm.mean", model.norm.mean)
    add("norm.std", model.norm.std())
    layers = {}
    for net_name, seq in (("encoder", model.encoder), ("actor", model.actor)):
        lins = _linears(seq)
        layers[net_name] = []
        for i, lin in enumerate(lins):
            add(f"{net_name}.{i}.weight", lin.weight)
            add(f"{net_name}.{i}.bias", lin.bias)
            layers[net_name].append(
                {"in": lin.in_features, "out": lin.out_features, "activation": _act_name(seq, i, len(lins))}
            )
    header = {
        "arch": ARCH,
        "obs_dim": model.proprio + model.params,
        "proprio": model.proprio,
        "params": model.params,
        "latent": LATENT,
        "act_dim": model.act_dim,
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
    if test_obs is not None:
        obs = torch.as_tensor(np.asarray(test_obs, dtype=np.float32))
        side["test"] = {
            "obs": obs.tolist(),
            "action": model.action_mean(obs).tolist(),
            "latent": model.latent(model.norm(obs)).tolist(),
        }
    with open(str(path) + ".json", "w") as f:
        json.dump(side, f)
