// RP2350 Hardware Groovebox UI: full-screen landscape controller for the
// rust-games music engine (crates/music, music.wasm). Exposes the research-backed
// Genre DNA parameters (BassStyle, ChordStyle, Swing, Harmonic Extensions,
// Scale Mode, Perc Density, Lead Hook Density, 80s Tom Fills, Gated Reverb,
// Sidechain Pump, Tape Saturation) alongside a 16×8 Discrete RGB LED Pixel
// Matrix, live Synth Sculptor knobs, Role Mute/Focus/Solo keys, and Scale-Locked
// Jam Pads.

import { WORKLET_URL } from './measure.js';

const $ = (id) => document.getElementById(id);

// ── Wasm Engine on Main Thread (Tables + Song Summary & Patterns) ──
const WASM_URL = new URL('music.wasm?v=20261024a', import.meta.url);
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
const PROFILES = Object.fromEntries(tables.profiles || []);
const BPATCH = Object.fromEntries(tables.patches || []);

function makeSongWithSettings(payload) {
  const [ptr, len] = passText(JSON.stringify(payload));
  return JSON.parse(text(wasm.music_song(ptr, len, payload.seed ?? 1, payload.bpm ?? -1, payload.key ?? -1)));
}

// ── Musical Roles (8 Rows of the RGB LED Matrix & Channel Strip) ──
const ROLES = [
  { key: 'K',  name: 'k',      label: 'KICK',  short: 'K',  color: [255, 46, 110],  hex: '#ff2e6e', drum: 'kick' },
  { key: 'C',  name: 'c',      label: 'CLAP',  short: 'C',  color: [255, 123, 51],  hex: '#ff7b33', drum: 'snare' },
  { key: 'H',  name: 'h',      label: 'HATS',  short: 'H',  color: [255, 202, 58],  hex: '#ffca3a', drum: 'hat' },
  { key: 'P',  name: 'p',      label: 'PERC',  short: 'P',  color: [184, 242, 58],  hex: '#b8f23a', drum: 'tomM' },
  { key: 'B',  name: 'bass',   label: 'BASS',  short: 'B',  color: [26, 255, 122],  hex: '#1aff7a', synth: true },
  { key: 'Pd', name: 'chords', label: 'CHORD', short: 'Pd', color: [58, 168, 255],  hex: '#3aa8ff', synth: true },
  { key: 'A',  name: 'arp',    label: 'ARP',   short: 'A',  color: [164, 120, 255], hex: '#a478ff', synth: true },
  { key: 'L',  name: 'lead',   label: 'LEAD',  short: 'L',  color: [255, 79, 227],  hex: '#ff4fe3', synth: true },
];

const BASS_STYLES = [
  { key: 'eighths',    label: '8THS',   hud: '8TH' },
  { key: 'house',      label: 'HOUSE',  hud: 'HSE' },
  { key: 'rolling',    label: 'ROLL',   hud: 'ROL' },
  { key: 'acid',       label: '303',    hud: '303' },
  { key: 'trance',     label: 'TRANCE', hud: 'TRN' },
  { key: 'dnb',        label: 'DNB',    hud: 'DNB' },
  { key: 'wobble',     label: 'WOBBLE', hud: 'WOB' },
  { key: 'orchestral', label: 'ORCH',   hud: 'ORC' },
];

const MODE_NAMES = ['PHRYG', 'AEOL', 'DORIAN', 'MIXOL', 'IONIAN'];
const MODE_SCALES = [
  [0, 1, 3, 5, 7, 8, 10, 12], // Phrygian
  [0, 2, 3, 5, 7, 8, 10, 12], // Aeolian
  [0, 2, 3, 5, 7, 9, 10, 12], // Dorian
  [0, 2, 4, 5, 7, 9, 10, 12], // Mixolydian
  [0, 2, 4, 5, 7, 9, 11, 12], // Ionian
];
const DRUM_PAD_LANES = [
  ['kick', 'KICK'], ['snare', 'SNARE'], ['clap', 'CLAP'], ['hat', 'HAT'],
  ['ohat', 'OHAT'], ['rim', 'RIM'], ['tomM', 'TOM'], ['crash', 'CRASH'],
];

// ── State ──
const q = new URLSearchParams(location.search);
let genre = (q.get('genre') in PROFILES) ? q.get('genre') : 'synthwave';
let seed = Math.max(1, Number(q.get('seed')) || 7);
let dna = {};
let mix = { tape: 0.35, glue: 0.5, reverb: 1.0, revSize: 2.4, revTone: 0.5, master: 1.0 };
let song = null;
let playing = false;
let looping = false;
let matrixMode = 'step'; // 'step' | 'song' | 'dna'
let roleAction = 'mute'; // 'mute' | 'focus' | 'solo'
let focusedRole = 'B';
let jamMode = 'synth'; // 'synth' | 'drums'
let jamOctave = 4;
const muted = new Set();
let soloRole = null;
const patchOverrides = {};

let ctx = null, lab = null, volNode = null, analyser = null;
let posAt = { bar: 0, sec: 0, step: 0, barIndex: 0, bars: 128, t: performance.now(), playing: false };
let lastGr = 1.0, lastLoad = 0.0;
let hudOverlay = null; // { text: 'SWG 58', norm: 0.5, until: timestamp, color: [r,g,b] }

const post = (m) => lab?.port.postMessage(m);
const setLcd = (msg) => { $('lcdLine').textContent = msg; };

function showHud(label, norm, color = [58, 168, 255]) {
  hudOverlay = { text: label.toUpperCase().slice(0, 6), norm: Math.max(0, Math.min(1, norm)), color, until: performance.now() + 750 };
}

// ── Populate Genre Preset Selector ──
for (const [k, p] of Object.entries(PROFILES)) {
  $('genreSel').append(new Option(p.name.toUpperCase(), k));
}
$('genreSel').value = genre;

function updateBackLink() {
  $('backLab').href = `index.html?genre=${encodeURIComponent(genre)}&seed=${seed}`;
}

function loadProfileDefaults(gKey) {
  const p = PROFILES[gKey] || PROFILES.synthwave;
  dna = {
    bpm: Math.round(p.bpm),
    swing: Math.round(p.swing),
    mode: p.mode,
    bass: p.bass,
    chords: p.chords,
    ext: Number(p.ext.toFixed(2)),
    arp: !!p.arp,
    leadDensity: Number(p.leadDensity.toFixed(1)),
    percDensity: Number(p.percDensity.toFixed(2)),
    tomFills: !!p.tomFills,
    gated: Number(p.gated.toFixed(2)),
    pump: Number(p.pump.toFixed(2)),
  };
  mix.tape = Number(p.tape.toFixed(2));
  mix.glue = Number(p.glue.toFixed(2));
  mix.reverb = Number(p.reverb.toFixed(2));
  for (const k of Object.keys(patchOverrides)) delete patchOverrides[k];
  $('dnaBadge').textContent = `${p.name} Profile`;
}

function buildSongPayload(morph = false) {
  return {
    type: 'song',
    genre,
    seed,
    bpm: dna.bpm,
    swing: dna.swing,
    mode: dna.mode,
    bass: dna.bass,
    chords: dna.chords,
    ext: dna.ext,
    arp: dna.arp,
    leadDensity: dna.leadDensity,
    percDensity: dna.percDensity,
    tomFills: dna.tomFills,
    gated: dna.gated,
    pump: dna.pump,
    morph,
  };
}

let morphTimer = null;
function rebuildSong(morph = false) {
  const payload = buildSongPayload(morph);
  song = makeSongWithSettings(payload);
  // Re-apply any manual synth patch edits on top of the regenerated song
  for (const [layerName, labP] of Object.entries(patchOverrides)) {
    const l = song?.layers?.find((x) => x.name === layerName);
    if (l && l.lab) Object.assign(l.lab, labP);
  }
  updateBackLink();
  syncSwitchesUI();
  syncAllKnobsUI();
  syncSynthKnobsToFocused();
  buildJamPads();
  if (lab) {
    post(payload);
    post({ type: 'mix', ...mix });
    for (const [part, labP] of Object.entries(patchOverrides)) {
      post({ type: 'patch', part, lab: labP });
    }
    sendMutes();
  }
  if (song) {
    setLcd(`${song.style} · ${song.bpm} BPM · SWG ${song.swing}%`);
  }
}

function scheduleMorph(badgeText) {
  $('dnaBadge').textContent = badgeText || 'Custom DNA';
  if (morphTimer) clearTimeout(morphTimer);
  morphTimer = setTimeout(() => {
    morphTimer = null;
    rebuildSong(true);
  }, 45);
}

// ── Bass Style Pills & Structural Switches ──
function buildBassSwitchRow() {
  const el = $('bassRow');
  el.innerHTML = '';
  for (const b of BASS_STYLES) {
    const btn = document.createElement('button');
    btn.className = 'sw-pill' + (dna.bass === b.key ? ' on' : '');
    btn.textContent = b.label;
    btn.dataset.bass = b.key;
    btn.onclick = () => {
      dna.bass = b.key;
      syncSwitchesUI();
      showHud(`BAS ${b.hud}`, BASS_STYLES.indexOf(b) / (BASS_STYLES.length - 1), [26, 255, 122]);
      scheduleMorph(`Bass: ${b.label}`);
    };
    el.append(btn);
  }
}

function syncSwitchesUI() {
  for (const btn of $('bassRow').children) {
    btn.classList.toggle('on', btn.dataset.bass === dna.bass);
  }
  const isStabs = dna.chords === 'stabs';
  $('chordStyleBtn').classList.toggle('on', isStabs);
  $('chordStyleBtn').innerHTML = `CHORD: <b>${isStabs ? 'STABS' : 'PAD'}</b>`;

  $('arpBtn').classList.toggle('on', !!dna.arp);
  $('arpBtn').innerHTML = `ARP: <b>${dna.arp ? 'ON' : 'OFF'}</b>`;

  $('fillsBtn').classList.toggle('on', !!dna.tomFills);
  $('fillsBtn').innerHTML = `FILL: <b>${dna.tomFills ? 'TOMS' : 'HOUSE'}</b>`;
}

$('chordStyleBtn').onclick = () => {
  dna.chords = dna.chords === 'stabs' ? 'sustain' : 'stabs';
  syncSwitchesUI();
  showHud(dna.chords === 'stabs' ? 'STABS' : 'PADS', dna.chords === 'stabs' ? 1 : 0, [58, 168, 255]);
  scheduleMorph(`Chords: ${dna.chords.toUpperCase()}`);
};
$('arpBtn').onclick = () => {
  dna.arp = !dna.arp;
  syncSwitchesUI();
  showHud(dna.arp ? 'ARP ON' : 'ARPOFF', dna.arp ? 1 : 0, [164, 120, 255]);
  scheduleMorph(`Arp: ${dna.arp ? 'ON' : 'OFF'}`);
};
$('fillsBtn').onclick = () => {
  dna.tomFills = !dna.tomFills;
  syncSwitchesUI();
  showHud(dna.tomFills ? '80STOM' : 'HSETURN', dna.tomFills ? 1 : 0, [255, 202, 58]);
  scheduleMorph(`Fills: ${dna.tomFills ? '80s Toms' : 'House'}`);
};

// ── Touch/Mouse Rotary Knob Component ──
const allKnobs = [];

function createKnob(container, cfg) {
  const cell = document.createElement('div');
  cell.className = 'knob-cell';
  cell.dataset.knob = cfg.id;
  cell.innerHTML = `
    <svg class="knob-svg" viewBox="0 0 48 48">
      <circle cx="24" cy="24" r="18" fill="#0d1017" stroke="#283040" stroke-width="2.5"/>
      <path class="k-track" fill="none" stroke="#222938" stroke-width="3.5" stroke-linecap="round"/>
      <path class="k-arc" fill="none" stroke="${cfg.color || '#3aa8ff'}" stroke-width="3.5" stroke-linecap="round"/>
      <circle cx="24" cy="24" r="12.5" fill="#1b212e" stroke="#364056" stroke-width="1.2"/>
      <line class="k-ptr" x1="24" y1="24" x2="24" y2="12" stroke="#edf2f4" stroke-width="2.2" stroke-linecap="round"/>
    </svg>
    <div class="knob-label">${cfg.label}</div>
    <div class="knob-val"></div>
  `;
  const track = cell.querySelector('.k-track');
  const arc = cell.querySelector('.k-arc');
  const ptr = cell.querySelector('.k-ptr');
  const valEl = cell.querySelector('.knob-val');

  const polar = (cx, cy, r, deg) => {
    const rad = ((deg - 90) * Math.PI) / 180;
    return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
  };
  const arcPath = (a0, a1) => {
    if (Math.abs(a1 - a0) < 0.5) return '';
    const [x0, y0] = polar(24, 24, 18, a0);
    const [x1, y1] = polar(24, 24, 18, a1);
    const large = a1 - a0 > 180 ? 1 : 0;
    return `M ${x0.toFixed(2)} ${y0.toFixed(2)} A 18 18 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
  };
  track.setAttribute('d', arcPath(-135, 135));

  const updateVisual = () => {
    const v = cfg.get();
    const norm = Math.max(0, Math.min(1, (v - cfg.min) / (cfg.max - cfg.min || 1)));
    const deg = -135 + norm * 270;
    arc.setAttribute('d', arcPath(-135, deg));
    const [px, py] = polar(24, 24, 11.5, deg);
    ptr.setAttribute('x2', px.toFixed(2));
    ptr.setAttribute('y2', py.toFixed(2));
    valEl.textContent = cfg.fmt ? cfg.fmt(v) : String(v);
  };

  let startY = 0, startVal = 0;
  cell.onpointerdown = (e) => {
    e.preventDefault();
    cell.setPointerCapture(e.pointerId);
    cell.classList.add('dragging');
    startY = e.clientY;
    startVal = cfg.get();
  };
  cell.onpointermove = (e) => {
    if (!cell.classList.contains('dragging')) return;
    const dy = startY - e.clientY;
    const span = cfg.max - cfg.min;
    const raw = startVal + (dy / 110) * span;
    const stepped = Math.round(raw / cfg.step) * cfg.step;
    const clamped = Math.max(cfg.min, Math.min(cfg.max, Number(stepped.toFixed(4))));
    if (clamped !== cfg.get()) {
      cfg.set(clamped);
      updateVisual();
      const norm = (clamped - cfg.min) / (span || 1);
      showHud(cfg.hud ? cfg.hud(clamped) : `${cfg.label.slice(0, 3)} ${Math.round(norm * 99)}`, norm, cfg.rgb || [58, 168, 255]);
    }
  };
  cell.onpointerup = cell.onpointercancel = () => cell.classList.remove('dragging');
  cell.ondblclick = () => {
    if (cfg.reset != null) {
      cfg.set(cfg.reset());
      updateVisual();
    }
  };

  container.append(cell);
  const handle = { id: cfg.id, updateVisual, cfg };
  allKnobs.push(handle);
  updateVisual();
  return handle;
}

function syncAllKnobsUI() {
  for (const k of allKnobs) k.updateVisual();
}

// ── Build Genre DNA Knobs (Left Module) ──
const DNA_KNOB_DEFS = [
  {
    id: 'swing', label: 'SWING', min: 50, max: 66, step: 1, color: '#ffca3a', rgb: [255, 202, 58],
    get: () => dna.swing,
    set: (v) => { dna.swing = v; scheduleMorph(`Swing ${v}%`); },
    fmt: (v) => `${v}%`,
    hud: (v) => `SWG ${v}`,
  },
  {
    id: 'ext', label: 'HARMONY', min: 0, max: 1, step: 0.05, color: '#3aa8ff', rgb: [58, 168, 255],
    get: () => dna.ext,
    set: (v) => { dna.ext = v; scheduleMorph(`Harmony ${Math.round(v * 100)}%`); },
    fmt: (v) => v < 0.2 ? 'TRIAD' : v < 0.65 ? '7THS' : '9TH/11',
    hud: (v) => `EXT ${Math.round(v * 99)}`,
  },
  {
    id: 'mode', label: 'MODE', min: 0, max: 4, step: 1, color: '#a478ff', rgb: [164, 120, 255],
    get: () => dna.mode,
    set: (v) => { dna.mode = v; buildJamPads(); scheduleMorph(`Mode ${MODE_NAMES[v]}`); },
    fmt: (v) => MODE_NAMES[v] || 'AEOL',
    hud: (v) => MODE_NAMES[v] || 'AEOL',
  },
  {
    id: 'percDensity', label: 'TOP PERC', min: 0, max: 1, step: 0.05, color: '#b8f23a', rgb: [184, 242, 58],
    get: () => dna.percDensity,
    set: (v) => { dna.percDensity = v; scheduleMorph(`Perc ${Math.round(v * 100)}%`); },
    fmt: (v) => `${Math.round(v * 100)}%`,
    hud: (v) => `PRC ${Math.round(v * 99)}`,
  },
  {
    id: 'leadDensity', label: 'LEAD HOOK', min: 0, max: 5.5, step: 0.5, color: '#ff4fe3', rgb: [255, 79, 227],
    get: () => dna.leadDensity,
    set: (v) => { dna.leadDensity = v; scheduleMorph(v === 0 ? 'Lead OFF' : `Lead ${v}/bar`); },
    fmt: (v) => v <= 0.05 ? 'OFF' : `${v.toFixed(1)}/B`,
    hud: (v) => v <= 0.05 ? 'LD OFF' : `LD ${v.toFixed(1)}`,
  },
  {
    id: 'bpm', label: 'TEMPO', min: 76, max: 178, step: 1, color: '#1aff7a', rgb: [26, 255, 122],
    get: () => dna.bpm,
    set: (v) => { dna.bpm = v; scheduleMorph(`${v} BPM`); },
    fmt: (v) => `${v} BPM`,
    hud: (v) => `BPM${v}`,
  },
];

// ── Build Era & Bus Physics Knobs (Right Module Top) ──
const BUS_KNOB_DEFS = [
  {
    id: 'pump', label: 'PUMP', min: 0, max: 0.8, step: 0.02, color: '#ff2e6e', rgb: [255, 46, 110],
    get: () => dna.pump,
    set: (v) => { dna.pump = v; scheduleMorph(`Pump ${Math.round(v * 100)}%`); },
    fmt: (v) => `${Math.round(v * 100)}%`,
    hud: (v) => `PMP ${Math.round(v * 99)}`,
  },
  {
    id: 'gated', label: '80S GATE', min: 0, max: 0.9, step: 0.05, color: '#ff7b33', rgb: [255, 123, 51],
    get: () => dna.gated,
    set: (v) => { dna.gated = v; scheduleMorph(`Gated ${Math.round(v * 100)}%`); },
    fmt: (v) => v === 0 ? 'DRY' : `${Math.round(v * 100)}%`,
    hud: (v) => `GAT ${Math.round(v * 99)}`,
  },
  {
    id: 'tape', label: 'TAPE SAT', min: 0, max: 0.9, step: 0.02, color: '#ffca3a', rgb: [255, 202, 58],
    get: () => mix.tape,
    set: (v) => { mix.tape = v; post({ type: 'mix', tape: v }); },
    fmt: (v) => `${Math.round(v * 100)}%`,
    hud: (v) => `TAP ${Math.round(v * 99)}`,
  },
  {
    id: 'reverb', label: 'HALL REV', min: 0, max: 1.8, step: 0.05, color: '#3aa8ff', rgb: [58, 168, 255],
    get: () => mix.reverb,
    set: (v) => { mix.reverb = v; post({ type: 'mix', reverb: v }); },
    fmt: (v) => `${Math.round(v * 100)}%`,
    hud: (v) => `REV ${Math.round(v * 55)}`,
  },
];

// ── Build Active Synth Sculptor Knobs (Right Module Bottom) ──
const synthState = { cutoff: 1800, res: 0.25, envMod: 0.5, decay: 0.35 };

function getFocusedLayer() {
  if (!song?.layers) return null;
  const roleInfo = ROLES.find((r) => r.key === focusedRole);
  return song.layers.find((l) => l.role === focusedRole || l.name === roleInfo?.name)
    || song.layers.find((l) => l.lab);
}

function syncSynthKnobsToFocused() {
  const l = getFocusedLayer();
  if (!l || !l.lab) {
    $('synthBadge').textContent = 'NONE';
    return;
  }
  const p = l.lab;
  $('synthBadge').textContent = `${l.name.toUpperCase()} · ${p.name || p.kind}`;
  synthState.cutoff = p.cutoff ?? p.brightness ?? 2200;
  synthState.res = p.res ?? p.feedback ?? 0.25;
  synthState.envMod = p.envMod ?? p.velIndex ?? p.drive ?? 0.45;
  synthState.decay = p.decay ?? p.fd ?? p.d ?? 0.35;
  syncAllKnobsUI();
  // Also set audition synth to match focused layer so Scale Synth Jam Pads play this voice
  post({ type: 'audition', lab: structuredClone(p) });
}

function applyFocusedSynthEdit(mutate) {
  const l = getFocusedLayer();
  if (!l || !l.lab) return;
  mutate(l.lab);
  patchOverrides[l.name] = structuredClone(l.lab);
  post({ type: 'patch', part: l.name, lab: patchOverrides[l.name] });
  post({ type: 'audition', lab: patchOverrides[l.name] });
}

const SYNTH_KNOB_DEFS = [
  {
    id: 'cutoff', label: 'CUTOFF', min: 80, max: 12000, step: 20, color: '#1aff7a', rgb: [26, 255, 122],
    get: () => synthState.cutoff,
    set: (v) => {
      synthState.cutoff = v;
      applyFocusedSynthEdit((p) => { if ('brightness' in p && !('cutoff' in p)) p.brightness = v; else p.cutoff = v; });
    },
    fmt: (v) => v >= 1000 ? `${(v / 1000).toFixed(1)}k` : `${Math.round(v)}Hz`,
    hud: (v) => `CUT ${Math.round((v / 12000) * 99)}`,
  },
  {
    id: 'res', label: 'RESO', min: 0, max: 0.92, step: 0.02, color: '#ff2e6e', rgb: [255, 46, 110],
    get: () => synthState.res,
    set: (v) => {
      synthState.res = v;
      applyFocusedSynthEdit((p) => { if (p.kind === 'fm') p.feedback = v; else p.res = v; });
    },
    fmt: (v) => `${Math.round(v * 100)}%`,
    hud: (v) => `RES ${Math.round(v * 99)}`,
  },
  {
    id: 'envMod', label: 'ENV / MOD', min: 0, max: 1.5, step: 0.03, color: '#ffca3a', rgb: [255, 202, 58],
    get: () => synthState.envMod,
    set: (v) => {
      synthState.envMod = v;
      applyFocusedSynthEdit((p) => {
        if (p.kind === 'fm') p.velIndex = v;
        else p.envMod = v;
      });
    },
    fmt: (v) => `${Math.round(v * 100)}%`,
    hud: (v) => `ENV ${Math.round((v / 1.5) * 99)}`,
  },
  {
    id: 'decay', label: 'DECAY', min: 0.05, max: 1.8, step: 0.02, color: '#a478ff', rgb: [164, 120, 255],
    get: () => synthState.decay,
    set: (v) => {
      synthState.decay = v;
      applyFocusedSynthEdit((p) => {
        if ('decay' in p || p.kind === 'tb303') p.decay = v;
        else { p.d = v; p.fd = v; }
      });
    },
    fmt: (v) => `${Math.round(v * 1000)}ms`,
    hud: (v) => `DEC ${Math.round((v / 1.8) * 99)}`,
  },
];

loadProfileDefaults(genre);
buildBassSwitchRow();
for (const d of DNA_KNOB_DEFS) createKnob($('dnaKnobs'), d);
for (const d of BUS_KNOB_DEFS) createKnob($('busKnobs'), d);
for (const d of SYNTH_KNOB_DEFS) createKnob($('synthKnobs'), d);

// ── Bottom Left: 8 Illuminated Role Channel Keys ──
function sendMutes() {
  if (!song?.layers) return;
  const mutedNames = [];
  for (const l of song.layers) {
    if (muted.has(l.role)) mutedNames.push(l.name);
  }
  post({ type: 'mute', names: mutedNames });
  const soloLayer = soloRole ? song.layers.find((l) => l.role === soloRole)?.name : null;
  post({ type: 'solo', name: soloLayer || '' });
}

function buildRolePads() {
  const el = $('roleGrid');
  el.innerHTML = '';
  for (const r of ROLES) {
    const b = document.createElement('button');
    b.className = 'role-pad'
      + (muted.has(r.key) ? ' muted' : '')
      + (focusedRole === r.key ? ' focused' : '')
      + (soloRole === r.key ? ' soloed' : '');
    b.style.setProperty('--c', r.hex);
    b.dataset.role = r.key;
    b.innerHTML = `<i class="led"></i><span>${r.short}</span><small>${r.label}</small>`;
    b.onclick = () => {
      if (roleAction === 'focus') {
        if (r.synth) {
          focusedRole = r.key;
          syncSynthKnobsToFocused();
        } else if (r.drum) {
          triggerDrumHit(r.drum);
        }
      } else if (roleAction === 'solo') {
        soloRole = soloRole === r.key ? null : r.key;
        sendMutes();
      } else {
        if (muted.has(r.key)) muted.delete(r.key);
        else muted.add(r.key);
        sendMutes();
      }
      buildRolePads();
    };
    el.append(b);
  }
}

for (const [btnId, mode] of [['actMute', 'mute'], ['actFocus', 'focus'], ['actSolo', 'solo']]) {
  $(btnId).onclick = () => {
    roleAction = mode;
    for (const id of ['actMute', 'actFocus', 'actSolo']) $(id).classList.toggle('on', id === btnId);
  };
}
$('unmuteAll').onclick = () => {
  muted.clear();
  soloRole = null;
  sendMutes();
  buildRolePads();
};

// ── Bottom Right: 8 Scale-Locked Jam / Drum Pads ──
const NOTE_NAMES = ['C', 'C#', 'D', 'Eb', 'E', 'F', 'F#', 'G', 'Ab', 'A', 'Bb', 'B'];
function songTonicPc() {
  if (!song?.style) return 9; // default A
  const parts = song.style.split('·')[1]?.trim().split(/\s+/) || [];
  const idx = NOTE_NAMES.indexOf(parts[0]);
  return idx >= 0 ? idx : 9;
}

async function triggerSynthNoteOn(midi, padEl) {
  await ensureAudio(); await ctx.resume();
  const l = getFocusedLayer();
  if (l?.lab) post({ type: 'audition', lab: l.lab });
  post({ type: 'on', midi, vel: 0.92, accent: true });
  padEl?.classList.add('down');
}
function triggerSynthNoteOff(midi, padEl) {
  post({ type: 'off', midi });
  padEl?.classList.remove('down');
}
async function triggerDrumHit(lane, padEl) {
  await ensureAudio(); await ctx.resume();
  post({ type: 'hit', lane, vel: 0.95 });
  if (padEl) {
    padEl.classList.add('down');
    setTimeout(() => padEl.classList.remove('down'), 110);
  }
}

function buildJamPads() {
  const el = $('jamGrid');
  el.innerHTML = '';
  if (jamMode === 'synth') {
    const pc = songTonicPc();
    const scale = MODE_SCALES[dna.mode] || MODE_SCALES[1];
    const baseMidi = 12 * (jamOctave + 1) + pc;
    $('jamTitle').textContent = `SCALE PADS (${NOTE_NAMES[pc]} ${MODE_NAMES[dna.mode] || 'MIN'})`;
    for (let i = 0; i < 8; i++) {
      const m = baseMidi + scale[i];
      const noteName = NOTE_NAMES[m % 12] + (Math.floor(m / 12) - 1);
      const b = document.createElement('button');
      b.className = 'jam-pad';
      b.innerHTML = `<span>${noteName}</span><small>DEG ${i + 1}</small>`;
      b.onpointerdown = (e) => { b.setPointerCapture(e.pointerId); triggerSynthNoteOn(m, b); };
      b.onpointerup = b.onpointercancel = () => triggerSynthNoteOff(m, b);
      el.append(b);
    }
  } else {
    $('jamTitle').textContent = 'DRUM PADS (LIVE KIT)';
    for (const [lane, label] of DRUM_PAD_LANES) {
      const b = document.createElement('button');
      b.className = 'jam-pad';
      b.innerHTML = `<span>${label}</span><small>${lane}</small>`;
      b.onpointerdown = () => triggerDrumHit(lane, b);
      el.append(b);
    }
  }
}

$('jamModeSynth').onclick = () => {
  jamMode = 'synth';
  $('jamModeSynth').classList.add('on');
  $('jamModeDrums').classList.remove('on');
  buildJamPads();
};
$('jamModeDrums').onclick = () => {
  jamMode = 'drums';
  $('jamModeDrums').classList.add('on');
  $('jamModeSynth').classList.remove('on');
  buildJamPads();
};
$('jamOctDown').onclick = () => { jamOctave = Math.max(2, jamOctave - 1); buildJamPads(); };
$('jamOctUp').onclick = () => { jamOctave = Math.min(6, jamOctave + 1); buildJamPads(); };

// ── Top Bezel Controls (Genre, Seed, Dice, Reset, Loop, Play, Fullscreen) ──
$('genreSel').onchange = () => {
  genre = $('genreSel').value;
  loadProfileDefaults(genre);
  rebuildSong(playing);
  showHud(genre.slice(0, 6), 1.0, [26, 255, 122]);
};
$('seedBtn').onclick = () => {
  const next = prompt('Enter song seed (1–99999):', String(seed));
  if (next != null && Number(next) >= 1) {
    seed = Math.floor(Number(next));
    $('seedVal').textContent = String(seed);
    rebuildSong(playing);
  }
};
$('diceBtn').onclick = () => {
  seed = 1 + Math.floor(Math.random() * 9999);
  $('seedVal').textContent = String(seed);
  rebuildSong(playing);
  showHud(`SD${seed}`, (seed % 100) / 100, [255, 202, 58]);
};
$('resetDnaBtn').onclick = () => {
  loadProfileDefaults(genre);
  rebuildSong(playing);
  showHud('RESET', 0.5, [58, 168, 255]);
};
$('loopBtn').onclick = () => {
  looping = !looping;
  $('loopBtn').classList.toggle('on', looping);
  if (looping) {
    const b0 = Math.floor((posAt.barIndex || 0) / 4) * 4;
    post({ type: 'loop', start: b0, end: b0 + 4 });
    showHud(`LP ${b0 + 1}`, 1.0, [255, 202, 58]);
  } else {
    post({ type: 'loop' });
    showHud('LP OFF', 0.0, [255, 202, 58]);
  }
};
$('fsBtn').onclick = () => {
  if (!document.fullscreenElement) {
    document.documentElement.requestFullscreen?.().catch(() => {});
  } else {
    document.exitFullscreen?.().catch(() => {});
  }
};

for (const id of ['modeStep', 'modeSong', 'modeDna']) {
  $(id).onclick = () => {
    matrixMode = $(id).dataset.mode;
    for (const m of ['modeStep', 'modeSong', 'modeDna']) $(m).classList.toggle('on', m === id);
  };
}

// ── Web Audio Worklet Initialization ──
let starting = null;
function ensureAudio() { return (starting ||= initAudio()); }
async function initAudio() {
  const AC = window.AudioContext || window.webkitAudioContext;
  try { if (navigator.audioSession) navigator.audioSession.type = 'playback'; } catch {}
  const ac = new AC({ latencyHint: 'playback' });
  await ac.audioWorklet.addModule(WORKLET_URL);
  lab = new AudioWorkletNode(ac, 'music-lab', {
    numberOfInputs: 0,
    outputChannelCount: [2],
    processorOptions: {
      module,
      seed: 5,
      messages: [buildSongPayload(false), { type: 'mix', ...mix }],
    },
  });
  lab.port.onmessage = (m) => onWorkletMsg(m.data);
  volNode = ac.createGain();
  volNode.gain.value = 0.8;
  analyser = ac.createAnalyser();
  analyser.fftSize = 1024;
  lab.connect(volNode);
  volNode.connect(analyser);
  analyser.connect(ac.destination);
  ctx = ac;
  syncSynthKnobsToFocused();
  sendMutes();
  window.groovebox.ctx = ctx;
  window.groovebox.lab = lab;
}

async function startAt(bar = 0) {
  await ensureAudio();
  await ctx.resume();
  post({ type: 'play', bar, delay: 0.02 });
  playing = true;
  $('playBtn').textContent = '■ STOP';
  $('playBtn').classList.add('on');
  if (looping) {
    const b0 = Math.floor(bar / 4) * 4;
    post({ type: 'loop', start: b0, end: b0 + 4 });
  }
}

function stopPlayback() {
  playing = false;
  post({ type: 'stop' });
  $('playBtn').textContent = '▶ PLAY';
  $('playBtn').classList.remove('on');
}

$('playBtn').onclick = async () => {
  if (!playing) {
    // Start on the first full groove/main bar if stopped at 0 so the user immediately hears the band
    const startBar = posAt.barIndex > 0 ? posAt.barIndex : (song?.sections?.[1]?.bar ?? 0);
    await startAt(startBar);
  } else {
    stopPlayback();
  }
};

function onWorkletMsg(d) {
  if (d.type === 'pos' && d.pos) {
    const p = d.pos;
    lastGr = d.gr ?? 1.0;
    lastLoad = d.load ?? 0.0;
    posAt = {
      sec: p.sec,
      bar: p.barIndex + p.step / 16,
      barIndex: p.barIndex,
      step: p.step,
      bars: p.bars,
      t: performance.now(),
      playing: p.playing,
    };
    const secName = song?.sections?.[p.sec]?.name?.toUpperCase() || 'SECT';
    $('posBadge').textContent = `${secName} · BAR ${p.barIndex + 1} · ${p.step + 1}`;
    const grDb = lastGr < 0.999 ? (20 * Math.log10(lastGr)).toFixed(1) : '0.0';
    $('grBadge').textContent = `GLUE ${grDb} dB`;
    $('cpuChip').textContent = `DSP ${Math.round(lastLoad * 100)}%`;
    window.groovebox.pos = p;
  }
}

// ── 3×5 Pixel Font for the 16×8 RGB LED Matrix HUD Overlay ──
const PIXEL_FONT = {
  '0': [7,5,5,5,7], '1': [2,6,2,2,7], '2': [7,1,7,4,7], '3': [7,1,7,1,7], '4': [5,5,7,1,1],
  '5': [7,4,7,1,7], '6': [7,4,7,5,7], '7': [7,1,1,1,1], '8': [7,5,7,5,7], '9': [7,5,7,1,7],
  'A': [2,5,7,5,5], 'B': [6,5,6,5,6], 'C': [3,4,4,4,3], 'D': [6,5,5,5,6], 'E': [7,4,7,4,7],
  'F': [7,4,7,4,4], 'G': [3,4,5,5,3], 'H': [5,5,7,5,5], 'I': [7,2,2,2,7], 'J': [1,1,1,5,2],
  'K': [5,5,6,5,5], 'L': [4,4,4,4,7], 'M': [5,7,7,5,5], 'N': [5,7,7,7,5], 'O': [2,5,5,5,2],
  'P': [6,5,6,4,4], 'Q': [2,5,5,7,3], 'R': [6,5,6,5,5], 'S': [3,4,2,1,6], 'T': [7,2,2,2,2],
  'U': [5,5,5,5,7], 'V': [5,5,5,5,2], 'W': [5,5,7,7,5], 'X': [5,5,2,5,5], 'Y': [5,5,2,2,2],
  'Z': [7,1,2,4,7], '.': [0,0,0,0,2], '%': [5,1,2,4,5], ' ': [0,0,0,0,0],
};

// ── 16×8 Discrete RGB LED Pixel Matrix Renderer ──
const canvas = $('ledMatrix');
const g = canvas.getContext('2d');

// Click on the 16×8 RGB LED Matrix:
// - In SONG mode: jumps to the clicked section column!
// - In STEP mode: triggers/auditions that role row and focuses it!
canvas.onpointerdown = async (e) => {
  const rect = canvas.getBoundingClientRect();
  const nx = (e.clientX - rect.left) / rect.width;
  const ny = (e.clientY - rect.top) / rect.height;
  const col = Math.floor((nx - 0.06) / (0.83 / 16));
  const row = Math.floor((ny - 0.08) / (0.88 / 8));
  if (col < 0 || col >= 16 || row < 0 || row >= 8) return;

  if (matrixMode === 'song' && song?.sections?.length) {
    const secIdx = Math.min(song.sections.length - 1, Math.floor((col / 16) * song.sections.length));
    const targetBar = song.sections[secIdx].bar;
    await startAt(targetBar);
    showHud(`BAR${targetBar + 1}`, col / 15, [26, 255, 122]);
  } else {
    const r = ROLES[row];
    if (r.synth) {
      focusedRole = r.key;
      syncSynthKnobsToFocused();
      buildRolePads();
      const pc = songTonicPc();
      const scale = MODE_SCALES[dna.mode] || MODE_SCALES[1];
      const midi = 12 * (jamOctave + 1) + pc + scale[col % 8];
      await triggerSynthNoteOn(midi, null);
      setTimeout(() => triggerSynthNoteOff(midi, null), 180);
    } else if (r.drum) {
      await triggerDrumHit(r.drum, null);
    }
  }
};

function computeStepGrid(barIndex) {
  // Returns 8x16 cells: { level: 0..1, accent: bool, slide: bool, sustain: bool }
  const grid = Array.from({ length: 8 }, () =>
    Array.from({ length: 16 }, () => ({ level: 0, accent: false, slide: false, sustain: false }))
  );
  if (!song?.layers) return grid;

  ROLES.forEach((roleInfo, rIdx) => {
    if (muted.has(roleInfo.key) || (soloRole && soloRole !== roleInfo.key)) return;
    const layer = song.layers.find((l) => l.role === roleInfo.key);
    if (!layer || !layer.clips || !layer.patterns) return;
    const clip = layer.clips.find((c) => barIndex >= c.bar && barIndex < c.bar + c.bars);
    if (!clip) return;
    const pat = layer.patterns[clip.patIdx ?? 0];
    if (!pat || !pat.notes || pat.len <= 0) return;

    const barInClip = barIndex - clip.bar;
    const patStepStart = (barInClip * 16) % pat.len;
    const patStepEnd = patStepStart + 16;

    for (const n of pat.notes) {
      if (n.at >= patStepStart - 0.1 && n.at < patStepEnd - 0.05) {
        const s0 = Math.max(0, Math.min(15, Math.round(n.at - patStepStart)));
        const cell = grid[rIdx][s0];
        cell.level = Math.max(cell.level, n.vel || 0.75);
        if (n.accent) cell.accent = true;
        if (n.slide) cell.slide = true;
        const durSteps = Math.min(16 - s0, Math.floor(n.len || 1));
        for (let d = 1; d < durSteps; d++) {
          const trail = grid[rIdx][s0 + d];
          trail.level = Math.max(trail.level, (n.vel || 0.7) * 0.42);
          trail.sustain = true;
        }
      }
    }
  });
  return grid;
}

const timeBuf = new Float32Array(256);

function renderMatrix() {
  requestAnimationFrame(renderMatrix);
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  const cw = canvas.clientWidth * dpr;
  const ch = canvas.clientHeight * dpr;
  if (cw > 0 && ch > 0 && (canvas.width !== cw || canvas.height !== ch)) {
    canvas.width = cw;
    canvas.height = ch;
  }
  const W = canvas.width, H = canvas.height;
  g.fillStyle = '#050609';
  g.fillRect(0, 0, W, H);

  // Live interpolated bar & 16th-step
  let curBar = posAt.barIndex || 0;
  let curStep = posAt.step || 0;
  if (posAt.playing && song?.bpm) {
    const dt = (performance.now() - posAt.t) / 1000;
    const exactBar = (posAt.bar + dt * (song.bpm / 240)) % (song.bars || 128);
    curBar = Math.floor(exactBar);
    curStep = Math.floor((exactBar - curBar) * 16) % 16;
  }

  const leftPad = W * 0.058;
  const topPad = H * 0.085;
  const gridW = W * 0.84;
  const gridH = H * 0.88;
  const cellW = gridW / 16;
  const cellH = gridH / 8;

  // Top step numbers 1..16
  g.font = `bold ${Math.max(8, Math.floor(H * 0.036))}px ui-monospace, monospace`;
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  for (let c = 0; c < 16; c++) {
    const isBeat = c % 4 === 0;
    g.fillStyle = (posAt.playing && c === curStep && matrixMode === 'step')
      ? '#ffffff'
      : isBeat ? '#8d99ae' : '#434c5e';
    g.fillText(String(c + 1), leftPad + (c + 0.5) * cellW, topPad * 0.5);
  }

  // Left role labels K..L
  for (let r = 0; r < 8; r++) {
    const role = ROLES[r];
    g.fillStyle = muted.has(role.key) ? '#3a4150' : role.hex;
    g.fillText(role.short, leftPad * 0.48, topPad + (r + 0.5) * cellH);
  }

  const stepGrid = computeStepGrid(curBar);

  // Flash physical role button LEDs on the bottom strip when notes trigger
  const roleButtons = $('roleGrid').children;
  for (let r = 0; r < 8; r++) {
    const btn = roleButtons[r];
    if (!btn) continue;
    const activeCell = stepGrid[r][curStep];
    const led = btn.querySelector('.led');
    if (led) {
      const lit = posAt.playing && activeCell.level > 0.05;
      led.style.opacity = lit ? '1' : '0.35';
      led.style.transform = lit ? 'scaleX(1.25)' : 'scaleX(1)';
    }
  }

  const now = performance.now();
  const hudActive = hudOverlay && now < hudOverlay.until;

  // Draw 16×8 discrete RGB LED cells inside a milled eggcrate baffle
  for (let r = 0; r < 8; r++) {
    const baseRgb = ROLES[r].color;
    for (let c = 0; c < 16; c++) {
      const x = leftPad + c * cellW;
      const y = topPad + r * cellH;
      const padX = Math.max(1.5, cellW * 0.11);
      const padY = Math.max(1.5, cellH * 0.12);
      const pw = cellW - padX * 2;
      const ph = cellH - padY * 2;

      // Dark recessed LED cavity
      g.fillStyle = '#0c0f16';
      g.fillRect(x + padX, y + padY, pw, ph);
      g.strokeStyle = '#181e2b';
      g.lineWidth = 1;
      g.strokeRect(x + padX, y + padY, pw, ph);

      let rgb = baseRgb;
      let alpha = 0.06;
      let isAccent = false;
      let isSlide = false;

      if (matrixMode === 'step') {
        const cell = stepGrid[r][c];
        if (cell.level > 0) {
          alpha = 0.25 + cell.level * 0.75;
          isAccent = cell.accent;
          isSlide = cell.slide;
        }
        if (posAt.playing && c === curStep) {
          if (cell.level > 0) {
            alpha = 1.0;
            isAccent = true;
          } else {
            rgb = [180, 200, 230];
            alpha = 0.16;
          }
        }
      } else if (matrixMode === 'song' && song?.sections?.length) {
        const secIdx = Math.min(song.sections.length - 1, Math.floor((c / 16) * song.sections.length));
        const sec = song.sections[secIdx];
        const layer = song.layers.find((l) => l.role === ROLES[r].key);
        const clip = layer?.clips?.find((cl) => cl.bar <= sec.bar && cl.bar + cl.bars > sec.bar);
        if (clip && !muted.has(ROLES[r].key)) {
          const markAlpha = { x: 0.9, v: 0.75, r: 0.85, s: 0.45, f: 0.3 }[clip.mark] ?? 0.7;
          alpha = markAlpha;
          if (clip.mark === 'f') rgb = [140, 100, 255];
          if (clip.mark === 'r') rgb = [255, 210, 60];
        }
        if (secIdx === posAt.sec && posAt.playing) {
          alpha = Math.min(1, alpha + 0.22);
          isAccent = clip != null;
        }
      } else if (matrixMode === 'dna') {
        const vals = [
          (dna.swing - 50) / 16,
          dna.ext,
          dna.mode / 4,
          dna.percDensity,
          dna.leadDensity / 5.5,
          (dna.bpm - 76) / 102,
          dna.pump / 0.8,
          dna.gated / 0.9,
          mix.tape / 0.9,
          mix.reverb / 1.8,
          synthState.cutoff / 12000,
          synthState.res / 0.92,
          synthState.envMod / 1.5,
          synthState.decay / 1.8,
          dna.arp ? 0.9 : 0.15,
          dna.tomFills ? 0.9 : 0.25,
        ];
        const v = vals[c] ?? 0.5;
        const barRow = 7 - r;
        if (barRow / 7 <= v) {
          alpha = 0.45 + 0.55 * (barRow / 7);
          rgb = c < 6 ? [26, 255, 122] : c < 10 ? [255, 123, 51] : [58, 168, 255];
        }
      }

      // Momentary 3×5 Pixel Font HUD Overlay when twisting a knob
      if (hudActive) {
        alpha *= 0.22; // dim background pattern
        if (r >= 1 && r <= 5) {
          const charIdx = Math.floor(c / 3);
          const bitCol = c % 3;
          const ch = hudOverlay.text[charIdx] || ' ';
          const fontRows = PIXEL_FONT[ch] || PIXEL_FONT[' '];
          const rowBits = fontRows[r - 1] || 0;
          if (bitCol < 3 && ((rowBits >> (2 - bitCol)) & 1)) {
            rgb = hudOverlay.color;
            alpha = 1.0;
            isAccent = true;
          }
        } else if (r === 7) {
          const barCols = Math.round(hudOverlay.norm * 16);
          if (c < barCols) {
            rgb = hudOverlay.color;
            alpha = 0.95;
          }
        }
      }

      // Draw glowing LED diffuser lens
      g.fillStyle = `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha.toFixed(3)})`;
      g.fillRect(x + padX + 1, y + padY + 1, pw - 2, ph - 2);

      // Specular hot core for active notes / accents
      if (alpha > 0.55) {
        g.fillStyle = isAccent
          ? 'rgba(255, 255, 255, 0.85)'
          : `rgba(255, 255, 255, ${(alpha * 0.38).toFixed(2)})`;
        g.fillRect(x + padX + pw * 0.25, y + padY + ph * 0.25, pw * 0.5, ph * 0.5);
      }

      // 303 Slide tie bridge between this LED and the previous LED
      if (isSlide && !hudActive && c > 0) {
        g.fillStyle = 'rgba(58, 255, 220, 0.85)';
        g.fillRect(x - padX * 1.2, y + cellH * 0.38, padX * 2.4, cellH * 0.24);
      }
    }
  }

  // Rightmost 2 columns: Discrete Stereo Output Peak & Glue Gain Reduction LEDs
  let peak = 0;
  if (analyser && posAt.playing) {
    analyser.getFloatTimeDomainData(timeBuf);
    for (let i = 0; i < timeBuf.length; i++) peak = Math.max(peak, Math.abs(timeBuf[i]));
  }
  const grDb = Math.max(0, -20 * Math.log10(Math.max(0.01, lastGr)));
  const grNorm = Math.min(1, grDb / 8.0);
  const meterX = leftPad + gridW + W * 0.015;
  const meterW = (W - meterX - W * 0.015) / 2;

  for (let r = 0; r < 8; r++) {
    const y = topPad + r * cellH + cellH * 0.14;
    const h = cellH * 0.72;
    const lvlIdx = 7 - r;
    const outLit = peak * 1.35 >= (lvlIdx + 0.5) / 8;
    const grLit = grNorm >= (r + 0.5) / 8;

    // Output level LED (green -> amber -> red)
    const outCol = lvlIdx >= 7 ? '255,46,110' : lvlIdx >= 5 ? '255,202,58' : '26,255,122';
    g.fillStyle = `rgba(${outCol}, ${outLit ? 0.95 : 0.1})`;
    g.fillRect(meterX, y, meterW - 2, h);

    // Glue compression gain-reduction LED (top-down amber/cyan)
    g.fillStyle = `rgba(58, 168, 255, ${grLit ? 0.95 : 0.1})`;
    g.fillRect(meterX + meterW + 1, y, meterW - 2, h);
  }
}

// ── Boot ──
$('seedVal').textContent = String(seed);
rebuildSong(false);
buildRolePads();
buildJamPads();
requestAnimationFrame(renderMatrix);

window.groovebox = {
  PROFILES,
  get genre() { return genre; },
  get seed() { return seed; },
  get dna() { return dna; },
  get song() { return song; },
  get playing() { return playing; },
  get matrixMode() { return matrixMode; },
  startAt,
  stopPlayback,
  rebuildSong,
};
