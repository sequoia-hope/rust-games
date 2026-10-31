# rust-games

Hobby games in Rust on [Bevy](https://bevyengine.org/), made for the browser
(WebGPU, with a WebGL2 build for browsers without it) and for the desktop.
They have almost no asset files: terrain, models, textures, music and sound
are generated in code. All of them live here in one Cargo workspace, along
with the engine crates they share.

**Play them at https://sequoia-hope.github.io/rust-games/**

## The games

| Game | What it is | Play | Source |
|---|---|---|---|
| **Midnight Playground** | A street racer: sprints from mountain switchbacks to neon downtown, an endless night-city cruise, rivals, traffic and Hot Pursuit police. | [play](https://sequoia-hope.github.io/rust-games/midnight-playground/) | [games/midnight-playground](games/midnight-playground/) |
| **Snowboarding** | Race a CPU rider down the mountain, or ride alone against the clock or for trick points. Carve, ollie, spin, flip and grab, but land it. | [play](https://sequoia-hope.github.io/rust-games/snowboarding/) | [games/snowboarding](games/snowboarding/) |
| **Mech Works** | A four-tonne walking mech on a learned gait: nothing is animated, a neural network drives its twelve motors. Shoot it, break its actuators, swap its parts. | [play](https://sequoia-hope.github.io/rust-games/mech-works/) | [games/mech-works](games/mech-works/) |

There's more on the site too:

- **[Music Lab](https://sequoia-hope.github.io/rust-games/music-lab/)**
  ([apps/music-lab](apps/music-lab/)): the games' music engine on its own.
  It makes a synthwave, house, orchestral, techno, trance or drum & bass
  track from a seed, then lets you take it apart part by part and knob by
  knob.
- **[Midnight Playground Radio](https://sequoia-hope.github.io/rust-games/midnight-playground/tools/radio.html)**:
  the racer's radio stations on a dial, playing around the clock and
  composed as you listen.

## The shared crates

- **[Playground Kit](crates/engine/)** (`crates/engine`, `pk_*`): the shared
  engine. It has a deterministic math kernel, rng and noise (`pk_math`),
  lockstep simulation (`pk_lockstep`), a 2D canvas (`pk_canvas`), the Bevy
  client shell (`pk_shell`) and the build tooling behind each game's
  `cargo xtask` (`pk_xtask`). It also carries the patched wgpu and
  bevy_ui_render that every game builds on.
- **[The music engine](crates/music/)** (`crates/music`): procedural music.
  It has the instruments, sequencer, composer, genre grammars and mix, and
  the radio station player.

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) shows how the crates and games
fit together.

## Building

The toolchain is pinned in `rust-toolchain.toml` (Rust plus the
`wasm32-unknown-unknown` target), so `rustup` installs it on the first build.

```sh
cargo nextest run --workspace                           # every test
cd games/snowboarding && cargo xtask web --release      # a game's web build, into its dist/
```

Each game's own README has the details for that game. Every push
to `main` rebuilds and publishes the site (`.github/workflows/pages.yml`,
from the list in [site/games.toml](site/games.toml)).
