//! Plays a [`Song`] (the new generator's per-layer score) to samples on the
//! lab's instruments, kits and mix: the classic [`crate::engine::Engine`]'s
//! sound, with what the score adds (research/RULES.md, "What the engine
//! lacks"): a sidechain depth per layer, keyed by the layers that key it
//! (#6); automation of any patch parameter, a layer's level and filters,
//! and the music bus's filters (#2); notes at fractional 16ths with their
//! own velocity (#1, #3); swing per layer.
//!
//! ```text
//!   layer: instrument or drum machine ─ channel ─ [hp, lp] ─ × level × energy × pump ─┐
//!          └ reverb and delay sends                                                  ├─ bus hp/lp ─┐
//!   delay return ────────────────────────────────────────────────────────────────────┘             │
//!   risers, downlifters, reverb return ────────────────────────────────────────────────────────────┤
//!                                              energy filter ─ glue ─ tape ─ master ─ limiter ─ out
//! ```
//!
//! Every note is laid out in samples when the song is set (no allocation
//! while it plays), in time order; playing walks that list.

use crate::dsp::{Compressor, Limiter, PingPong, Reverb, Rng, Svf, Tape};
use crate::engine::{Channel, DRUM_LEVEL, EngineEvent, Mix, Position, Sweep, lane_rev};
use crate::instruments::{Drums, Instrument, KitVoice, TriggerOpt, kit_voice, make_instrument};
use crate::score::{Automation, Fx, Song, Sound};
use crate::to_u32;

/// One note, laid out: when (samples from the song's start), which layer,
/// and what.
#[derive(Clone, Debug)]
struct Ev {
    t: u64,
    layer: usize,
    note: usize,
    pattern: usize,
    vel: f64,
    /// Samples (0: the voice's own decay).
    dur: f64,
    glide_from: Option<f64>,
}

/// An instrument or a drum machine.
enum Voice {
    Synth(Box<Instrument>),
    Drums {
        drums: Drums,
        kit: String,
        tweak: Vec<(String, Vec<(String, f64)>)>,
        cache: Vec<(String, KitVoice)>,
    },
}

struct LayerState {
    voice: Voice,
    ch: Channel,
    /// Filters a layer's automation can move (`hp`, `lp`).
    hp: Option<([Svf; 2], f64)>,
    lp: Option<([Svf; 2], f64)>,
    level: f64,
    /// The energy gate's gain, eased.
    gate: f64,
    muted: bool,
}

/// Plays one song.
pub struct SongPlayer {
    seed: u32,
    sr: f64,
    r: Rng,
    song: Option<Song>,
    layers: Vec<LayerState>,
    evs: Vec<Ev>,
    /// The next event to fire.
    next: usize,
    /// Samples from the song's start.
    t: u64,
    /// Samples until playing starts.
    wait: u64,
    total: u64,
    playing: bool,
    energy: f64,
    solo: Option<String>,
    // The sidechain: 0 at rest, 1 fully ducked.
    duck: f64,
    duck_t: u64,
    duck_on: bool,
    fx: Vec<Sweep>,
    /// Which of the song's fx have started.
    fx_next: usize,
    fx_at: Vec<(u64, Fx)>,
    bus_hp: [Svf; 2],
    bus_lp: [Svf; 2],
    bus_hz: (f64, f64),
    en_lp: [Svf; 2],
    en_hz: f64,
    delay: PingPong,
    reverb: Reverb,
    glue: Compressor,
    tape: [Tape; 2],
    lim: Limiter,
    mix: Mix,
    gr_sum: f64,
    gr_n: u64,
    events: Vec<EngineEvent>,
    section: Option<usize>,
}

impl SongPlayer {
    pub fn new(seed: u32, sr: f64) -> SongPlayer {
        let mut p = SongPlayer {
            seed,
            sr,
            r: Rng::new(seed),
            song: None,
            layers: Vec::new(),
            evs: Vec::new(),
            next: 0,
            t: 0,
            wait: 0,
            total: 0,
            playing: false,
            energy: 1.0,
            solo: None,
            duck: 0.0,
            duck_t: 0,
            duck_on: false,
            fx: Vec::new(),
            fx_next: 0,
            fx_at: Vec::new(),
            bus_hp: [Svf::new(); 2],
            bus_lp: [Svf::new(); 2],
            bus_hz: (0.0, 20000.0),
            en_lp: [Svf::new(); 2],
            en_hz: 20000.0,
            delay: PingPong::new(sr),
            reverb: Reverb::new(to_u32(seed as f64 + 11.0), sr),
            glue: Compressor::new(sr),
            tape: [Tape::new(sr); 2],
            lim: Limiter::new(0.95, sr),
            mix: Mix::default(),
            gr_sum: 0.0,
            gr_n: 0,
            events: Vec::new(),
            section: None,
        };
        p.set_mix(Mix::default());
        p
    }

    pub fn song(&self) -> Option<&Song> {
        self.song.as_ref()
    }

    pub fn set_mix(&mut self, m: Mix) {
        self.mix = m;
        self.glue
            .set(-10.0 - m.glue * 10.0, 1.0 + m.glue * 2.5, 0.012, 0.18);
        for t in &mut self.tape {
            t.set(m.tape);
        }
        self.reverb.set(m.rev_size, m.rev_tone);
    }

    pub fn mix(&self) -> Mix {
        self.mix
    }

    /// Loads a song (stopped, at its start) and lays out its notes.
    pub fn set_song(&mut self, song: Song) {
        let sr = self.sr;
        self.layers = song
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let voice = match &l.sound {
                    Sound::Synth(lab) => Voice::Synth(Box::new(make_instrument(
                        (**lab).clone(),
                        to_u32(self.seed as f64 * 31.0 + i as f64),
                        sr,
                    ))),
                    Sound::Drums { kit, tweak } => Voice::Drums {
                        drums: Drums::new(to_u32(self.seed as f64 + 3.0 + i as f64 * 7.0), sr),
                        kit: kit.clone(),
                        tweak: tweak.clone(),
                        cache: Vec::new(),
                    },
                };
                let filt = |p: &str| {
                    l.autos
                        .iter()
                        .any(|a| a.param == p)
                        .then(|| ([Svf::new(); 2], -1.0))
                };
                LayerState {
                    voice,
                    ch: Channel::new(&l.ch, sr),
                    hp: filt("hp"),
                    lp: filt("lp"),
                    level: 1.0,
                    gate: if self.energy >= l.min_energy {
                        1.0
                    } else {
                        0.0
                    },
                    muted: false,
                }
            })
            .collect();
        self.evs = lay_out(&song, sr);
        let step = song.step_secs() * sr;
        self.total = (song.bars as f64 * 16.0 * step).round() as u64;
        self.fx_at = song
            .fx
            .iter()
            .map(|f| {
                let bar = match f {
                    Fx::Riser { bar, .. } | Fx::Downlifter { bar, .. } => *bar,
                };
                ((bar * 16.0 * step).round() as u64, f.clone())
            })
            .collect();
        self.fx_at.sort_by_key(|(t, _)| *t);
        self.delay.time = (song.delay_beats * 60.0 / song.bpm).min(2.0);
        self.delay.fb = song.delay_fb;
        self.song = Some(song);
        self.playing = false;
        self.seek(0);
    }

    fn seek(&mut self, bar: u32) {
        let Some(song) = &self.song else { return };
        let step = song.step_secs() * self.sr;
        self.t = (bar as f64 * 16.0 * step).round() as u64;
        self.next = self.evs.partition_point(|e| e.t < self.t);
        self.fx_next = self.fx_at.partition_point(|(t, _)| *t < self.t);
        self.fx.clear();
        self.section = None;
    }

    /// Starts at `bar` after `delay_s` seconds.
    pub fn play(&mut self, bar: u32, delay_s: f64) {
        if self.song.is_none() {
            return;
        }
        self.seek(bar);
        self.wait = (delay_s.max(0.0) * self.sr).round() as u64;
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn set_energy(&mut self, e: f64) {
        self.energy = e.clamp(0.0, 1.0);
    }

    pub fn energy(&self) -> f64 {
        self.energy
    }

    /// Mutes layers by name (`drums` mutes every drum layer).
    pub fn set_mute(&mut self, names: &[String]) {
        let Some(song) = &self.song else { return };
        for (st, l) in self.layers.iter_mut().zip(&song.layers) {
            st.muted = names
                .iter()
                .any(|n| *n == l.name || (n == "drums" && matches!(l.sound, Sound::Drums { .. })));
        }
    }

    pub fn set_solo(&mut self, name: Option<String>) {
        self.solo = name;
    }

    /// Replaces a layer's patch members (the lab's knobs).
    pub fn set_patch(&mut self, layer: &str, lab: crate::track::Lab) {
        let Some(song) = &self.song else { return };
        let Some(i) = song.layers.iter().position(|l| l.name == layer) else {
            return;
        };
        if let Voice::Synth(inst) = &mut self.layers[i].voice {
            inst.set_patch(lab);
        }
    }

    /// Where the song is.
    pub fn position(&self) -> Option<Position> {
        let song = self.song.as_ref()?;
        let bar = self.bar_now() as u32;
        let sec = song.section_at(bar).unwrap_or(0);
        let step = ((self.bar_now().fract()) * 16.0) as u32;
        Some(Position {
            sec,
            bar: bar - song.sections.get(sec).map_or(0, |s| s.bar),
            step,
            bar_index: bar,
            bars: song.bars,
            playing: self.playing,
        })
    }

    fn bar_now(&self) -> f64 {
        match &self.song {
            Some(s) => self.t as f64 / (16.0 * s.step_secs() * self.sr),
            None => 0.0,
        }
    }

    pub fn take_gr(&mut self) -> f64 {
        let g = if self.gr_n != 0 {
            self.gr_sum / self.gr_n as f64
        } else {
            1.0
        };
        self.gr_sum = 0.0;
        self.gr_n = 0;
        g
    }

    pub fn take_events(&mut self) -> Vec<EngineEvent> {
        std::mem::take(&mut self.events)
    }

    /// The kit voice of a drum layer's lane, with its tweaks; cached.
    fn kit_lane(
        kit: &str,
        tweak: &[(String, Vec<(String, f64)>)],
        cache: &mut Vec<(String, KitVoice)>,
        lane: &str,
    ) -> Option<KitVoice> {
        if let Some((_, k)) = cache.iter().find(|(n, _)| n == lane) {
            return Some(*k);
        }
        let mut k = kit_voice(kit, lane, None)?;
        if let Some((_, ps)) = tweak.iter().find(|(n, _)| n == lane) {
            for (p, v) in ps {
                k.set(p, *v);
            }
        }
        k.rev = Some(k.rev.or(lane_rev(lane)).unwrap_or(0.0));
        cache.push((lane.to_owned(), k));
        Some(k)
    }

    fn fire(&mut self, i: usize) {
        let song = self.song.as_ref().expect("a song");
        let e = &self.evs[i];
        let layer = &song.layers[e.layer];
        let note = &layer.patterns[e.pattern].notes[e.note];
        let st = &mut self.layers[e.layer];
        match &mut st.voice {
            Voice::Synth(inst) => inst.trigger(
                &note.midis,
                e.vel,
                e.dur.max(1.0),
                &TriggerOpt {
                    glide_from: e.glide_from,
                    accent: note.accent,
                },
            ),
            Voice::Drums {
                drums,
                kit,
                tweak,
                cache,
            } => {
                let lane = note.lane.as_deref().unwrap_or("kick");
                if let Some(k) = Self::kit_lane(kit, tweak, cache, lane) {
                    drums.hit(lane, &k, e.vel, 1.0, e.dur as usize);
                }
            }
        }
        if layer.keys_pump && e.vel > 0.3 {
            self.duck_t = 0;
            self.duck_on = true;
        }
    }

    /// Automation, the energy gates and the section events: every 64
    /// samples.
    fn control(&mut self) {
        let bar = self.bar_now();
        let sr = self.sr;
        let Some(song) = &self.song else { return };
        for (st, l) in self.layers.iter_mut().zip(&song.layers) {
            for a in &l.autos {
                let Some(v) = a.at(bar) else { continue };
                match a.param.as_str() {
                    "level" => st.level = v,
                    "hp" => set_filter(&mut st.hp, v, sr),
                    "lp" => set_filter(&mut st.lp, v, sr),
                    p => {
                        if let Voice::Synth(inst) = &mut st.voice {
                            inst.lab_mut().set(p, v);
                        }
                    }
                }
            }
            let want = if self.energy + 1e-9 >= l.min_energy {
                1.0
            } else {
                0.0
            };
            st.gate += (want - st.gate) * 0.08;
        }
        let hz = |a: &Automation| a.at(bar);
        let mut bus = (0.0, 20000.0);
        for a in &song.bus {
            match a.param.as_str() {
                "hp" => bus.0 = hz(a).unwrap_or(0.0),
                "lp" => bus.1 = hz(a).unwrap_or(20000.0),
                _ => {}
            }
        }
        if bus != self.bus_hz {
            self.bus_hz = bus;
            for f in &mut self.bus_hp {
                f.set(bus.0.max(10.0), 0.707, sr);
            }
            for f in &mut self.bus_lp {
                f.set(bus.1.min(sr * 0.45), 0.707, sr);
            }
        }
        let en_hz = 250.0 * 2f64.powf(self.energy * 6.3);
        self.en_hz += (en_hz - self.en_hz) * 0.05;
        if self.playing {
            let sec = song.section_at(bar as u32);
            if sec != self.section {
                self.section = sec;
                if let Some(s) = sec {
                    self.events.push(EngineEvent::Section(s));
                }
            }
        }
    }

    /// Renders `l.len()` stereo samples into `l` and `r` (overwritten).
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let sr = self.sr;
        let (gain, rel) = match &self.song {
            Some(s) => (s.gain, s.pump_release),
            None => (1.0, 0.2),
        };
        let k_duck = 1.0 - (-1.0 / (rel / 3.0 * sr)).exp();
        let mut o = [0.0f64; 2];
        let mut s = [0.0f64; 2];
        for i in 0..l.len() {
            if (i & 63) == 0 {
                self.control();
            }
            if self.playing {
                if self.wait > 0 {
                    self.wait -= 1;
                } else {
                    while self.next < self.evs.len() && self.evs[self.next].t <= self.t {
                        self.fire(self.next);
                        self.next += 1;
                    }
                    while self.fx_next < self.fx_at.len() && self.fx_at[self.fx_next].0 <= self.t {
                        let step = self.song.as_ref().map_or(0.1, |s| s.step_secs());
                        let sweep = match &self.fx_at[self.fx_next].1 {
                            Fx::Riser { bars, level, .. } => Sweep::new(
                                300.0,
                                9000.0,
                                0.002,
                                0.16 * level,
                                (bars * 16.0 * step * sr).round(),
                                1.4,
                                sr,
                            ),
                            Fx::Downlifter { bars, level, .. } => Sweep::new(
                                6000.0,
                                250.0,
                                0.12 * level,
                                0.0005,
                                (bars * 16.0 * step * sr).round(),
                                1.2,
                                sr,
                            ),
                        };
                        self.fx.push(sweep);
                        self.fx_next += 1;
                    }
                    self.t += 1;
                    if self.t >= self.total {
                        // The song wraps (the lab loops it; a radio moves on
                        // when it hears the end).
                        self.events.push(EngineEvent::End);
                        self.seek(0);
                    }
                }
            }
            // The sidechain: down in 6 ms, held to 30 ms, back with the
            // release (the classic engine's pump, per layer depth).
            if self.duck_on {
                let ms = self.duck_t as f64 / sr * 1000.0;
                self.duck_t += 1;
                if ms < 6.0 {
                    self.duck = self.duck.max(ms / 6.0);
                } else if ms > 30.0 {
                    self.duck -= self.duck * k_duck;
                    if self.duck < 0.001 {
                        self.duck = 0.0;
                        self.duck_on = false;
                    }
                }
            }
            let mut ml = 0.0;
            let mut mr = 0.0;
            let mut rv = [0.0f64; 2];
            let mut dl = 0.0;
            let Some(song) = &self.song else {
                l[i] = 0.0;
                r[i] = 0.0;
                continue;
            };
            let solo = self.solo.as_deref();
            for (st, ly) in self.layers.iter_mut().zip(&song.layers) {
                let heard = !st.muted
                    && solo.is_none_or(|n| {
                        n == ly.name || (n == "drums" && matches!(ly.sound, Sound::Drums { .. }))
                    });
                let g = st.level
                    * st.gate
                    * (1.0 - ly.pump * self.duck)
                    * if heard { 1.0 } else { 0.0 };
                match &mut st.voice {
                    Voice::Synth(inst) => {
                        inst.run(&mut st.ch.buf);
                        st.ch.run(&mut o);
                    }
                    Voice::Drums { drums, .. } => {
                        let mut d = [0.0f64; 2];
                        let mut drv = [0.0f64; 2];
                        drums.run(|h, y| {
                            let y = y * DRUM_LEVEL;
                            let p = h.pan;
                            let (a, b) = (y * (1.0 - p).min(1.0), y * (1.0 + p).min(1.0));
                            d[0] += a;
                            d[1] += b;
                            if let Some(rv) = h.k.rev.filter(|v| *v != 0.0) {
                                drv[0] += a * rv;
                                drv[1] += b * rv;
                            }
                        });
                        st.ch.buf = d;
                        st.ch.run(&mut o);
                        rv[0] += drv[0] * g;
                        rv[1] += drv[1] * g;
                    }
                }
                if let Some((f, _)) = &mut st.hp {
                    f[0].run(o[0]);
                    f[1].run(o[1]);
                    o = [f[0].hp, f[1].hp];
                }
                if let Some((f, _)) = &mut st.lp {
                    o = [f[0].run(o[0]), f[1].run(o[1])];
                }
                let (a, b) = (o[0] * g, o[1] * g);
                ml += a;
                mr += b;
                rv[0] += a * st.ch.rev;
                rv[1] += b * st.ch.rev;
                dl += (a + b) * 0.5 * st.ch.dly;
            }
            s[0] = 0.0;
            s[1] = 0.0;
            self.delay.run(dl, &mut s);
            ml += s[0];
            mr += s[1];
            if self.bus_hz.0 > 20.0 {
                self.bus_hp[0].run(ml);
                self.bus_hp[1].run(mr);
                ml = self.bus_hp[0].hp;
                mr = self.bus_hp[1].hp;
            }
            if self.bus_hz.1 < 19000.0 {
                ml = self.bus_lp[0].run(ml);
                mr = self.bus_lp[1].run(mr);
            }
            let mut f = self.fx.len();
            while f > 0 {
                f -= 1;
                let y = self.fx[f].run(&mut self.r);
                ml += y;
                mr += y;
                rv[0] += y * 0.5;
                rv[1] += y * 0.5;
                if self.fx[f].done() {
                    self.fx.remove(f);
                }
            }
            s[0] = 0.0;
            s[1] = 0.0;
            self.reverb
                .run(rv[0] * self.mix.reverb, rv[1] * self.mix.reverb, &mut s);
            ml += s[0] * 0.42;
            mr += s[1] * 0.42;
            if self.en_hz < 18000.0 {
                self.en_lp[0].set(self.en_hz, 0.8, sr);
                self.en_lp[1].set(self.en_hz, 0.8, sr);
                ml = self.en_lp[0].run(ml);
                mr = self.en_lp[1].run(mr);
            }
            ml *= gain;
            mr *= gain;
            let gr = if self.mix.glue > 0.0 {
                self.glue.run(ml, mr)
            } else {
                1.0
            };
            self.gr_sum += gr;
            self.gr_n += 1;
            ml = self.tape[0].run(ml * gr) * self.mix.master;
            mr = self.tape[1].run(mr * gr) * self.mix.master;
            self.lim.run(ml, mr, &mut o);
            l[i] = o[0] as f32;
            r[i] = o[1] as f32;
        }
    }
}

/// Sets an automated filter's cutoff (only when it moved).
fn set_filter(f: &mut Option<([Svf; 2], f64)>, hz: f64, sr: f64) {
    if let Some((svf, at)) = f
        && (*at - hz).abs() > 0.5
    {
        *at = hz;
        for x in svf.iter_mut() {
            x.set(hz.clamp(10.0, sr * 0.45), 0.707, sr);
        }
    }
}

/// Every note of the song in samples, in time order, with the song's (or
/// the layer's) swing and each mono line's glides.
fn lay_out(song: &Song, sr: f64) -> Vec<Ev> {
    let step = song.step_secs() * sr;
    let mut evs = Vec::new();
    for (li, layer) in song.layers.iter().enumerate() {
        let swing = layer.swing.unwrap_or(song.swing);
        let delay = ((2.0 * swing / 100.0) - 1.0).max(0.0);
        let mut mine: Vec<(f64, usize, usize, f64)> = Vec::new();
        for clip in &layer.clips {
            let p = &layer.patterns[clip.pattern];
            if p.len <= 0.0 {
                continue;
            }
            let start = clip.bar as f64 * 16.0;
            let end = start + clip.bars as f64 * 16.0;
            let mut k = 0.0;
            while start + k * p.len < end {
                for (ni, n) in p.notes.iter().enumerate() {
                    let at = start + k * p.len + n.at;
                    if at < end && at >= start {
                        mine.push((at, clip.pattern, ni, n.vel * clip.gain));
                    }
                }
                k += 1.0;
            }
        }
        mine.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut last: Option<f64> = None;
        for (at, pi, ni, vel) in mine {
            let n = &layer.patterns[pi].notes[ni];
            // Swing delays the off 16ths (even steps, counting from 1).
            let off = at.fract() == 0.0 && (at as i64) % 2 == 1;
            let at_s = if off { at + delay } else { at };
            let glide_from = if n.slide { last } else { None };
            if let Some(m) = n.midis.last() {
                last = Some(*m);
            }
            evs.push(Ev {
                t: (at_s * step).round().max(0.0) as u64,
                layer: li,
                note: ni,
                pattern: pi,
                vel: vel.clamp(0.0, 1.0),
                dur: (n.len * step).round(),
                glide_from,
            });
        }
    }
    evs.sort_by_key(|e| e.t);
    evs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::{Layer, Mark, Note, Pattern, Role};

    fn little_song() -> Song {
        let mut s = Song::new("test", 1, 120.0, 8);
        let mut k = Layer::new(
            "kick",
            Role::Kick,
            Sound::Drums {
                kit: "tr909".into(),
                tweak: Vec::new(),
            },
        );
        k.keys_pump = true;
        let p = k.pattern(Pattern {
            name: "4/4".into(),
            len: 16.0,
            notes: (0..4)
                .map(|b| Note::hit(b as f64 * 4.0, "kick", 1.0))
                .collect(),
        });
        k.clip(0, 8, p, Mark::Core);
        let mut b = Layer::new(
            "bass",
            Role::Bass,
            Sound::Synth(Box::new(crate::patches::bpatch("acid").expect("acid"))),
        );
        b.pump = 0.5;
        b.min_energy = 0.5;
        let p = b.pattern(Pattern {
            name: "8ths".into(),
            len: 16.0,
            notes: (0..8)
                .map(|i| Note::pitched(i as f64 * 2.0, 1.0, vec![36.0], 0.8))
                .collect(),
        });
        b.clip(4, 4, p, Mark::Core);
        s.layers = vec![k, b];
        s.sections = vec![
            crate::score::Section {
                name: "intro".into(),
                bar: 0,
                bars: 4,
                energy: 0.4,
            },
            crate::score::Section {
                name: "groove".into(),
                bar: 4,
                bars: 4,
                energy: 0.8,
            },
        ];
        s
    }

    fn rms(p: &mut SongPlayer, secs: f64) -> f64 {
        let n = (secs * 48000.0 / 128.0) as usize;
        let (mut l, mut r) = (vec![0f32; 128], vec![0f32; 128]);
        let mut sum = 0.0;
        for _ in 0..n {
            p.process(&mut l, &mut r);
            sum += l.iter().chain(&r).map(|x| (*x as f64).powi(2)).sum::<f64>();
        }
        (sum / (n as f64 * 256.0)).sqrt()
    }

    #[test]
    fn notes_are_laid_out_on_the_grid_with_swing() {
        let mut s = little_song();
        s.swing = 60.0;
        let evs = lay_out(&s, 48000.0);
        let step = s.step_secs() * 48000.0;
        let kicks: Vec<u64> = evs
            .iter()
            .filter(|e| e.layer == 0)
            .map(|e| e.t)
            .take(3)
            .collect();
        assert_eq!(
            kicks,
            vec![0, (4.0 * step).round() as u64, (8.0 * step).round() as u64]
        );
        // The bass's 8ths fall on odd steps (from 1) only: no swing.
        let first_bass = evs.iter().find(|e| e.layer == 1).unwrap();
        assert_eq!(first_bass.t, (64.0 * step).round() as u64);
        assert_eq!(evs.iter().filter(|e| e.layer == 1).count(), 32);
        assert!(evs.windows(2).all(|w| w[0].t <= w[1].t));
    }

    #[test]
    fn it_plays_wraps_and_gates_by_energy() {
        let mut p = SongPlayer::new(5, 48000.0);
        p.set_song(little_song());
        p.play(0, 0.0);
        let intro = rms(&mut p, 7.9);
        assert!(intro > 0.01, "the kick: {intro}");
        let groove = rms(&mut p, 7.9);
        assert!(groove > intro, "the bass joins: {groove} vs {intro}");
        let ends = p
            .take_events()
            .iter()
            .filter(|e| matches!(e, EngineEvent::End))
            .count();
        assert_eq!(ends, 0);
        rms(&mut p, 0.5);
        assert!(
            p.take_events()
                .iter()
                .any(|e| matches!(e, EngineEvent::End)),
            "16 s: it wraps"
        );
        p.play(4, 0.0);
        p.set_solo(Some("bass".into()));
        p.set_energy(0.2);
        rms(&mut p, 1.0);
        let low = rms(&mut p, 3.0);
        p.set_energy(1.0);
        rms(&mut p, 1.0);
        let high = rms(&mut p, 3.0);
        assert!(
            high > low * 10.0,
            "the bass drops out at low energy: {low} vs {high}"
        );
    }
}
