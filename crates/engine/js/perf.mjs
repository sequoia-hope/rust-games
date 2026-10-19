// Frame time, CPU time per frame, memory, load time and reloads of the web
// build in headless Chrome on the dev machine's GPU.
//
//   cargo xtask web --release
//   node js/perf.mjs [--backend webgpu|webgl2] [--secs N] [--reloads 10] [--reload-levels a,b]
//       [--query "level=x&t=0.2"]   (the page's parameters)
//       [--phone] [--throttle N] [--trace N] [--tag name] [--dist next]
//       [--size WxH --dpr N] [--headed] [--angle gl|vulkan] [--capped]
//       [--global __game] [--url|--port]
//   node js/perf.mjs --run --wait "<js>" [--pos "<js>"] ...   a scenario of the game's own
//
// Default mode: the page's own measurement (`index.html?perf=1`, the kit's
// template/web/perf.js: the page the owner opens on the phone), reading
// the status global's `perf` when it is done. `--run`: a scenario instead
// (a race on the autopilot, an attract mode): the page is opened with
// `--query`, timing starts when the JS expression `--wait` is truthy and
// runs for `--secs` (default 30); `--pos` is an expression for a position
// sample per frame (the per-km table). In both, `G` in an expression is
// the status global.
//
// The page is loaded through the registered dev server (`server.mjs`:
// `--url`, `--port`, `$PORT`, `proj port`; no default). Frame rate is
// uncapped (no vsync, no frame-rate limit) unless `--capped`. `--backend
// webgl2` also turns WebGPU off in Chrome, so the page must fall back by
// itself.
//
// CPU time per frame: frame intervals alone say little on a shared machine
// or when the GPU sets the pace, so the page is also timed from inside:
// every requestAnimationFrame callback is wrapped before the page's own
// scripts run, and the time spent in the callbacks of one frame is that
// frame's work (Bevy does all of a frame's work in requestAnimationFrame).
// `--trace N` also records N seconds of Chrome's trace in the middle and
// gives the renderer main thread's CPU time per frame (thread time, so
// time descheduled by other load is not counted), the GPU process's, and
// GC. The load average is sampled every two seconds through the run.
//
//   --phone          a phone in landscape (844 x 390 CSS px at dpr 3, touch, mobile)
//   --throttle N     CPU throttling (Emulation.setCPUThrottlingRate), N x slower
//   --trace N        N seconds of Chrome tracing from a quarter into the recording
//   --tag name       a suffix for the result file
//   --dist name      the build in dist/<name>/ (default next)
//   --out dir        where the JSON result goes (default report/perf/ under the root)
//
// Prints the page's report and a JSON summary; writes the full result to
// <out>/<tag or level>-<backend>.json.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { argv, baseUrl, chromeFlags, isMain, launch, projectRoot, sleep, statusGlobal } from './server.mjs';

// Installed before the page's scripts: times every requestAnimationFrame
// callback and sums the callbacks that share a frame's timestamp.
const CPU_TIMER = `(() => {
  const raf = window.requestAnimationFrame.bind(window);
  const T = [], MS = [];
  let curT = -1;
  window.__cpuFrames = { t: T, ms: MS };
  window.requestAnimationFrame = (cb) => raf((ts) => {
    const a = performance.now();
    try { return cb(ts); } finally {
      const d = performance.now() - a;
      if (ts === curT) MS[MS.length - 1] += d; else { curT = ts; T.push(a); MS.push(d); }
    }
  });
})();`;

// CPU statistics of the frames between t0 and t1 (the page's clock).
async function cpuStats(page, t0, t1) {
  const ms = await page.evaluate((a, b) => {
    const f = window.__cpuFrames;
    const out = [];
    for (let i = 0; i < f.t.length; i++) if (f.t[i] >= a && f.t[i] <= b) out.push(f.ms[i]);
    return out;
  }, t0, t1);
  const o = Float64Array.from(ms).sort();
  const q = (p) => Math.round(o[Math.min(o.length - 1, Math.floor((p / 100) * o.length))] * 100) / 100;
  const mean = ms.reduce((x, y) => x + y, 0) / Math.max(1, ms.length);
  return { frames: ms.length, p50: q(50), p90: q(90), p95: q(95), p99: q(99), max: q(100), mean: Math.round(mean * 100) / 100 };
}

// Chrome's trace for `secs` seconds: the renderer main thread's CPU time
// (thread duration of its top-level tasks) and wall time per frame, the
// part of it inside requestAnimationFrame callbacks, GC, and the GPU
// process's main thread.
export async function traceFor(cdp, secs) {
  // Events are aggregated as they arrive (a whole trace of a page at
  // hundreds of frames a second does not fit in one string).
  const per = new Map();
  const names = new Map();
  const counts = new Map();
  const th = (k) => {
    let t = per.get(k);
    if (!t) per.set(k, (t = { taskCpu: 0, taskWall: 0, rafCpu: 0, rafWall: 0, callbacks: 0, frames: 0, gc: 0 }));
    return t;
  };
  const onData = ({ value }) => {
    for (const e of value) {
      const k = `${e.pid}:${e.tid}`;
      if (e.ph === 'M' && e.name === 'thread_name') { names.set(k, e.args.name); continue; }
      if (e.ph !== 'X') continue;
      counts.set(e.name, (counts.get(e.name) || 0) + 1);
      const t = th(k);
      if (e.name === 'ThreadControllerImpl::RunTask') { t.taskCpu += e.tdur || 0; t.taskWall += e.dur || 0; }
      else if (e.name === 'FireAnimationFrame') { t.rafCpu += e.tdur || 0; t.rafWall += e.dur || 0; t.callbacks++; }
      else if (e.name === 'PageAnimator::serviceScriptedAnimations') t.frames++;
      else if (e.name === 'MajorGC' || e.name === 'MinorGC') t.gc += e.tdur || 0;
    }
  };
  cdp.on('Tracing.dataCollected', onData);
  const done = new Promise((r) => cdp.once('Tracing.tracingComplete', r));
  await cdp.send('Tracing.start', { categories: 'toplevel,devtools.timeline', transferMode: 'ReportEvents' });
  await sleep(secs * 1000);
  await cdp.send('Tracing.end');
  await done;
  cdp.off('Tracing.dataCollected', onData);
  // The page's main thread: the CrRendererMain with the most rAF callbacks.
  const main = [...per].filter(([k]) => names.get(k) === 'CrRendererMain').sort((x, y) => y[1].callbacks - x[1].callbacks)[0];
  if (!main) return { error: 'no main thread in the trace' };
  const t = main[1];
  const n = t.frames || t.callbacks;
  const r2 = (x) => Math.round(x * 100) / 100;
  const gpu = [...per].filter(([k]) => names.get(k) === 'CrGpuMain');
  return {
    seconds: secs, frames: n, rafCallbacks: t.callbacks,
    cpuPerFrameMs: r2(t.taskCpu / 1000 / n), wallPerFrameMs: r2(t.taskWall / 1000 / n),
    rafCpuPerFrameMs: r2(t.rafCpu / 1000 / n), rafWallPerFrameMs: r2(t.rafWall / 1000 / n),
    gcCpuPerFrameMs: r2(t.gc / 1000 / n),
    mainBusyShare: r2(t.taskWall / 1000 / (secs * 1000)),
    gpuCpuPerFrameMs: r2(gpu.reduce((a, [, x]) => a + x.taskCpu, 0) / 1000 / n),
    gpuBusyShare: r2(gpu.reduce((a, [, x]) => a + x.taskWall, 0) / 1000 / (secs * 1000)),
    allCpuPerFrameMs: r2([...per].reduce((a, [, x]) => a + x.taskCpu, 0) / 1000 / n),
    topEvents: [...counts].sort((x, y) => y[1] - x[1]).slice(0, 8),
  };
}

// The same statistics the page's perf.js computes, for a scenario run.
export function summarise(dts, at, ss) {
  const sorted = Float64Array.from(dts).sort();
  const pct = (p) => sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))];
  const windows = [];
  let n = 0, sum = 0;
  for (const dt of dts) { n++; sum += dt; if (sum >= 500) { windows.push((1000 * n) / sum); n = 0; sum = 0; } }
  windows.sort((a, b) => a - b);
  const w = (p) => windows[Math.min(windows.length - 1, Math.floor((p / 100) * windows.length))];
  const r1 = (x) => Math.round(x * 10) / 10;
  return {
    seconds: r1(at[at.length - 1] || 0), frames: dts.length, fpsMedian: r1(w(50)), fps5: r1(w(5)),
    p50: r1(pct(50)), p90: r1(pct(90)), p95: r1(pct(95)), p99: r1(pct(99)), p999: r1(pct(99.9)), max: r1(sorted[sorted.length - 1]),
    over50First30: dts.filter((d, i) => d > 50 && at[i] <= 30).length, over50: dts.filter((d) => d > 50).length,
    over33: dts.filter((d) => d > 33.4).length,
    slow: dts.map((d, i) => ({ ms: r1(d), t: r1(at[i]), s: Math.round(ss[i]) })).filter((x) => x.ms > 50).slice(0, 20),
    km: [...dts.reduce((m, d, i) => {
      const k = Math.floor(ss[i] / 1000);
      if (!m.has(k)) m.set(k, []);
      m.get(k).push(d);
      return m;
    }, new Map())].map(([km, v]) => {
      const o = Float64Array.from(v).sort();
      const q = (p) => r1(o[Math.min(o.length - 1, Math.floor((p / 100) * o.length))]);
      return { km, frames: v.length, p50: q(50), p95: q(95), max: r1(o[o.length - 1]) };
    }),
  };
}

/// `defaults`: a game's own defaults for the options (query, wait, pos,
/// run, out, label). Returns the result (also written to disk).
export async function run({ args = process.argv.slice(2), defaults = {}, root = projectRoot() } = {}) {
  const { opt, flag } = argv(args);
  const G = statusGlobal(opt);
  const backend = opt('--backend', defaults.backend || 'webgpu');
  const secs = opt('--secs', defaults.secs ?? null);
  const scenario = flag('--run') || !!defaults.run;
  const reloads = opt('--reloads', defaults.reloads ?? '10');
  const reloadLevels = opt('--reload-levels', defaults.reloadLevels ?? null);
  const capped = flag('--capped');
  const timeoutS = Number(opt('--timeout', 1800));
  const phone = flag('--phone');
  const throttle = Number(opt('--throttle', 0));
  const traceS = Number(opt('--trace', 0));
  const tag = opt('--tag', defaults.tag ?? null);
  const dist = opt('--dist', defaults.dist || 'next');
  const headed = flag('--headed');
  const angle = opt('--angle', 'vulkan');
  const query = new URLSearchParams(defaults.query || '');
  for (const [k, v] of new URLSearchParams(opt('--query', ''))) query.set(k, v);
  // Scenario expressions, with `G` the status global.
  const withG = (expr) => `((G) => (${expr}))(window[${JSON.stringify(G)}])`;
  const waitExpr = opt('--wait', defaults.wait || '(G?.frames || 0) > 60');
  const posExpr = opt('--pos', defaults.pos || '0');
  const label = opt('--label', defaults.label || query.get('level') || 'perf');

  const base = baseUrl(opt, root);
  const flags = chromeFlags({ backend, capped, gesture: true, angle });
  const browser = await launch(flags, { headed, timeoutS, root });
  const errors = [];
  let result = null;
  const load = os.loadavg().map((x) => x.toFixed(1));
  const loadSamples = [];
  let loadSampler = null;

  try {
    const page = await browser.newPage();
    if (phone) {
      await page.setViewport({ width: 844, height: 390, deviceScaleFactor: 3, isMobile: true, hasTouch: true, isLandscape: true });
    } else {
      const [w, h] = opt('--size', '1280x800').split('x').map(Number);
      await page.setViewport({ width: w, height: h, deviceScaleFactor: Number(opt('--dpr', 1)) });
    }
    const cdp = await page.createCDPSession();
    if (throttle > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: throttle });
    await page.evaluateOnNewDocument(CPU_TIMER);
    loadSampler = setInterval(() => loadSamples.push(os.loadavg()[0]), 2000);
    page.on('console', (m) => {
      const t = m.text();
      if (m.type() === 'error' && !/Failed to load resource/.test(t)) errors.push(t);
      if (process.env.PK_VERBOSE) console.log(`[page ${m.type()}] ${t}`);
    });
    page.on('pageerror', (e) => errors.push(String(e)));
    const t0 = Date.now();
    if (scenario) {
      const runS = Number(secs) || 30;
      if (backend === 'webgl2') query.set('backend', 'webgl2');
      const url = `${base}dist/${dist}/index.html?${query}`;
      console.error(`${label} ${backend}: ${url}  (load average ${load.join(' ')})`);
      await page.goto(url);
      await page.waitForFunction(withG(waitExpr), { timeout: timeoutS * 1000, polling: 200 });
      const t0Page = await page.evaluate(() => performance.now());
      const traced = traceS ? (async () => { await sleep((runS * 1000) / 4); return traceFor(cdp, Math.min(traceS, runS / 2)); })() : null;
      result = await page.evaluate(async (runSecs, posSrc, g) => {
        const dts = [], at = [], ss = [];
        const pos = new Function('G', `return (${posSrc});`);
        let last = null, t0 = null, on = true;
        const frame = (now) => {
          if (!on) return;
          requestAnimationFrame(frame);
          if (t0 === null) t0 = now;
          if (last !== null) { dts.push(now - last); at.push((now - t0) / 1000); ss.push(Number(pos(window[g])) || 0); }
          last = now;
        };
        requestAnimationFrame(frame);
        await new Promise((r) => setTimeout(r, runSecs * 1000));
        on = false;
        return { dts, at, ss, backend: window[g]?.backend ?? null };
      }, runS, posExpr, G);
      result.cpu = await cpuStats(page, t0Page, await page.evaluate(() => performance.now()));
      if (traced) result.trace = await traced;
      result.flight = summarise(result.dts, result.at, result.ss);
      result.device = { backend: result.backend };
      delete result.dts; delete result.at; delete result.ss;
    } else {
      query.set('perf', '1');
      if (secs) query.set('secs', secs);
      query.set('reloads', reloads);
      if (reloadLevels) query.set('reloadLevels', reloadLevels);
      if (backend === 'webgl2') query.set('backend', 'webgl2');
      const url = `${base}dist/${dist}/index.html?${query}`;
      console.error(`${label} ${backend}: ${url}  (load average ${load.join(' ')})`);
      await page.goto(url);
      let last = '';
      let traced = null, flightSeen = null;
      for (;;) {
        const p = await page.evaluate((g) => ({ perf: window[g]?.perf, state: window[g]?.state, error: window[g]?.error }), G);
        if (p.error || p.state === 'failed' || p.state === 'nogpu') throw new Error(`client ${p.state}: ${p.error || ''}`);
        if (p.perf?.phase === 'flight' && flightSeen === null) flightSeen = Date.now();
        if (traceS && !traced && flightSeen !== null && p.perf?.phase === 'flight' && Date.now() - flightSeen > (p.perf.secs * 1000) / 4) {
          traced = await traceFor(cdp, Math.min(traceS, p.perf.secs / 2));
        }
        if (p.perf?.phase === 'done') {
          result = p.perf.result;
          result.cpu = await cpuStats(page, ...result.flight.window);
          if (traced) result.trace = traced;
          break;
        }
        const now = JSON.stringify(p.perf || p.state);
        if (now !== last && process.env.PK_VERBOSE) console.error(now);
        last = now;
        if (Date.now() - t0 > timeoutS * 1000) throw new Error(`not done after ${timeoutS} s (${now})`);
        await sleep(1000);
      }
    }
  } catch (e) {
    errors.push(String(e.message || e));
  } finally {
    clearInterval(loadSampler);
    await browser.close();
  }

  if (result) {
    result.loadAverage = load;
    result.loadAverageAfter = os.loadavg().map((x) => x.toFixed(1));
    const ls = loadSamples.length ? loadSamples : [os.loadavg()[0]];
    result.loadDuring = { min: +Math.min(...ls).toFixed(1), mean: +(ls.reduce((a, b) => a + b, 0) / ls.length).toFixed(1), max: +Math.max(...ls).toFixed(1) };
    result.setup = { phone, throttle: throttle || 1, capped, scenario, query: query.toString() };
    const out = path.resolve(root, opt('--out', defaults.out || 'report/perf'));
    fs.mkdirSync(out, { recursive: true });
    const file = path.join(out, `${label}-${backend}${scenario ? '-run' : ''}${tag ? '-' + tag : ''}.json`);
    fs.writeFileSync(file, JSON.stringify(result, null, 1));
    if (result.text) console.log(result.text);
    const f = result.flight;
    console.log(JSON.stringify({
      label, backend: result.device?.backend,
      firstFrameS: result.load?.firstFrameMs ? +(result.load.firstFrameMs / 1000).toFixed(2) : null,
      readyS: result.load?.readyMs ? +(result.load.readyMs / 1000).toFixed(2) : null,
      cpu: result.cpu, trace: result.trace ?? null, loadDuring: result.loadDuring, setup: result.setup,
      flight: { ...f, km: undefined, slow: undefined }, wasmPeakMB: result.verdict?.wasmPeakMB ?? null,
      reloadsWasmMB: result.reloads?.map((x) => x.wasmMB) ?? null,
      load: `${result.loadAverage.join(' ')} → ${result.loadAverageAfter.join(' ')}`,
    }));
    console.error(`written ${file}`);
  }
  if (errors.length) {
    console.error(`errors:\n  ${errors.join('\n  ')}`);
    process.exitCode = 1;
  }
  return result;
}

if (isMain(import.meta.url)) await run();
