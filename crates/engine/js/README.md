# Headless-Chrome tools

One tool per file, each a module with `run({ args, defaults, root })` and
a command line when run directly. They run inside a game (the project
root is the git top level of the current directory, or `$PK_PROJECT_ROOT`)
and need `puppeteer-core` in that game's `package.json`
(`npm install --save-dev puppeteer-core`; nothing else) and Chrome at
`$CHROME_PATH` (default `/usr/bin/google-chrome`). Every tool reads the
page's status global named by `--global` (default `__game`), finds the
registered dev server by `--url`, then `--port`, then `$PORT`, then
`proj port` (no default), and takes the page's parameters with `--query`.
A game keeps thin wrappers that import these with its own defaults
(Midnight Playground's `tools/parity/*.mjs`).

| File | What |
|---|---|
| `server.mjs` | the shared pieces: `baseUrl`, `chromeFlags` (GPU, WebGPU/Vulkan/ANGLE, WebGL2 forcing), `launch`, `puppeteer`, `statusGlobal`, `projectRoot`, `argv` |
| `harness.mjs` | the e2e harness: request interception serving the working tree, device presets (desktop, Pixel landscape and portrait, iPhone), the `Game` class (CDP eval, `waitFor` with a snapshot on timeout, `frames`, `tap`/`click` on a UI node's centre with the occlusion check, screenshots as downloads) |
| `perf.mjs` | frame time, CPU time per frame (a rAF timer injected before the page's scripts), memory, load time and reloads; `--trace` for Chrome's trace per frame (renderer main, GPU process, GC); `--phone`, `--throttle`; `--run --wait --pos` for a game's scenario |
| `prof.mjs` | a CPU profile (CDP Profiler at 250 µs) by function and by the enclosing Bevy system, of the names-kept build (`cargo xtask prof`); `--callers <regex>` |
| `gl-calls.mjs` | WebGL2 calls a frame by name, GPU time per frame and per pass (and per program) through `EXT_disjoint_timer_query_webgl2`, shader dump and regex substitution |
| `build-diff.mjs` | two dist builds compared pixel for pixel on WebGL2, with a red overlay of the differing pixels |
| `png.mjs` | the PNG codec `build-diff.mjs` uses |
| `console.mjs` | the page's console for a build that does not come up, then the status global's state |
| `shot.mjs` | pictures of the build (`G.screenshot` as a download), after a wait and a key sequence |
| `stress.mjs` | screen changes hammered in a seeded random walk over a step catalogue (a parameter; random menu keys by default), checking frames, errors and heap growth |

The page side of `perf.mjs` is `template/web/perf.js`, which the game's
`index.html` loads with `?perf=1`.
