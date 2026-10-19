// Music Lab page: make a track from a genre and a seed, play it, and take
// it apart: its sections, its parts (mute, solo, live patch edits), every
// instrument on a keyboard, the drum machine on pads, the mix, a spectrum
// and offline measurements. Two generators: the new one (crates/music
// generator: per-instrument layers, research-backed profiles, surprise
// scoring), whose songs show their layers over time and the notes behind
// them, and the classic one (the games' radio today). Everything audible is the music engine
// (crates/music, Rust) in an AudioWorklet (lab-worklet.js, music.wasm); the
// page runs a second copy of the wasm on the main thread for the tables and
// to make the same track the worklet plays, to show it.
//
//   music-lab worklet ─ speaker (high-pass, mono) ─ volume ─ analyser ─ out

import { KNOBS, FM_OP_KNOBS, DRUM_KNOBS } from './knobs.js';
import { WORKLET_URL, fullBar, render, analyse } from './measure.js';

const $ = (id) => document.getElementById(id);
const status = (html, err = false) => { $('status').innerHTML = html; $('status').classList.toggle('err', err); };

// ── The engine on the main thread: tables and tracks ─────────────
const WASM_URL = new URL('music.wasm', import.meta.url);
const module = await WebAssembly.compileStreaming(fetch(WASM_URL));
const wasm = (await WebAssembly.instantiate(module, {})).exports;
const text = (ptr) => new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, ptr, wasm.music_text_len()));
function passText(s) {
  const b = new TextEncoder().encode(s);
  const ptr = wasm.music_alloc(b.length);
  new Uint8Array(wasm.memory.buffer, ptr, b.length).set(b);
  return [ptr, b.length];
}
const tables = JSON.parse(text(wasm.music_tables()));
const CLASSIC = Object.fromEntries(tables.genres);
const NEW = Object.fromEntries(tables.generators);
const GENRES = { ...NEW, ...CLASSIC };
const BPATCH = Object.fromEntries(tables.patches);
const KITS = Object.fromEntries(tables.kits.map(([k, lanes]) => [k, Object.fromEntries(lanes)]));
const KIT_NAME = { tr808: 'TR-808', tr909: 'TR-909', latin: 'Latin', country: 'Country' };
// A genre's track, as the worklet will make it.
function generate(genre, seed, dejavu) {
  const [ptr, len] = passText(genre);
  return JSON.parse(text(wasm.music_generate(ptr, len, seed, dejavu ?? -1)));
}
// A new generator's song (its summary: sections, layers, clips, notes).
function makeSong(genre, seed) {
  const [ptr, len] = passText(genre);
  return JSON.parse(text(wasm.music_song(ptr, len, seed, -1, -1)));
}
// A song shown with the classic track's fields, so the sections and parts
// tables serve both.
function asTrack(song) {
  const parts = {};
  for (const l of song.layers) if (l.lab) parts[l.name] = { type: l.role, lab: l.lab };
  return {
    title: song.title, style: song.style, bpm: song.bpm,
    sections: song.sections.map((s) => ({ bars: s.bars, drums: s.name, p: Object.fromEntries(song.layers.filter((l) => l.clips.some((c) => c.bar < s.bar + s.bars && c.bar + c.bars > s.bar)).map((l) => [l.name, 1])) })),
    parts,
  };
}

// A song is what makes it: (genre, seed, déjà vu), and the kit it plays on.
const made = [];
let cur = null, playing = false;
let ctx = null, lab = null, hp = null, mono = null, vol = null, analyser = null;
const mute = new Set();
let solo = null;
let kitTweak = {};

// ── Controls ─────────────────────────────────────────────────────
function fillSongs() {
  const s = $('song');
  s.innerHTML = '';
  made.forEach((S, i) => s.append(new Option(`${S.track.title} · ${S.track.style} · ${S.track.bpm} bpm`, String(i))));
  s.value = String(made.indexOf(cur));
}
$('song').onchange = () => selectSong(made[Number($('song').value)]);

function bindRange(el, fmt = (v) => v, on = null) {
  const out = el.parentElement.querySelector('output');
  const show = () => { if (out) out.textContent = fmt(Number(el.value)); };
  el.addEventListener('input', () => { show(); on?.(Number(el.value)); });
  show();
}
const pct = (v) => Math.round(v * 100) + '%';
const post = (m) => lab?.port.postMessage(m);
bindRange($('dejavu'), pct);
bindRange($('energy'), pct, (v) => post({ type: 'energy', value: v }));
for (const k of ['tape', 'glue', 'reverb', 'revTone']) bindRange($(k), pct, (v) => post({ type: 'mix', [k]: v }));
bindRange($('revSize'), (v) => v.toFixed(1), (v) => post({ type: 'mix', revSize: v }));
bindRange($('vol'), pct, (v) => vol?.gain.setTargetAtTime(v, ctx.currentTime, 0.03));
const mixNow = () => ({ tape: +$('tape').value, glue: +$('glue').value, reverb: +$('reverb').value, revSize: +$('revSize').value, revTone: +$('revTone').value });

// The speaker: what a laptop or a phone leaves of the low end (a 12 dB/oct
// high-pass), and a phone's single speaker (mono).
const SPEAKERS = { headphones: [0, false], big: [30, false], laptop: [170, false], phone: [330, true] };
function applySpeaker() {
  if (!ctx) return;
  const [hz, isMono] = SPEAKERS[$('speaker').value];
  hp.frequency.value = hz || 10;
  mono.channelCount = isMono ? 1 : 2;
}
$('speaker').onchange = applySpeaker;

for (const sel of [$('kit'), $('kit2')]) for (const k of Object.keys(KITS)) sel.append(new Option(KIT_NAME[k] ?? k, k));
function setKit(name) {
  $('kit').value = name; $('kit2').value = name;
  cur.kit = name;
  post({ type: 'kit', name, tweak: kitTweak });
  buildPads(); buildParts();
}
$('kit').onchange = () => setKit($('kit').value);
$('kit2').onchange = () => setKit($('kit2').value);

for (const [label, list] of [['New generator', NEW], ['Classic (the radio today)', CLASSIC]]) {
  const g = document.createElement('optgroup'); g.label = label;
  for (const [k, v] of Object.entries(list)) g.append(new Option(v, k));
  $('genre').append(g);
}
$('dice').onclick = () => { $('seed').value = 1 + Math.floor(Math.random() * 99999); makeTrack(true); };
$('make').onclick = () => makeTrack(true);
function makeTrack(autoplay) {
  const genre = $('genre').value, seed = Math.max(1, Number($('seed').value) | 0), dejavu = Number($('dejavu').value);
  let S;
  if (genre in NEW) {
    const song = makeSong(genre, seed);
    S = { genre, seed, song, track: asTrack(song), kit: 'tr909' };
  } else {
    const track = generate(genre, seed, dejavu);
    S = { genre, seed, dejavu, track, kit: track.kitName ?? 'tr909' };
  }
  made.unshift(S);
  if (made.length > 30) made.pop();
  selectSong(S, autoplay);
}

// ── Song ─────────────────────────────────────────────────────────
const trackMsg = (S) => (S.song ? { type: 'song', genre: S.genre, seed: S.seed } : { type: 'track', genre: S.genre, seed: S.seed, dejavu: S.dejavu, kit: S.kit });
function selectSong(S, autoplay = false) {
  cur = S;
  fillSongs();
  const T = S.track;
  $('songNote').textContent = `${GENRES[S.genre]}, seed ${S.seed}: ${T.bpm} bpm, ${T.sections.reduce((a, s) => a + s.bars, 0)} bars.`;
  $('kit').value = S.kit; $('kit2').value = S.kit;
  mute.clear(); solo = null; editing = null; $('partEdit').hidden = true;
  buildSections(); buildParts(); buildPads(); buildNotes(); drawLayers();
  if (lab) {
    post(trackMsg(S));
    post({ type: 'kit', name: S.kit, tweak: kitTweak });
    sendMutes();
  }
  if (playing || autoplay) startAt(0);
}

function sectionBar(i) { let b = 0; for (let k = 0; k < i; k++) b += cur.track.sections[k].bars; return b; }
function buildSections() {
  const el = $('sections');
  el.innerHTML = '';
  cur.track.sections.forEach((s, i) => {
    const b = document.createElement('button');
    const parts = Object.keys(s.p || {}).join(', ');
    b.textContent = `${i + 1}. ${s.drums ?? '–'} ${s.bars}`;
    b.title = `bar ${sectionBar(i) + 1}: ${parts || 'no parts'}${s.drop ? ', drop' : ''}${s.riser ? ', riser' : ''}`;
    b.onclick = () => startAt(sectionBar(i));
    el.append(b);
  });
}

async function startAt(bar) {
  try {
    await ensureAudio();
    await ctx.resume();
    post({ type: 'play', bar, delay: 0.03 });
    playing = true;
    showPlaying();
  } catch (e) { status('Audio failed: ' + e.message, true); console.error(e); }
}
function stopAll() {
  playing = false;
  post({ type: 'stop' });
  showPlaying();
}
async function togglePlay() {
  if (!playing) { status('Starting audio…'); await startAt(0); } else stopAll();
}
$('play').onclick = togglePlay;

function showPlaying() {
  $('play').textContent = playing ? '■ Stop' : '▶ Play';
  $('play').classList.toggle('on', playing);
  status(playing ? `Playing <b>${cur.track.title}</b> (${cur.track.style}). Space to stop.` : 'Stopped.');
}

// ── Parts ────────────────────────────────────────────────────────
let editing = null;
function sendMutes() {
  post({ type: 'mute', names: [...mute] });
  post({ type: 'solo', name: solo });
}
function buildParts() {
  const tb = $('parts');
  tb.innerHTML = '<tr><th>Part</th><th>Plays</th><th>Instrument</th><th></th></tr>';
  const rows = Object.entries(cur.track.parts).map(([name, p]) => [name, p.type, `${p.lab.kind} · ${p.lab.name ?? ''}`]);
  if (cur.song) for (const l of cur.song.layers) if (!l.lab) rows.push([l.name, l.role, l.sound + (l.gated ? ' · gated reverb' : '')]);
  rows.push(['drums', 'drums', cur.song ? 'every drum layer' : KIT_NAME[cur.kit] ?? cur.kit]);
  for (const [name, type, inst] of rows) {
    const tr = document.createElement('tr');
    tr.innerHTML = `<td>${name}</td><td>${type}</td><td>${inst}</td><td></td>`;
    const td = tr.lastChild;
    const mk = (label, on, fn) => { const x = document.createElement('button'); x.className = 'sm' + (on ? ' on' : ''); x.textContent = label; x.onclick = fn; td.append(x); };
    mk('Mute', mute.has(name), () => { mute.has(name) ? mute.delete(name) : mute.add(name); sendMutes(); buildParts(); });
    mk('Solo', solo === name, () => { solo = solo === name ? null : name; sendMutes(); buildParts(); });
    if (name !== 'drums') mk('Edit', editing === name, () => editPart(name));
    tb.append(tr);
  }
}

function knobPanel(el, labP, send) {
  el.innerHTML = '';
  const add = (label, get, set, min, max, step) => {
    const l = document.createElement('label'); l.className = 'sl';
    l.innerHTML = `${label} <input type="range" min="${min}" max="${max}" step="${step}"><output></output>`;
    const inp = l.querySelector('input'), out = l.querySelector('output');
    inp.value = get() ?? min;
    const show = () => { out.textContent = +Number(inp.value).toFixed(3); };
    show();
    inp.addEventListener('input', () => { set(Number(inp.value)); show(); send(); });
    el.append(l);
  };
  for (const [k, label, min, max, step] of KNOBS[labP.kind] ?? []) add(label, () => labP[k], (v) => { labP[k] = v; }, min, max, step);
  if (labP.kind === 'fm') {
    (labP.ops || []).forEach((op, i) => {
      if (!op) return;
      for (const [k, label, min, max, step] of FM_OP_KNOBS) add(`Op ${i + 1} ${label}`, () => op[k], (v) => { op[k] = v; }, min, max, step);
    });
  }
}
function editPart(name) {
  editing = editing === name ? null : name;
  buildParts();
  $('partEdit').hidden = !editing;
  if (!editing) return;
  const P = cur.track.parts[name];
  $('partEditTitle').textContent = `${name}: ${P.lab.kind} · ${P.lab.name ?? ''}`;
  knobPanel($('partKnobs'), P.lab, () => post({ type: 'patch', part: name, lab: structuredClone(P.lab) }));
}
$('partCopy').onclick = () => copyJson(editing && { [editing]: cur.track.parts[editing].lab }, 'partNote');
$('partReset').onclick = () => {
  if (!editing) return;
  // The patch as the grammar made it.
  const base = cur.song
    ? makeSong(cur.genre, cur.seed).layers.find((l) => l.name === editing).lab
    : generate(cur.genre, cur.seed, cur.dejavu).parts[editing].lab;
  cur.track.parts[editing].lab = base;
  post({ type: 'patch', part: editing, lab: structuredClone(base) });
  const n = editing; editing = null; editPart(n);
};

async function copyJson(obj, noteId) {
  if (!obj) return;
  const json = JSON.stringify(obj, null, 2);
  $('json').hidden = false; $('json').value = json;
  try { await navigator.clipboard.writeText(json); $(noteId).textContent = 'Copied.'; } catch { $(noteId).textContent = 'Select the text below and copy it.'; $('json').select(); }
}

// ── Instruments (keyboard) ───────────────────────────────────────
let inst = 'acid' in BPATCH ? 'acid' : Object.keys(BPATCH)[0], instP = structuredClone(BPATCH[inst]), octave = 3;
const byKind = {};
for (const [k, v] of Object.entries(BPATCH)) (byKind[v.kind] ||= []).push(k);
const KIND_NAME = { tb303: 'TB-303', juno: 'Juno-style poly', mono: 'Moog-style mono', fm: 'DX-style FM', string: 'Plucked string', bowed: 'Bowed string' };
for (const [kind, names] of Object.entries(byKind)) {
  const g = document.createElement('optgroup'); g.label = KIND_NAME[kind] ?? kind;
  for (const n of names) g.append(new Option(n, n));
  $('inst').append(g);
}
$('inst').value = inst;
function selectInst(k) {
  inst = k; instP = structuredClone(BPATCH[k]);
  post({ type: 'audition', lab: instP });
  knobPanel($('instKnobs'), instP, () => post({ type: 'audition', lab: structuredClone(instP) }));
}
$('inst').onchange = () => selectInst($('inst').value);
$('instCopy').onclick = () => copyJson({ [inst]: instP }, 'instNote');
$('instReset').onclick = () => selectInst(inst);
$('oct-').onclick = () => { octave = Math.max(0, octave - 1); buildKeys(); };
$('oct+').onclick = () => { octave = Math.min(6, octave + 1); buildKeys(); };

const held = new Map();
async function noteOn(midi, vel = 0.9, extra = {}) {
  await ensureAudio(); await ctx.resume();
  post({ type: 'on', midi, vel, ...extra });
  document.querySelector(`.keys [data-m="${midi}"]`)?.classList.add('down');
}
function noteOff(midi) {
  post({ type: 'off', midi });
  document.querySelector(`.keys [data-m="${midi}"]`)?.classList.remove('down');
}
function buildKeys() {
  const el = $('keys');
  el.innerHTML = '';
  const base = 12 * (octave + 1), W = 100 / 15;
  let wi = 0;
  for (let i = 0; i < 25; i++) {
    const m = base + i, pc = i % 12, black = [1, 3, 6, 8, 10].includes(pc);
    const d = document.createElement('div');
    d.dataset.m = m;
    d.className = black ? 'b' : 'w';
    if (black) { d.style.left = `calc(${wi * W}% - ${W * 0.3}%)`; d.style.width = W * 0.6 + '%'; } else { d.style.left = wi * W + '%'; d.style.width = W + '%'; wi++; }
    if (pc === 0) d.innerHTML = `<span>C${octave + 1 + Math.floor(i / 12) - 1}</span>`;
    d.onpointerdown = (e) => { d.setPointerCapture(e.pointerId); noteOn(m); };
    d.onpointerup = d.onpointercancel = () => noteOff(m);
    el.append(d);
  }
}
const KEYMAP = 'awsedftgyhujk';
document.addEventListener('keydown', (e) => {
  if (e.target.tagName === 'INPUT' && e.target.type === 'number') return;
  if (e.repeat || e.metaKey || e.ctrlKey) return;
  const k = e.key.toLowerCase();
  if (e.key === ' ') { e.preventDefault(); togglePlay(); return; }
  if ($('keysOn').checked) {
    const i = KEYMAP.indexOf(k);
    if (i >= 0) { const m = 12 * (octave + 1) + i; held.set(k, m); noteOn(m); return; }
    if (k === 'z') { $('oct-').click(); return; }
    if (k === 'x') { $('oct+').click(); return; }
  }
});
document.addEventListener('keyup', (e) => { const k = e.key.toLowerCase(); if (held.has(k)) { noteOff(held.get(k)); held.delete(k); } });

// Demo phrases: an acid line for the 303, chords for the polys, a riff for monos.
let demoTimer = null;
$('demo').onclick = async () => {
  clearInterval(demoTimer);
  await ensureAudio(); await ctx.resume();
  const kind = instP.kind, step = 60 / 124 / 4;
  let ev;
  if (kind === 'tb303') {
    const line = [[36, 1, 0], [36, 0, 0], [48, 0, 1], [46, 1, 0], [0], [36, 0, 0], [39, 1, 1], [41, 0, 0], [36, 0, 0], [0], [48, 1, 0], [36, 0, 0], [43, 0, 1], [46, 0, 0], [36, 1, 0], [34, 0, 0]];
    ev = line.map(([m, acc, sl], i) => m ? { t: i, midi: m + 12 * (octave - 2), dur: step * (sl ? 1.05 : 0.55), accent: !!acc, glideFrom: i && line[i - 1][2] ? line[i - 1][0] + 12 * (octave - 2) : undefined } : null).filter(Boolean);
  } else if (kind === 'mono') {
    const ns = [0, 0, 12, 0, 10, 0, 7, 3];
    ev = ns.map((n, i) => ({ t: i * 2, midi: 12 * (octave + 1) + n, dur: step * 1.6 }));
  } else {
    const chords = [[0, 3, 7, 10], [5, 8, 12, 15], [3, 7, 10, 14], [-2, 2, 5, 9]];
    ev = [];
    chords.forEach((c, i) => c.forEach((n) => ev.push({ t: i * 8, midi: 12 * (octave + 1) + n, dur: step * 7 })));
  }
  let i = 0;
  const t0 = performance.now();
  demoTimer = setInterval(() => {
    const now = (performance.now() - t0) / 1000 / step;
    while (i < ev.length && ev[i].t <= now) {
      const e = ev[i++];
      post({ type: 'on', midi: e.midi, vel: e.accent ? 1 : 0.8, dur: e.dur, accent: e.accent, glideFrom: e.glideFrom });
    }
    if (i >= ev.length) clearInterval(demoTimer);
  }, 5);
};

// ── Drum machine ─────────────────────────────────────────────────
let pad = 'kick';
function buildPads() {
  const el = $('pads');
  el.innerHTML = '';
  const lanes = KITS[cur.kit] ?? {};
  if (!(pad in lanes)) pad = Object.keys(lanes)[0];
  for (const lane of Object.keys(lanes)) {
    const b = document.createElement('button');
    b.textContent = lane;
    b.className = lane === pad ? 'sel' : '';
    b.onpointerdown = async () => {
      await ensureAudio(); await ctx.resume();
      post({ type: 'hit', lane, vel: 1 });
      if (pad !== lane) { pad = lane; buildPads(); }
    };
    el.append(b);
  }
  const k = { ...(lanes[pad] || {}), ...(kitTweak[pad] || {}) };
  const knobs = DRUM_KNOBS[k.t] ?? DRUM_KNOBS.default;
  const el2 = $('drumKnobs');
  el2.innerHTML = '';
  for (const [key, label, min, max, step] of knobs) {
    const l = document.createElement('label'); l.className = 'sl';
    l.innerHTML = `${pad} ${label.toLowerCase()} <input type="range" min="${min}" max="${max}" step="${step}"><output></output>`;
    const inp = l.querySelector('input'), out = l.querySelector('output');
    inp.value = k[key] ?? min;
    out.textContent = +Number(inp.value).toFixed(3);
    inp.addEventListener('input', () => {
      (kitTweak[pad] ||= {})[key] = Number(inp.value);
      out.textContent = +Number(inp.value).toFixed(3);
      post({ type: 'kit', name: cur.kit, tweak: kitTweak });
    });
    inp.addEventListener('change', () => post({ type: 'hit', lane: pad, vel: 1 }));
    el2.append(l);
  }
}
$('drumCopy').onclick = () => copyJson({ [cur.kit]: Object.fromEntries(Object.entries(kitTweak).map(([l, t]) => [l, { ...KITS[cur.kit][l], ...t }])) }, 'drumNote');
$('drumReset').onclick = () => { kitTweak = {}; post({ type: 'kit', name: cur.kit, tweak: kitTweak }); buildPads(); };

// ── Audio ────────────────────────────────────────────────────────
let starting = null;
function ensureAudio() { return (starting ||= start()); }
async function start() {
  const AC = window.AudioContext || window.webkitAudioContext;
  if (!AC) throw new Error('this browser has no Web Audio');
  if (!window.isSecureContext || !window.AudioWorkletNode) throw new Error('AudioWorklet needs a secure page (https or localhost)');
  // iOS: a playback session (music), not ambient sound, before the context.
  try { if (navigator.audioSession) navigator.audioSession.type = 'playback'; } catch { /* older Safari */ }
  const ac = new AC({ latencyHint: 'playback' });
  await ac.audioWorklet.addModule(WORKLET_URL);
  lab = new AudioWorkletNode(ac, 'music-lab', {
    numberOfInputs: 0, outputChannelCount: [2],
    processorOptions: { module, seed: 5, messages: [trackMsg(cur), { type: 'kit', name: cur.kit, tweak: kitTweak }, { type: 'mix', ...mixNow() }] },
  });
  lab.port.onmessage = (m) => onLab(m.data);
  lab.onprocessorerror = () => status('The audio thread stopped with an error (see the console). Reload to try again.', true);
  hp = ac.createBiquadFilter(); hp.type = 'highpass'; hp.Q.value = 0.707;
  mono = ac.createGain(); mono.channelCountMode = 'explicit'; mono.channelInterpretation = 'speakers';
  vol = ac.createGain(); vol.gain.value = Number($('vol').value);
  analyser = ac.createAnalyser(); analyser.fftSize = 8192; analyser.smoothingTimeConstant = 0.75;
  lab.connect(hp); hp.connect(mono); mono.connect(vol); vol.connect(analyser); analyser.connect(ac.destination);
  ctx = ac;
  applySpeaker();
  post({ type: 'audition', lab: instP });
  post({ type: 'energy', value: Number($('energy').value) });
  sendMutes();
  window.musicLab.ctx = ctx; window.musicLab.lab = lab;
}
document.addEventListener('click', () => { if (ctx && ctx.state !== 'running') ctx.resume(); }, { capture: true });

function onLab(d) {
  if (d.type === 'pos' && d.pos) {
    const p = d.pos;
    $('rSec').textContent = `${p.sec + 1} / ${cur.track.sections.length}`;
    $('rBar').textContent = `${p.barIndex + 1} / ${p.bars}`;
    $('rGr').textContent = d.gr < 0.999 ? (20 * Math.log10(d.gr)).toFixed(1) + ' dB' : '0 dB';
    if (d.load != null) {
      $('rLoad').textContent = Math.round(d.load * 100) + '%';
      $('loadM').firstChild.style.width = Math.min(100, d.load * 100) + '%';
      $('loadM').classList.toggle('warn', d.load > 0.6);
    }
    [...$('sections').children].forEach((b, i) => b.classList.toggle('cur', i === p.sec && p.playing));
    window.musicLab.pos = p;
    posAt = { bar: p.barIndex + p.step / 16, t: performance.now(), playing: p.playing };
  } else if (d.type === 'error') {
    status(`The engine refused a message: ${d.message}`, true);
    console.error(d);
  }
}

// ── Layers over time (a new generator's song) ────────────────────
// One row per layer, its clips drawn where they play (filled: the core;
// outlined: a variation; faint: filtered or sparse; a tick: a hit), the
// sections above, a playhead. Click to play from that bar.
let posAt = null;
const MARK = { x: 1, v: 0.65, f: 0.3, s: 0.4, r: 0.85, 1: 1 };
const ROLE_COLOR = { K: '#ff3d7f', C: '#ff7a59', H: '#ffb84d', P: '#d6c94d', B: '#4dff8a', Sub: '#2fd16a', Pd: '#6fb7ff', A: '#9f8cff', L: '#e07cff', Atm: '#5ad6d6', X: '#cfd6e2', V: '#ffd0e0' };
function drawLayers() {
  const card = $('layersCard');
  card.hidden = !cur?.song;
  if (!cur?.song) return;
  const c = $('layers'), g = c.getContext('2d');
  const song = cur.song, rows = song.layers.length;
  const W = c.clientWidth * devicePixelRatio, rowH = 22 * devicePixelRatio, top = 26 * devicePixelRatio, left = 70 * devicePixelRatio;
  c.width = W; c.height = top + rows * rowH + 4;
  c.style.height = c.height / devicePixelRatio + 'px';
  const bx = (bar) => left + (bar / song.bars) * (W - left - 4);
  g.fillStyle = '#0a0c10'; g.fillRect(0, 0, c.width, c.height);
  g.font = `${11 * devicePixelRatio}px system-ui`; g.textBaseline = 'middle';
  for (const s of song.sections) {
    const x0 = bx(s.bar), x1 = bx(s.bar + s.bars);
    g.fillStyle = `rgba(111,183,255,${0.06 + 0.18 * s.energy})`; g.fillRect(x0, 0, x1 - x0, top - 4);
    g.strokeStyle = '#262c36'; g.beginPath(); g.moveTo(x0, 0); g.lineTo(x0, c.height); g.stroke();
    g.fillStyle = '#8b94a3'; g.fillText(s.name, x0 + 3, (top - 4) / 2, Math.max(0, x1 - x0 - 6));
  }
  song.layers.forEach((l, i) => {
    const y = top + i * rowH;
    g.fillStyle = '#8b94a3'; g.fillText(l.name, 4, y + rowH / 2, left - 8);
    const col = ROLE_COLOR[l.role] ?? '#cfd6e2';
    for (const k of l.clips) {
      const x0 = bx(k.bar), x1 = Math.max(bx(k.bar + k.bars), x0 + 2);
      g.globalAlpha = MARK[k.mark] ?? 1;
      g.fillStyle = col;
      if (k.mark === 'v') { g.strokeStyle = col; g.lineWidth = 2; g.strokeRect(x0 + 1, y + 3, x1 - x0 - 2, rowH - 6); }
      else g.fillRect(x0 + 1, y + 3, x1 - x0 - 2, rowH - 6);
      g.globalAlpha = 1;
    }
  });
  for (const f of song.fx) {
    g.fillStyle = 'rgba(255,255,255,0.12)';
    g.fillRect(bx(f.bar), top - 4, bx(f.bar + f.bars) - bx(f.bar), 3);
  }
  if (posAt) {
    const bar = posAt.playing ? posAt.bar + ((performance.now() - posAt.t) / 1000) * (cur.song.bpm / 240) : posAt.bar;
    g.fillStyle = '#4dff8a'; g.fillRect(bx(Math.min(bar, song.bars)), 0, 2 * devicePixelRatio, c.height);
  }
  c.onclick = (e) => {
    const x = (e.offsetX * devicePixelRatio - left) / (W - left - 4);
    if (x >= 0) startAt(Math.floor(x * song.bars / 4) * 4);
  };
}
function buildNotes() {
  const el = $('notes');
  $('notesCard').hidden = !cur?.song;
  if (!cur?.song) return;
  el.innerHTML = '';
  for (const n of cur.song.notes) {
    const li = document.createElement('li');
    li.textContent = n;
    el.append(li);
  }
}
addEventListener('resize', () => drawLayers());

// ── Spectrum ─────────────────────────────────────────────────────
const freq = new Float32Array(4096);
function frame() {
  requestAnimationFrame(frame);
  if (cur?.song && posAt?.playing) drawLayers();
  if (!ctx) return;
  const c = $('spec'), g = c.getContext('2d');
  const W = c.width, H = c.height, sr = ctx.sampleRate;
  analyser.getFloatFrequencyData(freq);
  g.fillStyle = '#0a0c10'; g.fillRect(0, 0, W, H);
  const fx = (f) => (Math.log(f / 20) / Math.log(20000 / 20)) * W;
  const dbY = (d) => H - ((d + 110) / 100) * H;
  g.strokeStyle = '#262c36'; g.fillStyle = '#8b94a3'; g.font = '20px system-ui';
  for (const f of [50, 100, 200, 500, 1000, 2000, 5000, 10000]) {
    const x = fx(f); g.beginPath(); g.moveTo(x, 0); g.lineTo(x, H); g.stroke();
    g.fillText(f >= 1000 ? f / 1000 + 'k' : f, x + 3, H - 6);
  }
  g.strokeStyle = '#ff3d7f'; g.lineWidth = 2; g.beginPath();
  const bin = sr / analyser.fftSize;
  let first = true;
  for (let k = 1; k < freq.length; k++) {
    const f = k * bin; if (f < 20) continue;
    const x = fx(f), y = dbY(freq[k]);
    if (first) { g.moveTo(x, y); first = false; } else g.lineTo(x, y);
  }
  g.stroke();
}
requestAnimationFrame(frame);

// ── Measure ──────────────────────────────────────────────────────
$('measure').onclick = async () => {
  const tb = $('mTable');
  $('measure').disabled = true;
  tb.innerHTML = '<tr><th>Song</th><th>Genre</th><th>RMS dB</th><th>Peak</th><th>Crest dB</th><th>Width</th><th>sub</th><th>low</th><th>low-mid</th><th>mid</th><th>high</th><th>air</th></tr>';
  let n = 0;
  try {
    for (const S of made) {
      const r = analyse(await render(module, trackMsg(S), { bar: fullBar(S.track), mix: mixNow() }));
      n++; $('mNote').textContent = `${n} / ${made.length}…`;
      tb.insertAdjacentHTML('beforeend', `<tr><td>${S.track.title}</td><td>${GENRES[S.genre]}</td><td>${r.rmsDb}</td><td>${r.peak}</td><td>${r.crestDb}</td><td>${r.width}</td>${['sub', 'low', 'lowmid', 'mid', 'high', 'air'].map((b) => `<td>${r.bandsDb[b]}</td>`).join('')}</tr>`);
    }
    $('mNote').textContent = 'Done. Bands are dB against the low band (40–150 Hz).';
  } catch (e) { $('mNote').textContent = 'Failed: ' + e.message; console.error(e); }
  $('measure').disabled = false;
};

window.musicLab = { GENRES, BPATCH, KITS, made, generate, makeSong, selectSong, togglePlay, startAt, get cur() { return cur; }, get playing() { return playing; } };
const q = new URLSearchParams(location.search);
if (q.get('genre') in GENRES) $('genre').value = q.get('genre');
if (q.get('seed')) $('seed').value = q.get('seed');
makeTrack(false);
selectInst(inst);
buildKeys();
status('Tap <b>▶ Play</b> to start.');
