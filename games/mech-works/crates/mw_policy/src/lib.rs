//! The trained networks, run in plain Rust: a few hundred thousand weights
//! take microseconds on a CPU, so there is no ONNX runtime and no GPU
//! round trip. Loads the blob `train/policy.py` exports (its docstring has
//! the layout) and runs it without allocating per step.

use serde::Deserialize;

/// What went wrong reading a blob.
#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

fn err<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error(msg.into()))
}

pub const BLOB_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
pub struct LayerSpec {
    #[serde(rename = "in")]
    pub inputs: usize,
    #[serde(rename = "out")]
    pub outputs: usize,
    pub activation: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TensorSpec {
    name: String,
    shape: Vec<usize>,
    offset: usize,
}

#[derive(Clone, Debug, Deserialize)]
struct Layers {
    encoder: Vec<LayerSpec>,
    actor: Vec<LayerSpec>,
}

/// The blob's header.
#[derive(Clone, Debug, Deserialize)]
pub struct Header {
    pub arch: String,
    pub obs_dim: usize,
    pub proprio: usize,
    pub params: usize,
    pub latent: usize,
    pub act_dim: usize,
    pub obs_clip: f32,
    layers: Layers,
    tensors: Vec<TensorSpec>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Activation {
    None,
    Elu,
    Tanh,
}

impl Activation {
    fn parse(s: &str) -> Result<Self, Error> {
        match s {
            "none" => Ok(Self::None),
            "elu" => Ok(Self::Elu),
            "tanh" => Ok(Self::Tanh),
            other => err(format!("unknown activation {other:?}")),
        }
    }

    fn apply(self, x: f32) -> f32 {
        match self {
            Self::None => x,
            Self::Elu => {
                if x > 0.0 {
                    x
                } else {
                    x.exp_m1()
                }
            }
            Self::Tanh => x.tanh(),
        }
    }
}

/// A dense layer: `out = act(W · in + b)`, W row-major (out × in).
#[derive(Clone, Debug)]
struct Dense {
    inputs: usize,
    outputs: usize,
    weight: Vec<f32>,
    bias: Vec<f32>,
    activation: Activation,
}

impl Dense {
    fn forward(&self, x: &[f32], out: &mut [f32]) {
        for (o, (row, b)) in out
            .iter_mut()
            .zip(self.weight.chunks_exact(self.inputs).zip(&self.bias))
        {
            let mut acc = *b;
            for (w, v) in row.iter().zip(x) {
                acc += w * v;
            }
            *o = self.activation.apply(acc);
        }
    }
}

/// A chain of dense layers with scratch space for its activations.
#[derive(Clone, Debug)]
struct Mlp {
    layers: Vec<Dense>,
    scratch: [Vec<f32>; 2],
}

impl Mlp {
    fn new(layers: Vec<Dense>) -> Self {
        let widest = layers.iter().map(|l| l.outputs).max().unwrap_or(0);
        Self {
            layers,
            scratch: [vec![0.0; widest], vec![0.0; widest]],
        }
    }

    fn outputs(&self) -> usize {
        self.layers.last().map_or(0, |l| l.outputs)
    }

    /// Layer i reads scratch[(i + 1) % 2] (or `x`) and writes scratch[i % 2]
    /// (or `out`, for the last).
    fn forward(&mut self, x: &[f32], out: &mut [f32]) {
        let n = self.layers.len();
        let [a, b] = &mut self.scratch;
        for (i, layer) in self.layers.iter().enumerate() {
            let (read, write) = if i % 2 == 0 {
                (&*b, &mut *a)
            } else {
                (&*a, &mut *b)
            };
            let input = if i == 0 { x } else { &read[..layer.inputs] };
            if i == n - 1 {
                layer.forward(input, out);
            } else {
                layer.forward(input, &mut write[..layer.outputs]);
            }
        }
    }
}

/// The teacher: normalized observation → encoder(body parameters) = z →
/// actor(proprioception, z) = action.
#[derive(Clone, Debug)]
pub struct Policy {
    pub header: Header,
    mean: Vec<f32>,
    std: Vec<f32>,
    encoder: Mlp,
    actor: Mlp,
    norm: Vec<f32>,
    actor_in: Vec<f32>,
    latent: Vec<f32>,
}

impl Policy {
    /// Reads a blob.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 16 || &bytes[..8] != b"MWPOLICY" {
            return err("not a Mech Works policy blob");
        }
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let version = u32_at(8);
        if version != BLOB_VERSION {
            return err(format!(
                "blob version {version}, this build reads {BLOB_VERSION}"
            ));
        }
        let hlen = u32_at(12) as usize;
        let hend = 16 + hlen;
        if bytes.len() < hend {
            return err("truncated header");
        }
        let header: Header =
            serde_json::from_slice(&bytes[16..hend]).map_err(|e| Error(format!("header: {e}")))?;
        if header.arch != "teacher_v1" {
            return err(format!("unknown architecture {:?}", header.arch));
        }
        let data_start = hend + (4 - hend % 4) % 4;
        let data = &bytes[data_start.min(bytes.len())..];
        let floats: Vec<f32> = data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        let tensor = |name: &str| -> Result<Vec<f32>, Error> {
            let t = header
                .tensors
                .iter()
                .find(|t| t.name == name)
                .ok_or_else(|| Error(format!("missing tensor {name}")))?;
            let len: usize = t.shape.iter().product();
            floats
                .get(t.offset..t.offset + len)
                .map(<[f32]>::to_vec)
                .ok_or_else(|| Error(format!("tensor {name} runs past the data")))
        };
        let net = |name: &str, specs: &[LayerSpec]| -> Result<Mlp, Error> {
            let mut layers = Vec::new();
            for (i, s) in specs.iter().enumerate() {
                let weight = tensor(&format!("{name}.{i}.weight"))?;
                let bias = tensor(&format!("{name}.{i}.bias"))?;
                if weight.len() != s.inputs * s.outputs || bias.len() != s.outputs {
                    return err(format!("{name}.{i}: shape mismatch"));
                }
                layers.push(Dense {
                    inputs: s.inputs,
                    outputs: s.outputs,
                    weight,
                    bias,
                    activation: Activation::parse(&s.activation)?,
                });
            }
            Ok(Mlp::new(layers))
        };
        let encoder = net("encoder", &header.layers.encoder)?;
        let actor = net("actor", &header.layers.actor)?;
        let mean = tensor("norm.mean")?;
        let std = tensor("norm.std")?;
        if mean.len() != header.obs_dim || std.len() != header.obs_dim {
            return err("normalization length differs from obs_dim");
        }
        if encoder.outputs() != header.latent || actor.outputs() != header.act_dim {
            return err("network outputs differ from the header");
        }
        Ok(Self {
            norm: vec![0.0; header.obs_dim],
            actor_in: vec![0.0; header.proprio + header.latent],
            latent: vec![0.0; header.latent],
            mean,
            std,
            encoder,
            actor,
            header,
        })
    }

    /// The action for `obs` (`obs_dim` long) into `action` (`act_dim`).
    pub fn act(&mut self, obs: &[f32], action: &mut [f32]) {
        let h = &self.header;
        assert_eq!(obs.len(), h.obs_dim, "observation length");
        for (((n, o), m), s) in self.norm.iter_mut().zip(obs).zip(&self.mean).zip(&self.std) {
            *n = ((o - m) / s).clamp(-h.obs_clip, h.obs_clip);
        }
        self.encoder
            .forward(&self.norm[h.proprio..], &mut self.latent);
        self.actor_in[..h.proprio].copy_from_slice(&self.norm[..h.proprio]);
        self.actor_in[h.proprio..].copy_from_slice(&self.latent);
        self.actor.forward(&self.actor_in, action);
    }

    /// The latent z of the last [`Policy::act`].
    pub fn latent(&self) -> &[f32] {
        &self.latent
    }

    /// Weights in the deployed networks.
    pub fn parameter_count(&self) -> usize {
        let count = |m: &Mlp| {
            m.layers
                .iter()
                .map(|l| l.weight.len() + l.bias.len())
                .sum::<usize>()
        };
        count(&self.encoder) + count(&self.actor)
    }
}
