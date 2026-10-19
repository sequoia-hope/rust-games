// Pictures of the web build, through the registered server, in headless
// Chrome on the real GPU (flags borrowed from midnight-playground's
// tools/parity/lib/server.mjs). Saves PNGs under shots/ (served at /shots/)
// and prints the page's console errors and window.__sb.
//
//   node tools/shot.mjs --query "course=timber&go=1&auto=1&skip=20" --name timber \
//       [--wait 6] [--every 1.5 --count 3] [--backend webgl2] [--size 1280x720] [--touch]
//       [--keys "Space:700,ArrowRight+Shift:900,wait:500"]  (held together with +, for ms, in turn, before the shots)
//
// The server's address: --url, else $PORT, else `proj port`; no default.
import puppeteer from 'puppeteer-core';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const flag = (k) => args.includes(k);
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');

function baseUrl() {
  const given = opt('--url', null);
  if (given) return given.endsWith('/') ? given : given + '/';
  let port = process.env.PORT;
  if (!port) {
    try {
      port = execFileSync('proj', ['port'], { cwd: ROOT, encoding: 'utf8' }).trim();
    } catch (e) {
      throw new Error(`no server address: pass --url, set $PORT, or register the project (proj port failed: ${e.message})`);
    }
  }
  return `http://127.0.0.1:${port}/`;
}

const backend = opt('--backend', 'webgpu');
const [w, h] = opt('--size', '1280x720').split('x').map(Number);
const name = opt('--name', 'shot');
const wait = Number(opt('--wait', 5));
const every = Number(opt('--every', 1));
const count = Number(opt('--count', 1));
const out = path.join(ROOT, 'shots');
fs.mkdirSync(out, { recursive: true });

const flags = ['--ignore-gpu-blocklist', '--mute-audio', '--enable-gpu', '--use-angle=vulkan',
  '--autoplay-policy=no-user-gesture-required'];
if (backend === 'webgpu') flags.push('--enable-unsafe-webgpu', '--enable-features=Vulkan');
else flags.push('--disable-blink-features=WebGPU', '--disable-features=WebGPU');

const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', headless: 'new', args: flags });
const page = await browser.newPage();
const cdp = await page.createCDPSession();
await cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: out });
page.on('console', (m) => { if (['error', 'warn'].includes(m.type())) console.log(`[${m.type()}] ${m.text().slice(0, 400)}`); });
page.on('pageerror', (e) => console.log(`[pageerror] ${String(e).slice(0, 400)}`));
await page.setViewport({ width: w, height: h, hasTouch: flag('--touch'), isMobile: flag('--touch') });
const q = opt('--query', '');
const url = `${baseUrl()}?${q}${backend === 'webgl2' ? '&backend=webgl2' : ''}`;
console.log('open', url);
await page.goto(url);
await page.waitForFunction(() => (window.__sb?.frames || 0) > 5 || window.__sb?.state === 'failed', { timeout: 90000 });
if (flag('--tap')) await page.touchscreen.tap(w * 0.2, h * 0.6);
await new Promise((r) => setTimeout(r, wait * 1000));
for (const step of (opt('--keys', '') || '').split(',').filter(Boolean)) {
  const [names, ms] = step.split(':');
  const keys = names === 'wait' ? [] : names.split('+');
  for (const k of keys) await page.keyboard.down(k);
  await new Promise((r) => setTimeout(r, Number(ms)));
  for (const k of keys.reverse()) await page.keyboard.up(k);
}
for (let i = 0; i < count; i++) {
  if (i) await new Promise((r) => setTimeout(r, every * 1000));
  const file = count > 1 ? `${name}-${i}.png` : `${name}.png`;
  try { fs.unlinkSync(path.join(out, file)); } catch {}
  await page.evaluate((f) => { window.__sb.shot = null; window.__sb.screenshot(f); }, file);
  // The frame arrives as a download.
  for (let k = 0; k < 200 && !fs.existsSync(path.join(out, file)); k++) await new Promise((r) => setTimeout(r, 100));
  if (!fs.existsSync(path.join(out, file))) console.log(`no ${file} after 20 s`);
  const sb = await page.evaluate(() => { const o = {}; for (const [k, v] of Object.entries(window.__sb)) if (typeof v !== 'function') o[k] = v; return o; });
  console.log(`shots/${file}`, JSON.stringify(sb));
}
await browser.close();
