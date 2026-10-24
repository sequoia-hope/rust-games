// Simulation-driven sound effects processor for Mech Works (docs/AUDIO.md).
// Runs `mw_audio.wasm` on the Web Audio rendering thread: the main thread posts
// per-step telemetry frames (contact forces, slip speeds, 12-joint currents,
// torques, temperatures, trips/faults, projectile impacts, UI events), and
// `process()` synthesizes 128-frame stereo blocks at the audio sample rate.

const BLOCK = 128;

class MwSfxProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const { module, seed } = options.processorOptions;
    this.x = new WebAssembly.Instance(module, {}).exports;
    this.x.mw_audio_init(sampleRate, seed || 0x6d776175);
    this.port.onmessage = (e) => this.onMessage(e.data);
  }

  onMessage(msg) {
    const x = this.x;
    if (msg.steps) {
      for (const f of msg.steps) {
        x.mw_audio_set_body(
          f.mass ?? 4000,
          f.dust ?? 0,
          f.term ? 1 : 0,
          f.vx ?? 0,
          f.vy ?? 0,
          f.vz ?? 0,
          f.wx ?? 0,
          f.wy ?? 0,
          f.wz ?? 0,
        );
        if (f.feet) {
          for (let i = 0; i < 2 && i < f.feet.length; i++) {
            const ft = f.feet[i];
            x.mw_audio_set_foot(i, ft[0] ? 1 : 0, ft[1], ft[2], ft[3]);
          }
        }
        if (f.joints) {
          for (let j = 0; j < 12 && j < f.joints.length; j++) {
            const jt = f.joints[j];
            x.mw_audio_set_joint(
              j,
              jt[0],
              jt[1],
              jt[2],
              jt[3],
              jt[4],
              jt[5],
              jt[6],
              jt[7],
              jt[8] ? 1 : 0,
              jt[9],
            );
          }
        }
        if (f.impacts) {
          for (const imp of f.impacts) {
            x.mw_audio_push_impact(imp[0], imp[1], imp[2]);
          }
        }
        if (f.ui) {
          for (const u of f.ui) {
            x.mw_audio_push_ui(u);
          }
        }
        x.mw_audio_commit_step();
      }
    }
    if (msg.ui !== undefined) {
      x.mw_audio_push_ui(msg.ui);
      x.mw_audio_commit_step();
    }
  }

  process(inputs, outputs) {
    const x = this.x;
    x.mw_audio_render_128();
    const lPtr = x.mw_audio_out_l_ptr();
    const rPtr = x.mw_audio_out_r_ptr();
    const out = outputs[0];
    if (lPtr && rPtr && out && out.length > 0) {
      out[0].set(new Float32Array(x.memory.buffer, lPtr, BLOCK));
      if (out.length > 1) {
        out[1].set(new Float32Array(x.memory.buffer, rPtr, BLOCK));
      }
    }
    return true;
  }
}

registerProcessor('mw-sfx', MwSfxProcessor);
