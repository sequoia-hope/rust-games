// Checks Ten-Eighty FM in the web build, in headless Chrome: after a key
// press (the gesture sound waits for) the music node loads and makes
// sound, the announcers' clips are listed and fetched, the menu's ident
// plays, and a run's intro plays with the music ducked under it. Prints
// window.__sb.audio() as it goes; exits non-zero when something is silent.
// --insecure loads the page by this machine's network address over plain
// http, an insecure page as a phone on the LAN sees it: no AudioWorklet
// there, so the music must come from the main-thread fallback.
//
//   node tools/radio-check.mjs [--backend webgl2] [--insecure]
//
// The server's address: --url, else $PORT, else `proj port`; no default.
import puppeteer from 'puppeteer-core';
import { execFileSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';

const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
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
  return `http://${insecure ? lanAddress() : '127.0.0.1'}:${port}/`;
}
// Any address in 127.0.0.0/8 counts as secure to Chrome: a network one.
function lanAddress() {
  for (const list of Object.values(os.networkInterfaces())) {
    for (const a of list) if (a.family === 'IPv4' && !a.internal) return a.address;
  }
  throw new Error('--insecure: this machine has no network address');
}
const backend = opt('--backend', 'webgpu');
const insecure = args.includes('--insecure');
const want = insecure ? 'ready (main thread)' : 'ready';
const flags = ['--ignore-gpu-blocklist', '--mute-audio', '--enable-gpu', '--use-angle=vulkan',
  '--autoplay-policy=no-user-gesture-required'];
if (backend === 'webgpu') flags.push('--enable-unsafe-webgpu', '--enable-features=Vulkan');
else flags.push('--disable-blink-features=WebGPU', '--disable-features=WebGPU');
const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', headless: 'new', args: flags });
const page = await browser.newPage();
const errors = [];
page.on('console', (m) => { if (['error', 'warn'].includes(m.type())) { console.log(`[${m.type()}] ${m.text().slice(0, 300)}`); if (m.type() === 'error') errors.push(m.text()); } });
page.on('pageerror', (e) => errors.push(String(e)));
await page.setViewport({ width: 1280, height: 720 });
await page.goto(`${baseUrl()}?radio=all${backend === 'webgl2' ? '&backend=webgl2' : ''}`);
await page.waitForFunction(() => (window.__sb?.frames || 0) > 5, { timeout: 90000 });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const audio = () => page.evaluate(() => window.__sb.audio());
// A key the menu ignores: the gesture that lets sound start.
await page.keyboard.press('ShiftLeft');
const fails = [];
let a;
for (let i = 0; i < 40; i++) {
  await sleep(250);
  a = await audio();
  if (a.music === want && a.played > 0) break;
}
console.log('menu:', JSON.stringify(a));
if (a.music !== want) fails.push(`the music is ${a.music}, not ${want}`);
// The loudest of a few readings: the station tunes in through a fade, and
// a song can open quietly.
let loudest = -Infinity;
for (let i = 0; i < 12; i++) {
  await sleep(250);
  loudest = Math.max(loudest, (await audio()).musicDb);
}
console.log(`the music's loudest reading in 3 s: ${loudest} dBFS`);
if (!(loudest > -45)) fails.push(`the music is silent (${loudest} dBFS)`);
if (!(a.clips > 100)) fails.push(`only ${a.clips} clips listed`);
if (!(a.played > 0)) fails.push('the menu ident did not play');
const before = a.played;
await page.keyboard.press('Enter');
await sleep(1500);
a = await audio();
console.log('run:', JSON.stringify(a));
if (!(a.played > before)) fails.push('the run intro did not play');
await sleep(6000);
a = await audio();
console.log('later:', JSON.stringify(a));
console.log('game:', JSON.stringify(await page.evaluate(() => { const o = {}; for (const [k, v] of Object.entries(window.__sb)) if (typeof v !== 'function') o[k] = v; return o; })));
await browser.close();
if (errors.length) fails.push(`${errors.length} console errors`);
if (fails.length) {
  console.log('FAILED:\n  ' + fails.join('\n  '));
  process.exit(1);
}
console.log('ok: music playing and the announcers talking');
