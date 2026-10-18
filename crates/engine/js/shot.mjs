// Pictures of the web build, through the registered server, in headless
// Chrome on the real GPU. Saves PNGs under `--out` (default shots/) and
// prints the page's console errors and the status global.
//
//   node js/shot.mjs --query "level=x&autostart=1" --name menu \
//       [--wait 6] [--every 1.5 --count 3] [--backend webgl2] [--size 1280x720] [--touch] [--tap]
//       [--keys "Space:700,ArrowRight+Shift:900,wait:500"]  (held together with +, for ms, in turn, before the shots)
//       [--dist next] [--global __game] [--url|--port]
//
// The frame arrives as a download (`G.screenshot(name)` saves the next
// frame), so the WebGPU backend is pictured too.

import fs from 'node:fs';
import path from 'node:path';
import { argv, baseUrl, chromeFlags, isMain, launch, projectRoot, sleep, statusGlobal } from './server.mjs';

export async function run({ args = process.argv.slice(2), defaults = {}, root = projectRoot() } = {}) {
  const { opt, flag } = argv(args);
  const G = statusGlobal(opt);
  const backend = opt('--backend', defaults.backend || 'webgpu');
  const [w, h] = opt('--size', '1280x720').split('x').map(Number);
  const name = opt('--name', 'shot');
  const wait = Number(opt('--wait', 5));
  const every = Number(opt('--every', 1));
  const count = Number(opt('--count', 1));
  const dist = opt('--dist', defaults.dist || 'next');
  const out = path.resolve(root, opt('--out', defaults.out || 'shots'));
  fs.mkdirSync(out, { recursive: true });
  const query = new URLSearchParams(defaults.query || '');
  for (const [k, v] of new URLSearchParams(opt('--query', ''))) query.set(k, v);
  if (backend === 'webgl2') query.set('backend', 'webgl2');

  const browser = await launch(chromeFlags({ backend, gesture: true, capped: true }), { root });
  try {
    const page = await browser.newPage();
    const cdp = await page.createCDPSession();
    await cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: out });
    page.on('console', (m) => { if (['error', 'warn'].includes(m.type())) console.log(`[${m.type()}] ${m.text().slice(0, 400)}`); });
    page.on('pageerror', (e) => console.log(`[pageerror] ${String(e).slice(0, 400)}`));
    await page.setViewport({ width: w, height: h, hasTouch: flag('--touch'), isMobile: flag('--touch') });
    const url = `${baseUrl(opt, root)}dist/${dist}/index.html?${query}`;
    console.log('open', url);
    await page.goto(url);
    await page.waitForFunction((g) => (window[g]?.frames || 0) > 5 || window[g]?.state === 'failed', { timeout: 90000 }, G);
    if (flag('--tap')) await page.touchscreen.tap(w * 0.2, h * 0.6);
    await sleep(wait * 1000);
    for (const step of (opt('--keys', '') || '').split(',').filter(Boolean)) {
      const [names, ms] = step.split(':');
      const keys = names === 'wait' ? [] : names.split('+');
      for (const k of keys) await page.keyboard.down(k);
      await sleep(Number(ms));
      for (const k of keys.reverse()) await page.keyboard.up(k);
    }
    const files = [];
    for (let i = 0; i < count; i++) {
      if (i) await sleep(every * 1000);
      const file = count > 1 ? `${name}-${i}.png` : `${name}.png`;
      try { fs.unlinkSync(path.join(out, file)); } catch { /* none yet */ }
      await page.evaluate((f, g) => { window[g].shot = null; window[g].screenshot(f); }, file, G);
      for (let k = 0; k < 200 && !fs.existsSync(path.join(out, file)); k++) await sleep(100);
      if (!fs.existsSync(path.join(out, file))) console.log(`no ${file} after 20 s`);
      const status = await page.evaluate((g) => { const o = {}; for (const [k, v] of Object.entries(window[g])) if (typeof v !== 'function') o[k] = v; return o; }, G);
      console.log(`${path.relative(root, path.join(out, file))}`, JSON.stringify(status));
      files.push(path.join(out, file));
    }
    return files;
  } finally {
    await browser.close();
  }
}

if (isMain(import.meta.url)) await run();
