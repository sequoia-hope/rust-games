//! The run's HUD: the clock and splits, place or score, speed, the course
//! progress bar with each rider's marker, the countdown and big messages,
//! trick names with their points, the touch controls, and a hint of the
//! keys at the start.

use crate::Screen;
use crate::input::Controls;
use crate::ride::{Feed, Run};
use crate::rider_view::palette;
use crate::ui::{self, Fonts, clock, full_screen, ordinal, text, thousands};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use sb_sim::race::{Mode, SessionEvent, State};
use sb_sim::rider::Event;
use sb_sim::tricks::Landing;

pub fn plugin(app: &mut App) {
    app.init_resource::<HudState>()
        .add_systems(
            OnEnter(Screen::Ride),
            spawn_hud.after(crate::ride::start_run),
        )
        .add_systems(OnExit(Screen::Ride), despawn::<HudRoot>)
        .add_systems(
            Update,
            (events, update_hud, touch_overlay)
                .chain()
                .after(crate::ride::RideSet)
                .run_if(in_state(Screen::Ride)),
        );
}

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
enum HudText {
    Time,
    Split,
    Place,
    PlaceLabel,
    Speed,
    Msg,
    Trick,
    Points,
    Hint,
    Course,
}

#[derive(Component)]
struct Marker(usize);

/// The box behind the keys' hint, gone with it.
#[derive(Component)]
struct HintPill;

/// The place / score box, hidden in Time Attack.
#[derive(Component)]
struct PlaceBox;

const HUD_BG: Color = Color::srgba(0.02, 0.05, 0.13, 0.6);
/// Dark enough to read over sunlit snow, light enough to see through.
const TOUCH_BG: Color = Color::srgba(0.02, 0.05, 0.13, 0.45);

#[derive(Component, Clone, Copy)]
enum Touch {
    Rest,
    Stick,
    Knob,
    Jump,
    Grab,
}

/// The pause button (touch and mouse).
#[derive(Component)]
pub struct PauseButton;

#[derive(Resource, Default)]
struct HudState {
    msg: String,
    msg_color: Option<Color>,
    msg_t: f32,
    msg_len: f32,
    trick: String,
    points: String,
    trick_t: f32,
    split: String,
    split_t: f32,
    age: f32,
}

pub fn despawn<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn spawn_hud(
    mut commands: Commands,
    fonts: Res<Fonts>,
    mut st: ResMut<HudState>,
    run: Option<Res<Run>>,
) {
    *st = HudState::default();
    let course_name = run
        .as_ref()
        .map(|r| format!("{} · {}", r.session.course.id.name(), r.session.mode.name()))
        .unwrap_or_default();
    let riders: Vec<usize> = run
        .as_ref()
        .map(|r| r.session.entrants.iter().map(|e| e.character).collect())
        .unwrap_or_default();
    let f = &*fonts;
    commands
        .spawn((HudRoot, full_screen(), FocusPolicy::Pass, GlobalZIndex(1)))
        .with_children(|root| {
            // Top left: the clock.
            root.spawn((Node {
                position_type: PositionType::Absolute,
                left: Val::VMin(3.0),
                top: Val::VMin(2.5),
                padding: UiRect::axes(Val::VMin(2.0), Val::VMin(1.0)),
                border_radius: BorderRadius::all(Val::VMin(1.5)),
                flex_direction: FlexDirection::Column,
                ..default()
            }, BackgroundColor(HUD_BG)))
            .with_children(|c| {
                c.spawn(text(f, "TIME", 2.6, ui::DIM, false));
                c.spawn((HudText::Time, text(f, "0:00.00", 6.5, ui::WHITE, true)));
                c.spawn((HudText::Split, text(f, "", 3.2, ui::ICE, false)));
            });
            // Top right: place or score.
            root.spawn((Node {
                position_type: PositionType::Absolute,
                right: Val::VMin(12.0),
                top: Val::VMin(2.5),
                padding: UiRect::axes(Val::VMin(2.0), Val::VMin(1.0)),
                border_radius: BorderRadius::all(Val::VMin(1.5)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                ..default()
            }, BackgroundColor(HUD_BG), PlaceBox))
            .with_children(|c| {
                c.spawn((HudText::PlaceLabel, text(f, "PLACE", 2.6, ui::DIM, false)));
                c.spawn((HudText::Place, text(f, "", 6.5, ui::GOLD, true)));
            });
            // The pause button.
            root.spawn((
                PauseButton,
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::VMin(2.5),
                    top: Val::VMin(2.5),
                    width: Val::VMin(7.5),
                    height: Val::VMin(7.5),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::VMin(1.5)),
                    ..default()
                },
                BackgroundColor(ui::PANEL),
            ))
            .with_children(|c| {
                c.spawn(text(f, "II", 3.6, ui::WHITE, true));
            });
            // The course's name, during the countdown.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::VMin(3.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|c| {
                c.spawn((HudText::Course, text(f, course_name.to_uppercase(), 3.4, ui::WHITE, true)));
            });
            // The middle: messages and tricks.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(20.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::VMin(0.5),
                ..default()
            })
            .with_children(|c| {
                c.spawn((HudText::Msg, text(f, "", 13.0, ui::WHITE, true), ui::shadow()));
                c.spawn((HudText::Trick, text(f, "", 5.4, ui::WHITE, true), ui::shadow()));
                c.spawn((HudText::Points, text(f, "", 4.4, ui::GOLD, true), ui::shadow()));
            });
            // Bottom right: speed.
            root.spawn((Node {
                position_type: PositionType::Absolute,
                right: Val::VMin(3.0),
                bottom: Val::VMin(2.5),
                padding: UiRect::axes(Val::VMin(2.0), Val::VMin(1.0)),
                border_radius: BorderRadius::all(Val::VMin(1.5)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Baseline,
                column_gap: Val::VMin(1.0),
                ..default()
            }, BackgroundColor(HUD_BG)))
            .with_children(|c| {
                c.spawn((HudText::Speed, text(f, "0", 8.0, ui::WHITE, true)));
                c.spawn(text(f, "KM/H", 2.8, ui::DIM, false));
            });
            // Bottom left: the run down the mountain.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::VMin(3.0),
                    bottom: Val::VMin(4.0),
                    width: Val::VMin(36.0),
                    height: Val::VMin(1.0),
                    border_radius: BorderRadius::all(Val::VMin(0.5)),
                    ..default()
                },
                BackgroundColor(HUD_BG),
            ))
            .with_children(|c| {
                for (i, ch) in riders.iter().enumerate().rev() {
                    let col = palette(*ch)[0];
                    c.spawn((
                        Marker(i),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(0.0),
                            top: Val::VMin(-1.0),
                            width: Val::VMin(3.0),
                            height: Val::VMin(3.0),
                            margin: UiRect::left(Val::VMin(-1.5)),
                            border: UiRect::all(Val::VMin(0.4)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BorderColor::all(Color::WHITE),
                        BackgroundColor(Color::srgb(col[0], col[1], col[2])),
                    ));
                }
            });
            // The keys, at first.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::VMin(11.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|c| {
                c.spawn((
                    HintPill,
                    Node {
                        padding: UiRect::axes(Val::VMin(2.0), Val::VMin(0.8)),
                        border_radius: BorderRadius::all(Val::VMin(1.2)),
                        ..default()
                    },
                    BackgroundColor(HUD_BG),
                ))
                .with_children(|c| {
                c.spawn((
                    HudText::Hint,
                    text(
                        f,
                        "ARROWS steer · spin · flip    SPACE hold to crouch, release to jump    SHIFT grab    UP tuck · DOWN brake",
                        2.6,
                        ui::WHITE,
                        false,
                    ),
                    ui::shadow(),
                ));
                });
            });
            // Touch controls.
            for (kind, label) in [(Touch::Jump, "JUMP"), (Touch::Grab, "GRAB")] {
                let right = if matches!(kind, Touch::Jump) { 3.0 } else { 25.0 };
                root.spawn((
                    kind,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::VMin(right),
                        bottom: Val::VMin(if right > 10.0 { 17.0 } else { 23.0 }),
                        width: Val::VMin(19.0),
                        height: Val::VMin(19.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::VMin(0.5)),
                        border_radius: BorderRadius::MAX,
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.9)),
                    BackgroundColor(TOUCH_BG),
                    FocusPolicy::Pass,
                ))
                .with_children(|c| {
                    c.spawn((text(f, label, 4.4, ui::WHITE, true), ui::shadow()));
                });
            }
            // Where the stick lives, until a thumb is on it.
            root.spawn((
                Touch::Rest,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::VMin(8.0),
                    bottom: Val::VMin(12.0),
                    width: Val::VMin(24.0),
                    height: Val::VMin(24.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::VMin(0.5)),
                    border_radius: BorderRadius::MAX,
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.7)),
                BackgroundColor(TOUCH_BG),
                FocusPolicy::Pass,
            ))
            .with_children(|c| {
                c.spawn((text(f, "STEER", 3.4, ui::WHITE, true), ui::shadow()));
            });
            for kind in [Touch::Stick, Touch::Knob] {
                let size = if matches!(kind, Touch::Stick) { 24.0 } else { 10.0 };
                root.spawn((
                    kind,
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::VMin(size),
                        height: Val::VMin(size),
                        border: UiRect::all(Val::VMin(0.5)),
                        border_radius: BorderRadius::MAX,
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.9)),
                    BackgroundColor(if matches!(kind, Touch::Knob) { Color::srgba(0.55, 0.85, 1.0, 0.85) } else { TOUCH_BG }),
                    FocusPolicy::Pass,
                ));
            }
        });
}

fn events(time: Res<Time>, feed: Res<Feed>, run: Option<Res<Run>>, mut st: ResMut<HudState>) {
    let Some(run) = run else { return };
    let dt = if run.paused { 0.0 } else { time.delta_secs() };
    st.age += dt;
    st.msg_t += dt;
    st.trick_t += dt;
    st.split_t += dt;
    let s = &run.session;
    let say = |st: &mut HudState, m: &str, color: Option<Color>, len: f32| {
        st.msg = m.into();
        st.msg_color = color;
        st.msg_t = 0.0;
        st.msg_len = len;
    };
    for ev in &feed.0 {
        match ev {
            SessionEvent::Count(n) => say(&mut st, &n.to_string(), None, 0.9),
            SessionEvent::Go => say(&mut st, "GO!", Some(ui::GOLD), 1.0),
            SessionEvent::Rider(0, Event::Trick(t)) => {
                st.trick = t.name.to_uppercase();
                st.points = format!("+{}", thousands(t.points));
                st.trick_t = 0.0;
            }
            SessionEvent::Rider(
                0,
                Event::Land {
                    landing: Landing::Sloppy,
                    ..
                },
            ) => {
                if st.trick_t > 0.1 {
                    st.trick = "SLOPPY LANDING".into();
                    st.points = String::new();
                    st.trick_t = 0.0;
                }
            }
            SessionEvent::Rider(0, Event::Crash { tree }) => say(
                &mut st,
                if *tree { "TIMBER!" } else { "WIPEOUT!" },
                Some(ui::HOT),
                1.4,
            ),
            SessionEvent::Checkpoint {
                who: 0,
                index,
                time,
            } => {
                let mut line = format!("CHECKPOINT {}  {}", index + 1, clock(*time));
                if s.mode == Mode::Race
                    && let Some(rival) = s.entrants.get(1)
                    && let Some(t) = rival.splits.get(*index)
                {
                    let gap = time - t;
                    line += &format!("   {}{:.2}", if gap < 0.0 { "-" } else { "+" }, gap.abs());
                }
                st.split = line;
                st.split_t = 0.0;
            }
            SessionEvent::Finish { who: 0, .. } => say(&mut st, "FINISH!", Some(ui::GOLD), 2.4),
            SessionEvent::Finish { who, .. } if *who > 0 && s.entrants[0].finish.is_none() => {
                st.split = format!("{} FINISHED", s.entrants[*who].name().to_uppercase());
                st.split_t = 0.0;
            }
            _ => {}
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_hud(
    controls: Res<Controls>,
    run: Option<Res<Run>>,
    st: Res<HudState>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor, &mut UiTransform)>,
    mut markers: Query<(&Marker, &mut Node), (Without<PlaceBox>, Without<HintPill>)>,
    mut place_box: Query<&mut Node, (With<PlaceBox>, Without<HintPill>)>,
    mut pill: Query<&mut Node, (With<HintPill>, Without<PlaceBox>, Without<Marker>)>,
) {
    let hint_on = !controls.touch_seen && st.age < 10.0;
    for mut n in &mut pill {
        n.display = if hint_on {
            Display::Flex
        } else {
            Display::None
        };
    }
    let Some(run) = run else { return };
    let s = &run.session;
    let me = s.player();
    let done = s.state == State::Done;
    for (kind, mut t, mut color, mut transform) in &mut texts {
        let (value, c): (String, Option<Color>) = match kind {
            HudText::Time => (clock(me.finish.unwrap_or(s.time)), None),
            HudText::Split => (
                if st.split_t < 3.5 {
                    st.split.clone()
                } else {
                    String::new()
                },
                None,
            ),
            HudText::PlaceLabel => (
                if s.mode == Mode::TrickAttack {
                    "SCORE"
                } else if s.mode == Mode::Race {
                    "PLACE"
                } else {
                    ""
                }
                .into(),
                None,
            ),
            HudText::Place => match s.mode {
                Mode::Race => (ordinal(s.place(0)).into(), None),
                Mode::TrickAttack => (thousands(me.score), None),
                Mode::TimeAttack => (String::new(), None),
            },
            HudText::Speed => (format!("{:.0}", me.rider.speed() * 3.6), None),
            HudText::Msg => {
                if st.msg_t < st.msg_len && !done {
                    // The pop is a scale, never a font size: every size
                    // a text is set at gets glyph atlases of its own that
                    // are never freed (a leak of megabytes a run).
                    let k = (st.msg_t / 0.18).min(1.0);
                    let scale = Vec2::splat(1.5 - 0.5 * k);
                    if transform.scale != scale {
                        transform.scale = scale;
                    }
                    let a = ((st.msg_len - st.msg_t) / 0.3).clamp(0.0, 1.0);
                    (
                        st.msg.clone(),
                        Some(st.msg_color.unwrap_or(ui::WHITE).with_alpha(a)),
                    )
                } else {
                    (String::new(), None)
                }
            }
            HudText::Trick | HudText::Points => {
                let a = ((2.6 - st.trick_t) / 0.4).clamp(0.0, 1.0);
                if a > 0.0 && !done {
                    let v = if matches!(kind, HudText::Trick) {
                        st.trick.clone()
                    } else {
                        st.points.clone()
                    };
                    let base = if matches!(kind, HudText::Trick) {
                        ui::WHITE
                    } else {
                        ui::GOLD
                    };
                    (v, Some(base.with_alpha(a)))
                } else {
                    (String::new(), None)
                }
            }
            HudText::Hint => {
                let a = if controls.touch_seen {
                    0.0
                } else {
                    ((9.0 - st.age) / 1.0).clamp(0.0, 1.0)
                };
                (
                    if a > 0.0 && !done {
                        t.0.clone()
                    } else {
                        String::new()
                    },
                    Some(ui::WHITE.with_alpha(a)),
                )
            }
            HudText::Course => (
                if matches!(s.state, State::Countdown(_)) {
                    t.0.clone()
                } else {
                    String::new()
                },
                None,
            ),
        };
        // The hint and the course's name keep their text until they go.
        let once = matches!(kind, HudText::Hint | HudText::Course);
        if t.0 != value && (!once || value.is_empty()) {
            t.0 = value;
        }
        if let Some(c) = c {
            color.0 = c;
        }
    }
    for mut node in &mut place_box {
        node.display = if s.mode == Mode::TimeAttack {
            Display::None
        } else {
            Display::Flex
        };
    }
    for (m, mut node) in &mut markers {
        node.left = Val::Percent(100.0 * s.progress(m.0));
    }
}

fn touch_overlay(
    controls: Res<Controls>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q: Query<(&Touch, &mut Node, &mut BackgroundColor)>,
) {
    let show = controls.touch_seen;
    let vmin = window
        .single()
        .map_or(8.0, |w| w.width().min(w.height()) / 100.0);
    for (kind, mut node, mut bg) in &mut q {
        match kind {
            Touch::Jump | Touch::Grab => {
                node.display = if show { Display::Flex } else { Display::None };
                let on = if matches!(kind, Touch::Jump) {
                    controls.touch_jump
                } else {
                    controls.touch_grab
                };
                bg.0 = if on {
                    Color::srgba(0.55, 0.85, 1.0, 0.7)
                } else {
                    TOUCH_BG
                };
            }
            Touch::Rest => {
                node.display = if show && controls.stick.is_none() {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            Touch::Stick | Touch::Knob => {
                let size = if matches!(kind, Touch::Stick) {
                    24.0
                } else {
                    10.0
                } * vmin;
                if let Some((anchor, knob)) = controls.stick {
                    let at = if matches!(kind, Touch::Stick) {
                        anchor
                    } else {
                        knob
                    };
                    node.display = Display::Flex;
                    node.left = Val::Px(at.x - size / 2.0);
                    node.top = Val::Px(at.y - size / 2.0);
                } else {
                    node.display = Display::None;
                }
            }
        }
    }
}
