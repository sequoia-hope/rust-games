//! The game as a whole, headless ([`crate::headless`]: every system of the
//! client, no window, no GPU), driven frame by frame with key presses and
//! touches as the player makes them, through every screen change there is:
//! the menu's settings (each course change rebuilds the world), starting a
//! run, pausing, restarting, the results, quitting to the menu.
//!
//! After every frame [`Harness::check`] holds the game to what must always
//! be true whatever the order of events: one HUD in a run and none in the
//! menu, one panel at most, the run's riders all there and nobody else's,
//! the showcase rider only in the menu, the world spawned exactly once for
//! its course, nothing NaN, and the meshes not piling up. A panic anywhere
//! in the game fails the test too. These are the crashes that come "on a
//! screen change": the browser's own run of the same idea, with the GPU, is
//! `tools/stress.mjs`.

use crate::hud::HudRoot;
use crate::menu::{Panel, Showcase};
use crate::options::Options;
use crate::ride::{RiderSlot, Run};
use crate::world::{Built, CourseEntity};
use crate::{Screen, Setup};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use sb_sim::course::CourseId;
use sb_sim::race::{Mode, State};
use std::collections::HashMap;
use std::time::Duration;

struct Harness {
    app: App,
    frame: u64,
    /// The world's entity count for each course, once seen.
    world_size: HashMap<CourseId, usize>,
    /// The most meshes seen, after the first run.
    meshes_after_warmup: Option<usize>,
    max_meshes: usize,
    touch_id: u64,
}

const FRAME: f64 = 1.0 / 60.0;

impl Harness {
    fn new(o: Options) -> Harness {
        let mut app = crate::headless(o);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(FRAME)));
        app.finish();
        app.cleanup();
        let mut h = Harness {
            app,
            frame: 0,
            world_size: HashMap::new(),
            meshes_after_warmup: None,
            max_meshes: 0,
            touch_id: 0,
        };
        h.frames(3);
        h
    }

    fn query(q: &str) -> Harness {
        Harness::new(Options::from_query(q))
    }

    fn world(&mut self) -> &mut World {
        self.app.world_mut()
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
            self.frame += 1;
            self.check();
        }
    }

    fn seconds(&mut self, s: f64) {
        self.frames((s / FRAME).ceil() as usize);
    }

    fn send_key(&mut self, k: KeyCode, state: ButtonState) {
        self.world().write_message(KeyboardInput {
            key_code: k,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }

    /// A key down for a frame, then up.
    fn key(&mut self, k: KeyCode) {
        self.send_key(k, ButtonState::Pressed);
        self.frames(1);
        self.send_key(k, ButtonState::Released);
        self.frames(1);
    }

    fn keys(&mut self, ks: &[KeyCode]) {
        for k in ks {
            self.key(*k);
        }
    }

    fn hold(&mut self, k: KeyCode, frames: usize) {
        self.send_key(k, ButtonState::Pressed);
        self.frames(frames);
        self.send_key(k, ButtonState::Released);
        self.frames(1);
    }

    /// A finger down at `at`, moved by `by` over `frames`, then lifted.
    fn swipe(&mut self, at: Vec2, by: Vec2, frames: usize) {
        self.touch_id += 1;
        let id = self.touch_id;
        let send = |w: &mut World, phase, position| {
            w.write_message(TouchInput {
                phase,
                position,
                window: Entity::PLACEHOLDER,
                force: None,
                id,
            });
        };
        send(self.world(), TouchPhase::Started, at);
        self.frames(1);
        for i in 1..=frames {
            let p = at + by * (i as f32 / frames as f32);
            send(self.world(), TouchPhase::Moved, p);
            self.frames(1);
        }
        send(self.world(), TouchPhase::Ended, at + by);
        self.frames(1);
    }

    fn screen(&self) -> Screen {
        *self.app.world().resource::<bevy::state::state::State<Screen>>().get()
    }

    fn count<T: Component>(&mut self) -> usize {
        self.world().query_filtered::<(), With<T>>().iter(self.world()).count()
    }

    fn panels(&mut self) -> Vec<Panel> {
        self.world().query::<&Panel>().iter(self.world()).copied().collect()
    }

    fn run_state(&self) -> Option<State> {
        self.app.world().get_resource::<Run>().map(|r| r.session.state)
    }

    fn paused(&self) -> bool {
        self.app.world().get_resource::<Run>().is_some_and(|r| r.paused)
    }

    fn setup(&mut self) -> Mut<'_, Setup> {
        self.world().resource_mut::<Setup>()
    }

    /// Frames until `f` holds, failing after `secs`.
    fn until(&mut self, secs: f64, what: &str, f: impl Fn(&mut Harness) -> bool) {
        let n = (secs / FRAME) as usize;
        for _ in 0..n {
            if f(self) {
                return;
            }
            self.frames(1);
        }
        panic!("frame {}: not {what} within {secs} s", self.frame);
    }

    /// From the menu into a run, by its keys (Enter on the focused row).
    fn ride(&mut self) {
        assert_eq!(self.screen(), Screen::Menu);
        self.key(KeyCode::Enter);
        self.frames(2);
        assert_eq!(self.screen(), Screen::Ride, "Enter on the menu starts a run");
    }

    /// The pause panel, by its key.
    fn pause(&mut self) {
        self.key(KeyCode::KeyP);
        assert!(self.paused(), "P pauses");
        assert_eq!(self.panels(), vec![Panel::Pause]);
    }

    /// From a run to the menu, through the pause panel.
    fn quit(&mut self) {
        if self.run_state() == Some(State::Done) {
            self.until(2.0, "showing the results", |h| h.panels() == vec![Panel::Results]);
            self.keys(&[KeyCode::ArrowDown, KeyCode::Enter]);
        } else {
            if !self.paused() {
                self.pause();
            }
            self.keys(&[KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::Enter]);
        }
        self.frames(2);
        assert_eq!(self.screen(), Screen::Menu);
    }

    /// What must hold after every frame, whatever happened before.
    fn check(&mut self) {
        let frame = self.frame;
        let screen = self.screen();
        let huds = self.count::<HudRoot>();
        let riders = self.count::<RiderSlot>();
        let showcase = self.count::<Showcase>();
        let panels = self.panels();
        let run = self
            .app
            .world()
            .get_resource::<Run>()
            .map(|r| (r.session.entrants.len(), r.session.state));
        assert!(panels.len() <= 1, "frame {frame}: {} panels at once: {panels:?}", panels.len());
        match screen {
            Screen::Ride => {
                let Some((entrants, _)) = run else {
                    panic!("frame {frame}: a run on screen without a Run");
                };
                assert_eq!(huds, 1, "frame {frame}: {huds} HUDs in a run");
                assert_eq!(riders, entrants, "frame {frame}: {riders} riders drawn for {entrants} entrants");
                assert_eq!(showcase, 0, "frame {frame}: the menu's rider is still out in a run");
                assert!(!panels.contains(&Panel::Title), "frame {frame}: the title panel in a run");
            }
            Screen::Menu => {
                assert!(run.is_none(), "frame {frame}: a Run left over in the menu");
                assert_eq!(huds, 0, "frame {frame}: a HUD in the menu");
                assert_eq!(riders, 0, "frame {frame}: {riders} run riders left in the menu");
                assert_eq!(showcase, 1, "frame {frame}: {showcase} showcase riders in the menu");
                assert_eq!(panels, vec![Panel::Title], "frame {frame}: the menu's panels");
            }
        }

        // The world: spawned once for its course, never twice, never
        // partly.
        let course = self.app.world().resource::<Built>().course.as_ref().map(|c| c.id);
        let want = self.app.world().resource::<Setup>().course;
        let size = self.count::<CourseEntity>();
        if let Some(id) = course
            && id == want
        {
            let first = *self.world_size.entry(id).or_insert(size);
            assert_eq!(size, first, "frame {frame}: {id:?}'s world has {size} entities, {first} the first time");
        }

        // Nothing NaN on screen.
        let mut q = self.world().query_filtered::<&Transform, With<RiderSlot>>();
        for t in q.iter(self.app.world()) {
            assert!(t.translation.is_finite() && t.rotation.is_finite(), "frame {frame}: a rider at {t:?}");
        }
        if let Some(run) = self.app.world().get_resource::<Run>() {
            for e in &run.session.entrants {
                let r = &e.rider;
                assert!(r.pos.is_finite() && r.vel.is_finite() && r.yaw.is_finite(), "frame {frame}: {r:?}");
            }
        }

        // Meshes come and go with the world and the riders; they must not
        // pile up.
        let meshes = self.app.world().resource::<Assets<Mesh>>().len();
        self.max_meshes = self.max_meshes.max(meshes);
        if let Some(base) = self.meshes_after_warmup {
            assert!(
                meshes <= base * 2 + 200,
                "frame {frame}: {meshes} meshes alive, {base} after the first run: a leak"
            );
        }
    }

    /// Notes the mesh count once everything has been built once.
    fn warmed_up(&mut self) {
        self.meshes_after_warmup = Some(self.max_meshes);
    }
}

const COURSE_ROW: usize = 1;

#[test]
fn boots_to_the_menu() {
    let mut h = Harness::query("");
    h.frames(10);
    assert_eq!(h.screen(), Screen::Menu);
    assert!(h.app.world().resource::<Built>().course.is_some(), "the world is built behind the menu");
}

#[test]
fn every_course_and_mode_rides_and_quits() {
    let mut h = Harness::query("");
    for course in CourseId::ALL {
        for mode in Mode::ALL {
            {
                let mut s = h.setup();
                s.course = course;
                s.mode = mode;
            }
            h.frames(2);
            h.ride();
            h.seconds(4.5);
            assert!(matches!(h.run_state(), Some(State::Running)), "{course:?} {mode:?}: running after the countdown");
            h.hold(KeyCode::ArrowLeft, 20);
            h.hold(KeyCode::Space, 30);
            h.seconds(1.0);
            h.quit();
        }
    }
}

#[test]
fn escape_pauses_and_stays_paused() {
    let mut h = Harness::query("go=1");
    h.seconds(1.0);
    h.key(KeyCode::Escape);
    h.frames(5);
    assert!(h.paused(), "Esc pauses (and the same press must not resume)");
    assert_eq!(h.panels(), vec![Panel::Pause]);
    h.key(KeyCode::Escape);
    h.frames(2);
    assert!(!h.paused(), "Esc again resumes");
    assert!(h.panels().is_empty());
    h.pause();
    h.key(KeyCode::Backspace);
    assert!(!h.paused(), "Backspace resumes");
}

#[test]
fn restarting_again_and_again_leaks_nothing() {
    let mut h = Harness::query("go=1&course=timber");
    h.seconds(5.0);
    h.warmed_up();
    let entities = h.world().entities().len();
    for i in 0..25 {
        h.pause();
        h.keys(&[KeyCode::ArrowDown, KeyCode::Enter]);
        h.frames(2);
        assert_eq!(h.screen(), Screen::Ride);
        assert!(matches!(h.run_state(), Some(State::Countdown(_))), "restart {i}: a fresh countdown");
        assert!(!h.paused());
        h.frames(1 + i % 7);
    }
    h.seconds(1.0);
    let now = h.world().entities().len();
    assert!(now <= entities + 50, "{now} entities after 25 restarts, {entities} before");
}

#[test]
fn spinning_through_the_courses_in_the_menu() {
    let mut h = Harness::query("");
    // To the course row.
    h.keys(&[KeyCode::ArrowUp; 8][..crate::menu::RIDE_ROW - COURSE_ROW]);
    assert_eq!(h.app.world().resource::<crate::menu::Focus>().index, COURSE_ROW);
    for _ in 0..2 * CourseId::ALL.len() {
        h.key(KeyCode::ArrowRight);
    }
    h.warmed_up();
    // A course a frame, then back, then the other way.
    for i in 0..40 {
        h.send_key(KeyCode::ArrowRight, ButtonState::Pressed);
        h.frames(1);
        h.send_key(KeyCode::ArrowRight, ButtonState::Released);
        if i % 3 == 0 {
            h.send_key(KeyCode::ArrowLeft, ButtonState::Pressed);
            h.frames(1);
            h.send_key(KeyCode::ArrowLeft, ButtonState::Released);
        }
    }
    h.frames(3);
    // And straight into a run from a course just built.
    h.key(KeyCode::ArrowRight);
    h.ride();
    h.seconds(1.0);
    h.quit();
}

#[test]
fn changing_rider_and_mode_in_the_menu() {
    let mut h = Harness::query("");
    // Mode (row 0), rider (row 2), assist (row 3), radio (row 4), back
    // and forth.
    for (row, n) in [(0, 7), (2, 9), (3, 3), (4, 6)] {
        h.world().resource_mut::<crate::menu::Focus>().index = row;
        for i in 0..n {
            h.key(if i % 4 == 3 { KeyCode::ArrowLeft } else { KeyCode::ArrowRight });
        }
    }
    h.world().resource_mut::<crate::menu::Focus>().index = crate::menu::RIDE_ROW;
    h.ride();
    h.seconds(1.0);
    h.quit();
}

#[test]
fn any_rider_number_in_the_address() {
    for q in ["rider=9", "rider=-1", "rider=x", "rider=4&go=1", "course=nowhere&mode=x&go=1"] {
        let mut h = Harness::query(q);
        h.frames(20);
        if h.screen() == Screen::Ride {
            h.quit();
        }
        h.frames(5);
    }
}

#[test]
fn straight_into_every_course() {
    for course in CourseId::ALL {
        for mode in Mode::ALL {
            let mut h = Harness::query(&format!("go=1&auto=1&course={}&mode={}", course.key(), mode.key()));
            h.seconds(6.0);
            assert_eq!(h.screen(), Screen::Ride);
            h.quit();
            h.frames(5);
        }
    }
}

#[test]
fn results_then_ride_again_then_menu() {
    for course in CourseId::ALL {
        let mut h = Harness::query(&format!("go=1&auto=1&skip=400&course={}", course.key()));
        h.until(120.0, "finished", |h| h.run_state() == Some(State::Done));
        h.until(2.0, "showing the results", |h| h.panels() == vec![Panel::Results]);
        // P on the results does nothing.
        h.key(KeyCode::KeyP);
        assert_eq!(h.panels(), vec![Panel::Results]);
        h.key(KeyCode::Enter);
        h.frames(2);
        assert!(matches!(h.run_state(), Some(State::Countdown(_))), "{course:?}: RIDE AGAIN restarts");
        h.until(150.0, "finished again", |h| h.run_state() == Some(State::Done));
        h.quit();
        h.ride();
        h.quit();
    }
}

#[test]
fn touch_through_a_run() {
    let mut h = Harness::query("go=1&touch=1");
    h.seconds(4.0);
    for i in 0..10 {
        // Steer, jump and grab, wherever the fingers land.
        let x = 100.0 + 90.0 * i as f32;
        h.swipe(Vec2::new(x, 300.0), Vec2::new(60.0 - 12.0 * i as f32, -40.0), 8);
    }
    assert!(h.app.world().resource::<Setup>().assist, "touch turns the landing assist on");
    h.quit();
}

/// Presses at random, seeded, for a long while: anything a player could do
/// in any order, keys mashed on every screen.
#[test]
fn mashing_every_key() {
    let mut h = Harness::query("");
    let keys = [
        KeyCode::Enter,
        KeyCode::Space,
        KeyCode::Escape,
        KeyCode::KeyP,
        KeyCode::Backspace,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ShiftLeft,
    ];
    let mut x = 0x2545_F491u32;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    let (mut rides, mut menus) = (0, 0);
    let mut was = h.screen();
    for _ in 0..3000 {
        let r = next();
        match r % 10 {
            0..=5 => h.key(keys[(r / 10) as usize % keys.len()]),
            6 => h.hold(keys[(r / 10) as usize % keys.len()], (r / 100 % 40) as usize),
            7 => h.swipe(Vec2::new((r % 1280) as f32, (r / 7 % 720) as f32), Vec2::new(40.0, -30.0), 3),
            _ => h.frames((r / 10 % 30) as usize),
        }
        let now = h.screen();
        if now != was {
            match now {
                Screen::Ride => rides += 1,
                Screen::Menu => menus += 1,
            }
            was = now;
        }
    }
    assert!(rides >= 3 && menus >= 3, "the mashing went between the screens: {rides} runs, {menus} menus");
}

fn said(h: &Harness) -> Vec<&'static str> {
    h.app.world().resource::<crate::radio::Announcers>().log.iter().map(|(t, _)| *t).collect()
}

fn talking(h: &Harness) -> bool {
    h.app.world().resource::<crate::radio::Announcers>().now.is_some()
}

#[test]
fn the_announcers_greet_the_menu_and_call_a_run() {
    let mut h = Harness::query("course=timber&mode=race&auto=1");
    h.seconds(3.0);
    assert_eq!(said(&h), vec!["ident"], "the menu opens with the broadcast's open");
    assert!(talking(&h), "a caption is up while they talk");
    h.ride();
    h.frames(3);
    let intro = *said(&h).last().unwrap();
    assert!(["timber", "kai"].contains(&intro), "the run starts with its intro, not {intro}");
    // To the finish (the autopilot rides), and the result is called.
    h.until(150.0, "finished", |h| h.run_state() == Some(State::Done));
    h.seconds(1.0);
    let last = *said(&h).last().unwrap();
    assert!(["win", "win-clean", "lose", "lose-trees"].contains(&last), "the race's result is called, not {last}: {:?}", said(&h));
}

#[test]
fn a_pause_cuts_them_off_and_keeps_them_quiet() {
    let mut h = Harness::query("go=1&course=crystal");
    h.frames(3);
    assert!(talking(&h), "the intro is being said");
    h.pause();
    assert!(!talking(&h), "the pause cuts the intro off");
    let n = said(&h).len();
    h.seconds(60.0);
    assert_eq!(said(&h).len(), n, "nothing said over the pause");
    assert!(!talking(&h));
}

#[test]
fn radio_off_and_music_only_keep_them_quiet() {
    for q in ["radio=off", "radio=music"] {
        let mut h = Harness::query(&format!("{q}&go=1&auto=1&course=pipe"));
        h.seconds(40.0);
        assert!(said(&h).is_empty(), "{q}: {:?}", said(&h));
    }
    let mut h = Harness::query("radio=voices");
    h.seconds(3.0);
    assert_eq!(said(&h), vec!["ident"], "voices only still talks");
}

#[test]
fn the_radio_setting_cycles_in_the_menu() {
    let mut h = Harness::query("radio=all");
    h.world().resource_mut::<crate::menu::Focus>().index = 4;
    let mut seen = Vec::new();
    for _ in 0..4 {
        h.key(KeyCode::ArrowRight);
        seen.push(h.app.world().resource::<Setup>().radio);
    }
    use crate::radio::Radio;
    assert_eq!(seen, vec![Radio::Music, Radio::Voices, Radio::Off, Radio::All]);
}

#[test]
#[ignore = "a report: cargo test -p sb_game mesh_bytes -- --ignored --nocapture"]
fn mesh_bytes() {
    let mut h = Harness::query("");
    for course in CourseId::ALL {
        h.setup().course = course;
        h.frames(3);
        let meshes = h.app.world().resource::<Assets<Mesh>>();
        let (mut n, mut verts, mut bytes) = (0, 0, 0);
        for (_, m) in meshes.iter() {
            n += 1;
            verts += m.count_vertices();
            bytes += m.count_vertices() * m.get_vertex_size() as usize
                + m.indices().map_or(0, |i| i.len() * 4);
        }
        println!("{course:?}: {n} meshes, {verts} vertices, {:.1} MB", bytes as f64 / 1048576.0);
    }
}

/// The bytes of mesh written each frame, Added or Modified: every write
/// is a whole upload to the GPU, through a buffer made for it.
#[derive(Resource, Default)]
struct Uploads(Vec<usize>);

fn count_uploads(mut events: MessageReader<AssetEvent<Mesh>>, meshes: Res<Assets<Mesh>>, mut up: ResMut<Uploads>) {
    let mut bytes = 0;
    for e in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = e
            && let Some(m) = meshes.get(*id)
        {
            bytes += m.count_vertices() * m.get_vertex_size() as usize + m.indices().map_or(0, |i| i.len() * 4);
        }
    }
    up.0.push(bytes);
}

/// What got Avalanche Bowl's tab killed on the owner's iPhone within
/// seconds of the course appearing (2026-10-18), though no browser here
/// minded: its falling snow, a mesh rebuilt each frame, was a fresh 200 KB
/// upload a frame. Riding every course, what is written to meshes each
/// frame stays small; anything that moves every frame belongs in a shader
/// (`snowfall.wgsl`) or a transform.
#[test]
fn every_course_uploads_little_per_frame() {
    for course in CourseId::ALL {
        let mut h = Harness::query(&format!("go=1&auto=1&course={}", course.key()));
        h.app.init_resource::<Uploads>().add_systems(Last, count_uploads);
        h.seconds(1.0);
        h.world().resource_mut::<Uploads>().0.clear();
        h.seconds(12.0);
        let up = &h.app.world().resource::<Uploads>().0;
        let mean = up.iter().sum::<usize>() as f64 / up.len() as f64 / 1024.0;
        let max = *up.iter().max().unwrap() as f64 / 1024.0;
        println!("{course:?}: {mean:.1} KB a frame on average, {max:.1} KB at most");
        // The spray off the board writes a few KB while it flies.
        assert!(mean <= 8.0, "{course:?}: {mean:.1} KB of mesh written a frame on average");
        assert!(max <= 64.0, "{course:?}: {max:.1} KB of mesh written in one frame");
    }
}
