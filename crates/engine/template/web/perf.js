// The on-device measurement: `index.html?perf=1` (Playground Kit's
// template/web/perf.js; `js/perf.mjs` runs it in headless Chrome and reads
// the result from the status global's `perf`).
//
// Once the client is running, it records every frame's time from
// requestAnimationFrame for `secs` seconds (the game is expected to be
// doing something by itself: an attract mode, an autopilot, a fly camera
// chosen by the page's other parameters), then, where the page offers
// `reload(arg)`, reloads `reloads` times, noting the wasm memory's
// high-water mark after each, and shows the results on the device.
//
// Parameters (beside the page's own):
//   secs=N           recording length (default 60)
//   reloads=N        reloads after the recording (default 10 when the page has reload(), else 0)
//   reloadLevels=a,b the arguments to cycle through on reload (default: G.level)
//   settle=N         seconds of frames watched after each reload (default 3)
//
// The status global may publish `pos` (a distance along a route, in
// metres) for the per-kilometre table, and `firstFrameMs`, `readyMs`,
// `warmUp`, `lateFrames`, `level` for the load section.

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const nextFrame = () => new Promise((r) => requestAnimationFrame(r));
const mb = (bytes) => Math.round(bytes / 1048576);

function memory(G) {
  const wasm = G.wasmMemoryBytes ? mb(G.wasmMemoryBytes()) : null;
  const heap = performance.memory ? mb(performance.memory.usedJSHeapSize) : null;
  return { wasmMB: wasm, jsHeapMB: heap };
}

// Every frame's time (ms) from requestAnimationFrame, with the time since
// the start (s) and the game's position, until stop().
function recorder(G) {
  const dts = [], at = [], s = [];
  let last = null, t0 = null, on = true;
  const frame = (now) => {
    if (!on) return;
    requestAnimationFrame(frame);
    if (t0 === null) t0 = now;
    if (last !== null) { dts.push(now - last); at.push((now - t0) / 1000); s.push(G.pos || 0); }
    last = now;
  };
  requestAnimationFrame(frame);
  return {
    elapsed: () => (t0 === null || last === null ? 0 : (last - t0) / 1000),
    worst: () => (dts.length ? Math.max(...dts.slice(-2000)) : 0),
    stop: () => { on = false; return { dts, at, s }; },
  };
}

const pct = (sorted, p) => (sorted.length ? sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))] : null);
const r1 = (x) => (x === null || x === undefined ? null : Math.round(x * 10) / 10);

// Frame-time statistics: percentiles, frames per second and the worst
// frame of each half-second window, hitches, and the split per kilometre.
function summarise({ dts, at, s }) {
  const sorted = Float64Array.from(dts).sort();
  const windows = [];
  let n = 0, sum = 0;
  for (const dt of dts) {
    n++; sum += dt;
    if (sum >= 500) { windows.push((1000 * n) / sum); n = 0; sum = 0; }
  }
  const fps = windows.sort((a, b) => a - b);
  const over = (ms, within) => dts.filter((d, i) => d > ms && (within === undefined || at[i] <= within)).length;
  const worstI = dts.length ? dts.indexOf(Math.max(...dts)) : -1;
  const byKm = new Map();
  dts.forEach((d, i) => {
    const k = Math.floor(s[i] / 1000);
    if (!byKm.has(k)) byKm.set(k, []);
    byKm.get(k).push(d);
  });
  const km = [...byKm].map(([k, v]) => {
    const o = Float64Array.from(v).sort();
    return { km: k, frames: v.length, p50: r1(pct(o, 50)), p95: r1(pct(o, 95)), max: r1(o[o.length - 1]) };
  });
  return {
    seconds: r1(at.length ? at[at.length - 1] : 0),
    frames: dts.length,
    fpsMedian: r1(pct(fps, 50)),
    fps5: r1(pct(fps, 5)),
    p50: r1(pct(sorted, 50)), p90: r1(pct(sorted, 90)), p95: r1(pct(sorted, 95)),
    p99: r1(pct(sorted, 99)), p999: r1(pct(sorted, 99.9)), max: r1(sorted[sorted.length - 1] ?? null),
    meanMs: r1(dts.reduce((a, b) => a + b, 0) / Math.max(1, dts.length)),
    over50First30: over(50, 30), over50: over(50), over33: over(33.4),
    worstAt: worstI >= 0 ? { t: r1(at[worstI]), s: Math.round(s[worstI]) } : null,
    slow: dts.map((d, i) => ({ ms: r1(d), t: r1(at[i]), s: Math.round(s[i]) })).filter((x) => x.ms > 50).slice(0, 20),
    km,
  };
}

function panel() {
  const el = document.createElement('div');
  el.id = 'perf';
  el.style.cssText = 'position:fixed;left:0;right:0;top:0;z-index:20;font:13px/1.35 ui-monospace,Menlo,monospace;color:#e8ecf4;background:rgba(5,6,12,.82);padding:8px 12px calc(8px + env(safe-area-inset-bottom));padding-top:calc(8px + env(safe-area-inset-top));white-space:pre-wrap;max-height:100%;overflow:auto;-webkit-overflow-scrolling:touch';
  document.body.appendChild(el);
  return el;
}

const sec = (ms) => (ms === null || ms === undefined ? '-' : (ms / 1000).toFixed(2) + ' s');

function report(r) {
  const L = [];
  const d = r.device, ld = r.load, f = r.flight;
  L.push(`${document.title.toUpperCase()} — measurement, ${r.date}`);
  L.push(`${d.backend}${d.fallback ? ' (fallback: ' + d.fallback + ')' : ''} ${d.adapter || ''} · ${d.canvas} px @ dpr ${d.dpr}`);
  if (r.level) L.push(`${r.level}`);
  L.push('');
  L.push('LOAD (from navigation)');
  L.push(`  first frame   ${sec(ld.firstFrameMs)}`);
  L.push(`  ready         ${sec(ld.readyMs)}${ld.warmUp !== undefined ? `  (${ld.warmUp} warm-up pipelines)` : ''}`);
  L.push(`  wasm file     ${ld.wasmFileMB} MB`);
  L.push('');
  L.push(`FRAMES (${f.seconds} s, ${f.frames} frames)`);
  L.push(`  fps median / 5th pct    ${f.fpsMedian} / ${f.fps5}   (half-second windows)`);
  L.push(`  frame ms p50 p95 p99    ${f.p50} / ${f.p95} / ${f.p99}`);
  L.push(`  frame ms p99.9 max      ${f.p999} / ${f.max}${f.worstAt ? `  (at ${f.worstAt.t} s, pos ${f.worstAt.s} m)` : ''}`);
  L.push(`  frames > 50 ms          ${f.over50First30} in the first 30 s, ${f.over50} in all`);
  L.push(`  frames > 33 ms          ${f.over33}`);
  L.push(`  pipelines after warm-up ${f.lateFrames ? f.lateFrames + ' frames with one compiling' : 'none'}`);
  if (f.slow.length) L.push(`  slow: ${f.slow.map((x) => `${x.ms} ms @${x.t}s pos ${x.s}`).join(', ')}`);
  if (f.km.length > 1) {
    L.push('  per km: frame ms p50 / p95 / max');
    for (let i = 0; i < f.km.length; i += 2) {
      L.push('  ' + f.km.slice(i, i + 2).map((b) => `${String(b.km).padStart(3)} km ${String(b.p50).padStart(5)} /${String(b.p95).padStart(5)} /${String(b.max).padStart(6)}`).join('    '));
    }
  }
  L.push('');
  L.push('MEMORY (wasm: high-water mark; JS heap where the browser tells)');
  L.push(`  after load    wasm ${r.memory.load.wasmMB} MB${r.memory.load.jsHeapMB !== null ? `, JS heap ${r.memory.load.jsHeapMB} MB` : ''}`);
  L.push(`  after frames  wasm ${r.memory.flight.wasmMB} MB${r.memory.flight.jsHeapMB !== null ? `, JS heap ${r.memory.flight.jsHeapMB} MB` : ''}`);
  if (r.reloads.length) {
    L.push('');
    L.push(`RELOADS (${r.reloads.length})`);
    L.push('   #  arg       ready s  wasm MB  heap MB  worst ms after');
    for (const x of r.reloads) {
      L.push(`  ${String(x.i).padStart(2)}  ${String(x.level ?? x.arg).padEnd(8)}  ${(x.ms / 1000).toFixed(1).padStart(7)}  ${String(x.wasmMB).padStart(7)}  ${String(x.jsHeapMB ?? '-').padStart(7)}  ${String(x.worstAfterMs).padStart(14)}`);
    }
    L.push(`  wasm growth: ${r.verdict.wasmGrowthMB >= 0 ? '+' : ''}${r.verdict.wasmGrowthMB} MB from reload 1 to ${r.reloads.length}; ${r.verdict.wasmGrowthAllMB >= 0 ? '+' : ''}${r.verdict.wasmGrowthAllMB} MB from the first load`);
  }
  L.push('');
  L.push(`no frame over 50 ms in the first 30 s: ${r.verdict.noHitch30 ? 'yes' : 'NO'}`);
  if (r.errors.length) L.push(`errors: ${r.errors.join(' | ')}`);
  return L.join('\n');
}

export async function runPerf(G, params) {
  const el = panel();
  const status = (t) => { el.textContent = 'perf: ' + t; };
  const errors = [];
  window.addEventListener('error', (e) => errors.push(String(e.message || e)));
  G.perf = { phase: 'load' };
  let lock = null;
  try { lock = await navigator.wakeLock?.request('screen'); } catch { /* not offered */ }

  status('loading…');
  while (G.state !== 'running') {
    if (G.state === 'failed') { status('failed: ' + G.error); G.perf = { phase: 'failed', error: G.error }; return; }
    await sleep(100);
  }
  const secs = Number(params.get('secs')) || 60;
  const reloads = params.has('reloads') ? Number(params.get('reloads')) : (G.reload ? 10 : 0);
  const levels = (params.get('reloadLevels') || String(G.level ?? '')).split(',');
  const settle = Number(params.get('settle') || 3);
  const r = {
    date: new Date().toISOString().slice(0, 16).replace('T', ' '),
    level: G.level,
    device: {
      backend: G.backend, adapter: G.adapter, fallback: G.fallback || null,
      dpr: window.devicePixelRatio, canvas: `${document.getElementById('game').width}×${document.getElementById('game').height}`,
      ua: navigator.userAgent,
    },
    load: {
      firstFrameMs: G.firstFrameMs === undefined ? null : Math.round(G.firstFrameMs),
      readyMs: G.readyMs === undefined ? null : Math.round(G.readyMs),
      warmUp: G.warmUp, wasmFileMB: G.wasmMB,
    },
    memory: { load: memory(G) },
    reloads: [],
    errors,
  };

  // The recording.
  G.perf = { phase: 'flight', secs };
  // Its window on the page's clock, for tools that time each frame's work
  // themselves (js/perf.mjs's CPU timer).
  const flightT0 = performance.now();
  const rec = recorder(G);
  while (rec.elapsed() < secs) {
    status(`recording ${Math.floor(rec.elapsed())} / ${Math.round(secs)} s${G.pos ? ` · pos ${Math.round(G.pos)} m` : ''} · worst ${r1(rec.worst())} ms`);
    await sleep(500);
  }
  r.flight = summarise(rec.stop());
  r.flight.window = [flightT0, performance.now()];
  r.flight.lateFrames = G.lateFrames || 0;
  r.memory.flight = memory(G);

  // The reloads.
  for (let i = 1; i <= reloads && G.reload; i++) {
    const arg = levels[(i - 1) % levels.length];
    G.perf = { phase: 'reload', i, of: reloads };
    status(`reload ${i} / ${reloads}: ${arg}…`);
    try {
      const t0 = performance.now();
      const x = (await G.reload(arg || undefined)) || {};
      const after = recorder(G);
      await sleep(settle * 1000);
      const { dts } = after.stop();
      r.reloads.push({ i, arg, ms: Math.round(performance.now() - t0), ...x, ...memory(G), worstAfterMs: r1(dts.length ? Math.max(...dts) : null) });
    } catch (e) {
      errors.push(`reload ${i}: ${e.message || e}`);
      break;
    }
    await nextFrame();
  }

  const w = r.reloads.map((x) => x.wasmMB);
  r.verdict = {
    noHitch30: r.flight.over50First30 === 0,
    wasmGrowthMB: w.length ? w[w.length - 1] - w[0] : 0,
    wasmGrowthAllMB: w.length ? w[w.length - 1] - r.memory.load.wasmMB : 0,
    wasmPeakMB: Math.max(r.memory.load.wasmMB, r.memory.flight.wasmMB, ...w),
  };
  r.text = report(r);
  G.perf = { phase: 'done', result: r };
  try { await lock?.release(); } catch { /* gone */ }

  el.textContent = r.text + '\n\n';
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;gap:10px;flex-wrap:wrap';
  const button = (label, fn) => {
    const b = Object.assign(document.createElement('button'), { textContent: label });
    b.style.cssText = 'font:15px system-ui;padding:8px 14px;border-radius:6px;border:0;background:var(--accent, #5ad0ff);color:#000';
    b.addEventListener('click', fn);
    row.appendChild(b);
    return b;
  };
  const copy = button('Copy results', async () => {
    try { await navigator.clipboard.writeText(r.text + '\n\n' + JSON.stringify(r)); copy.textContent = 'Copied'; } catch { copy.textContent = 'Copy failed: select the text'; }
  });
  button('Hide', () => { el.remove(); });
  el.appendChild(row);
}
