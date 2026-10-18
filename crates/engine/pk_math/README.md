# pk_math

Deterministic scalar math for lockstep games: the same bits on every
platform, native and `wasm32-unknown-unknown`.

- `kernel`: every inexact `Math` function (`sin` .. `hypot_n`) on the `libm`
  crate, with ECMAScript's edge semantics for `pow` and `hypot`.
- `Rng`, `Mulberry32`, `rrange`, `rpick`, `hash2`: seeded, integer-exact
  randomness that can live inside a simulation state.
- `Noise2D`, `fbm`, `ridged`: simplex noise on exact operations only.
- `clamp`, `lerp`, `inv_lerp`, `smoothstep`, `damp`, `wrap_angle`.
- `vec`: `Vec3`, `Quat`, `Iso3`, `Aabb` in `f64`, exact operations only.
- `js` (cargo feature, on by default): `Math.round`, `Math.sign`, `max`/`min`
  with NaN and signed-zero ordering, `||`, ToInt32, `imul`, `fround`, for
  games ported from JavaScript.

## The rule

Nothing downstream calls `f64::sin` and friends, `powi` or `mul_add`:
those use the platform's math library on native, and two machines in a
lockstep game would drift apart within seconds. Every inexact function goes
through `kernel`. `+ - * /`, `sqrt`, `floor`, `ceil`, `trunc` and `abs` are
exact by IEEE 754 and need no wrapper. The crate itself is
`#![forbid(unsafe_code)]` with no clock, threads or hash maps, and `libm` is
pinned exactly, because a new libm version may round differently from the
goldens a game has recorded.

## Why not `noise`, `fastnoise-lite` or glam's `libm` feature

None of them promises bit-identical results across platforms, which is the
one property this crate exists for. `noise` and `fastnoise-lite` reach for
the platform's math and may change their algorithms between versions. glam
with the `libm` feature routes its trigonometry through libm, but it does
not promise that its operations stay exact or that SIMD stays off, and its
`f64` types pull in a dependency for what is a few hundred lines of `+ - * /`
and `sqrt` here. See `docs/ECOSYSTEM.md` rows 13 and 14 in the kit.

## Tests

`cargo test -p pk_math` natively and `cargo test -p pk_math --target
wasm32-unknown-unknown` under Node (`wasm-bindgen-test`; the kit's
`.cargo/config.toml` sets the runner). `tests/golden/math.json` is a copy of
Midnight Playground's math golden, written by its JS oracle with the kernel
on; `tests/golden.rs` requires the same bits.
