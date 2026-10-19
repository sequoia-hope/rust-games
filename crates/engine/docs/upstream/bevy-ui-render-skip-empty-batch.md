# bevy_ui_render: do not draw an empty UI batch

Drafted 2026-10-16 from `patches/bevy_ui_render/` (bevy_ui_render 0.19.1,
D1200). Nothing has been posted. `bevy-ui-render-skip-empty-batch.diff` is the
local patch against pristine 0.19.1: the same five-line guard in the five UI
draw commands (`render_pass.rs` `DrawUiNode`, `ui_material_pipeline.rs`,
`ui_texture_slice_pipeline.rs`, `box_shadow.rs`, `gradient.rs`). It applies to
0.19.1 (`patch --dry-run`) and to today's `main` (all five hunks, four with
line offsets, `render_pass.rs` at the same lines).

## Upstream state (checked 2026-10-16)

- `main`'s `DrawUiNode::render` (`crates/bevy_ui_render/src/render_pass.rs`,
  lines 209 to 244) is unchanged in substance: it sets the vertex and index
  buffers and calls `pass.draw_indexed(batch.range.clone(), 0, 0..1)` with no
  emptiness check. The other four draw commands are the same shape.
- `main`'s `prepare_uinodes` (`lib.rs`) still opens a batch at
  `range: vertices_index..vertices_index` and closes it with
  `existing_batch.unwrap().1.range.end = vertices_index` after the loop, and
  bumps the phase item's `batch_range` by one regardless, so a batch whose
  nodes added no vertices is still drawn.
- GitHub search of bevyengine/bevy for "index count of 0", "is unusual",
  "UiBatch empty", "empty batch ui", "prepare_uinodes" found nothing on this
  (the one hit for "index count of 0" is an unrelated bevy_solari PR). Not
  filed by anyone.

## Before posting

- Reword the five identical comments; they say "Midnight Playground (D1200)".
  Suggested: `// A batch whose nodes added no vertices has an empty range;`
  `// drawing it is a no-op that WebGPU implementations warn about.`
- Decide between the two fixes:
  1. **The guard in the draw commands** (this diff). Five near-identical
     hunks, obviously safe, and it also covers any future producer of an
     empty `UiBatch`. Bevy UI code review will likely accept it as is, or ask
     for `RenderCommandResult::Skip` instead of `Success` (either works; the
     patch returns `Success`, and `Skip` reads as the more accurate answer
     for "nothing to draw").
  2. **Fix at the source in `prepare_uinodes`**: when closing a batch whose
     range is empty, do not push the `UiBatch` (and do not bump the phase
     item's `batch_range`), so the render command gets `None` and returns
     `Skip` already. One place instead of five, but the batch is only known
     empty when it closes, and the other four pipelines have their own
     `prepare_*` loops with the same shape, so the source fix is also five
     places. The guard is the smaller PR.
  Recommendation: open with the guard; mention the source fix in the
  description and let the reviewer pick.
- Bevy's AI policy requires disclosure for AI-assisted work (see the template
  section below and bevy#25998's review, where a missing disclosure and a
  Cursor co-author trailer blocked an otherwise-approved PR). No DCO sign-off
  is required.
- Bevy's `main` is the 0.20 development branch. If you want the fix in a
  0.19.x point release, say so in the PR; the maintainers decide backports.

## Proposed title

`Skip drawing empty UI batches`

## PR description (Bevy's template)

# Objective

- `prepare_uinodes` (and the box-shadow, gradient, texture-slice and
  `UiMaterial` equivalents) opens a new batch with an empty index range and
  closes it after adding each node's quads. When a batch's nodes add no
  quads, e.g. a fully clipped node or an empty `Text`, the batch stays empty
  and is still drawn: `draw_indexed(0..0, 0, 0..1)`.
- On WebGPU this is a validation-clean no-op, but Chrome logs
  `Calling [RenderPassEncoder "ui"].Draw with an index count of 0 is unusual.`
  the first time it happens. In our game that is once, as the main menu first
  draws, and it fails our end-to-end suite's "no console warnings" check.
  Anyone running wasm tests against Chrome's console will hit the same.

## Solution

- In each of the five UI `RenderCommand` `render` implementations, return
  before `draw_indexed` / `draw` when `batch.range.is_empty()`. An empty draw
  does nothing, so no frame changes; the warning goes away.
- Alternative considered: skipping the empty batch where it is produced in
  `prepare_uinodes` and the four sibling `prepare_*` systems (do not insert
  the `UiBatch` when its range closes empty). That is the same number of
  places; the guard in the draw command is smaller and also catches any
  future producer. Happy to switch if you prefer the source fix.

## Testing

- Bevy 0.19.1 with this change, WebGPU build in Chrome: the warning is gone
  and our e2e suites pass; screenshots of the menu and HUD are unchanged.
  `<owner: add which examples you ran, e.g. `cargo run --example ui`,
  `text`, `box_shadow`, `gradients`, `ui_material`, on native and
  wasm (`cargo run -p build-wasm-example -- --api webgpu ui`)>`
- Reviewers can reproduce the warning with any UI scene that contains an
  empty `Text` or a node clipped to nothing, run in Chrome with the console
  open, before the change.

## AI Disclosure

`<owner: write in your own words. Factual basis: the five-line guard was
written by an AI assistant working with you in your codebase, you reviewed
and shipped it; this description was drafted by an AI assistant and edited by
you. Bevy's policy requires that you understand and can defend the change,
which here is small enough to be obvious.>`

## Labels a triager will add

`A-UI`, `A-Rendering`, `C-Bug`, `O-Web`, `D-Trivial`, and `AI-Assisted`
(because of the disclosure). You cannot set them yourself as a new
contributor; no need to ask.

## What the owner still does by hand

1. Reword the comments (above) and decide `Success` vs `Skip`.
2. Fork bevyengine/bevy, branch from `main`, apply the diff (it applies with
   offsets), `cargo fmt`, `cargo clippy -p bevy_ui_render`, run a UI example
   natively and, if set up, on wasm in Chrome to confirm the warning is gone.
3. Fill the Testing and AI Disclosure sections in your own words.
4. Locally, the patch can be dropped once the e2e no-warnings test filters
   this one message (ECOSYSTEM.md section 3); the patch and the filter are
   independent of the PR.
