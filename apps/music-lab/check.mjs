// The Music Lab in headless Chrome, with no server and no port: the built
// site (dist/, from ./build.sh) answered by request interception. Makes a
// track, plays it and waits for the engine to report it playing, plays the
// keyboard and a pad, changes genre, measures a song offline, and fails on
// any page error or a message the engine refused.
//
//   ./build.sh && node check.mjs
// puppeteer-core comes from Midnight Playground's node_modules (the kit's
// harness looks there too); PK_PROJECT_ROOT points elsewhere.

import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromeFlags, launch, sleep } from '../../crates/engine/js/server.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const dist = path.join(here, 'dist');
const root = process.env.PK_PROJECT_ROOT || path.join(here, '../../games/midnight-playground');
const ORIGIN = 'https://music-lab.test/';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };

const browser = await launch(chromeFlags({ backend: 'webgl2', gesture: true }), { root });
const errors = [];
let failed = false;
const check = (ok, what) => { console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`); if (!ok) failed = true; };
try {
  const page = await browser.newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.setRequestInterception(true);
  page.on('request', async (req) => {
    const url = new URL(req.url());
    if (url.origin + '/' !== ORIGIN) return req.abort();
    const file = path.join(dist, decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname));
    try {
      req.respond({ status: 200, contentType: TYPES[path.extname(file)] ?? 'application/octet-stream', body: await fs.readFile(file) });
    } catch { req.respond({ status: 404, body: 'not found' }); }
  });
  await page.goto(ORIGIN + '?genre=techno&seed=42');
  await page.waitForFunction(() => window.musicLab?.cur, { timeout: 30000 });
  const first = await page.evaluate(() => ({ genre: window.musicLab.cur.genre, title: window.musicLab.cur.track.title, parts: Object.keys(window.musicLab.cur.track.parts).length }));
  check(first.genre === 'techno' && first.parts > 0, `made a track from the query: ${first.title}, ${first.parts} parts`);

  await page.click('#play');
  const playing = await page.waitForFunction(() => window.musicLab.pos?.playing && window.musicLab.pos.barIndex >= 1, { timeout: 30000 }).then(() => true, () => false);
  check(playing, 'plays: the engine reports the song moving');
  const level = await page.evaluate(async () => {
    const a = window.musicLab.ctx.createAnalyser();
    window.musicLab.lab.connect(a);
    const buf = new Float32Array(a.fftSize);
    let peak = 0;
    for (let i = 0; i < 20; i++) { await new Promise((r) => setTimeout(r, 50)); a.getFloatTimeDomainData(buf); for (const x of buf) peak = Math.max(peak, Math.abs(x)); }
    return peak;
  });
  check(level > 0.01, `is heard: peak ${level.toFixed(3)}`);

  await page.click('#play'); // stop
  await page.select('#genre', 'trance');
  await page.click('#make');
  await sleep(500);
  const second = await page.evaluate(() => window.musicLab.cur.genre);
  check(second === 'trance', 'Make: a trance track');

  // The new generator: a song with layers and notes, playing.
  await page.select('#genre', 'chicago');
  await page.click('#make');
  const fresh = await page.waitForFunction(() => window.musicLab.cur.song && window.musicLab.pos?.playing && window.musicLab.pos.barIndex >= 1, { timeout: 30000 }).then(() => true, () => false);
  const layers = await page.evaluate(() => ({ n: window.musicLab.cur.song.layers.length, shown: !document.querySelector('#layersCard').hidden, notes: document.querySelectorAll('#notes li').length }));
  check(fresh && layers.n >= 6 && layers.shown && layers.notes >= 5, `new generator: a Chicago house song plays, ${layers.n} layers drawn, ${layers.notes} notes`);
  await page.click('#play'); // stop

  await page.select('#genre', 'orchestral');
  await page.click('#make');
  const orch = await page.waitForFunction(() => window.musicLab.cur.genre === 'orchestral' && window.musicLab.cur.song && window.musicLab.pos?.playing && window.musicLab.pos.barIndex >= 1, { timeout: 30000 }).then(() => true, () => false);
  const orchInfo = await page.evaluate(() => ({ title: window.musicLab.cur.track.title, n: window.musicLab.cur.song.layers.length }));
  check(orch && orchInfo.n >= 8, `orchestral mode: ${orchInfo.title} plays, ${orchInfo.n} layers`);
  await page.click('#play'); // stop

  await page.click('#demo');
  await page.click('#pads button');
  await sleep(1500);

  await page.click('#measure');
  const measured = await page.waitForFunction(() => document.querySelector('#mNote').textContent.startsWith('Done') || document.querySelector('#mNote').textContent.startsWith('Failed'), { timeout: 120000 }).then(() => page.$eval('#mNote', (e) => e.textContent), (e) => String(e));
  const rows = await page.$$eval('#mTable tr', (rs) => rs.slice(1).map((r) => [...r.children].map((c) => c.textContent)));
  check(measured.startsWith('Done') && rows.length === 4 && rows.every((r) => Number(r[2]) > -60), `measured offline: ${rows.map((r) => `${r[1]} ${r[2]} dB`).join(', ')}`);

  const status = await page.$eval('#status', (e) => e.textContent);
  check(!/refused|failed|stopped with an error/i.test(status), `status: ${status}`);
  check(errors.length === 0, `no page errors${errors.length ? ': ' + errors.join(' | ') : ''}`);
} finally {
  await browser.close();
}
process.exit(failed ? 1 : 0);
