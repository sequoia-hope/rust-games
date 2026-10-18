# wgpu WebGPU backend: destroy buffers and textures when dropped

Drafted 2026-10-18 from `patches/wgpu/` (wgpu 29.0.4, D9) and
`patches/wgpu-30/` (30.0.1). **Nothing has been posted.**
`wgpu-29-webgpu-destroy-on-drop.diff` is against the kit's wgpu 29 (on top
of the limits cache, D1182); `wgpu-30-webgpu-destroy-on-drop.diff` is
against pristine 30.0.1, the one to rebase onto trunk. Both touch
`src/backend/webgpu.rs` only.

## The problem

On the WebGPU backend `Drop for WebBuffer` and `Drop for WebTexture` are
empty, so a dropped buffer or texture lives until the JS garbage collector
finds its wrapper. The collector sees a few bytes, not the GPU memory
behind them, so it has no reason to hurry, and Safari often does not. Bevy
re-makes buffers routinely (cluster buffers for each view every frame,
mesh-uniform buffers re-made at exactly their size as they grow, staging),
so on WebGPU the memory held depends on GC timing. Measured in
Snowboarding: 8,076 buffers made in a minute of riding, none destroyed;
46 to 60 MB made and not destroyed, of which Chrome had collected down to
24 to 27 MB at any time. Native wgpu frees on the last drop.

## Upstream state (checked 2026-10-18)

- **gfx-rs/wgpu#4092** "Resource should be destroyed when dropped in the
  Web backend" (@mikialex, 2023-08), **closed 2024-05-07** by @grovesNL as
  not actionable. The thread found the obstacle: a resource may still be
  used by a bind group or by a recorded, unsubmitted command buffer, and
  destroying it then is a validation error. @cwfitzgerald noted bind groups
  would have to be kept alive too; @mikialex judged the extra reference
  counting too expensive; @grovesNL preferred explicit `destroy()` but
  ended with "We could reconsider some kind of optional refcounting at the
  WebGPU backend level sometime if it's a big problem."
- Searches for newer issues or PRs (webgpu destroy/drop/leak/garbage) found
  nothing else on this. trunk's `Drop for WebBuffer` is still a no-op.

## What the patch does (the refcounting #4092 asked about)

- `Life`: owns the JS buffer or texture, `destroy()`s it in `Drop`. One
  `Rc<Life>` is shared by every clone of the handle (the dispatch enum
  holds `WebBuffer` by value and derives `Clone`, so `Drop` on `WebBuffer`
  itself would fire per clone).
- `Keep`: what must stay alive. A texture view keeps its texture; a bind
  group keeps everything it binds; encoders, passes and bundle encoders
  push what they record (bind groups, vertex/index/indirect buffers, copy
  sources and destinations, attachments, executed bundles) into a shared
  list, skipping a repeat of the last entry (draws set the same buffers
  over and over). `finish()` moves the list into the command buffer, which
  drops it right after `submit()`: WebGPU frees a destroyed resource's
  memory once submitted work using it completes.
- A canvas's current texture and (wgpu 30) a texture wrapped with
  `wrap_external_texture` are `Life::Borrowed`: never destroyed.
- `Buffer::destroy()` / `Texture::destroy()` still destroy at once.

## Evidence for the cost question

Midnight Playground, Sierra race on the autopilot, uncapped WebGPU, three
alternating 30 s runs each, same machine (`rust-perf.mjs --race`):

| | CPU ms/frame mean | p99 | frames in 30 s | 5th-percentile fps |
|---|---|---|---|---|
| stock | 2.79 / 2.72 / 2.70 | 8.0 / 7.5 / 7.0 | 10,148 / 10,284 / 10,475 | 191 / 193 / 225 |
| patched | 2.52 / 2.61 / 2.60 | 6.0 / 6.4 / 6.9 | 11,270 / 10,912 / 10,899 | 317 / 293 / 286 |

No cost visible; the slowest frames improve (less GC of GPU wrappers).
Correctness: Snowboarding's stress, course and resize runs and Midnight's
race show no WebGPU validation error; Snowboarding holds 69 live buffers
(24 MB) through a ride, Midnight 481 (135 MB) with ~2 made and destroyed a
frame; 25 window resizes destroy the old render targets.

## Invariants a reviewer will ask about

- Nothing reachable from Rust can use a destroyed resource: every path
  that hands a buffer or texture to the browser for later use holds its
  `Life` (audited: every encoder, compute/render pass and bundle encoder
  method that takes a buffer, texture, view, bind group or bundle).
- Immediate calls (`Queue::write_buffer`, `write_texture`, `map_async`)
  need nothing: they act before returning, or the API's borrows keep the
  handle alive (a mapped range borrows its buffer).
- A dropped `CommandEncoder` that was never finished drops its list: what
  only it used is destroyed, which is right.
- Single-threaded (`Rc`, `RefCell`), as the rest of the WebGPU backend.

## CHANGELOG (draft)

`#### WebGPU` under `### Changes`: "Buffers and textures are destroyed when
the last handle, view, bind group or unsubmitted command buffer using them
is dropped, as on native backends, instead of waiting for the JavaScript
garbage collector."

## To do by hand

- Rebase `wgpu-30-…diff` onto trunk; run `cargo xtask run-wasm` on the
  examples and the CTS subset the maintainers use for the web backend.
- Decide with the maintainers whether it is opt-out (a backend option) or
  the default; #4092's objection was cost, which the table answers for one
  real app.
- wgpu's CONTRIBUTING asks that PR text be written by hand; this file is
  raw material.
