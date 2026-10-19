// The Music Lab's audio thread: the music engine (music.wasm, the Rust
// crate crates/music) as a lab session (crates/music/src/lab.rs). The page
// posts the same messages the JS lab's worklet took (track, play, stop,
// patch, kit, audition, on, off, hit, energy, mix, mute, solo); they go to
// the wasm as JSON. About 15 times a second this posts back
// { type: 'pos', pos, gr, load } and any { type: 'section' | 'end' }.
//
// The page compiles the wasm and hands the WebAssembly.Module over in
// processorOptions (a worklet scope cannot fetch), with the seed. A
// worklet scope has no TextEncoder or TextDecoder, so the UTF-8 is done by
// hand below.

const BLOCK = 128;

function utf8(s) {
  const out = [];
  for (const ch of s) {
    let c = ch.codePointAt(0);
    if (c < 0x80) out.push(c);
    else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 63));
    else if (c < 0x10000) out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
    else out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
  }
  return out;
}

function fromUtf8(b) {
  let s = '';
  for (let i = 0; i < b.length;) {
    const c = b[i++];
    let cp;
    if (c < 0x80) cp = c;
    else if (c < 0xe0) cp = ((c & 31) << 6) | (b[i++] & 63);
    else if (c < 0xf0) cp = ((c & 15) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63);
    else cp = ((c & 7) << 18) | ((b[i++] & 63) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63);
    s += String.fromCodePoint(cp);
  }
  return s;
}

const now = globalThis.performance?.now ? () => globalThis.performance.now() : () => Date.now();

class MusicLab extends AudioWorkletProcessor {
  constructor(opts) {
    super();
    const o = opts?.processorOptions || {};
    this.x = new WebAssembly.Instance(o.module, {}).exports;
    this.w = this.x.music_lab_new(sampleRate, o.seed ?? 5);
    this.n = 0; this.busy = 0; this.blocks = 0;
    this.port.onmessage = (m) => this.send(m.data);
    for (const m of o.messages || []) this.send(m);
  }
  text(ptr) {
    return fromUtf8(new Uint8Array(this.x.memory.buffer, ptr, this.x.music_text_len()));
  }
  send(msg) {
    const x = this.x;
    const b = utf8(JSON.stringify(msg));
    const ptr = x.music_alloc(b.length);
    new Uint8Array(x.memory.buffer, ptr, b.length).set(b);
    if (x.music_lab_msg(this.w, ptr, b.length) < 0) {
      this.port.postMessage({ type: 'error', message: this.text(x.music_text()), msg });
    }
  }
  process(_in, outputs) {
    const x = this.x;
    const t0 = now();
    const ptr = x.music_lab_process(this.w);
    this.busy += now() - t0; this.blocks++;
    // A fresh view every block: the wasm's memory may have grown.
    const lr = new Float32Array(x.memory.buffer, ptr, 2 * BLOCK);
    const out = outputs[0];
    out[0].set(lr.subarray(0, BLOCK));
    if (out.length > 1) out[1].set(lr.subarray(BLOCK, 2 * BLOCK));
    if (++this.n >= 25) {
      this.n = 0;
      const p = JSON.parse(this.text(x.music_lab_poll(this.w)));
      for (const e of p.events) this.port.postMessage(e);
      const budget = (this.blocks * BLOCK * 1000) / sampleRate;
      this.port.postMessage({ type: 'pos', pos: p.pos, gr: p.gr, load: budget ? this.busy / budget : null });
      this.busy = 0; this.blocks = 0;
    }
    return true;
  }
}

registerProcessor('music-lab', MusicLab);
