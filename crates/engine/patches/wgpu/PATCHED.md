# wgpu 29.0.4, patched for Playground Kit (and the games built on it)

The crates.io release of `wgpu` 29.0.4, unchanged except for
`src/backend/webgpu.rs` (and one line in `src/lib.rs`, below), with two
changes. The second, destroy on drop, is described after the first.

## 1. Limits and features read once (D1182)

`WebDevice` reads the device's limits and
features from the browser once and keeps them (`limits` and `features`,
two `OnceCell`s), instead of mapping every field of `GPUSupportedLimits`
on each call. A WebGPU device's limits and features never change after it
is made, so the answers are the same.

Why: Bevy 0.19's `SetMeshBindGroup` asks `RenderDevice::limits()` on every
draw (`skins_use_uniform_buffers`), which cost about 7 % of a phone's frame
on Coast and Sierra (docs/rust-port/DECISIONS.md, D1182; first found in
D866 item 3).

## 2. Buffers and textures destroyed when dropped (D9, 2026-10-18)

The release never calls `destroy()` on a buffer or texture: `Drop for
WebBuffer` and `Drop for WebTexture` are empty, so the GPU memory waits for
the JS garbage collector, which sees only a small wrapper (upstream chose
this: gfx-rs/wgpu#4092). Bevy re-makes buffers often (per-frame cluster
buffers, mesh-uniform buffers re-made at exactly their size as they grow,
staging), so on WebGPU what is held depends on when the collector runs;
Safari is in no hurry. Now each buffer and texture has a `Life` shared by
every clone of its handle, which destroys it when the last one goes, and
everything that can still use it holds that `Life` too, as wgpu-core does
natively: a texture's views, the bind groups that bind it, and the
encoders, passes and render bundles that record it, until the command
buffer is submitted (WebGPU frees the memory once the submitted work is
done). A canvas's current texture is `Life::Borrowed`, never destroyed.
Search for "DESTROY ON DROP"; the diff is
`docs/upstream/wgpu-29-webgpu-destroy-on-drop.diff`, against change 1.

Measured: Snowboarding, 8,076 buffers made in a minute and none destroyed
before, all but 69 destroyed after (24 MB held, not 46 to 60 MB); Midnight
Playground's Sierra race, ~2 buffers a frame all destroyed, 481 live, no
WebGPU errors; uncapped frame time no worse, the slowest frames better
(5th-percentile fps about 200 before, about 300 after; D9).

## Moving to a new wgpu release

Used through `[patch.crates-io]` in the workspace's Cargo.toml. Copy the
release's crate here (from `~/.cargo/registry/src/*/wgpu-<version>/`,
without its Cargo.lock), apply both changes (search for D1182 and for
DESTROY ON DROP), and update this note. Remove change 1 once a release
caches the limits; change 2 once one destroys dropped resources.
`patches/wgpu-30/` is the same change 2 on wgpu 30.0.1, for Bevy 0.20.

Also `#![allow(warnings)]` at the top of `src/lib.rs`: Cargo caps the
lints of a crates.io dependency but not of a path one, and this release
has a few unused-import warnings in our feature set.
