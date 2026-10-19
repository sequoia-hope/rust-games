//! Sound, on the web, through the Web Audio API: three buses into one
//! master.
//!
//! - **Effects**: wind that rises with speed, the edge's hiss when carving
//!   and the scrape when braking (filtered noise), and short synthesised
//!   cues: the countdown, take-offs, landings, tricks, crashes, the finish.
//! - **Music**: Ten-Eighty FM, `sb_music`'s station player running as its
//!   own wasm in an AudioWorklet (`web/music-worklet.js`, `sb_music.wasm`,
//!   both beside the page), at full energy in a run and thinner in the menu
//!   and the pause. A page without AudioWorklet (Safari over plain http: it
//!   is for secure pages only) plays the same player on the main thread
//!   through a ScriptProcessorNode instead.
//! - **Voices**: the announcers' clips (`audio/announcers/<clip>.mp3`,
//!   listed in its `index.json`), fetched in the background once sound
//!   starts, decoded when a bit is about to be said, and played one at a
//!   time with the music ducked under them. [`crate::radio`] decides what
//!   to say and when; it asks for clips through [`voice_prepare`],
//!   [`voice_start`] and [`voice_stop`].
//!
//! The context wakes inside a user gesture (`web::gesture`), as browsers
//! require. Natively there is no sound yet (Bevy's audio is not built in):
//! the voice calls answer [`Clip::Missing`] and the captions run on their
//! own.

#[cfg(not(target_arch = "wasm32"))]
use crate::radio::Radio;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, web_audio::drive.after(crate::ride::RideSet));
    #[cfg(not(target_arch = "wasm32"))]
    let _ = app;
}

/// Where a clip stands when asked to play.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clip {
    /// Playing now, for this many seconds.
    Playing(f32),
    /// Still on its way: ask again next frame.
    Loading,
    /// Not recorded (or not for these words), failed, or no sound here.
    Missing,
}

#[cfg(target_arch = "wasm32")]
pub use web_audio::{
    gesture, ready, save_radio, saved_radio, status, voice_prepare, voice_start, voice_stop,
};

/// Whether sound can play yet (on the web, after the first gesture). The
/// announcers wait for it, so the first thing they say is heard.
#[cfg(not(target_arch = "wasm32"))]
pub fn ready() -> bool {
    true
}
#[cfg(not(target_arch = "wasm32"))]
pub fn voice_prepare(_clips: &[String]) {}
#[cfg(not(target_arch = "wasm32"))]
pub fn voice_start(_clip: &str) -> Clip {
    Clip::Missing
}
#[cfg(not(target_arch = "wasm32"))]
pub fn voice_stop() {}
/// The RADIO setting the player chose last time (the web keeps it).
#[cfg(not(target_arch = "wasm32"))]
pub fn saved_radio() -> Radio {
    Radio::All
}
#[cfg(not(target_arch = "wasm32"))]
pub fn save_radio(_r: Radio) {}

#[cfg(target_arch = "wasm32")]
mod web_audio {
    use super::Clip;
    use crate::radio::Radio;
    use crate::ride::{Feed, Run};
    use crate::{Screen, Setup};
    use bevy::prelude::*;
    use sb_sim::race::{SessionEvent, State};
    use sb_sim::rider::{Event, Phase};
    use sb_sim::tricks::Landing;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::{JsFuture, spawn_local};
    use web_sys::{
        AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, AudioProcessingEvent,
        AudioWorkletNode, AudioWorkletNodeOptions, BiquadFilterNode, BiquadFilterType, GainNode,
        OscillatorType, ScriptProcessorNode,
    };

    /// The levels of the buses (the music's generator runs about -23 dBFS
    /// RMS, the clips -16).
    const SFX_VOL: f32 = 1.0;
    const MUSIC_VOL: f32 = 0.75;
    const VOICE_VOL: f32 = 1.0;
    /// The music under a voice.
    const DUCK: f32 = 0.42;
    /// The worklet's files, beside the page (relative: the game may be
    /// served under a sub-path).
    const WORKLET_URL: &str = "music-worklet.js";
    const MUSIC_WASM_URL: &str = "sb_music.wasm";
    const CLIPS_URL: &str = "audio/announcers/";
    const MUSIC_SEED: u32 = 1080;
    const RADIO_KEY: &str = "sb.radio";

    struct Loop {
        filter: BiquadFilterNode,
        gain: GainNode,
    }

    #[derive(Default)]
    enum MusicState {
        #[default]
        Idle,
        Loading,
        Ready(AudioWorkletNode),
        /// No AudioWorklet here (an insecure page, an old browser): the
        /// player on the main thread.
        Script(ScriptMusic),
        Failed,
    }

    /// The fallback's player, its node, and the energy it is asked for
    /// (approached block by block, as the worklet's parameter is).
    struct ScriptMusic {
        _node: ScriptProcessorNode,
        energy: Rc<std::cell::Cell<f32>>,
    }

    /// The fallback's buffer in frames: 85 ms at 48 kHz, long enough to
    /// ride out a slow frame, as long as a frame never takes longer.
    const SCRIPT_FRAMES: u32 = 4096;

    enum Decoded {
        Decoding,
        Ready(AudioBuffer),
        Failed,
    }

    #[derive(Default)]
    struct Voices {
        /// Clip → the words it was recorded from (`index.json`); `None`
        /// until it is in.
        index: Option<HashMap<String, String>>,
        index_asked: bool,
        /// Compressed bytes, fetched ahead.
        bytes: HashMap<String, js_sys::ArrayBuffer>,
        decoded: HashMap<String, Decoded>,
        playing: Option<AudioBufferSourceNode>,
        playing_until: f64,
        /// Clips played, and the last asked for, for the test hook.
        played: u32,
        last: String,
    }

    struct Sfx {
        ctx: AudioContext,
        master: GainNode,
        sfx: GainNode,
        /// The music's level (the setting), and the duck under it.
        music: GainNode,
        duck: GainNode,
        voice: GainNode,
        noise: AudioBuffer,
        wind: Loop,
        carve: Loop,
        radio: Rc<RefCell<MusicState>>,
        voices: Rc<RefCell<Voices>>,
        /// Taps the music bus (before the duck), for the test hook.
        tap: web_sys::AnalyserNode,
    }

    thread_local! {
        static SFX: RefCell<Option<Sfx>> = const { RefCell::new(None) };
    }

    fn make() -> Result<Sfx, JsValue> {
        // An iPhone in Silent mode mutes Web Audio unless the page asks to
        // be heard as media does (Safari 17 and later).
        if let Some(nav) = web_sys::window().map(|w| w.navigator())
            && let Ok(session) = js_sys::Reflect::get(&nav, &JsValue::from_str("audioSession"))
            && session.is_object()
        {
            let _ = js_sys::Reflect::set(
                &session,
                &JsValue::from_str("type"),
                &JsValue::from_str("playback"),
            );
        }
        let ctx = AudioContext::new()?;
        let master = ctx.create_gain()?;
        master.gain().set_value(0.8);
        master.connect_with_audio_node(&ctx.destination())?;
        let bus = |v: f32, to: &GainNode| -> Result<GainNode, JsValue> {
            let g = ctx.create_gain()?;
            g.gain().set_value(v);
            g.connect_with_audio_node(to)?;
            Ok(g)
        };
        let sfx = bus(SFX_VOL, &master)?;
        let music = bus(0.0, &master)?;
        let duck = bus(1.0, &music)?;
        let tap = ctx.create_analyser()?;
        tap.set_fft_size(2048);
        let voice = bus(VOICE_VOL, &master)?;
        let rate = ctx.sample_rate();
        let len = (rate * 2.0) as u32;
        let noise = ctx.create_buffer(1, len, rate)?;
        let mut data = vec![0f32; len as usize];
        let mut x = 0x1234_5678u32;
        let mut brown = 0.0f32;
        for d in data.iter_mut() {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let w = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
            brown = (brown + w * 0.08) * 0.985;
            *d = w * 0.6 + brown * 1.5;
        }
        noise.copy_to_channel(&data, 0)?;
        let looped = |kind: BiquadFilterType, f: f32, q: f32| -> Result<Loop, JsValue> {
            let src = ctx.create_buffer_source()?;
            src.set_buffer(Some(&noise));
            src.set_loop(true);
            let filter = ctx.create_biquad_filter()?;
            filter.set_type(kind);
            filter.frequency().set_value(f);
            filter.q().set_value(q);
            let gain = ctx.create_gain()?;
            gain.gain().set_value(0.0);
            src.connect_with_audio_node(&filter)?;
            filter.connect_with_audio_node(&gain)?;
            gain.connect_with_audio_node(&sfx)?;
            src.start()?;
            Ok(Loop { filter, gain })
        };
        let wind = looped(BiquadFilterType::Bandpass, 400.0, 0.7)?;
        let carve = looped(BiquadFilterType::Highpass, 2500.0, 0.5)?;
        Ok(Sfx {
            ctx,
            master,
            sfx,
            music,
            duck,
            voice,
            noise,
            wind,
            carve,
            radio: Rc::default(),
            voices: Rc::default(),
            tap,
        })
    }

    /// Called by the page inside a user gesture: make or wake the context.
    pub fn gesture() {
        SFX.with(|s| {
            let mut s = s.borrow_mut();
            if s.is_none() {
                *s = make().ok();
            }
            if let Some(s) = s.as_ref()
                && s.ctx.state() != AudioContextState::Running
            {
                let _ = s.ctx.resume();
            }
        });
    }

    pub fn ready() -> bool {
        SFX.with(|s| {
            s.borrow()
                .as_ref()
                .is_some_and(|s| s.ctx.state() == AudioContextState::Running)
        })
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn saved_radio() -> Radio {
        storage()
            .and_then(|s| s.get_item(RADIO_KEY).ok().flatten())
            .and_then(|k| Radio::from_key(&k))
            .unwrap_or_default()
    }

    pub fn save_radio(r: Radio) {
        if let Some(s) = storage() {
            let _ = s.set_item(RADIO_KEY, r.key());
        }
    }

    fn log(m: &str) {
        web_sys::console::warn_1(&JsValue::from_str(m));
    }

    async fn fetch_bytes(url: &str) -> Result<js_sys::ArrayBuffer, JsValue> {
        let w = web_sys::window().ok_or("no window")?;
        let resp: web_sys::Response = JsFuture::from(w.fetch_with_str(url))
            .await?
            .unchecked_into();
        if !resp.ok() {
            return Err(JsValue::from_str(&format!("{url}: HTTP {}", resp.status())));
        }
        Ok(JsFuture::from(resp.array_buffer()?).await?.unchecked_into())
    }

    // ── The music ──────────────────────────────────────────────────────

    /// `ctx.audioWorklet.addModule(worklet)`, the wasm fetched and
    /// compiled, and the node made and tuned to the station.
    async fn load_music(
        ctx: AudioContext,
        out: GainNode,
        tap: web_sys::AnalyserNode,
    ) -> Result<AudioWorkletNode, JsValue> {
        // An insecure context or an old browser has no `audioWorklet`.
        let has = js_sys::Reflect::get(&ctx, &JsValue::from_str("audioWorklet"))?;
        if has.is_undefined() || has.is_null() {
            return Err(JsValue::from_str("no AudioWorklet"));
        }
        let added = ctx.audio_worklet()?.add_module(WORKLET_URL)?;
        let bytes = fetch_bytes(MUSIC_WASM_URL).await?;
        JsFuture::from(added).await?;
        let module = JsFuture::from(js_sys::WebAssembly::compile(&bytes)).await?;
        let po = js_sys::Object::new();
        js_sys::Reflect::set(&po, &"module".into(), &module)?;
        js_sys::Reflect::set(&po, &"seed".into(), &JsValue::from(MUSIC_SEED))?;
        let opts = AudioWorkletNodeOptions::new();
        opts.set_processor_options(Some(&po));
        opts.set_number_of_inputs(0);
        opts.set_output_channel_count(&js_sys::Array::of1(&JsValue::from(2)));
        let node = AudioWorkletNode::new_with_options(&ctx, "sb-radio", &opts)?;
        node.connect_with_audio_node(&out)?;
        node.connect_with_audio_node(&tap)?;
        // Tune in: the station as of now, on the wall clock (every
        // player hears the same song at the same moment).
        let now = js_sys::Date::now() / 1000.0;
        let day = (now / 86400.0).floor();
        let params = node.parameters()?;
        let t = ctx.current_time();
        let set = |k: &str, v: f32| {
            if let Some(p) = params.get(k) {
                let _ = p.set_value_at_time(v, t);
            }
        };
        set("wallDay", day as f32);
        set("wallSec", (now - day * 86400.0) as f32);
        set("tune", 1.0);
        set("station", 0.0);
        Ok(node)
    }

    /// The fallback: the station player in this wasm, filling a
    /// ScriptProcessorNode's buffers on the main thread (about 5 % of a
    /// desktop core), tuned as `load_music` tunes the worklet.
    fn script_music(
        ctx: &AudioContext,
        out: &GainNode,
        tap: &web_sys::AnalyserNode,
    ) -> Result<ScriptMusic, JsValue> {
        use sb_music::player::Player;
        use wasm_bindgen::closure::Closure;
        const BLOCK: usize = 128;
        let node = ctx.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
            SCRIPT_FRAMES,
            1,
            2,
        )?;
        let rate = ctx.sample_rate();
        let mut player = Player::new(MUSIC_SEED, rate as f64);
        let now = js_sys::Date::now() / 1000.0;
        let day = (now / 86400.0).floor();
        let (day, sec) = (day as f32, (now - day * 86400.0) as f32);
        let energy = Rc::new(std::cell::Cell::new(1.0f32));
        let want = energy.clone();
        let mut at = 1.0f32;
        // The worklet's `setTargetAtTime(e, t, 0.6)`, per block.
        let step = 1.0 - (-(BLOCK as f32) / rate / 0.6).exp();
        let (mut l, mut r) = (Vec::new(), Vec::new());
        let fill =
            Closure::<dyn FnMut(AudioProcessingEvent)>::new(move |e: AudioProcessingEvent| {
                let Ok(buf) = e.output_buffer() else { return };
                let n = buf.length() as usize;
                l.resize(n, 0.0);
                r.resize(n, 0.0);
                for (cl, cr) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
                    at += (want.get() - at) * step;
                    player.set_params(0.0, day, sec, 1.0, at);
                    player.process(cl, cr);
                }
                let _ = buf.copy_to_channel(&l, 0);
                let _ = buf.copy_to_channel(&r, 1);
            });
        node.set_onaudioprocess(Some(fill.as_ref().unchecked_ref()));
        // The node plays for the rest of the page's life, and so does this.
        fill.forget();
        node.connect_with_audio_node(out)?;
        node.connect_with_audio_node(tap)?;
        Ok(ScriptMusic {
            _node: node,
            energy,
        })
    }

    impl Sfx {
        fn start_music(&self) {
            if !matches!(*self.radio.borrow(), MusicState::Idle) {
                return;
            }
            let has = js_sys::Reflect::get(&self.ctx, &JsValue::from_str("audioWorklet"))
                .is_ok_and(|w| !w.is_undefined() && !w.is_null());
            if !has {
                *self.radio.borrow_mut() = match script_music(&self.ctx, &self.duck, &self.tap) {
                    Ok(m) => MusicState::Script(m),
                    Err(e) => {
                        log(&format!("the music could not start: {e:?}"));
                        MusicState::Failed
                    }
                };
                return;
            }
            *self.radio.borrow_mut() = MusicState::Loading;
            let (ctx, out, state, tap) = (
                self.ctx.clone(),
                self.duck.clone(),
                self.radio.clone(),
                self.tap.clone(),
            );
            spawn_local(async move {
                let r = load_music(ctx, out, tap).await;
                *state.borrow_mut() = match r {
                    Ok(n) => MusicState::Ready(n),
                    Err(e) => {
                        log(&format!("the music could not start: {e:?}"));
                        MusicState::Failed
                    }
                };
            });
        }

        fn set_energy(&self, e: f32) {
            match &*self.radio.borrow() {
                MusicState::Ready(n) => {
                    if let Ok(p) = n.parameters()
                        && let Some(p) = p.get("energy")
                    {
                        let _ = p.set_target_at_time(e, self.ctx.current_time(), 0.6);
                    }
                }
                MusicState::Script(m) => m.energy.set(e),
                _ => {}
            }
        }

        // ── The voices ─────────────────────────────────────────────────

        /// `index.json`, then every clip's bytes, one after another.
        fn load_voices(&self) {
            if self.voices.borrow().index_asked {
                return;
            }
            self.voices.borrow_mut().index_asked = true;
            let v = self.voices.clone();
            spawn_local(async move {
                let index = match fetch_bytes(&format!("{CLIPS_URL}index.json")).await {
                    Ok(b) => parse_index(&js_sys::Uint8Array::new(&b).to_vec()),
                    Err(e) => {
                        log(&format!("the announcers' clips are missing: {e:?}"));
                        HashMap::new()
                    }
                };
                let ids: Vec<String> = index.keys().cloned().collect();
                v.borrow_mut().index = Some(index);
                for id in ids {
                    if v.borrow().bytes.contains_key(&id) {
                        continue;
                    }
                    if let Ok(b) = fetch_bytes(&format!("{CLIPS_URL}{id}.mp3")).await {
                        v.borrow_mut().bytes.insert(id, b);
                    }
                }
            });
        }

        fn prepare(&self, clips: &[String]) {
            let mut v = self.voices.borrow_mut();
            // Only what is about to be said stays decoded (a clip decoded is
            // ten times its bytes).
            v.decoded.retain(|k, _| clips.contains(k));
            for id in clips {
                if v.decoded.contains_key(id) {
                    continue;
                }
                v.decoded.insert(id.clone(), Decoded::Decoding);
                let (ctx, voices, id) = (self.ctx.clone(), self.voices.clone(), id.clone());
                spawn_local(async move {
                    let have = voices.borrow().bytes.get(&id).cloned();
                    let bytes = match have {
                        Some(b) => Ok(b),
                        None => fetch_bytes(&format!("{CLIPS_URL}{id}.mp3")).await,
                    };
                    let decoded = match bytes {
                        // decodeAudioData takes the buffer: a copy, so the
                        // cached bytes stay.
                        Ok(b) => match ctx.decode_audio_data(&b.slice(0)) {
                            Ok(p) => JsFuture::from(p)
                                .await
                                .ok()
                                .and_then(|b| b.dyn_into::<AudioBuffer>().ok()),
                            Err(_) => None,
                        },
                        Err(_) => None,
                    };
                    let mut v = voices.borrow_mut();
                    if v.decoded.contains_key(&id) {
                        v.decoded
                            .insert(id, decoded.map_or(Decoded::Failed, Decoded::Ready));
                    }
                });
            }
        }

        fn start(&self, id: &str, words: &str) -> Clip {
            let mut v = self.voices.borrow_mut();
            v.last = id.to_owned();
            let Some(index) = &v.index else {
                return Clip::Loading;
            };
            // A clip recorded from other words is not this line.
            if index.get(id).is_none_or(|t| t != words) {
                return Clip::Missing;
            }
            let buf = match v.decoded.get(id) {
                Some(Decoded::Ready(b)) => b.clone(),
                Some(Decoded::Decoding) => return Clip::Loading,
                Some(Decoded::Failed) => return Clip::Missing,
                None => {
                    drop(v);
                    self.prepare(&[id.to_owned()]);
                    return Clip::Loading;
                }
            };
            let Ok(src) = self.ctx.create_buffer_source() else {
                return Clip::Missing;
            };
            src.set_buffer(Some(&buf));
            if src.connect_with_audio_node(&self.voice).is_err() {
                return Clip::Missing;
            }
            let t = self.ctx.current_time() + 0.02;
            let end = t + buf.duration();
            if let Some(old) = v.playing.take() {
                let node: &web_sys::AudioScheduledSourceNode = old.as_ref();
                let _ = node.stop();
            }
            let node: &web_sys::AudioScheduledSourceNode = src.as_ref();
            let _ = node.start_with_when(t);
            let g = self.duck.gain();
            let _ = g.cancel_scheduled_values(t);
            let _ = g.set_target_at_time(DUCK, t, 0.05);
            let _ = g.set_target_at_time(1.0, end, 0.25);
            v.playing = Some(src);
            v.playing_until = end;
            v.played += 1;
            Clip::Playing(buf.duration() as f32)
        }

        fn stop(&self) {
            let mut v = self.voices.borrow_mut();
            if let Some(old) = v.playing.take() {
                let node: &web_sys::AudioScheduledSourceNode = old.as_ref();
                let _ = node.stop();
                let now = self.ctx.current_time();
                if v.playing_until > now {
                    let g = self.duck.gain();
                    let _ = g.cancel_scheduled_values(now);
                    let _ = g.set_target_at_time(1.0, now, 0.1);
                }
            }
        }
    }

    /// `{"clips": {"<id>": {"text": ..}}}` → id → text.
    fn parse_index(bytes: &[u8]) -> HashMap<String, String> {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return HashMap::new();
        };
        let Ok(v) = js_sys::JSON::parse(text) else {
            return HashMap::new();
        };
        let mut out = HashMap::new();
        let Ok(clips) = js_sys::Reflect::get(&v, &"clips".into()) else {
            return out;
        };
        let Some(clips) = clips.dyn_ref::<js_sys::Object>() else {
            return out;
        };
        for entry in js_sys::Object::entries(clips).iter() {
            let pair: js_sys::Array = entry.unchecked_into();
            let id = pair.get(0).as_string();
            let text = js_sys::Reflect::get(&pair.get(1), &"text".into())
                .ok()
                .and_then(|t| t.as_string());
            if let (Some(id), Some(text)) = (id, text) {
                out.insert(id, text);
            }
        }
        out
    }

    /// The words of a clip, from the script.
    fn words(id: &str) -> Option<&'static str> {
        let mut it = id.rsplitn(3, '-');
        let line: usize = it.next()?.parse().ok()?;
        let bit: usize = it.next()?.parse().ok()?;
        let topic = it.next()?;
        sb_radio::topic(topic)
            .get(bit.checked_sub(1)?)?
            .get(line.checked_sub(1)?)
            .map(|(_, t)| *t)
    }

    /// For tests: `{"music": "ready", "musicDb": -23.1, "clips": 198,
    /// "played": 3}`: the music node's state and its level now (dBFS RMS,
    /// before the setting's volume), the clips listed, and the clips played.
    pub fn status() -> String {
        SFX.with(|s| {
            let s = s.borrow();
            let Some(s) = s.as_ref() else { return "{\"music\": \"no sound yet\"}".into() };
            let music = match &*s.radio.borrow() {
                MusicState::Idle => "idle",
                MusicState::Loading => "loading",
                MusicState::Ready(_) => "ready",
                MusicState::Script(_) => "ready (main thread)",
                MusicState::Failed => "failed",
            };
            let mut buf = vec![0f32; 2048];
            s.tap.get_float_time_domain_data(&mut buf);
            let rms = (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt();
            let v = s.voices.borrow();
            let decoded: Vec<String> = v
                .decoded
                .iter()
                .map(|(k, d)| {
                    let st = match d {
                        Decoded::Decoding => "decoding",
                        Decoded::Ready(_) => "ready",
                        Decoded::Failed => "failed",
                    };
                    format!("\"{k}\": \"{st}\"")
                })
                .collect();
            format!(
                "{{\"music\": \"{music}\", \"musicDb\": {:.1}, \"clips\": {}, \"fetched\": {}, \"played\": {}, \"last\": \"{}\", \"decoded\": {{{}}}, \"context\": \"{:?}\"}}",
                20.0 * rms.max(1e-6).log10(),
                v.index.as_ref().map_or(0, |i| i.len()),
                v.bytes.len(),
                v.played,
                v.last,
                decoded.join(", "),
                s.ctx.state()
            )
        })
    }

    pub fn voice_prepare(clips: &[String]) {
        SFX.with(|s| {
            if let Some(s) = s.borrow().as_ref() {
                s.prepare(clips);
            }
        });
    }

    pub fn voice_start(clip: &str) -> Clip {
        let Some(w) = words(clip) else {
            return Clip::Missing;
        };
        SFX.with(|s| {
            s.borrow()
                .as_ref()
                .map_or(Clip::Missing, |s| s.start(clip, w))
        })
    }

    pub fn voice_stop() {
        SFX.with(|s| {
            if let Some(s) = s.borrow().as_ref() {
                s.stop();
            }
        });
    }

    impl Sfx {
        fn tone(&self, kind: OscillatorType, f0: f32, f1: f32, at: f64, len: f64, vol: f32) {
            let Ok(o) = self.ctx.create_oscillator() else {
                return;
            };
            let Ok(g) = self.ctx.create_gain() else {
                return;
            };
            o.set_type(kind);
            let t = self.ctx.current_time() + at;
            let _ = o.frequency().set_value_at_time(f0, t);
            let _ = o
                .frequency()
                .exponential_ramp_to_value_at_time(f1.max(1.0), t + len);
            let _ = g.gain().set_value_at_time(0.0001, t);
            let _ = g.gain().exponential_ramp_to_value_at_time(vol, t + 0.008);
            let _ = g.gain().exponential_ramp_to_value_at_time(0.0001, t + len);
            let _ = o.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(&self.sfx);
            let _ = o.start_with_when(t);
            let _ = o.stop_with_when(t + len + 0.05);
        }

        fn burst(&self, f: f32, len: f64, vol: f32) {
            let Ok(src) = self.ctx.create_buffer_source() else {
                return;
            };
            let Ok(filter) = self.ctx.create_biquad_filter() else {
                return;
            };
            let Ok(g) = self.ctx.create_gain() else {
                return;
            };
            src.set_buffer(Some(&self.noise));
            filter.set_type(BiquadFilterType::Lowpass);
            filter.frequency().set_value(f);
            let t = self.ctx.current_time();
            let _ = g.gain().set_value_at_time(vol, t);
            let _ = g.gain().exponential_ramp_to_value_at_time(0.0001, t + len);
            let _ = src.connect_with_audio_node(&filter);
            let _ = filter.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(&self.sfx);
            let _ = src.start_with_when_and_grain_offset(t, (t * 0.37) % 1.5);
            let node: &web_sys::AudioScheduledSourceNode = src.as_ref();
            let _ = node.stop_with_when(t + len + 0.05);
        }
    }

    pub fn drive(
        run: Option<Res<Run>>,
        feed: Res<Feed>,
        setup: Res<Setup>,
        screen: Res<bevy::state::state::State<Screen>>,
    ) {
        SFX.with(|s| {
            let s = s.borrow();
            let Some(sfx) = s.as_ref() else { return };
            // A context not running (not yet woken, or Safari's
            // "interrupted": a call, the app in the background) keeps its
            // clock still, so the ramps below would pile up at one instant,
            // frame after frame, without end. Nothing is heard anyway.
            if sfx.ctx.state() != AudioContextState::Running {
                return;
            }
            let _ = &sfx.master;
            let now = sfx.ctx.current_time();
            let set = |p: &web_sys::AudioParam, v: f32| {
                let _ = p.set_target_at_time(v, now, 0.06);
            };

            // The radio.
            if setup.radio.music() {
                sfx.start_music();
            }
            if setup.radio.voices() {
                sfx.load_voices();
            }
            let _ = sfx.music.gain().set_target_at_time(
                if setup.radio.music() { MUSIC_VOL } else { 0.0 },
                now,
                0.3,
            );
            let energy = match (screen.get(), run.as_deref()) {
                (Screen::Menu, _) => 0.5,
                (Screen::Ride, Some(r)) if r.paused => 0.35,
                (Screen::Ride, Some(r)) if r.session.state == State::Done => 0.7,
                (Screen::Ride, Some(r)) if matches!(r.session.state, State::Countdown(_)) => 0.8,
                _ => 1.0,
            };
            sfx.set_energy(energy);

            let Some(run) = run.as_ref() else {
                set(&sfx.wind.gain.gain(), 0.0);
                set(&sfx.carve.gain.gain(), 0.0);
                return;
            };
            let r = &run.session.player().rider;
            let speed = if run.paused { 0.0 } else { r.speed() };
            let on_snow = r.phase != Phase::Air;
            set(&sfx.wind.gain.gain(), (speed / 30.0).powi(2).min(1.0) * 0.5);
            set(&sfx.wind.filter.frequency(), 250.0 + speed * 35.0);
            let carve = if on_snow && !run.paused {
                (r.spray * 0.6 + 0.12) * (speed / 20.0).min(1.0)
            } else {
                0.0
            };
            set(&sfx.carve.gain.gain(), carve * 0.45);
            set(
                &sfx.carve.filter.frequency(),
                1500.0 + r.skid * 1500.0 - r.lean.abs() * 500.0,
            );
            for ev in &feed.0 {
                match ev {
                    SessionEvent::Count(_) => {
                        sfx.tone(OscillatorType::Square, 660.0, 660.0, 0.0, 0.14, 0.12)
                    }
                    SessionEvent::Go => {
                        sfx.tone(OscillatorType::Square, 990.0, 990.0, 0.0, 0.4, 0.14)
                    }
                    SessionEvent::Rider(0, Event::Jump { .. }) => sfx.burst(900.0, 0.18, 0.25),
                    SessionEvent::Rider(0, Event::Land { landing, impact }) => {
                        let v = (impact / 12.0).clamp(0.15, 1.0);
                        sfx.tone(OscillatorType::Sine, 90.0, 40.0, 0.0, 0.2, 0.5 * v);
                        sfx.burst(1800.0, 0.25, 0.4 * v);
                        if *landing == Landing::Sloppy {
                            sfx.tone(OscillatorType::Sawtooth, 220.0, 150.0, 0.05, 0.25, 0.06);
                        }
                    }
                    SessionEvent::Rider(0, Event::Trick(t)) => {
                        let notes: &[f32] = if t.points > 1500 {
                            &[523.0, 659.0, 784.0, 1047.0]
                        } else {
                            &[659.0, 784.0, 988.0]
                        };
                        for (i, f) in notes.iter().enumerate() {
                            sfx.tone(
                                OscillatorType::Triangle,
                                *f,
                                *f,
                                i as f64 * 0.07,
                                0.22,
                                0.16,
                            );
                        }
                    }
                    SessionEvent::Rider(0, Event::Crash { .. }) => {
                        sfx.burst(600.0, 0.6, 0.7);
                        sfx.tone(OscillatorType::Sine, 70.0, 30.0, 0.0, 0.4, 0.6);
                    }
                    SessionEvent::Finish { who: 0, .. } => {
                        for (i, f) in [523.0, 659.0, 784.0, 1047.0, 1319.0].iter().enumerate() {
                            sfx.tone(
                                OscillatorType::Triangle,
                                *f,
                                *f,
                                i as f64 * 0.09,
                                0.35,
                                0.16,
                            );
                        }
                    }
                    _ => {}
                }
            }
            if run.session.state == State::Done {
                set(&sfx.carve.gain.gain(), 0.0);
            }
        });
    }
}
