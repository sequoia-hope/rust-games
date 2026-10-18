// Hammers the screen changes of the web build in headless Chrome on the
// real GPU, to catch the crashes that come with them: the menu's course,
// mode and rider changes (each course change rebuilds the world), starting
// a run, pausing, restarting and quitting to the menu, in a seeded random
// order, fast and slow. After every step it checks that frames still come
// and that the page logged no error or panic. Exits non-zero on a failure,
// printing the steps that led to it.
//
//   node tools/stress.mjs [--steps 200] [--seed 1] [--backend webgl2] [--touch]
//       [--size 844x390] [--query "assist=1"]
//
// The server's address: --url, else $PORT, else `proj port`; no default.
import puppeteer from 'puppeteer-core';
import { execFileSync } from 'node:child_process';
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
const steps = Number(opt('--steps', 200));
let seed = Number(opt('--seed', 1)) >>> 0;
const rand = () => {
  // mulberry32
  seed = (seed + 0x6d2b79f5) >>> 0;
  let t = seed;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const pick = (xs) => xs[Math.floor(rand() * xs.length)];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const flags = ['--ignore-gpu-blocklist', '--mute-audio', '--enable-gpu', '--use-angle=vulkan',
  '--autoplay-policy=no-user-gesture-required'];
if (backend === 'webgpu') flags.push('--enable-unsafe-webgpu', '--enable-features=Vulkan');
else flags.push('--disable-blink-features=WebGPU', '--disable-features=WebGPU');

const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', headless: 'new', args: flags });
const page = await browser.newPage();
const problems = [];
const IGNORE = [/favicon/, /AudioContext was not allowed/];
page.on('console', (m) => {
  const t = m.text();
  if (m.type() === 'error' && !IGNORE.some((r) => r.test(t))) problems.push(`[console] ${t.slice(0, 600)}`);
  if (/panicked|RuntimeError|unreachable/.test(t)) problems.push(`[panic] ${t.slice(0, 600)}`);
});
page.on('pageerror', (e) => problems.push(`[pageerror] ${String(e).slice(0, 600)}`));
await page.setViewport({ width: w, height: h, hasTouch: flag('--touch'), isMobile: flag('--touch') });
const url = `${baseUrl()}?${opt('--query', '')}${backend === 'webgl2' ? '&backend=webgl2' : ''}`;
console.log('open', url, `(${steps} steps, ${backend})`);
await page.goto(url);
await page.waitForFunction(() => (window.__sb?.frames || 0) > 5 || window.__sb?.state === 'failed', { timeout: 90000 });

const sb = () => page.evaluate(() => ({ ...window.__sb }));
const key = async (k, ms = 40) => {
  await page.keyboard.down(k);
  await sleep(ms);
  await page.keyboard.up(k);
  await sleep(30);
};
const log = [];
async function alive(what) {
  const a = (await sb()).frames;
  await sleep(250);
  const s = await sb();
  if (s.state === 'failed') problems.push(`[failed] ${s.error}`);
  if (!(s.frames > a)) problems.push(`[frozen] no frames for 250 ms after ${what}`);
  return s;
}

const menuMoves = [
  ['course+', ['ArrowDown', 'ArrowRight']],
  ['course-', ['ArrowLeft']],
  ['mode', ['ArrowUp', 'ArrowRight']],
  ['rider', ['ArrowDown', 'ArrowRight']],
  ['assist', ['ArrowDown', 'Enter']],
];

let rides = 0;
const mems = [];
for (let i = 0; i < steps && !problems.length; i++) {
  const s = await sb();
  let what;
  if (s.screen === 'menu') {
    const r = rand();
    if (r < 0.35) {
      // Spin through the courses, fast: a rebuild a frame.
      const n = 1 + Math.floor(rand() * 6);
      for (let k = 0; k < n; k++) await key(pick(['ArrowLeft', 'ArrowRight']), 10);
      what = `menu: ${n} quick setting changes`;
    } else if (r < 0.65) {
      const [name, keys] = pick(menuMoves);
      for (const k of keys) await key(k);
      what = `menu: ${name}`;
    } else {
      // Rows: mode 0, course 1, rider 2, assist 3, radio 4, ride 5. Enter
      // on a setting row rides too (on assist and radio it changes them).
      await key('Enter');
      rides++;
      what = 'menu: ride';
    }
  } else if (s.screen === 'ride') {
    const r = rand();
    if (s.state === 'done') {
      await key(rand() < 0.5 ? 'Enter' : 'ArrowDown');
      if (rand() < 0.5) await key('Enter');
      what = 'results: again or menu';
    } else if (r < 0.3) {
      await sleep(rand() * 2500);
      what = 'ride: wait';
    } else if (r < 0.55) {
      await key('Escape');
      await key('ArrowDown');
      await key('Enter');
      what = 'pause: restart';
    } else if (r < 0.85) {
      await key('Escape');
      await key('ArrowDown');
      await key('ArrowDown');
      await key('Enter');
      what = 'pause: menu';
    } else {
      await key('Escape');
      await sleep(rand() * 600);
      await key('Escape');
      what = 'pause and resume';
    }
  } else {
    what = `waiting (${s.screen})`;
    await sleep(200);
  }
  const after = await alive(what);
  const mem = await page.evaluate(() => window.__sb.memMB?.() ?? 0);
  mems.push(mem);
  log.push(`${i}: ${what} -> ${after.screen}/${after.state} ${after.course} frame ${after.frames} heap ${mem} MB`);
  if (flag('--verbose')) console.log(log[log.length - 1]);
}

// The wasm heap never shrinks, but once every course has been built and
// left it must stop growing: compare the last third's peak with the
// middle third's.
const third = Math.floor(mems.length / 3);
if (third > 10) {
  const mid = Math.max(...mems.slice(third, 2 * third));
  const last = Math.max(...mems.slice(2 * third));
  console.log(`heap: ${mems[0]} MB at the start, ${mid} MB by the middle, ${last} MB at the end`);
  if (last > mid * 1.25 + 16) problems.push(`[memory] the heap kept growing: ${mid} MB -> ${last} MB`);
}
const end = await sb();
await browser.close();
if (problems.length) {
  console.log('\nFAILED. The last steps:');
  for (const l of log.slice(-12)) console.log('  ' + l);
  console.log('\nProblems:');
  for (const p of [...new Set(problems)].slice(0, 20)) console.log('  ' + p);
  process.exit(1);
}
console.log(`ok: ${log.length} steps, ${rides} runs started, ${end.frames} frames, no errors`);
