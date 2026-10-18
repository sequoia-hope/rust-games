// Measures the wasm heap across repeated screen changes, to find what
// leaks. Scenarios: course (course changes in the menu), ride (ride and
// quit), restart (restart from pause), long (one long ride). Fails when a
// scenario's heap grows more than 12 MB after its first step: the heap of
// a wasm never shrinks, and a phone's browser kills a tab that keeps
// growing (an 8 MB-a-run leak of glyph atlases did, 2026-10-16).
//   node tools/heap-probe.mjs [webgl2|webgpu] [scenario...] [--query q]
// The server's address: $PORT, else `proj port`; no default.
import puppeteer from 'puppeteer-core';
const args = process.argv.slice(2);
const backend = ['webgl2', 'webgpu'].includes(args[0]) ? args.shift() : 'webgl2';
const qi = args.indexOf('--query');
const query = qi >= 0 ? args.splice(qi, 2)[1] : '';
const scenarios = args.length ? args : ['course', 'ride', 'restart', 'long'];
const flags = ['--ignore-gpu-blocklist', '--mute-audio', '--enable-gpu', '--use-angle=vulkan'];
if (backend === 'webgpu') flags.push('--enable-unsafe-webgpu', '--enable-features=Vulkan');
else flags.push('--disable-blink-features=WebGPU', '--disable-features=WebGPU');
import { execFileSync } from 'node:child_process';
const port = process.env.PORT || (() => {
  try {
    return execFileSync('proj', ['port'], { cwd: new URL('..', import.meta.url).pathname, encoding: 'utf8' }).trim();
  } catch (e) {
    throw new Error(`no server address: set $PORT, or register the project (proj port failed: ${e.message})`);
  }
})();
let failed = false;
const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', headless: 'new', args: flags });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
for (const sc of scenarios) {
  const page = await browser.newPage();
  page.on('console', (m) => { if (m.type() === 'error') console.log('[err]', m.text().slice(0, 300)); });
  await page.setViewport({ width: 1280, height: 720 });
  await page.goto(`http://127.0.0.1:${port}/?backend=${backend}&${query}`);
  await page.waitForFunction(() => (window.__sb?.frames || 0) > 5, { timeout: 90000 });
  const key = async (k) => { await page.keyboard.down(k); await sleep(50); await page.keyboard.up(k); await sleep(50); };
  const mem = () => page.evaluate(() => window.__sb.memMB());
  const out = [await mem()];
  if (sc === 'course') {
    for (const k of ['ArrowUp', 'ArrowUp', 'ArrowUp']) await key(k);
    for (let i = 0; i < 16; i++) { await key('ArrowRight'); await sleep(300); out.push(await mem()); }
  } else if (sc === 'ride') {
    for (let i = 0; i < 10; i++) {
      await key('Enter'); await sleep(1500);
      await key('KeyP'); await key('ArrowDown'); await key('ArrowDown'); await key('Enter'); await sleep(500);
      out.push(await mem());
    }
  } else if (sc === 'restart') {
    await key('Enter'); await sleep(1000);
    for (let i = 0; i < 10; i++) { await key('KeyP'); await key('ArrowDown'); await key('Enter'); await sleep(1500); out.push(await mem()); }
  } else if (sc === 'long') {
    await key('Enter');
    for (let i = 0; i < 10; i++) { await sleep(1500); out.push(await mem()); }
  }
  const grew = out[out.length - 1] - out[1];
  console.log(`${backend} ${sc}: ${out.join(' ')} MB${grew > 12 ? `  GREW ${grew.toFixed(1)} MB` : ''}`);
  if (grew > 12) failed = true;
  await page.close();
}
await browser.close();
if (failed) process.exit(1);
