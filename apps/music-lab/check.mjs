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

  // Autoplay with multi-genre select and fine-grained key/tempo controls.
  await page.click('#selNone');
  await page.click('#autoGenres button[data-g="french"]');
  await page.click('#autoGenres button[data-g="acid"]');
  await page.select('#key', '9'); // A
  await page.click('#autoplay');
  const autoOn = await page.waitForFunction(() => window.musicLab.autoplaying && window.musicLab.playing && ['french', 'acid'].includes(window.musicLab.cur.genre), { timeout: 30000 }).then(() => true, () => false);
  const autoFirst = await page.evaluate(() => window.musicLab.cur.genre);
  await page.click('#nextSong');
  const autoSecond = await page.evaluate(() => window.musicLab.cur.genre);
  check(autoOn && ['french', 'acid'].includes(autoSecond) && autoSecond !== autoFirst, `autoplay multi-genre select: ${autoFirst} → ${autoSecond}`);
  await page.click('#play'); // stop

  const status = await page.$eval('#status', (e) => e.textContent);
  check(!/refused|failed|stopped with an error/i.test(status), `status: ${status}`);

  // Seamlessly open the RP2350 Hardware Groovebox from the player and go back and forth with zero audio hiccup.
  // Note: Puppeteer's setViewport reloads the page when toggling isMobile/hasTouch, so set viewport before starting playback.
  await page.setViewport({ width: 844, height: 390, isMobile: true, hasTouch: true, isLandscape: true });
  await page.waitForFunction(() => window.musicLab?.cur, { timeout: 30000 });
  await page.select('#genre', 'synthwave');
  await page.click('#make');
  await page.waitForFunction(() => window.musicLab.cur?.genre === 'synthwave' && window.musicLab.pos?.playing && window.musicLab.pos.barIndex >= 1, { timeout: 30000 });
  const beforeHw = await page.evaluate(() => ({
    genre: window.musicLab.cur.genre,
    seed: window.musicLab.cur.seed,
    title: window.musicLab.cur.track.title,
    chords: window.groovebox.dna.chords,
    bar: window.musicLab.pos.barIndex,
  }));
  await page.click('#openHardware');
  const hwPlaying = await page.waitForFunction(
    () => window.musicLab.view === 'groovebox' && window.groovebox?.playing && window.groovebox?.pos?.playing && window.groovebox?.song?.layers?.length > 0,
    { timeout: 30000 },
  ).then(() => true, () => false);
  await page.click('#bassRow button[data-bass="acid"]');
  await page.click('#chordStyleBtn');
  await sleep(300);
  const hwState = await page.evaluate(() => ({
    genre: window.groovebox.genre,
    seed: window.groovebox.seed,
    title: window.groovebox.song.title,
    bass: window.groovebox.dna.bass,
    chords: window.groovebox.dna.chords,
    sameCtx: window.groovebox.ctx === window.musicLab.ctx && window.groovebox.lab === window.musicLab.lab,
    hasPatterns: window.groovebox.song.layers.some((l) => l.patterns?.length > 0),
  }));
  const expectedChords = beforeHw.chords === 'stabs' ? 'sustain' : 'stabs';
  check(
    hwPlaying && hwState.sameCtx && hwState.genre === beforeHw.genre && hwState.seed === beforeHw.seed && hwState.title === beforeHw.title && hwState.bass === 'acid' && hwState.chords === expectedChords && hwState.hasPatterns,
    `hardware groovebox (iPhone landscape): seamlessly opens exact playing preset and morphs Genre DNA (${hwState.title}, bass=${hwState.bass}, chords=${hwState.chords})`,
  );

  // Freely go back to player and back to groovebox while audio continues uninterrupted.
  await page.click('#backLab');
  const backInLab = await page.evaluate(() => ({
    view: window.musicLab.view,
    playing: window.musicLab.playing && window.musicLab.pos?.playing,
    genre: window.musicLab.cur.genre,
    seed: window.musicLab.cur.seed,
    bassOverride: window.musicLab.cur.dnaOverrides?.bass,
  }));
  await page.click('#openHardware');
  const backInHw = await page.evaluate(() => ({
    view: window.musicLab.view,
    playing: window.groovebox.playing && window.groovebox.pos?.playing,
    genre: window.groovebox.genre,
    seed: window.groovebox.seed,
    bass: window.groovebox.dna.bass,
  }));
  check(
    backInLab.view === 'lab' && backInLab.playing && backInLab.bassOverride === 'acid' && backInHw.view === 'groovebox' && backInHw.playing && backInHw.bass === 'acid',
    `seamless round-trip player ↔ groovebox preserves live playback and morphed preset (${backInHw.genre} #${backInHw.seed})`,
  );
  await page.click('#playBtn'); // stop

  check(errors.length === 0, `no page errors${errors.length ? ': ' + errors.join(' | ') : ''}`);
} finally {
  await browser.close();
}
process.exit(failed ? 1 : 0);
