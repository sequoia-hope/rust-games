---
name: level-viewer
description: Use the level viewer ("god mode") of a Playground Kit game (today Midnight Playground's ?view=god) to review a generated level with the owner: the four cameras, links that carry a camera pose, hiding scene groups, the draw-call and triangle readout, screenshots, and how a draft is annotated and handed back. Use when asked to look at a level, check scenery placement, send the owner a view, compare two views, or when a world-making or render change needs eyes on it.
---

# Level viewer: the shared table

The viewer is Rust only and changes nothing in the game: the level built
whole, seen through a free camera. It becomes `pk_render::ViewerPlugin<R:
ViewerRoute>` in kit phase 5; today it is `crates/mp_game/src/viewer/` in
Midnight Playground (`~/Software/midnight-playground`; paths relative to it).

## Read first

- `docs/rust-port/SPEC.md` 8.6: entry, cameras, controls, panel, budgets,
  the test bridge.
- `docs/vision/WORLD.md` 10.1: how levels get designed together. The owner
  sets the feel in plain words with photos and real places; Claude drafts
  into the world plan and level data, never only into a chat; the viewer
  is where both look at the same thing.
- `crates/mp_game/src/viewer/mod.rs` header (the per-frame loop),
  `link.rs` header (every query parameter), `panel.rs` header (the panel's
  rows and the `vw-…` control ids), `groups.rs` (the top-level scene
  groups and how hiding works), `stats.rs` (what the readout counts).
- `tools/parity/viewer-shots.mjs`: every level from the overview, street
  level, orbit and ride, with frame time, draws, triangles, memory.

## Opening it

Through the registered server only (`proj up midnight-playground`):

```
/?view=god&level=peninsula
/?view=god&level=coast&mode=orbit&cam=120.5,18,-340,1.57,-0.2&orbit=140,6,-350&fog=0&far=1
```

Natively: `cargo run -p mp_game -- --query "view=god&level=coast"`. The
menu's level card has a **Level viewer** button, so a phone needs no URL.
The level is the client's own build (`?world=export` draws the export).

## Cameras

| Mode | `mode=` | What it is for |
|---|---|---|
| Free fly | `free` | walk or fly anywhere; `speed=` m/s, Shift faster; pointer lock to look |
| Orbit | `orbit` | circle, tilt and zoom about `orbit=x,y,z` (a picked point or the route at the slider) |
| Overview | `over` | top-down whole level with the route and zone boundaries; click to fly there |
| Ride | `ride` | the game's fly camera along the route: `s`, `h`, `back`, `lat`, `v`, `yaw`, `pitch` |

Keyboard and mouse (WASD, Q/E or Space/Ctrl, wheel for speed or zoom),
gamepad (sticks, triggers rise and sink, bumpers speed), touch (one finger
looks, a thumb stick moves, pinch zooms). The world's focus (chunk
cut-offs, LOD, sky dome, distance-shown nodes) follows the camera, as the
game's follows the car.

## Links carry a pose

The panel's **Link** copies the full query: `mode`, `cam=x,y,z,yaw,pitch`
(metres and radians, every mode), `orbit`, the ride parameters, the
toggles `fog`, `far`, `anim` (1 or 0), `t=` (time of day pinned at that
route fraction; absent, it follows the camera), `hide=a,b` (groups by
name), `speed`, `panel=0`. A link opens the same view exactly, so an
annotation is a link plus a sentence. The same string is the native
`--query`.

## Panel

Route position slider and zone name; time of day (follow or pin); fog,
extended far plane (the whole level; costs more, must not crash a phone),
animators running or frozen; the level's top-level **groups** (terrain,
road, each scenery module, sea, sky dome) shown or hidden, by moving their
entities to a render layer nothing draws; **readout**: camera position,
nearest route position, frame time, draw calls and triangles (counted as
three.js counts them: one call per drawn object, shadow pass included,
instanced triangles times instances; only while shown), wasm memory;
**screenshot** (a PNG download); **link**. Every control has an id
`vw-…` for the test bridge; `__mp.viewer` (to be `__game.viewer`) reports
mode and pose and takes a pose, so a script can place the camera.

## Annotating a draft with the owner

1. Build the draft into data (the plan, the level, the kit), never only
   into a chat. Push it.
2. Take the pictures: `cargo xtask web --release && node
   tools/parity/viewer-shots.mjs --levels peninsula --device iphone`
   writes `parity/report/viewer/<level>-<view>.png` and an `index.html`;
   view it at `/parity/report/viewer/` through the registered server.
3. Walk the level yourself in free fly first. Look for scenery on the
   tarmac, floating or sunk objects, seams between sections, zone
   boundaries that fall mid-town, the sea on dry land. Fix what you can
   see before asking the owner to look.
4. Send the owner **links, one per thing**, each with one sentence of what
   to look at and what you think. Hide groups to isolate a question
   (`hide=land,flora` shows the road and terrain alone). Pin `t=` so the
   light matches.
5. The owner answers in plain words ("trees cover the road", "the camera
   angle goes wrong on 84"). Turn each into a test that fails (D1218's
   `stream_clear.rs`, D1211's camera floor) before the fix, then a link
   that shows the fix at the same pose.
6. Record the round in DECISIONS: the owner's words, what was found, the
   test, the fix, and what is still open.

## Budgets

Free fly and orbit hold the race's frame-time and memory budgets (SPEC
6.6). Overview and the extended far plane may cost more; measure with
`viewer-shots.mjs --race` for the race on the same build beside them.

## Common mistakes

- Describing a view in words when a link would show it.
- Reviewing a drawn level's heights on the client: drawn levels have no
  terrain heights there (D434); open levels do.
- Forgetting that the viewer's level is built whole: a streaming bug
  (D1217's offset sections) does not show here. Drive the streamed level
  too (`drive-branch.mjs`).
- Taking screenshots on WebGPU for a pixel comparison: `build-diff.mjs`
  needs the WebGL2 canvas.
- Opening the page through any server but the registered one.
