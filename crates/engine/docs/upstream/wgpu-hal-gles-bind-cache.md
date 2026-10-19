# wgpu-hal GL backend: skip binding what GL already has (three PRs)

Drafted 2026-10-16 from `patches/wgpu-hal/` (wgpu-hal 29.0.4, D1190). Nothing
here has been posted. The diffs beside this file:

| File | What | Applies to |
|---|---|---|
| `wgpu-hal-gles-bind-cache.diff` | the whole local patch, minus `#![allow(warnings)]` | pristine 29.0.4 (verified with `patch --dry-run`) |
| `wgpu-hal-gles-1-vertex-buffer-dirty.diff` | PR 1 | pristine 29.0.4 |
| `wgpu-hal-gles-2-program-texture-sampler-cache.diff` | PR 2 | on top of PR 1 (also applies to pristine alone) |
| `wgpu-hal-gles-2-fixup-untracked-binds.diff` | two hunks PR 2 needs that the local patch lacks (see "Two gaps found while drafting") | on top of PR 2 (also applies to the full patch) |
| `wgpu-hal-gles-3-texture-mip-range.diff` | PR 3 | on top of PR 2 |

The stack 1 + 2 + 3 reproduces the local patch exactly (verified byte for byte).
The split is clean except for one region: both PR 2 and PR 3 edit the
`C::BindTexture` arm of `Queue::process` in `src/gles/queue.rs` (PR 2 adds the
early return and the cache write before and after the `bind_texture` call; PR 3
wraps the two `tex_parameter_i32` calls that follow). PR 3 as written assumes PR
2's lines are present; applied to pristine alone it succeeds with fuzz, but
opening it independently means rebasing that one hunk by hand. PR 1 and PR 2
share nothing (PR 1 touches `command.rs` and the `BufferBinding` derive in
`mod.rs`; PR 2 touches `queue.rs`, the `Queue` struct, and `adapter.rs`).

## Before posting: reword the project-local comments

The diffs are faithful to the local patch, so their comments say "Midnight
Playground (D1190)" and "D1190:". Upstream should not see those. Replace each
with the plain reason; suggested wording is given per PR below. The behaviour
is unchanged by this.

## Does it still apply to trunk? (checked 2026-10-16)

Trunk's `wgpu-hal/src/gles/` still binds unconditionally at every place the
patch touches: `SetProgram` calls `use_program` each time, `BindSampler` calls
`bind_sampler` each time, `BindTexture` binds and sets `TEXTURE_BASE_LEVEL` and
`TEXTURE_MAX_LEVEL` on every bind, `SetIndexBuffer` binds each time, and
`set_vertex_buffer` always dirties the attribute mask. No other code in trunk's
GL backend sets `TEXTURE_BASE_LEVEL` or `TEXTURE_MAX_LEVEL`. `Queue` still has
`current_index_buffer`, `AdapterShared` still has `max_msaa_samples` where the
patch adds `tex_levels`. So all three apply in spirit. Mechanically:

- The full diff and the stack apply to trunk with line offsets, except one hunk:
  `C::CopyTextureToTexture` was restructured by #10004 (the copy now binds the
  destination texture once before a per-slice loop). The patch's one-line cache
  invalidation goes right after that `gl.bind_texture(dst_target, Some(dst))`,
  which is now near line 727 instead of 717. Everything else is an offset.
- `set_vertex_buffer`'s signature gained a generic (`BufferBinding<'a,
  super::Buffer, wgt::BufferAddress>`); the body is the same, the hunk applies
  with fuzz 2.
- The CHANGELOG now has a `### Performance` section under `## Unreleased` with
  only a `#### General` subsection; add a `#### GLES` subsection for these.

No prior art found: GitHub search over gfx-rs/wgpu for gles + redundant / bind
cache / state tracking / TEXTURE_MAX_LEVEL / use_program returned nothing
relevant (searched 2026-10-16). #9834 (external WebGL texture interop, merged
2026-08) is the closest neighbour and is the reason to state the interop
invariant below.

## Measurements the descriptions quote (D1190, PATCHED.md)

| What | Value |
|---|---|
| WebGL2 GL calls a frame, Sierra, 685 draws, before | about 24,600 |
| same scene in the three.js game, 780 draws | about 4,400 |
| WebGL2 GL calls a frame after the whole patch | about 15,400 |
| of which `texParameteri` for the mip levels, before | about 3,900 a frame |
| main-thread CPU gain, whole patch | about 5 % |
| GPU time | unchanged |
| pictures | identical (build-diff) |

What is not yet measured: the per-PR share. Each PR's description below has a
`<N>` to fill. Measure on the GPU machine with each PR applied alone (and the
stack), unpatched build first, from the midnight-playground checkout:

```
node tools/parity/gl-calls.mjs --game rust --backend webgl2
```

## Two gaps found while drafting (fix before posting PR 2, and locally)

`Queue::perform_shader_clear` (`queue.rs`, the `MESA_I915_SRGB_SHADER_CLEAR`
workaround used by `C::ClearColorF` on sRGB targets) calls `gl.use_program` on
the clear program and does not restore the previous one. Pristine wgpu is fine
because the next `C::SetProgram` always re-issues `use_program`. With PR 2's
program cache, a pass that draws with program P, followed by a pass whose clear
runs the shader clear, followed by `SetProgram(P)`, would skip the bind and
draw with the clear program. The first hunk of
`wgpu-hal-gles-2-fixup-untracked-binds.diff` records the clear program in the
cache after the bind so the next `SetProgram` differs and binds. It only
affects devices with that workaround (old Mesa i915), which is why the game
never saw it, but a reviewer will.

The second hunk is for `C::ClearBuffer` on an index buffer, found by auditing
every `bind_buffer` in `process` (table below): the clear binds the target
buffer to `ELEMENT_ARRAY_BUFFER` and leaves it there, so `current_index_buffer`
must be updated or the next `SetIndexBuffer` of the previous buffer is
wrongly skipped.

Both hunks apply to `patches/wgpu-hal/src/gles/queue.rs` as well (verified
with `patch --dry-run` against the full patch); applying them locally is the
owner's call, since it is outside this directory.

Audit of every other GL bind inside `Queue::process` and friends, for the
reviewer's question "what else binds a program, texture unit, sampler, or
ELEMENT_ARRAY_BUFFER behind the cache's back?":

| Call site (queue.rs, 29.0.4) | Covered by |
|---|---|
| `reset_state`: `use_program(None)`, `bind_buffer(ELEMENT_ARRAY_BUFFER, None)` | clears the whole `BindCache` and `current_index_buffer` |
| `perform_shader_clear`: `use_program(clear)` | the fixup diff |
| `CopyBufferToTexture`, `CopyTextureToTexture`, `CopyExternalImageToTexture`: `bind_texture(dst_target, dst)` on whatever unit is active | `bind_cache.textures = Default::default()` after each (all units, conservative) |
| buffer barrier with `INDEX` usage: `bind_buffer(ELEMENT_ARRAY_BUFFER, raw)` | `current_index_buffer = None` |
| `CopyBufferToBuffer` on the WebGL element-buffer path (`ELEMENT_ARRAY_BUFFER` as copy target when `INDEX_BUFFER_ROLE_CHANGE` is absent) | already restores `ELEMENT_ARRAY_BUFFER` to `*self.current_index_buffer` when done, so GL and the tracked value agree; nothing to add |
| `ClearBuffer` on an index buffer (`dst_target == ELEMENT_ARRAY_BUFFER`, both the zero-buffer copy and the `buffer_sub_data` fallback) | **not covered by the local patch**: it binds the cleared buffer as the element buffer and does not restore. Pristine is fine (the next `SetIndexBuffer` rebinds); with the cache the next `SetIndexBuffer` of the previously tracked buffer would be skipped. The fixup diff's second hunk records the cleared buffer as current. Rare in practice (`clear_buffer` on an index buffer), which is why the game never saw it. |
| `device.rs`: `create_texture` binds the new texture; `create_pipeline` calls `use_program` | happen between submits; `submit` holds the context lock for all its command buffers and calls `reset_state` before each, so the cache never outlives the lock |

Every other `bind_buffer` in `process` targets `COPY_READ_BUFFER`,
`COPY_WRITE_BUFFER`, `PIXEL_UNPACK_BUFFER`, `PIXEL_PACK_BUFFER` or an indexed
uniform / storage binding, none of which the cache tracks.

## Reviewer invariants, stated once

1. **Cache lifetime.** `BindCache` is filled only inside `Queue::process` and
   cleared by `reset_state`, which `submit` calls at the start of every command
   buffer while holding the GL context lock for the whole submission. Nothing
   outside the queue runs GL calls between a `reset_state` and the end of that
   command buffer, so GL state the cache describes cannot change behind it.
   Device-side binds (texture creation, program linking) happen between
   submissions and are forgotten by the next `reset_state`.
2. **Copies that bind a texture.** The three copies that call `bind_texture`
   outside `C::BindTexture` clear every unit's texture entry. Over-invalidating
   is deliberate: the active unit at that point is whatever the last
   `BindTexture` left, and the copy's `bind_texture` replaces it.
3. **Index buffer.** `SetIndexBuffer` now consults the `current_index_buffer`
   the queue already tracked (it was written but never read for this purpose).
   The buffer barrier that binds `ELEMENT_ARRAY_BUFFER` resets it.
4. **Mip range is texture state, not unit state.** `TEXTURE_BASE_LEVEL` /
   `TEXTURE_MAX_LEVEL` live on the texture object, so `tex_levels` is keyed by
   `glow::Texture`, lives on `AdapterShared` (across submissions), and is
   written at the only two places the backend sets those parameters.
   `destroy_texture` removes the entry because GL may hand the same name to a
   new texture. Textures the backend did not create (`texture_from_raw`,
   external GL interop, the #9834 WebGL external textures) are tracked from
   their first bind; if the embedding application changes their level
   parameters on a shared context, the cache will not notice. That is the same
   class of assumption the existing `current_index_buffer` tracking already
   makes, and worth saying in the PR so reviewers can decide whether
   `texture_from_raw` should seed or skip the map.
5. **Vertex buffer dirty check.** `set_vertex_buffer` compares the new
   `(raw, offset)` with the recorded one and does nothing if equal. Attribute
   pointers still get re-issued whenever they must: `set_render_pipeline`
   dirties every buffer the pipeline uses on the non-layout path and dirties
   buffers whose stride or step changed on the `VERTEX_BUFFER_LAYOUT` path;
   `begin_encoding` resets the encoder state, so the first bind in a command
   buffer always records; the per-draw instance offset still dirties through
   `instance_vbuf_mask`. The GL vertex attribute state this relies on persists
   across passes within one command buffer, and the state is per encoder.
6. **`DEPTH_STENCIL_TEXTURE_MODE`** is still set on every bind (it is also
   texture state). Could be folded into PR 3's map; left out to keep PR 3 small.
   Mention it as a follow-up.
7. **No behavioural change on correct programs.** Each skipped call would have
   set GL state to the value it already holds.

## Testing done locally (for the Testing sections)

- WebGL2 build of a Bevy 0.19 game, Chrome: GL call count per frame counted
  by a CDP-side tracer, pictures compared pixel for pixel with the unpatched
  build across four levels: identical.
- GPU timings via `EXT_disjoint_timer_query_webgl2`: unchanged.
- Not run: wgpu's own test suite (`cargo xtask test`, see `docs/testing.md` in
  the wgpu repo), native GL (desktop GL / GLES on Linux or Android), the
  `MESA_I915_SRGB_SHADER_CLEAR` path. Say so in the PR.

---

## PR 1: `set_vertex_buffer` does not dirty an unchanged binding

**Title:** `gles: don't re-point vertex attributes when set_vertex_buffer binds what is already bound`

**Comment wording for the diff** (replace the `D1190:` comment in `command.rs`):
`// The same buffer at the same offset needs no new attribute pointers; the`
`// attributes are still dirtied by pipeline changes and by instance offsets.`

### Connections

Part 1 of 3 of reducing redundant GL calls in the GL backend. Independent of
#NNNN2 and #NNNN3 (program/texture/sampler cache; per-texture mip range).

### Description

On the GL backend every `set_vertex_buffer` sets the buffer's dirty bit, so the
next draw re-issues `glBindBuffer`, `glEnableVertexAttribArray`,
`glVertexAttribPointer` and `glVertexAttribDivisor` for every attribute of that
buffer, even when the buffer and offset are the same as the previous draw's.
Renderers that pack many meshes into a few large buffers (Bevy's mesh
allocator slabs, for instance) bind the same buffer at the same offset draw
after draw, and pay those calls per attribute per draw.

This compares the incoming `(buffer, offset)` with the recorded binding and
returns early when equal, leaving the dirty bit alone. The attributes are still
re-pointed whenever they must be: `set_render_pipeline` dirties the buffers the
new pipeline uses (or whose layout changed), `begin_encoding` resets the
encoder state, and the per-draw instance offset still dirties through
`instance_vbuf_mask`. `BufferBinding` derives `PartialEq` for the comparison.

Measured on a Bevy 0.19 scene under WebGL2 in Chrome (685 draws a frame): the
whole three-PR series takes the frame from about 24,600 GL calls to about
15,400; this PR alone removes `<N>`. Every one of those calls crosses from wasm
into the browser's GL command buffer, so on a 2017 laptop they were a visible
part of the frame.

### Testing

Pixel-for-pixel identical frames against the unpatched build across four
scenes (WebGL2, Chrome); GL call counts from a CDP tracer. Not run: native GL,
`cargo xtask test`. `<owner: add what you ran>`

### Squash or Rebase?

Squash.

### LLM Use?

`<owner: write this by hand. wgpu's CONTRIBUTING asks that descriptions be
hand-written and that the author own the code. The code was written by an AI
assistant working with the author and reviewed by the author; this draft
description was also AI-written and must be rewritten in the owner's words.>`

### Checklist

- [x] I self-reviewed and fully understand this PR.
- [x] `CHANGELOG.md` entries for the user-facing effects of this change are present.
- [x] The PR is minimal, and doesn't make sense to land as multiple PRs.
- [x] Commits are logically scoped and individually reviewable.
- [x] The PR description has enough context to understand the motivation and solution implemented.
- [ ] (If applicable) WebGPU implementations built with `wgpu` may be affected behaviorally.
- [ ] (If applicable) Validation and feature gates are in place to confine behavioral changes.
- [ ] (If applicable) Tests demonstrate the validation and altered logic works.

### CHANGELOG entry (`## Unreleased` / `### Performance` / new `#### GLES`)

```
#### GLES

- `set_vertex_buffer` with the buffer and offset already bound no longer re-points that buffer's vertex attributes on the next draw. By @sequoia-hope in [#NNNN1](https://github.com/gfx-rs/wgpu/pull/NNNN1).
```

---

## PR 2: cache the bound program, textures and samplers per command buffer

**Title:** `gles: skip use_program, bind_texture, bind_sampler and index-buffer binds that GL already has`

**Comment wording for the diff** (replace the three `Midnight Playground (D1190)` / `D1190` comments):
- `mod.rs` `Queue::bind_cache`: `/// What GL has bound since the current command buffer began, so repeated binds of the same program, texture or sampler cost no GL call. Cleared by `reset_state` and by the copies that bind a texture outside `C::BindTexture`.`
- `mod.rs` `BindCache`: `/// GL's current program and texture-unit bindings, as `Queue::process` last set them.`
- `queue.rs` copies: `// Binds on the active unit behind the cache's back.`
- `queue.rs` `SetIndexBuffer`: `// Already bound.`
- `queue.rs` barrier: `// The next SetIndexBuffer must bind again.`
- `queue.rs` `BindTexture`: `// The unit already holds this texture with this mip range.`

Include `wgpu-hal-gles-2-fixup-untracked-binds.diff` in this PR.

### Connections

Part 2 of 3; independent of #NNNN1. #NNNN3 (per-texture mip range) builds on
this one's `BindTexture` change.

### Description

`Queue::process` issues `glUseProgram`, `glActiveTexture` + `glBindTexture`,
`glBindSampler` and `glBindBuffer(GL_ELEMENT_ARRAY_BUFFER)` for every
`SetProgram`, `BindTexture`, `BindSampler` and `SetIndexBuffer` command the
encoder recorded, and wgpu-core records a full bind group's worth of them on
every `set_bind_group`. Consecutive draws with the same pipeline and the same
textures (most of any frame) therefore rebind what GL already has.

This adds a small `BindCache` to `Queue`: the current program and, per texture
unit, the bound texture (with target and mip range) and sampler. A command
whose binding matches the cache is a no-op. The cache is only meaningful within
one command buffer: `reset_state`, which `submit` runs before each command
buffer while holding the context lock, clears it, so nothing outside the queue
can change GL state behind it. The three copy commands that `bind_texture` on
the active unit clear the texture entries; the shader-clear workaround records
the clear program; the buffer barrier that binds `GL_ELEMENT_ARRAY_BUFFER`
forgets the index buffer. `SetIndexBuffer` reuses the `current_index_buffer`
the queue already tracked, and `ClearBuffer` on an index buffer now records
the buffer it leaves bound.

Measured on a Bevy 0.19 scene under WebGL2 in Chrome (685 draws a frame; the
same scene in three.js makes about 4,400 GL calls for 780 draws): the whole
series takes the frame from about 24,600 GL calls to about 15,400, this PR
alone removes `<N>`; main-thread CPU down about 5 %, GPU time unchanged.

Textures the application imported (`texture_from_raw`, external GL interop)
are cached like any other from their first bind; an embedder that rebinds
texture units on a shared context between wgpu's own GL calls was already
outside what the backend tracks (`current_index_buffer` makes the same
assumption).

### Testing

As PR 1, plus: the cache invalidation paths that the game exercises are
`reset_state` (every frame) and `CopyBufferToTexture` (texture uploads).
`CopyTextureToTexture`, `CopyExternalImageToTexture`, `ClearBuffer` on an
index buffer and the shader-clear workaround were reasoned about, not run. `<owner: add what you ran>`

### Squash or Rebase?

Squash.

### LLM Use?

`<owner: as PR 1, by hand>`

### Checklist

Same as PR 1.

### CHANGELOG entry (`### Performance` / `#### GLES`)

```
- The GL backend now skips `glUseProgram`, `glBindTexture`, `glBindSampler` and index-buffer binds whose target is already bound, tracked per command buffer. By @sequoia-hope in [#NNNN2](https://github.com/gfx-rs/wgpu/pull/NNNN2).
```

---

## PR 3: set a texture's mip range only when it changes

**Title:** `gles: set TEXTURE_BASE_LEVEL / TEXTURE_MAX_LEVEL only when a view's mip range differs from the texture's last`

**Comment wording for the diff**:
- `mod.rs` `AdapterShared::tex_levels`: `/// The mip range each texture's `TEXTURE_BASE_LEVEL` and `TEXTURE_MAX_LEVEL` were last set to, so a bind sets them only when the view's range differs. Those parameters are texture state, not unit state. `destroy_texture` removes the entry, as GL may reuse the name.`
- `device.rs`: `// GL may give the name to a new texture.`
- `queue.rs`: `// Texture state, not unit state: set only when this view's range differs from the last one set.`

### Connections

Part 3 of 3. Depends on #NNNN2 (edits the same `BindTexture` arm).

### Description

Every `BindTexture` sets `TEXTURE_BASE_LEVEL` and `TEXTURE_MAX_LEVEL` to the
view's mip range: two `glTexParameteri` per texture per draw, and in GL these
are properties of the texture object, not of the texture unit, so re-setting
them for a texture whose range has not changed does nothing. In the measured
scene that was about 3,900 calls a frame.

`AdapterShared` gains `tex_levels`, a map from `glow::Texture` to the range it
was last set to. `BindTexture` sets the parameters only when the view's range
differs and records it. `destroy_texture` removes the entry, since GL can hand
the deleted name to a new texture. These are the only two places the backend
touches those parameters, so the map is authoritative for textures the backend
created; imported textures are tracked from their first bind.
`DEPTH_STENCIL_TEXTURE_MODE` is still set every bind and could use the same
map in a follow-up.

### Testing

As PR 2. The pixel comparison covers mip-mapped textures viewed with
different ranges (the game views the same textures through full-range views
only, so a reviewer may want a test with two views of one texture with
different `base_mip_level` bound in alternation).

### Squash or Rebase?

Squash.

### LLM Use?

`<owner: by hand>`

### Checklist

Same as PR 1.

### CHANGELOG entry (`### Performance` / `#### GLES`)

```
- The GL backend sets `TEXTURE_BASE_LEVEL` and `TEXTURE_MAX_LEVEL` only when a view's mip range differs from what the texture was last set to, instead of on every bind. By @sequoia-hope in [#NNNN3](https://github.com/gfx-rs/wgpu/pull/NNNN3).
```

---

## What the owner still does by hand

1. Apply `wgpu-hal-gles-2-fixup-untracked-binds.diff` to the local
   `patches/wgpu-hal/` too (both hunks), so the shipped build and the PR match.
2. Reword the comments (above) and drop the `D1190` references.
3. Fork gfx-rs/wgpu, branch from `trunk`, apply 1 (then 2 + fixup, then 3 on
   top), fix the one `CopyTextureToTexture` hunk by hand, `cargo fmt`,
   `cargo clippy -p wgpu-hal --features gles` and the wasm build
   (`cargo clippy --target wasm32-unknown-unknown -p wgpu-hal --features webgl`),
   `cargo xtask test` if wanted (see wgpu's `docs/testing.md`).
4. Measure `<N>` per PR with `gl-calls.mjs` (GPU machine only).
5. Write the LLM Use section and the descriptions in your own words; wgpu
   asks for that explicitly. No DCO sign-off is required (nothing in
   CONTRIBUTING asks for `Signed-off-by`).
6. Confirm `@sequoia-hope` is the GitHub handle to credit (it is the account
   `gh` is logged in as).
