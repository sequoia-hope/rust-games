// Frame stutter (docs/vision/peninsula.md, stage 6a; DECISIONS D1238): the
// open world driven on the real GPU in headless Chrome, its frame times
// held to budgets, and every long frame explained by the work the client
// did around it (`window.__mp.frameWork`, one entry a frame from
// `crate::frame_work`: the stream's events, the entities it spawned and
// dropped, the bytes it made for the GPU, its time on the main thread, the
// pipelines compiling).
//
// The scenarios: the menu while the level views are prepared behind it
// and then flying (D1237); a cruise on the loop on autodrive across
// section boundaries; a turn off Highway 1 onto Tunitas Creek Road (the
// ring crossing a junction), steered with the keyboard along the
// corridors; an event started from the cruise (hold gas and brake); and,
// only when asked for (`MP_STUTTER_ONLY=long`, eight minutes), a long
// drive up El Camino from San Mateo through the squares to San Francisco,
// held to the same budgets and to not getting worse as it goes (D1252).
//
//   cargo xtask web --release && node --test tools/parity/e2e/stutter.test.mjs
//   (MP_BACKEND=webgl2 for the WebGL2 build, MP_DEVICE=phone for phone
//    emulation, MP_STUTTER_SECS=<s> for the cruise's length (default 120),
//    MP_STUTTER_LONG_SECS=<s> for the long drive's (default 480),
//    MP_STUTTER_ONLY=menu,cruise,branch,event,long to pick scenarios)
//
// The budgets are per backend and device, from baselines measured on the
// owner's machine (D1238); the load average is printed with each, since
// the machine is shared. A frame is a spike past SPIKE_MS; each spike is
// printed with its cause.

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { ROOT } from '../lib/jstree.mjs';
import { launch, openGame, sleep } from './harness.mjs';

const BACKEND = process.env.MP_BACKEND || 'webgpu';
const DEVICE = process.env.MP_DEVICE || 'desktop';
const SECS = Number(process.env.MP_STUTTER_SECS || 120);
const LONG_SECS = Number(process.env.MP_STUTTER_LONG_SECS || 480);
const ONLY = process.env.MP_STUTTER_ONLY ? process.env.MP_STUTTER_ONLY.split(',') : null;
const backendQ = process.env.MP_BACKEND ? `backend=${process.env.MP_BACKEND}&` : '';
const SPIKE_MS = 50;

// Budgets. A long frame with the client's work recorded around it (the
// stream's, the assets added, the pipelines compiling) is the client's
// stutter and is held hard: `work` is the longest such frame allowed. A
// long frame with nothing recorded is the machine (it is shared: other
// builds, the GPU's other clients) or work this suite does not see, and
// is held to an allowance: `other` ms at most, `other50PerMin` frames over
// 50 ms a minute. `p95` and `over33PerMin` are the overall smoothness.
// `null`: reported, not held. Measured on the owner's machine (D1238).
const HARD = { p95: 20, over33PerMin: 10, work: 50, other: 150, other50PerMin: 2 };
const BUDGETS = {
  'webgpu/desktop': { menuHolding: null, menuFlying: HARD, cruise: HARD, branch: HARD, event: HARD, long: HARD },
  'webgl2/desktop': { menuHolding: null, menuFlying: HARD, cruise: HARD, branch: HARD, event: HARD, long: HARD },
};
BUDGETS['webgpu/phone'] = BUDGETS['webgpu/desktop'];
BUDGETS['webgl2/phone'] = BUDGETS['webgl2/desktop'];
const budgets = BUDGETS[`${BACKEND}/${DEVICE}`] ?? BUDGETS[`${BACKEND}/desktop`];

let browser;
before(async () => { browser = await launch(); });
after(async () => { await browser?.close(); });

const want = (name) => !ONLY || ONLY.includes(name);
const load = () => os.loadavg().map((x) => x.toFixed(1)).join(' ');

// ── Recording ───────────────────────────────────────────────────────────

/// Starts the page's rAF recorder: each frame's delta with the client's
/// frame number (`__mp.frames`), and empties `__mp.frameWork`.
async function record(game) {
  await game.eval(() => {
    window.__mp.frameWork?.splice(0);
    if (window.__st) { window.__st.s = []; return; }
    window.__st = { s: [] };
    let last = null;
    const f = (now) => {
      const G = window.__mp;
      if (last !== null) window.__st.s.push([now - last, G.frames | 0, G.holding ? 1 : 0]);
      last = now;
      requestAnimationFrame(f);
    };
    requestAnimationFrame(f);
  });
}

/// What the page recorded since the last call: [dt, frame, holding] per
/// rAF, and the client's work entries by frame.
function drain(game) {
  return game.eval(() => ({ s: window.__st.s.splice(0), w: (window.__mp.frameWork || []).splice(0) }));
}

class Tape {
  constructor() { this.s = []; this.w = new Map(); }
  add({ s, w }) {
    this.s.push(...s);
    for (const e of w) {
      const o = this.w.get(e.f);
      if (!o) { this.w.set(e.f, e); continue; }
      for (const [k, v] of Object.entries(e)) {
        if (k === 'f') continue;
        if (k === 'events') o.events = [...(o.events || []), ...v];
        else if (k === 'pipelines') o.pipelines = Math.max(o.pipelines || 0, v);
        else o[k] = (o[k] || 0) + v;
      }
    }
  }
  /// The samples matching `keep` (by holding), as a tape of their own.
  filter(keep) { const t = new Tape(); t.s = this.s.filter(keep); t.w = this.w; return t; }
}

async function pump(game, tape) { tape.add(await drain(game)); }

// ── Analysis ────────────────────────────────────────────────────────────

const q = (a, p) => (a.length ? a[Math.min(a.length - 1, Math.floor(p * a.length))] : 0);

/// The work the client did in frames `f0..=f1`, merged, with the cause
/// of a long frame in words.
function workIn(tape, f0, f1) {
  const m = { events: [] };
  for (let f = f0; f <= f1; f++) {
    const e = tape.w.get(f);
    if (!e) continue;
    for (const [k, v] of Object.entries(e)) {
      if (k === 'f') continue;
      if (k === 'events') m.events.push(...v);
      else if (k === 'pipelines') m.pipelines = Math.max(m.pipelines || 0, v);
      else m[k] = (m[k] || 0) + v;
    }
  }
  return m;
}

function cause(m) {
  const c = [];
  if (m.pipelines) c.push(`${m.pipelines} pipeline(s) compiling`);
  if (m.unpackMs >= 3) c.push(`a worker's section read back in ${m.unpackMs.toFixed(0)} ms`);
  if (m.stepMs >= 3) c.push(`a section built here, ${m.stepMs.toFixed(0)} ms`);
  if (m.spawnMs >= 3 || m.spawned) {
    c.push(`spawning ${(m.spawnMs || 0).toFixed(0)} ms (${m.spawned || 0} entities, meshes ${m.meshKB || 0} kB, images ${m.imageKB || 0} kB, instances ${m.instanceKB || 0} kB)`);
  } else if (m.meshKB || m.imageKB) {
    c.push(`assets added: meshes ${m.meshKB || 0} kB, images ${m.imageKB || 0} kB`);
  }
  if (m.despawned) c.push(`${m.despawned} entities dropped`);
  if (m.streamMs >= 3 && !c.length) c.push(`the stream's systems ${m.streamMs.toFixed(0)} ms`);
  const ev = m.events.filter((e) => !e.startsWith('ring'));
  if (ev.length) c.push(`stream: ${ev.join(', ')}`);
  return c.length ? c.join('; ') : 'no stream work recorded (the GPU, a GC, or the race itself)';
}

/// Frame stats and the spikes with their causes.
function analyse(tape) {
  const s = tape.s;
  const dts = s.map((x) => x[0]).sort((a, b) => a - b);
  const total = s.reduce((a, x) => a + x[0], 0);
  const stats = {
    n: s.length, secs: +(total / 1000).toFixed(1),
    p50: +q(dts, 0.5).toFixed(1), p95: +q(dts, 0.95).toFixed(1), max: +(dts.at(-1) ?? 0).toFixed(1),
    over33: dts.filter((x) => x > 33.4).length, over50: dts.filter((x) => x > 50).length,
  };
  const spikes = [];
  for (let i = 0; i < s.length; i++) {
    if (s[i][0] <= SPIKE_MS) continue;
    const f1 = s[i][1];
    const f0 = (i > 0 ? s[i - 1][1] : f1) - 1;
    const m = workIn(tape, f0, f1);
    const work = !!(m.pipelines || m.unpackMs >= 3 || m.stepMs >= 3 || m.spawnMs >= 3 || m.spawned || m.meshKB || m.imageKB
      || m.despawned || m.streamMs >= 3 || m.events.some((e) => !e.startsWith('ring')));
    spikes.push({ ms: +s[i][0].toFixed(0), frame: f1, work, cause: cause(m) });
  }
  return { stats, spikes };
}

function print(label, { stats, spikes }) {
  console.log(`${label} [${BACKEND}, ${DEVICE}, load ${load()}]: ${stats.n} frames in ${stats.secs} s, `
    + `p50 ${stats.p50} ms, p95 ${stats.p95} ms, max ${stats.max} ms, over 33 ms ${stats.over33}, over 50 ms ${stats.over50} `
    + `(${spikes.filter((sp) => sp.work).length} with the client's work around them)`);
  const top = [...spikes].sort((a, b) => b.ms - a.ms).slice(0, 20);
  for (const sp of top.sort((a, b) => a.frame - b.frame)) console.log(`  frame ${sp.frame}: ${sp.ms} ms: ${sp.cause}`);
  if (spikes.length > top.length) console.log(`  (${spikes.length - top.length} more spikes)`);
}

/// The budget's breaches, each with the offending frames.
function breaches(label, r, b) {
  if (!b) return [];
  const { stats, spikes } = r;
  const mins = Math.max(stats.secs / 60, 1 / 6);
  const out = [];
  if (stats.p95 > b.p95) out.push(`p95 ${stats.p95} ms over ${b.p95} ms`);
  if (stats.over33 > b.over33PerMin * mins) out.push(`${stats.over33} frames over 33 ms (budget ${(b.over33PerMin * mins).toFixed(1)})`);
  const work = spikes.filter((sp) => sp.work && sp.ms > b.work);
  if (work.length) out.push(`${work.length} frames over ${b.work} ms with the client's work around them`);
  const other = spikes.filter((sp) => !sp.work);
  const worst = Math.max(0, ...other.map((sp) => sp.ms));
  if (worst > b.other) out.push(`a frame of ${worst} ms with no work recorded (budget ${b.other} ms)`);
  if (other.length > b.other50PerMin * mins) out.push(`${other.length} frames over ${SPIKE_MS} ms with no work recorded (budget ${(b.other50PerMin * mins).toFixed(1)})`);
  if (!out.length) return [];
  const list = [...spikes].sort((a, b2) => (b2.work - a.work) || (b2.ms - a.ms)).slice(0, 12).map((sp) => `    frame ${sp.frame}: ${sp.ms} ms: ${sp.cause}`);
  return [`${label} [${BACKEND}, ${DEVICE}]: ${out.join(', ')}\n${list.join('\n')}`];
}

// ── Driving ─────────────────────────────────────────────────────────────

function corridor(id) {
  const txt = fs.readFileSync(path.join(ROOT, `assets/atlas/peninsula/corridors/${id}.txt`), 'utf8');
  return txt.split('\n').filter((l) => l && !l.startsWith('#')).map((l) => l.split(/\s+/).slice(0, 2).map(Number));
}

const car = (game) => game.eval(() => {
  const r = window.__mp.race;
  return r ? { x: r.x, z: r.z, yaw: r.yaw, s: r.s, speed: r.speed, road: r.road, state: r.state, event: r.event, hold: r.hold, time: r.time } : null;
});

/// Steers the car along `route` ([x, z] points) with the arrow keys at
/// `speed(i)` m/s, until `done(car, i)` or `secs`; pumps the tape every
/// half second.
async function follow(game, tape, route, { speed, done, secs, log = 10 }) {
  const held = new Set();
  const key = async (k, on) => {
    if (on && !held.has(k)) { held.add(k); await game.page.keyboard.down(k); }
    if (!on && held.has(k)) { held.delete(k); await game.page.keyboard.up(k); }
  };
  let at = 0, lastPump = Date.now(), lastLog = Date.now(), c = null;
  const t0 = Date.now();
  while (Date.now() - t0 < secs * 1000) {
    c = await car(game);
    if (!c) { await sleep(50); continue; }
    let bi = at, bd = Infinity;
    for (let i = Math.max(0, at - 5); i < Math.min(route.length, at + 40); i++) {
      const d = (route[i][0] - c.x) ** 2 + (route[i][1] - c.z) ** 2;
      if (d < bd) { bd = d; bi = i; }
    }
    at = bi;
    if (done(c, at)) break;
    const aim = route[Math.min(route.length - 1, at + 5)];
    let a = Math.atan2(aim[1] - c.z, aim[0] - c.x) - c.yaw;
    a = Math.atan2(Math.sin(a), Math.cos(a));
    const v = speed(at);
    await key('ArrowRight', a > 0.04);
    await key('ArrowLeft', a < -0.04);
    await key('ArrowUp', c.speed < v);
    await key('ArrowDown', c.speed > v + 3);
    if (Date.now() - lastPump > 500) { await pump(game, tape); lastPump = Date.now(); }
    if (Date.now() - lastLog > log * 1000) {
      lastLog = Date.now();
      console.log(`    t ${((Date.now() - t0) / 1000).toFixed(0)} s: road ${c.road} s ${c.s.toFixed(0)} speed ${c.speed.toFixed(1)}, route ${at}/${route.length}, ${await sections(game)}`);
    }
    await sleep(30);
  }
  for (const k of [...held]) await key(k, false);
  await pump(game, tape);
  return { car: c, at };
}

const sections = (game) => game.eval(() => {
  const st = window.__mp.stream;
  if (!st) return 'no stream';
  return `ring at ${st.at}: ` + Object.entries(st.sections).map(([k, v]) => (v === 'up' ? k : `${k} ${v}`)).join(' ');
});

/// Waits until every section the stream holds is up and nothing has
/// compiled or spawned for a second (after a jump of the car).
async function settle(game, timeout = 120000) {
  const t0 = Date.now();
  let quietSince = null;
  while (Date.now() - t0 < timeout) {
    const r = await game.eval(() => {
      const st = window.__mp.stream;
      const w = (window.__mp.frameWork || []).splice(0);
      const busy = w.some((e) => e.spawned || e.pipelines || e.events?.length);
      const up = !!st && Object.values(st.sections).every((v) => v === 'up' || v === 'failed');
      return { busy, up };
    });
    if (r.up && !r.busy) { quietSince ??= Date.now(); if (Date.now() - quietSince > 1000) return; } else quietSince = null;
    await sleep(200);
  }
  throw new Error(`the stream did not settle: ${await sections(game)}`);
}

const CRUISE = 'level=peninsula&autostart=sports&cruise=1&seed=1&pursuit=0&hq=0';

async function openCruise(query) {
  const game = await openGame(browser, { device: DEVICE, query: backendQ + CRUISE + query });
  await game.waitFor(() => (window.__mp?.race?.time ?? -1) >= 0.5, { timeout: 240000, interval: 200, what: 'the cruise to start' });
  return game;
}

/// Holds the scenario to its budget; collects the breaches.
const failures = [];
function judge(label, tape, budget) {
  const r = analyse(tape);
  print(label, r);
  failures.push(...breaches(label, r, budget));
  return r;
}

// ── Scenarios ───────────────────────────────────────────────────────────

test('the menu: holds still while the views are prepared, then flies smoothly', { skip: !want('menu') }, async () => {
  const game = await openGame(browser, { device: DEVICE, query: backendQ.replace(/&$/, '') });
  try {
    const tape = new Tape();
    await record(game);
    const t0 = Date.now();
    while (await game.eval(() => window.__mp.holding !== false)) {
      if (Date.now() - t0 > 240000) throw new Error('the menu held for 4 minutes');
      await sleep(500);
      await pump(game, tape);
    }
    console.log(`menu: held ${((Date.now() - t0) / 1000).toFixed(1)} s after ready`);
    const flyFrom = tape.s.length;
    const t1 = Date.now();
    while (Date.now() - t1 < 15000) { await sleep(500); await pump(game, tape); }
    const holding = new Tape(); holding.s = tape.s.slice(0, flyFrom).concat(tape.s.slice(flyFrom).filter((x) => x[2])); holding.w = tape.w;
    const flying = new Tape(); flying.s = tape.s.slice(flyFrom).filter((x) => !x[2]); flying.w = tape.w;
    judge('menu while the views are prepared (the picture holds still)', holding, budgets.menuHolding);
    judge('menu flying', flying, budgets.menuFlying);
    assert.deepEqual(game.errors, []);
  } finally { await game.close(); }
});

test('a cruise round the loop on autodrive, across section boundaries', { skip: !want('cruise') }, async () => {
  const game = await openCruise('&autodrive=1');
  try {
    const tape = new Tape();
    const late0 = await game.eval(() => window.__mp.lateFrames | 0);
    await record(game);
    const c0 = await car(game);
    const t0 = Date.now();
    let lastLog = 0;
    while (Date.now() - t0 < SECS * 1000) {
      await sleep(500);
      await pump(game, tape);
      if (Date.now() - lastLog > 10000) {
        lastLog = Date.now();
        const c = await car(game);
        console.log(`    t ${((Date.now() - t0) / 1000).toFixed(0)} s: road ${c.road} s ${c.s.toFixed(0)} speed ${c.speed.toFixed(1)}, ${await sections(game)}`);
      }
    }
    const c1 = await car(game);
    const late = (await game.eval(() => window.__mp.lateFrames | 0)) - late0;
    const crossed = tape.s.length && [...tape.w.values()].flatMap((e) => e.events || []).filter((e) => e.startsWith('ring')).length;
    console.log(`cruise: s ${c0.s.toFixed(0)} to ${c1.s.toFixed(0)}, ${crossed} section boundaries, ${late} frames with pipelines compiling after the warm-up`);
    assert.ok(crossed >= 2, `the cruise crossed ${crossed} section boundaries`);
    judge(`cruise, ${SECS} s on autodrive`, tape, budgets.cruise);
    assert.deepEqual(game.errors, []);
  } finally { await game.close(); }
});

test('a turn off Highway 1 onto Tunitas Creek Road: the ring crosses the junction', { skip: !want('branch') }, async () => {
  const game = await openCruise('');
  try {
    // The junction: the place on the loop nearest Tunitas's Highway 1 end.
    const tun = corridor('tunitas');
    const mouth = await game.eval((p) => {
      const len = window.__mp.routeLength || 40000;
      let best = Infinity, bs = 0;
      for (let s = 0; s < len; s += 5) {
        const f = window.__mp.trackFrame(s);
        const d = (f[0] - p[0]) ** 2 + (f[2] - p[1]) ** 2;
        if (d < best) { best = d; bs = s; }
      }
      return { s: bs, d: Math.sqrt(best) };
    }, tun[0]);
    assert.ok(mouth.d < 30, `Tunitas's end is ${mouth.d.toFixed(0)} m from the loop`);
    const s0 = mouth.s - 700;
    await game.evalG((G, s) => G.stage({ cmd: 'reset', s, lat: 0 }), s0);
    await settle(game);
    const road0 = await game.eval((a, b) => {
      const out = [];
      for (let s = a; s < b; s += 5) { const f = window.__mp.trackFrame(window.__mp.trackWrap(s)); out.push([f[0], f[2]]); }
      return out;
    }, s0, mouth.s);
    const route = road0.concat(tun);
    const turn = road0.length;
    const tape = new Tape();
    const late0 = await game.eval(() => window.__mp.lateFrames | 0);
    await record(game);
    const r = await follow(game, tape, route, {
      speed: (i) => (Math.abs(i - turn) < 40 ? 12 : 22),
      done: (c, i) => i > turn + 300,
      secs: 200,
    });
    const late = (await game.eval(() => window.__mp.lateFrames | 0)) - late0;
    console.log(`branch: from s ${s0.toFixed(0)} on the loop to ${r.at - turn} points up Tunitas (road ${r.car?.road}), ${late} frames with pipelines compiling after the warm-up`);
    assert.equal(r.car?.road, 1, 'on Tunitas Creek Road (road 1)');
    judge('the turn onto Tunitas Creek Road', tape, budgets.branch);
    assert.deepEqual(game.errors, []);
  } finally { await game.close(); }
});

test('an event started from the cruise: hold gas and brake, the countdown, the race', { skip: !want('event') }, async () => {
  const game = await openCruise('');
  try {
    await settle(game);
    const tape = new Tape();
    await record(game);
    await game.page.keyboard.down('ArrowUp');
    await game.page.keyboard.down('ArrowDown');
    await sleep(1800);
    await game.page.keyboard.up('ArrowUp');
    await game.page.keyboard.up('ArrowDown');
    await pump(game, tape);
    await game.waitFor(() => window.__mp.race?.event != null, { timeout: 10000, what: 'the event to begin' });
    await game.waitFor(() => window.__mp.race?.state === 'racing', { timeout: 20000, what: 'the countdown to end' });
    await pump(game, tape);
    // The race: along the loop ahead.
    const c = await car(game);
    const route = await game.eval((a) => {
      const out = [];
      for (let s = a; s < a + 1500; s += 5) { const f = window.__mp.trackFrame(window.__mp.trackWrap(s)); out.push([f[0], f[2]]); }
      return out;
    }, c.s);
    await follow(game, tape, route, { speed: () => 25, done: (_, i) => i > route.length - 10, secs: 20 });
    judge('an event from the cruise (hold, countdown, 20 s of racing)', tape, budgets.event);
    assert.deepEqual(game.errors, []);
  } finally { await game.close(); }
});

test('a long drive up El Camino: the stream keeps up and nothing grows', { skip: !ONLY?.includes('long') }, async () => {
  const game = await openCruise('&start=san-mateo');
  try {
    await settle(game);
    // El Camino north from the car (the corridor runs south to north).
    const road = corridor('el-camino');
    const c0 = await car(game);
    let from = 0, best = Infinity;
    road.forEach(([x, z], i) => { const d = (x - c0.x) ** 2 + (z - c0.z) ** 2; if (d < best) { best = d; from = i; } });
    const route = road.slice(from);
    // What the world holds (`__mp.census`) and the wasm memory, a sample
    // every 30 s.
    const census = [];
    const sample = async (t) => census.push({ t, ...(await game.eval(() => ({
      ...(window.__mp.census || {}), wasmMB: Math.round((window.__mp.wasmMemoryBytes?.() || 0) / 1048576),
    }))) });
    const tape = new Tape();
    await record(game);
    await sample(0);
    const t0 = Date.now();
    // Driven 30 s at a time, each part's frames kept for the trend.
    const parts = [];
    let at = 0;
    while ((Date.now() - t0) / 1000 < LONG_SECS - 1 && at < route.length - 10) {
      const n0 = tape.s.length;
      const left = LONG_SECS - (Date.now() - t0) / 1000;
      const rest = route.slice(at);
      const r = await follow(game, tape, rest, { speed: () => 30, done: (_, i) => i >= rest.length - 10, secs: Math.min(30, left), log: 30 });
      at += r.at;
      parts.push(tape.s.slice(n0));
      await sample(Math.round((Date.now() - t0) / 1000));
    }
    const c1 = await car(game);
    console.log(`long drive: ${(Math.hypot(c1.x - c0.x, c1.z - c0.z) / 1000).toFixed(1)} km from (${c0.x.toFixed(0)}, ${c0.z.toFixed(0)}) to (${c1.x.toFixed(0)}, ${c1.z.toFixed(0)}) in ${((Date.now() - t0) / 1000).toFixed(0)} s`);
    for (const c of census) console.log(`    t ${c.t} s: ${Object.entries(c).filter(([k]) => k !== 't').map(([k, v]) => `${k} ${v}`).join(' ')}`);
    const res = judge(`a long drive, ${LONG_SECS} s up El Camino`, tape, budgets.long);
    // Not worse at the end: the last third's frames against the first's.
    const third = Math.max(1, Math.floor(parts.length / 3));
    const stats = (ps) => analyse(Object.assign(new Tape(), { s: ps.flat(), w: tape.w })).stats;
    const a = stats(parts.slice(0, third)), b = stats(parts.slice(-third));
    console.log(`long drive: first third p95 ${a.p95} ms, ${a.over33} over 33 ms in ${a.secs} s; last third p95 ${b.p95} ms, ${b.over33} over 33 ms in ${b.secs} s`);
    if (b.p95 > a.p95 + 3) failures.push(`long drive: p95 ${a.p95} ms in the first third, ${b.p95} ms in the last`);
    if (b.over33 / b.secs > (a.over33 + 5) / a.secs * 2) failures.push(`long drive: ${a.over33} frames over 33 ms in the first third, ${b.over33} in the last`);
    // Nothing the world holds grows without bound: what the second half
    // holds at most against the first half's (the squares differ, so a
    // margin), and the stream's own caches against fixed bounds.
    const half = census.slice(Math.floor(census.length / 2));
    const most = (list, k) => Math.max(0, ...list.map((c) => c[k] ?? 0));
    const first = census.slice(0, Math.floor(census.length / 2) || 1);
    for (const k of ['entities', 'meshes', 'materials', 'images']) {
      if (most(half, k) > most(first, k) * 1.6 + 50) failures.push(`long drive: ${k} grew from at most ${most(first, k)} to ${most(half, k)}`);
    }
    if (most(half, 'wasmMB') > most(first, 'wasmMB') + 128) failures.push(`long drive: the wasm memory grew from ${most(first, 'wasmMB')} MB to ${most(half, 'wasmMB')} MB`);
    if (most(census, 'imageCache') > 64) failures.push(`long drive: the image cache holds ${most(census, 'imageCache')} textures`);
    if (most(census, 'stashKB') > 64 << 10) failures.push(`long drive: the texture stash holds ${most(census, 'stashKB')} kB`);
    if (most(census, 'sections') > 25) failures.push(`long drive: the stream holds ${most(census, 'sections')} sections`);
    if (most(census, 'blockMap') >= 512) failures.push(`long drive: the block map is full (${most(census, 'blockMap')})`);
    assert.ok(res.stats.n > 0);
    assert.deepEqual(game.errors, []);
  } finally { await game.close(); }
});

test('the budgets hold', async () => {
  assert.ok(!failures.length, `stutter over budget:\n${failures.join('\n')}`);
});
