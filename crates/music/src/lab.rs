//! The Music Lab on this engine: what the lab page's audio thread runs
//! (`apps/music-lab/`, through [`crate::wasm`]'s `music_lab_*` exports). A
//! [`Session`] is one [`Engine`] driven by the page's messages, the same
//! ones the JS lab's worklet took (`tools/music-lab/worklet.js` in
//! Midnight Playground, now frozen), as JSON:
//!
//! | `type` | fields | does |
//! |---|---|---|
//! | `track` | `genre`, `seed`, `dejavu?`, `kit?` | makes the genre's track and loads it (every part with its lab patch) |
//! | `play` | `bar?`, `delay?` (s) | starts at a bar |
//! | `stop` | | stops |
//! | `patch` | `part`, `lab` | replaces a part's patch members (live knob edits) |
//! | `kit` | `name`, `tweak?` (`{lane: {param: value}}`) | the drum machine and its per-lane edits |
//! | `audition` | `lab` | the keyboard's instrument |
//! | `on` / `off` | `midi`, `vel?`, `dur?` (s), `accent?`, `glideFrom?` | keyboard notes |
//! | `hit` | `lane`, `vel?` | a drum pad |
//! | `energy` | `value` | 0..1, the game's intensity |
//! | `mix` | any of `glue`, `tape`, `reverb`, `revSize`, `revTone`, `master` | the bus |
//! | `mute` / `solo` | `names` / `name` | per part or drum lane (`drums`: all of them) |
//!
//! The song is made here from its genre and seed, not sent: the page makes
//! the same track with [`generate`] to show it, and both are the grammar's
//! output, so what the lab plays is what a game's radio plays.

use crate::engine::{Engine, EngineEvent, Mix};
use crate::genres::GENRES;
use crate::instruments::TriggerOpt;
use crate::json::{Val, canon, parse};
use crate::track::{Lab, Track};

/// A genre's track for `seed` (`dejavu`: how often a block replays its core,
/// the grammar's default when `None`); `None` for an unknown genre.
pub fn generate(genre: &str, seed: u32, dejavu: Option<f64>) -> Option<Track> {
    GENRES
        .iter()
        .find(|g| g.key == genre)
        .map(|g| (g.make)(seed, dejavu))
}

/// The lab's tables as JSON, for the page: `{genres: [[key, name]],
/// patches: [[name, lab]], kits: [[name, [[lane, voice]]]], lanes: [..]}`
/// (pairs, in the tables' order).
pub fn tables_json() -> String {
    let pairs = |items: Vec<(String, Val)>| {
        Val::Arr(
            items
                .into_iter()
                .map(|(k, v)| Val::Arr(vec![Val::Str(k), v]))
                .collect(),
        )
    };
    let v = Val::obj(vec![
        (
            "genres",
            Some(pairs(
                GENRES
                    .iter()
                    .map(|g| (g.key.to_owned(), Val::Str(g.name.to_owned())))
                    .collect(),
            )),
        ),
        (
            "patches",
            Some(pairs(
                crate::patches::all()
                    .iter()
                    .map(|(n, l)| ((*n).to_owned(), l.to_val()))
                    .collect(),
            )),
        ),
        (
            "kits",
            Some(pairs(
                crate::kits::KITS
                    .iter()
                    .map(|(n, lanes)| {
                        let lanes = lanes
                            .iter()
                            .map(|(l, k)| ((*l).to_owned(), k.to_val()))
                            .collect();
                        ((*n).to_owned(), pairs(lanes))
                    })
                    .collect(),
            )),
        ),
        (
            "lanes",
            Some(Val::Arr(
                crate::seq::DRUM_LANES
                    .iter()
                    .map(|l| Val::Str((*l).to_owned()))
                    .collect(),
            )),
        ),
    ]);
    canon(&v)
}

/// One lab node: an engine and the page's messages.
pub struct Session {
    engine: Engine,
    sr: f64,
    /// Section and end events since the last [`Session::poll`].
    events: Vec<Val>,
}

impl Session {
    pub fn new(seed: u32, sr: f64) -> Session {
        Session {
            engine: Engine::new(seed, sr),
            sr,
            events: Vec::new(),
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// One message from the page (the table above), as JSON text.
    pub fn message(&mut self, json: &str) -> Result<(), String> {
        let d = parse(json)?;
        let s = |k: &str| d.get(k).and_then(Val::as_str);
        let n = |k: &str| d.get(k).and_then(Val::as_f64);
        let e = &mut self.engine;
        match s("type").ok_or("a message needs a type")? {
            "track" => {
                let genre = s("genre").ok_or("track: no genre")?;
                let seed = n("seed").unwrap_or(1.0).max(1.0) as u32;
                let t = generate(genre, seed, n("dejavu"))
                    .ok_or_else(|| format!("track: no genre {genre}"))?;
                e.set_track(&t);
                if let Some(kit) = s("kit") {
                    e.set_kit(kit);
                }
                // A new song's sequencer starts at energy 1: keep the page's.
                let energy = e.energy();
                e.set_energy(energy);
            }
            "play" => {
                let bar = n("bar").unwrap_or(0.0).max(0.0) as u32;
                e.play(bar, n("delay").unwrap_or(0.02));
            }
            "stop" => e.stop(),
            "patch" => {
                let part = s("part").ok_or("patch: no part")?;
                let lab = d.get("lab").ok_or("patch: no lab")?;
                e.set_patch(part, Lab::from_val(lab));
            }
            "kit" => {
                if let Some(name) = s("name") {
                    e.set_kit(name);
                }
                let tweak = match d.get("tweak") {
                    Some(Val::Obj(lanes)) => lanes
                        .iter()
                        .map(|(lane, ps)| {
                            let ps = match ps {
                                Val::Obj(ps) => ps
                                    .iter()
                                    .filter_map(|(k, v)| v.as_f64().map(|v| (k.clone(), v)))
                                    .collect(),
                                _ => Vec::new(),
                            };
                            (lane.clone(), ps)
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                e.set_kit_tweak(tweak);
            }
            "audition" => {
                let lab = d.get("lab").ok_or("audition: no lab")?;
                e.set_audition(Lab::from_val(lab));
            }
            "on" => {
                let midi = n("midi").ok_or("on: no midi")?;
                let opt = TriggerOpt {
                    glide_from: n("glideFrom"),
                    accent: matches!(d.get("accent"), Some(Val::Bool(true))),
                };
                let dur = n("dur").map(|s| (s * self.sr + 0.5).floor());
                e.note_on(midi, n("vel").unwrap_or(0.9), dur, &opt);
            }
            "off" => e.note_off(n("midi").ok_or("off: no midi")?),
            "hit" => e.hit(s("lane").ok_or("hit: no lane")?, n("vel").unwrap_or(1.0)),
            "energy" => e.set_energy(n("value").unwrap_or(1.0).clamp(0.0, 1.0)),
            "mix" => {
                let m = e.mix();
                e.set_mix(Mix {
                    glue: n("glue").unwrap_or(m.glue),
                    tape: n("tape").unwrap_or(m.tape),
                    reverb: n("reverb").unwrap_or(m.reverb),
                    rev_size: n("revSize").unwrap_or(m.rev_size),
                    rev_tone: n("revTone").unwrap_or(m.rev_tone),
                    master: n("master").unwrap_or(m.master),
                });
            }
            "mute" => {
                let names = d
                    .get("names")
                    .and_then(Val::as_arr)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect();
                if let Some(seq) = e.seq_mut() {
                    seq.mute = names;
                }
            }
            "solo" => {
                let name = s("name").filter(|n| !n.is_empty()).map(str::to_owned);
                if let Some(seq) = e.seq_mut() {
                    seq.solo = name;
                }
            }
            other => return Err(format!("unknown message type {other}")),
        }
        Ok(())
    }

    /// Renders `l.len()` frames.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.engine.process(l, r);
        for ev in self.engine.take_events() {
            self.events.push(match ev {
                EngineEvent::Section(i) => Val::obj(vec![
                    ("type", Val::str("section")),
                    ("sec", Val::num(i as f64)),
                ]),
                EngineEvent::End => Val::obj(vec![("type", Val::str("end"))]),
            });
        }
    }

    /// What the page shows, as JSON: `{pos: {sec, bar, step, barIndex,
    /// bars, playing} | null, gr, events: [..]}`; `gr` is the glue's mean
    /// gain since the last poll, and the events are taken.
    pub fn poll(&mut self) -> String {
        let pos = self.engine.position().map(|p| {
            Val::obj(vec![
                ("sec", Val::num(p.sec as f64)),
                ("bar", Val::num(p.bar as f64)),
                ("step", Val::num(p.step as f64)),
                ("barIndex", Val::num(p.bar_index as f64)),
                ("bars", Val::num(p.bars as f64)),
                ("playing", Some(Val::Bool(p.playing))),
            ])
        });
        let v = Val::obj(vec![
            ("pos", Some(pos.unwrap_or(Val::Null))),
            ("gr", Val::num(self.engine.take_gr())),
            ("events", Some(Val::Arr(std::mem::take(&mut self.events)))),
        ]);
        canon(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(s: &mut Session, blocks: usize) -> f64 {
        let (mut l, mut r) = (vec![0.0f32; 128], vec![0.0f32; 128]);
        let mut sum = 0.0;
        for _ in 0..blocks {
            s.process(&mut l, &mut r);
            sum += l.iter().chain(&r).map(|x| (*x as f64).powi(2)).sum::<f64>();
        }
        (sum / (blocks as f64 * 256.0)).sqrt()
    }

    #[test]
    fn a_track_plays_and_reports_where_it_is() {
        let mut s = Session::new(5, 48000.0);
        s.message(r#"{"type":"track","genre":"house","seed":7,"dejavu":0.75}"#)
            .unwrap();
        s.message(r#"{"type":"play","bar":0}"#).unwrap();
        assert!(rms(&mut s, 400) > 0.005, "the song is heard");
        let p = parse(&s.poll()).unwrap();
        assert_eq!(
            p.get("pos").and_then(|p| p.get("playing")),
            Some(&Val::Bool(true))
        );
        s.message(r#"{"type":"stop"}"#).unwrap();
        assert!(
            s.message(r#"{"type":"track","genre":"polka","seed":1}"#)
                .is_err()
        );
        assert!(s.message(r#"{"type":"nonsense"}"#).is_err());
    }

    #[test]
    fn the_keyboard_and_pads_sound_without_a_song() {
        let mut s = Session::new(5, 48000.0);
        // The bus is never quite silent (the tape's hiss).
        let floor = rms(&mut s, 200);
        let lab = crate::patches::all()[0].1.to_val();
        s.message(&format!(r#"{{"type":"audition","lab":{}}}"#, canon(&lab)))
            .unwrap();
        s.message(r#"{"type":"on","midi":48,"vel":0.9}"#).unwrap();
        let note = rms(&mut s, 100);
        assert!(note > 3.0 * floor, "the note sounds: {note} over {floor}");
        s.message(r#"{"type":"off","midi":48}"#).unwrap();
        rms(&mut s, 1500);
        let after = rms(&mut s, 100);
        assert!(after < note / 3.0, "and stops: {after}");
        s.message(r#"{"type":"kit","name":"tr808","tweak":{"kick":{"decay":0.2}}}"#)
            .unwrap();
        s.message(r#"{"type":"hit","lane":"kick"}"#).unwrap();
        let hit = rms(&mut s, 40);
        assert!(hit > 3.0 * floor, "the pad sounds: {hit}");
    }

    #[test]
    fn the_tables_parse() {
        let t = parse(&tables_json()).unwrap();
        assert_eq!(
            t.get("genres").and_then(Val::as_arr).map(|g| g.len()),
            Some(GENRES.len())
        );
        assert!(
            t.get("patches")
                .and_then(Val::as_arr)
                .is_some_and(|p| !p.is_empty())
        );
    }
}
