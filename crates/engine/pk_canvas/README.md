# pk_canvas

A subset of the browser's Canvas 2D API on [tiny-skia], with text shaped by
[harfrust] (HarfBuzz in Rust) and outlined by [skrifa], tuned pixel by pixel
against Chrome's GPU canvas. Method names are the Canvas 2D names in snake
case (`fill_rect`, `create_linear_gradient`, `set_global_composite_operation`,
`put_image_data`...), so a JavaScript game's texture generators port line
for line and the goldens captured from the browser still hold. Rectangles,
paths, arcs and ellipses, `roundRect`, linear and radial gradients with
Skia's dither, `globalAlpha`, the composite operations in use, `shadowBlur`,
`filter: blur()`, clips, `fillText`/`strokeText`/`measureText`,
`getImageData`/`putImageData`, `drawImage`, transforms and save/restore.
The crate's `lib.rs` lists every call and which of Chrome's rules each
follows; the measurements behind them are Midnight Playground's
DECISIONS D151 to D153 and D650 to D653.

It builds for native and `wasm32-unknown-unknown` with
`#![forbid(unsafe_code)]`, and every inexact function goes through
`pk_math`'s kernel, so the same bytes come out on every platform: a world
builder in a web worker and a native test draw the same texture.

## Fonts

The kit carries no fonts. A game builds a `FontBook` once from the files it
bundles, under the family names its texture code asks for, and draws on
`Canvas::with_fonts`:

```rust
use std::sync::Arc;
use pk_canvas::{Canvas, FontBook};

let roboto: Arc<[u8]> = Arc::from(&include_bytes!("fonts/Roboto[wdth,wght].ttf")[..]);
let mut book = FontBook::new();
book.add("Arial", roboto.clone(), (100.0, 900.0), false, "Roboto");
book.add("Arial Black", roboto, (900.0, 900.0), false, "Roboto"); // pinned at 900
book.set_generic("sans-serif", "Arial");
book.add_fallback("Arimo"); // for a character no requested family has
let book = Arc::new(book);

let mut g = Canvas::with_fonts(1024, 160, book.clone());
g.set_font("bold 64px \"Arial Narrow\", Arial, sans-serif");
g.fill_text("PORT MERIDIAN", 512.0, 80.0);
```

`FontBook` is the canvas's `document.fonts`: `add` is `new FontFace(family,
data, {weight, style})`, `select` is CSS font matching (style first, then
weight; a weight range serves a variable font's `wght` axis), `set_generic`
maps `sans-serif` and friends, and the fallback list is what a browser's
system-font fallback is to a port. `Canvas::new` has no fonts: text draws
nothing and measures zero. A port of browser code that names system fonts
registers open-licence substitutes under those names (Midnight Playground
does this from one manifest, `assets/fonts/fonts.json`, baked by its own
`build.rs`), so every device draws the same signs. There is no
`bundled-fonts` feature: `include_bytes!` in the game is the whole
mechanism (kit DECISIONS D4).

## Why not an existing crate

- **vello** renders paths and gradients on the GPU, with a CPU fallback
  that is still settling, and does not aim at Chrome's exact coverage; the
  textures here are drawn once on the CPU, in a worker, and compared to
  browser captures to a mean difference under 3/255.
- **cosmic-text** lays out paragraphs with fontdb and swash. Canvas text is
  one line, shaped word by word as Blink does, with glyph origins on Skia's
  quarter-pixel grid and its A8 mask gamma; that is a few hundred lines on
  harfrust and skrifa, and cosmic-text's own hinting and rasteriser would
  change the pixels the goldens were captured against.
- **bevy-canvas-2d** is unstable, on Bevy 0.17, and draws into Bevy's
  renderer; this crate has no engine, so world generation stays a library
  (PLAN.md section 2) and the same code runs in a web worker.

`docs/ECOSYSTEM.md` section 2 item 9 names this as something the kit has
that the ecosystem does not; tiny-skia, harfrust and skrifa are the parts
worth reusing, and are.

[tiny-skia]: https://github.com/linebender/tiny-skia
[harfrust]: https://github.com/harfbuzz/harfrust
[skrifa]: https://github.com/googlefonts/fontations
