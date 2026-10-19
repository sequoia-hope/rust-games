// The radio's processor in a browser: the music engine's station player
// (the schedule, the songs and the tuner) as its own small wasm
// (music.wasm, no wasm-bindgen), run on the audio thread. A game's native
// backend runs the same player with the same calls in the same order, so
// both render alike.
//
// The page compiles the wasm and hands over, in processorOptions (a
// worklet scope cannot fetch):
// - `module`: the WebAssembly.Module, instantiated here, synchronously,
//   once per node;
// - `seed`: the player's seed;
// - `stations`: the game's station table, its JSON as a Uint8Array
//   (encoded by the page: a worklet scope has no TextEncoder).

const BLOCK = 128;

class RadioProcessor extends AudioWorkletProcessor {
  // Read once per block (k-rate). `station` is an index in the game's
  // station table (-1: off); `wallDay` and `wallSec` are the wall time at
  // the moment of tuning (days since the Unix epoch, and seconds into the
  // day: an f32 cannot hold Unix seconds); `tune` is a serial that
  // re-syncs when it changes; `energy` the game's intensity.
  static get parameterDescriptors() {
    const k = (name, defaultValue, minValue, maxValue) =>
      ({ name, defaultValue, minValue, maxValue, automationRate: 'k-rate' });
    return [
      k('station', -1, -1, 63),
      k('wallDay', 0, 0, 1e6),
      k('wallSec', 0, 0, 86400),
      k('tune', 0, 0, 1e9),
      k('energy', 1, 0, 1),
    ];
  }

  constructor(options) {
    super();
    const { module, seed, stations } = options.processorOptions;
    const x = (this.x = new WebAssembly.Instance(module, {}).exports);
    const ptr = x.music_alloc(stations.length);
    new Uint8Array(x.memory.buffer, ptr, stations.length).set(stations);
    if (x.music_set_stations(ptr, stations.length) < 0) {
      throw new Error('music-radio: the station table is bad');
    }
    this.w = x.music_new(sampleRate, seed);
  }

  process(inputs, outputs, params) {
    const x = this.x;
    x.music_params(this.w, params.station[0], params.wallDay[0], params.wallSec[0], params.tune[0], params.energy[0]);
    const ptr = x.music_process(this.w);
    // A fresh view every block: the wasm's memory may have grown (and its
    // old buffer detached).
    const lr = new Float32Array(x.memory.buffer, ptr, 2 * BLOCK);
    const out = outputs[0];
    out[0].set(lr.subarray(0, BLOCK));
    if (out.length > 1) out[1].set(lr.subarray(BLOCK, 2 * BLOCK));
    // A source that keeps playing (silence while off) until the page lets
    // go of it.
    return true;
  }
}

registerProcessor('music-radio', RadioProcessor);
