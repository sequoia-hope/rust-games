// Pixel comparison of two builds on WebGL2 (a WebGL2 canvas composites
// into Chrome's own screenshots; a WebGPU one does not): the same frozen
// view of each case from dist/<a>/ and dist/<b>/, through the registered
// dev server, compared pixel for pixel. `dist/prof/` (`cargo xtask prof`,
// a copy of the previous live build) is the usual "before". Prints, per
// case, how many pixels differ and the largest channel difference; writes
// the shots and a red overlay of the differing pixels to `--out`.
//
//   node js/build-diff.mjs [--a prof] [--b next] [--cases sierra,coast] [--param level]
//       [--query "s=300&freeze=1"] [--wait "<js>"] [--out report/build-diff]
//       [--global __game] [--url|--port]
//
// Each case is one value of `--param` (default `level`) added to
// `--query`; a lone case with no `--param` value compares the page as
// `--query` opens it. A shot is taken 1.5 s after the JS expression
// `--wait` (default: `ready`) is truthy.

import fs from 'node:fs';
import path from 'node:path';
import { decode, encode } from './png.mjs';
import { argv, baseUrl, chromeFlags, isMain, launch, projectRoot, sleep, statusGlobal } from './server.mjs';

export async function run({ args = process.argv.slice(2), defaults = {}, root = projectRoot() } = {}) {
  const { opt } = argv(args);
  const G = statusGlobal(opt);
  const A = opt('--a', defaults.a || 'prof'), B = opt('--b', defaults.b || 'next');
  const cases = (opt('--cases', defaults.cases || '')).split(',');
  const param = opt('--param', defaults.param || 'level');
  const query = opt('--query', defaults.query || '');
  const withG = (expr) => `((G) => (${expr}))(window[${JSON.stringify(G)}])`;
  const waitExpr = opt('--wait', defaults.wait || 'G && G.ready');
  const out = path.resolve(root, opt('--out', defaults.out || 'report/build-diff'));
  fs.mkdirSync(out, { recursive: true });
  const base = baseUrl(opt, root);
  const results = [];
  const browser = await launch(chromeFlags({ backend: 'webgl2', angle: 'gl' }), { root });
  try {
    for (const c of cases) {
      const q = new URLSearchParams(query);
      if (c) q.set(param, c);
      q.set('backend', 'webgl2');
      const name = c || 'page';
      const shots = {};
      for (const dist of [A, B]) {
        const page = await browser.newPage();
        page.on('pageerror', (e) => console.error(`[${dist} ${name} page error] ${e}`));
        await page.setViewport({ width: 1280, height: 800, deviceScaleFactor: 1 });
        await page.goto(`${base}dist/${dist}/index.html?${q}`);
        await page.waitForFunction(withG(waitExpr), { timeout: 300000, polling: 200 });
        await sleep(1500);
        const file = path.join(out, `${name}-${dist}.png`);
        await page.screenshot({ path: file });
        shots[dist] = decode(fs.readFileSync(file));
        await page.close();
      }
      const a = shots[A], b = shots[B];
      if (a.w !== b.w || a.h !== b.h) throw new Error(`${name}: sizes differ`);
      let diff = 0, maxd = 0;
      const over = Buffer.alloc(a.w * a.h * 3);
      for (let i = 0, j = 0, o = 0; i < a.px.length; i += a.bpp, j += b.bpp, o += 3) {
        const d = Math.max(Math.abs(a.px[i] - b.px[j]), Math.abs(a.px[i + 1] - b.px[j + 1]), Math.abs(a.px[i + 2] - b.px[j + 2]));
        if (d) { diff++; if (d > maxd) maxd = d; over[o] = 255; } else { over[o] = b.px[j] / 3; over[o + 1] = b.px[j + 1] / 3; over[o + 2] = b.px[j + 2] / 3; }
      }
      fs.writeFileSync(path.join(out, `${name}-diff.png`), encode(a.w, a.h, over));
      console.log(`${name}: ${a.w}x${a.h}, ${diff} pixels differ (${(100 * diff / (a.w * a.h)).toFixed(3)}%), max channel difference ${maxd}`);
      results.push({ name, w: a.w, h: a.h, diff, maxd });
    }
  } finally { await browser.close(); }
  return results;
}

if (isMain(import.meta.url)) await run();
