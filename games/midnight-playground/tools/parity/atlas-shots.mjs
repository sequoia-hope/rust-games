// Pictures of the atlas (DECISIONS D1246) on a cruise of the open world:
// the minimap, the map opened with G (the whole map), Z (3 km round the
// car), the wheel zoomed in, and the minimap zoomed out; on a desktop
// window and on a phone held sideways (the touch layout); then an event's
// route.
//
//   node tools/parity/atlas-shots.mjs [--start san-mateo] [--backend webgl2]
//        [--dist next] [--out <dir>] [--url <base> | --port N]
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';
import { ROOT } from './lib/jstree.mjs';
import { baseUrl, chromeFlags } from './lib/server.mjs';

const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const backend = opt('--backend', 'webgl2');
const start = opt('--start', 'san-mateo');
const out = opt('--out', path.join(ROOT, 'parity/report/atlas'));
fs.mkdirSync(out, { recursive: true });
const base = baseUrl(opt);
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', headless: 'new', args: chromeFlags({ backend, race: true }) });
const sizes = [
  { name: 'desktop', width: 1280, height: 720, dpr: 1, touch: '0' },
  { name: 'phone', width: 844, height: 390, dpr: 2, touch: '1' },
];
for (const sz of sizes) {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text().slice(0, 300)); });
  await page.setViewport({ width: sz.width, height: sz.height, deviceScaleFactor: sz.dpr });
  const q = new URLSearchParams({ level: 'peninsula', cruise: '1', start, autostart: 'sports', seed: '1', pursuit: '0', touch: sz.touch });
  await page.goto(`${base}dist/${opt('--dist', 'next')}/index.html?${q}`);
  await page.waitForFunction('(window.__mp?.race?.time ?? -1) >= 0.5', { timeout: 240000, polling: 200 });
  // The fields build a slice a frame from the race's start.
  await wait(4000);
  const shot = async (k) => { await wait(400); await page.screenshot({ path: path.join(out, `${sz.name}-${k}.png`) }); console.log(`${sz.name}-${k}.png`); };
  await shot('0-minimap');
  await page.keyboard.press('KeyG');
  await shot('1-whole');
  await page.keyboard.press('KeyZ');
  await shot('2-region');
  if (sz.touch === '0') {
    await page.mouse.move(sz.width * 0.5, sz.height * 0.5);
    for (let i = 0; i < 6; i++) { await page.mouse.wheel({ deltaY: -120 }); await wait(30); }
    await shot('3-closer');
  }
  await page.keyboard.press('KeyG');
  await page.keyboard.press('KeyZ');
  await shot('4-minimap-wide');
  if (errors.length) console.log(`${sz.name} errors:\n  ${errors.slice(0, 8).join('\n  ')}`);
  await page.close();
}
// An event from the crest's start (the cruise's own start sits on it):
// hold gas and brake, then the map shows the route to its finish.
{
  const page = await browser.newPage();
  await page.setViewport({ width: 1280, height: 720 });
  const q = new URLSearchParams({ level: 'peninsula', cruise: '1', autostart: 'sports', seed: '1', pursuit: '0', touch: '0' });
  await page.goto(`${base}dist/${opt('--dist', 'next')}/index.html?${q}`);
  await page.waitForFunction('(window.__mp?.race?.time ?? -1) >= 0.5', { timeout: 240000, polling: 200 });
  await wait(3000);
  await page.keyboard.down('ArrowUp');
  await page.keyboard.down('ArrowDown');
  await wait(1800);
  await page.keyboard.up('ArrowUp');
  await page.keyboard.up('ArrowDown');
  await wait(6000);
  const ev = await page.evaluate(() => window.__mp?.race?.event);
  console.log(`event: ${ev}`);
  await page.keyboard.press('KeyG');
  await wait(1500);
  await page.screenshot({ path: path.join(out, 'event-1-whole.png') });
  console.log('event-1-whole.png');
  await page.close();
}
await browser.close();
