//! The screens around a run: the title menu (mode, course and rider, with
//! the chosen rider standing at the course's start gate and the camera
//! circling), the pause panel and the results.
//!
//! Every panel is a column of buttons: arrows or the stick move the focus,
//! left and right change a setting, Enter / Space / A choose; a click or a
//! tap chooses directly.

use crate::camera::{CamFocus, CamMode};
use crate::hud::{PauseButton, despawn};
use crate::input::Controls;
use crate::ride::{Run, RideSet};
use crate::rider_view::{RiderPose, spawn_rider};
use crate::ui::{self, Fonts, clock, full_screen, ordinal, text, thousands};
use crate::world::Built;
use crate::radio::Radio;
use crate::{Screen, Setup};
use bevy::prelude::*;
use sb_sim::course::CourseId;
use sb_sim::race::{CHARACTERS, Mode, State};

pub fn plugin(app: &mut App) {
    app.init_resource::<Focus>()
        .add_systems(OnEnter(Screen::Menu), (spawn_menu, spawn_showcase))
        .add_systems(OnExit(Screen::Menu), (despawn::<Panel>, despawn::<Showcase>))
        .add_systems(OnExit(Screen::Ride), despawn::<Panel>)
        .add_systems(
            Update,
            (refresh_menu, showcase).run_if(in_state(Screen::Menu)),
        )
        .add_systems(Update, ride_panels.after(RideSet).run_if(in_state(Screen::Ride)))
        .add_systems(Update, (navigate, highlight).chain().after(ride_panels));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Mode(i32),
    Course(i32),
    Rider(i32),
    Assist,
    Radio(i32),
    Ride,
    Resume,
    Restart,
    Quit,
}

/// A panel on screen (one at a time).
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
pub(crate) enum Panel {
    Title,
    Pause,
    Results,
}

/// A focusable row: its index, and what choosing it does.
#[derive(Component)]
struct Row(usize, Action);

/// A clickable part of a row (the arrows beside a setting).
#[derive(Component)]
struct Click(Action);

#[derive(Component)]
enum Value {
    Mode,
    Course,
    Rider,
    Assist,
    Radio,
    Info,
    Stats,
}

#[derive(Resource, Default)]
pub(crate) struct Focus {
    pub(crate) index: usize,
    count: usize,
}

#[derive(Component)]
pub(crate) struct Showcase;

const MODES: [Mode; 3] = Mode::ALL;

/// The DROP IN row; the settings are the rows above it.
pub(crate) const RIDE_ROW: usize = 5;

fn cycle<T: Copy + PartialEq>(all: &[T], now: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == now).unwrap_or(0) as i32;
    all[(i + d).rem_euclid(all.len() as i32) as usize]
}

fn row_node() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        width: Val::Percent(100.0),
        padding: UiRect::axes(Val::VMin(2.0), Val::VMin(0.9)),
        border_radius: BorderRadius::all(Val::VMin(1.2)),
        border: UiRect::all(Val::VMin(0.35)),
        ..default()
    }
}

fn spawn_menu(mut commands: Commands, fonts: Res<Fonts>, mut focus: ResMut<Focus>) {
    *focus = Focus { index: RIDE_ROW, count: RIDE_ROW + 1 };
    let f = &*fonts;
    commands
        .spawn((Panel::Title, full_screen(), GlobalZIndex(5)))
        .with_children(|root| {
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::VMin(5.0),
                top: Val::VMin(3.0),
                width: Val::VMin(70.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::VMin(1.2),
                ..default()
            })
            .with_children(|col| {
                col.spawn((text(f, "SNOWBOARDING", 10.0, ui::WHITE, true), ui::shadow()));
                col.spawn((
                    text(f, "1080° STYLE  ·  RUST + BEVY + WASM", 2.8, ui::ICE, true),
                    ui::shadow(),
                    Node {
                        margin: UiRect::bottom(Val::VMin(1.0)),
                        ..default()
                    },
                ));
                for (i, (label, value, act)) in [
                    ("MODE", Value::Mode, Action::Mode(1)),
                    ("COURSE", Value::Course, Action::Course(1)),
                    ("RIDER", Value::Rider, Action::Rider(1)),
                    ("LANDING ASSIST", Value::Assist, Action::Assist),
                    ("RADIO", Value::Radio, Action::Radio(1)),
                ]
                .into_iter()
                .enumerate()
                {
                    let (prev, next) = match act {
                        Action::Mode(_) => (Action::Mode(-1), Action::Mode(1)),
                        Action::Course(_) => (Action::Course(-1), Action::Course(1)),
                        Action::Assist => (Action::Assist, Action::Assist),
                        Action::Radio(_) => (Action::Radio(-1), Action::Radio(1)),
                        _ => (Action::Rider(-1), Action::Rider(1)),
                    };
                    col.spawn((Row(i, act), Button, row_node(), BackgroundColor(ui::PANEL), BorderColor::all(Color::NONE)))
                        .with_children(|r| {
                            r.spawn(text(f, label, 2.8, ui::DIM, true));
                            r.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::VMin(1.5),
                                ..default()
                            })
                            .with_children(|v| {
                                v.spawn((Click(prev), Button, Node { padding: UiRect::horizontal(Val::VMin(1.5)), ..default() }))
                                    .with_children(|b| {
                                        b.spawn(text(f, "<", 4.4, ui::ICE, true));
                                    });
                                v.spawn((value, text(f, "", 4.4, ui::WHITE, true)));
                                v.spawn((Click(next), Button, Node { padding: UiRect::horizontal(Val::VMin(1.5)), ..default() }))
                                    .with_children(|b| {
                                        b.spawn(text(f, ">", 4.4, ui::ICE, true));
                                    });
                            });
                        });
                }
                col.spawn((
                    Row(RIDE_ROW, Action::Ride),
                    Button,
                    Node {
                        justify_content: JustifyContent::Center,
                        ..row_node()
                    },
                    BackgroundColor(Color::srgba(0.9, 0.3, 0.12, 0.85)),
                    BorderColor::all(Color::NONE),
                ))
                .with_children(|r| {
                    r.spawn(text(f, "DROP IN", 5.0, ui::WHITE, true));
                });
                col.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::VMin(2.0),
                        margin: UiRect::top(Val::VMin(0.5)),
                        padding: UiRect::all(Val::VMin(2.0)),
                        border_radius: BorderRadius::all(Val::VMin(1.2)),
                        ..default()
                    },
                    BackgroundColor(ui::PANEL),
                ))
                .with_children(|info| {
                    info.spawn((
                        Value::Info,
                        text(f, "", 2.7, ui::WHITE, false),
                        Node {
                            flex_shrink: 1.0,
                            ..default()
                        },
                    ));
                    info.spawn((Value::Stats, text(f, "", 2.3, ui::ICE, false), Node { flex_shrink: 0.0, ..default() }));
                });
            });
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::VMin(2.5),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|c| {
                c.spawn((
                    text(
                        f,
                        "ARROWS / STICK choose  ·  ENTER / A drop in  ·  keyboard, gamepad or touch",
                        2.4,
                        ui::WHITE,
                        false,
                    ),
                    ui::shadow(),
                ));
            });
        });
}

fn bar(v: f32) -> String {
    let n = ((v - 0.85) / 0.3 * 10.0).round().clamp(1.0, 10.0) as usize;
    format!("{}{}", "=".repeat(n), "-".repeat(10 - n))
}

fn refresh_menu(setup: Res<Setup>, mut q: Query<(&Value, &mut Text)>) {
    for (v, mut t) in &mut q {
        let s = match v {
            Value::Mode => {
                if setup.course == CourseId::Pipe {
                    Mode::TrickAttack.name().to_uppercase()
                } else {
                    setup.mode.name().to_uppercase()
                }
            }
            Value::Course => setup.course.name().to_uppercase(),
            Value::Assist => if setup.assist { "ON" } else { "OFF" }.into(),
            Value::Radio => setup.radio.label().into(),
            Value::Rider => {
                let c = &CHARACTERS[setup.character];
                format!("{} · {}", c.name.to_uppercase(), c.style.to_uppercase())
            }
            Value::Info => {
                let mode = if setup.course == CourseId::Pipe { Mode::TrickAttack } else { setup.mode };
                let diff = ["", "EASY", "MEDIUM", "EXPERT"][setup.course.difficulty() as usize];
                format!("{}\n{diff}  ·  {}", setup.course.blurb(), mode.blurb())
            }
            Value::Stats => {
                let st = CHARACTERS[setup.character].stats;
                format!(
                    "SPEED {}\nTURN {}\nJUMP {}\nSPIN {}\nBALANCE {}",
                    bar(st.speed),
                    bar(st.turn),
                    bar(st.jump),
                    bar(st.spin),
                    bar(st.balance)
                )
            }
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// The chosen rider at the start gate, the camera circling.
fn spawn_showcase(
    mut commands: Commands,
    setup: Res<Setup>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_rider(&mut commands, &mut meshes, &mut materials, setup.character, (Showcase, ShowcaseOf(setup.character)));
}

#[derive(Component)]
struct ShowcaseOf(usize);

#[allow(clippy::too_many_arguments)]
fn showcase(
    mut commands: Commands,
    time: Res<Time>,
    setup: Res<Setup>,
    built: Res<Built>,
    mut q: Query<(Entity, &ShowcaseOf, &mut RiderPose)>,
    mut focus: ResMut<CamFocus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, of, mut pose) in &mut q {
        if of.0 != setup.character {
            commands.entity(e).despawn();
            spawn_rider(&mut commands, &mut meshes, &mut materials, setup.character, (Showcase, ShowcaseOf(setup.character)));
            continue;
        }
        let Some(course) = &built.course else { continue };
        let s = course.start_s - 4.0;
        let g = course.ground_su(s, 0.0);
        let p = course.world(s, 0.0);
        *pose = RiderPose {
            pos: p,
            yaw: course.frame(s).theta,
            normal: g.normal,
            crouch: 0.2,
            t: time.elapsed_secs(),
            idle: true,
            ..default()
        };
        focus.pos = p;
        focus.s = s;
        focus.vel = Vec3::ZERO;
        focus.mode = CamMode::Orbit {
            radius: 4.6,
            height: 1.4,
            rate: 0.22,
        };
    }
}

fn spawn_panel(commands: &mut Commands, f: &Fonts, kind: Panel, title: &str, lines: &[String], buttons: &[(&str, Action)]) {
    commands
        .spawn((
            kind,
            GlobalZIndex(6),
            BackgroundColor(Color::srgba(0.0, 0.02, 0.08, 0.45)),
            Node {
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..full_screen()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::VMin(1.4),
                    padding: UiRect::all(Val::VMin(4.0)),
                    min_width: Val::VMin(56.0),
                    border_radius: BorderRadius::all(Val::VMin(2.0)),
                    ..default()
                },
                BackgroundColor(ui::PANEL),
            ))
            .with_children(|col| {
                col.spawn(text(f, title, 8.0, ui::GOLD, true));
                for l in lines {
                    col.spawn(text(f, l.as_str(), 3.4, ui::WHITE, false));
                }
                col.spawn(Node {
                    height: Val::VMin(1.5),
                    ..default()
                });
                for (i, (label, act)) in buttons.iter().enumerate() {
                    col.spawn((
                        Row(i, *act),
                        Button,
                        Node {
                            justify_content: JustifyContent::Center,
                            width: Val::VMin(44.0),
                            ..row_node()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|r| {
                        r.spawn(text(f, *label, 4.0, ui::WHITE, true));
                    });
                }
            });
        });
}

/// The pause and results panels, as the run's state asks.
#[allow(clippy::too_many_arguments)]
fn ride_panels(
    mut commands: Commands,
    fonts: Res<Fonts>,
    controls: Res<Controls>,
    run: Option<ResMut<Run>>,
    panels: Query<(Entity, &Panel)>,
    pause_btn: Query<&Interaction, (Changed<Interaction>, With<PauseButton>)>,
    mut focus: ResMut<Focus>,
) {
    let Some(mut run) = run else { return };
    let current = panels.iter().next().map(|(_, p)| *p);
    let done = run.session.state == State::Done;
    if !done
        && (controls.pause || pause_btn.iter().any(|i| *i == Interaction::Pressed))
        && current != Some(Panel::Results)
    {
        run.paused = !run.paused;
    }
    let want = if done && run.done_for > 0.4 {
        Some(Panel::Results)
    } else if run.paused {
        Some(Panel::Pause)
    } else {
        None
    };
    if want == current {
        return;
    }
    for (e, _) in &panels {
        commands.entity(e).despawn();
    }
    let f = &*fonts;
    match want {
        Some(Panel::Pause) => {
            *focus = Focus { index: 0, count: 3 };
            spawn_panel(
                &mut commands,
                f,
                Panel::Pause,
                "PAUSED",
                &[],
                &[("RESUME", Action::Resume), ("RESTART", Action::Restart), ("MENU", Action::Quit)],
            );
        }
        Some(Panel::Results) => {
            *focus = Focus { index: 0, count: 2 };
            let s = &run.session;
            let me = s.player();
            let mut lines = Vec::new();
            let title = match s.mode {
                Mode::Race => {
                    let place = s.place(0);
                    if place == 1 { "YOU WIN!".to_string() } else { format!("{} PLACE", ordinal(place)) }
                }
                Mode::TimeAttack => "FINISHED".into(),
                Mode::TrickAttack => "TRICK SCORE".into(),
            };
            if s.mode == Mode::TrickAttack {
                lines.push(format!("SCORE  {}", thousands(me.score)));
            }
            lines.push(format!("TIME  {}", me.finish.map_or("--".into(), clock)));
            if s.mode == Mode::Race
                && let Some(r) = s.entrants.get(1)
            {
                lines.push(format!(
                    "{}  {}",
                    r.name().to_uppercase(),
                    r.finish.map_or("still riding".into(), clock)
                ));
            }
            if let Some(b) = &me.best {
                lines.push(format!("BEST TRICK  {}  +{}", b.name.to_uppercase(), thousands(b.points)));
            }
            lines.push(format!("TRICKS {}   ·   CRASHES {}", me.tricks.len(), me.crashes));
            spawn_panel(&mut commands, f, Panel::Results, &title, &lines, &[("RIDE AGAIN", Action::Restart), ("MENU", Action::Quit)]);
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn navigate(
    controls: Res<Controls>,
    mut focus: ResMut<Focus>,
    rows: Query<(&Row, &Interaction), Changed<Interaction>>,
    all_rows: Query<&Row>,
    clicks: Query<(&Click, &Interaction), Changed<Interaction>>,
    panels: Query<&Panel>,
    mut setup: ResMut<Setup>,
    mut next: ResMut<NextState<Screen>>,
    run: Option<ResMut<Run>>,
) {
    if panels.is_empty() {
        return;
    }
    let mut act: Option<Action> = None;
    for (c, i) in &clicks {
        if *i == Interaction::Pressed {
            act = Some(c.0);
        }
    }
    if act.is_none() {
        for (r, i) in &rows {
            match i {
                Interaction::Pressed => {
                    focus.index = r.0;
                    act = Some(r.1);
                }
                Interaction::Hovered => focus.index = r.0,
                Interaction::None => {}
            }
        }
    }
    if focus.count > 0 {
        if controls.down {
            focus.index = (focus.index + 1) % focus.count;
        }
        if controls.up {
            focus.index = (focus.index + focus.count - 1) % focus.count;
        }
    }
    let focused = all_rows.iter().find(|r| r.0 == focus.index).map(|r| r.1);
    if act.is_none() && (controls.left || controls.right) {
        let d = if controls.left { -1 } else { 1 };
        act = match focused {
            Some(Action::Mode(_)) => Some(Action::Mode(d)),
            Some(Action::Course(_)) => Some(Action::Course(d)),
            Some(Action::Rider(_)) => Some(Action::Rider(d)),
            Some(Action::Assist) => Some(Action::Assist),
            Some(Action::Radio(_)) => Some(Action::Radio(d)),
            _ => None,
        };
    }
    let title = panels.iter().any(|p| *p == Panel::Title);
    if act.is_none() && controls.confirm {
        act = match focused {
            Some(Action::Mode(_) | Action::Course(_) | Action::Rider(_)) => Some(Action::Ride),
            Some(Action::Assist) => Some(Action::Assist),
            Some(Action::Radio(_)) => Some(Action::Radio(1)),
            other => other,
        };
    }
    // Esc is both pause and back: `ride_panels` has just toggled the pause
    // with it, so it must not resume the panel it opened this frame.
    if act.is_none() && controls.back && !controls.pause && !title {
        act = Some(Action::Resume);
    }
    let Some(act) = act else { return };
    match act {
        Action::Mode(d) => setup.mode = cycle(&MODES, setup.mode, d),
        Action::Course(d) => setup.course = cycle(&CourseId::ALL, setup.course, d),
        Action::Rider(d) => setup.character = (setup.character as i32 + d).rem_euclid(CHARACTERS.len() as i32) as usize,
        Action::Assist => {
            setup.assist = !setup.assist;
            setup.assist_chosen = true;
        }
        Action::Radio(d) => {
            setup.radio = cycle(&Radio::ALL, setup.radio, d);
            crate::audio::save_radio(setup.radio);
        }
        Action::Ride | Action::Restart => next.set(Screen::Ride),
        Action::Resume => {
            if let Some(mut r) = run
                && r.session.state != State::Done
            {
                r.paused = false;
            }
        }
        Action::Quit => next.set(Screen::Menu),
    }
}

fn highlight(focus: Res<Focus>, mut rows: Query<(&Row, &mut BorderColor)>) {
    for (r, mut b) in &mut rows {
        let c = if r.0 == focus.index { ui::ICE } else { Color::NONE };
        *b = BorderColor::all(c);
    }
}
