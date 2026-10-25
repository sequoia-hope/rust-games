// Music Lab page: make a track from a genre and a seed, play it, and take
// it apart: its sections, its parts (mute, solo, live patch edits), every
// instrument on a keyboard, the drum machine on pads, the mix, a spectrum
// and offline measurements — plus the full-screen RP2350 Hardware Groovebox
// controller sharing the exact same AudioContext, AudioWorkletNode, and live
// playback state so switching between Player and Groovebox is instantaneous
// with zero hiccup in audio.
//
//   music-lab worklet ─ speaker (high-pass, mono) ─ volume ─ analyser ─ out

import { KNOBS, FM_OP_KNOBS, DRUM_KNOBS } from './knobs.js';
import { WORKLET_URL, fullBar, render, analyse } from './measure.js';
import { mountGroovebox } from './hardware.js';

const $ = (id) => document.getElementById(id);
const status = (html, err = false) => { $('status').innerHTML = html; $('status').classList.toggle('err', err); };

// ── The engine on the main thread: tables and tracks ─────────────
const WASM_URL = new URL('music.wasm?v=20261024b', import.meta.url);
const module = await WebAssembly.compileStreaming(fetch(WASM_URL, { cache: 'no-store' }));
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
const PROFILES = Object.fromEntries(tables.profiles || []);
const BPATCH = Object.fromEntries(tables.patches);
const KITS = Object.fromEntries(tables.kits.map(([k, lanes]) => [k, Object.fromEntries(lanes)]));
const KIT_NAME = { tr808: 'TR-808', tr909: 'TR-909', latin: 'Latin', country: 'Country' };
const CLASSIC_TO_PROFILE = {
  house: 'chicago', techno: 'detroit', trance: 'uplifting', eurobeat: 'outrun',
  psytrance: 'acid', dnb: 'liquid', garage: 'deep', chicha: 'synthwave', country: 'orchestral',
};

// A genre's track, as the worklet will make it.
function generate(genre, seed, dejavu) {
  const [ptr, len] = passText(genre);
  return JSON.parse(text(wasm.music_generate(ptr, len, seed, dejavu ?? -1)));
}
// A new generator's song (its summary: sections, layers, clips, patterns, notes).
function makeSong(genre, seed, bpm = -1, key = -1) {
  const [ptr, len] = passText(genre);
  return JSON.parse(text(wasm.music_song(ptr, len, seed, bpm, key)));
}
function makeSongWithSettings(payload) {
  const [ptr, len] = passText(JSON.stringify(payload));
  return JSON.parse(text(wasm.music_song(ptr, len, payload.seed ?? 1, payload.bpm ?? -1, payload.key ?? -1)));
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

// A song is what makes it: (genre, seed, déjà vu / DNA overrides), and the kit it plays on.
const made = [];
let cur = null, playing = false, autoplaying = false, view = 'lab';
const autoGenres = new Set(Object.keys(NEW));
let ctx = null, lab = null, hp = null, mono = null, vol = null, analyser = null;
const mute = new Set();
let solo = null;
let kitTweak = {};
let posAt = { sec: 0, bar: 0, barIndex: 0, step: 0, bars: 128, t: performance.now(), playing: false };
let lastGr = 1.0, lastLoad = 0.0;
let groovebox = null;

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
bindRange($('bpm'), (v) => (v < 60 ? 'auto' : `${v} bpm`));
bindRange($('dejavu'), pct);
bindRange($('energy'), pct, (v) => { $('energyAuto').checked = false; post({ type: 'energy', value: v }); });
for (const k of ['tape', 'glue', 'reverb', 'revTone']) {
  bindRange($(k), pct, (v) => {
    post({ type: 'mix', [k]: v });
    groovebox?.syncAllKnobsUI();
  });
}
bindRange($('revSize'), (v) => v.toFixed(1), (v) => post({ type: 'mix', revSize: v }));
bindRange($('vol'), pct, (v) => vol?.gain.setTargetAtTime(v, ctx.currentTime, 0.03));
const mixNow = () => ({ tape: +$('tape').value, glue: +$('glue').value, reverb: +$('reverb').value, revSize: +$('revSize').value, revTone: +$('revTone').value });
function setMixParam(k, v) {
  const el = $(k);
  if (el) {
    el.value = v;
    const out = el.parentElement?.querySelector('output');
    if (out) out.textContent = k === 'revSize' ? Number(v).toFixed(1) : pct(Number(v));
  }
  post({ type: 'mix', [k]: v });
}

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

const ALL_KEYS = Object.keys(GENRES);
const EDM_KEYS = ALL_KEYS.filter((k) => !['orchestral', 'chicha', 'country'].includes(k));
function buildAutoGenres() {
  const el = $('autoGenres');
  el.innerHTML = '';
  for (const [k, name] of Object.entries(GENRES)) {
    const b = document.createElement('button');
    b.className = 'sm' + (autoGenres.has(k) ? ' on' : '');
    b.textContent = name;
    b.dataset.g = k;
    b.title = k in NEW ? `${name} (New generator)` : `${name} (Classic)`;
    b.onclick = () => {
      if (autoGenres.has(k)) autoGenres.delete(k); else autoGenres.add(k);
      buildAutoGenres();
    };
    el.append(b);
  }
  $('autoCount').textContent = autoGenres.size ? `(${autoGenres.size} of ${ALL_KEYS.length})` : `(using ${GENRES[$('genre').value]})`;
}
const setAutoPool = (keys) => { autoGenres.clear(); for (const k of keys) autoGenres.add(k); buildAutoGenres(); };
$('selAll').onclick = () => setAutoPool(ALL_KEYS);
$('selNew').onclick = () => setAutoPool(Object.keys(NEW));
$('selEdm').onclick = () => setAutoPool(EDM_KEYS);
$('selClassic').onclick = () => setAutoPool(Object.keys(CLASSIC));
$('selNone').onclick = () => setAutoPool([]);
$('genre').onchange = () => { if (!autoGenres.size) buildAutoGenres(); };
buildAutoGenres();

function createSongEntry(genre, seed, { bpm = -1, key = -1, dejavu = Number($('dejavu').value), dnaOverrides = null } = {}) {
  const ov = dnaOverrides ? { ...dnaOverrides } : {};
  if (bpm >= 60) ov.bpm = bpm;
  if (key >= 0) ov.key = key;
  if (genre in NEW) {
    const payload = { type: 'song', genre, seed, ...ov };
    const song = makeSongWithSettings(payload);
    const kit = song.layers.find((l) => !l.lab)?.sound ?? 'tr909';
    return { genre, seed, bpm: ov.bpm ?? -1, key: ov.key ?? -1, dnaOverrides: ov, patchOverrides: {}, song, track: asTrack(song), kit };
  }
  const track = generate(genre, seed, dejavu);
  return { genre, seed, dejavu, dnaOverrides: {}, patchOverrides: {}, song: null, track, kit: track.kitName ?? 'tr909' };
}

$('dice').onclick = () => { $('seed').value = 1 + Math.floor(Math.random() * 99999); makeTrack(true); };
$('make').onclick = () => makeTrack(true);
function makeTrack(autoplay) {
  const genre = $('genre').value, seed = Math.max(1, Number($('seed').value) | 0), dejavu = Number($('dejavu').value);
  const bpmVal = Number($('bpm').value), bpm = bpmVal >= 60 ? bpmVal : -1;
  const key = Number($('key').value);
  const S = createSongEntry(genre, seed, { bpm, key, dejavu });
  made.unshift(S);
  if (made.length > 30) made.pop();
  selectSong(S, autoplay);
}

function nextAutoTrack(playNow = true) {
  const pool = autoGenres.size ? [...autoGenres] : [$('genre').value];
  const choices = pool.length > 1 && cur ? pool.filter((g) => g !== cur.genre) : pool;
  $('genre').value = choices[Math.floor(Math.random() * choices.length)];
  $('seed').value = 1 + Math.floor(Math.random() * 99999);
  makeTrack(playNow);
}

// ── Song ─────────────────────────────────────────────────────────
const trackMsg = (S, morph = false) => (S.song
  ? { type: 'song', genre: S.genre, seed: S.seed, ...(S.dnaOverrides || {}), ...(morph ? { morph: true } : {}) }
  : { type: 'track', genre: S.genre, seed: S.seed, dejavu: S.dejavu, kit: S.kit });

function syncLabUIFromCur() {
  if (!cur) return;
  fillSongs();
  $('genre').value = cur.genre;
  $('seed').value = String(cur.seed);
  const bpmVal = cur.dnaOverrides?.bpm ?? 59;
  $('bpm').value = bpmVal >= 60 ? bpmVal : 59;
  $('bpm').parentElement.querySelector('output').textContent = bpmVal < 60 ? 'auto' : `${bpmVal} bpm`;
  $('key').value = String(cur.dnaOverrides?.key ?? -1);
  const T = cur.track;
  $('songNote').textContent = `${GENRES[cur.genre]}, seed ${cur.seed}: ${T.bpm} bpm, ${T.sections.reduce((a, s) => a + s.bars, 0)} bars.`;
  $('kit').value = cur.kit; $('kit2').value = cur.kit;
  buildSections(); buildParts(); buildPads(); buildNotes(); drawLayers();
}

function selectSong(S, autoplay = false, { morphInWorklet = false } = {}) {
  const prev = cur;
  cur = S;
  if (S !== prev) {
    mute.clear(); solo = null; editing = null; $('partEdit').hidden = true;
    const totalBars = S.song?.bars ?? S.track.sections.reduce((a, s) => a + s.bars, 0);
    if (!morphInWorklet) {
      posAt = { sec: 0, bar: 0, barIndex: 0, step: 0, bars: totalBars, t: performance.now(), playing };
    }
  }
  syncLabUIFromCur();
  groovebox?.syncFromCur();
  if (lab) {
    const canMorph = morphInWorklet && prev?.song && S.song;
    post(trackMsg(S, canMorph));
    post({ type: 'kit', name: S.kit, tweak: kitTweak });
    post({ type: 'mix', ...mixNow() });
    post({ type: 'energy', value: Number($('energy').value) });
    for (const [part, labP] of Object.entries(S.patchOverrides || {})) {
      post({ type: 'patch', part, lab: labP });
    }
    sendMutes();
    if ((playing || autoplay) && !canMorph) startAt(0);
  } else if (playing || autoplay) {
    startAt(0);
  }
}

// Rebuilds the current song in-place when Genre DNA is tweaked in the Groovebox,
// morphing the worklet's SongPlayer without losing its bar/step, voices, or tails.
function rebuildCurrentSong(morph = true) {
  if (!cur) return;
  const wasSong = !!cur.song;
  if (!(cur.genre in NEW)) {
    cur.genre = CLASSIC_TO_PROFILE[cur.genre] || 'synthwave';
  }
  const payload = { type: 'song', genre: cur.genre, seed: cur.seed, ...(cur.dnaOverrides || {}), ...(morph && wasSong ? { morph: true } : {}) };
  const song = makeSongWithSettings(payload);
  for (const [layerName, labP] of Object.entries(cur.patchOverrides || {})) {
    const l = song?.layers?.find((x) => x.name === layerName);
    if (l && l.lab) Object.assign(l.lab, labP);
  }
  cur.song = song;
  cur.track = asTrack(song);
  cur.kit = song.layers.find((l) => !l.lab)?.sound ?? cur.kit;
  syncLabUIFromCur();
  groovebox?.syncFromCur();
  if (lab) {
    post(payload);
    post({ type: 'mix', ...mixNow() });
    for (const [part, labP] of Object.entries(cur.patchOverrides || {})) {
      post({ type: 'patch', part, lab: labP });
    }
    sendMutes();
    if (!wasSong && playing) {
      post({ type: 'play', bar: posAt?.barIndex || 0, delay: 0.02 });
    }
  }
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
    post({ type: 'play', bar, delay: 0.02 });
    playing = true;
    if (posAt) {
      posAt.bar = bar;
      posAt.barIndex = bar;
      posAt.step = 0;
      posAt.t = performance.now();
      posAt.playing = true;
    }
    showPlaying();
  } catch (e) { status('Audio failed: ' + e.message, true); console.error(e); }
}
function stopAll() {
  playing = false;
  if (posAt) posAt.playing = false;
  post({ type: 'stop' });
  showPlaying();
}
async function togglePlay() {
  if (!playing) {
    status('Starting audio…');
    const resumeBar = posAt?.barIndex > 0 ? posAt.barIndex : 0;
    await startAt(resumeBar);
  } else {
    stopAll();
  }
}
async function toggleAutoplay() {
  autoplaying = !autoplaying;
  $('autoplay').classList.toggle('on', autoplaying);
  if (autoplaying && !playing) {
    status('Starting autoplay…');
    if (autoGenres.size > 0 && !autoGenres.has(cur.genre)) nextAutoTrack(true);
    else await startAt(posAt?.barIndex > 0 ? posAt.barIndex : 0);
  } else {
    showPlaying();
  }
}
$('play').onclick = togglePlay;
$('autoplay').onclick = toggleAutoplay;
$('nextSong').onclick = () => nextAutoTrack(true);

// ── Seamless Player ↔ Groovebox View Switching (Zero Audio Hiccup) ──
async function openGroovebox({ fromPopState = false, autoStart = true, replaceUrl = false } = {}) {
  if (!playing && cur && ($('genre').value !== cur.genre || (Number($('seed').value) | 0) !== cur.seed)) {
    makeTrack(false);
  }
  view = 'groovebox';
  document.body.classList.add('groovebox-open');
  $('labView').hidden = true;
  $('hw').hidden = false;
  groovebox?.syncFromCur();
  if (!fromPopState && cur) {
    const u = new URL(location.href);
    if (!u.pathname.endsWith('hardware.html')) u.searchParams.set('view', 'groovebox');
    u.searchParams.set('genre', cur.genre);
    u.searchParams.set('seed', String(cur.seed));
    if (replaceUrl) history.replaceState({ view: 'groovebox' }, '', u);
    else history.pushState({ view: 'groovebox' }, '', u);
  }
  if (!playing && autoStart) {
    const resumeBar = posAt?.barIndex > 0 ? posAt.barIndex : 0;
    await startAt(resumeBar);
  } else {
    showPlaying();
  }
}

function closeGroovebox({ fromPopState = false, replaceUrl = false } = {}) {
  view = 'lab';
  document.body.classList.remove('groovebox-open');
  $('hw').hidden = true;
  $('labView').hidden = false;
  syncLabUIFromCur();
  if (!fromPopState && cur) {
    const u = new URL(location.href);
    if (u.pathname.endsWith('hardware.html')) {
      u.pathname = u.pathname.replace(/hardware\.html$/, 'index.html');
    }
    u.searchParams.delete('view');
    u.searchParams.set('genre', cur.genre);
    u.searchParams.set('seed', String(cur.seed));
    if (replaceUrl) history.replaceState({ view: 'lab' }, '', u);
    else history.pushState({ view: 'lab' }, '', u);
  }
  showPlaying();
}

$('openHardware').onclick = () => openGroovebox({ autoStart: true });
window.addEventListener('popstate', () => {
  const pq = new URLSearchParams(location.search);
  const wantHw = location.pathname.endsWith('hardware.html') || pq.get('view') === 'groovebox';
  if (wantHw && view !== 'groovebox') openGroovebox({ fromPopState: true, autoStart: false });
  else if (!wantHw && view !== 'lab') closeGroovebox({ fromPopState: true });
});

function showPlaying() {
  $('play').textContent = playing ? '■ Stop' : '▶ Play';
  $('play').classList.toggle('on', playing);
  $('autoplay').classList.toggle('on', autoplaying);
  groovebox?.syncTransportUI();
  const mode = autoplaying ? ` · Autoplay (${autoGenres.size || 1} genres)` : '';
  status(playing ? `Playing <b>${cur.track.title}</b> (${cur.track.style})${mode}. Space to stop.` : `Stopped${mode}.`);
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
    mk('Mute', mute.has(name), () => { mute.has(name) ? mute.delete(name) : mute.add(name); sendMutes(); buildParts(); groovebox?.buildRolePads(); });
    mk('Solo', solo === name, () => { solo = solo === name ? null : name; sendMutes(); buildParts(); groovebox?.buildRolePads(); });
    if (name !== 'drums' && cur.track.parts[name]) mk('Edit', editing === name, () => editPart(name));
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
  knobPanel($('partKnobs'), P.lab, () => {
    const cloned = structuredClone(P.lab);
    (cur.patchOverrides ||= {})[name] = cloned;
    const l = cur.song?.layers?.find((x) => x.name === name);
    if (l && l.lab) Object.assign(l.lab, cloned);
    post({ type: 'patch', part: name, lab: cloned });
    groovebox?.syncSynthKnobsToFocused(false);
  });
}
$('partCopy').onclick = () => copyJson(editing && { [editing]: cur.track.parts[editing].lab }, 'partNote');
$('partReset').onclick = () => {
  if (!editing) return;
  // The patch as the grammar made it.
  const base = cur.song
    ? makeSongWithSettings(trackMsg(cur, false)).layers.find((l) => l.name === editing).lab
    : generate(cur.genre, cur.seed, cur.dejavu).parts[editing].lab;
  delete cur.patchOverrides?.[editing];
  cur.track.parts[editing].lab = base;
  const l = cur.song?.layers?.find((x) => x.name === editing);
  if (l) l.lab = structuredClone(base);
  post({ type: 'patch', part: editing, lab: structuredClone(base) });
  groovebox?.syncSynthKnobsToFocused(false);
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
  if (e.key === 'Escape' && view === 'groovebox') { e.preventDefault(); closeGroovebox(); return; }
  if (view === 'lab' && $('keysOn').checked) {
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
  for (const [part, labP] of Object.entries(cur?.patchOverrides || {})) {
    post({ type: 'patch', part, lab: labP });
  }
  sendMutes();
  window.musicLab.ctx = ctx; window.musicLab.lab = lab;
}
document.addEventListener('click', () => { if (ctx && ctx.state !== 'running') ctx.resume(); }, { capture: true });

function onLab(d) {
  if (d.type === 'pos' && d.pos) {
    const p = d.pos;
    lastGr = d.gr ?? 1.0;
    lastLoad = d.load ?? lastLoad;
    $('rSec').textContent = `${p.sec + 1} / ${cur.track.sections.length}`;
    $('rBar').textContent = `${p.barIndex + 1} / ${p.bars}`;
    $('rGr').textContent = lastGr < 0.999 ? (20 * Math.log10(lastGr)).toFixed(1) + ' dB' : '0 dB';
    if (d.load != null) {
      $('rLoad').textContent = Math.round(d.load * 100) + '%';
      $('loadM').firstChild.style.width = Math.min(100, d.load * 100) + '%';
      $('loadM').classList.toggle('warn', d.load > 0.6);
    }
    if ($('energyAuto').checked && p.playing && p.bars > 0) {
      const frac = (p.barIndex + p.step / 16) / p.bars;
      const e = +(0.5 + 0.5 * Math.min(1, frac / 0.67)).toFixed(2);
      if (Math.abs(Number($('energy').value) - e) >= 0.01) {
        $('energy').value = e;
        $('energy').parentElement.querySelector('output').textContent = pct(e);
        post({ type: 'energy', value: e });
      }
    }
    [...$('sections').children].forEach((b, i) => b.classList.toggle('cur', i === p.sec && p.playing));
    window.musicLab.pos = p;
    posAt = { sec: p.sec, bar: p.barIndex + p.step / 16, barIndex: p.barIndex, step: p.step, bars: p.bars, t: performance.now(), playing: p.playing };
    groovebox?.onPos(p, lastGr, d.load);
  } else if (d.type === 'end' && autoplaying && playing) {
    nextAutoTrack(true);
  } else if (d.type === 'error') {
    status(`The engine refused a message: ${d.message}`, true);
    console.error(d);
  }
}

// ── Layers over time (a new generator's song) ────────────────────
// One row per layer, its clips drawn where they play (filled: the core;
// outlined: a variation; faint: filtered or sparse; a tick: a hit), the
// sections above, a playhead. Click to play from that bar.
const MARK = { x: 1, v: 0.65, f: 0.3, s: 0.4, r: 0.85, 1: 1 };
const ROLE_COLOR = { K: '#ff3d7f', C: '#ff7a59', H: '#ffb84d', P: '#d6c94d', B: '#4dff8a', Sub: '#2fd16a', Pd: '#6fb7ff', A: '#9f8cff', L: '#e07cff', Atm: '#5ad6d6', X: '#cfd6e2', V: '#ffd0e0' };
function drawLayers() {
  const card = $('layersCard');
  card.hidden = !cur?.song;
  if (!cur?.song) return;
  const c = $('layers'), g = c.getContext('2d');
  const W = c.clientWidth * devicePixelRatio;
  if (W <= 0) return;
  const song = cur.song, rows = song.layers.length;
  const rowH = 22 * devicePixelRatio, top = 26 * devicePixelRatio, left = 70 * devicePixelRatio;
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
    const bar = (playing && posAt.playing) ? posAt.bar + ((performance.now() - posAt.t) / 1000) * (cur.song.bpm / 240) : posAt.bar;
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
addEventListener('resize', () => { if (view === 'lab') drawLayers(); });

// ── Spectrum ─────────────────────────────────────────────────────
const freq = new Float32Array(4096);
function frame() {
  requestAnimationFrame(frame);
  if (view !== 'lab') return;
  if (cur?.song && playing && posAt?.playing) drawLayers();
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

// ── Mount the RP2350 Hardware Groovebox on the Shared Engine ─────
groovebox = mountGroovebox({
  PROFILES,
  NEW,
  CLASSIC,
  GENRES,
  BPATCH,
  mute,
  getCur: () => cur,
  getPlaying: () => playing,
  getCtx: () => ctx,
  getLab: () => lab,
  getAnalyser: () => analyser,
  getPosAt: () => posAt,
  getGrLoad: () => ({ gr: lastGr, load: lastLoad }),
  getSolo: () => solo,
  setSolo: (s) => { solo = s; },
  ensureAudio,
  post,
  startAt,
  stopAll,
  sendMutes,
  getMix: mixNow,
  setMixParam,
  rebuildCurrentSong,
  setDnaOverride: (field, value) => {
    if (!cur) return;
    (cur.dnaOverrides ||= {})[field] = value;
  },
  onPresetChange: (newGenre, newSeed) => {
    const S = createSongEntry(newGenre, newSeed);
    made.unshift(S);
    if (made.length > 30) made.pop();
    selectSong(S, playing, { morphInWorklet: playing });
  },
  onResetProfile: () => {
    if (!cur) return;
    cur.dnaOverrides = {};
    cur.patchOverrides = {};
    rebuildCurrentSong(playing);
  },
  onPatchEdit: (partName, labPatch) => {
    if (!cur) return;
    (cur.patchOverrides ||= {})[partName] = labPatch;
    if (cur.track.parts[partName]) cur.track.parts[partName].lab = structuredClone(labPatch);
    const l = cur.song?.layers?.find((x) => x.name === partName);
    if (l && l.lab) Object.assign(l.lab, labPatch);
    post({ type: 'patch', part: partName, lab: labPatch });
    post({ type: 'audition', lab: labPatch });
    if (editing === partName) {
      editing = null;
      editPart(partName);
    }
  },
  onMutesChanged: () => {
    sendMutes();
    buildParts();
  },
  closeGroovebox: () => closeGroovebox(),
});

window.musicLab = {
  GENRES, BPATCH, KITS, made, autoGenres, generate, makeSong, selectSong, togglePlay, toggleAutoplay, nextAutoTrack, startAt,
  openGroovebox, closeGroovebox,
  get cur() { return cur; }, get playing() { return playing; }, get autoplaying() { return autoplaying; }, get view() { return view; },
};

const q = new URLSearchParams(location.search);
const startInGroovebox = location.pathname.endsWith('hardware.html') || q.get('view') === 'groovebox';
if (q.get('seed')) $('seed').value = q.get('seed');
else if (startInGroovebox) $('seed').value = '7';
if (q.get('genre') in GENRES) {
  $('genre').value = q.get('genre');
  makeTrack(false);
} else {
  for (const g of Object.keys(NEW).reverse()) {
    $('genre').value = g;
    makeTrack(false);
  }
}
selectInst(inst);
buildKeys();
if (startInGroovebox) {
  openGroovebox({ autoStart: false, replaceUrl: true });
} else {
  status('Tap <b>▶ Play</b> to start.');
}
