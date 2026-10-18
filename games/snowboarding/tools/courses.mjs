// Rides every course of the web build in headless Chrome on the real GPU
// and fails on what kills a phone's tab though no browser here minds:
// besides errors, panics and freezes, a heap that grows through the ride,
// and GPU buffers made afresh every frame. Avalanche Bowl's falling snow
// was a new 200 KB buffer a frame (a mesh rebuilt on the CPU), and the
// owner's iPhone killed the tab within seconds of the course appearing
// (2026-10-18) while Chrome rode it happily. Counted here by wrapping the
// page's own calls: WebGL2's bufferData, WebGPU's createBuffer.
//
// Two limits on the new GPU buffers a frame: no course may make more than
// 16 KB a frame beyond the course that makes the least (that was the
// snow), and none more than 64 KB. What is left is Bevy's (measured
// 2026-10-18): on WebGL2 its CPU light clustering makes two 16 KB uniform
// buffers for the view every frame, even with nothing to cluster; on
// WebGPU the same is 36 bytes, plus each view's mesh uniforms, re-made at
// exactly their size whenever more meshes are in sight than before (until
// the first run of a course has seen them all). WebGPU's GPU clustering,
// 500 KB a frame, is off (`camera::no_light_clustering`).
//
//   node tools/courses.mjs [--backend webgl2] [--seconds 8] [--size 844x390 --touch]
//       [--courses crystal,avalanche] [--excess-kb 16] [--ceiling-kb 64]
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
const seconds = Number(opt('--seconds', 8));
const courses = opt('--courses', 'crystal,timber,avalanche,pipe').split(',');
const excessKB = Number(opt('--excess-kb', 16));
const ceilingKB = Number(opt('--ceiling-kb', 64));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const flags = ['--ignore-gpu-blocklist', '--mute-audio', '--enable-gpu', '--use-angle=vulkan',
  '--autoplay-policy=no-user-gesture-required'];
if (backend === 'webgpu') flags.push('--enable-unsafe-webgpu', '--enable-features=Vulkan');
else flags.push('--disable-blink-features=WebGPU', '--disable-features=WebGPU');

// Counts the bytes of every GPU buffer the page makes (or re-makes: a
// bufferData is a new store), into window.__gpu.
function countBuffers() {
  const c = (window.__gpu = { made: 0, calls: 0 });
  const wrap = (proto, name, size) => {
    if (!proto || !proto[name]) return;
    const orig = proto[name];
    proto[name] = function (...a) {
      c.made += size(a);
      c.calls++;
      return orig.apply(this, a);
    };
  };
  if (typeof WebGL2RenderingContext !== 'undefined') {
    wrap(WebGL2RenderingContext.prototype, 'bufferData', (a) =>
      typeof a[1] === 'number' ? a[1] : (a[1]?.byteLength ?? 0));
  }
  if (typeof GPUDevice !== 'undefined') wrap(GPUDevice.prototype, 'createBuffer', (a) => a[0]?.size ?? 0);
}

const browser = await puppeteer.launch({ executablePath: '/usr/bin/google-chrome', headless: 'new', args: flags });
const IGNORE = [/favicon/, /AudioContext was not allowed/];
const problems = [];
const rates = [];
for (const course of courses) {
  const page = await browser.newPage();
  const mine = [];
  page.on('console', (m) => {
    const t = m.text();
    if (m.type() === 'error' && !IGNORE.some((r) => r.test(t))) mine.push(`[console] ${t.slice(0, 600)}`);
    if (/panicked|RuntimeError|unreachable/.test(t)) mine.push(`[panic] ${t.slice(0, 600)}`);
  });
  page.on('pageerror', (e) => mine.push(`[pageerror] ${String(e).slice(0, 600)}`));
  await page.evaluateOnNewDocument(countBuffers);
  await page.setViewport({ width: w, height: h, hasTouch: flag('--touch'), isMobile: flag('--touch') });
  const url = `${baseUrl()}?course=${course}&go=1&auto=1${backend === 'webgl2' ? '&backend=webgl2' : ''}`;
  await page.goto(url);
  await page.waitForFunction(() => (window.__sb?.frames || 0) > 5 || window.__sb?.state === 'failed', { timeout: 90000 });
  const at = () => page.evaluate(() => ({ ...window.__sb, mem: window.__sb.memMB?.() ?? 0, gpu: { ...window.__gpu } }));
  // Past the first frames, where everything is made once.
  await sleep(2000);
  const a = await at();
  const mems = [a.mem];
  for (let i = 0; i < seconds; i++) {
    await sleep(1000);
    mems.push((await at()).mem);
  }
  const b = await at();
  const frames = b.frames - a.frames;
  const kb = (b.gpu.made - a.gpu.made) / Math.max(1, frames) / 1024;
  const grew = Math.max(...mems) - mems[0];
  console.log(`${backend} ${course}: ${b.state}, ${frames} frames, ${kb.toFixed(1)} KB of new GPU buffers a frame, ` +
    `heap ${mems[0]} -> ${Math.max(...mems)} MB`);
  if (b.state === 'failed') mine.push(`[failed] ${b.error}`);
  if (b.screen !== 'ride') mine.push(`[screen] on ${b.screen}, not riding`);
  if (frames < seconds * 10) mine.push(`[frozen] ${frames} frames in ${seconds} s`);
  if (kb > ceilingKB) mine.push(`[churn] ${kb.toFixed(1)} KB of new GPU buffers a frame (ceiling ${ceilingKB} KB): something is re-made every frame`);
  rates.push([course, kb]);
  if (grew > 6) mine.push(`[memory] the heap grew ${grew.toFixed(1)} MB through the ride`);
  problems.push(...[...new Set(mine)].map((p) => `${course}: ${p}`));
  await page.close();
}
await browser.close();
const [least, min] = rates.reduce((a, b) => (b[1] < a[1] ? b : a));
for (const [course, kb] of rates) {
  if (kb - min > excessKB) {
    problems.push(`${course}: [churn] ${(kb - min).toFixed(1)} KB of new GPU buffers a frame more than ${least} ` +
      `(${kb.toFixed(1)} against ${min.toFixed(1)}; at most ${excessKB}): something of this course's is re-made every frame`);
  }
}
if (problems.length) {
  console.log('\nFAILED:');
  for (const p of problems.slice(0, 20)) console.log('  ' + p);
  process.exit(1);
}
console.log(`ok: ${courses.length} courses ridden on ${backend}`);
