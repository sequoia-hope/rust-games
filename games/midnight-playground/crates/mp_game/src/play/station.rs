//! The radio in the client (radio.md 7, DECISIONS D1151): which station the
//! setting means, what is playing on it (the same schedule the node runs,
//! computed from the wall clock for the pause screen and the HUD), the
//! race's energy, and the DJ's director: when a DJ says something, and what
//! (only in a race: the menus are music only).
//!
//! Presentation, not simulation: its random choices use their own
//! generator, never the simulation's streams, and each player hears their
//! own radio.

use mp_audio::game::GameAudio;
use mp_audio::stations::{STATIONS, level_station, station};
use music::radio::{Station, cue};

/// Seconds between two full DJ breaks at the least.
const BREAK_GAP: f64 = 95.0;
/// Shorter breather after a quick tune-in station ID so song seams still speak.
const IDENT_GAP: f64 = 40.0;
/// A break at a song change, this often.
const BREAK_CHANCE: f64 = 0.55;
/// An ident shortly after tuning in, this often.
const IDENT_CHANCE: f64 = 0.5;
/// Lines not said again until this many others have been.
const HISTORY: usize = 14;

/// The station a setting means for a level: `None` is the playlist.
pub fn resolve(setting: &str, level: &str) -> Option<usize> {
    let key = match setting {
        "playlist" => return None,
        "auto" => level_station(level),
        k => k,
    };
    station(key).map(|st| {
        STATIONS
            .iter()
            .position(|s| s.key == st.key)
            .expect("in STATIONS")
    })
}

/// The station `dir` steps along the dial from `setting` (+1 the next, -1
/// the one before), round the stations; the radio stays on (D1160). From
/// the playlist (the radio off) it is the first station either way.
pub fn step_setting(setting: &str, level: &str, dir: i32) -> String {
    let n = STATIONS.len() as i32;
    match resolve(setting, level) {
        None => STATIONS[0].key.to_owned(),
        Some(i) => STATIONS[(i as i32 + dir).rem_euclid(n) as usize]
            .key
            .to_owned(),
    }
}

/// What the radio plays with it switched on: `remembered` (the station last
/// chosen, or `auto`), never the playlist.
pub fn radio_on_setting(remembered: &str) -> String {
    if remembered == "playlist" || !crate::ui::store::station_ok(remembered) {
        "auto".to_owned()
    } else {
        remembered.to_owned()
    }
}

/// `"The Tide 88.1"`.
pub fn label(st: &Station) -> String {
    format!("{} {}", st.name, st.freq)
}

/// What the race is doing, for the energy and the DJ's silences.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scene {
    /// A race is on (not the menus).
    pub racing: bool,
    pub countdown: bool,
    pub results: bool,
    pub pursuit: bool,
    /// The player's speed, m/s.
    pub speed: f64,
    /// The police radio spoke this frame.
    pub police: bool,
}

/// The client's side of the radio.
#[derive(Debug)]
pub struct StationState {
    /// The station tuned, if any.
    pub station: Option<usize>,
    /// The second the cue was last computed at, and its slot.
    cue_sec: i64,
    slot: Option<(usize, u64, usize)>,
    /// The now-playing line (`♪ station · song · style`).
    pub text: String,
    /// A line for the HUD to flash (the station or a new song).
    pub flash: Option<String>,
    pub energy: f64,
    last_frame: Option<f64>,
    // The DJ.
    history: Vec<String>,
    last_break: f64,
    due: Option<f64>,
    due_ident: bool,
    police_at: f64,
    prefetched: Option<&'static str>,
    rng: u64,
}

impl Default for StationState {
    fn default() -> Self {
        StationState::new()
    }
}

impl StationState {
    pub fn new() -> StationState {
        StationState {
            station: None,
            cue_sec: i64::MIN,
            slot: None,
            text: String::new(),
            flash: None,
            energy: 0.7,
            last_frame: None,
            history: Vec::new(),
            last_break: f64::MIN,
            due: None,
            due_ident: false,
            police_at: f64::MIN,
            prefetched: None,
            rng: 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// A tune (or the playlist: `None`) at wall time `wall`.
    pub fn tuned(&mut self, station: Option<usize>, wall: f64) {
        if station != self.station {
            self.station = station;
            self.slot = None;
            self.cue_sec = i64::MIN;
            self.text.clear();
            self.rng ^= (wall * 1000.0) as u64;
            match station {
                Some(i) => {
                    self.flash = Some(label(&STATIONS[i]));
                    // An ident a couple of seconds in, sometimes.
                    let want = self.chance(IDENT_CHANCE);
                    self.due = want.then_some(wall + 2.0);
                    self.due_ident = want;
                    self.refresh(wall);
                    self.flash = Some(self.text.clone());
                }
                None => {
                    self.flash = Some("Playlist".into());
                    self.due = None;
                    self.due_ident = false;
                }
            }
        }
    }

    /// A draw in 0..1 (an xorshift of its own; never the simulation's).
    fn next(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }

    fn chance(&mut self, p: f64) -> bool {
        self.next() < p
    }

    /// Recomputes the cue at most once a second; true when the song changed.
    fn refresh(&mut self, wall: f64) -> bool {
        let Some(i) = self.station else { return false };
        let sec = wall.floor() as i64;
        if sec == self.cue_sec {
            return false;
        }
        self.cue_sec = sec;
        let st = &STATIONS[i];
        let c = cue(st, wall);
        let slot = Some((i, c.block, c.slot));
        let changed = self.slot.is_some() && slot != self.slot;
        self.slot = slot;
        let title = c
            .piece
            .title()
            .or_else(|| c.piece.track().map(|t| t.id.clone()))
            .unwrap_or_default();
        let style = c.piece.style().unwrap_or_default();
        self.text = format!("{} · {title} · {style}", label(st));
        changed
    }

    /// Every frame: the energy, the now-playing line, the DJ.
    pub fn frame(&mut self, audio: &mut GameAudio, wall: f64, scene: Scene) {
        let dt = self
            .last_frame
            .map_or(0.016, |t| (wall - t).clamp(0.0, 0.5));
        self.last_frame = Some(wall);
        if scene.police {
            self.police_at = wall;
        }
        // Energy: the mix follows the race (sound.md 3.4).
        let target = if !scene.racing {
            0.7
        } else if scene.results {
            0.35
        } else if scene.countdown {
            0.5
        } else if scene.pursuit {
            0.95
        } else {
            0.6 + 0.3 * (scene.speed / 45.0).clamp(0.0, 1.0)
        };
        let e = self.energy + (target - self.energy) * dt.min(1.0);
        if (e - self.energy).abs() > 0.02 || (target - self.energy).abs() < 0.02 && e != self.energy
        {
            self.energy = e;
            audio.set_energy(e);
        }
        let Some(i) = self.station else { return };
        if audio.station() != Some(i) {
            // Not tuned (yet, or no AudioWorklet): nothing to say.
            return;
        }
        if self.refresh(wall) {
            self.flash = Some(self.text.clone());
            if self.chance(BREAK_CHANCE) {
                self.due = Some(wall);
                self.due_ident = false;
            }
        }
        let st = &STATIONS[i];
        let Some(dj) = st.dj.as_deref() else { return };
        self.prefetch(audio, dj);
        if !scene.racing {
            // The menus are music only: a break due now is dropped, not
            // kept for the race (the owner, 2026-10-09).
            self.due = None;
            self.due_ident = false;
            return;
        }
        if let Some(due) = self.due
            && wall >= due
        {
            let quiet = !scene.countdown && wall - self.police_at > 4.0;
            if !quiet {
                return; // keep it pending
            }
            let ident_only = self.due_ident;
            self.due = None;
            self.due_ident = false;
            if wall - self.last_break >= BREAK_GAP {
                self.say(audio, dj, wall, ident_only);
            }
        }
    }

    /// Warm the DJ's short station idents ahead of time; song-seam chatter
    /// fetches lazily per clip via `DjVoice::buffer`.
    fn prefetch(&mut self, audio: &mut GameAudio, dj: &'static str) {
        if self.prefetched == Some(dj) {
            return;
        }
        let Some(voice) = audio.dj() else { return };
        let Some(Ok(index)) = voice.index().result() else {
            return;
        };
        let ids: Vec<String> = index
            .keys()
            .filter(|id| topic_of(id, dj) == Some("ident"))
            .cloned()
            .collect();
        let _ = voice.prefetch(&ids);
        self.prefetched = Some(dj);
    }

    fn pick_topic(&mut self, wall: f64, ident_only: bool) -> &'static str {
        if ident_only {
            return "ident";
        }
        let local_hour = ((wall as i64 - 8 * 3600).rem_euclid(86_400) / 3600) as u32;
        let mut weights: [(&'static str, f64); 14] = [
            ("music", 0.23),
            ("ident", 0.09),
            ("places", 0.11),
            ("scene", 0.11),
            ("weather", 0.08),
            ("callers", 0.07),
            ("banter", 0.06),
            ("offair", 0.06),
            ("ads", 0.05),
            ("raves", 0.04),
            ("tired", 0.03),
            ("camping", 0.03),
            ("morning", 0.0),
            ("night", 0.0),
        ];
        if (5..10).contains(&local_hour) {
            weights[12].1 = 0.10;
        } else if !(10..20).contains(&local_hour) {
            weights[13].1 = 0.12;
        }
        let total: f64 = weights.iter().map(|(_, w)| *w).sum();
        let mut r = self.next() * total;
        for &(topic, w) in &weights {
            if w <= 0.0 {
                continue;
            }
            if r < w {
                return topic;
            }
            r -= w;
        }
        "music"
    }

    fn pick_clip(
        &mut self,
        index: &mp_audio::dj::Index,
        dj: &str,
        wall: f64,
        ident_only: bool,
    ) -> Option<(String, u32)> {
        let wanted = self.pick_topic(wall, ident_only);
        let local_hour = ((wall as i64 - 8 * 3600).rem_euclid(86_400) / 3600) as u32;
        let allowed = |t: &str| match t {
            // `player` clips are filled-in car/road examples, not generic chatter.
            "player" => false,
            "morning" => (5..10).contains(&local_hour),
            "night" => !(10..20).contains(&local_hour),
            _ => true,
        };
        let mut ids: Vec<(&String, u32)> = index
            .iter()
            .filter(|(id, _)| topic_of(id, dj) == Some(wanted))
            .filter(|(id, _)| !self.history.contains(id))
            .map(|(id, n)| (id, *n))
            .collect();
        if ids.is_empty() && !ident_only {
            ids = index
                .iter()
                .filter(|(id, _)| topic_of(id, dj).is_some_and(allowed))
                .filter(|(id, _)| !self.history.contains(id))
                .map(|(id, n)| (id, *n))
                .collect();
        }
        ids.sort();
        if ids.is_empty() {
            return None;
        }
        let k = (self.next() * ids.len() as f64) as usize;
        let (id, takes) = ids[k.min(ids.len() - 1)];
        let take = 1 + (self.next() * takes.max(1) as f64) as u32;
        let id = id.clone();
        self.history.push(id.clone());
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
        Some((id, take.min(takes.max(1))))
    }

    /// A DJ break: station ident on tune-in, or a full contextual topic on song seams.
    fn say(&mut self, audio: &mut GameAudio, dj: &'static str, wall: f64, ident_only: bool) {
        let Some(voice) = audio.dj() else { return };
        let Some(Ok(index)) = voice.index().result() else {
            return;
        };
        let Some((id, take)) = self.pick_clip(&index, dj, wall, ident_only) else {
            return;
        };
        self.last_break = if ident_only {
            wall - BREAK_GAP + IDENT_GAP
        } else {
            wall
        };
        audio.dj_say(&id, take);
    }
}

/// The topic of a clip id `<dj>-<topic>-<n>` when it is `dj`'s.
fn topic_of<'a>(id: &'a str, dj: &str) -> Option<&'a str> {
    let rest = id.strip_prefix(dj)?.strip_prefix('-')?;
    let (topic, n) = rest.rsplit_once('-')?;
    n.parse::<u32>().ok()?;
    Some(topic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_resolve_and_step_round_the_dial() {
        assert_eq!(resolve("playlist", "coast"), None);
        assert_eq!(resolve("auto", "coast"), Some(0));
        assert_eq!(resolve("auto", "sierra"), Some(1));
        assert_eq!(resolve("pacifico", "coast"), Some(2));
        assert_eq!(resolve("auto", "desert"), Some(3));
        assert_eq!(resolve("auto", "streets"), Some(4));
        assert_eq!(resolve("auto", "cruise"), Some(4));
        assert_eq!(resolve("auto", "redwoods"), Some(5));
        assert_eq!(resolve("nope", "coast"), None);
        assert_eq!(step_setting("playlist", "coast", 1), "tide");
        assert_eq!(step_setting("tide", "coast", 1), "ridgeline");
        assert_eq!(step_setting("auto", "sierra", 1), "pacifico");
        assert_eq!(step_setting("pacifico", "coast", 1), "roadhouse");
        assert_eq!(step_setting("roadhouse", "coast", 1), "loft");
        assert_eq!(step_setting("loft", "coast", 1), "nocturne");
        // Round the stations, never off.
        assert_eq!(step_setting("nocturne", "coast", 1), "tide");
        assert_eq!(step_setting("tide", "coast", -1), "nocturne");
        assert_eq!(radio_on_setting("playlist"), "auto");
        assert_eq!(radio_on_setting("ridgeline"), "ridgeline");
        assert_eq!(radio_on_setting("kzzz"), "auto");
    }

    #[test]
    fn clip_ids_parse() {
        assert_eq!(topic_of("marisol-ident-3", "marisol"), Some("ident"));
        assert_eq!(topic_of("kit-music-10", "kit"), Some("music"));
        assert_eq!(topic_of("kit-music-10", "marisol"), None);
        assert_eq!(topic_of("marisol-weather", "marisol"), None);
    }

    #[test]
    fn a_tune_flashes_the_station_and_refreshes_once_a_second() {
        let mut s = StationState::new();
        let wall = music::radio::EPOCH + 1000.0;
        s.tuned(Some(0), wall);
        assert!(s.flash.take().unwrap().starts_with("The Tide 88.1 · "));
        assert!(s.text.contains(" · "));
        assert!(!s.refresh(wall + 0.5), "same second, no recompute");
        s.tuned(None, wall);
        assert_eq!(s.flash.take().as_deref(), Some("Playlist"));
        assert_eq!(s.station, None);
    }

    #[test]
    fn dj_director_rotates_contextual_topics_without_player_templates_or_repeats() {
        let mut index = mp_audio::dj::Index::new();
        for topic in [
            "ident", "music", "morning", "weather", "places", "scene", "player", "raves", "tired",
            "camping", "offair", "callers", "ads", "banter", "night",
        ] {
            for k in 1..=4 {
                index.insert(format!("marisol-{topic}-{k}"), 1);
            }
        }
        let mut s = StationState::new();
        // 01:00 local Pacific (09:00 UTC): night window, never morning or player.
        let night_wall = 9.0 * 3600.0;
        let (first_id, _) = s.pick_clip(&index, "marisol", night_wall, true).unwrap();
        assert_eq!(topic_of(&first_id, "marisol"), Some("ident"));

        let mut topics = std::collections::BTreeSet::new();
        let mut recent = std::collections::VecDeque::new();
        for _ in 0..80 {
            let (id, take) = s.pick_clip(&index, "marisol", night_wall, false).unwrap();
            assert_eq!(take, 1);
            let t = topic_of(&id, "marisol").unwrap().to_owned();
            assert_ne!(t, "player", "template player lines must not fire randomly");
            assert_ne!(t, "morning", "morning lines must not fire at 1am");
            assert!(!recent.contains(&id), "repeated clip {id} within history");
            recent.push_back(id);
            if recent.len() > HISTORY {
                recent.pop_front();
            }
            topics.insert(t);
        }
        assert!(
            topics.len() >= 8 && topics.contains("night") && topics.contains("places"),
            "expected broad topic rotation, got {topics:?}"
        );
    }
}
