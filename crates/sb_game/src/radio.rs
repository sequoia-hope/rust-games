//! The commentary in the client: the announcers ([`sb_radio`]) told what
//! happens in the game, their lines said one after another with captions
//! at the foot of the screen, and the radio's mix (music and voices, as
//! the menu's RADIO setting says) handed to the sound.
//!
//! The director decides what to say; this module turns the run's events
//! into its cues (and spots what no event says: the lead changing hands,
//! a long flight, a flat-out tuck), and plays each bit line by line. A line
//! lasts as long as its clip when the clip plays ([`crate::audio`]), else
//! (no sound natively, a clip not recorded yet, or slow to arrive) as long
//! as it takes to read, so the captions work everywhere and the tests can
//! run them headless.

use crate::ride::{Feed, RideSet, Run};
use crate::ui::{self, Fonts};
use crate::{Screen, Setup};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use sb_radio::director::{Cue, Director, Scene};
use sb_radio::{Who, clip_id};
use sb_sim::race::{Mode, SessionEvent, State};
use sb_sim::rider::Event;
use sb_sim::tricks::Landing;
use std::collections::VecDeque;

pub fn plugin(app: &mut App) {
    app.init_resource::<Announcers>()
    .init_resource::<Watch>()
    .add_systems(Startup, spawn_captions)
    .add_systems(Update, (direct, narrate, captions).chain().after(RideSet));
}

/// What the RADIO setting plays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Radio {
    /// Music and the announcers.
    #[default]
    All,
    Music,
    Voices,
    Off,
}

impl Radio {
    pub const ALL: [Radio; 4] = [Radio::All, Radio::Music, Radio::Voices, Radio::Off];
    pub fn music(self) -> bool {
        matches!(self, Radio::All | Radio::Music)
    }
    pub fn voices(self) -> bool {
        matches!(self, Radio::All | Radio::Voices)
    }
    pub fn label(self) -> &'static str {
        match self {
            Radio::All => "MUSIC + VOICES",
            Radio::Music => "MUSIC ONLY",
            Radio::Voices => "VOICES ONLY",
            Radio::Off => "OFF",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Radio::All => "all",
            Radio::Music => "music",
            Radio::Voices => "voices",
            Radio::Off => "off",
        }
    }
    pub fn from_key(k: &str) -> Option<Radio> {
        Radio::ALL.into_iter().find(|r| r.key() == k)
    }
}

/// A line to say: its clip, who says it and the words.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub clip: String,
    pub who: Who,
    pub text: &'static str,
}

/// The line being said.
#[derive(Clone, Debug)]
pub struct Saying {
    pub line: Line,
    /// Seconds since it was asked for.
    pub age: f32,
    /// How long it lasts, once known (its clip started, or given up on).
    pub len: Option<f32>,
}

#[derive(Resource)]
pub struct Announcers {
    pub director: Director,
    /// Lines still to say, in order.
    pub queue: VecDeque<Line>,
    pub now: Option<Saying>,
    /// Every bit said, for tests: (topic, bit).
    pub log: Vec<(&'static str, usize)>,
}

impl Default for Announcers {
    fn default() -> Self {
        Announcers {
            director: Director::new(0x1080),
            queue: VecDeque::new(),
            now: None,
            log: Vec::new(),
        }
    }
}

/// How long a clip may keep a line waiting before it is shown without it.
const CLIP_WAIT: f32 = 0.7;

/// What no event says, watched from frame to frame.
#[derive(Resource, Default)]
struct Watch {
    /// The run's serial, to see a new one start.
    serial: u64,
    /// The screen the last frame was on.
    last: Option<Scene>,
    place: usize,
    /// The place being held, and for how long.
    new_place: usize,
    held: f32,
    air: f32,
    air_said: bool,
    fast: f32,
}

fn scene(screen: Screen, run: Option<&Run>) -> Scene {
    match (screen, run) {
        (Screen::Ride, Some(r)) if r.paused => Scene::Paused,
        (Screen::Ride, Some(r)) if r.session.state == State::Done && r.done_for > 0.4 => Scene::Results,
        (Screen::Ride, _) => Scene::Riding,
        (Screen::Menu, _) => Scene::Menu,
    }
}

#[allow(clippy::too_many_arguments)]
fn direct(
    time: Res<Time>,
    screen: Res<bevy::state::state::State<Screen>>,
    setup: Res<Setup>,
    run: Option<Res<Run>>,
    feed: Res<Feed>,
    mut a: ResMut<Announcers>,
    mut w: ResMut<Watch>,
) {
    let dt = time.delta_secs();
    let sc = scene(*screen.get(), run.as_deref());
    let a = &mut *a;
    // On the web, the first thing they say waits for sound to start (the
    // first tap or key), so it is heard.
    if !crate::audio::ready() {
        return;
    }
    if !setup.radio.voices() {
        a.queue.clear();
        a.now = None;
        crate::audio::voice_stop();
        w.last = Some(sc);
        return;
    }
    if sc == Scene::Paused && w.last != Some(Scene::Paused) {
        a.queue.clear();
        a.now = None;
        crate::audio::voice_stop();
    }
    if let Some(run) = run.as_deref().filter(|_| *screen.get() == Screen::Ride) {
        let s = &run.session;
        if run.serial != w.serial {
            let again = w.serial != 0 && w.last.is_some_and(|l| l != Scene::Menu);
            w.serial = run.serial;
            w.place = s.place(0);
            w.new_place = w.place;
            w.held = 0.0;
            a.director.cue(Cue::Start {
                course: s.course.id,
                character: s.player().character,
                again,
            });
        }
        for ev in &feed.0 {
            let cue = match ev {
                SessionEvent::Go => Some(Cue::Go),
                SessionEvent::Rider(0, Event::Trick(t)) => Some(Cue::Trick {
                    points: t.points,
                    spin_deg: t.spin_deg,
                }),
                SessionEvent::Rider(0, Event::Land { landing: Landing::Sloppy, .. }) => Some(Cue::Sloppy),
                SessionEvent::Rider(0, Event::Crash { tree }) => Some(Cue::Crash { tree: *tree }),
                SessionEvent::Finish { who: 0, .. } => Some(Cue::Finish {
                    mode: s.mode,
                    won: s.mode == Mode::Race && s.place(0) == 1,
                }),
                SessionEvent::Finish { who, .. } if *who > 0 && s.player().finish.is_none() => Some(Cue::RivalDone),
                _ => None,
            };
            if let Some(c) = cue {
                a.director.cue(c);
            }
        }
        if s.state == State::Running && !run.paused && s.player().finish.is_none() {
            let me = &s.player().rider;
            // The lead, once a new place has held for a second.
            if s.mode == Mode::Race && s.time > 4.0 {
                let p = s.place(0);
                if p != w.new_place {
                    w.new_place = p;
                    w.held = 0.0;
                }
                w.held += dt;
                if w.held > 1.0 && w.new_place != w.place {
                    a.director.cue(if w.new_place < w.place { Cue::Lead } else { Cue::Behind });
                    w.place = w.new_place;
                }
            }
            // A long flight, once per jump.
            if me.airborne() {
                w.air += dt;
                if w.air > 1.6 && !w.air_said {
                    w.air_said = true;
                    a.director.cue(Cue::Air);
                }
            } else {
                w.air = 0.0;
                w.air_said = false;
            }
            // Flat out.
            if me.speed() > 27.0 {
                w.fast += dt;
                if w.fast > 1.5 {
                    w.fast = 0.0;
                    a.director.cue(Cue::Fast);
                }
            } else {
                w.fast = 0.0;
            }
        }
    }
    w.last = Some(sc);

    if let Some(say) = a.director.update(dt, sc) {
        if say.cut {
            a.queue.clear();
            a.now = None;
            crate::audio::voice_stop();
        }
        let lines: Vec<Line> = say
            .lines()
            .iter()
            .enumerate()
            .map(|(i, (who, text))| Line {
                clip: clip_id(say.topic, say.bit, i),
                who: *who,
                text,
            })
            .collect();
        crate::audio::voice_prepare(&lines.iter().map(|l| l.clip.clone()).collect::<Vec<_>>());
        a.queue.extend(lines);
        a.log.push((say.topic, say.bit));
    }
}

/// Says the queued lines one after another.
fn narrate(time: Res<Time>, mut a: ResMut<Announcers>) {
    let dt = time.delta_secs();
    let a = &mut *a;
    if let Some(now) = &mut a.now {
        now.age += dt;
        if now.len.is_none() {
            match crate::audio::voice_start(&now.line.clip) {
                crate::audio::Clip::Playing(secs) => now.len = Some(now.age + secs),
                crate::audio::Clip::Loading if now.age < CLIP_WAIT => {}
                _ => now.len = Some(now.age + Director::estimate(&[(now.line.who, now.line.text)])),
            }
        }
        if now.len.is_some_and(|l| now.age >= l + 0.15) {
            a.now = None;
        }
    }
    if a.now.is_none() {
        match a.queue.pop_front() {
            Some(line) => {
                a.now = Some(Saying {
                    line,
                    age: 0.0,
                    len: None,
                });
            }
            None => a.director.done(),
        }
    }
}

#[derive(Component)]
struct Caption;

/// The row the caption is centred in.
#[derive(Component)]
struct CaptionRow;

#[derive(Component)]
enum CaptionText {
    Who,
    Words,
}

fn spawn_captions(mut commands: Commands, fonts: Res<Fonts>) {
    let f = &*fonts;
    commands
        .spawn((
            CaptionRow,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                padding: UiRect::horizontal(Val::VMin(2.0)),
                bottom: Val::VMin(3.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            FocusPolicy::Pass,
            GlobalZIndex(8),
        ))
        .with_children(|row| {
            row.spawn((
                Caption,
                Node {
                    max_width: Val::Vw(62.0),
                    flex_shrink: 1.0,
                    padding: UiRect::axes(Val::VMin(1.8), Val::VMin(0.8)),
                    border_radius: BorderRadius::all(Val::VMin(1.2)),
                    column_gap: Val::VMin(1.2),
                    align_items: AlignItems::Baseline,
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.05, 0.13, 0.72)),
                FocusPolicy::Pass,
            ))
            .with_children(|c| {
                c.spawn((CaptionText::Who, ui::text(f, "", 3.3, ui::HOT, true)));
                c.spawn((CaptionText::Words, ui::text(f, "", 3.3, ui::WHITE, false)));
            });
        });
}

fn captions(
    a: Res<Announcers>,
    screen: Res<bevy::state::state::State<Screen>>,
    mut rows: Query<&mut Node, (With<CaptionRow>, Without<Caption>)>,
    mut boxes: Query<&mut Node, (With<Caption>, Without<CaptionRow>)>,
    mut texts: Query<(&CaptionText, &mut Text, &mut TextColor)>,
) {
    // In the menu, right of its column of settings and above its line of
    // keys; in a run, at the foot of the screen.
    let menu = *screen.get() == Screen::Menu;
    let (left, bottom) = if menu { (Val::VMin(77.0), Val::VMin(7.0)) } else { (Val::Px(0.0), Val::VMin(3.0)) };
    for mut n in &mut rows {
        if n.bottom != bottom || n.left != left {
            n.bottom = bottom;
            n.left = left;
        }
    }
    let line = a.now.as_ref().map(|n| &n.line);
    for mut n in &mut boxes {
        let d = if line.is_some() { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    let Some(line) = line else { return };
    for (kind, mut t, mut color) in &mut texts {
        let (s, c) = match kind {
            CaptionText::Who => (
                line.who.name().to_uppercase(),
                match line.who {
                    Who::Dani => ui::HOT,
                    Who::Wren => ui::ICE,
                },
            ),
            CaptionText::Words => (line.text.to_string(), ui::WHITE),
        };
        if t.0 != s {
            t.0 = s;
        }
        if color.0 != c {
            color.0 = c;
        }
    }
}
