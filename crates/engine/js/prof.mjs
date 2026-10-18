// A CPU profile of the web build in headless Chrome, by function and by
// Bevy system.
//
//   cargo xtask web --release && cargo xtask prof
//   node js/prof.mjs [--backend webgpu|webgl2] [--query "level=x&autostart=1"]
//       [--wait "<js>"] [--pos "<js>"] [--phone | --size WxH] [--throttle N] [--secs N]
//       [--dist prof] [--tag name] [--global __game] [--url|--port]
//   node js/prof.mjs --from report/perf/prof-<label>-<backend>.cpuprofile
//       [--callers <regex>] [--depth N] [--top N]
//
// The build must keep its function names (`cargo xtask prof` writes
// dist/prof/; the live build is stripped and shows `wasm-function[1234]`).
// Chrome's sampling profiler (CDP `Profiler`, 250 µs) runs for `--secs`
// (default 10) once the JS expression `--wait` is truthy (`G` is the
// status global; default: 120 frames drawn), through the registered dev
// server (`server.mjs`). `--pos` samples a position before and after.
//
// Three reports from the samples:
//   self      the functions with the most samples of their own
//   inclusive the functions with the most samples under them, without the
//             schedule's and the glue's frames every sample passes through
//   systems   inclusive time by the nearest enclosing Bevy system: a sample
//             is attributed to the innermost `FunctionSystem<fn(...), (),
//             (), path::to::system>` frame above it (the system's name is
//             that last generic argument); samples under no system are
//             "(outside systems)": the browser, GC, wasm-bindgen glue
//   callers   with `--callers <regex>`: the paths (callers' chains, `--depth`
//             frames) that reach the matching functions, by inclusive time
//
// `--phone` is a phone in landscape with the CPU throttled 4x; `--throttle`
// alone throttles the desktop viewport (a 2017 laptop is about a
// 4x-throttled core here). The raw profile goes to
// <out>/prof-<label>-<backend>[-tag].cpuprofile (`--from` reports one again).

import fs from 'node:fs';
import path from 'node:path';
import { argv, baseUrl, chromeFlags, isMain, launch, projectRoot, sleep, statusGlobal } from './server.mjs';

const clean = (s) => s.replace(/::h[0-9a-f]{16}$/, '').replace(/\[[0-9a-f]{16}\]/g, '');
const name = (n) => clean(n.callFrame.functionName || `(${n.callFrame.url.split('/').pop()}:${n.callFrame.lineNumber})`);

/// The Bevy system a frame belongs to, from its function name, or null.
export function systemOf(fn) {
  if (!fn.includes('FunctionSystem<') && !fn.includes('SystemState<')) return null;
  const m = fn.match(/, \(\), \(\), ([^<>]+?)> as /) || fn.match(/, ([a-z_][\w:]*?)> as /);
  return m ? m[1] : null;
}

export function report(profile, meta, { callersRe = null, depth = 6, top = 40 } = {}) {
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const parent = new Map();
  for (const n of profile.nodes) for (const c of n.children || []) parent.set(c, n.id);
  for (const n of profile.nodes) n.hits = 0;
  for (const s of profile.samples) byId.get(s).hits++;
  const total = profile.samples.length;
  const pct = (x) => ((100 * x) / total).toFixed(1).padStart(5);
  const sorted = (m) => [...m].sort((a, b) => b[1] - a[1]);

  if (callersRe) {
    // Inclusive time of each subtree, then the chains above the outermost
    // matching frames.
    const tot = new Map();
    const sub = (n) => {
      if (tot.has(n.id)) return tot.get(n.id);
      let t = n.hits;
      for (const c of n.children || []) t += sub(byId.get(c));
      tot.set(n.id, t);
      return t;
    };
    const re = new RegExp(callersRe);
    const paths = new Map();
    for (const n of profile.nodes) {
      if (!re.test(name(n))) continue;
      let a = parent.get(n.id), inner = false;
      while (a !== undefined) {
        if (re.test(name(byId.get(a)))) { inner = true; break; }
        a = parent.get(a);
      }
      if (inner) continue;
      const chain = [];
      let id = parent.get(n.id);
      while (id !== undefined && chain.length < depth) {
        chain.push(name(byId.get(id)).slice(0, 300));
        id = parent.get(id);
      }
      const k = chain.join('\n      <- ');
      paths.set(k, (paths.get(k) || 0) + sub(n));
    }
    console.log(`--- callers of /${callersRe}/ (inclusive, % of ${total} samples) ---`);
    for (const [k, v] of sorted(paths).slice(0, top)) console.log(`${pct(v)}\n      <- ${k}\n`);
    return;
  }

  const self = new Map();
  const systems = new Map();
  const incl = new Map();
  for (const n of profile.nodes) {
    if (!n.hits) continue;
    self.set(name(n), (self.get(name(n)) || 0) + n.hits);
    const seen = new Set();
    for (let id = n.id; id !== undefined; id = parent.get(id)) {
      const k = name(byId.get(id));
      if (!seen.has(k)) { seen.add(k); incl.set(k, (incl.get(k) || 0) + n.hits); }
    }
    let id = n.id, sys = null;
    while (id !== undefined) {
      sys = systemOf(byId.get(id).callFrame.functionName);
      if (sys) break;
      id = parent.get(id);
    }
    const k = sys ? clean(sys) : '(outside systems)';
    systems.set(k, (systems.get(k) || 0) + n.hits);
  }
  const idle = (self.get('(idle)') || 0) + (self.get('(program)') || 0);
  console.log(`${meta}\nsamples ${total}, idle+program ${pct(idle).trim()}%, gc ${pct(self.get('(garbage collector)') || 0).trim()}%`);
  console.log('--- self ---');
  for (const [k, v] of sorted(self).slice(0, top)) console.log(pct(v), k.slice(0, 160));
  console.log('--- by system (inclusive, nearest enclosing) ---');
  for (const [k, v] of sorted(systems).slice(0, top)) console.log(pct(v), k);
  // Inclusive time by function, without the schedule's and the glue's
  // frames that every sample passes through.
  const skip = /^\(|run_unsafe|apply_deferred|executor|Schedule|FnOnce|call_once|closure|World::run|App::update|__wbg|wasm-function|wasm-to-js|js-to-wasm|run_schedule|SystemState|FunctionSystem|RenderGraphRunner|run_graph/;
  console.log('--- inclusive, by function ---');
  for (const [k, v] of sorted(incl).filter(([k]) => !skip.test(k)).slice(0, top)) console.log(pct(v), k.slice(0, 160));
}

export async function run({ args = process.argv.slice(2), defaults = {}, root = projectRoot() } = {}) {
  const { opt, flag } = argv(args);
  const G = statusGlobal(opt);
  const backend = opt('--backend', defaults.backend || 'webgpu');
  const phone = flag('--phone');
  const throttle = Number(opt('--throttle', phone ? 4 : 0));
  const secs = Number(opt('--secs', defaults.secs ?? 10));
  const dist = opt('--dist', defaults.dist || 'prof');
  const tag = opt('--tag', null);
  const from = opt('--from', null);
  const reportOpts = { callersRe: opt('--callers', null), depth: Number(opt('--depth', 6)), top: Number(opt('--top', 40)) };
  if (from) {
    report(JSON.parse(fs.readFileSync(from, 'utf8')), `${from}`, reportOpts);
    return;
  }
  const query = new URLSearchParams(defaults.query || '');
  for (const [k, v] of new URLSearchParams(opt('--query', ''))) query.set(k, v);
  if (backend === 'webgl2') query.set('backend', 'webgl2');
  const withG = (expr) => `((G) => (${expr}))(window[${JSON.stringify(G)}])`;
  const waitExpr = opt('--wait', defaults.wait || '(G?.frames || 0) > 120');
  const posExpr = opt('--pos', defaults.pos || '0');
  const label = opt('--label', defaults.label || query.get('level') || 'prof');
  const out = path.resolve(root, opt('--out', defaults.out || 'report/perf'));

  const base = baseUrl(opt, root);
  const browser = await launch(chromeFlags({ backend, gesture: true }), { root });
  try {
    const page = await browser.newPage();
    if (phone) {
      await page.setViewport({ width: 844, height: 390, deviceScaleFactor: 3, isMobile: true, hasTouch: true, isLandscape: true });
    } else {
      const [w, h] = opt('--size', '1280x800').split('x').map(Number);
      await page.setViewport({ width: w, height: h, deviceScaleFactor: 1 });
    }
    const cdp = await page.createCDPSession();
    if (throttle > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: throttle });
    const url = `${base}dist/${dist}/index.html?${query}`;
    console.error(`${url}  (${backend}${phone ? ', phone' : ''}${throttle > 1 ? `, throttle ${throttle}` : ''})`);
    await page.goto(url);
    await page.waitForFunction(withG(waitExpr), { timeout: 600000, polling: 200 });
    const got = await page.evaluate((g) => ({ backend: window[g]?.backend }), G);
    await cdp.send('Profiler.enable');
    await cdp.send('Profiler.setSamplingInterval', { interval: 250 });
    await cdp.send('Profiler.start');
    const s0 = Number(await page.evaluate(withG(posExpr))) || 0;
    await sleep(secs * 1000);
    const { profile } = await cdp.send('Profiler.stop');
    const s1 = Number(await page.evaluate(withG(posExpr))) || 0;
    fs.mkdirSync(out, { recursive: true });
    const file = path.join(out, `prof-${label}-${backend}${tag ? '-' + tag : ''}.cpuprofile`);
    fs.writeFileSync(file, JSON.stringify(profile));
    if (!profile.nodes.some((n) => /FunctionSystem</.test(n.callFrame.functionName))) {
      console.error(`warning: no Bevy system names in the profile; is dist/${dist}/ a names-kept build (cargo xtask prof)?`);
    }
    report(profile, `${label} ${got.backend || backend}, pos ${s0.toFixed(0)} to ${s1.toFixed(0)} over ${secs} s; written ${path.relative(root, file)}`, reportOpts);
  } finally {
    await browser.close();
  }
}

if (isMain(import.meta.url)) await run();
