// RP2350 Hardware Groovebox UI: full-screen landscape controller for the
// rust-games music engine (crates/music, music.wasm). Shares the live
// AudioContext, AudioWorkletNode, and current song state with the Music Lab
// player (main.js) so switching between Player and Groovebox is instantaneous
// with zero audio hiccup.

const $ = (id) => document.getElementById(id);

// ── Musical Roles (8 Rows of the RGB LED Matrix & Channel Strip) ──
const ROLES = [
  { key: 'K',  name: 'k',      label: 'KICK',  short: 'K',  color: [255, 46, 110],  hex: '#ff2e6e', drum: 'kick',  classicLanes: ['kick'] },
  { key: 'C',  name: 'c',      label: 'CLAP',  short: 'C',  color: [255, 123, 51],  hex: '#ff7b33', drum: 'snare', classicLanes: ['snare', 'clap'] },
  { key: 'H',  name: 'h',      label: 'HATS',  short: 'H',  color: [255, 202, 58],  hex: '#ffca3a', drum: 'hat',   classicLanes: ['hat', 'ohat'] },
  { key: 'P',  name: 'p',      label: 'PERC',  short: 'P',  color: [184, 242, 58],  hex: '#b8f23a', drum: 'tomM',  classicLanes: ['rim', 'tomL', 'tomM', 'tomH', 'cowbell', 'shaker', 'perc'] },
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
const MODE_FULL_NAMES = ['Phrygian', 'Aeolian', 'Dorian', 'Mixolydian', 'Ionian'];
const MODE_SCALES = [
  [0, 1, 3, 5, 7, 8, 10, 12], // Phrygian
  [0, 2, 3, 5, 7, 8, 10, 12], // Aeolian
  [0, 2, 3, 5, 7, 9, 10, 12], // Dorian
  [0, 2, 4, 5, 7, 9, 10, 12], // Mixolydian
  [0, 2, 4, 5, 7, 9, 11, 12], // Ionian
];
const NOTE_NAMES = ['C', 'C#', 'D', 'Eb', 'E', 'F', 'F#', 'G', 'Ab', 'A', 'Bb', 'B'];
const DRUM_PAD_LANES = [
  ['kick', 'KICK'], ['snare', 'SNARE'], ['clap', 'CLAP'], ['hat', 'HAT'],
  ['ohat', 'OHAT'], ['rim', 'RIM'], ['tomM', 'TOM'], ['crash', 'CRASH'],
];

const CLASSIC_TO_PROFILE = {
  house: 'chicago', techno: 'detroit', trance: 'uplifting', eurobeat: 'outrun',
  psytrance: 'acid', dnb: 'liquid', garage: 'deep', chicha: 'synthwave', country: 'orchestral',
};

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

export function parseModeFromStyle(style) {
  if (!style) return null;
  const keyPart = style.split('·')[1]?.trim() || '';
  const modeWord = keyPart.split(/\s+/)[1];
  const idx = MODE_FULL_NAMES.indexOf(modeWord);
  return idx >= 0 ? idx : null;
}

export function parseTonicFromStyle(style) {
  if (!style) return 9;
  const keyPart = style.split('·')[1]?.trim() || '';
  const noteWord = keyPart.split(/\s+/)[0]?.replace(/m$/, '');
  const idx = NOTE_NAMES.indexOf(noteWord);
  return idx >= 0 ? idx : 9;
}

// Builds a Song-compatible view from a classic Track so the 16×8 LED Matrix,
// Role Mutes/Focus/Solo, and Synth Sculptor work seamlessly on classic tracks too.
function classicTrackToGrooveboxSong(track, genre, seed) {
  let barAt = 0;
  const sections = (track.sections || []).map((s) => {
    const sec = { name: s.drums || 'sec', bar: barAt, bars: s.bars, energy: s.drop ? 1.0 : 0.7 };
    barAt += s.bars;
    return sec;
  });
  const totalBars = barAt || 64;
  const layers = [];
  const drumPats = track.drums || {};
  const drumPatNames = Object.keys(drumPats);

  for (const r of ROLES.filter((x) => x.drum)) {
    const patterns = drumPatNames.map((patName) => {
      const lanesObj = drumPats[patName] || {};
      const notes = [];
      let maxLen = 16;
      for (const lane of r.classicLanes || [r.drum]) {
        const raw = lanesObj[lane];
        if (!raw) continue;
        const chars = [...raw.replace(/\s+/g, '')];
        maxLen = Math.max(maxLen, chars.length);
        for (let i = 0; i < chars.length; i++) {
          const ch = chars[i];
          if (ch === 'X' || ch === 'x' || ch === 'o') {
            notes.push({
              at: i,
              len: 1,
              vel: ch === 'X' ? 1.0 : ch === 'x' ? 0.8 : 0.42,
              accent: ch === 'X',
              lane,
            });
          }
        }
      }
      return { name: patName, len: maxLen, notes };
    });
    const clips = [];
    sections.forEach((sec, idx) => {
      const dName = track.sections[idx]?.drums;
      const patIdx = drumPatNames.indexOf(dName);
      if (patIdx >= 0 && patterns[patIdx]?.notes.length > 0) {
        clips.push({ bar: sec.bar, bars: sec.bars, patIdx, pattern: dName, mark: 'x' });
      }
    });
    layers.push({
      name: r.drum,
      role: r.key,
      sound: track.kitName || 'tr909',
      muteNames: r.classicLanes,
      patterns,
      clips,
    });
  }

  const TYPE_ROLE = { bass: 'B', chord: 'Pd', arp: 'A', mel: 'L' };
  const usedRoles = new Set();
  for (const [partName, part] of Object.entries(track.parts || {})) {
    let role = TYPE_ROLE[part.type] || 'L';
    if (usedRoles.has(role) && role === 'Pd') role = 'A';
    usedRoles.add(role);
    const patEntries = Object.entries(part.pat || {});
    const patNames = patEntries.map(([k]) => k);
    const patterns = patEntries.map(([patName, patStr]) => {
      const notes = [];
      let len = 16;
      if (part.type === 'bass') {
        const chars = [...String(patStr).replace(/\s+/g, '')];
        len = Math.max(16, chars.length);
        for (let i = 0; i < chars.length; i++) {
          const c = chars[i];
          if ('-~.'.includes(c)) continue;
          let nLen = 1;
          let slide = false;
          while (i + nLen < chars.length && (chars[i + nLen] === '-' || chars[i + nLen] === '~')) {
            if (chars[i + nLen] === '~') slide = true;
            nLen++;
          }
          const accent = c !== c.toLowerCase();
          notes.push({ at: i, len: nLen, vel: accent ? 0.95 : 0.78, accent, slide });
        }
      } else if (part.type === 'chord') {
        const chars = [...String(patStr).replace(/\s+/g, '')];
        len = Math.max(16, chars.length);
        for (let i = 0; i < chars.length; i++) {
          const c = chars[i];
          if (c !== 'x' && c !== 'X') continue;
          let nLen = 1;
          while (i + nLen < chars.length && chars[i + nLen] === '-') nLen++;
          notes.push({ at: i, len: nLen, vel: c === 'X' ? 0.95 : 0.78, accent: c === 'X' });
        }
      } else {
        const toks = String(patStr).trim().split(/\s+/).filter(Boolean);
        const res = part.res || 1;
        len = Math.max(16, toks.length * res);
        for (let i = 0; i < toks.length; i++) {
          const t = toks[i];
          if (t === '_' || t === '.') continue;
          let nLen = 1;
          while (i + nLen < toks.length && toks[i + nLen] === '_') nLen++;
          const accent = t.endsWith('!');
          notes.push({ at: i * res, len: nLen * res, vel: accent ? 0.95 : 0.8, accent });
        }
      }
      return { name: patName, len, notes };
    });
    const clips = [];
    sections.forEach((sec, idx) => {
      const pKey = track.sections[idx]?.p?.[partName];
      const patIdx = patNames.indexOf(pKey);
      if (patIdx >= 0) {
        clips.push({ bar: sec.bar, bars: sec.bars, patIdx, pattern: pKey, mark: 'x' });
      }
    });
    layers.push({
      name: partName,
      role,
      sound: `${part.lab?.kind || 'synth'} · ${part.lab?.name || ''}`,
      lab: part.lab,
      muteNames: [partName],
      patterns,
      clips,
    });
  }

  return {
    title: track.title || `${genre} ${seed}`,
    style: track.style || genre,
    genre,
    seed,
    bpm: track.bpm || 120,
    swing: Math.round(50 + (track.swing || 0) * 50),
    bars: totalBars,
    sections,
    layers,
    fx: [],
    notes: [],
  };
}

export function mountGroovebox(app) {
  const { PROFILES, NEW, CLASSIC } = app;
  let genre = 'synthwave';
  let seed = 1;
  let dna = {
    bpm: 110, swing: 54, mode: 1, bass: 'eighths', chords: 'sustain',
    ext: 0.3, arp: true, leadDensity: 2.8, percDensity: 0.5,
    tomFills: true, gated: 0.5, pump: 0.45,
  };
  let song = null;
  let looping = false;
  let loopRange = null;
  let matrixMode = 'step'; // 'step' | 'song' | 'dna'
  let roleAction = 'mute'; // 'mute' | 'focus' | 'solo'
  let focusedRole = 'B';
  let jamMode = 'synth'; // 'synth' | 'drums'
  let jamOctave = 4;
  let hudOverlay = null;
  let morphTimer = null;

  const setLcd = (msg) => { $('lcdLine').textContent = msg; $('lcdLine').title = msg; };
  function showHud(label, norm, color = [58, 168, 255]) {
    hudOverlay = { text: label.toUpperCase().slice(0, 6), norm: Math.max(0, Math.min(1, norm)), color, until: performance.now() + 750 };
  }

  // ── Populate Genre Preset Selector (Both New Generator & Classic Radio Genres) ──
  $('genreSel').innerHTML = '';
  for (const [groupLabel, dict] of [['NEW GENERATOR', NEW], ['CLASSIC RADIO', CLASSIC]]) {
    const grp = document.createElement('optgroup');
    grp.label = groupLabel;
    for (const [k, name] of Object.entries(dict)) {
      grp.append(new Option(name.toUpperCase(), k));
    }
    $('genreSel').append(grp);
  }

  function profileFor(gKey) {
    return PROFILES[gKey] || PROFILES[CLASSIC_TO_PROFILE[gKey]] || PROFILES.synthwave;
  }

  function syncDnaFromCur(S) {
    const p = profileFor(S.genre);
    const ov = S.dnaOverrides || {};
    if (S.song) {
      const drawnMode = parseModeFromStyle(S.song.style) ?? p.mode;
      dna = {
        bpm: ov.bpm ?? Math.round(S.song.bpm),
        swing: ov.swing ?? Math.round(S.song.swing ?? p.swing),
        mode: ov.mode ?? drawnMode,
        bass: ov.bass ?? p.bass,
        chords: ov.chords ?? p.chords,
        ext: ov.ext ?? Number(p.ext.toFixed(2)),
        arp: ov.arp ?? !!p.arp,
        leadDensity: ov.leadDensity ?? Number(p.leadDensity.toFixed(1)),
        percDensity: ov.percDensity ?? Number(p.percDensity.toFixed(2)),
        tomFills: ov.tomFills ?? !!p.tomFills,
        gated: ov.gated ?? Number(p.gated.toFixed(2)),
        pump: ov.pump ?? Number(p.pump.toFixed(2)),
      };
      const hasCustomDna = Object.keys(ov).some((k) => !['bpm', 'key'].includes(k));
      $('dnaBadge').textContent = hasCustomDna ? 'Custom DNA' : p.name;
    } else {
      const T = S.track;
      dna = {
        bpm: Math.round(T.bpm || p.bpm),
        swing: Math.round(50 + (T.swing || 0) * 50),
        mode: parseModeFromStyle(T.style) ?? p.mode,
        bass: T.parts?.bass?.lab?.kind === 'tb303' ? 'acid' : p.bass,
        chords: p.chords,
        ext: Number(p.ext.toFixed(2)),
        arp: !!T.parts?.arp,
        leadDensity: T.parts?.lead ? Number(p.leadDensity.toFixed(1)) || 3.0 : 0.0,
        percDensity: Number(p.percDensity.toFixed(2)),
        tomFills: !!p.tomFills,
        gated: 0.0,
        pump: Number((T.pump?.depth ?? p.pump).toFixed(2)),
      };
      $('dnaBadge').textContent = `${CLASSIC[S.genre] || S.genre} Classic`;
    }
  }

  function scheduleMorph(field, value, badgeText) {
    $('dnaBadge').textContent = badgeText || 'Custom DNA';
    app.setDnaOverride(field, value);
    if (morphTimer) clearTimeout(morphTimer);
    morphTimer = setTimeout(() => {
      morphTimer = null;
      app.rebuildCurrentSong(true);
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
        scheduleMorph('bass', b.key, `Bass: ${b.label}`);
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
    scheduleMorph('chords', dna.chords, `Chords: ${dna.chords.toUpperCase()}`);
  };
  $('arpBtn').onclick = () => {
    dna.arp = !dna.arp;
    syncSwitchesUI();
    showHud(dna.arp ? 'ARP ON' : 'ARPOFF', dna.arp ? 1 : 0, [164, 120, 255]);
    scheduleMorph('arp', dna.arp, `Arp: ${dna.arp ? 'ON' : 'OFF'}`);
  };
  $('fillsBtn').onclick = () => {
    dna.tomFills = !dna.tomFills;
    syncSwitchesUI();
    showHud(dna.tomFills ? '80STOM' : 'HSETURN', dna.tomFills ? 1 : 0, [255, 202, 58]);
    scheduleMorph('tomFills', dna.tomFills, `Fills: ${dna.tomFills ? '80s Toms' : 'House'}`);
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
      set: (v) => { dna.swing = v; scheduleMorph('swing', v, `Swing ${v}%`); },
      fmt: (v) => `${v}%`,
      hud: (v) => `SWG ${v}`,
    },
    {
      id: 'ext', label: 'HARMONY', min: 0, max: 1, step: 0.05, color: '#3aa8ff', rgb: [58, 168, 255],
      get: () => dna.ext,
      set: (v) => { dna.ext = v; scheduleMorph('ext', v, `Harmony ${Math.round(v * 100)}%`); },
      fmt: (v) => v < 0.2 ? 'TRIAD' : v < 0.65 ? '7THS' : '9TH/11',
      hud: (v) => `EXT ${Math.round(v * 99)}`,
    },
    {
      id: 'mode', label: 'MODE', min: 0, max: 4, step: 1, color: '#a478ff', rgb: [164, 120, 255],
      get: () => dna.mode,
      set: (v) => { dna.mode = v; buildJamPads(); scheduleMorph('mode', v, `Mode ${MODE_NAMES[v]}`); },
      fmt: (v) => MODE_NAMES[v] || 'AEOL',
      hud: (v) => MODE_NAMES[v] || 'AEOL',
    },
    {
      id: 'percDensity', label: 'TOP PERC', min: 0, max: 1, step: 0.05, color: '#b8f23a', rgb: [184, 242, 58],
      get: () => dna.percDensity,
      set: (v) => { dna.percDensity = v; scheduleMorph('percDensity', v, `Perc ${Math.round(v * 100)}%`); },
      fmt: (v) => `${Math.round(v * 100)}%`,
      hud: (v) => `PRC ${Math.round(v * 99)}`,
    },
    {
      id: 'leadDensity', label: 'LEAD HOOK', min: 0, max: 5.5, step: 0.5, color: '#ff4fe3', rgb: [255, 79, 227],
      get: () => dna.leadDensity,
      set: (v) => { dna.leadDensity = v; scheduleMorph('leadDensity', v, v === 0 ? 'Lead OFF' : `Lead ${v}/bar`); },
      fmt: (v) => v <= 0.05 ? 'OFF' : `${v.toFixed(1)}/B`,
      hud: (v) => v <= 0.05 ? 'LD OFF' : `LD ${v.toFixed(1)}`,
    },
    {
      id: 'bpm', label: 'TEMPO', min: 76, max: 178, step: 1, color: '#1aff7a', rgb: [26, 255, 122],
      get: () => dna.bpm,
      set: (v) => { dna.bpm = v; scheduleMorph('bpm', v, `${v} BPM`); },
      fmt: (v) => `${v} BPM`,
      hud: (v) => `BPM${v}`,
    },
  ];

  // ── Build Era & Bus Physics Knobs (Right Module Top) ──
  const BUS_KNOB_DEFS = [
    {
      id: 'pump', label: 'PUMP', min: 0, max: 0.8, step: 0.02, color: '#ff2e6e', rgb: [255, 46, 110],
      get: () => dna.pump,
      set: (v) => { dna.pump = v; scheduleMorph('pump', v, `Pump ${Math.round(v * 100)}%`); },
      fmt: (v) => `${Math.round(v * 100)}%`,
      hud: (v) => `PMP ${Math.round(v * 99)}`,
    },
    {
      id: 'gated', label: '80S GATE', min: 0, max: 0.9, step: 0.05, color: '#ff7b33', rgb: [255, 123, 51],
      get: () => dna.gated,
      set: (v) => { dna.gated = v; scheduleMorph('gated', v, `Gated ${Math.round(v * 100)}%`); },
      fmt: (v) => v === 0 ? 'DRY' : `${Math.round(v * 100)}%`,
      hud: (v) => `GAT ${Math.round(v * 99)}`,
    },
    {
      id: 'tape', label: 'TAPE SAT', min: 0, max: 0.9, step: 0.02, color: '#ffca3a', rgb: [255, 202, 58],
      get: () => app.getMix().tape,
      set: (v) => app.setMixParam('tape', v),
      fmt: (v) => `${Math.round(v * 100)}%`,
      hud: (v) => `TAP ${Math.round(v * 99)}`,
    },
    {
      id: 'reverb', label: 'HALL REV', min: 0, max: 1.8, step: 0.05, color: '#3aa8ff', rgb: [58, 168, 255],
      get: () => app.getMix().reverb,
      set: (v) => app.setMixParam('reverb', v),
      fmt: (v) => `${Math.round(v * 100)}%`,
      hud: (v) => `REV ${Math.round(v * 55)}`,
    },
  ];

  // ── Build Active Synth Sculptor Knobs (Right Module Bottom) ──
  const synthState = { cutoff: 1800, res: 0.25, envMod: 0.5, decay: 0.35 };

  function getFocusedLayer() {
    if (!song?.layers) return null;
    const roleInfo = ROLES.find((r) => r.key === focusedRole);
    return song.layers.find((l) => (l.role === focusedRole || l.name === roleInfo?.name) && l.lab)
      || song.layers.find((l) => l.lab);
  }

  function syncSynthKnobsToFocused(updateAudition = false) {
    const l = getFocusedLayer();
    if (!l || !l.lab) {
      $('synthBadge').textContent = 'NONE';
      return;
    }
    const p = l.lab;
    $('synthBadge').textContent = `${l.name.toUpperCase()} · ${p.name || p.kind}`;
    synthState.cutoff = p.cutoff ?? p.brightness ?? 2200;
    synthState.res = p.res ?? p.feedback ?? 0.25;
    synthState.envMod = p.envMod ?? p.fenv ?? p.velIndex ?? p.drive ?? 0.45;
    synthState.decay = p.decay ?? p.fd ?? p.d ?? 0.35;
    syncAllKnobsUI();
    if (updateAudition) {
      app.post({ type: 'audition', lab: structuredClone(p) });
    }
  }

  function applyFocusedSynthEdit(mutate) {
    const l = getFocusedLayer();
    if (!l || !l.lab) return;
    mutate(l.lab);
    app.onPatchEdit(l.name, structuredClone(l.lab));
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
          else if (p.kind === 'tb303') p.env = v;
          else { p.envMod = v; p.fenv = v; }
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

  buildBassSwitchRow();
  for (const d of DNA_KNOB_DEFS) createKnob($('dnaKnobs'), d);
  for (const d of BUS_KNOB_DEFS) createKnob($('busKnobs'), d);
  for (const d of SYNTH_KNOB_DEFS) createKnob($('synthKnobs'), d);

  // ── Bottom Left: 8 Illuminated Role Channel Keys (Shared Mute/Solo with Player) ──
  function roleMuteNames(roleKey) {
    if (!song?.layers) return [];
    const matching = song.layers.filter((l) => l.role === roleKey);
    const names = [];
    for (const l of matching) {
      if (l.muteNames) names.push(...l.muteNames);
      else if (l.name) names.push(l.name);
    }
    return names;
  }

  function isRoleMuted(roleKey) {
    const names = roleMuteNames(roleKey);
    return names.length > 0 && names.every((n) => app.mute.has(n));
  }

  function isRoleSoloed(roleKey) {
    const s = app.getSolo();
    if (!s) return false;
    return roleMuteNames(roleKey).includes(s);
  }

  function buildRolePads() {
    const el = $('roleGrid');
    el.innerHTML = '';
    for (const r of ROLES) {
      const muted = isRoleMuted(r.key);
      const soloed = isRoleSoloed(r.key);
      const b = document.createElement('button');
      b.className = 'role-pad'
        + (muted ? ' muted' : '')
        + (focusedRole === r.key ? ' focused' : '')
        + (soloed ? ' soloed' : '');
      b.style.setProperty('--c', r.hex);
      b.dataset.role = r.key;
      b.innerHTML = `<i class="led"></i><span>${r.short}</span><small>${r.label}</small>`;
      b.onclick = () => {
        if (roleAction === 'focus') {
          if (r.synth) {
            focusedRole = r.key;
            syncSynthKnobsToFocused(true);
          } else if (r.drum) {
            triggerDrumHit(r.drum);
          }
        } else if (roleAction === 'solo') {
          const names = roleMuteNames(r.key);
          const target = names[0] || null;
          app.setSolo(isRoleSoloed(r.key) ? null : target);
          app.onMutesChanged();
        } else {
          const names = roleMuteNames(r.key);
          if (isRoleMuted(r.key)) {
            for (const n of names) app.mute.delete(n);
          } else {
            for (const n of names) app.mute.add(n);
          }
          app.onMutesChanged();
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
    app.mute.clear();
    app.setSolo(null);
    app.onMutesChanged();
    buildRolePads();
  };

  // ── Bottom Right: 8 Scale-Locked Jam / Drum Pads ──
  function songTonicPc() {
    return parseTonicFromStyle(song?.style);
  }

  async function triggerSynthNoteOn(midi, padEl) {
    await app.ensureAudio();
    await app.getCtx().resume();
    const l = getFocusedLayer();
    if (l?.lab) app.post({ type: 'audition', lab: l.lab });
    app.post({ type: 'on', midi, vel: 0.92, accent: true });
    padEl?.classList.add('down');
  }
  function triggerSynthNoteOff(midi, padEl) {
    app.post({ type: 'off', midi });
    padEl?.classList.remove('down');
  }
  async function triggerDrumHit(lane, padEl) {
    await app.ensureAudio();
    await app.getCtx().resume();
    app.post({ type: 'hit', lane, vel: 0.95 });
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

  // ── Top Bezel Controls (Back, Genre, Seed, Dice, Reset, Loop, Play, Fullscreen) ──
  $('backLab').onclick = (e) => {
    e.preventDefault();
    app.closeGroovebox();
  };
  $('genreSel').onchange = () => {
    genre = $('genreSel').value;
    app.onPresetChange(genre, seed);
    showHud(genre.slice(0, 6), 1.0, [26, 255, 122]);
  };
  $('seedBtn').onclick = () => {
    const next = prompt('Enter song seed (1–99999):', String(seed));
    if (next != null && Number(next) >= 1) {
      seed = Math.floor(Number(next));
      $('seedVal').textContent = String(seed);
      app.onPresetChange(genre, seed);
    }
  };
  $('diceBtn').onclick = () => {
    seed = 1 + Math.floor(Math.random() * 99999);
    $('seedVal').textContent = String(seed);
    app.onPresetChange(genre, seed);
    showHud(`SD${seed}`, (seed % 100) / 100, [255, 202, 58]);
  };
  $('resetDnaBtn').onclick = () => {
    app.onResetProfile();
    showHud('RESET', 0.5, [58, 168, 255]);
  };
  $('loopBtn').onclick = () => {
    looping = !looping;
    $('loopBtn').classList.toggle('on', looping);
    const posAt = app.getPosAt();
    if (looping) {
      const b0 = Math.floor((posAt?.barIndex || 0) / 4) * 4;
      loopRange = [b0, b0 + 4];
      if (app.getCur()?.song) app.post({ type: 'loop', start: b0, end: b0 + 4 });
      showHud(`LP ${b0 + 1}`, 1.0, [255, 202, 58]);
    } else {
      loopRange = null;
      if (app.getCur()?.song) app.post({ type: 'loop' });
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
  $('playBtn').onclick = async () => {
    if (!app.getPlaying()) {
      const posAt = app.getPosAt();
      const startBar = posAt?.barIndex > 0 ? posAt.barIndex : 0;
      await app.startAt(startBar);
    } else {
      app.stopAll();
    }
  };

  for (const id of ['modeStep', 'modeSong', 'modeDna']) {
    $(id).onclick = () => {
      matrixMode = $(id).dataset.mode;
      for (const m of ['modeStep', 'modeSong', 'modeDna']) $(m).classList.toggle('on', m === id);
    };
  }

  // ── 16×8 Discrete RGB LED Pixel Matrix Renderer ──
  const canvas = $('ledMatrix');
  const g = canvas.getContext('2d');

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
      await app.startAt(targetBar);
      showHud(`BAR${targetBar + 1}`, col / 15, [26, 255, 122]);
    } else {
      const r = ROLES[row];
      if (r.synth) {
        focusedRole = r.key;
        syncSynthKnobsToFocused(true);
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
    const grid = Array.from({ length: 8 }, () =>
      Array.from({ length: 16 }, () => ({ level: 0, accent: false, slide: false, sustain: false }))
    );
    if (!song?.layers) return grid;

    ROLES.forEach((roleInfo, rIdx) => {
      if (isRoleMuted(roleInfo.key) || (app.getSolo() && !isRoleSoloed(roleInfo.key))) return;
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
    if (!document.body.classList.contains('groovebox-open')) return;
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

    const posAt = app.getPosAt() || { bar: 0, barIndex: 0, step: 0, sec: 0, playing: false, t: performance.now() };
    const isPlaying = app.getPlaying() && posAt.playing;

    let curBar = posAt.barIndex || 0;
    let curStep = posAt.step || 0;
    if (isPlaying && song?.bpm) {
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

    g.font = `bold ${Math.max(8, Math.floor(H * 0.036))}px ui-monospace, monospace`;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    for (let c = 0; c < 16; c++) {
      const isBeat = c % 4 === 0;
      g.fillStyle = (isPlaying && c === curStep && matrixMode === 'step')
        ? '#ffffff'
        : isBeat ? '#8d99ae' : '#434c5e';
      g.fillText(String(c + 1), leftPad + (c + 0.5) * cellW, topPad * 0.5);
    }

    for (let r = 0; r < 8; r++) {
      const role = ROLES[r];
      g.fillStyle = isRoleMuted(role.key) ? '#3a4150' : role.hex;
      g.fillText(role.short, leftPad * 0.48, topPad + (r + 0.5) * cellH);
    }

    const stepGrid = computeStepGrid(curBar);

    const roleButtons = $('roleGrid').children;
    for (let r = 0; r < 8; r++) {
      const btn = roleButtons[r];
      if (!btn) continue;
      const activeCell = stepGrid[r][curStep];
      const led = btn.querySelector('.led');
      if (led) {
        const lit = isPlaying && activeCell.level > 0.05;
        led.style.opacity = lit ? '1' : '0.35';
        led.style.transform = lit ? 'scaleX(1.25)' : 'scaleX(1)';
      }
    }

    const now = performance.now();
    const hudActive = hudOverlay && now < hudOverlay.until;
    const mix = app.getMix();

    for (let r = 0; r < 8; r++) {
      const baseRgb = ROLES[r].color;
      for (let c = 0; c < 16; c++) {
        const x = leftPad + c * cellW;
        const y = topPad + r * cellH;
        const padX = Math.max(1.5, cellW * 0.11);
        const padY = Math.max(1.5, cellH * 0.12);
        const pw = cellW - padX * 2;
        const ph = cellH - padY * 2;

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
          if (isPlaying && c === curStep) {
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
          if (clip && !isRoleMuted(ROLES[r].key)) {
            const markAlpha = { x: 0.9, v: 0.75, r: 0.85, s: 0.45, f: 0.3 }[clip.mark] ?? 0.7;
            alpha = markAlpha;
            if (clip.mark === 'f') rgb = [140, 100, 255];
            if (clip.mark === 'r') rgb = [255, 210, 60];
          }
          if (secIdx === posAt.sec && isPlaying) {
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

        if (hudActive) {
          alpha *= 0.22;
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

        g.fillStyle = `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha.toFixed(3)})`;
        g.fillRect(x + padX + 1, y + padY + 1, pw - 2, ph - 2);

        if (alpha > 0.55) {
          g.fillStyle = isAccent
            ? 'rgba(255, 255, 255, 0.85)'
            : `rgba(255, 255, 255, ${(alpha * 0.38).toFixed(2)})`;
          g.fillRect(x + padX + pw * 0.25, y + padY + ph * 0.25, pw * 0.5, ph * 0.5);
        }

        if (isSlide && !hudActive && c > 0) {
          g.fillStyle = 'rgba(58, 255, 220, 0.85)';
          g.fillRect(x - padX * 1.2, y + cellH * 0.38, padX * 2.4, cellH * 0.24);
        }
      }
    }

    let peak = 0;
    const analyser = app.getAnalyser();
    if (analyser && isPlaying) {
      analyser.getFloatTimeDomainData(timeBuf);
      for (let i = 0; i < timeBuf.length; i++) peak = Math.max(peak, Math.abs(timeBuf[i]));
    }
    const { gr: lastGr } = app.getGrLoad();
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

      const outCol = lvlIdx >= 7 ? '255,46,110' : lvlIdx >= 5 ? '255,202,58' : '26,255,122';
      g.fillStyle = `rgba(${outCol}, ${outLit ? 0.95 : 0.1})`;
      g.fillRect(meterX, y, meterW - 2, h);

      g.fillStyle = `rgba(58, 168, 255, ${grLit ? 0.95 : 0.1})`;
      g.fillRect(meterX + meterW + 1, y, meterW - 2, h);
    }
  }

  function syncFromCur() {
    const S = app.getCur();
    if (!S) return;
    genre = S.genre;
    seed = S.seed;
    $('genreSel').value = genre;
    $('seedVal').textContent = String(seed);
    $('backLab').href = `index.html?genre=${encodeURIComponent(genre)}&seed=${seed}`;
    song = S.song || classicTrackToGrooveboxSong(S.track, S.genre, S.seed);
    syncDnaFromCur(S);
    syncSwitchesUI();
    syncAllKnobsUI();
    syncSynthKnobsToFocused(false);
    buildRolePads();
    buildJamPads();
    syncTransportUI();
    if (song) {
      setLcd(`${song.style} · ${song.bpm} BPM · SWG ${song.swing ?? dna.swing}%`);
    }
  }

  function syncTransportUI() {
    const isPlaying = app.getPlaying();
    $('playLbl').textContent = isPlaying ? '■ STOP' : '▶ PLAY';
    $('playBtn').classList.toggle('on', isPlaying);
  }

  function onPos(p, gr, load) {
    if (looping && loopRange && !app.getCur()?.song && p.playing && p.barIndex >= loopRange[1]) {
      app.post({ type: 'play', bar: loopRange[0], delay: 0 });
    }
    const secName = song?.sections?.[p.sec]?.name?.toUpperCase() || 'SECT';
    $('posBadge').textContent = `${secName} · BAR ${p.barIndex + 1} · ${p.step + 1}`;
    const grDb = gr < 0.999 ? Math.max(-99.9, 20 * Math.log10(gr)).toFixed(1) : '0.0';
    $('grBadge').textContent = `GLUE ${grDb} dB`;
    if (load != null) $('cpuChip').textContent = `DSP ${Math.min(999, Math.round(load * 100))}%`;
  }

  requestAnimationFrame(renderMatrix);

  const api = {
    PROFILES,
    get genre() { return genre; },
    get seed() { return seed; },
    get dna() { return dna; },
    get song() { return song; },
    get playing() { return app.getPlaying(); },
    get matrixMode() { return matrixMode; },
    get ctx() { return app.getCtx(); },
    get lab() { return app.getLab(); },
    get pos() { return app.getPosAt(); },
    startAt: (bar) => app.startAt(bar),
    stopPlayback: () => app.stopAll(),
    rebuildSong: (morph = true) => app.rebuildCurrentSong(morph),
    syncFromCur,
    syncTransportUI,
    syncAllKnobsUI,
    syncSynthKnobsToFocused,
    buildRolePads,
    onPos,
  };
  window.groovebox = api;
  return api;
}
