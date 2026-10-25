// A long drive through the open world with the keyboard (D1252): steers
// along a road's corridor (assets/atlas/peninsula/corridors/<road>.txt) at
// a steady speed for minutes, and reports, per window of `--window` s, the
// frame times (p50, p95, max, frames over 33 and 50 ms), the JS heap, the
// wasm memory, and what the world holds (`__mp.census`: entities, assets,
// the stream's image cache and stash, sections, animation blocks). Each
// frame over `--long` ms is printed with the stream's work round it
// (`__mp.frameWork`). At a corridor's end the car turns back.
//
//   node tools/parity/long-drive.mjs --road el-camino --secs 600 \
//     --page dist/longdrive/index.html [--backend webgl2] [--json out.json]
//     [--events] (every stream event, with its frame) [--capped] [--gc <ms>]
//     [--menu <s>] [--throttle N] [--gpu-mem] [--trace <file> --trace-at <s> --trace-secs <n>]
//
// The page is reached through the registered server (`lib/server.mjs`:
// `--url`, `--port`, `$PORT` or `proj port`); from a worktree, which the
// server does not serve, pass `--url` to the main checkout's and a
// `--page` copied under it.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import puppeteer from 'puppeteer-core';
import { baseUrl, chromeFlags } from './lib/server.mjs';

const args = process.argv.slice(2);
const opt = (name, d) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : d;
};
const road = opt('road', 'el-camino');
const secs = Number(opt('secs', 300));
const speed = Number(opt('speed', 30));
const backend = opt('backend', 'webgpu');
const windowS = Number(opt('window', 30));
const longMs = Number(opt('long', 50));
const start = opt('start', 'san-mateo');
const page0 = opt('page', 'dist/next/index.html');
const query = opt('query', '');
const jsonOut = opt('json', null);
let dir = Number(opt('dir', 1));
const verbose = args.includes('--events');
// `--capped`: at the display's rate, as a player sees it (else uncapped).
const capped = args.includes('--capped');
// `--gc <ms>`: a full garbage collection that often (an experiment: what
// the collector's own rhythm does to the frames).
const gcMs = Number(opt('gc', 0));
// `--gpu-mem`: the GPU process's memory from `nvidia-smi` each window.
// Asking the driver stalls the GPU for a frame or two (100 to 230 ms
// here), so it is off unless asked, and those frames are its own.
const gpuMem = args.includes('--gpu-mem');
// `--trace <file> --trace-at <s> --trace-secs <n>`: a Chrome trace (every
// process: the GPU process's tasks, the collector) of n seconds from s.
const trace = opt('trace', null);
const traceAt = Number(opt('trace-at', 120));
const traceSecs = Number(opt('trace-secs', 15));
let tracing = 0; // 0 not yet, 1 on, 2 done

const route = fs
  .readFileSync(`assets/atlas/peninsula/corridors/${road}.txt`, 'utf8')
  .split('\n')
  .filter((l) => l && !l.startsWith('#'))
  .map((l) => l.split(/\s+/).slice(0, 2).map(Number));

// `--menu <s>`: start as a player does, from the menu after s seconds
// on it (its level views prepared and flying), not by `autostart`.
const menuS = opt('menu', null);
const url = `${baseUrl((n, d) => opt(n.slice(2), d))}${page0}?level=peninsula&cruise=1&start=${start}${menuS === null ? '&autostart=sports' : ''}&seed=1&pursuit=0${query ? '&' + query : ''}`;
const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: [...chromeFlags({ backend, race: true, capped }), ...(gcMs ? ['--js-flags=--expose-gc'] : [])],
});
const page = await browser.newPage();
await page.setViewport({ width: 1280, height: 720 });
const t0 = Date.now();
const log = (s) => console.log(`${((Date.now() - t0) / 1000).toFixed(1).padStart(7)} ${s}`);
page.on('pageerror', (e) => log('E ' + e));
page.on('console', (m) => {
  const t = m.text();
  if (/panic|error/i.test(t)) log('C ' + t.slice(0, 300));
});
log(url);
// The GPU calls of each frame (WebGPU: buffer writes and creations, texture
// writes, pipelines, bind groups; WebGL2: draws), kept for the long ones.
await page.evaluateOnNewDocument(() => {
  const G = (window.__ldg = { wbKB: 0, cbn: 0, cbKB: 0, wtn: 0, pipe: 0, bg: 0, draws: 0 });
  const wrap = (proto, name, f) => {
    if (!proto?.[name]) return;
    const o = proto[name];
    proto[name] = function (...a) {
      f(a);
      return o.apply(this, a);
    };
  };
  wrap(self.GPUQueue?.prototype, 'writeBuffer', (a) => {
    const d = a[2];
    G.wbKB += (a[4] ?? (d.byteLength ?? 0) - (a[3] ?? 0) * (d.BYTES_PER_ELEMENT ?? 1)) / 1024;
  });
  wrap(self.GPUQueue?.prototype, 'writeTexture', () => G.wtn++);
  wrap(self.GPUDevice?.prototype, 'createBuffer', (a) => {
    G.cbn++;
    G.cbKB += a[0].size / 1024;
  });
  wrap(self.GPUDevice?.prototype, 'createRenderPipeline', () => G.pipe++);
  wrap(self.GPUDevice?.prototype, 'createBindGroup', () => G.bg++);
  wrap(self.GPURenderPassEncoder?.prototype, 'drawIndexed', () => G.draws++);
  wrap(self.GPURenderPassEncoder?.prototype, 'draw', () => G.draws++);
  for (const n of ['drawElementsInstanced', 'drawElements', 'drawArrays', 'drawArraysInstanced'])
    wrap(self.WebGL2RenderingContext?.prototype, n, () => G.draws++);
});
await page.goto(url);
if (menuS !== null) {
  await page.waitForFunction('window.__mp?.ui?.("btn-start")?.visible && window.__mp.holding === false', { timeout: 300000, polling: 500 });
  log(`the menu is up; starting in ${menuS} s`);
  await new Promise((r) => setTimeout(r, Number(menuS) * 1000));
  await page.evaluate(() => window.__mp.reveal?.('btn-start'));
  await new Promise((r) => setTimeout(r, 500));
  const b = await page.evaluate(() => window.__mp.ui('btn-start'));
  await page.mouse.click(b.x + b.w / 2, b.y + b.h / 2);
}
// (The menu's start may reload the page, which ends a wait: wait again.)
for (let tries = 0; ; tries++) {
  try {
    await page.waitForFunction('(window.__mp?.race?.time ?? -1) >= 0.5', { timeout: 300000, polling: 200 });
    break;
  } catch (e) {
    if (tries >= 3) throw e;
    await new Promise((r) => setTimeout(r, 1000));
  }
}
log('racing');
// `--throttle N`: the CPU N times slower (DevTools' throttling), for a
// weaker machine's main thread.
const throttle = Number(opt('throttle', 0));
if (throttle > 1) {
  const c = await page.createCDPSession();
  await c.send('Emulation.setCPUThrottlingRate', { rate: throttle });
}
// A CPU profile of the drive (`--prof <file>`, a names-kept build from
// `cargo xtask prof`), with the long frames' spans in <file>.long.json
// for `probe-long`-style reports.
const prof = opt('prof', null);
let cdp;
let p0 = 0;
if (prof) {
  cdp = await page.createCDPSession();
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 500 });
  await cdp.send('Profiler.start');
  p0 = await page.evaluate(() => performance.now());
}
await page.evaluate(() => {
  window.__ld = [];
  window.__ldl = [];
  let last = null;
  const f = (now) => {
    requestAnimationFrame(f);
    const g = window.__ldg;
    if (last !== null) {
      const row = [now - last, window.__mp?.frames ?? 0];
      if (now - last > 30) window.__ldl.push([last, now]);
      if (now - last > 30) row.push(Object.fromEntries(Object.entries(g).map(([k, v]) => [k, Math.round(v)])));
      window.__ld.push(row);
    }
    for (const k in g) g[k] = 0;
    last = now;
  };
  requestAnimationFrame(f);
});
if (gcMs) await page.evaluate((ms) => setInterval(() => window.gc(), ms), gcMs);

const sample = () =>
  page.evaluate(() => {
    const r = window.__mp.race;
    const d = window.__ld.splice(0);
    const w = (window.__mp.frameWork ?? []).splice(0);
    return {
      x: r.x,
      z: r.z,
      yaw: r.yaw,
      speed: r.speed,
      d,
      w,
      heap: performance.memory?.usedJSHeapSize ?? 0,
      wasm: window.__mp.wasmMemoryBytes?.() ?? 0,
      census: window.__mp.census ?? null,
      at: window.__mp.stream?.at ?? null,
      sections: Object.values(window.__mp.stream?.sections ?? {}).reduce((a, v) => ((a[v] = (a[v] ?? 0) + 1), a), {}),
    };
  });

const held = new Set();
const key = async (k, on) => {
  if (on && !held.has(k)) {
    held.add(k);
    await page.keyboard.down(k);
  }
  if (!on && held.has(k)) {
    held.delete(k);
    await page.keyboard.up(k);
  }
};

let s = await sample();
let at = 0;
let best = Infinity;
route.forEach(([x, z], i) => {
  const d = (x - s.x) ** 2 + (z - s.z) ** 2;
  if (d < best) [best, at] = [d, i];
});

// The GPU memory of Chrome's GPU process, where `nvidia-smi` lists it
// (null elsewhere): what the GPU holds that the page cannot see.
// (found among the browser's descendants when first asked).
let gpuPid = null;
const findGpu = () => {
  try {
    const rows = execFileSync('ps', ['-e', '-o', 'pid=,ppid=,args='], { encoding: 'utf8' })
      .split('\n')
      .map((l) => l.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/))
      .filter(Boolean);
    const mine = new Set([String(browser.process().pid)]);
    for (let grew = true; grew; ) {
      grew = false;
      for (const [, pid, ppid] of rows) {
        if (mine.has(ppid) && !mine.has(pid)) {
          mine.add(pid);
          grew = true;
        }
      }
    }
    return rows.find(([, pid, , args]) => mine.has(pid) && args.includes('--type=gpu-process'))?.[1] ?? null;
  } catch {
    return null;
  }
};
const gpuMB = () => {
  gpuPid ??= findGpu();
  if (!gpuPid) return null;
  try {
    const out = execFileSync('nvidia-smi', { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    const line = out.split('\n').find((l) => l.split(/\s+/).includes(gpuPid));
    return line ? Number(line.match(/(\d+)MiB/)?.[1]) : null;
  } catch {
    return null;
  }
};

const windows = [];
let win = { t: 0, d: [], dw: [], kb: 0, long: [], census: null, heap: 0, wasm: 0 };
const recent = []; // the stream's work entries of the last few seconds
let lastSample = 0;
const tEnd = Date.now() + secs * 1000;
const tStart = Date.now();
const closeWindow = () => {
  const d = win.d.map((x) => x[0]).sort((a, b) => a - b);
  const q = (p) => (d.length ? d[Math.min(d.length - 1, Math.floor(p * d.length))] : 0);
  const row = {
    t: win.t,
    frames: d.length,
    p50: +q(0.5).toFixed(1),
    p95: +q(0.95).toFixed(1),
    max: +(d[d.length - 1] ?? 0).toFixed(0),
    over33: d.filter((x) => x > 33).length,
    over50: d.filter((x) => x > 50).length,
    // the frames with the stream's work in them
    work: win.dw.length,
    workMax: +Math.max(0, ...win.dw).toFixed(0),
    work33: win.dw.filter((x) => x > 33).length,
    // the most bytes made for the GPU in one frame
    uploadKB: win.kb,
    heapMB: +(win.heap / 1048576).toFixed(1),
    wasmMB: +(win.wasm / 1048576).toFixed(0),
    gpuMB: gpuMem ? gpuMB() : null,
    census: win.census,
    pos: [Math.round(s.x), Math.round(s.z), +s.speed.toFixed(0)],
    section: s.at,
    long: win.long,
  };
  windows.push(row);
  const c = row.census ?? {};
  log(
    `[${row.t}s] at (${row.pos}) section ${row.section} frames ${row.frames} p50 ${row.p50} p95 ${row.p95} max ${row.max} >33 ${row.over33} >50 ${row.over50} (stream frames ${row.work}, max ${row.workMax}, >33 ${row.work33}, upload ${row.uploadKB} kB) | heap ${row.heapMB} MB wasm ${row.wasmMB} MB gpu ${row.gpuMB} MB | ${Object.entries(c)
      .map(([k, v]) => `${k} ${v}`)
      .join(' ')}`,
  );
};
while (Date.now() < tEnd) {
  const now = Date.now();
  const full = now - lastSample > 1000;
  if (full) {
    lastSample = now;
    s = await sample();
    win.d.push(...s.d);
    win.heap = s.heap;
    win.wasm = s.wasm;
    win.census = s.census;
    recent.push(...s.w);
    while (recent.length > 600) recent.shift();
    if (verbose) for (const e of s.w) if (e.events) log(`  f ${e.f}: ${e.events.join(', ')}`);
    for (const e of s.w) win.kb = Math.max(win.kb, (e.meshKB ?? 0) + (e.imageKB ?? 0) + (e.instanceKB ?? 0));
    // The frames the stream worked in (spawned, uploaded, read back).
    const busy = new Set(recent.filter((e) => e.events || e.spawned || e.meshes || e.images || e.despawned).map((e) => e.f));
    for (const [dt, f] of s.d) if (busy.has(f) || busy.has(f - 1)) win.dw.push(dt);
    for (const [dt, f, gpu] of s.d) {
      if (dt <= longMs) continue;
      const work = recent.filter((e) => e.f >= f - 2 && e.f <= f);
      const line = `long ${dt.toFixed(0)} ms at frame ${f} (${Math.round(s.x)}, ${Math.round(s.z)}) section ${s.at} ${JSON.stringify(s.sections)} gpu ${JSON.stringify(gpu)}: ${work.map((e) => JSON.stringify(e)).join(' ') || 'no work'}`;
      win.long.push(line);
      log(line);
    }
    const el = (now - tStart) / 1000;
    if (el >= win.t + windowS) {
      closeWindow();
      win = { t: Math.round(el), d: [], dw: [], kb: 0, long: [], census: null, heap: 0, wasm: 0 };
    }
  } else {
    s = { ...s, ...(await page.evaluate(() => {
      const r = window.__mp.race;
      return { x: r.x, z: r.z, yaw: r.yaw, speed: r.speed };
    })) };
  }
  if (trace && tracing === 0 && (now - tStart) / 1000 >= traceAt) {
    tracing = 1;
    await page.tracing.start({
      path: trace,
      categories: ['toplevel', 'gpu', 'gpu.dawn', 'disabled-by-default-gpu.dawn', 'v8', 'v8.gc', 'blink.gc', 'devtools.timeline', 'disabled-by-default-devtools.timeline'],
    });
    log(`tracing for ${traceSecs} s`);
  } else if (tracing === 1 && (now - tStart) / 1000 >= traceAt + traceSecs) {
    tracing = 2;
    await page.tracing.stop();
    log(`trace ${trace}`);
  }
  let bi = at;
  let bd = Infinity;
  for (let i = Math.max(0, at - 40); i < Math.min(route.length, at + 40); i++) {
    const d = (route[i][0] - s.x) ** 2 + (route[i][1] - s.z) ** 2;
    if (d < bd) [bd, bi] = [d, i];
  }
  at = bi;
  if ((dir > 0 && at >= route.length - 3) || (dir < 0 && at <= 2)) {
    dir = -dir;
    log(`the corridor's end: turning back`);
  }
  const aim = route[Math.max(0, Math.min(route.length - 1, at + 5 * dir))];
  let a = Math.atan2(aim[1] - s.z, aim[0] - s.x) - s.yaw;
  a = Math.atan2(Math.sin(a), Math.cos(a));
  await key('ArrowRight', a > 0.04);
  await key('ArrowLeft', a < -0.04);
  const slow = Math.abs(a) > 1 ? 8 : speed;
  await key('ArrowUp', s.speed < slow);
  await key('ArrowDown', s.speed > slow + 4);
  await new Promise((r) => setTimeout(r, 30));
}
closeWindow();
if (prof) {
  const { profile } = await cdp.send('Profiler.stop');
  fs.writeFileSync(prof, JSON.stringify(profile));
  fs.writeFileSync(prof + '.long.json', JSON.stringify({ p0, long: await page.evaluate(() => window.__ldl) }));
  log(`profile ${prof}`);
}
if (jsonOut) fs.writeFileSync(jsonOut, JSON.stringify({ url, road, secs, backend, windows }, null, 1));
await browser.close();
