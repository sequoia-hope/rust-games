//! The estimator: infers the teacher's body latent z from the last second
//! of proprioception, so the policy walks without being told its body (the
//! adaptation half of Rapid Motor Adaptation; `train/estimator.py` trains
//! and exports it).
//!
//! Each policy step's proprioception is normalized and embedded once, into
//! a ring of the last `window` steps; [`Estimator::estimate`] runs the 1D
//! convolutions over the ring, oldest step first, and the two heads: z and
//! the body parameters (an auxiliary guess, for the demo's bars).

use serde::Deserialize;

use crate::{Activation, Error, LayerSpec, Mlp, TensorSpec, dense_net, err, split_blob, tensor};

#[derive(Clone, Debug, Deserialize)]
pub struct ConvSpec {
    #[serde(rename = "in")]
    pub inputs: usize,
    #[serde(rename = "out")]
    pub outputs: usize,
    pub kernel: usize,
    pub stride: usize,
    pub activation: String,
}

#[derive(Clone, Debug, Deserialize)]
struct EstimatorLayers {
    embed: Vec<LayerSpec>,
    conv: Vec<ConvSpec>,
    head_z: Vec<LayerSpec>,
    head_params: Vec<LayerSpec>,
}

/// The estimator blob's header.
#[derive(Clone, Debug, Deserialize)]
pub struct EstimatorHeader {
    pub arch: String,
    pub proprio: usize,
    /// History length, policy steps.
    pub window: usize,
    pub latent: usize,
    pub params: usize,
    pub obs_clip: f32,
    layers: EstimatorLayers,
    tensors: Vec<TensorSpec>,
}

/// A 1D convolution over time. Activations are time-major: step t's
/// channels are `x[t * channels..][..channels]`; the weight is PyTorch's
/// (out, in, kernel).
#[derive(Clone, Debug)]
struct Conv1d {
    inputs: usize,
    outputs: usize,
    kernel: usize,
    stride: usize,
    weight: Vec<f32>,
    bias: Vec<f32>,
    activation: Activation,
}

impl Conv1d {
    fn steps_out(&self, steps_in: usize) -> usize {
        (steps_in - self.kernel) / self.stride + 1
    }

    fn forward(&self, x: &[f32], steps_in: usize, out: &mut [f32]) {
        let (ci, k) = (self.inputs, self.kernel);
        for t in 0..self.steps_out(steps_in) {
            let window = &x[t * self.stride * ci..][..k * ci];
            for (o, b) in self.bias.iter().enumerate() {
                let mut acc = *b;
                for (i, w) in self.weight[o * ci * k..][..ci * k]
                    .chunks_exact(k)
                    .enumerate()
                {
                    for (j, w) in w.iter().enumerate() {
                        acc += w * window[j * ci + i];
                    }
                }
                out[t * self.outputs + o] = self.activation.apply(acc);
            }
        }
    }
}

/// The estimator and its history.
#[derive(Clone, Debug)]
pub struct Estimator {
    pub header: EstimatorHeader,
    mean: Vec<f32>,
    std: Vec<f32>,
    embed: Mlp,
    convs: Vec<Conv1d>,
    head_z: Mlp,
    head_params: Mlp,
    /// The last `window` steps' embeddings; slot `next` is the oldest.
    ring: Vec<f32>,
    next: usize,
    norm: Vec<f32>,
    bufs: [Vec<f32>; 2],
    latent: Vec<f32>,
    params: Vec<f32>,
}

impl Estimator {
    /// Reads a blob.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let (hjson, floats) = split_blob(bytes)?;
        let header: EstimatorHeader =
            serde_json::from_slice(hjson).map_err(|e| Error(format!("header: {e}")))?;
        if header.arch != "estimator_v1" {
            return err(format!("unknown architecture {:?}", header.arch));
        }
        let ts = &header.tensors;
        let l = &header.layers;
        let embed = dense_net(ts, &floats, "embed", &l.embed)?;
        let head_z = dense_net(ts, &floats, "head_z", &l.head_z)?;
        let head_params = dense_net(ts, &floats, "head_params", &l.head_params)?;
        let mut convs = Vec::new();
        let (mut channels, mut steps) = (embed.outputs(), header.window);
        let mut widest = channels * steps;
        for (i, s) in l.conv.iter().enumerate() {
            let weight = tensor(ts, &floats, &format!("conv.{i}.weight"))?;
            let bias = tensor(ts, &floats, &format!("conv.{i}.bias"))?;
            if s.inputs != channels
                || weight.len() != s.outputs * s.inputs * s.kernel
                || bias.len() != s.outputs
                || s.kernel > steps
                || s.stride == 0
            {
                return err(format!("conv.{i}: shape mismatch"));
            }
            let c = Conv1d {
                inputs: s.inputs,
                outputs: s.outputs,
                kernel: s.kernel,
                stride: s.stride,
                weight,
                bias,
                activation: Activation::parse(&s.activation)?,
            };
            steps = c.steps_out(steps);
            channels = c.outputs;
            widest = widest.max(channels * steps);
            convs.push(c);
        }
        let flat = channels * steps;
        let first_in = |m: &Mlp| m.layers.first().map_or(0, |d| d.inputs);
        if first_in(&head_z) != flat || first_in(&head_params) != flat {
            return err("heads' inputs differ from the convolutions' output");
        }
        if first_in(&embed) != header.proprio
            || head_z.outputs() != header.latent
            || head_params.outputs() != header.params
        {
            return err("network sizes differ from the header");
        }
        let mean = tensor(ts, &floats, "norm.mean")?;
        let std = tensor(ts, &floats, "norm.std")?;
        if mean.len() != header.proprio || std.len() != header.proprio {
            return err("normalization length differs from proprio");
        }
        Ok(Self {
            ring: vec![0.0; header.window * embed.outputs()],
            next: 0,
            norm: vec![0.0; header.proprio],
            bufs: [vec![0.0; widest], vec![0.0; widest]],
            latent: vec![0.0; header.latent],
            params: vec![0.0; header.params],
            mean,
            std,
            embed,
            convs,
            head_z,
            head_params,
            header,
        })
    }

    fn embed_into(&mut self, obs: &[f32], slot: usize) {
        let h = &self.header;
        assert!(obs.len() >= h.proprio, "observation length");
        for (((n, o), m), s) in self.norm.iter_mut().zip(obs).zip(&self.mean).zip(&self.std) {
            *n = ((o - m) / s).clamp(-h.obs_clip, h.obs_clip);
        }
        let c = self.embed.outputs();
        self.embed
            .forward(&self.norm, &mut self.ring[slot * c..][..c]);
    }

    /// Starts a history: every step of it is `obs` (an observation or just
    /// its proprioception part), as training began each episode.
    pub fn reset(&mut self, obs: &[f32]) {
        self.embed_into(obs, 0);
        let c = self.embed.outputs();
        let (first, rest) = self.ring.split_at_mut(c);
        for slot in rest.chunks_exact_mut(c) {
            slot.copy_from_slice(first);
        }
        self.next = 0;
    }

    /// Adds a policy step's observation to the history.
    pub fn push(&mut self, obs: &[f32]) {
        self.embed_into(obs, self.next);
        self.next = (self.next + 1) % self.header.window;
    }

    /// Runs the network on the history; then read [`Estimator::latent`] and
    /// [`Estimator::params`].
    pub fn estimate(&mut self) {
        let c = self.embed.outputs();
        let w = self.header.window;
        let [a, b] = &mut self.bufs;
        // The ring in order, oldest first.
        let split = self.next * c;
        a[..w * c - split].copy_from_slice(&self.ring[split..]);
        a[w * c - split..w * c].copy_from_slice(&self.ring[..split]);
        let (mut read, mut write) = (a, b);
        let mut steps = w;
        for conv in &self.convs {
            conv.forward(read, steps, write);
            steps = conv.steps_out(steps);
            std::mem::swap(&mut read, &mut write);
        }
        let flat = &read[..steps * self.convs.last().map_or(c, |l| l.outputs)];
        self.head_z.forward(flat, &mut self.latent);
        self.head_params.forward(flat, &mut self.params);
    }

    /// The estimated latent z, for [`crate::Policy::act_with_latent`].
    pub fn latent(&self) -> &[f32] {
        &self.latent
    }

    /// The estimated body parameters, normalized as `BodyParams` are.
    pub fn params(&self) -> &[f32] {
        &self.params
    }
}
