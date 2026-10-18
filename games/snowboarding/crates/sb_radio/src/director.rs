//! When the announcers speak, and what about. The client tells the
//! [`Director`] what happened ([`Cue`]) and, every frame, where the game is
//! ([`Scene`]) and how much time passed; the director answers with a bit
//! to say ([`Say`]) now and then, and is told when the talking is done.
//!
//! The rules, in short: the start of a run, the result and the crashes
//! always get a word; a big trick nearly always, a small one sometimes; the
//! lead changing hands, a long flight, a flat-out tuck now and then; a
//! quiet stretch in a run gets chatter, and so does the menu, slowly.
//! Nothing is said over the pause panel. What matters more cuts in on what
//! matters less; anything minor waits for a gap after the last word, so
//! they never talk over each other or over every little thing. A bit is
//! not said again until its topic's other bits have been (a shuffled bag
//! per topic). The result is said as the run went: the trees come up
//! only when one was hit, and a clean win only when nothing was crashed.
//!
//! Presentation, not simulation: its random draws are its own and never
//! touch the session's.

use crate::script::Bit;
use crate::topic;
use sb_sim::course::CourseId;
use sb_sim::race::{CHARACTERS, Mode};

/// Something the announcers can talk about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// A run is starting (its countdown). `again`: a restart.
    Start {
        course: CourseId,
        character: usize,
        again: bool,
    },
    Go,
    Trick {
        points: u32,
        spin_deg: u32,
    },
    Sloppy,
    Crash {
        tree: bool,
    },
    /// A long time in the air.
    Air,
    /// Flat out.
    Fast,
    /// The player took the lead in a race.
    Lead,
    /// The player lost it.
    Behind,
    /// The rival finished while the player rides on.
    RivalDone,
    /// The player finished. `won`: first in a race.
    Finish {
        mode: Mode,
        won: bool,
    },
}

/// Where the game is this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Menu,
    /// A run, before the results.
    Riding,
    Paused,
    /// The results are up.
    Results,
}

/// A bit to say: `topic`'s bit `bit` ([`crate::topic`]), its lines in turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Say {
    pub topic: &'static str,
    pub bit: usize,
    /// Cut off whatever is being said.
    pub cut: bool,
}

impl Say {
    pub fn lines(&self) -> Bit {
        topic(self.topic)[self.bit]
    }
}

/// How much a bit matters: it cuts in on less, and waits behind more.
fn weight(c: &Cue) -> u8 {
    match c {
        Cue::Finish { .. } => 5,
        Cue::Start { .. } => 4,
        Cue::Crash { .. } => 3,
        Cue::Trick { points, spin_deg } if big(*points, *spin_deg) => 3,
        Cue::Lead | Cue::Behind | Cue::RivalDone => 2,
        Cue::Go => 2,
        _ => 1,
    }
}

fn big(points: u32, spin_deg: u32) -> bool {
    points >= 1500 || spin_deg >= 720
}

/// Seconds of quiet after the last word before something minor is said.
const GAP: f32 = 2.5;
/// A queued bit goes stale after this long.
const STALE: f32 = 3.0;
/// Chatter after this much quiet in a run...
const IDLE: (f32, f32) = (14.0, 24.0);
/// ...and in the menu.
const MENU: (f32, f32) = (22.0, 40.0);
/// The menu's first word, once there.
const MENU_FIRST: f32 = 1.5;

pub struct Director {
    rng: u32,
    clock: f32,
    scene: Scene,
    /// When the bit being said ends (estimated until told).
    busy_until: f32,
    /// The weight of the bit being said.
    busy_weight: u8,
    /// When the last word ended (or will: the end of the bit being said).
    quiet_since: f32,
    /// A bit waiting for the talking to stop: (cue, weight, asked at).
    waiting: Option<(Cue, u8, f32)>,
    /// Shuffled bags, per topic: the bits not said yet.
    bags: Vec<(&'static str, Vec<usize>)>,
    /// Chatter when the clock passes this.
    next_chatter: f32,
    /// The menu has had its ident.
    greeted: bool,
    /// The last of each minor cue, to space them out.
    last_fast: f32,
    last_air: f32,
    last_lead: f32,
    /// The run so far, since its start: trees hit and other crashes.
    trees: u32,
    crashes: u32,
}

impl Director {
    pub fn new(seed: u32) -> Director {
        Director {
            rng: seed | 1,
            clock: 0.0,
            scene: Scene::Menu,
            busy_until: 0.0,
            busy_weight: 0,
            quiet_since: 0.0,
            waiting: None,
            bags: Vec::new(),
            next_chatter: MENU_FIRST,
            greeted: false,
            last_fast: -100.0,
            last_air: -100.0,
            last_lead: -100.0,
            trees: 0,
            crashes: 0,
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, (a, b): (f32, f32)) -> f32 {
        a + (b - a) * self.rand()
    }

    /// The next bit of `topic` from its bag, refilled and shuffled when
    /// empty (never the one just said first after a refill).
    fn draw(&mut self, topic_key: &'static str) -> Option<usize> {
        let n = topic(topic_key).len();
        if n == 0 {
            return None;
        }
        let i = match self.bags.iter().position(|(k, _)| *k == topic_key) {
            Some(i) => i,
            None => {
                self.bags.push((topic_key, Vec::new()));
                self.bags.len() - 1
            }
        };
        if self.bags[i].1.is_empty() {
            let last = self.bags[i].1.first().copied();
            let mut bag: Vec<usize> = (0..n).collect();
            for k in (1..n).rev() {
                let j = (self.rand() * (k + 1) as f32) as usize % (k + 1);
                bag.swap(k, j);
            }
            // The bag is drawn from the end.
            if n > 1 && bag.last().copied() == last {
                bag.swap(0, n - 1);
            }
            self.bags[i].1 = bag;
        }
        self.bags[i].1.pop()
    }

    fn talking(&self) -> bool {
        self.clock < self.busy_until
    }

    /// The talking ended (the client heard the last clip finish, or cut
    /// it): the director may speak again.
    pub fn done(&mut self) {
        if self.talking() {
            self.busy_until = self.clock;
            self.quiet_since = self.clock;
        }
    }

    /// The client knows how long the bit really lasts (its clips decoded).
    pub fn lasts(&mut self, secs: f32) {
        self.busy_until = self.clock + secs;
        self.quiet_since = self.busy_until;
    }

    /// How long a bit takes to say, before its clips are in: about
    /// fourteen characters a second, and a breath between lines.
    pub fn estimate(lines: &[(crate::Who, &str)]) -> f32 {
        lines
            .iter()
            .map(|(_, t)| 0.5 + t.len() as f32 / 14.0)
            .sum::<f32>()
    }

    /// Something happened.
    pub fn cue(&mut self, c: Cue) {
        if self.scene == Scene::Paused {
            return;
        }
        // The run's story, whether or not this cue is said.
        match c {
            Cue::Start { .. } => (self.trees, self.crashes) = (0, 0),
            Cue::Crash { tree: true } => self.trees += 1,
            Cue::Crash { tree: false } => self.crashes += 1,
            _ => {}
        }
        // Spacing for the minor ones.
        match c {
            Cue::Fast if self.clock - self.last_fast < 40.0 => return,
            Cue::Air if self.clock - self.last_air < 20.0 => return,
            Cue::Lead | Cue::Behind if self.clock - self.last_lead < 12.0 => return,
            _ => {}
        }
        match c {
            Cue::Fast => self.last_fast = self.clock,
            Cue::Air => self.last_air = self.clock,
            Cue::Lead | Cue::Behind => self.last_lead = self.clock,
            _ => {}
        }
        let w = weight(&c);
        if let Some((_, qw, _)) = self.waiting
            && qw > w
        {
            return;
        }
        self.waiting = Some((c, w, self.clock));
    }

    /// The topic and whether to say anything at all, for a cue.
    fn topic_for(&mut self, c: Cue) -> Option<&'static str> {
        Some(match c {
            Cue::Start {
                course,
                character,
                again,
            } => {
                if again && self.rand() < 0.5 {
                    "restart"
                } else if self.rand() < 0.35 {
                    match CHARACTERS[character % CHARACTERS.len()].name {
                        "Kai" => "kai",
                        "Mika" => "mika",
                        "Rob" => "rob",
                        _ => "jules",
                    }
                } else {
                    match course {
                        CourseId::Crystal => "crystal",
                        CourseId::Timber => "timber",
                        CourseId::Avalanche => "avalanche",
                        CourseId::Pipe => "pipe",
                    }
                }
            }
            Cue::Go => "go",
            Cue::Trick { points, spin_deg } => {
                if big(points, spin_deg) {
                    "big"
                } else if points >= 400 || self.rand() < 0.4 {
                    "trick"
                } else {
                    return None;
                }
            }
            Cue::Sloppy => {
                if self.rand() < 0.6 {
                    "sloppy"
                } else {
                    return None;
                }
            }
            Cue::Crash { tree: true } => "tree",
            Cue::Crash { tree: false } => "crash",
            Cue::Air => "air",
            Cue::Fast => "fast",
            Cue::Lead => "lead",
            Cue::Behind => "behind",
            Cue::RivalDone => "rival",
            Cue::Finish { mode, won } => match mode {
                Mode::Race if won && self.trees + self.crashes == 0 && self.rand() < 0.6 => {
                    "win-clean"
                }
                Mode::Race if won => "win",
                Mode::Race if self.trees > 0 && self.rand() < 0.75 => "lose-trees",
                Mode::Race => "lose",
                Mode::TimeAttack => "time",
                Mode::TrickAttack => "score",
            },
        })
    }

    fn say(&mut self, topic_key: &'static str, w: u8, cut: bool) -> Option<Say> {
        let bit = self.draw(topic_key)?;
        let s = Say {
            topic: topic_key,
            bit,
            cut,
        };
        self.busy_until = self.clock + Self::estimate(s.lines());
        self.quiet_since = self.busy_until;
        self.busy_weight = w;
        Some(s)
    }

    /// A frame: `dt` seconds passed and the game is at `scene`. Returns a
    /// bit to start saying now, if any.
    pub fn update(&mut self, dt: f32, scene: Scene) -> Option<Say> {
        self.clock += dt;
        if scene != self.scene {
            let was = self.scene;
            self.scene = scene;
            match scene {
                Scene::Paused => {
                    // A pause cuts them off (the client stops the clip)
                    // and forgets what was waiting.
                    self.waiting = None;
                    if self.talking() {
                        self.busy_until = self.clock;
                        self.quiet_since = self.clock;
                    }
                }
                Scene::Menu => {
                    self.waiting = None;
                    let first = if self.greeted { 8.0 } else { MENU_FIRST };
                    self.next_chatter = self.clock + first;
                }
                Scene::Riding if was != Scene::Paused => {
                    self.next_chatter = self.clock + self.between(IDLE);
                }
                _ => {}
            }
        }
        if scene == Scene::Paused {
            return None;
        }

        if let Some((c, w, at)) = self.waiting {
            if self.clock - at > STALE && w < 4 {
                self.waiting = None;
            } else {
                let cut_in = self.talking() && w >= 4 && w > self.busy_weight;
                let gap_ok = !self.talking() && (w >= 3 || self.clock - self.quiet_since >= GAP);
                if cut_in || gap_ok {
                    self.waiting = None;
                    let t = self.topic_for(c);
                    if let Some(t) = t {
                        let s = self.say(t, w, cut_in);
                        self.next_chatter =
                            self.next_chatter.max(self.busy_until + self.between(IDLE));
                        return s;
                    }
                }
            }
        }

        // Chatter in the quiet.
        if !self.talking() && self.waiting.is_none() && self.clock >= self.next_chatter {
            match scene {
                Scene::Menu => {
                    let t = if !self.greeted || self.rand() < 0.3 {
                        "ident"
                    } else {
                        "menu"
                    };
                    self.greeted = true;
                    let s = self.say(t, 0, false);
                    self.next_chatter = self.busy_until + self.between(MENU);
                    return s;
                }
                Scene::Riding => {
                    let s = self.say("idle", 0, false);
                    self.next_chatter = self.busy_until + self.between(IDLE);
                    return s;
                }
                _ => {}
            }
        }
        None
    }

    /// Is a bit being said (by the estimate, or as the client told it)?
    pub fn is_talking(&self) -> bool {
        self.talking()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// Runs `secs` of frames at `scene`, saying what is asked (and taking
    /// as long as estimated), collecting the topics.
    fn run(d: &mut Director, secs: f32, scene: Scene, said: &mut Vec<&'static str>) {
        for _ in 0..(secs / DT) as usize {
            if let Some(s) = d.update(DT, scene) {
                said.push(s.topic);
            }
        }
    }

    fn start() -> Cue {
        Cue::Start {
            course: CourseId::Crystal,
            character: 0,
            again: false,
        }
    }

    #[test]
    fn the_menu_opens_with_an_ident_then_chats_slowly() {
        let mut d = Director::new(1);
        let mut said = Vec::new();
        run(&mut d, 3.0, Scene::Menu, &mut said);
        assert_eq!(said, vec!["ident"]);
        run(&mut d, 120.0, Scene::Menu, &mut said);
        assert!(
            (3..=7).contains(&said.len()),
            "two minutes of menu: {said:?}"
        );
        assert!(said.iter().all(|t| *t == "ident" || *t == "menu"));
    }

    #[test]
    fn a_run_starts_with_an_intro_and_the_finish_always_gets_a_word() {
        for seed in 1..40 {
            let mut d = Director::new(seed);
            let mut said = Vec::new();
            run(&mut d, 0.1, Scene::Riding, &mut said);
            d.cue(start());
            run(&mut d, 0.1, Scene::Riding, &mut said);
            assert_eq!(said.len(), 1, "seed {seed}");
            assert!(
                ["crystal", "kai"].contains(&said[0]),
                "seed {seed}: the intro is the course's or the rider's: {said:?}"
            );
            // The finish comes while they are still talking: it cuts in.
            d.cue(Cue::Finish {
                mode: Mode::Race,
                won: true,
            });
            run(&mut d, 0.1, Scene::Riding, &mut said);
            assert!(
                ["win", "win-clean"].contains(said.last().unwrap()),
                "seed {seed}: {said:?}"
            );
        }
    }

    #[test]
    fn minor_things_wait_for_a_gap_and_go_stale() {
        let mut d = Director::new(5);
        let mut said = Vec::new();
        d.update(DT, Scene::Riding);
        d.cue(Cue::Crash { tree: false });
        run(&mut d, 0.1, Scene::Riding, &mut said);
        assert_eq!(said, vec!["crash"]);
        // A small thing during the crash line: dropped, not said late.
        d.cue(Cue::Trick {
            points: 900,
            spin_deg: 360,
        });
        run(&mut d, 6.0, Scene::Riding, &mut said);
        assert_eq!(
            said,
            vec!["crash"],
            "the trick went stale while they talked"
        );
        // After a gap, it is said.
        d.cue(Cue::Trick {
            points: 900,
            spin_deg: 360,
        });
        run(&mut d, 0.2, Scene::Riding, &mut said);
        assert_eq!(said, vec!["crash", "trick"]);
    }

    #[test]
    fn nothing_over_the_pause() {
        let mut d = Director::new(9);
        let mut said = Vec::new();
        d.update(DT, Scene::Riding);
        d.cue(start());
        run(&mut d, 0.1, Scene::Riding, &mut said);
        assert!(d.is_talking());
        d.update(DT, Scene::Paused);
        assert!(!d.is_talking(), "a pause cuts them off");
        d.cue(Cue::Crash { tree: true });
        run(&mut d, 60.0, Scene::Paused, &mut said);
        assert_eq!(said.len(), 1, "a minute paused, nothing said: {said:?}");
    }

    #[test]
    fn a_long_quiet_run_gets_chatter_and_no_bit_repeats_soon() {
        let mut d = Director::new(3);
        let mut said = Vec::new();
        let mut bits = Vec::new();
        d.update(DT, Scene::Riding);
        for _ in 0..(300.0 / DT) as usize {
            if let Some(s) = d.update(DT, Scene::Riding) {
                said.push(s.topic);
                bits.push(s.bit);
            }
        }
        assert!(said.iter().all(|t| *t == "idle"));
        assert!(
            (8..=20).contains(&said.len()),
            "five quiet minutes: {} bits",
            said.len()
        );
        let n = topic("idle").len();
        for w in bits.windows(n) {
            let mut seen = w.to_vec();
            seen.sort();
            seen.dedup();
            // Bags: any run of n draws spans at most two bags.
            assert!(seen.len() >= n / 2, "{bits:?}");
        }
        for w in bits.windows(2) {
            assert_ne!(w[0], w[1], "the same bit twice running: {bits:?}");
        }
    }

    /// What is said at the finish of a race, after `crashes`.
    fn result(seed: u32, crashes: &[Cue], won: bool) -> &'static str {
        let mut d = Director::new(seed);
        d.update(DT, Scene::Riding);
        d.cue(start());
        run(&mut d, 0.1, Scene::Riding, &mut Vec::new());
        for c in crashes {
            d.cue(*c);
            run(&mut d, 10.0, Scene::Riding, &mut Vec::new());
        }
        d.cue(Cue::Finish {
            mode: Mode::Race,
            won,
        });
        (0..30)
            .find_map(|_| d.update(DT, Scene::Riding))
            .expect("the result is called")
            .topic
    }

    #[test]
    fn the_result_is_said_as_the_run_went() {
        let tree = Cue::Crash { tree: true };
        let fall = Cue::Crash { tree: false };
        let mut said = std::collections::HashSet::new();
        for seed in 1..60 {
            let lost = result(seed, &[], false);
            assert_eq!(lost, "lose", "seed {seed}: no tree was hit");
            let lost = result(seed, &[fall], false);
            assert_eq!(lost, "lose", "seed {seed}: a fall, not a tree");
            said.insert(result(seed, &[tree], false));
            said.insert(result(seed, &[], true));
            let won = result(seed, &[fall], true);
            assert_eq!(won, "win", "seed {seed}: a win with a fall is not clean");
            let won = result(seed, &[tree], true);
            assert_eq!(won, "win", "seed {seed}: nor one with a tree");
        }
        for t in ["lose", "lose-trees", "win", "win-clean"] {
            assert!(said.contains(t), "{t} is said sometimes: {said:?}");
        }
        // A new run forgets the last one's trees.
        let mut d = Director::new(2);
        d.update(DT, Scene::Riding);
        d.cue(tree);
        d.cue(start());
        assert_eq!((d.trees, d.crashes), (0, 0));
    }

    #[test]
    fn only_the_tree_topics_mention_trees() {
        let trees = ["tree", "lose-trees", "timber", "menu", "ident"];
        for (k, bits) in crate::script::TOPICS {
            if trees.contains(k) {
                continue;
            }
            for b in *bits {
                for (_, t) in *b {
                    let t = t.to_lowercase();
                    assert!(
                        !["tree", "pine", "trunk", "timber"]
                            .iter()
                            .any(|w| t.contains(w)),
                        "{k} is said whether or not a tree was hit: {t}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_cue_has_something_to_say() {
        let cues = [
            start(),
            Cue::Start {
                course: CourseId::Pipe,
                character: 3,
                again: true,
            },
            Cue::Go,
            Cue::Trick {
                points: 3000,
                spin_deg: 1080,
            },
            Cue::Crash { tree: true },
            Cue::Crash { tree: false },
            Cue::Air,
            Cue::Fast,
            Cue::Lead,
            Cue::Behind,
            Cue::RivalDone,
            Cue::Finish {
                mode: Mode::Race,
                won: false,
            },
            Cue::Finish {
                mode: Mode::TimeAttack,
                won: false,
            },
            Cue::Finish {
                mode: Mode::TrickAttack,
                won: false,
            },
        ];
        for c in cues {
            let mut d = Director::new(11);
            d.update(DT, Scene::Riding);
            run(&mut d, 5.0, Scene::Riding, &mut Vec::new());
            d.cue(c);
            let s = (0..30).find_map(|_| d.update(DT, Scene::Riding));
            let s = s.unwrap_or_else(|| panic!("{c:?}: nothing said"));
            assert!(!s.lines().is_empty());
        }
    }
}
