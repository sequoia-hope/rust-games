//! The atlas (DECISIONS D1251): a map of the whole level over the race,
//! for finding the way round the open world rather than only the next
//! turns the minimap shows. The race goes on under it: it is a glance.
//!
//! - **Open and close:** G, the left bumper (or D-pad up where the bumper
//!   is bound to something), a tap or click on the minimap, or its own ×.
//! - **Zoom:** Z (or the right stick's click, else D-pad down, or the ZOOM
//!   button) steps between the whole map and 3 km round the car, which
//!   follows it; the mouse wheel, + and − and the right stick zoom in
//!   between, a drag pans. With the atlas closed, Z steps the minimap
//!   between its own scale and a wide one ([`MiniZoom`]); MINI does it
//!   from the atlas.
//! - **What it shows:** the network (major roads bolder than city
//!   streets), the water and the built-up land, an event's route, its
//!   start and finish or a delivery's drop, the events' starts in a
//!   cruise, rivals and police, the player's arrow, the towns and the
//!   roads' stretches by name, and a scale bar.
//!
//! The roads are drawn from offset fields built once per level on the CPU
//! ([`field`]), a slice a frame, beginning once a race is running; the
//! shader (`atlas.wgsl`) strokes them in screen pixels, so the map stays
//! sharp at every zoom from one texture sample. The view and the markers
//! are uniforms written in place each frame the atlas is open
//! ([`material`]); closed, a 1 px node keeps its pipeline built, so the
//! first open does not compile a shader.

pub mod field;
mod material;
pub mod view;

use super::Play;
use super::flow::Mode;
use super::gamepad::{self, Binding};
use super::hud::model::HudIn;
use crate::ui::widgets::{self, Bp, T};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::image::ImageSampler;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::UiGlobalTransform;
use bevy::ui::widget::TextShadow;
use bevy::ui_render::prelude::MaterialNode;
use bevy::window::PrimaryWindow;
use field::{FieldBuilder, Grid, Label, LabelKind, Line};
use material::{AtlasMaterial, AtlasParams, AtlasWrites, Dot};
use mp_track::track::Track;
use view::{Panel, Preset, View};

pub fn plugin(app: &mut App) {
    material::plugin(app);
    app.init_resource::<MiniZoom>()
        .init_resource::<AtlasState>();
}

/// The minimap's scale: its own, or zoomed out ([`super::hud`] reads it).
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiniZoom {
    pub wide: bool,
}

/// What can be tapped: the minimap (opens the atlas) and the atlas's
/// buttons. Their boxes go into [`Play::atlas`] each frame, CSS px, so the
/// race's input (`read_input`) takes a tap on them before the touch
/// controls see it, as it does the radio's (D1160).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Open,
    Close,
    Zoom,
    Mini,
}

#[derive(Default, Debug)]
pub struct Taps {
    /// Each button's box, CSS px: (left, top, width, height).
    pub rects: Vec<(Btn, (f32, f32, f32, f32))>,
    pub queued: Vec<Btn>,
}

impl Taps {
    /// The button under (x, y), CSS px, a little grown for fingers.
    pub fn hit(&self, x: f32, y: f32) -> Option<Btn> {
        const SLOP: f32 = 4.0;
        self.rects
            .iter()
            .find(|(_, r)| {
                x >= r.0 - SLOP && x <= r.0 + r.2 + SLOP && y >= r.1 - SLOP && y <= r.1 + r.3 + SLOP
            })
            .map(|(b, _)| *b)
    }
}

// ── The level's map ────────────────────────────────────────────────────

/// Texels along the network field's longer side.
const NET_DIM: u32 = 1536;
/// The finest texel, metres (a small level).
const NET_FINEST: f64 = 2.0;
/// How far from a road a field keeps its offsets, in texels.
const REACH: f64 = 16.0;
/// Texels along the water's longer side.
const WATER_DIM: u32 = 768;
/// Texel visits a frame while the atlas is shut, and while it is open
/// and still drawing.
const BUDGET_SHUT: usize = 30_000;
const BUDGET_OPEN: usize = 250_000;

/// A field being built, or built.
enum Stage {
    Building(Box<FieldBuilder>),
    Done(Grid, Handle<Image>),
}

/// The water and land being sampled a few rows a frame, or sampled.
enum WaterStage {
    Building { grid: Grid, row: u32, data: Vec<u8> },
    Done(Grid, Handle<Image>),
    None,
}

/// A level's map: its network's lines, box, places and fields.
struct LevelMap {
    key: (&'static str, usize, usize),
    bounds: [f64; 4],
    labels: Vec<Label>,
    net: Stage,
    water: WaterStage,
    /// The route's (start, finish) and its field.
    route: Option<((u64, u64), Stage)>,
}

impl LevelMap {
    fn new(t: &Track) -> LevelMap {
        let lines = field::network_lines(t);
        let bounds = field::bounds(&lines).unwrap_or([-500.0, -500.0, 500.0, 500.0]);
        let g = Grid::over(bounds, 400.0, NET_DIM, NET_FINEST);
        let water = if t.level.cover.is_some() {
            WaterStage::Building {
                grid: Grid::over(bounds, 1200.0, WATER_DIM, NET_FINEST * 2.0),
                row: 0,
                data: Vec::new(),
            }
        } else {
            WaterStage::None
        };
        LevelMap {
            key: key_of(t),
            bounds,
            labels: field::labels(t),
            net: Stage::Building(Box::new(FieldBuilder::new(g, lines, 2, REACH * g.texel))),
            water,
            route: None,
        }
    }
}

fn key_of(t: &Track) -> (&'static str, usize, usize) {
    (t.level.id, t.n, t.branches.len())
}

fn field_image(g: &Grid, data: Vec<u8>, format: TextureFormat) -> Image {
    let mut img = Image::new(
        Extent3d {
            width: g.w,
            height: g.h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::linear();
    img
}

/// A 1 × 1 field of nothing, standing in until one is built.
fn empty_image(format: TextureFormat) -> Image {
    let far = field::f16_bits(field::FAR).to_le_bytes();
    let data = match format {
        TextureFormat::Rgba16Float => [far, [0, 0], far, [0, 0]].concat(),
        TextureFormat::Rg16Float => [far, [0, 0]].concat(),
        _ => vec![0, 0],
    };
    let g = Grid {
        x0: 0.0,
        z0: 0.0,
        texel: 1.0,
        w: 1,
        h: 1,
    };
    field_image(&g, data, format)
}

/// Works a stage on by `budget` texel visits; the image when it is done.
fn work(
    stage: &mut Stage,
    budget: usize,
    images: &mut Assets<Image>,
    format: TextureFormat,
) -> bool {
    let Stage::Building(b) = stage else {
        return false;
    };
    if !b.step(budget) {
        return false;
    }
    let g = b.grid;
    let Stage::Building(b) = std::mem::replace(stage, Stage::Done(g, Handle::default())) else {
        unreachable!()
    };
    let h = images.add(field_image(&g, b.finish(), format));
    *stage = Stage::Done(g, h);
    true
}

/// The water (sea, bay, lakes) and the built-up land under a grid, a few
/// rows a frame: R wet, G urban.
fn work_water(w: &mut WaterStage, t: &Track, rows: u32, images: &mut Assets<Image>) -> bool {
    let WaterStage::Building { grid, row, data } = w else {
        return false;
    };
    let Some(cover) = t.level.cover.as_ref() else {
        *w = WaterStage::None;
        return false;
    };
    let ground = t.level.ground.as_ref();
    let sea_y = t.level.sea_y.unwrap_or(0.0);
    let g = *grid;
    let end = (*row + rows).min(g.h);
    for j in *row..end {
        for i in 0..g.w {
            let (x, z) = g.at(i, j);
            let c = mp_track::Cover::of(cover(x, z));
            // The cover's sea class also fills cells it has no data for
            // (patches in the towns): sea only where the ground is at the
            // sea's level too; lakes and the bay's water wherever they are.
            let sea =
                c == mp_track::Cover::Sea && ground.as_ref().is_none_or(|g| g(x, z) < sea_y + 1.0);
            let wet = sea || c == mp_track::Cover::Water;
            data.push(if wet { 255 } else { 0 });
            data.push(if c == mp_track::Cover::Urban { 255 } else { 0 });
        }
    }
    *row = end;
    if end < g.h {
        return false;
    }
    let data = std::mem::take(data);
    let h = images.add(field_image(&g, data, TextureFormat::Rg8Unorm));
    *w = WaterStage::Done(g, h);
    true
}

// ── State ──────────────────────────────────────────────────────────────

/// The atlas: open or shut, where it looks, the level's map, its nodes.
#[derive(Resource, Default)]
pub struct AtlasState {
    pub open: bool,
    /// The preset the last Z stepped to.
    preset: Option<Preset>,
    view: Option<View>,
    map: Option<LevelMap>,
    mat: Option<Handle<AtlasMaterial>>,
    ui: Option<Ui>,
    /// The cursor's last place while dragging the map, CSS px.
    drag: Option<Vec2>,
    /// The pad's toggle and zoom buttons last frame.
    pad_prev: (bool, bool),
    /// `?map=` has been applied.
    asked: bool,
    last: Option<AtlasParams>,
    last_labels: Option<(View, Panel, (f64, f64))>,
    /// The nodes were built this frame (and are not there to move yet).
    fresh: bool,
}

/// What the nodes were built for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    bp: Bp,
    open: bool,
    rect: (f32, f32, f32, f32),
    wide: bool,
    mph: bool,
    level: (&'static str, usize, usize),
}

struct Ui {
    key: Key,
    root: Entity,
    labels: Vec<Entity>,
    sub: Option<Entity>,
    scale_bar: Option<Entity>,
    scale_text: Option<Entity>,
}

#[derive(Component)]
struct AtlasRoot;

/// A label node (the level's labels, in order).
#[derive(Component, Clone, Copy)]
struct LabelNode;

/// The panel's box, CSS px: (left, top, width, height). On a touch screen
/// it fills the screen inside the insets, under the touch controls (which
/// stay on top and keep working); on a desktop it leaves a margin round it.
pub fn panel_rect(bp: &Bp, in_l: f32, in_r: f32) -> (f32, f32, f32, f32) {
    if bp.touch {
        let top = (bp.inset_top + 6.0).max(8.0);
        (in_l, top, bp.w - in_l - in_r, bp.h - top - 8.0)
    } else {
        let mx = (bp.w * 0.04).max(20.0);
        let my = (bp.h * 0.05).max(18.0);
        (mx, my, bp.w - 2.0 * mx, bp.h - 2.0 * my)
    }
}

/// The level's places as markers in a cruise: each event's start.
fn event_kind_rgb(kind: mp_track::EventKind, a: f32) -> [f32; 4] {
    super::hud::minimap_rgb(super::beacons::srgb(kind), a)
}

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const FINISH: u32 = 0x4dff8a;
const COP_RED: u32 = 0xff3040;
const COP_BLUE: u32 = 0x2f6bff;

/// The markers, panel CSS px, in drawing order (the player's arrow is the
/// shader's own).
fn markers(race: &super::flow::Race, hin: &HudIn, v: &View, p: Panel, clock: f64) -> Vec<Dot> {
    let st = &race.session.curr;
    let lr = &race.session.lr;
    let t = &lr.track;
    let rgb = super::hud::minimap_rgb;
    let at = |x: f64, z: f64| v.to_screen(p, x, z);
    let mut out = Vec::new();
    let running = mp_sim::event::running(lr, st);
    // Traffic, zoomed in.
    if v.mpp < 4.0 {
        for o in &hin.traffic {
            let (x, y) = at(o.x, o.z);
            out.push(Dot {
                x,
                y,
                r: 2.2,
                fill: [180.0 / 255.0, 190.0 / 255.0, 210.0 / 255.0, 0.7],
                stroke: None,
            });
        }
    }
    if st.race.cruise && running.is_none() {
        // The events' starts.
        for e in &lr.level.events {
            let (sx, sz) = mp_sim::event::start_point(t, e);
            let (x, y) = at(sx, sz);
            out.push(Dot {
                x,
                y,
                r: 5.5,
                fill: event_kind_rgb(e.kind, 1.0),
                stroke: Some((2.0, BLACK)),
            });
        }
    } else if let Some(e) = running {
        let (sx, sz) = mp_sim::event::start_point(t, e);
        let (x, y) = at(sx, sz);
        out.push(Dot {
            x,
            y,
            r: 4.5,
            fill: event_kind_rgb(e.kind, 1.0),
            stroke: Some((1.5, BLACK)),
        });
        if let Some([dx, dz]) = e.drop {
            let (x, y) = at(dx, dz);
            // A delivery's drop pulses.
            let pulse = 0.5 + 0.5 * (clock * 4.0).sin();
            out.push(Dot {
                x,
                y,
                r: 9.0 + 3.0 * pulse,
                fill: event_kind_rgb(e.kind, 0.25),
                stroke: None,
            });
            out.push(Dot {
                x,
                y,
                r: 6.5,
                fill: event_kind_rgb(e.kind, 1.0),
                stroke: Some((2.0, WHITE)),
            });
        } else {
            let f = t.frame(e.finish);
            let (x, y) = at(f.x, f.z);
            out.push(Dot {
                x,
                y,
                r: 6.5,
                fill: rgb(FINISH, 1.0),
                stroke: Some((2.0, BLACK)),
            });
        }
    } else if !t.is_loop {
        let f = t.frame(t.finish_s);
        let (x, y) = at(f.x, f.z);
        out.push(Dot {
            x,
            y,
            r: 6.5,
            fill: rgb(FINISH, 1.0),
            stroke: Some((2.0, BLACK)),
        });
    }
    for r in &hin.rivals {
        let (x, y) = at(r.x, r.z);
        out.push(Dot {
            x,
            y,
            r: 4.5,
            fill: rgb(r.color, 1.0),
            stroke: Some((1.5, BLACK)),
        });
    }
    if let Some(pu) = &hin.pursuit {
        let phase = if pu.flash {
            ((clock * 4.0).floor() as i64).rem_euclid(2) as usize
        } else {
            0
        };
        for (i, u) in pu.units.iter().enumerate() {
            let (x, y) = at(u.x, u.z);
            if u.disabled {
                out.push(Dot {
                    x,
                    y,
                    r: 4.0,
                    fill: [120.0 / 255.0, 126.0 / 255.0, 140.0 / 255.0, 0.6],
                    stroke: None,
                });
                continue;
            }
            let red = (i + phase) % 2 == 0;
            out.push(Dot {
                x,
                y,
                r: 4.5,
                fill: rgb(if red { COP_RED } else { COP_BLUE }, 1.0),
                stroke: Some((1.5, BLACK)),
            });
        }
    }
    // Off the panel: not drawn (and not counted against the shader's cap).
    out.retain(|d| d.x > -20.0 && d.y > -20.0 && d.x < p.w + 20.0 && d.y < p.h + 20.0);
    out
}

/// The route an event (or a lap of the loop) follows on the level's road:
/// its start and finish, metres along road 0.
fn route_of(race: &super::flow::Race) -> Option<(f64, f64)> {
    let lr = &race.session.lr;
    let st = &race.session.curr;
    let t = &lr.track;
    if let Some(e) = mp_sim::event::running(lr, st) {
        return e.drop.is_none().then_some((e.s, e.finish));
    }
    // A race round an open level's loop: the loop.
    (!st.race.cruise && t.is_loop && !t.branches.is_empty()).then(|| (0.0, t.n as f64 - 1.0))
}

/// The route's colour: the event's kind's, else the accent.
fn route_rgb(race: &super::flow::Race) -> Vec4 {
    let lr = &race.session.lr;
    match mp_sim::event::running(lr, &race.session.curr) {
        Some(e) => Vec4::from_array(event_kind_rgb(e.kind, 1.0)),
        None => Vec4::from_array(super::hud::minimap_rgb(0xff3860, 1.0)),
    }
}

// ── The frame ──────────────────────────────────────────────────────────

/// The keys, the mouse and the pads.
#[derive(SystemParam)]
pub(super) struct Controls<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    wheel: MessageReader<'w, 's, MouseWheel>,
    pads: Option<Res<'w, super::gamepad_io::PadsRes>>,
    time: Res<'w, Time<bevy::time::Real>>,
}

/// The nodes the frame moves and the boxes it reads.
#[derive(SystemParam)]
pub(super) struct Nodes<'w, 's> {
    labels: Query<'w, 's, (&'static mut Node, &'static mut Visibility), With<LabelNode>>,
    texts: Query<'w, 's, &'static mut Text>,
    bars: Query<'w, 's, &'static mut Node, Without<LabelNode>>,
    buttons: Query<
        'w,
        's,
        (
            &'static Btn,
            &'static ComputedNode,
            &'static UiGlobalTransform,
        ),
    >,
    minimaps: Query<
        'w,
        's,
        (
            &'static super::hud::El,
            &'static ComputedNode,
            &'static UiGlobalTransform,
        ),
    >,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    mut commands: Commands,
    mut play: ResMut<Play>,
    mut st: ResMut<AtlasState>,
    mut mini: ResMut<MiniZoom>,
    opts: Res<crate::Opts>,
    ui_state: Option<Res<crate::ui::UiState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ctl: Controls,
    mut images: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<AtlasMaterial>>,
    mut writes: ResMut<AtlasWrites>,
    q: Nodes,
) {
    let Controls {
        keys,
        mouse,
        mut wheel,
        pads,
        time,
    } = ctl;
    let Nodes {
        labels: mut nodes,
        mut texts,
        mut bars,
        buttons,
        minimaps,
    } = q;
    let st = &mut *st;
    let play = &mut *play;
    let driving = play
        .race
        .as_ref()
        .is_some_and(|r| r.mode == Mode::Race && !play.hold);
    let (Some(race), true, Ok(w)) = (play.race.as_ref(), driving, windows.single()) else {
        if let Some(u) = st.ui.take() {
            commands.entity(u.root).despawn();
        }
        play.atlas.rects.clear();
        play.atlas.queued.clear();
        st.drag = None;
        wheel.clear();
        return;
    };
    if opts.o.param("hud") == Some("0") {
        return;
    }
    let t = race.session.lr.track.clone();
    let css = play.css_scale.max(0.01);
    let bp = Bp::new(
        w.width() * css,
        w.height() * css,
        1.0 / css,
        play.touch_ui,
        play.insets.top as f32,
    );
    let in_l = (play.insets.left as f32).max(16.0);
    let in_r = (play.insets.right as f32).max(16.0);
    let rect = panel_rect(&bp, in_l, in_r);
    let panel = Panel {
        w: f64::from(rect.2),
        h: f64::from(rect.3),
    };
    let car = {
        let p = &race.session.curr.players[race.me()].v;
        (p.x, p.z, p.yaw)
    };

    // The level's map, built a slice at a time from the race's start.
    if st.map.as_ref().map(|m| m.key) != Some(key_of(&t)) {
        st.map = Some(LevelMap::new(&t));
        st.view = None;
        st.mat = None;
    }
    let map = st.map.as_mut().expect("map");
    let budget = if st.open { BUDGET_OPEN } else { BUDGET_SHUT };
    let mut changed = work(
        &mut map.net,
        budget,
        &mut images,
        TextureFormat::Rgba16Float,
    );
    changed |= work_water(
        &mut map.water,
        &t,
        if st.open { 64 } else { 8 },
        &mut images,
    );
    // The route's field, when there is one and the atlas is open.
    let want = route_of(race).map(|(a, b)| (a.to_bits(), b.to_bits()));
    if st.open && map.route.as_ref().map(|r| r.0) != want {
        map.route = want.map(|k| {
            let (s0, s1) = (f64::from_bits(k.0), f64::from_bits(k.1));
            let line: Line = field::route_line(&t, s0, s1);
            let b = field::bounds(std::slice::from_ref(&line)).unwrap_or(map.bounds);
            let tex = match &map.net {
                Stage::Building(fb) => fb.grid.texel,
                Stage::Done(g, _) => g.texel,
            };
            let g = Grid::over(b, 400.0, NET_DIM, tex);
            (
                k,
                Stage::Building(Box::new(FieldBuilder::new(
                    g,
                    vec![line],
                    1,
                    REACH * g.texel,
                ))),
            )
        });
        changed = true;
    }
    if let Some((_, r)) = map.route.as_mut() {
        changed |= work(r, budget, &mut images, TextureFormat::Rg16Float);
    }
    let done = |s: &Stage| match s {
        Stage::Done(g, h) => Some((*g, h.clone())),
        Stage::Building(_) => None,
    };
    let net = done(&map.net);
    let route = map.route.as_ref().and_then(|(_, s)| done(s));
    let water = match &map.water {
        WaterStage::Done(g, h) => Some((*g, h.clone())),
        _ => None,
    };
    let bounds = map.bounds;
    // The material, its textures swapped in as they are built.
    let mat_textures = |images: &mut Assets<Image>| AtlasMaterial {
        p: AtlasParams::default(),
        net: net
            .as_ref()
            .map(|n| n.1.clone())
            .unwrap_or_else(|| images.add(empty_image(TextureFormat::Rgba16Float))),
        route: route
            .as_ref()
            .map(|n| n.1.clone())
            .unwrap_or_else(|| images.add(empty_image(TextureFormat::Rg16Float))),
        water: water
            .as_ref()
            .map(|n| n.1.clone())
            .unwrap_or_else(|| images.add(empty_image(TextureFormat::Rg8Unorm))),
    };
    if st.mat.is_none() {
        let m = mat_textures(&mut images);
        st.mat = Some(mats.add(m));
    } else if changed && let Some(mut m) = st.mat.as_ref().and_then(|h| mats.get_mut(h)) {
        let n = mat_textures(&mut images);
        m.net = n.net;
        m.route = n.route;
        m.water = n.water;
    }
    let mat = st.mat.clone().expect("material");

    // The controls.
    let mut toggle = keys.just_pressed(KeyCode::KeyG);
    let mut zoom_step = keys.just_pressed(KeyCode::KeyZ);
    let mut mini_step = false;
    let mut close = false;
    for b in std::mem::take(&mut play.atlas.queued) {
        match b {
            Btn::Open => toggle = true,
            Btn::Close => close = true,
            Btn::Zoom => zoom_step = true,
            Btn::Mini => mini_step = true,
        }
    }
    let mut stick = 0.0;
    if let Some(pads) = pads.as_ref()
        && let Some(p) = pads.pads.active.as_ref()
    {
        let map = pads.pads.map_for(p);
        let (tb, zb) = pad_buttons(map);
        let held = |i: usize| p.buttons.get(i).is_some_and(|b| b.pressed);
        let now = (tb.is_some_and(held), zb.is_some_and(held));
        toggle |= now.0 && !st.pad_prev.0;
        zoom_step |= now.1 && !st.pad_prev.1;
        st.pad_prev = now;
        if !bound_axis(map, 3) {
            let a = p.axes.get(3).copied().unwrap_or(0.0);
            if a.abs() > 0.2 {
                stick = a;
            }
        }
    }
    // `?map=whole|region`: open at the start (pictures, tests).
    if !st.asked {
        st.asked = true;
        if let Some(m) = opts.o.param("map") {
            st.open = true;
            st.preset = Some(if m == "region" {
                Preset::Region
            } else {
                Preset::Whole
            });
        }
        if opts.o.param("minimap") == Some("wide") {
            mini.wide = true;
        }
    }
    if toggle {
        st.open = !st.open;
    }
    if close {
        st.open = false;
    }
    if zoom_step && !st.open {
        mini_step = true;
    }
    if mini_step {
        mini.wide = !mini.wide;
    }
    if !st.open {
        st.drag = None;
        wheel.clear();
    }

    // The view.
    let car2 = (car.0, car.1);
    let mut v = match st.view {
        Some(v) if st.open => v,
        _ => View::preset(st.preset.unwrap_or(Preset::Whole), bounds, panel, car2),
    };
    if st.open {
        if zoom_step {
            let next = match v.nearest(bounds, panel) {
                Preset::Whole => Preset::Region,
                Preset::Region => Preset::Whole,
            };
            st.preset = Some(next);
            v = View::preset(next, bounds, panel, car2);
        }
        let centre = (panel.w / 2.0, panel.h / 2.0);
        let dt = time.delta_secs_f64();
        if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
            v.zoom((2.0 * dt).exp(), centre, bounds, panel);
        }
        if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
            v.zoom((-2.0 * dt).exp(), centre, bounds, panel);
        }
        if stick != 0.0 {
            v.zoom((-2.5 * stick * dt).exp(), centre, bounds, panel);
        }
        // The mouse: the wheel zooms about the cursor, a drag pans.
        let cursor = w
            .cursor_position()
            .map(|c| c * css - Vec2::new(rect.0, rect.1));
        let inside = cursor.filter(|c| c.x >= 0.0 && c.y >= 0.0 && c.x <= rect.2 && c.y <= rect.3);
        for e in wheel.read() {
            let Some(c) = inside else { continue };
            let steps = match e.unit {
                MouseScrollUnit::Line => f64::from(e.y),
                MouseScrollUnit::Pixel => f64::from(e.y) / 60.0,
            };
            v.zoom(
                1.2f64.powf(steps),
                (f64::from(c.x), f64::from(c.y)),
                bounds,
                panel,
            );
        }
        if !play.touch_ui {
            let on_button = |c: Vec2| play.atlas.hit(c.x + rect.0, c.y + rect.1).is_some();
            if mouse.just_pressed(MouseButton::Left)
                && let Some(c) = inside
                && !on_button(c)
            {
                st.drag = Some(c);
            }
            if !mouse.pressed(MouseButton::Left) {
                st.drag = None;
            }
            if let (Some(d), Some(c)) = (st.drag, cursor) {
                let m = c - d;
                if m != Vec2::ZERO {
                    v.pan(f64::from(m.x), f64::from(m.y), bounds);
                    st.drag = Some(c);
                }
            }
        }
        v.track(car2);
        st.view = Some(v);
    } else {
        st.view = None;
    }

    // The nodes.
    let mph = ui_state.as_ref().is_none_or(|u| u.settings.mph);
    let key = Key {
        bp,
        open: st.open,
        rect,
        wide: mini.wide,
        mph,
        level: key_of(&t),
    };
    if st.ui.as_ref().map(|u| u.key) != Some(key) {
        if let Some(u) = st.ui.take() {
            commands.entity(u.root).despawn();
        }
        let labels = st.map.as_ref().map_or(&[][..], |m| &m.labels[..]);
        st.ui = Some(build(&mut commands, &key, &mat, labels));
        st.last = None;
        st.last_labels = None;
        st.fresh = true;
    } else {
        // The nodes spawned last frame exist now.
        st.fresh = false;
    }

    // The boxes to tap: the minimap always, the buttons when open.
    play.atlas.rects.clear();
    let boxed = |n: &ComputedNode, g: &UiGlobalTransform| {
        let k = n.inverse_scale_factor * css;
        let c = g.affine().translation * k;
        let s = n.size * k;
        (c.x - s.x / 2.0, c.y - s.y / 2.0, s.x, s.y)
    };
    if st.open {
        for (b, n, g) in &buttons {
            if n.size.x > 0.0 {
                play.atlas.rects.push((*b, boxed(n, g)));
            }
        }
    } else {
        for (e, n, g) in &minimaps {
            if *e == super::hud::El::Minimap && n.size.x > 0.0 {
                play.atlas.rects.push((Btn::Open, boxed(n, g)));
            }
        }
    }

    // The uniforms.
    let mut p = AtlasParams::default();
    if let (true, Some(v)) = (st.open, st.view) {
        let hin = super::hud::hud_in(race);
        // Opaque: the UI is drawn into the scene's HDR target before its tone
        // mapping, so any of a bright scene left showing through glows.
        p.head = Vec4::new(1.0, 0.0, 1.0, 0.0);
        p.view = Vec4::new(v.cx as f32, v.cz as f32, v.mpp as f32, 0.0);
        p.size = Vec4::new(panel.w as f32, panel.h as f32, 0.0, 1.0);
        if let Some((g, _)) = &net {
            p.head.w = 1.0;
            p.net = Vec4::new(
                g.x0 as f32,
                g.z0 as f32,
                g.texel as f32,
                (REACH * g.texel) as f32,
            );
            p.net_dim = Vec4::new(g.w as f32, g.h as f32, 0.0, 0.0);
        }
        if let Some((g, _)) = &route {
            p.view.w = 1.0;
            p.route = Vec4::new(
                g.x0 as f32,
                g.z0 as f32,
                g.texel as f32,
                (REACH * g.texel) as f32,
            );
            p.route_dim = Vec4::new(g.w as f32, g.h as f32, 0.0, 0.0);
            p.route_color = route_rgb(race);
        }
        if let Some((g, _)) = &water {
            p.size.z = 1.0;
            p.water = Vec4::new(g.x0 as f32, g.z0 as f32, g.texel as f32, 0.0);
            p.water_dim = Vec4::new(g.w as f32, g.h as f32, 0.0, 0.0);
        }
        let (sx, sy) = v.to_screen(panel, car.0, car.1);
        p.player = Vec4::new(sx as f32, sy as f32, car.2.cos() as f32, car.2.sin() as f32);
        let clock = time.elapsed_secs_f64();
        let dots = markers(race, &hin, &v, panel, clock);
        material::set_shapes(&mut p, &dots);

        // The labels, the scale bar and the zoom's line, when the view moved.
        // (The player's place to 8 px, so a still view does not move the
        // labels every frame the car does.)
        let near = ((sx / 8.0).round(), (sy / 8.0).round());
        if st.last_labels != Some((v, panel, near))
            && !st.fresh
            && let (Some(u), Some(m)) = (st.ui.as_ref(), st.map.as_ref())
        {
            st.last_labels = Some((v, panel, near));
            place_labels(u, m, &v, panel, (sx, sy), &mut nodes);
            let (metres, px) = view::scale_bar(v.mpp, 90.0);
            if let Some(mut n) = u.scale_bar.and_then(|e| bars.get_mut(e).ok()) {
                n.width = bp.px(px as f32);
            }
            if let Some(mut tx) = u.scale_text.and_then(|e| texts.get_mut(e).ok()) {
                let s = view::scale_text(metres, mph);
                if tx.0 != s {
                    tx.0 = s;
                }
            }
            if let Some(mut tx) = u.sub.and_then(|e| texts.get_mut(e).ok()) {
                let across = v.mpp * panel.w.min(panel.h);
                let s = if v.nearest(bounds, panel) == Preset::Whole && !v.follow {
                    "WHOLE MAP".to_owned()
                } else {
                    format!(
                        "{} ACROSS{}",
                        view::scale_text(across, mph).to_uppercase(),
                        if v.follow { " · FOLLOWING" } else { "" }
                    )
                };
                if tx.0 != s {
                    tx.0 = s;
                }
            }
        }
    }
    if st.last != Some(p) {
        st.last = Some(p);
        writes.0 = Some(p);
    }
}

/// The pad's toggle and zoom buttons: the left bumper and the right
/// stick's click, or D-pad up and down where those are bound to an action
/// in the pad's map (a remap may have put anything anywhere).
fn pad_buttons(map: &gamepad::PadMap) -> (Option<usize>, Option<usize>) {
    let free = |i: usize| {
        !map.iter()
            .any(|(_, list)| list.contains(&Binding::Button(i)))
    };
    let pick = |a: usize, b: usize| {
        if free(a) {
            Some(a)
        } else if free(b) {
            Some(b)
        } else {
            None
        }
    };
    (pick(4, 12), pick(11, 13))
}

/// Whether an action is bound to the axis.
fn bound_axis(map: &gamepad::PadMap, axis: usize) -> bool {
    map.iter().any(|(_, list)| {
        list.iter()
            .any(|b| matches!(b, Binding::Axis { axis: a, .. } if *a == axis))
    })
}

/// A label's size, CSS px.
fn label_style(kind: LabelKind) -> T {
    match kind {
        LabelKind::Town => T::new(13.0).bold().ls(0.12),
        LabelKind::Road => T::new(10.5).w(600).ls(0.08).c(widgets::rgba(0xc8d2ec, 0.8)),
        LabelKind::Zone => T::new(10.5).w(600).ls(0.1).c(widgets::rgba(0xffcf4d, 0.75)),
    }
}

/// Its box's size, CSS px, from its text (Rajdhani's capitals are about
/// 0.56 em wide).
fn label_size(l: &Label) -> (f64, f64) {
    let t = label_style(l.kind);
    let n = l.text.chars().count() as f64;
    let size = f64::from(t.size);
    (
        n * size * (0.56 + f64::from(t.spacing)),
        size * f64::from(widgets::LINE_NORMAL),
    )
}

fn place_labels(
    u: &Ui,
    m: &LevelMap,
    v: &View,
    p: Panel,
    player: (f64, f64),
    nodes: &mut Query<(&mut Node, &mut Visibility), With<LabelNode>>,
) {
    let at: Vec<(f64, f64, f64, f64)> = m
        .labels
        .iter()
        .map(|l| {
            let (x, y) = v.to_screen(p, l.x, l.z);
            let (w, h) = label_size(l);
            (x, y, w, h)
        })
        .collect();
    let (px, py) = player;
    let placed = view::place(&at, &[(px - 16.0, py - 16.0, 32.0, 32.0)], p, 10.0);
    let k = u.key.bp.k;
    for (i, e) in u.labels.iter().enumerate() {
        let Ok((mut n, mut vis)) = nodes.get_mut(*e) else {
            continue;
        };
        match placed.iter().find(|q| q.index == i) {
            Some(q) => {
                n.left = Val::Px((q.x - q.w / 2.0) as f32 * k);
                n.top = Val::Px(q.y as f32 * k);
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

fn build(commands: &mut Commands, key: &Key, mat: &Handle<AtlasMaterial>, labels: &[Label]) -> Ui {
    let bp = &key.bp;
    let px = |v: f32| bp.px(v);
    if !key.open {
        // Shut: a 1 px node, so the pipeline is built before it is wanted.
        let root = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(1.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                MaterialNode(mat.clone()),
                GlobalZIndex(1),
                AtlasRoot,
            ))
            .id();
        return Ui {
            key: *key,
            root,
            labels: Vec::new(),
            sub: None,
            scale_bar: None,
            scale_text: None,
        };
    }
    let (l, t, w, h) = key.rect;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(l),
                top: px(t),
                width: px(w),
                height: px(h),
                border_radius: BorderRadius::all(px(16.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            MaterialNode(mat.clone()),
            GlobalZIndex(1),
            AtlasRoot,
        ))
        .id();
    let shadow = TextShadow {
        offset: Vec2::new(0.0, 1.0) * bp.k,
        color: Color::srgba(0.0, 0.0, 0.0, 0.85),
    };
    // The places.
    let mut label_nodes = Vec::new();
    for lb in labels {
        let ts = label_style(lb.kind);
        let e = commands
            .spawn((
                Text::new(lb.text.clone()),
                ts.font(bp.k),
                TextColor(ts.color),
                ts.spacing(bp.k),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
                shadow,
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                Visibility::Hidden,
                LabelNode,
                ChildOf(root),
            ))
            .id();
        label_nodes.push(e);
    }
    // The header: MAP and the zoom's line.
    let head = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(16.0),
                top: px(12.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let tt = T::new(if bp.touch { 16.0 } else { 20.0 }).bold().ls(0.22);
    commands.spawn((
        Text::new("MAP"),
        tt.font(bp.k),
        TextColor(widgets::fg()),
        tt.spacing(bp.k),
        shadow,
        ChildOf(head),
    ));
    let ts = T::new(if bp.touch { 10.5 } else { 12.0 }).w(600).ls(0.14);
    let sub = commands
        .spawn((
            Text::new("WHOLE MAP"),
            ts.font(bp.k),
            TextColor(widgets::dim()),
            ts.spacing(bp.k),
            shadow,
            ChildOf(head),
        ))
        .id();
    // The scale bar under it.
    let tsc = T::new(11.0).w(600).ls(0.08);
    let scale_text = commands
        .spawn((
            Text::new(""),
            tsc.font(bp.k),
            TextColor(widgets::fg()),
            tsc.spacing(bp.k),
            shadow,
            Node {
                margin: UiRect::top(px(6.0)),
                ..default()
            },
            ChildOf(head),
        ))
        .id();
    let scale_bar = commands
        .spawn((
            Node {
                width: px(90.0),
                height: px(4.0),
                margin: UiRect::top(px(2.0)),
                border: UiRect::all(px(1.0)),
                ..default()
            },
            BackgroundColor(widgets::white(0.85)),
            BorderColor::all(Color::BLACK),
            ChildOf(head),
        ))
        .id();
    // The buttons, top right (the radio's pills).
    let z = if bp.touch { 34.0 } else { 36.0 };
    let line = widgets::white(0.22);
    let row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12.0),
                top: px(10.0),
                column_gap: px(6.0),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let btn = |commands: &mut Commands, b: Btn, label: &str, wide: bool| {
        let id = commands
            .spawn((
                Node {
                    height: px(z),
                    min_width: px(z),
                    padding: UiRect::horizontal(px(if wide { 12.0 } else { 0.0 })),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(z / 2.0)),
                    ..default()
                },
                BackgroundColor(widgets::rgba(0x0a0e18, 0.62)),
                BorderColor::all(line),
                b,
                ChildOf(row),
            ))
            .id();
        let t = T::new(if bp.touch { 12.0 } else { 13.0 }).bold().ls(0.08);
        commands.spawn((
            Text::new(label),
            t.font(bp.k),
            TextColor(widgets::fg()),
            t.spacing(bp.k),
            ChildOf(id),
        ));
    };
    btn(commands, Btn::Zoom, "ZOOM", true);
    btn(
        commands,
        Btn::Mini,
        if key.wide {
            "MINIMAP: WIDE"
        } else {
            "MINIMAP: NEAR"
        },
        true,
    );
    btn(commands, Btn::Close, "X", false);
    // The keys, bottom right, where there is a keyboard.
    if !bp.touch {
        let tk = T::new(11.5).w(600).ls(0.1);
        commands.spawn((
            Text::new("G  CLOSE    Z  ZOOM    WHEEL / DRAG  MOVE"),
            tk.font(bp.k),
            TextColor(widgets::dim()),
            tk.spacing(bp.k),
            shadow,
            Node {
                position_type: PositionType::Absolute,
                right: px(16.0),
                bottom: px(12.0),
                ..default()
            },
            ChildOf(root),
        ));
    }
    Ui {
        key: *key,
        root,
        labels: label_nodes,
        sub: Some(sub),
        scale_bar: Some(scale_bar),
        scale_text: Some(scale_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::gamepad::default_map;

    #[test]
    fn the_panel_leaves_a_margin_on_a_desktop_and_fills_a_phone() {
        let bp = Bp::new(1280.0, 720.0, 1.0, false, 0.0);
        let (l, t, w, h) = panel_rect(&bp, 16.0, 16.0);
        assert!(l >= 20.0 && t >= 18.0 && (l + w - (1280.0 - l)).abs() < 1e-3 && h > 600.0);
        let bp = Bp::new(844.0, 390.0, 1.0, true, 0.0);
        let (l, t, w, h) = panel_rect(&bp, 47.0, 47.0);
        assert_eq!((l, t), (47.0, 8.0));
        assert_eq!(w, 844.0 - 94.0);
        assert_eq!(h, 390.0 - 16.0);
    }

    #[test]
    fn the_pad_uses_free_buttons() {
        let mut map = default_map();
        // The standard map leaves the left bumper and the stick clicks.
        assert_eq!(pad_buttons(&map), (Some(4), Some(11)));
        assert!(!bound_axis(&map, 3) && bound_axis(&map, 0));
        // A remap that puts the nitro on the bumper moves the map to the
        // D-pad.
        map[4].1 = vec![Binding::Button(4)];
        assert_eq!(pad_buttons(&map).0, Some(12));
    }

    #[test]
    fn a_tap_finds_its_button() {
        let t = Taps {
            rects: vec![
                (Btn::Open, (700.0, 20.0, 104.0, 104.0)),
                (Btn::Close, (10.0, 10.0, 30.0, 30.0)),
            ],
            queued: Vec::new(),
        };
        assert_eq!(t.hit(750.0, 60.0), Some(Btn::Open));
        assert_eq!(t.hit(12.0, 42.0), Some(Btn::Close));
        assert_eq!(t.hit(400.0, 200.0), None);
    }
}
