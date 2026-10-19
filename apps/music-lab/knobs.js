// The lab page's knobs: per instrument kind, per FM operator and per drum
// voice type, [key, label, min, max, step]. The keys are the patch
// parameters' names in the music engine (crates/music: Lab, KitVoice).
// Copied from the JS lab (Midnight Playground's tools/music-lab/
// instruments.js, now frozen); the engine's tables themselves come from the
// wasm (music_tables).

// Knobs for the page, per kind: [key, label, min, max, step].
export const KNOBS = {
  tb303: [['cutoff', 'Cutoff (Hz)', 60, 3000, 1], ['res', 'Resonance', 0, 1, 0.01], ['env', 'Env mod', 0, 1, 0.01], ['decay', 'Decay (s)', 0.05, 2, 0.01], ['accent', 'Accent', 0, 1, 0.01], ['drive', 'Drive', 0, 3, 0.01], ['glide', 'Slide time (s)', 0.01, 0.2, 0.001], ['gain', 'Level', 0, 1.5, 0.01]],
  juno: [['saw', 'Saw', 0, 1, 0.01], ['pulse', 'Pulse', 0, 1, 0.01], ['pw', 'Pulse width', 0.05, 0.95, 0.01], ['pwm', 'PWM (LFO)', 0, 1, 0.01], ['sub', 'Sub', 0, 1, 0.01], ['noise', 'Noise', 0, 0.5, 0.01], ['unison', 'Unison', 1, 7, 1], ['detune', 'Detune (c)', 0, 40, 0.5], ['hpf', 'HPF (Hz)', 20, 1000, 1], ['cutoff', 'Cutoff (Hz)', 80, 12000, 1], ['res', 'Resonance', 0, 1, 0.01], ['fenv', 'Env (oct)', 0, 5, 0.01], ['keytrack', 'Keytrack', 0, 1, 0.01], ['fa', 'F attack', 0.001, 3, 0.001], ['fd', 'F decay', 0.01, 3, 0.01], ['fs', 'F sustain', 0, 1, 0.01], ['a', 'Attack', 0.001, 3, 0.001], ['d', 'Decay', 0.01, 3, 0.01], ['s', 'Sustain', 0, 1, 0.01], ['r', 'Release', 0.01, 4, 0.01], ['lfoRate', 'LFO (Hz)', 0.05, 8, 0.01], ['chorus', 'Chorus (0, I, II, I+II)', 0, 3, 1], ['drive', 'Drive', 0, 3, 0.01], ['gain', 'Level', 0, 1.5, 0.01]],
  mono: [['cutoff', 'Cutoff (Hz)', 40, 12000, 1], ['res', 'Resonance', 0, 1, 0.01], ['fenv', 'Env (oct)', 0, 5, 0.01], ['keytrack', 'Keytrack', 0, 1, 0.01], ['drive', 'Drive', 0, 4, 0.01], ['sub', 'Sub', 0, 1, 0.01], ['noise', 'Noise', 0, 0.5, 0.01], ['fa', 'F attack', 0.001, 2, 0.001], ['fd', 'F decay', 0.01, 2, 0.01], ['fs', 'F sustain', 0, 1, 0.01], ['a', 'Attack', 0.001, 2, 0.001], ['d', 'Decay', 0.01, 2, 0.01], ['s', 'Sustain', 0, 1, 0.01], ['r', 'Release', 0.01, 3, 0.01], ['glide', 'Glide (s)', 0, 0.4, 0.001], ['vib', 'Vibrato (c)', 0, 40, 0.5], ['gain', 'Level', 0, 1.5, 0.01]],
  fm: [['fbk', 'Feedback', 0, 1.5, 0.01], ['r', 'Release', 0.01, 3, 0.01], ['chorus', 'Chorus (0, I, II, I+II)', 0, 3, 1], ['gain', 'Level', 0, 1.5, 0.01]],
  bowed: [['cutoff', 'Cutoff (Hz)', 200, 12000, 1], ['res', 'Resonance', 0, 1, 0.01], ['drive', 'Drive', 0, 3, 0.01], ['noise', 'Bow noise', 0, 0.5, 0.01], ['a', 'Attack', 0.001, 1, 0.001], ['d', 'Decay', 0.01, 3, 0.01], ['s', 'Sustain', 0, 1, 0.01], ['r', 'Release', 0.01, 3, 0.01], ['vib', 'Vibrato (c)', 0, 60, 0.5], ['vibRate', 'Vibrato rate (Hz)', 2, 9, 0.01], ['vibDelay', 'Vibrato delay (s)', 0, 1, 0.01], ['glide', 'Glide (s)', 0, 0.4, 0.001], ['body', 'Body (Hz)', 60, 1200, 1], ['bodyQ', 'Body Q', 0.3, 8, 0.01], ['bodyMix', 'Body mix', 0, 1.5, 0.01], ['body2', 'Body 2 (Hz)', 60, 2000, 1], ['body2Mix', 'Body 2 mix', 0, 1.5, 0.01], ['dry', 'Dry', 0, 1, 0.01], ['gain', 'Level', 0, 1.5, 0.01]],
  string: [['decay', 'Ring (s)', 0.05, 8, 0.01], ['damp', 'Damping', 0, 0.5, 0.01], ['pick', 'Pick', 0, 1, 0.01], ['pickPos', 'Pick position', 0, 0.5, 0.01], ['body', 'Body (Hz)', 100, 6000, 1], ['bodyQ', 'Body Q', 0.3, 6, 0.01], ['bodyMix', 'Body mix', 0, 1.5, 0.01], ['tone', 'Tone (Hz)', 500, 12000, 1], ['drive', 'Drive', 0, 3, 0.01], ['wah', 'Wah (oct)', 0, 4, 0.01], ['wahHz', 'Wah from (Hz)', 100, 1500, 1], ['wahRate', 'Wah rate (Hz)', 0.1, 8, 0.01], ['trem', 'Tremolo', 0, 1, 0.01], ['tremRate', 'Tremolo rate (Hz)', 1, 12, 0.01], ['bend', 'Slide in (semi)', 0, 3, 0.01], ['bendT', 'Slide time (s)', 0.01, 0.3, 0.001], ['glide', 'Glide (s)', 0, 0.4, 0.001], ['strum', 'Strum (s)', 0, 0.06, 0.001], ['swell', 'Swell (s)', 0, 1, 0.01], ['vib', 'Vibrato (c)', 0, 60, 0.5], ['body2', 'Body 2 (Hz)', 0, 2000, 1], ['body2Mix', 'Body 2 mix', 0, 1.5, 0.01], ['strings', 'Strings a note', 1, 3, 1], ['r', 'Mute (s)', 0.01, 3, 0.01], ['chorus', 'Chorus (0, I, II, I+II)', 0, 3, 1], ['gain', 'Level', 0, 1.5, 0.01]],
};
// FM operators get their own knobs: index/level, ratio and decay per op.
export const FM_OP_KNOBS = [['l', 'level', 0, 4, 0.01], ['r', 'ratio', 0.5, 16, 0.01], ['d', 'decay', 0.01, 4, 0.01], ['s', 'sustain', 0, 1, 0.01]];
// Drum knobs, per voice type.
export const DRUM_KNOBS = {
  kick808: [['tune', 'Tune (Hz)', 30, 90, 0.5], ['decay', 'Decay', 0.1, 2.5, 0.01], ['pitch', 'Pitch sweep', 1, 5, 0.01], ['click', 'Click', 0, 1.5, 0.01], ['drive', 'Drive', 0.5, 4, 0.01], ['lvl', 'Level', 0, 2, 0.01]],
  snare808: [['tune', 'Tune (Hz)', 120, 300, 1], ['tone', 'Snappy mix', 0, 1, 0.01], ['snappy', 'Snappy decay', 0.03, 0.5, 0.005], ['decay', 'Tone decay', 0.03, 0.4, 0.005], ['lvl', 'Level', 0, 2, 0.01]],
  clap: [['bp', 'Tone (Hz)', 600, 2500, 1], ['tail', 'Tail', 0.05, 0.6, 0.01], ['bursts', 'Bursts', 1, 6, 1], ['spacing', 'Spacing', 0.004, 0.025, 0.0005], ['lvl', 'Level', 0, 2, 0.01]],
  metal: [['tune', 'Tune', 0.5, 2.5, 0.01], ['decay', 'Decay', 0.01, 2, 0.005], ['hp', 'HPF (Hz)', 2000, 12000, 10], ['noise', 'Noise', 0, 1, 0.01], ['lvl', 'Level', 0, 2, 0.01]],
  cymbal: [['tune', 'Tune', 0.5, 2.5, 0.01], ['decay', 'Decay', 0.2, 4, 0.01], ['noise', 'Noise', 0, 1, 0.01], ['lvl', 'Level', 0, 2, 0.01]],
  tom: [['tune', 'Tune (Hz)', 50, 300, 1], ['decay', 'Decay', 0.05, 1.5, 0.01], ['pitch', 'Pitch sweep', 1, 3, 0.01], ['noise', 'Noise', 0, 0.5, 0.01], ['lvl', 'Level', 0, 2, 0.01]],
  default: [['decay', 'Decay', 0.01, 2, 0.005], ['lvl', 'Level', 0, 2, 0.01]],
};
DRUM_KNOBS.kick909 = DRUM_KNOBS.kick808;
DRUM_KNOBS.snare909 = DRUM_KNOBS.snare808;
DRUM_KNOBS.boom = DRUM_KNOBS.tom;
DRUM_KNOBS.conga = [['tune', 'Tune (Hz)', 100, 600, 1], ['decay', 'Decay', 0.05, 1, 0.01], ['slap', 'Slap', 0, 1, 1], ['lvl', 'Level', 0, 2, 0.01]];
DRUM_KNOBS.brush = [['len', 'Stroke (s)', 0.02, 0.4, 0.005], ['bp', 'Tone (Hz)', 1000, 8000, 10], ['lvl', 'Level', 0, 2, 0.01]];
DRUM_KNOBS.guiro = [['len', 'Stroke (s)', 0.02, 0.4, 0.005], ['rate0', 'Ridges from (Hz)', 20, 300, 1], ['rate1', 'Ridges to (Hz)', 20, 400, 1], ['bp', 'Tone (Hz)', 1000, 6000, 10], ['lvl', 'Level', 0, 2, 0.01]];
