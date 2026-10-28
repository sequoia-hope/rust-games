//! The atlas (`?view=atlas`, the menu's ATLAS tab): the open world seen
//! whole in 3D, every place a run of it can start marked with a pin. It is
//! the level viewer (this module's parent) on the peninsula with a map's
//! camera and a UI of its own in place of the viewer's panel.
//!
//! - **The world** is the peninsula's own streamed build (`stream`): the
//!   base's coarse terrain, sea and sky everywhere, and the sections round
//!   the orbit's centre, so a pin flown down to shows its real road. The
//!   level is night the whole way round; the atlas lights it with a late
//!   afternoon's sky ([`day_keys`]) and no fog, so the land reads.
//! - **The pins** ([`pins`]) are the starts of the menu's open-world levels
//!   (`mp_levels::ATLAS_LEVELS`): each level's race start and each of its
//!   cruise starts, those within [`MERGE`] of one another one pin. A pin's
//!   card lists what can start there, by level, each mode a button.
//! - **A run** is saved as the menu saves its choices (the level, its mode
//!   and its start) and the game started on it (`?level=…&race=1`), which
//!   reads them as a run from the menu does.
//! - **Controls:** a drag (one finger, the left button) moves the map, the
//!   right button turns and tilts it, the wheel or a pinch zooms; a tap on
//!   a pin (or its name in the card's list) flies to it. Keys: WASD or the
//!   arrows move, Q and E turn, R and F tilt, + and − (or Z and X) zoom,
//!   Tab and Shift+Tab step through the pins, [ and ] through a pin's
//!   runs once one is picked (Enter starts it), Home shows the whole map,
//!   Esc goes back. Pad: left stick moves, right stick turns and tilts, the
//!   triggers zoom, the D-pad's ◂ ▸ step through the pins and ▴ ▾ through
//!   the runs, A starts the run, Y the whole map, B back.

use super::cams::{self, Orbit, Pose};
use super::panel::{PanelArea, VControl, bp};
use super::{Act, Viewer};
use crate::status::Status;
use crate::ui::widgets::{self as w, Bp, T};
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::text::TextLayout;
use bevy::ui::{UiTransform, Val2};
use mp_track::Track;

/// Starts closer than this (m) share a pin.
pub const MERGE: f64 = 400.0;
/// How steeply the map is seen, and how far a flight to a pin comes down.
const PITCH: f64 = -0.95;
const PITCH_MIN: f64 = -1.5;
const PITCH_MAX: f64 = -0.22;
const PIN_DIST: f64 = 1400.0;
const PIN_PITCH: f64 = -0.62;
const DIST_MIN: f64 = 120.0;

/// A run a pin offers: a level, its mode (`race`, `pursuit` or
/// `cruise`, the menu's store values) and, for a cruise, the start.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub level: &'static str,
    pub mode: &'static str,
    pub start: Option<&'static str>,
}

impl Run {
    pub fn label(&self) -> &'static str {
        match self.mode {
            "pursuit" => "Hot Pursuit",
            "cruise" => "Cruise",
            _ => "Race",
        }
    }

    /// The address that starts it (the store holds the rest, [`save`]).
    pub fn query(&self) -> String {
        format!("level={}&race=1", self.level)
    }
}

/// A place on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    pub id: &'static str,
    pub name: &'static str,
    pub place: &'static str,
    pub pos: DVec3,
    /// By level, in the order of `ATLAS_LEVELS`: the level's title and
    /// its runs here.
    pub groups: Vec<(&'static str, Vec<Run>)>,
}

impl Pin {
    pub fn runs(&self) -> impl Iterator<Item = &Run> {
        self.groups.iter().flat_map(|(_, r)| r)
    }

    fn add(&mut self, title: &'static str, run: Run) {
        match self.groups.iter_mut().find(|(t, _)| *t == title) {
            Some((_, rs)) => {
                if !rs.contains(&run) {
                    rs.push(run);
                }
            }
            None => self.groups.push((title, vec![run])),
        }
    }
}

/// Where a road of `track`'s network is at `s` (negative: from its end,
/// as `Track::chosen_start` reads a start).
fn at(track: &Track, road: &str, s: f64) -> Option<DVec3> {
    let r = track.road_index(road)?;
    let t = track.road(r);
    let s = if s < 0.0 { t.length + s } else { s };
    let f = t.frame(s.clamp(0.0, t.length));
    Some(DVec3::new(f.x, f.y, f.z))
}

/// The pins of the atlas's levels: every cruise start, in each level's
/// order, then each level's race start; a start within [`MERGE`] of a pin
/// joins it.
pub fn pins() -> Vec<Pin> {
    let mut pins: Vec<Pin> = Vec::new();
    let place = |pins: &mut Vec<Pin>, pos: DVec3, new: Pin, title, run: Run| {
        let near = pins.iter_mut().find(|p| {
            let d = p.pos - pos;
            d.x.hypot(d.z) < MERGE
        });
        match near {
            Some(p) => p.add(title, run),
            None => {
                let mut p = new;
                p.add(title, run);
                pins.push(p);
            }
        }
    };
    let levels: Vec<_> = mp_levels::ATLAS_LEVELS
        .iter()
        .map(|id| mp_levels::level_by_id(id))
        .collect();
    let tracks: Vec<_> = levels
        .iter()
        .map(|l| {
            Track::new(l)
                .map_err(|e| warn!("atlas: {}: {e}", l.id))
                .ok()
        })
        .collect();
    for (l, t) in levels.iter().zip(&tracks) {
        let Some(t) = t else { continue };
        if !l.open {
            continue;
        }
        for sp in &l.starts {
            let Some(pos) = at(t, sp.road, sp.s) else {
                warn!("atlas: {}: start {} is on no road", l.id, sp.id);
                continue;
            };
            let pin = Pin {
                id: sp.id,
                name: sp.name,
                place: sp.place,
                pos,
                groups: Vec::new(),
            };
            let run = Run {
                level: l.id,
                mode: "cruise",
                start: Some(sp.id),
            };
            place(&mut pins, pos, pin, l.title, run);
        }
    }
    for (l, t) in levels.iter().zip(&tracks) {
        let Some(t) = t else { continue };
        let Some(pos) = at(t, "", t.start_s) else {
            continue;
        };
        let pin = Pin {
            id: l.id,
            name: l.title,
            place: l.num,
            pos,
            groups: Vec::new(),
        };
        let mut modes = vec!["race"];
        if l.police.is_some() {
            modes.push("pursuit");
        }
        // A cruise from the race's start, where the level has no starts
        // of its own (they are pinned above).
        if l.open && l.starts.is_empty() {
            modes.push("cruise");
        }
        for mode in modes {
            let run = Run {
                level: l.id,
                mode,
                start: None,
            };
            place(&mut pins, pos, pin.clone(), l.title, run);
        }
    }
    // The levels in the menu's order, each one's runs as its card listed
    // its modes.
    for p in &mut pins {
        for (_, runs) in &mut p.groups {
            runs.sort_by_key(|r| {
                ["race", "pursuit", "cruise"]
                    .iter()
                    .position(|m| *m == r.mode)
            });
        }
        p.groups.sort_by_key(|(t, _)| {
            levels
                .iter()
                .position(|l| l.title == *t)
                .unwrap_or(usize::MAX)
        });
    }
    pins
}

/// The atlas's sky: Sierra's late afternoon (its key at 0.18), the sun
/// low enough that the hills stand out in relief.
pub fn day_keys() -> Vec<mp_track::SkyKey> {
    let day = mp_track::SkyKey {
        s: 0.0,
        sun_el: 17.0,
        zen: 0x3a64a8,
        hor: 0xe8cfa6,
        sun: 0xffdca8,
        sun_i: 3.3,
        hemi_s: 0xc0cfe6,
        hemi_g: 0x857a64,
        hemi_i: 1.45,
        fog: 0xd2c3a8,
        fog_d: 0.00025,
        exp: 1.0,
        night: 0.0,
    };
    vec![day, mp_track::SkyKey { s: 1.0, ..day }]
}

/// The atlas's state in the viewer.
#[derive(Clone, Debug, Default)]
pub struct AtlasMode {
    pub pins: Vec<Pin>,
    /// The pin picked, and the run focused on its card (keys and pads).
    pub sel: Option<usize>,
    pub run: usize,
    /// The level's sky swapped for the day's.
    pub sky_set: bool,
    /// A run asked for: the game starts on it once the frame is done.
    pub go: Option<Run>,
}

impl AtlasMode {
    pub fn selected(&self) -> Option<&Pin> {
        self.sel.and_then(|i| self.pins.get(i))
    }

    pub fn focused_run(&self) -> Option<&Run> {
        self.selected()?.runs().nth(self.run)
    }
}

/// The orbit that shows the whole network, seen from the south at
/// [`PITCH`].
pub fn whole(v: &Viewer) -> Orbit {
    let b = v.bounds;
    let (w, d) = (b[1] - b[0], b[3] - b[2]);
    let aspect = v.view.0 / v.view.1.max(1.0);
    // Tilted, the box's depth is foreshortened by the pitch's sine.
    let fit = cams::fit_height(w, d * (-PITCH).sin(), aspect);
    // A little south of the middle, so the map stands clear of the card
    // along the screen's foot (the crest, the races' start, is the
    // southernmost pin).
    Orbit {
        centre: DVec3::new((b[0] + b[1]) / 2.0, v.ground, (b[2] + b[3]) / 2.0 + d * 0.1),
        dist: fit * 1.08,
        yaw: 0.0,
        pitch: PITCH,
    }
}

/// The farthest the camera may pull back.
pub fn dist_max(v: &Viewer) -> f64 {
    whole(v).dist * 1.8
}

/// The orbit over a pin.
pub fn over_pin(pin: &Pin, yaw: f64) -> Orbit {
    Orbit {
        centre: pin.pos,
        dist: PIN_DIST,
        yaw,
        pitch: PIN_PITCH,
    }
}

/// A frame of the map's camera: `pan` drags the ground under the pointer,
/// the keys and the left stick move over it, `look` turns and tilts,
/// `zoom` and `up` come down or pull back.
pub fn step(o: &mut Orbit, d: &cams::Drive, dt: f64, view_h: f64, max: f64) {
    o.yaw = cams::wrap_angle(o.yaw + d.look.0);
    o.pitch = (o.pitch + d.look.1).clamp(PITCH_MIN, PITCH_MAX);
    let zoom = d.zoom + d.up * dt * 1.5;
    o.dist = (o.dist * (-zoom).exp()).clamp(DIST_MIN, max.max(DIST_MIN * 2.0));
    let p = Pose::new(DVec3::ZERO, o.yaw, o.pitch);
    // Ground metres per px at the centre; along the view the ground is
    // stretched by the tilt.
    let k = 2.0 * o.dist * (cams::FOV_Y_DEG.to_radians() / 2.0).tan() / view_h.max(1.0);
    let stretch = 1.0 / (-o.pitch).sin().max(0.35);
    o.centre -= p.right() * d.pan.0 * k;
    o.centre += p.ahead() * d.pan.1 * k * stretch;
    let rate = o.dist * 0.9 * if d.boost > 0.0 { d.boost } else { 1.0 };
    o.centre += (p.ahead() * d.fwd + p.right() * d.right) * rate * dt;
}

/// Saves a run's choices where the menu keeps them (`ui::store`), so the
/// game started on it, and the menu after it, have them.
pub fn save(run: &Run) {
    let mut store = crate::ui::store::platform();
    store.set_str("level", run.level);
    store.set_str(&format!("mode.{}", run.level), run.mode);
    if let Some(s) = run.start {
        store.set_str(&format!("start.{}", run.level), s);
    }
}

// ── The map under the world ─────────────────────────────────────────────

/// The base leaves each road's corridor to its sections, which come up
/// only round the point the camera looks at: from above the rest would
/// be holes. The underlay is the level's own ground (`Level::ground`)
/// over the whole network's box, coloured by its land cover and shaded
/// by the sun, a little below the base so it shows only where the base
/// has no tiles; the roads are drawn over everything from afar.
#[derive(Component)]
pub struct Underlay;
#[derive(Component)]
pub struct RoadLines(pub f64);

/// How far below the ground the underlay lies (m), and its cells across
/// the box's long side.
const DROP: f64 = 20.0;
const CELLS: f64 = 420.0;
/// Below this orbit distance the roads' lines go: the real roads show.
const LINES_NEAR: f64 = 2500.0;
/// The lines' width, metres per metre of orbit distance (about two and a
/// half pixels across, a street's half that).
const LINE_W: f64 = 0.0035;

fn cover_rgb(c: u8) -> [f32; 3] {
    use mp_track::Cover;
    let hex = match Cover::of(c) {
        Cover::Sea => 0x23405a,
        Cover::Water => 0x335a7a,
        Cover::Urban => 0xb3ab9c,
        Cover::Crop => 0x9fae6a,
        Cover::Grass => 0xb0a66a,
        Cover::Shrub => 0x7f8a52,
        Cover::Forest => 0x3e5c33,
        Cover::Barren => 0xa89a80,
        Cover::Wetland => 0x6f8a6a,
        Cover::Sand => 0xd8c9a2,
    };
    let c = Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8).to_linear();
    [c.red, c.green, c.blue]
}

fn unlit(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        ..default()
    })
}

/// The underlay's mesh: a grid over `b` (min x, max x, min z, max z).
pub fn underlay_mesh(track: &Track, b: [f64; 4]) -> Option<Mesh> {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let ground = track.level.ground.as_ref()?;
    let cover = track.level.cover.as_ref();
    let (w, d) = (b[1] - b[0], b[3] - b[2]);
    let step = (w.max(d) / CELLS).max(20.0);
    let (nx, nz) = (
        (w / step).ceil() as usize + 1,
        (d / step).ceil() as usize + 1,
    );
    let mut h = vec![0.0f64; nx * nz];
    for j in 0..nz {
        for i in 0..nx {
            h[j * nx + i] = ground(b[0] + i as f64 * step, b[2] + j as f64 * step);
        }
    }
    let sun = DVec3::new(-0.45, 0.8, -0.4).normalize();
    let mut pos = Vec::with_capacity(nx * nz);
    let mut col = Vec::with_capacity(nx * nz);
    for j in 0..nz {
        for i in 0..nx {
            let (x, z) = (b[0] + i as f64 * step, b[2] + j as f64 * step);
            let hy = h[j * nx + i];
            let hx = h[j * nx + (i + 1).min(nx - 1)] - h[j * nx + i.saturating_sub(1)];
            let hz = h[(j + 1).min(nz - 1) * nx + i] - h[j.saturating_sub(1) * nx + i];
            let n = DVec3::new(-hx, 2.0 * step, -hz).normalize();
            let shade = (0.55 + 0.55 * n.dot(sun).max(0.0)) as f32;
            let c = cover.map_or([0.4, 0.42, 0.3], |f| cover_rgb(f(x, z)));
            pos.push([x as f32, (hy - DROP) as f32, z as f32]);
            col.push([c[0] * shade, c[1] * shade, c[2] * shade, 1.0]);
        }
    }
    let mut idx = Vec::with_capacity((nx - 1) * (nz - 1) * 6);
    for j in 0..nz - 1 {
        for i in 0..nx - 1 {
            let a = (j * nx + i) as u32;
            let (b1, c1, d1) = (a + 1, a + nx as u32, a + nx as u32 + 1);
            idx.extend_from_slice(&[a, c1, b1, b1, c1, d1]);
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let normals = vec![[0.0f32, 1.0, 0.0]; pos.len()];
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    Some(m)
}

/// The roads' lines at a width for orbit distance `dist`: a ribbon per
/// polyline of the network (`play::atlas::field`, thinned), a little over
/// the ground, majors orange, streets pale.
pub fn lines_mesh(track: &Track, lines: &[crate::play::atlas::field::Line], dist: f64) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let ground = track.level.ground.as_ref();
    let lift = (dist * 0.002).clamp(4.0, 120.0);
    let mut pos = Vec::new();
    let mut col = Vec::new();
    let mut idx = Vec::new();
    let major = Color::srgb_u8(0xff, 0x8a, 0x3c).to_linear();
    let minor = Color::srgb_u8(0xf0, 0xe6, 0xd2).to_linear();
    for l in lines {
        let half = dist * LINE_W * if l.major { 0.5 } else { 0.3 };
        let c = if l.major { major } else { minor };
        let c = [c.red, c.green, c.blue, 1.0];
        for seg in l.pts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let (dx, dz) = (b.0 - a.0, b.1 - a.1);
            let len = dx.hypot(dz);
            if len < 1e-6 {
                continue;
            }
            // Each segment its own quad, lengthened by the half-width at
            // both ends so the joints close.
            let (ux, uz) = (dx / len, dz / len);
            let (rx, rz) = (-uz * half, ux * half);
            let (a, b) = (
                (a.0 - ux * half, a.1 - uz * half),
                (b.0 + ux * half, b.1 + uz * half),
            );
            let y = |p: (f64, f64)| ground.map_or(0.0, |g| g(p.0, p.1)) + lift;
            let (ya, yb) = (y(a), y(b));
            let base = pos.len() as u32;
            for (x, yy, z) in [
                (a.0 - rx, ya, a.1 - rz),
                (a.0 + rx, ya, a.1 + rz),
                (b.0 - rx, yb, b.1 - rz),
                (b.0 + rx, yb, b.1 + rz),
            ] {
                pos.push([x as f32, yy as f32, z as f32]);
                col.push(c);
            }
            idx.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
        }
    }
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let normals = vec![[0.0f32, 1.0, 0.0]; pos.len()];
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
    m
}

/// The network's lines, once.
#[derive(Resource, Default)]
pub struct NetLines(pub Option<Vec<crate::play::atlas::field::Line>>);

/// The underlay once the viewer has started; the roads' lines rebuilt as
/// the camera's distance changes by half again, and gone close up.
#[allow(clippy::too_many_arguments)]
pub fn map_layers(
    mut commands: Commands,
    v: Res<Viewer>,
    tr: Res<crate::TrackRes>,
    status: Res<Status>,
    mut net: ResMut<NetLines>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    under: Query<(), With<Underlay>>,
    mut roads: Query<(Entity, &RoadLines, &mut Visibility)>,
) {
    let Some(track) = &tr.track else { return };
    if v.atlas_mode.is_none() || !status.ready || v.bounds[0] >= v.bounds[1] {
        return;
    }
    if under.is_empty() {
        let b = v.bounds;
        let m = 3000.0;
        if let Some(mesh) = underlay_mesh(track, [b[0] - m, b[1] + m, b[2] - m, b[3] + m]) {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(unlit(&mut materials)),
                Transform::IDENTITY,
                bevy::light::NotShadowCaster,
                Underlay,
            ));
        }
    }
    let lines = net
        .0
        .get_or_insert_with(|| crate::play::atlas::field::network_lines(track));
    let dist = v.orbit.dist;
    let near = dist < LINES_NEAR;
    // Drawn for a distance within half again of this one.
    let fits = |r: &RoadLines| (1.0 / 1.5..1.5).contains(&(dist / r.0));
    let mut have = false;
    for (e, r, mut vis) in &mut roads {
        if !fits(r) {
            commands.entity(e).despawn();
            continue;
        }
        have = true;
        let want = if near {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
    if !have && !near {
        commands.spawn((
            Mesh3d(meshes.add(lines_mesh(track, lines, dist))),
            MeshMaterial3d(unlit(&mut materials)),
            Transform::IDENTITY,
            bevy::light::NotShadowCaster,
            RoadLines(dist),
        ));
    }
}

// ── The UI ──────────────────────────────────────────────────────────────

/// The atlas's header and card (rebuilt when `Viewer::dirty`).
#[derive(Component)]
pub struct AtlasUi;

/// The pins' layer, and a pin (moved each frame).
#[derive(Component)]
pub struct PinsRoot;
#[derive(Component)]
pub struct PinNode(pub usize);

/// The pin's dot: its centre is the pin's point, this far (CSS px) from
/// the node's corner.
const DOT: f32 = 14.0;
const DOT_AT: f32 = 12.0;

fn chip(p: &mut ChildSpawnerCommands, bp: &Bp, id: &str, act: Act, label: &str, sel: bool) {
    let c = bp.compact;
    let mut e = p.spawn((
        Node {
            padding: UiRect::axes(
                bp.px(if c { 9.0 } else { 12.0 }),
                bp.px(if c { 4.0 } else { 5.0 }),
            ),
            border: w::border(bp, 1.0),
            border_radius: w::pill(),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderColor::all(if sel { w::accent() } else { w::white(0.22) }),
        BackgroundColor(if sel {
            w::rgba(0xff3860, 0.3)
        } else {
            w::white(0.07)
        }),
        VControl {
            id: id.into(),
            act: Some(act),
            value: serde_json::json!(label),
            sel,
        },
    ));
    if sel {
        e.insert(w::hgrad(w::accent(), w::rgb(0xff7a3c)));
    }
    e.with_children(|p| {
        p.spawn((
            w::text(
                label.to_uppercase(),
                T::new(if c { 11.0 } else { 12.5 })
                    .bold()
                    .ls(0.12)
                    .c(if sel { Color::WHITE } else { w::fg() }),
                bp.k,
            ),
            TextLayout::no_wrap(),
        ));
    });
}

fn panel_node(bp: &Bp) -> (Node, BackgroundColor, BorderColor, GlobalZIndex, PanelArea) {
    let c = bp.compact;
    (
        Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            row_gap: bp.px(if c { 6.0 } else { 8.0 }),
            padding: UiRect::axes(
                bp.px(if c { 10.0 } else { 14.0 }),
                bp.px(if c { 8.0 } else { 12.0 }),
            ),
            border: w::border(bp, 1.0),
            border_radius: w::radius(bp, 14.0),
            ..default()
        },
        BackgroundColor(w::rgba(0x080a12, 0.82)),
        BorderColor::all(w::white(0.14)),
        GlobalZIndex(20),
        PanelArea,
    )
}

/// The header (top left: the title, the way back, the controls) and the
/// card (bottom: the picked pin's runs, or every pin to pick from), and
/// the pins' layer once.
pub fn build(
    mut commands: Commands,
    mut v: ResMut<Viewer>,
    roots: Query<Entity, With<AtlasUi>>,
    pins_root: Query<Entity, With<PinsRoot>>,
    fonts: Option<Res<w::UiFonts>>,
) {
    if !v.dirty || fonts.is_none() {
        return;
    }
    let Some(a) = v.atlas_mode.clone() else {
        return;
    };
    v.dirty = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
    let bp = bp(&v);
    let c = bp.compact;
    let m = if c { 8.0 } else { 12.0 };
    let ins = v.insets;

    // The header.
    let (mut n, bg, border, z, area) = panel_node(&bp);
    n.left = bp.px(m + ins[3]);
    n.top = bp.px(m + ins[0]);
    n.max_width = bp.px((bp.w - 2.0 * m).min(420.0));
    commands
        .spawn((n, bg, border, z, area, AtlasUi))
        .with_children(|p| {
            p.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: bp.px(12.0),
                ..default()
            })
            .with_children(|p| {
                chip(p, &bp, "at-back", Act::Back, "Menu", false);
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|p| {
                    p.spawn((
                        w::text(
                            "THE PENINSULA",
                            T::new(10.5).bold().ls(0.35).c(w::accent()),
                            bp.k,
                        ),
                        TextLayout::no_wrap(),
                    ));
                    p.spawn((
                        w::text("Atlas", T::new(if c { 18.0 } else { 22.0 }).bold(), bp.k),
                        TextLayout::no_wrap(),
                    ));
                });
            });
            if !c {
                let help = if v.touch {
                    "Drag to move \u{00B7} pinch to zoom \u{00B7} tap a pin"
                } else {
                    "Drag to move \u{00B7} right-drag to turn \u{00B7} scroll to zoom \u{00B7} click a pin"
                };
                p.spawn(w::text(help, T::new(12.0).c(w::dim()), bp.k));
            }
        });

    // The card.
    let (mut n, bg, border, z, area) = panel_node(&bp);
    let width = (bp.w - 2.0 * m - ins[1] - ins[3]).min(560.0);
    n.left = Val::Percent(50.0);
    n.margin = UiRect::left(bp.px(-width / 2.0));
    n.width = bp.px(width);
    n.bottom = bp.px(m + ins[2]);
    commands
        .spawn((n, bg, border, z, area, AtlasUi))
        .with_children(|p| match a.selected() {
            Some(pin) => {
                p.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::FlexStart,
                    column_gap: bp.px(10.0),
                    ..default()
                })
                .with_children(|p| {
                    p.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|p| {
                        p.spawn((
                            w::text(pin.place, T::new(10.5).bold().ls(0.35).c(w::accent()), bp.k),
                            TextLayout::no_wrap(),
                        ));
                        p.spawn((
                            w::text(
                                pin.name,
                                T::new(if c { 18.0 } else { 24.0 }).bold().italic(),
                                bp.k,
                            ),
                            TextLayout::no_wrap(),
                            VControl {
                                id: "at-pin-name".into(),
                                act: None,
                                value: serde_json::json!(pin.id),
                                sel: false,
                            },
                        ));
                    });
                    chip(p, &bp, "at-whole", Act::Whole, "Whole map", false);
                });
                let mut k = 0;
                for (title, runs) in &pin.groups {
                    p.spawn(Node {
                        align_items: AlignItems::Center,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: bp.px(8.0),
                        row_gap: bp.px(6.0),
                        ..default()
                    })
                    .with_children(|p| {
                        p.spawn((
                            w::text(*title, T::new(if c { 13.0 } else { 15.0 }).bold(), bp.k),
                            TextLayout::no_wrap(),
                            Node {
                                min_width: bp.px(if c { 130.0 } else { 190.0 }),
                                ..default()
                            },
                        ));
                        for r in runs {
                            let id = format!("at-run-{}-{}", r.level, r.mode);
                            chip(p, &bp, &id, Act::Run(k), r.label(), k == a.run);
                            k += 1;
                        }
                    });
                }
            }
            None => {
                p.spawn(w::text(
                    "Pick a place to start",
                    T::new(if c { 15.0 } else { 18.0 }).bold(),
                    bp.k,
                ));
                p.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: bp.px(6.0),
                    row_gap: bp.px(6.0),
                    ..default()
                })
                .with_children(|p| {
                    for (i, pin) in a.pins.iter().enumerate() {
                        chip(
                            p,
                            &bp,
                            &format!("at-pick-{}", pin.id),
                            Act::Pin(i),
                            pin.name,
                            false,
                        );
                    }
                });
            }
        });

    // The pins, the picked one marked (`place` moves them).
    for e in &pins_root {
        commands.entity(e).despawn();
    }
    {
        let k = bp.k;
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                GlobalZIndex(5),
                PinsRoot,
            ))
            .with_children(|p| {
                for (i, pin) in a.pins.iter().enumerate() {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            height: Val::Px(2.0 * DOT_AT * k),
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0 * k),
                            padding: UiRect::new(
                                Val::Px((DOT_AT - DOT / 2.0) * k),
                                Val::Px(0.0),
                                Val::Px(0.0),
                                Val::Px(0.0),
                            ),
                            ..default()
                        },
                        UiTransform::IDENTITY,
                        Visibility::Hidden,
                        PinNode(i),
                        VControl {
                            id: format!("at-pin-{}", pin.id),
                            act: Some(Act::Pin(i)),
                            value: serde_json::json!(pin.name),
                            sel: false,
                        },
                    ))
                    .with_children(|p| {
                        let sel = a.sel == Some(i);
                        p.spawn((
                            Node {
                                width: Val::Px(DOT * k),
                                height: Val::Px(DOT * k),
                                border: UiRect::all(Val::Px(2.0 * k)),
                                border_radius: w::pill(),
                                ..default()
                            },
                            BackgroundColor(if sel { w::gold() } else { w::accent() }),
                            BorderColor::all(Color::WHITE),
                            w::shadow(
                                &bp,
                                &[(
                                    0.0,
                                    0.0,
                                    if sel { 14.0 } else { 8.0 },
                                    2.0,
                                    w::rgba(if sel { 0xffd060 } else { 0xff3860 }, 0.75),
                                )],
                            ),
                        ));
                        p.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(7.0 * k), Val::Px(2.0 * k)),
                                border: UiRect::all(Val::Px(1.0 * k)),
                                border_radius: w::pill(),
                                ..default()
                            },
                            BackgroundColor(w::rgba(0x080a12, if sel { 0.9 } else { 0.72 })),
                            BorderColor::all(if sel { w::gold() } else { w::white(0.12) }),
                        ))
                        .with_children(|p| {
                            p.spawn((
                                w::text(
                                    pin.name,
                                    T::new(if sel { 14.0 } else { 12.5 }).bold().c(if sel {
                                        w::gold()
                                    } else {
                                        w::fg()
                                    }),
                                    k,
                                ),
                                TextLayout::no_wrap(),
                            ));
                        });
                    });
                }
            });
    }
}

/// What `place` last placed the pins for: the pose, the view, readiness.
type Placed = Option<(Pose, (f64, f64), bool)>;

/// Each frame: every pin where its point is on the screen (hidden behind
/// the camera or off it), the picked one marked.
pub fn place(
    v: Res<Viewer>,
    status: Res<Status>,
    mut pins: Query<(&PinNode, &mut UiTransform, &mut Visibility, &mut VControl)>,
    added: Query<(), Added<PinNode>>,
    mut last: Local<Placed>,
) {
    let Some(a) = &v.atlas_mode else { return };
    let key = (v.pose, v.view, status.ready);
    if *last == Some(key) && added.is_empty() {
        return;
    }
    *last = Some(key);
    let bp = bp(&v);
    let k = bp.k;
    let (vw, vh) = v.view;
    for (pn, mut t, mut vis, mut ctl) in &mut pins {
        let Some(pin) = a.pins.get(pn.0) else {
            continue;
        };
        let sel = a.sel == Some(pn.0);
        if ctl.sel != sel {
            ctl.sel = sel;
        }
        let on = if status.ready {
            cams::project(&v.pose, vw, vh, pin.pos)
                .filter(|(x, y)| (-40.0..vw + 40.0).contains(x) && (-40.0..vh + 40.0).contains(y))
        } else {
            None
        };
        match on {
            Some((x, y)) => {
                let tr = Val2::px(x as f32 - DOT_AT * k, y as f32 - DOT_AT * k);
                if t.translation != tr {
                    t.translation = tr;
                }
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
            None => {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_start_of_the_open_world_is_a_pin() {
        let pins = pins();
        let ids: Vec<_> = pins.iter().map(|p| p.id).collect();
        // The peninsula's cruise starts, its first the crest.
        let pen = mp_levels::level_by_id("peninsula");
        assert_eq!(ids, pen.starts.iter().map(|s| s.id).collect::<Vec<_>>());
        // Both races start at the crest, so its pin carries both levels:
        // Midnight on the Coast (race, pursuit, cruise) and the Loop.
        let crest = &pins[0];
        let titles: Vec<_> = crest.groups.iter().map(|(t, _)| *t).collect();
        assert_eq!(titles, ["Midnight on the Coast", "The Loop"]);
        let modes = |t: &str| -> Vec<&str> {
            crest
                .groups
                .iter()
                .find(|(g, _)| *g == t)
                .map(|(_, r)| r.iter().map(|r| r.mode).collect())
                .unwrap_or_default()
        };
        assert_eq!(
            modes("Midnight on the Coast"),
            ["race", "pursuit", "cruise"]
        );
        assert_eq!(modes("The Loop"), ["race", "cruise"]);
        // Every other pin is a cruise of the peninsula from its start.
        for p in &pins[1..] {
            let runs: Vec<_> = p.runs().collect();
            assert_eq!(runs.len(), 1, "{}", p.id);
            assert_eq!(
                *runs[0],
                Run {
                    level: "peninsula",
                    mode: "cruise",
                    start: Some(p.id),
                }
            );
        }
        // No two pins on top of each other.
        for (i, a) in pins.iter().enumerate() {
            for b in &pins[i + 1..] {
                let d = a.pos - b.pos;
                assert!(d.x.hypot(d.z) >= MERGE, "{} and {}", a.id, b.id);
            }
        }
    }

    #[test]
    fn a_run_is_what_the_menu_would_start() {
        // The game reads the level, its mode and its start from the store
        // (`ui::plugin`); the address only names the level and races.
        for p in pins() {
            for r in p.runs() {
                let l = mp_levels::level_by_id(r.level);
                assert_eq!(l.id, r.level);
                assert!(mp_levels::ATLAS_LEVELS.contains(&r.level));
                match r.mode {
                    "pursuit" => assert!(l.police.is_some()),
                    "cruise" => assert!(l.open),
                    m => assert_eq!(m, "race"),
                }
                if let Some(s) = r.start {
                    assert!(l.starts.iter().any(|x| x.id == s));
                }
                assert_eq!(r.query(), format!("level={}&race=1", r.level));
            }
        }
    }

    #[test]
    fn the_map_keeps_its_tilt_and_reach() {
        let mut o = Orbit {
            centre: DVec3::ZERO,
            dist: 5000.0,
            yaw: 0.0,
            pitch: PITCH,
        };
        let d = cams::Drive {
            look: (0.0, -5.0),
            zoom: -20.0,
            ..Default::default()
        };
        step(&mut o, &d, 0.016, 800.0, 60_000.0);
        assert_eq!(o.pitch, PITCH_MIN);
        assert_eq!(o.dist, 60_000.0);
        // Dragging the map down brings what lies ahead into view (north at
        // yaw 0).
        let d = cams::Drive {
            pan: (0.0, 100.0),
            ..Default::default()
        };
        let z0 = o.centre.z;
        step(&mut o, &d, 0.016, 800.0, 60_000.0);
        assert!(o.centre.z < z0);
    }
}
