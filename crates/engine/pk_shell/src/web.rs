//! Web glue (Midnight Playground SPEC 8.4 and 8.5; its `web.rs`,
//! `ui/web.rs` and `play/web.rs`): the page (`template/web/index.html`)
//! picks the backend, loads the wasm, calls the game's `run`, and then
//! talks to the client through the functions here, which the game's crate
//! exports to it with `#[wasm_bindgen]` wrappers (`gesture`, `gesture_at`,
//! `screenshot`, `stage`, `backend`).
//!
//! - **Gestures.** The page calls [`gesture`] inside its pointer-down,
//!   pointer-up, touch-end, click and key-down handlers, which is where
//!   audio resume, fullscreen, the landscape lock and the motion permission
//!   must happen (they are refused a frame later). Every function given to
//!   [`on_gesture`] runs there. [`gesture_at`] carries where the pointer
//!   is: a tap on a control marked [`FullscreenOnTap`] goes fullscreen and
//!   asks for landscape (once per tap: pointer-up, touch-end and click all
//!   come, 800 ms apart at most, D843); a tap on one marked
//!   [`PermissionOnTap`] (or on a fullscreen control) runs the permission
//!   hook, for an iPhone's motion access, before the fullscreen request,
//!   which uses the tap up (a browser with both would refuse the other).
//! - **The pixel ratio** (D437, D575, D1194, D1195, D1199).
//!   [`set_pixel_ratio`] draws the canvas at `pr` pixels per CSS pixel: the
//!   window's scale factor override, so Bevy's logical pixels are CSS
//!   pixels, and its size in pixels, the canvas's CSS box times `pr`,
//!   which the surface (and so the canvas's backing store) is configured
//!   from. The override alone changes only the logical size: winit sizes
//!   the window from the canvas's box and the device's ratio whatever
//!   Bevy's factor. The page's `#game` rule is `width: 100% !important`,
//!   so the smaller backing store is scaled into the page's box.
//!   [`RenderScale`] is the policy: the scene draws at `hq ? min(dpr,
//!   hq_cap) : 1` times the render scale; a screen (a menu over the scene)
//!   at `max(that, min(dpr, screen_min))`, because its text is drawn into
//!   the same canvas, where a DOM's was always sharp. While the screen's
//!   ratio is the higher, the scene's camera draws into an image at the
//!   scene's ratio and a second camera shows it, stretched, under the UI
//!   ([`screen_pixel_ratio`], D1199): a game set to Half for its frame
//!   rate keeps that frame rate on the menu, and play pays nothing when
//!   the ratios are the same.
//! - **Visibility.** A hidden page (`visibilitychange`) sends
//!   [`FocusLost`](crate::FocusLost), the same message as a native window
//!   losing the focus; a game pauses on it.
//! - **Stage commands.** The page's `G.stage(cmd)` (a JSON object with a
//!   `cmd`) queues a command; each frame the registered handler for its
//!   name runs with the world ([`register_stage`]), and `staged` counts
//!   what was applied so a test harness can wait for the frame that
//!   applied its writes (D1200).
//! - **The status.** [`publish_status`] writes the contract's fields on the
//!   global each frame (`crate::bridge`).

use crate::bridge;
use crate::status::LoadStatus;
use crate::ui::{Rect, css_shown};
use crate::{FocusGained, FocusLost};
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::Extent3d;
use bevy::ui::{CalculatedClip, ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, WindowRef};
use serde_json::Value;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};

/// The canvas's id in the page (`template/web/index.html`).
pub const CANVAS_ID: &str = "game";

/// The page's query string (`location.search`).
pub fn query() -> String {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default()
}

/// The canvas element.
pub fn canvas() -> Option<web_sys::Element> {
    web_sys::window()?.document()?.get_element_by_id(CANVAS_ID)
}

/// `window.devicePixelRatio`.
pub fn device_pixel_ratio() -> f64 {
    web_sys::window().map_or(1.0, |w| w.device_pixel_ratio())
}

/// `document.hidden`.
pub fn hidden() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .is_some_and(|d| d.hidden())
}

// ── Visibility ───────────────────────────────────────────────────────────

/// The page went hidden since the last frame (`visibilitychange`). A hidden
/// page gets no frames, so the event is latched and the message comes in
/// the first frame back, before the game's ticks.
static WENT_HIDDEN: AtomicBool = AtomicBool::new(false);

/// `document.addEventListener('visibilitychange', …)`.
pub fn install_visibility() {
    if let Some(d) = web_sys::window().and_then(|w| w.document()) {
        let f = Closure::<dyn FnMut()>::new(|| {
            if hidden() {
                WENT_HIDDEN.store(true, Ordering::Relaxed);
            }
        });
        let _ = d.add_event_listener_with_callback("visibilitychange", f.as_ref().unchecked_ref());
        f.forget();
    }
}

/// [`FocusLost`] while the page is hidden (or went hidden since the last
/// frame), [`FocusGained`] when it comes back.
pub fn focus_messages(
    mut was_hidden: Local<bool>,
    mut lost: MessageWriter<FocusLost>,
    mut gained: MessageWriter<FocusGained>,
) {
    let went = WENT_HIDDEN.swap(false, Ordering::Relaxed);
    let now = hidden();
    if went || now {
        lost.write(FocusLost);
    }
    if *was_hidden && !now {
        gained.write(FocusGained);
    }
    *was_hidden = now;
}

// ── Gestures ─────────────────────────────────────────────────────────────

static GESTURES: AtomicU32 = AtomicU32::new(0);
static ON_GESTURE: Mutex<Vec<fn(&str)>> = Mutex::new(Vec::new());
static PERMISSION: Mutex<Option<fn(bool)>> = Mutex::new(None);

/// Runs `f` inside every gesture the page passes in (`pointerdown`,
/// `pointerup`, `touchend`, `click`, `keydown`): the sound wakes there.
pub fn on_gesture(f: fn(&str)) {
    ON_GESTURE.lock().unwrap_or_else(|e| e.into_inner()).push(f);
}

/// The permission hook: runs inside a tap on a [`PermissionOnTap`] control
/// (`true`) or a [`FullscreenOnTap`] one (`false`), before the fullscreen
/// request.
pub fn set_permission_hook(f: fn(bool)) {
    *PERMISSION.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

/// The gesture bridge: the page calls this inside a user gesture (the
/// game exports it as `gesture`).
pub fn gesture(kind: &str) {
    GESTURES.fetch_add(1, Ordering::Relaxed);
    let fs: Vec<fn(&str)> = ON_GESTURE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    for f in fs {
        f(kind);
    }
}

/// Gestures the page has passed in so far.
pub fn gestures() -> u32 {
    GESTURES.load(Ordering::Relaxed)
}

/// Whether taps may go fullscreen (a touch screen, and the setting allows
/// it) and ask a permission (a touch screen). The game sets it.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct TapPolicy {
    pub fullscreen: bool,
    pub permission: bool,
}

/// A control that goes fullscreen (and asks for landscape) when tapped
/// on a touch screen: a game's Start, Again, Restart.
#[derive(Component)]
pub struct FullscreenOnTap;

/// A control whose tap asks a permission inside the gesture: the steering
/// choice's Tilt option on an iPhone.
#[derive(Component)]
pub struct PermissionOnTap;

/// The marked controls' shown boxes in CSS px, as of the last frame:
/// (fullscreen allowed, fullscreen rects, permission allowed, permission
/// rects).
static TAP_RECTS: Mutex<(bool, Vec<Rect>, bool, Vec<Rect>)> =
    Mutex::new((false, Vec::new(), false, Vec::new()));

type TapQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedNode,
        &'static UiGlobalTransform,
        &'static InheritedVisibility,
        Option<&'static CalculatedClip>,
        Has<FullscreenOnTap>,
        Has<PermissionOnTap>,
    ),
    Or<(With<FullscreenOnTap>, With<PermissionOnTap>)>,
>;

/// Where the tap-sensitive controls are this frame.
fn collect_tap_rects(policy: Res<TapPolicy>, scale: Res<CanvasScale>, q: TapQuery) {
    let css = scale.css.max(0.01);
    let mut full = Vec::new();
    let mut perm = Vec::new();
    for (n, t, vis, clip, is_full, is_perm) in q.iter() {
        if !vis.get() {
            continue;
        }
        let r = css_shown(n, t, clip, css);
        if is_full {
            full.push(r);
        }
        if is_perm && policy.permission {
            perm.push(r);
        }
    }
    *TAP_RECTS.lock().unwrap_or_else(|e| e.into_inner()) =
        (policy.fullscreen, full, policy.permission, perm);
}

/// The page's gesture handlers call this inside the up-events with the
/// pointer's position in CSS px (the game exports it as `gesture_at`).
pub fn gesture_at(_kind: &str, x: f64, y: f64) {
    let (on, full, _, perm) = TAP_RECTS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let p = Vec2::new(x as f32, y as f32);
    let inside = |rs: &[Rect]| rs.iter().any(|r| crate::ui::inside(*r, p));
    let tapped_full = inside(&full);
    let tapped_perm = inside(&perm);
    if (tapped_full || tapped_perm)
        && let Some(f) = *PERMISSION.lock().unwrap_or_else(|e| e.into_inner())
    {
        f(tapped_perm);
    }
    if on && tapped_full {
        // One tap brings pointer-up, touch-end and click: ask once.
        static LAST: Mutex<f64> = Mutex::new(-1e9);
        let now = js_sys::Date::now();
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if now - *last >= 800.0 {
            let f = js_sys::Function::new_no_args(
                "if (document.fullscreenElement) return; const el = document.documentElement; \
                 const req = el.requestFullscreen || el.webkitRequestFullscreen; if (!req) return; \
                 try { Promise.resolve(req.call(el, { navigationUI: 'hide' })) \
                   .then(() => screen.orientation && screen.orientation.lock && screen.orientation.lock('landscape')) \
                   .catch(() => {}); } catch (e) { /* not allowed here */ }",
            );
            let _ = f.call0(&JsValue::NULL);
            *last = now;
        }
    }
}

// ── Screenshots ──────────────────────────────────────────────────────────

static SCREENSHOT: Mutex<Option<String>> = Mutex::new(None);

/// Captures the next frame with Bevy's screenshot and hands it to the
/// browser as a download named `name` (a dev and test hook: headless Chrome
/// does not composite a WebGPU canvas into its own screenshots). Sets the
/// global's `shot` to the name when done.
pub fn screenshot(name: String) {
    *SCREENSHOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(name);
}

fn screenshot_hook(mut commands: Commands) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let Some(name) = SCREENSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let done = name.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(name))
        .observe(move |_: On<ScreenshotCaptured>| {
            bridge::set("shot", done.as_str());
        });
}

// ── The pixel ratio ──────────────────────────────────────────────────────

/// The pixel-ratio policy (D437, D1194, D575).
#[derive(Resource, Debug, Clone, Copy)]
pub struct RenderScale {
    /// High quality: the scene draws at the device's ratio, capped.
    pub hq: bool,
    /// The cap on the device's ratio with `hq` (1.5: a 3× phone draws at
    /// 1.5×).
    pub hq_cap: f64,
    /// The render scale, 0.25 to 1 (`Full`, `3/4`, `Half`).
    pub scale: f64,
    /// A screen's ratio is at least `min(dpr, screen_min)` (2: sharp text).
    pub screen_min: f64,
}

impl Default for RenderScale {
    fn default() -> Self {
        RenderScale {
            hq: true,
            hq_cap: 1.5,
            scale: 1.0,
            screen_min: 2.0,
        }
    }
}

impl RenderScale {
    /// The ratio the scene draws at: `(hq ? min(dpr, hq_cap) : 1) × scale`.
    pub fn scene_ratio(&self) -> f32 {
        let dpr = device_pixel_ratio();
        ((if self.hq { dpr.min(self.hq_cap) } else { 1.0 }) * self.scale) as f32
    }

    /// The ratio a screen draws at: `max(scene, min(dpr, screen_min))`.
    pub fn screen_ratio(&self) -> f32 {
        let dpr = device_pixel_ratio() as f32;
        self.scene_ratio().max(dpr.min(self.screen_min as f32))
    }
}

/// Draws the canvas at `pr` pixels per CSS pixel (the module's notes).
/// Called every frame (a window resize puts winit's size back); nothing is
/// written when nothing changes.
pub fn set_pixel_ratio(w: &mut Window, pr: f32) {
    if (w.resolution.scale_factor() - pr).abs() > 1e-3 {
        w.resolution.set_scale_factor_override(Some(pr));
    }
    let Some(want) = canvas_pixels(pr) else {
        return;
    };
    if (
        w.resolution.physical_width(),
        w.resolution.physical_height(),
    ) != want
    {
        w.resolution.set_physical_resolution(want.0, want.1);
    }
}

/// The canvas's CSS box at `pr` pixels per CSS pixel, or `None` before the
/// page has laid it out.
pub fn canvas_pixels(pr: f32) -> Option<(u32, u32)> {
    let canvas = canvas()?;
    let (cw, ch) = (canvas.client_width(), canvas.client_height());
    if cw <= 0 || ch <= 0 {
        return None;
    }
    Some((
        (cw as f32 * pr).round().max(1.0) as u32,
        (ch as f32 * pr).round().max(1.0) as u32,
    ))
}

/// Whether the shell drives the pixel ratio each frame (a game with
/// screens over its scene turns it on), and whether a screen is up now
/// (the game says each frame).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PixelRatioPolicy {
    pub on: bool,
    pub screen_up: bool,
}

/// The set [`screen_pixel_ratio`] runs in (`Update`): a game orders its
/// screen building before it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelRatioSet;

/// The set the stage commands run in (`Update`): a game orders its pointer
/// handling after it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StageSet;

/// The screens' camera (D1199): while a screen draws at a sharper ratio
/// than the scene's, it draws the scene's picture and the UI into the
/// window; otherwise it is off.
#[derive(Component)]
pub struct ScreenCam;

/// The scene's picture under the screens (D1199), the screens' camera's
/// lowest node.
#[derive(Component)]
pub struct ScenePicture;

/// The image the scene draws into under a sharper screen (D1199).
#[derive(Resource)]
pub struct SceneImage(pub Handle<Image>);

/// The screens' camera, off, and the node that shows the scene's picture
/// on it, hidden (D1199).
fn spawn_screen_cam(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::TextureFormat;
    use bevy::render::view::Msaa;
    let mut image = Image::new_target_texture(1, 1, TextureFormat::Rgba8UnormSrgb, None);
    // Drawn into, never uploaded: no copy of its pixels in memory.
    image.data = None;
    // Drawn afresh every frame, so a resize (a phone turned on a screen)
    // keeps nothing of the old picture. Bevy's copy of it would need
    // COPY_SRC, which a target texture lacks: WebGPU rejected the copy and
    // Bevy quit the app on the validation error (D1204).
    image.copy_on_resize = false;
    image.sampler = ImageSampler::linear();
    let image = images.add(image);
    let cam = commands
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                is_active: false,
                ..default()
            },
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            ScreenCam,
        ))
        .id();
    commands.spawn((
        ImageNode::new(image.clone()),
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(i32::MIN),
        UiTargetCamera(cam),
        ScenePicture,
    ));
    commands.insert_resource(SceneImage(image));
}

type SceneCams<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut RenderTarget, Has<IsDefaultUiCamera>),
    (With<Camera3d>, Without<ScreenCam>),
>;
type ScreenCams<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Camera, Has<IsDefaultUiCamera>),
    (With<ScreenCam>, Without<Camera3d>),
>;

/// Sharp text on the screens, the scene's pixel ratio while playing, and
/// the scene at its own ratio under every screen (D1199): a screen draws
/// its text at the screens' ratio, but the scene behind it (an attract
/// camera's flight, a paused run) costs what play does. While the
/// screens' ratio is the higher, the scene's camera draws into
/// [`SceneImage`], sized at the scene's ratio, and the screens' camera
/// draws it, stretched, under the UI at the window's. When the two are the
/// same the scene's camera draws into the window as before and the
/// screens' camera is off, so play costs nothing more.
#[allow(clippy::too_many_arguments)]
pub fn screen_pixel_ratio(
    mut commands: Commands,
    policy: Res<PixelRatioPolicy>,
    scale: Res<RenderScale>,
    scene_image: Option<Res<SceneImage>>,
    mut images: ResMut<Assets<Image>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut scene_cams: SceneCams,
    mut screen_cams: ScreenCams,
    mut picture: Query<&mut Node, With<ScenePicture>>,
) {
    if !policy.on {
        return;
    }
    let want = if policy.screen_up {
        scale.screen_ratio()
    } else {
        scale.scene_ratio()
    };
    if let Ok(mut w) = windows.single_mut() {
        set_pixel_ratio(&mut w, want);
    }

    let scene_ratio = scale.scene_ratio();
    let (
        Some(scene_image),
        Ok((scene, mut target, scene_default)),
        Ok((screen, mut cam, screen_default)),
    ) = (
        scene_image,
        scene_cams.single_mut(),
        screen_cams.single_mut(),
    )
    else {
        return;
    };
    let size = canvas_pixels(scene_ratio).filter(|_| want > scene_ratio + 1e-3);
    if let Some((w, h)) = size {
        let extent = Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        if images
            .get(&scene_image.0)
            .is_some_and(|i| i.texture_descriptor.size != extent)
            && let Some(mut i) = images.get_mut(&scene_image.0)
        {
            i.resize(extent);
        }
    }
    let split = size.is_some();
    let into = split.then(|| ImageRenderTarget {
        handle: scene_image.0.clone(),
        scale_factor: scene_ratio,
    });
    let now = match &*target {
        RenderTarget::Image(t) => Some(t),
        _ => None,
    };
    if now != into.as_ref() {
        *target = into.map_or(
            RenderTarget::Window(WindowRef::Primary),
            RenderTarget::Image,
        );
    }
    if cam.is_active != split {
        cam.is_active = split;
    }
    if let Ok(mut node) = picture.single_mut() {
        let display = if split { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    // The UI goes with the camera that draws into the window.
    for (e, is, should) in [
        (scene, scene_default, !split),
        (screen, screen_default, split),
    ] {
        if should && !is {
            commands.entity(e).insert(IsDefaultUiCamera);
        } else if is && !should {
            commands.entity(e).remove::<IsDefaultUiCamera>();
        }
    }
}

/// CSS px per logical px (`css`: the canvas's CSS width over the window's
/// logical width; they differ under the pixel-ratio override) and the
/// override's factor over the device's (`touch`: what takes a pointer
/// position in logical px to CSS px), measured each frame.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CanvasScale {
    pub css: f32,
    pub touch: f32,
}

impl Default for CanvasScale {
    fn default() -> Self {
        CanvasScale {
            css: 1.0,
            touch: 1.0,
        }
    }
}

fn measure_canvas(mut scale: ResMut<CanvasScale>, windows: Query<&Window, With<PrimaryWindow>>) {
    let css_w = canvas().map_or(0, |c| c.client_width());
    if let Ok(w) = windows.single()
        && css_w > 0
        && w.width() > 0.0
    {
        let k = css_w as f32 / w.width();
        if (k - scale.css).abs() > 1e-3 {
            scale.css = k;
        }
        let t = w.resolution.scale_factor() / w.resolution.base_scale_factor().max(0.01);
        if (t - scale.touch).abs() > 1e-4 {
            scale.touch = t;
        }
    }
}

// ── What the page measured ───────────────────────────────────────────────

/// `isTouchDevice` as the page decided it (`G.touch`), if it has.
pub fn page_touch() -> Option<bool> {
    bridge::get_bool("touch")
}

/// The safe-area insets (`env(safe-area-inset-*)`), CSS px.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// The insets the page measured (`G.insets`).
pub fn insets() -> Option<Insets> {
    let ins = bridge::get("insets")?;
    let num = |k: &str| {
        bridge::field(&ins, k)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    };
    Some(Insets {
        top: num("top"),
        right: num("right"),
        bottom: num("bottom"),
        left: num("left"),
    })
}

/// How far the page's build line reaches up from the window's foot
/// (`G.footer`), CSS px.
pub fn footer() -> Option<f64> {
    bridge::get_num("footer")
}

/// The browser's own prompt (no text field is drawn in the canvas, and a
/// phone's keyboard only comes up for the DOM's). The answer, trimmed, or
/// `None` if cancelled or empty.
pub fn prompt(message: &str, current: &str) -> Option<String> {
    let w = web_sys::window()?;
    let a = w.prompt_with_message_and_default(message, current).ok()??;
    let a = a.trim().to_string();
    (!a.is_empty()).then_some(a)
}

/// Copies `text` to the clipboard (`navigator.clipboard.writeText`); where
/// there is none (an insecure page), shows it in the browser's prompt to
/// copy by hand.
pub fn copy_text(text: &str) {
    let Some(w) = web_sys::window() else { return };
    let nav = w.navigator();
    let write = js_sys::Reflect::get(&nav, &"clipboard".into())
        .ok()
        .filter(|c| !c.is_undefined())
        .and_then(|c| {
            let f = js_sys::Reflect::get(&c, &"writeText".into()).ok()?;
            let f = f.dyn_into::<js_sys::Function>().ok()?;
            f.call1(&c, &text.into()).ok()
        });
    if write.is_none() {
        let _ = w.prompt_with_message_and_default("Copy this link", text);
    }
}

// ── Stage commands ───────────────────────────────────────────────────────

static STAGE: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Stage commands applied so far (`G.staged`).
static STAGED: AtomicU32 = AtomicU32::new(0);
type StageFn = fn(&mut World, &Value);
static HANDLERS: Mutex<Vec<(&'static str, StageFn)>> = Mutex::new(Vec::new());

/// `G.stage(json)`: queues a command (the game exports it as `stage`).
pub fn stage(cmd: String) {
    STAGE.lock().unwrap_or_else(|e| e.into_inner()).push(cmd);
}

/// Registers the handler for commands whose `cmd` is `name`.
pub fn register_stage(name: &'static str, f: StageFn) {
    HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((name, f));
}

/// Stage commands applied so far.
pub fn staged() -> u32 {
    STAGED.load(Ordering::Relaxed)
}

fn drain_stage(world: &mut World) {
    let cmds: Vec<String> = std::mem::take(&mut *STAGE.lock().unwrap_or_else(|e| e.into_inner()));
    for c in cmds {
        STAGED.fetch_add(1, Ordering::Relaxed);
        let Ok(v) = serde_json::from_str::<Value>(&c) else {
            continue;
        };
        let name = v["cmd"].as_str().unwrap_or("");
        let handler = HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, f)| *f);
        match handler {
            Some(f) => f(world, &v),
            None => warn!("stage: unknown command {c}"),
        }
    }
}

// ── The status ───────────────────────────────────────────────────────────

/// The contract's fields and the shell's extras on the global, each frame.
pub fn publish_status(status: Res<LoadStatus>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let now = window.performance().map_or(0.0, |p| p.now());
    if status.frames == 1 {
        bridge::set("firstFrameMs", now);
    }
    if status.ready && status.ready_frames == 1 {
        bridge::set("readyMs", now);
    }
    bridge::set("state", status.state.name());
    bridge::set("progress", status.progress);
    bridge::set("frames", status.frames as f64);
    bridge::set("frameMs", status.frame_ms);
    bridge::set("pipelinesWaiting", status.pipelines_waiting as f64);
    bridge::set("ready", status.ready);
    bridge::set("gestures", gestures());
    bridge::set("staged", staged());
    bridge::set("backend", crate::backend());
    bridge::set("warmUp", status.warm_up as f64);
    bridge::set("lateFrames", status.late_frames as f64);
    bridge::set("scenes", status.scenes);
    if let Some(e) = &status.error {
        bridge::set("error", e.as_str());
    }
}

/// The web glue: the status global under its name and aliases, the status
/// published each frame, screenshots, the pixel ratio and split-screen
/// camera, the canvas scale, the tap-sensitive controls, the stage
/// commands.
pub struct WebPlugin {
    /// The status global's name (`__game`).
    pub global: &'static str,
    /// Other names for the same object (`__mp`).
    pub aliases: &'static [&'static str],
}

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        bridge::install(self.global, self.aliases);
        app.init_resource::<RenderScale>()
            .init_resource::<PixelRatioPolicy>()
            .init_resource::<CanvasScale>()
            .init_resource::<TapPolicy>()
            .add_systems(Startup, spawn_screen_cam)
            .add_systems(PreUpdate, measure_canvas)
            .add_systems(
                Update,
                (
                    drain_stage.in_set(StageSet),
                    screen_pixel_ratio.in_set(PixelRatioSet),
                    screenshot_hook,
                ),
            )
            .add_systems(Last, (collect_tap_rects, publish_status));
    }
}
