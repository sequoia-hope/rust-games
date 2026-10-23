// Offline renders and numbers, so a change can be checked without ears:
// the lab's worklet (the Rust engine) on an OfflineAudioContext, then
// loudness, peak, crest factor, stereo width and the balance of six bands
// against the low band (40–150 Hz). The band split is the JS engine lab's
// (Midnight Playground's tools/engine-lab/measure.js).

export const WORKLET_URL = new URL('lab-worklet.js?v=20261023c', import.meta.url);

// The bar where the song is fullest: the first section playing the most parts.
export function fullBar(T) {
  let best = 0, most = -1, bar = 0;
  for (const s of T.sections) {
    const n = Object.keys(s.p || {}).length + (s.drums ? 1 : 0);
    if (n > most) { most = n; best = bar; }
    bar += s.bars;
  }
  return best;
}

// `load`: the message that loads the song (a 'track' or a 'song');
// `module`: the compiled music.wasm.
export async function render(module, load, { bar = 0, secs = 12, rate = 48000, mix, seed = 5 } = {}) {
  const oc = new OfflineAudioContext(2, Math.ceil(secs * rate), rate);
  await oc.audioWorklet.addModule(WORKLET_URL);
  const messages = [
    load,
    ...(mix ? [{ type: 'mix', ...mix }] : []),
    { type: 'play', bar, delay: 0 },
  ];
  const node = new AudioWorkletNode(oc, 'music-lab', {
    numberOfInputs: 0, outputChannelCount: [2],
    processorOptions: { module, seed, messages },
  });
  node.connect(oc.destination);
  return oc.startRendering();
}

function fft(re, im) {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) { [re[i], re[j]] = [re[j], re[i]]; [im[i], im[j]] = [im[j], im[i]]; }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const ang = (-2 * Math.PI) / len, wr = Math.cos(ang), wi = Math.sin(ang);
    for (let i = 0; i < n; i += len) {
      let cr = 1, ci = 0;
      for (let j = 0; j < len / 2; j++) {
        const a = i + j, b = a + len / 2;
        const tr = re[b] * cr - im[b] * ci, ti = re[b] * ci + im[b] * cr;
        re[b] = re[a] - tr; im[b] = im[a] - ti; re[a] += tr; im[a] += ti;
        const ncr = cr * wr - ci * wi; ci = cr * wi + ci * wr; cr = ncr;
      }
    }
  }
}

// Stats over `buf` from `skip` seconds on.
export function analyse(buf, skip = 0.5) {
  const rate = buf.sampleRate, L = buf.getChannelData(0), R = buf.getChannelData(1);
  const i0 = Math.floor(skip * rate), n = buf.length - i0;
  let peak = 0, sum = 0, mid = 0, side = 0;
  const mono = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const l = L[i + i0], r = R[i + i0];
    peak = Math.max(peak, Math.abs(l), Math.abs(r));
    sum += (l * l + r * r) / 2;
    mono[i] = (l + r) / 2;
    const m = (l + r) / 2, s = (l - r) / 2;
    mid += m * m; side += s * s;
  }
  const F = 8192, hop = 4096, bands = { sub: [20, 40], low: [40, 150], lowmid: [150, 600], mid: [600, 1000], high: [1000, 4000], air: [4000, 12000] };
  const pw = Object.fromEntries(Object.keys(bands).map((k) => [k, 0]));
  const win = new Float32Array(F).map((_, i) => 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (F - 1)));
  for (let s = 0; s + F <= n; s += hop) {
    const re = new Float32Array(F), im = new Float32Array(F);
    for (let i = 0; i < F; i++) re[i] = mono[s + i] * win[i];
    fft(re, im);
    for (let k = 1; k < F / 2; k++) {
      const f = (k * rate) / F, e = re[k] * re[k] + im[k] * im[k];
      for (const [b, [lo, hi]] of Object.entries(bands)) if (f >= lo && f < hi) pw[b] += e;
    }
  }
  const db = (x) => +(10 * Math.log10(x + 1e-20)).toFixed(1);
  const rmsDb = db(sum / n), peakDb = 20 * Math.log10(peak + 1e-9);
  return {
    peak: +peak.toFixed(3), rmsDb,
    crestDb: +(peakDb - rmsDb).toFixed(1),
    width: +Math.sqrt(side / (mid + 1e-20)).toFixed(2),
    bandsDb: Object.fromEntries(Object.entries(pw).map(([k, v]) => [k, +(db(v) - db(pw.low)).toFixed(1)])),
  };
}
