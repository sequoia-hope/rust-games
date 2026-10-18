# wgpu 30.0.1, patched for Bevy 0.20 projects (render)

The crates.io release of `wgpu` 30.0.1, unchanged except for
`src/backend/webgpu.rs` and one line in `src/lib.rs`: buffers and textures
are destroyed when the last thing that can still use them is dropped,
change 2 of `patches/wgpu/PATCHED.md` (D9), ported from wgpu 29. The diff
is `docs/upstream/wgpu-30-webgpu-destroy-on-drop.diff`, against the
pristine crate.

Differences from the wgpu 29 port: `set_vertex_buffer` takes an
`Option<&Buffer>` here (keep it only when there is one), and a texture
wrapped with `wrap_external_texture` is the caller's, so its `Life` is
`Borrowed` like a canvas's current texture.

Not here: change 1 of the wgpu 29 patch (the limits cache, D1182). Add it
if a Bevy 0.20 game shows the same per-draw cost.

Bevy 0.19 needs wgpu 29 and Bevy 0.20 needs wgpu 30, and a
`[patch.crates-io]` must keep the same semver range, so the kit carries
both until every game is on one Bevy. Used by
`render/bevy_runtime/Cargo.toml`. Verified there by build and by code
audit only: render's WebGPU viewer gets no adapter in headless Chrome on
the dev machine (two GPUs), with or without this patch.

Also `#![allow(warnings)]` at the top of `src/lib.rs`, as in the wgpu 29
patch: Cargo caps a crates.io dependency's lints but not a path one's.
