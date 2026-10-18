# Upstream contributions drafted from `patches/`

Drafted 2026-10-16 for the owner to review. **Nothing here has been posted,
pushed, or filed.** Each `*.md` holds a proposed title, a description in the
project's PR template, the invariants a reviewer will ask about, the CHANGELOG
text, and what remains to do by hand. Each `*.diff` is a unified diff against
the pristine crates.io crate in `~/.cargo/registry/src/*/`, excluding
`Cargo.lock`, examples, and the `#![allow(warnings)]` line that only a path
dependency needs; every diff was checked with `patch --dry-run` against the
pristine crate. The diffs are faithful to the local patches, so their code
comments still say "Midnight Playground (D11xx)"; each `.md` gives the
wording to replace them with before posting.

## The four, in the recommended order (ECOSYSTEM.md section 3)

| # | Contribution | Files | Upstream status (2026-10-16) | Action |
|---|---|---|---|---|
| 1 | **wgpu-hal GL backend skips redundant binds** (D1190) | `wgpu-hal-gles-bind-cache.md`; whole patch `wgpu-hal-gles-bind-cache.diff`; split `wgpu-hal-gles-1-vertex-buffer-dirty.diff`, `wgpu-hal-gles-2-program-texture-sampler-cache.diff`, `wgpu-hal-gles-2-fixup-untracked-binds.diff`, `wgpu-hal-gles-3-texture-mip-range.diff` | Not filed by anyone; no prior art found. trunk still binds unconditionally everywhere the patch touches; the diffs apply to trunk with offsets except one `CopyTextureToTexture` hunk (restructured by #10004; a one-line manual rebase). | **Open three PRs** (1 and 2 independent, 3 on top of 2). First apply the fixup diff locally too: drafting found two binds the local cache does not see (the Mesa i915 shader-clear program, and `ClearBuffer` on an index buffer). Measure per-PR GL-call counts on the GPU machine. |
| 2 | **wgpu caches the WebGPU device's limits and features** (D1182) | `wgpu-webgpu-limits-cache.md`, `wgpu-webgpu-limits-cache.diff` | gfx-rs/wgpu#10513 (open since 2026-10-02, awaiting review, no comments) already does this on both adapter and device, with a CHANGELOG entry. Issue #10550 (2026-10-06) describes it, no comments. bevy#25998 (open, needs review) caches on Bevy's `RenderDevice` as well. | **No PR.** Post the comment drafted in the `.md` on #10550 (and optionally the short one on #10513) with the 7 %-of-a-phone-frame measurement. Drop the local patch when a wgpu release with #10513 reaches Bevy, or when bevy#25998 ships. |
| 3 | **bevy_ui_render does not draw an empty batch** (D1200) | `bevy-ui-render-skip-empty-batch.md`, `bevy-ui-render-skip-empty-batch.diff` | Not filed; nothing found in bevy's tracker. `main`'s `DrawUiNode` and `prepare_uinodes` are unchanged in substance; the diff applies to `main` with offsets. | **Open the small PR** (five identical guards). Locally, the patch can be dropped as soon as the e2e no-warnings test filters Chrome's "index count of 0 is unusual" message; the two are independent. |
| 4 | **wgpu WebGPU backend destroys buffers and textures when dropped** (D9, 2026-10-18) | `wgpu-webgpu-destroy-on-drop.md`, `wgpu-29-webgpu-destroy-on-drop.diff` (on the kit's wgpu 29), `wgpu-30-webgpu-destroy-on-drop.diff` (pristine 30.0.1) | gfx-rs/wgpu#4092 asked for exactly this and was closed in 2024 as not actionable over the cost of the refcounting it needs; nothing newer found. trunk still leaves dropped resources to the JS garbage collector. | **Reopen the question with numbers**: comment on #4092 (or a new issue citing it) with the Sierra A/B table, then offer the PR, perhaps behind an option. |

## Verification done here

- All seven diffs apply to the pristine crates with `patch --dry-run`.
- The wgpu-hal stack 1 + 2 + 3 reproduces `patches/wgpu-hal/src/gles/` byte
  for byte (the fixup adds two hunks on top that the local patch lacks).
- Trunk/main dry-runs as stated in the table; the wgpu limits diff does not
  apply to trunk (the surrounding `WebDevice` code moved) and does not need to.

## Common to all three

- No DCO / `Signed-off-by` is required by either project.
- wgpu asks that PR descriptions and large comments be written by hand and
  that the author own the code (CONTRIBUTING, "LLMs (AI)"); its template has
  an "LLM Use?" section. Bevy requires an AI disclosure section and its policy
  is enforced in review (bevy#25998 was held for it). The drafts here are
  AI-written and are meant as raw material, not as text to paste.
- GitHub handle assumed for CHANGELOG credit: `@sequoia-hope` (the account
  `gh` is logged in as here). Confirm.
