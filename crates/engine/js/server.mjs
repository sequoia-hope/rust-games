// What every headless-Chrome tool here shares: the registered dev server's
// address (never a default port), Chrome's path and flags, the status
// global's name, puppeteer-core from the game's node_modules, and the
// arguments helper.
//
// The server's address: `--url <base>`, else http://127.0.0.1 on `--port`,
// else `$PORT`, else `proj port` run in the project root; there is no
// default port. The server serves the main checkout, so a worktree is
// reached under its path there.
//
// The project root is the git top level of the current directory (or
// `$PK_PROJECT_ROOT`): the tools live in the kit and run inside a game.

import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

/// `opt('--key', default)` and `flag('--key')` over an argument list.
export function argv(args = process.argv.slice(2)) {
  const opt = (k, d = null) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
  const flag = (k) => args.includes(k);
  return { args, opt, flag };
}

/// The game's repository root.
export function projectRoot() {
  if (process.env.PK_PROJECT_ROOT) return path.resolve(process.env.PK_PROJECT_ROOT);
  try {
    return execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
  } catch {
    return process.cwd();
  }
}

/// The status global the page publishes (`GLOBAL` in the loader page).
export const statusGlobal = (opt) => opt('--global', null) || process.env.GAME_GLOBAL || '__game';

export const chromePath = () => process.env.CHROME_PATH || '/usr/bin/google-chrome';

export function baseUrl(opt, root = projectRoot()) {
  const given = opt('--url', null);
  if (given) return given.endsWith('/') ? given : given + '/';
  let port = opt('--port', null) || process.env.PORT;
  if (!port) {
    try {
      port = execFileSync('proj', ['port'], { cwd: root, encoding: 'utf8' }).trim();
    } catch (e) {
      throw new Error(`no server address: pass --url or --port, set $PORT, or register the project (proj port failed: ${e.message})`);
    }
  }
  if (!/^\d+$/.test(port)) throw new Error(`proj port gave "${port}"`);
  // A worktree is served under its path below the main checkout.
  let rel = '';
  try {
    const common = path.resolve(root, execFileSync('git', ['rev-parse', '--git-common-dir'], { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim());
    rel = path.relative(path.dirname(common), root);
  } catch {
    rel = '';
  }
  if (rel.startsWith('..')) throw new Error(`${root} is not below the served checkout`);
  return `http://127.0.0.1:${port}/${rel ? rel.split(path.sep).map(encodeURIComponent).join('/') + '/' : ''}`;
}

/// Chrome's flags for a measurement: the real GPU, no sound, and the
/// backend forced (`webgl2` turns WebGPU off so the page falls back by
/// itself). `angle`: what ANGLE runs WebGL on, vulkan or gl (the native
/// driver, as Chrome on Linux picks for Intel graphics).
export function chromeFlags({ backend = 'webgpu', capped = false, gesture = false, angle = 'vulkan' } = {}) {
  const flags = ['--ignore-gpu-blocklist', '--mute-audio', '--enable-gpu', `--use-angle=${angle}`, '--enable-precise-memory-info'];
  if (!capped) flags.push('--disable-gpu-vsync', '--disable-frame-rate-limit');
  if (gesture) flags.push('--autoplay-policy=no-user-gesture-required');
  if (backend === 'webgpu') flags.push('--enable-unsafe-webgpu', '--enable-features=Vulkan');
  if (backend === 'webgl2') flags.push('--disable-blink-features=WebGPU', '--disable-features=WebGPU');
  return flags;
}

/// puppeteer-core from the game's node_modules (this file lives in the kit,
/// which has none).
export async function puppeteer(root = projectRoot()) {
  const req = createRequire(path.join(root, 'package.json'));
  let file;
  try {
    file = req.resolve('puppeteer-core');
  } catch {
    throw new Error(`puppeteer-core is not installed in ${root}: npm install --save-dev puppeteer-core`);
  }
  return (await import(pathToFileURL(file).href)).default;
}

/// A browser on the real GPU with `flags`; `--headed` opens a window on
/// $DISPLAY instead (a laptop's headless Chrome may miss the GPU).
export async function launch(flags, { headed = false, timeoutS = 600, root = projectRoot() } = {}) {
  const p = await puppeteer(root);
  return p.launch({
    executablePath: chromePath(),
    headless: headed ? false : 'new',
    args: headed ? [...flags, '--window-size=1920,1080', '--window-position=0,0'] : flags,
    protocolTimeout: timeoutS * 1000,
    dumpio: !!process.env.PK_DUMPIO,
  });
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/// True when `url` (a module's import.meta.url) is the script Node ran.
export const isMain = (url) => process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(url);
