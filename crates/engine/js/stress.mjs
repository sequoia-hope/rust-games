// Hammers the web build's screen changes in headless Chrome on the real
// GPU, to catch the crashes that come with them: a seeded random walk over
// a game's step catalogue (menu changes, starting a run, pausing,
// restarting, quitting to the menu), fast and slow. After every step it
// checks that frames still come and that the page logged no error or
// panic, and that the wasm heap stops growing once everything has been
// built once. Exits non-zero on a failure, printing the steps that led to it.
//
//   node js/stress.mjs [--steps 200] [--seed 1] [--backend webgl2] [--touch]
//       [--size 844x390] [--query "assist=1"] [--dist next] [--global __game] [--url|--port] [--verbose]
//
// The catalogue is a parameter: `run({ steps: async (s, t) => what })`,
// given the status global's snapshot `s` and the tools `t` ({ page, key,
// rand, pick, sleep }), does one step and returns what it did. Without
// one, the walk presses random menu keys (arrows, Enter, Escape).

import { argv, baseUrl, chromeFlags, isMain, launch, projectRoot, sleep, statusGlobal } from './server.mjs';

const KEYS = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Enter', 'Escape'];
const keyboardChaos = async (s, { key, rand, pick }) => {
  const n = 1 + Math.floor(rand() * 4);
  for (let i = 0; i < n; i++) await key(pick(KEYS), rand() < 0.3 ? 10 : 60);
  return `${n} random keys on ${s.screen || s.state}`;
};

export async function run({ args = process.argv.slice(2), defaults = {}, root = projectRoot(), steps: catalogue = null, ignore = [] } = {}) {
  const { opt, flag } = argv(args);
  const G = statusGlobal(opt);
  const backend = opt('--backend', defaults.backend || 'webgpu');
  const [w, h] = opt('--size', '1280x720').split('x').map(Number);
  const steps = Number(opt('--steps', defaults.steps ?? 200));
  const dist = opt('--dist', defaults.dist || 'next');
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
  const query = new URLSearchParams(defaults.query || '');
  for (const [k, v] of new URLSearchParams(opt('--query', ''))) query.set(k, v);
  if (backend === 'webgl2') query.set('backend', 'webgl2');

  const browser = await launch(chromeFlags({ backend, gesture: true, capped: true }), { root });
  const page = await browser.newPage();
  const problems = [];
  const IGNORE = [/favicon/, /AudioContext was not allowed/, ...ignore];
  page.on('console', (m) => {
    const t = m.text();
    if (m.type() === 'error' && !IGNORE.some((r) => r.test(t))) problems.push(`[console] ${t.slice(0, 600)}`);
    if (/panicked|RuntimeError|unreachable/.test(t)) problems.push(`[panic] ${t.slice(0, 600)}`);
  });
  page.on('pageerror', (e) => problems.push(`[pageerror] ${String(e).slice(0, 600)}`));
  await page.setViewport({ width: w, height: h, hasTouch: flag('--touch'), isMobile: flag('--touch') });
  const url = `${baseUrl(opt, root)}dist/${dist}/index.html?${query}`;
  console.log('open', url, `(${steps} steps, ${backend})`);
  await page.goto(url);
  await page.waitForFunction((g) => (window[g]?.frames || 0) > 5 || window[g]?.state === 'failed', { timeout: 90000 }, G);

  // The status global's plain fields.
  const status = () => page.evaluate((g) => { const o = {}; for (const [k, v] of Object.entries(window[g] || {})) if (typeof v !== 'function') o[k] = v; return o; }, G);
  const key = async (k, ms = 40) => {
    await page.keyboard.down(k);
    await sleep(ms);
    await page.keyboard.up(k);
    await sleep(30);
  };
  const log = [];
  async function alive(what) {
    const a = (await status()).frames;
    await sleep(250);
    const s = await status();
    if (s.state === 'failed' || s.state === 'crashed') problems.push(`[${s.state}] ${s.error}`);
    if (!(s.frames > a)) problems.push(`[frozen] no frames for 250 ms after ${what}`);
    return s;
  }
  const tools = { page, key, rand, pick, sleep };
  const step = catalogue || keyboardChaos;

  const mems = [];
  for (let i = 0; i < steps && !problems.length; i++) {
    const s = await status();
    const what = await step(s, tools);
    const after = await alive(what);
    const mem = await page.evaluate((g) => Math.round((window[g].wasmMemoryBytes?.() ?? 0) / 1048576), G);
    mems.push(mem);
    log.push(`${i}: ${what} -> ${after.screen}/${after.state} frame ${after.frames} heap ${mem} MB`);
    if (flag('--verbose')) console.log(log[log.length - 1]);
  }

  // The wasm heap never shrinks, but once everything has been built and
  // left it must stop growing: compare the last third's peak with the
  // middle third's.
  const third = Math.floor(mems.length / 3);
  if (third > 10) {
    const mid = Math.max(...mems.slice(third, 2 * third));
    const last = Math.max(...mems.slice(2 * third));
    console.log(`heap: ${mems[0]} MB at the start, ${mid} MB by the middle, ${last} MB at the end`);
    if (last > mid * 1.25 + 16) problems.push(`[memory] the heap kept growing: ${mid} MB -> ${last} MB`);
  }
  const end = await status();
  await browser.close();
  if (problems.length) {
    console.log('\nFAILED. The last steps:');
    for (const l of log.slice(-12)) console.log('  ' + l);
    console.log('\nProblems:');
    for (const p of [...new Set(problems)].slice(0, 20)) console.log('  ' + p);
    process.exitCode = 1;
    return { ok: false, problems, log };
  }
  console.log(`ok: ${log.length} steps, ${end.frames} frames, no errors`);
  return { ok: true, log };
}

if (isMain(import.meta.url)) await run();
