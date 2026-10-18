//! A run down a course: the countdown, the riders (the player, and a CPU
//! rival in a race), the clock, checkpoints, finishing and the results.
//! Stepped one fixed tick ([`TICK`]) at a time.

use crate::ai::Brain;
use crate::course::{Course, CourseId};
use crate::rider::{Event, Input, Phase, Rider, Stats};
use crate::tricks::Trick;

/// The simulation's fixed step.
pub const TICK: f32 = 1.0 / 120.0;
/// The countdown before the gate opens.
pub const COUNTDOWN: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// One on one against a CPU rider.
    Race,
    /// Alone against the clock.
    TimeAttack,
    /// Points for tricks on the way down.
    TrickAttack,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Race, Mode::TimeAttack, Mode::TrickAttack];
    pub fn name(self) -> &'static str {
        match self {
            Mode::Race => "Match Race",
            Mode::TimeAttack => "Time Attack",
            Mode::TrickAttack => "Trick Attack",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Mode::Race => "Beat a CPU rider to the bottom.",
            Mode::TimeAttack => "Just you and the clock.",
            Mode::TrickAttack => "Spin, flip and grab for points.",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Mode::Race => "race",
            Mode::TimeAttack => "time",
            Mode::TrickAttack => "trick",
        }
    }
    pub fn from_key(k: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.key() == k)
    }
}

/// A rider to pick.
pub struct Character {
    pub name: &'static str,
    pub style: &'static str,
    pub stats: Stats,
}

pub const CHARACTERS: [Character; 4] = [
    Character {
        name: "Kai",
        style: "All-rounder",
        stats: Stats {
            speed: 1.0,
            turn: 1.0,
            jump: 1.0,
            spin: 1.0,
            balance: 1.0,
        },
    },
    Character {
        name: "Mika",
        style: "Technical",
        stats: Stats {
            speed: 0.97,
            turn: 1.12,
            jump: 0.98,
            spin: 1.15,
            balance: 1.1,
        },
    },
    Character {
        name: "Rob",
        style: "Speed",
        stats: Stats {
            speed: 1.06,
            turn: 0.9,
            jump: 0.96,
            spin: 0.9,
            balance: 0.95,
        },
    },
    Character {
        name: "Jules",
        style: "Big air",
        stats: Stats {
            speed: 0.99,
            turn: 0.98,
            jump: 1.13,
            spin: 1.05,
            balance: 0.94,
        },
    },
];

pub struct Entrant {
    pub character: usize,
    pub rider: Rider,
    pub brain: Option<Brain>,
    pub finish: Option<f32>,
    pub score: u32,
    pub tricks: Vec<Trick>,
    pub best: Option<Trick>,
    pub crashes: u32,
    pub splits: Vec<f32>,
    base_speed: f32,
}

impl Entrant {
    pub fn name(&self) -> &'static str {
        CHARACTERS[self.character].name
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Countdown(f32),
    Running,
    /// The player is down; the results are final.
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent {
    Rider(usize, Event),
    Count(u32),
    Go,
    Checkpoint { who: usize, index: usize, time: f32 },
    Finish { who: usize, time: f32 },
    Done,
}

pub struct Session {
    pub course: Course,
    pub mode: Mode,
    pub entrants: Vec<Entrant>,
    pub time: f32,
    pub state: State,
    pub events: Vec<SessionEvent>,
    /// The player's rider is driven by a CPU brain too (demo and tests).
    pub autopilot: bool,
    done_in: Option<f32>,
}

impl Session {
    pub fn new(course: CourseId, mode: Mode, character: usize, autopilot: bool) -> Session {
        let course = Course::new(course);
        let mode = if course.pipe.is_some() {
            Mode::TrickAttack
        } else {
            mode
        };
        let character = character % CHARACTERS.len();
        let mut entrants = Vec::new();
        let race = mode == Mode::Race;
        let mk = |character: usize, u: f32, brain: Option<Brain>| {
            let stats = CHARACTERS[character].stats;
            Entrant {
                character,
                rider: Rider::new(&course, course.start_s, u, stats),
                brain,
                finish: None,
                score: 0,
                tricks: Vec::new(),
                best: None,
                crashes: 0,
                splits: Vec::new(),
                base_speed: stats.speed,
            }
        };
        let seed = course.seed;
        entrants.push(mk(
            character,
            if race { -3.5 } else { 0.0 },
            autopilot.then(|| Brain::new(0.85, seed ^ 0x51)),
        ));
        if race {
            let rival = (character + 1) % CHARACTERS.len();
            entrants.push(mk(rival, 3.5, Some(Brain::new(0.7, seed ^ 0x77))));
        }
        Session {
            course,
            mode,
            entrants,
            time: 0.0,
            state: State::Countdown(COUNTDOWN),
            events: Vec::new(),
            autopilot,
            done_in: None,
        }
    }

    pub fn player(&self) -> &Entrant {
        &self.entrants[0]
    }

    /// 1-based place of an entrant: finishers by time, then by distance.
    pub fn place(&self, who: usize) -> usize {
        let key = |e: &Entrant| match e.finish {
            Some(t) => (0, t),
            None => (1, -e.rider.s),
        };
        let mine = key(&self.entrants[who]);
        1 + self
            .entrants
            .iter()
            .enumerate()
            .filter(|(i, e)| *i != who && key(e) < mine)
            .count()
    }

    /// The player's progress down the course, 0 .. 1.
    pub fn progress(&self, who: usize) -> f32 {
        let c = &self.course;
        ((self.entrants[who].rider.s - c.start_s) / (c.finish_s - c.start_s)).clamp(0.0, 1.0)
    }

    pub fn step(&mut self, input: Input) {
        match self.state {
            State::Countdown(t) => {
                let next = t - TICK;
                if next.ceil() < t.ceil() && next > 0.0 {
                    self.events.push(SessionEvent::Count(next.ceil() as u32));
                }
                for e in &mut self.entrants {
                    e.rider.hold(&self.course);
                    e.rider.crouch = 0.3;
                }
                if next <= 0.0 {
                    self.state = State::Running;
                    self.events.push(SessionEvent::Go);
                    for e in &mut self.entrants {
                        e.rider.vel = self.course.frame(self.course.start_s).dir * 3.0;
                    }
                } else {
                    self.state = State::Countdown(next);
                }
                return;
            }
            State::Running | State::Done => {}
        }
        self.time += TICK;

        // The CPU rival keeps the race close.
        if self.mode == Mode::Race && self.entrants.len() > 1 {
            let lead = self.entrants[0].rider.s;
            for e in self.entrants.iter_mut().skip(1) {
                let gap = (lead - e.rider.s) / 300.0;
                e.rider.stats.speed = e.base_speed * (1.0 + gap.clamp(-0.07, 0.09));
            }
        }

        let course = &self.course;
        for (i, e) in self.entrants.iter_mut().enumerate() {
            let finished = e.finish.is_some();
            let mut inp = match (&mut e.brain, i) {
                (Some(b), _) => b.think(&e.rider, course, TICK),
                (None, _) => input,
            };
            if finished {
                inp = Input {
                    steer: 0.0,
                    pitch: -0.6,
                    jump: false,
                    grab: false,
                };
            }
            e.rider.step(course, inp, TICK);
            for ev in e.rider.events.drain(..) {
                match &ev {
                    Event::Trick(t) if !finished => {
                        e.score += t.points;
                        if e.best.as_ref().is_none_or(|b| t.points > b.points) {
                            e.best = Some(t.clone());
                        }
                        e.tricks.push(t.clone());
                    }
                    Event::Crash { .. } => e.crashes += 1,
                    _ => {}
                }
                self.events.push(SessionEvent::Rider(i, ev));
            }
            if !finished {
                while e.splits.len() < course.checkpoints.len()
                    && e.rider.s >= course.checkpoints[e.splits.len()]
                {
                    e.splits.push(self.time);
                    self.events.push(SessionEvent::Checkpoint {
                        who: i,
                        index: e.splits.len() - 1,
                        time: self.time,
                    });
                }
                if e.rider.s >= course.finish_s && e.rider.phase != Phase::Air {
                    e.finish = Some(self.time);
                    self.events.push(SessionEvent::Finish {
                        who: i,
                        time: self.time,
                    });
                }
            }
        }

        if self.state == State::Running {
            if self.entrants[0].finish.is_some() && self.done_in.is_none() {
                self.done_in = Some(2.5);
            }
            if let Some(t) = &mut self.done_in {
                *t -= TICK;
                if *t <= 0.0 {
                    self.state = State::Done;
                    self.events.push(SessionEvent::Done);
                }
            }
            if self.time > 900.0 {
                self.state = State::Done;
                self.events.push(SessionEvent::Done);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(course: CourseId, mode: Mode) -> Session {
        let mut s = Session::new(course, mode, 0, true);
        let mut steps = 0;
        while s.state != State::Done && steps < 120 * 400 {
            s.step(Input::default());
            steps += 1;
        }
        s
    }

    #[test]
    fn cpu_riders_finish_every_course() {
        for id in CourseId::ALL {
            let s = run(id, Mode::Race);
            let p = s.player();
            assert!(p.finish.is_some(), "{id:?}: stuck at s {} of {}", p.rider.s, s.course.finish_s);
            let t = p.finish.unwrap();
            assert!(t > 20.0 && t < 240.0, "{id:?}: {t} s");
        }
    }

    /// A seeded mashing of every control, as a player who has no idea
    /// gives them: held, released, flicked, all at once.
    struct Masher(u32, Input, u32);

    impl Masher {
        fn next(&mut self) -> Input {
            if self.2 == 0 {
                let mut r = || {
                    self.0 ^= self.0 << 13;
                    self.0 ^= self.0 >> 17;
                    self.0 ^= self.0 << 5;
                    self.0
                };
                let a = r();
                let b = r();
                let steer = [-1.0, -0.4, 0.0, 0.0, 0.3, 1.0][(a % 6) as usize];
                let pitch = [-1.0, 0.0, 0.0, 0.0, 0.5, 1.0][(a / 6 % 6) as usize];
                self.1 = Input {
                    steer,
                    pitch,
                    jump: b % 3 == 0,
                    grab: b % 5 == 0,
                };
                self.2 = 1 + b / 7 % 90;
            }
            self.2 -= 1;
            self.1
        }
    }

    fn check_sane(s: &Session, at: &str) {
        let c = &s.course;
        assert!(s.time.is_finite(), "{at}: time {}", s.time);
        for (i, e) in s.entrants.iter().enumerate() {
            let r = &e.rider;
            let bad = !(r.pos.is_finite()
                && r.vel.is_finite()
                && r.normal.is_finite()
                && r.yaw.is_finite()
                && r.pitch.is_finite()
                && r.s.is_finite()
                && r.u.is_finite());
            assert!(!bad, "{at}: rider {i} went non-finite: {r:?}");
            assert!(r.vel.length() < 120.0, "{at}: rider {i} at {} m/s", r.vel.length());
            let ground = c.height(r.s, r.u);
            assert!(r.pos.y > ground - 1.5, "{at}: rider {i} {} m under the snow", ground - r.pos.y);
            assert!(r.u.abs() < c.half_width(r.s) + 400.0, "{at}: rider {i} flew off sideways, u {}", r.u);
        }
    }

    /// Every course, mode and rider, mashed for two minutes of riding:
    /// nothing goes NaN, through the snow or off the mountain, and the
    /// session's events stay sane.
    #[test]
    fn mashing_never_breaks_the_physics() {
        let mut combos = 0;
        for (ci, id) in CourseId::ALL.into_iter().enumerate() {
            for (mi, mode) in Mode::ALL.into_iter().enumerate() {
                let character = (ci + mi) % CHARACTERS.len();
                let mut s = Session::new(id, mode, character, false);
                s.entrants[0].rider.assist = if mi % 2 == 0 { 0.6 } else { 0.0 };
                let mut m = Masher(0x9E37_79B9 ^ (ci as u32 * 31 + mi as u32), Input::default(), 0);
                for t in 0..120 * 120 {
                    s.step(m.next());
                    if t % 4 == 0 {
                        check_sane(&s, &format!("{id:?} {mode:?} rider {character} tick {t}"));
                    }
                    for ev in s.events.drain(..) {
                        if let SessionEvent::Rider(who, _) | SessionEvent::Finish { who, .. } = ev {
                            assert!(who < s.entrants.len());
                        }
                    }
                    if s.state == State::Done {
                        break;
                    }
                }
                combos += 1;
            }
        }
        assert_eq!(combos, CourseId::ALL.len() * Mode::ALL.len());
    }

    /// Any rider number, in or out of range, makes a session.
    #[test]
    fn any_character_number() {
        for c in [0, 3, 4, 9, usize::MAX] {
            let s = Session::new(CourseId::Timber, Mode::Race, c, false);
            assert!(s.entrants.iter().all(|e| e.character < CHARACTERS.len()));
        }
    }
}
