// A picture of the whole page (canvas, panels, inspector) for the dev log,
// through the registered server: WebGL2, which headless Chrome composites
// into its own screenshots (shot.mjs's frame download has the canvas only).
// Steps run in turn after the page is up: `wait:<ms>`, `stage:<json>` (a
// client command, as the panel sends), `click:<css selector>`.
//
//   node tools/page-shot.mjs --url http://127.0.0.1:<port>/ --query "mode=impact" --name impact \
//       --steps 'stage:{"cmd":"walk","forward":0.8},wait:6000' [--size 1440x900]
import fs from 'node:fs';
import path from 'node:path';
import { argv, baseUrl, chromeFlags, launch, projectRoot, sleep } from '../../../crates/engine/js/server.mjs';

const { opt } = argv(process.argv.slice(2));
const root = projectRoot();
const [w, h] = opt('--size', '1440x900').split('x').map(Number);
const name = opt('--name', 'page');
const out = path.resolve(root, opt('--out', 'shots'));
fs.mkdirSync(out, { recursive: true });
const query = new URLSearchParams(opt('--query', ''));
query.set('backend', 'webgl2');

const browser = await launch(chromeFlags({ backend: 'webgl2', gesture: true, capped: true }), { root });
try {
  const page = await browser.newPage();
  page.on('console', (m) => { if (m.type() === 'error') console.log(`[error] ${m.text().slice(0, 300)}`); });
  page.on('pageerror', (e) => console.log(`[pageerror] ${String(e).slice(0, 300)}`));
  await page.setViewport({ width: w, height: h });
  const url = `${baseUrl(opt, root)}?${query}`;
  console.log('open', url);
  await page.goto(url);
  await page.waitForFunction(() => window.__mech?.state === 'running' || window.__mech?.state === 'failed', { timeout: 90000 });
  await sleep(2500);
  // Commas inside a JSON step are its own: split only before a step name.
  for (const step of (opt('--steps', '') || '').split(/,(?=(?:wait|stage|click):)/).filter(Boolean)) {
    const at = step.indexOf(':');
    const [kind, arg] = [step.slice(0, at), step.slice(at + 1)];
    if (kind === 'wait') await sleep(Number(arg));
    else if (kind === 'stage') await page.evaluate((c) => window.__mech.stage(JSON.parse(c)), arg);
    else if (kind === 'click') await page.click(arg);
  }
  const file = path.join(out, `${name}.png`);
  await page.screenshot({ path: file });
  console.log(file, await page.evaluate(() => window.__mech.state));
} finally {
  await browser.close();
}
