// A cruise of the open world begun at each of its starts
// (docs/vision/peninsula.md, stage 6f): the car is steered along the
// start's road with the keyboard for a while, a picture every few
// seconds, the frame times logged. For looking at the bay side's city.
//
//   node tools/parity/cruise-shots.mjs [--starts redwood-city,lombard]
//        [--secs 20] [--every 4] [--speed 18] [--backend webgl2]
//        [--dist next] [--out <dir>] [--url <base> | --port N]
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';
import { ROOT } from './lib/jstree.mjs';
import { baseUrl, chromeFlags } from './lib/server.mjs';

const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const backend = opt('--backend', 'webgpu');
const secs = Number(opt('--secs', 20));
const every = Number(opt('--every', 4)) * 1000;
const speed = Number(opt('--speed', 18));
const out = opt('--out', path.join(ROOT, 'parity/report/cruise'));
fs.mkdirSync(out, { recursive: true });
const base = baseUrl(opt);

// Each start's road (`mp_levels::peninsula::starts`).
const ROADS = {
  crest: 'coast-south',
  'redwood-city': 'el-camino',
  'san-mateo': 'el-camino',
  'south-city': 'el-camino',
  mission: 'el-camino',
  lombard: 'sf-lombard',
};
const starts = opt('--starts', Object.keys(ROADS).join(',')).split(',');

function corridor(id) {
  const txt = fs.readFileSync(path.join(ROOT, `assets/atlas/peninsula/corridors/${id}.txt`), 'utf8');
  return txt.split('\n').filter((l) => l && !l.startsWith('#')).map((l) => l.split(/\s+/).slice(0, 2).map(Number));
}

const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', headless: 'new', args: chromeFlags({ backend, race: true }) });
for (const start of starts) {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text().slice(0, 300)); });
  await page.setViewport({ width: 1280, height: 720 });
  const q = new URLSearchParams({ level: 'peninsula', cruise: '1', start, autostart: 'sports', seed: '1', pursuit: '0' });
  const url = `${base}dist/${opt('--dist', 'next')}/index.html?${q}`;
  const t0 = Date.now();
  await page.goto(url);
  await page.waitForFunction('(window.__mp?.race?.time ?? -1) >= 0.5', { timeout: 240000, polling: 200 });
  const ready = (Date.now() - t0) / 1000;
  await page.evaluate(() => { window.__dts = []; let last = null; const f = (now) => { requestAnimationFrame(f); if (last !== null) window.__dts.push(now - last); last = now; }; requestAnimationFrame(f); });
  const frames = () => page.evaluate(() => { const d = window.__dts.splice(0); if (!d.length) return 'no frames'; d.sort((a, b) => a - b); return `${d.length} frames p50 ${d[d.length >> 1].toFixed(1)} p95 ${d[Math.floor(d.length * 0.95)].toFixed(1)} max ${d[d.length - 1].toFixed(1)} ms, ${d.filter((x) => x > 50).length} over 50`; });
  const read = () => page.evaluate(() => { const r = window.__mp?.race; return r ? { x: r.x, z: r.z, yaw: r.yaw, speed: r.speed } : null; });
  const route = corridor(ROADS[start] ?? 'el-camino');
  let s = await read();
  let at = 0, best = 1e18;
  route.forEach(([x, z], i) => { const d = (x - s.x) ** 2 + (z - s.z) ** 2; if (d < best) { best = d; at = i; } });
  console.log(`${start}: racing ${ready.toFixed(1)} s after the page opened, at (${s.x.toFixed(0)}, ${s.z.toFixed(0)}), ${Math.sqrt(best).toFixed(1)} m from its road`);
  await page.screenshot({ path: path.join(out, `${start}-0.png`) });
  const held = new Set();
  const key = async (k, on) => { if (on && !held.has(k)) { held.add(k); await page.keyboard.down(k); } if (!on && held.has(k)) { held.delete(k); await page.keyboard.up(k); } };
  const tDrive = Date.now();
  let shot = 1, lastShot = tDrive;
  while (Date.now() - tDrive < secs * 1000) {
    s = await read();
    let bi = at, bd = 1e18;
    for (let i = Math.max(0, at - 5); i < Math.min(route.length, at + 40); i++) { const d = (route[i][0] - s.x) ** 2 + (route[i][1] - s.z) ** 2; if (d < bd) { bd = d; bi = i; } }
    at = bi;
    const aim = route[Math.min(route.length - 1, at + 5)];
    let a = Math.atan2(aim[1] - s.z, aim[0] - s.x) - s.yaw;
    a = Math.atan2(Math.sin(a), Math.cos(a));
    await key('ArrowRight', a > 0.04);
    await key('ArrowLeft', a < -0.04);
    await key('ArrowUp', s.speed < speed);
    await key('ArrowDown', s.speed > speed + 3);
    if (Date.now() - lastShot >= every) {
      lastShot = Date.now();
      console.log(`  ${((Date.now() - tDrive) / 1000).toFixed(0)} s: (${s.x.toFixed(0)}, ${s.z.toFixed(0)}) ${s.speed.toFixed(1)} m/s; ${await frames()}`);
      await page.screenshot({ path: path.join(out, `${start}-${shot++}.png`) });
    }
    await new Promise((r) => setTimeout(r, 30));
  }
  for (const k of [...held]) await key(k, false);
  if (errors.length) console.log(`  errors: ${errors.slice(0, 5).join(' | ')}`);
  await page.close();
}
await browser.close();
