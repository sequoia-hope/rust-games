# wgpu WebGPU backend: read the device's limits and features once

Drafted 2026-10-16 from `patches/wgpu/` (wgpu 29.0.4, D1182). Nothing has
been posted. `wgpu-webgpu-limits-cache.diff` is the local patch against pristine
29.0.4 (`src/backend/webgpu.rs` only; the `#![allow(warnings)]` line in
`src/lib.rs` is left out). It applies to 29.0.4 (`patch --dry-run`); it does
**not** apply to trunk (two of three hunks fail: `WebDevice` gained an
`uncaptured_error_listener` field and the constructor moved), and it does not
need to, because of what follows.

## Upstream state (checked 2026-10-16)

- **gfx-rs/wgpu#10550** "Requesting Limits every frame causes expensive
  resource thrashing", opened 2026-10-06 by @apetrock (John Delaney). Open, no
  labels, no comments. Describes exactly D1182: `map_wgt_limits` reads each
  limit through its own wasm-bindgen getter on every call; asks for a lazy
  cache of limits, features and adapter info.
- **gfx-rs/wgpu#10513** "Backend Caching of Heavy Requests", same author,
  opened 2026-10-02, last updated 2026-10-07, open, `REVIEW_REQUIRED`, no
  review and no comments yet. It **is the fix**: a `CachedProperties { features,
  limits, adapter_info }` of `OnceCell`s in an `Rc` on both `WebAdapter` and
  `WebDevice`, filled on first use, cloned or copied after. Our patch is the
  `WebDevice` half of it (two `Rc<OnceCell>` fields instead of one struct; no
  adapter, no `adapter_info`). It carries a CHANGELOG entry under a new
  `#### WebGPU` subsection of `### Performance`. Its author tested with
  `cargo clippy --target wasm32-unknown-unknown -p wgpu --no-default-features
  --features webgpu,wgsl` and `cargo xtask run-wasm` on every example.
- **bevyengine/bevy#25998** "Device Limits and Feature Cache to Avoid WGPU
  Conversion Thrashing", same author, opened 2026-10-01, open,
  `S-Needs-Review`, `C-Bug`, `A-Rendering`, `AI-Assisted`, last touched
  2026-10-16. Caches limits and features on `RenderDevice` (Bevy side) and
  changes callers from an owned value to a borrow across bevy_pbr,
  bevy_render, bevy_sprite_render, bevy_solari. @IceSentry asked for the
  upstream fix, which became #10513; @Zeophlite suggested keeping the Bevy
  cache until Bevy picks up the wgpu release. Either one shipping removes the
  7 % for Bevy users; #10513 removes it for every wgpu wasm user.
- trunk `wgpu/src/backend/webgpu.rs` still has no cache (`fn limits` maps on
  every call at both the adapter and device impls).

So: **no PR from us.** The useful contribution is the measurement, which
neither the issue nor the PR has (the PR says "profiling a Bevy wasm build
showed `map_wgt_limits` once per frame"; it is actually once per *draw* in
Bevy 0.19, via `SetMeshBindGroup` / `skins_use_uniform_buffers`), and a
"we run the equivalent in production" data point for the reviewer. Below is a
comment for #10550 and a shorter one for #10513. Post one or both; the
#10550 comment is the one ECOSYSTEM.md recommends.

## Comment draft for gfx-rs/wgpu#10550

> Some numbers from a shipped Bevy 0.19 game (WebGPU build, Chrome), in case
> they help prioritise #10513, which fixes this.
>
> Bevy 0.19's `SetMeshBindGroup` calls `RenderDevice::limits()` on **every
> draw** (through `skins_use_uniform_buffers`), not once a frame, and
> `features()` a few times a frame. With wgpu 29 on the browser backend each
> of those calls runs `map_wgt_limits`, i.e. one wasm-bindgen getter per limit.
> Profiling with CDP's sampling profiler:
>
> - desktop (RTX 3060, Chrome): about 0.08 ms of main-thread time a frame,
>   roughly 700 draws;
> - phone (`<owner: name the phone and browser; D1182 only says "the phone",
>   Coast and Sierra>`): about **7 % of the frame** on our two heaviest
>   scenes.
>
> We have been shipping wgpu 29.0.4 with the `WebDevice` half of #10513 (two
> `Rc<OnceCell<_>>` fields for limits and features, filled on first use) since
> 2026-10-09 through `[patch.crates-io]`, with no issues on Chrome, Safari or
> Firefox `<owner: trim to the browsers actually run>`. The patch is 16 lines
> against 29.0.4; #10513 is the same idea plus the adapter and `adapter_info`,
> so it supersedes ours. The invariant it relies on holds by spec: a
> `GPUDevice`'s `limits` and `features` are fixed at `requestDevice`, and a
> `GPUAdapter`'s do not change for the adapter's lifetime (an adapter can
> become invalid, but its reported values do not change).
>
> Happy to test a trunk build on the same scenes if that is useful. For the
> Bevy side, bevyengine/bevy#25998 caches on `RenderDevice`, which also
> removes the per-draw call; either one is enough for Bevy users.

Notes for the owner before posting:
- Fill or cut the two `<owner: ...>` brackets. D1182 and D866 give: desktop
  0.08 ms a frame (D866 item 3, Sierra); phone 7 % on Coast and Sierra
  (D1182). The browser on the phone is not written down in either decision.
- wgpu's CONTRIBUTING asks that descriptions be written by hand; a comment
  with measurements is fine, but put it in your own words.
- "16 lines" is the `+` count of `wgpu-webgpu-limits-cache.diff`.

## Shorter comment draft for gfx-rs/wgpu#10513 (optional)

> Data point for review: we have shipped the `WebDevice` part of this (limits
> and features in `OnceCell`s) on wgpu 29.0.4 since 2026-10-09 in a Bevy 0.19
> WebGPU game with no issues. Before it, `RenderDevice::limits()` on every
> draw cost about 7 % of a phone's frame on our heaviest scenes (details in
> #10550). The `WebAdapter` and `adapter_info` caching here looks right to me
> for the same reason: those values are fixed for the object's lifetime.

## The invariant a reviewer will ask about

A WebGPU device's `limits` and `features` never change after creation: they
are the values granted at `requestDevice` (the WebGPU spec defines
`GPUDevice.limits` and `GPUDevice.features` as fixed `GPUSupportedLimits` /
`GPUSupportedFeatures` objects). Caching the mapped Rust value is therefore
exact, not approximate. The same holds for the adapter's values. Device loss
does not change them either; a lost device keeps reporting the same objects.

## When to drop the local patch

When a wgpu release containing #10513 is on crates.io and Bevy's lock moves
to it, or when Bevy ships #25998 (then the per-draw call disappears and the
remaining `features()` calls a frame are cheap enough). Until then the patch
stays; PATCHED.md says how to carry it.
