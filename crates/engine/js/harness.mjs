// The e2e harness for a web build: headless Chrome on the GPU with WebGPU,
// the working tree answered by request interception (no server, no port),
// a fresh browser context per game, device presets, and a `Game` that
// drives the page through its status global (CDP eval, waits with a
// snapshot on timeout, frames, taps and clicks on a UI node's centre with
// an occlusion check, screenshots as downloads).
//
// Controls are found by id: `G.ui(id)` gives the node's box in CSS px
// ({ x, y, w, h, visible, enabled, z }), `G.uiNodes` every node,
// `G.reveal(id)` scrolls one into view; a tap or click lands on its
// centre after checking nothing else is on top. A game extends `Game`
// for its own selectors and flow helpers and passes the class to
// `openGame`.
//
//   import { launch, openGame, DEVICES, Game } from '../game-engine/js/harness.mjs';
//   const browser = await launch();
//   const game = await openGame(browser, { root, global: '__game', device: 'phone', query: 'level=x' });

import fs from 'node:fs/promises';
import path from 'node:path';
import { chromePath, projectRoot, puppeteer, sleep } from './server.mjs';

export { sleep };

const ORIGIN = 'https://game.test';
const TYPES = {
  '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css',
  '.json': 'application/json', '.wasm': 'application/wasm', '.ttf': 'font/ttf', '.woff2': 'font/woff2',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml', '.bin': 'application/octet-stream',
};

export const DEVICES = {
  desktop: { viewport: { width: 1280, height: 800, deviceScaleFactor: 1 } },
  phone: {
    userAgent: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Mobile Safari/537.36',
    viewport: { width: 915, height: 412, deviceScaleFactor: 2, isMobile: true, hasTouch: true, isLandscape: true },
  },
  phonePortrait: {
    userAgent: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Mobile Safari/537.36',
    viewport: { width: 412, height: 915, deviceScaleFactor: 2, isMobile: true, hasTouch: true, isLandscape: false },
  },
  // An iPhone held sideways.
  iphone: {
    userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
    viewport: { width: 844, height: 390, deviceScaleFactor: 3, isMobile: true, hasTouch: true, isLandscape: true },
  },
};

export async function launch({ root = projectRoot(), headful = !!process.env.PK_HEADFUL } = {}) {
  const p = await puppeteer(root);
  return p.launch({
    executablePath: chromePath(),
    headless: headful ? false : 'new',
    args: [
      '--enable-unsafe-webgpu', '--enable-features=Vulkan', '--use-angle=vulkan', '--ignore-gpu-blocklist', '--mute-audio',
      '--autoplay-policy=document-user-activation-required',
    ],
    protocolTimeout: 600000,
  });
}

const IGNORE = [/Failed to load resource/, /KHR_parallel_shader_compile/];

export class Game {
  constructor(page, cdp, context, global = '__game') {
    this.page = page;
    this.cdp = cdp;
    this.context = context;
    this.global = global;
    this.errors = [];
    this.warnings = [];
  }

  /// Evaluates `fn(...args)` in the page; `fn` may use `G` for the status
  /// global when given as a function of (G, ...args) through `evalG`.
  async eval(fn, ...args) {
    const expression = typeof fn === 'function' ? `(${fn})(...${JSON.stringify(args)})` : fn;
    const res = await this.cdp.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: false });
    if (res.exceptionDetails) {
      const d = res.exceptionDetails;
      throw new Error(`page eval failed: ${d.exception?.description || d.text}`);
    }
    return res.result.value;
  }

  /// `fn(G, ...args)` with G the status global.
  evalG(fn, ...args) {
    return this.eval(`((G, ...a) => (${fn})(G, ...a))(window[${JSON.stringify(this.global)}], ...${JSON.stringify(args)})`);
  }

  /// Polls until `fn` is truthy: a JS expression string, a `() => ...`
  /// evaluated in the page, or a `(G, ...args) => ...` given the status
  /// global and `args`.
  async waitFor(fn, { timeout = 15000, interval = 100, what = String(fn), args = [] } = {}) {
    const t0 = Date.now();
    for (;;) {
      const v = typeof fn === 'function' && fn.length >= 1 ? await this.evalG(fn, ...args) : await this.eval(fn, ...args);
      if (v) return v;
      if (Date.now() - t0 > timeout) {
        throw new Error(`timed out after ${timeout} ms waiting for ${what}\n  game: ${JSON.stringify(await this.snapshot())}`);
      }
      await sleep(interval);
    }
  }

  /// The status contract's fields, for messages; a game adds its own.
  snapshot() {
    return this.evalG((G) => ({
      state: G?.state, screen: G?.screen, frames: G?.frames, ready: G?.ready, backend: G?.backend, error: G?.error,
      fullscreen: !!document.fullscreenElement,
    }));
  }

  screen() { return this.snapshot().then((s) => s.screen); }

  ui(id) { return this.evalG((G, i) => G.ui(i), id); }

  /// A control's centre in CSS px, scrolled into view first; fails if it
  /// is not shown, off screen, or another control is on top of that point.
  async center(id) {
    const r = await this.centerOnce(id);
    if (r.error) throw new Error(r.error);
    return r;
  }

  async centerOnce(id) {
    await this.evalG((G, i) => G.reveal?.(i), id);
    await this.frames(3);
    return this.evalG((G, i) => {
      const u = G.ui(i);
      if (!u) return { error: 'no control ' + i };
      if (!u.visible || !u.w || !u.h) return { error: i + ' is not visible' };
      const x = u.x + u.w / 2, y = u.y + u.h / 2;
      // The topmost control there, as the client's own hit test picks it.
      let top = null;
      for (const [k, o] of Object.entries(G.uiNodes || {})) {
        if (!o.visible || !o.enabled || x < o.x || x > o.x + o.w || y < o.y || y > o.y + o.h) continue;
        if (!top || o.z > top.z || (o.z === top.z && o.w * o.h < top.w * top.h)) top = { k, ...o };
      }
      if (u.enabled && top && top.k !== i) return { error: `${i} is covered by ${top.k}` };
      return { x, y };
    }, id);
  }

  frames(n = 2) {
    return this.eval((k) => new Promise((res) => {
      let i = 0;
      const f = () => (++i >= k ? res(true) : requestAnimationFrame(f));
      requestAnimationFrame(f);
    }), n);
  }

  async tap(id) {
    const { x, y } = await this.center(id);
    await this.page.touchscreen.tap(x, y);
    await this.frames(2);
  }

  async click(id) {
    const { x, y } = await this.center(id);
    await this.page.mouse.click(x, y);
    await this.frames(2);
  }

  async key(code, holdMs = 0) {
    await this.page.keyboard.down(code);
    if (holdMs) await sleep(holdMs);
    else await this.frames(2);
    await this.page.keyboard.up(code);
    await this.frames(2);
  }

  async reload() {
    await this.page.reload();
    await this.waitReady();
  }

  /// The client loaded: `ready` and no longer on a loading screen; a game
  /// may wait for more (its UI nodes, a screen).
  async waitReady(timeout = 240000) {
    await this.waitFor((G) => {
      if (G?.state === 'failed') throw new Error('client failed: ' + G.error);
      return G?.ready === true && G.screen !== 'loading';
    }, { timeout, interval: 250, what: 'the client to load' });
    // The screen's first layouts (the pixel ratio changes with it).
    await this.frames(6);
  }

  /// `G.screenshot(name)`: the next frame, saved by the browser as a
  /// download into `dir` (`openGame`'s `downloads`).
  async shot(name, dir) {
    const file = path.join(dir, name);
    await fs.rm(file, { force: true });
    await this.evalG((G, n) => G.screenshot(n), name);
    for (let i = 0; i < 100; i++) {
      try { await fs.access(file); return file; } catch { await sleep(100); }
    }
    throw new Error('no screenshot ' + name);
  }

  async close() { await this.context.close(); }
}

/// Opens the build in a fresh browser context. `root`: the tree served;
/// `page`: the page's path in it (`dist/next/index.html`); `storage`:
/// localStorage entries seeded before boot (JSON-encoded here);
/// `rewrite(rel)`: a game's substitution of one served path for another
/// (a lighter scene for a heavy one); `downloads`: a directory for
/// `G.screenshot()`; `GameClass`: a subclass of `Game`.
export async function openGame(browser, {
  root = projectRoot(), global = '__game', page: pagePath = 'dist/next/index.html', device = 'desktop', query = '',
  storage = {}, rewrite = null, downloads = null, wait = true, GameClass = Game,
} = {}) {
  const context = await browser.createBrowserContext();
  const page = await context.newPage();
  const cdp = await page.createCDPSession();
  const game = new GameClass(page, cdp, context, global);
  if (downloads) await cdp.send('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads });
  page.on('pageerror', (e) => game.errors.push(String(e.message || e)));
  page.on('console', (m) => {
    const text = m.text();
    if (IGNORE.some((re) => re.test(text))) return;
    if (m.type() === 'error') game.errors.push(text);
    else if (m.type() === 'warn' || m.type() === 'warning') game.warnings.push(text);
    if (process.env.PK_VERBOSE) console.log(`[page ${m.type()}] ${text}`);
  });
  await page.setRequestInterception(true);
  page.on('request', async (req) => {
    const u = new URL(req.url());
    if (u.origin !== ORIGIN) return req.abort('blockedbyclient');
    let rel = decodeURIComponent(u.pathname).replace(/^\/+/, '') || 'index.html';
    if (rel.endsWith('/')) rel += 'index.html';
    if (rewrite) rel = (await rewrite(rel)) || rel;
    const file = path.resolve(root, rel);
    try {
      const body = await fs.readFile(file);
      await req.respond({ status: 200, contentType: TYPES[path.extname(file)] || 'application/octet-stream', body });
    } catch {
      await req.respond({ status: 404, contentType: 'text/plain', body: 'not found: ' + rel });
    }
  });
  page.on('response', (r) => { if (r.status() >= 400) game.errors.push(`HTTP ${r.status()} ${r.url()}`); });
  await page.emulate({ userAgent: DEVICES[device].userAgent || await browser.userAgent(), viewport: DEVICES[device].viewport });
  if (Object.keys(storage).length) {
    await page.evaluateOnNewDocument((entries) => {
      for (const [k, v] of Object.entries(entries)) if (localStorage.getItem(k) === null) localStorage.setItem(k, v);
    }, Object.fromEntries(Object.entries(storage).map(([k, v]) => [k, JSON.stringify(v)])));
  }
  const q = query.replace(/^\?/, '');
  await page.goto(`${ORIGIN}/${pagePath}${q ? '?' + q : ''}`);
  if (wait) await game.waitReady();
  return game;
}

/// A localStorage entry, JSON-decoded.
export const stored = (game, key) => game.eval((k) => {
  const v = localStorage.getItem(k);
  return v === null ? null : JSON.parse(v);
}, key);

export async function expectScreen(game, name, timeout = 8000) {
  await game.waitFor((G, n) => G.screen === n, { timeout, what: `the "${name}" screen`, args: [name] });
}

export const isShown = (game, id) => game.ui(id).then((u) => !!u && u.visible && u.w > 0 && u.h > 0);
