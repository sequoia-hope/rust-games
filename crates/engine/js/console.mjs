// The page's console, for a build that does not come up: every distinct
// message (but the warm-up's "still waiting" lines) for `--secs`, then the
// status global's state (ready, backend, state, error). Through the
// registered dev server; `--backend webgl2` turns WebGPU off.
//
//   node js/console.mjs [--backend webgpu|webgl2] [--dist next] [--secs 60]
//       [--query "level=sierra&freeze=1"] [--global __game] [--url|--port]
import { argv, baseUrl, chromeFlags, isMain, launch, sleep, statusGlobal } from './server.mjs';

export async function run({ args = process.argv.slice(2), defaults = {} } = {}) {
  const { opt } = argv(args);
  const backend = opt('--backend', defaults.backend || 'webgl2');
  const dist = opt('--dist', defaults.dist || 'next');
  const query = opt('--query', defaults.query || '');
  const secs = Number(opt('--secs', defaults.secs ?? 60));
  const G = statusGlobal(opt);
  const base = baseUrl(opt);
  const browser = await launch(chromeFlags({ backend, angle: opt('--angle', 'gl') }));
  const skip = /still waiting/;
  try {
    const page = await browser.newPage();
    const seen = new Map();
    page.on('console', (m) => { const t = m.text(); const k = t.slice(0, 200); if (!seen.has(k)) { seen.set(k, 1); if (!skip.test(t)) console.log(`[${m.type()}] ${t.slice(0, 1500)}`); } });
    page.on('pageerror', (e) => console.log(`[pageerror] ${e}`));
    await page.setViewport({ width: 1280, height: 800 });
    const url = `${base}dist/${dist}/index.html?${query}${backend === 'webgl2' ? '&backend=webgl2' : ''}`;
    console.log('open', url);
    await page.goto(url);
    await sleep(secs * 1000);
    const state = await page.evaluate((g) => { const s = window[g]; return JSON.stringify({ ready: s?.ready, backend: s?.backend, state: s?.state, frames: s?.frames, error: s?.error }); }, G);
    console.log('state:', state);
    return JSON.parse(state);
  } finally {
    await browser.close();
  }
}

if (isMain(import.meta.url)) await run();
