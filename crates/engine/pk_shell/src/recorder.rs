//! The run recording (Midnight Playground DECISIONS D1021, the format in its
//! `docs/rust-port/RECORDING.md`; its `recording.rs`): a JSON-lines log of
//! a session for an agent to examine afterwards, with the game's own
//! lines (a run's inputs and checkpoints, its settings, its sound) written
//! into the same file.
//!
//! `record=1` (`--record` natively) writes `recordings/<UTC date>-<name>.jsonl`
//! under the game's root natively (`record=<path>` names the file, or a
//! directory for it); the path is printed at the start and the end. On the
//! web the lines are kept in memory, read with [`recording_text`] (the game
//! exports it to the page as `recording()`) and saved as a download by the
//! page.
//!
//! What the shell writes (each line `{"t": seconds since start, "type":
//! …, …}`): the start (build, platform, commit, arguments, the game's
//! extras), the adapter, the loading status, the client's mode when it
//! changes ([`RecorderMode`]), one `frames` line a second (the frame
//! interval's count, mean, min, p99 and max, the main and render worlds'
//! CPU times from [`FrameTimes`], pipelines waiting), a `slow_frame` line
//! for every frame over [`SLOW_MS`], the window's focus, size and mode,
//! gamepad connections, the debug overlay toggled, the log's warnings and
//! errors (and the game's own info lines, natively), and the end. The game
//! writes its lines through [`RecordLine`] messages or straight into the
//! [`Recorder`] (its input chunks through [`InputLog`]).

use crate::debug::{FrameTimes, Overlay};
use crate::status::{LoadState, LoadStatus};
use crate::{FocusGained, FocusLost};
use bevy::app::AppExit;
use bevy::input::gamepad::GamepadConnectionEvent;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::PrimaryWindow;
use serde_json::{Value, json};
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

/// The format's version (the `start` line's `version`).
pub const VERSION: u32 = 1;
/// A frame over this is a `slow_frame` line (the status log's threshold,
/// D1005).
pub const SLOW_MS: f32 = 50.0;

static START: OnceLock<Instant> = OnceLock::new();
/// Recording is on: the log layer keeps lines for it.
static ON: AtomicBool = AtomicBool::new(false);
/// Log lines from any thread, with their time, until the next frame
/// writes them.
static LOG: Mutex<Vec<(f64, &'static str, String, String)>> = Mutex::new(Vec::new());
/// Targets whose info-level lines are kept too (the game's own crates).
static INFO_PREFIXES: RwLock<Vec<&'static str>> = RwLock::new(Vec::new());
/// The web's recording, for the page (`recording_text`).
#[cfg(target_arch = "wasm32")]
static WEB: Mutex<String> = Mutex::new(String::new());

/// Seconds since the recording started.
pub fn now() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// Keeps the info-level log lines of targets with these prefixes (a game's
/// `mp_` crates) in the recording, besides every warning and error.
pub fn keep_info_from(prefixes: &[&'static str]) {
    INFO_PREFIXES
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .extend_from_slice(prefixes);
}

/// Whether the log layer should keep a record of this level and target:
/// warnings and errors, and the info lines of the targets asked for.
pub fn wants_log(level: bevy::log::Level, target: &str) -> bool {
    ON.load(Ordering::Relaxed)
        && (level <= bevy::log::Level::WARN
            || (level == bevy::log::Level::INFO
                && INFO_PREFIXES
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .any(|p| target.starts_with(p))))
}

/// A log record (from the native log layer, `native::log_tally`).
pub fn log(level: bevy::log::Level, target: &str, message: &str) {
    let kind = match level {
        bevy::log::Level::ERROR => "error",
        bevy::log::Level::WARN => "warn",
        _ => "info",
    };
    let mut l = LOG.lock().unwrap_or_else(|e| e.into_inner());
    // A runaway logger must not fill the memory between two frames.
    if l.len() < 1000 {
        l.push((
            now(),
            kind,
            target.to_owned(),
            message.trim_end().to_owned(),
        ));
    }
}

/// `record=1|<path>`: where to write, if anywhere.
pub fn wanted(params: &crate::params::Params) -> Option<String> {
    params
        .get("record")
        .filter(|v| !v.is_empty() && *v != "0" && *v != "false")
        .map(str::to_owned)
}

/// A line the game writes: `{"t":…,"type":"<kind>", …json's fields}`.
#[derive(Message, Debug, Clone)]
pub struct RecordLine {
    pub kind: String,
    pub json: Value,
}

impl RecordLine {
    pub fn new(kind: impl Into<String>, json: Value) -> RecordLine {
        RecordLine {
            kind: kind.into(),
            json,
        }
    }
}

/// The client's mode as the game names it (`play/none`, `menu/menu`), for
/// the `frames`, `slow_frame` and `mode` lines; the load state when unset.
#[derive(Resource, Default, Debug, Clone)]
pub struct RecorderMode(pub Option<String>);

enum Out {
    #[cfg(not(target_arch = "wasm32"))]
    File(std::io::BufWriter<std::fs::File>),
    #[cfg(target_arch = "wasm32")]
    Web,
}

/// The second's frames.
#[derive(Default)]
struct Second {
    start: f64,
    frames: Vec<f32>,
    main_sum: f64,
    main_max: f32,
    render_sum: f64,
    render_max: f32,
    slow: u32,
    ticks: u32,
}

/// What was last written, to write changes only.
#[derive(Default)]
struct Seen {
    status: Option<LoadState>,
    ready: bool,
    mode: String,
    window: String,
    overlay: Option<bool>,
    adapter: bool,
}

#[derive(Resource)]
pub struct Recorder {
    out: Out,
    path: String,
    line: String,
    sec: Second,
    seen: Seen,
    frames: u64,
    slow: u64,
    lines: u64,
    bytes: u64,
    flushed_at: f64,
    polled_at: f64,
    ended: bool,
}

impl Recorder {
    /// Writes `{"t":…,"type":"kind",…}`: `rest` is the line's other fields,
    /// as a JSON object (its braces dropped) or `None`.
    pub fn write(&mut self, t: f64, kind: &str, rest: Option<Value>) {
        let mut l = std::mem::take(&mut self.line);
        l.clear();
        let _ = write!(l, "{{\"t\":{t:.4},\"type\":\"{kind}\"");
        if let Some(Value::Object(m)) = rest
            && !m.is_empty()
        {
            let s = Value::Object(m).to_string();
            l.push(',');
            l.push_str(&s[1..]);
        } else {
            l.push('}');
        }
        self.raw(&l);
        self.line = l;
    }

    /// One whole line, as given.
    pub fn raw(&mut self, l: &str) {
        self.lines += 1;
        self.bytes += l.len() as u64 + 1;
        match &mut self.out {
            #[cfg(not(target_arch = "wasm32"))]
            Out::File(f) => {
                use std::io::Write;
                let _ = f.write_all(l.as_bytes());
                let _ = f.write_all(b"\n");
            }
            #[cfg(target_arch = "wasm32")]
            Out::Web => {
                let mut w = WEB.lock().unwrap_or_else(|e| e.into_inner());
                w.push_str(l);
                w.push('\n');
            }
        }
    }

    /// Simulation ticks run this frame (for the `frames` line's `ticks`).
    pub fn add_ticks(&mut self, n: u32) {
        self.sec.ticks += n;
    }

    /// Lines written so far.
    pub fn lines(&self) -> u64 {
        self.lines
    }

    /// Whether the `end` line has been written.
    pub fn ended(&self) -> bool {
        self.ended
    }

    fn flush(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::io::Write;
            let Out::File(f) = &mut self.out;
            let _ = f.flush();
        }
    }

    /// The `end` line, once.
    fn end(&mut self, why: &str) {
        if self.ended {
            return;
        }
        self.ended = true;
        let t = now();
        self.write(
            t,
            "end",
            Some(json!({
                "why": why,
                "seconds": round3(t),
                "frames": self.frames,
                "slow_frames": self.slow,
                "lines": self.lines + 1,
            })),
        );
        self.flush();
        ON.store(false, Ordering::Relaxed);
        let msg = format!(
            "recording: wrote {} ({} lines, {:.0} KB)",
            self.path,
            self.lines,
            self.bytes as f64 / 1024.0
        );
        info!("{msg}");
        #[cfg(not(target_arch = "wasm32"))]
        println!("{msg}");
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.end("dropped");
    }
}

/// `x` to three decimals.
pub fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn r2(x: f32) -> f64 {
    (f64::from(x) * 100.0).round() / 100.0
}

/// A date and time in UTC from the system clock: (`20261005-221500`,
/// `2026-10-05T22:15:00Z`). Days to a civil date as in H. Hinnant's
/// `civil_from_days`.
#[cfg(not(target_arch = "wasm32"))]
pub fn utc_now() -> (String, String) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let (hh, mm, ss) = (rem / 3600, rem / 60 % 60, rem % 60);
    (
        format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}"),
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"),
    )
}

/// Where a native recording goes: `record=1` → `recordings/` under the
/// game's root; a path ending in `.jsonl` is the file; any other path is a
/// directory for it.
#[cfg(not(target_arch = "wasm32"))]
fn native_path(want: &str, root: &std::path::Path, name: &str, stamp: &str) -> PathBuf {
    let file = format!("{stamp}Z-{name}.jsonl");
    match want {
        "1" | "true" => root.join("recordings").join(file),
        p if p.ends_with(".jsonl") => PathBuf::from(p),
        dir => std::path::Path::new(dir).join(file),
    }
}

/// The session recorder, when `want` (the `record=` value) asks for it.
pub struct RecorderPlugin {
    /// The `record=` value (`recorder::wanted`).
    pub want: String,
    /// The file's name part (`<date>-<name>.jsonl`): the scene, the course.
    pub name: String,
    /// The game's root natively: `recordings/` goes under it, and the
    /// commit is read there.
    pub root: PathBuf,
    /// The `start` line's `banner`.
    pub banner: String,
    /// The `start` line's `file` on the web (how the page reads it).
    pub web_label: String,
    /// The game's own `start` fields (its scene, options, tick rate, chunk).
    pub extra: Value,
    /// The game's parameters, written as `query`.
    pub query: Vec<(String, String)>,
}

impl Plugin for RecorderPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RecordLine>()
            .init_resource::<RecorderMode>();
        let t0 = now();
        #[cfg(not(target_arch = "wasm32"))]
        let (out, path, date) = {
            let (stamp, date) = utc_now();
            let path = native_path(&self.want, &self.root, &self.name, &stamp);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::File::create(&path) {
                Ok(f) => (
                    Out::File(std::io::BufWriter::with_capacity(64 * 1024, f)),
                    path.display().to_string(),
                    date,
                ),
                Err(e) => {
                    eprintln!("recording: {}: {e}; not recording", path.display());
                    return;
                }
            }
        };
        #[cfg(target_arch = "wasm32")]
        let (out, path, date) = {
            WEB.lock().unwrap_or_else(|e| e.into_inner()).clear();
            (Out::Web, self.web_label.clone(), String::new())
        };
        ON.store(true, Ordering::Relaxed);
        let mut rec = Recorder {
            out,
            path: path.clone(),
            line: String::with_capacity(4096),
            sec: Second {
                start: t0,
                frames: Vec::with_capacity(512),
                ..default()
            },
            seen: Seen::default(),
            frames: 0,
            slow: 0,
            lines: 0,
            bytes: 0,
            flushed_at: t0,
            polled_at: f64::NEG_INFINITY,
            ended: false,
        };
        let query: Vec<Value> = self.query.iter().map(|(k, v)| json!([k, v])).collect();
        #[cfg(not(target_arch = "wasm32"))]
        let args: Vec<String> = std::env::args().skip(1).collect();
        #[cfg(target_arch = "wasm32")]
        let args: Vec<String> = Vec::new();
        // The checkout's commit, for a replay on the same code (natively).
        #[cfg(not(target_arch = "wasm32"))]
        let commit = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
        #[cfg(target_arch = "wasm32")]
        let commit: Option<String> = None;
        let mut start = json!({
            "version": VERSION,
            "banner": self.banner,
            "build": if cfg!(debug_assertions) { "debug" } else { "release" },
            "platform": crate::platform(),
            "date_utc": date,
            "commit": commit,
            "file": path,
            "args": args,
            "query": query,
        });
        // The game's fields, then the threshold, in the order the format
        // document lists them.
        if let Value::Object(s) = &mut start {
            if let Value::Object(e) = &self.extra {
                for (k, v) in e {
                    s.insert(k.clone(), v.clone());
                }
            }
            s.insert("slow_ms".into(), json!(SLOW_MS));
        }
        rec.write(t0, "start", Some(start));
        rec.flush();
        let msg = format!("recording to {path}");
        info!("{msg}");
        #[cfg(not(target_arch = "wasm32"))]
        println!("{msg}");
        app.insert_resource(rec)
            .add_systems(Last, frame.after(crate::debug::frame_end));
    }
}

/// Inputs to a simulation, chunked into `ticks` lines: every tick's input
/// since the last line, the state's hash after the last of them when the
/// state is at hand, and the simulation's events. The game encodes the
/// inputs (its own format, so its replay tool reads them unchanged) and
/// names the id key (`run`).
pub struct InputLog<I> {
    /// The key the id is written under (`"run"`).
    pub id_key: &'static str,
    pub id: u32,
    /// The state's tick when last seen.
    pub tick: u32,
    /// The first tick of the inputs not yet written.
    pub from: u32,
    pub inputs: Vec<I>,
    pub events: Vec<(u32, String)>,
    /// Ticks per line.
    pub chunk: usize,
}

impl<I: Clone> InputLog<I> {
    pub fn new(id_key: &'static str, id: u32, chunk: usize) -> InputLog<I> {
        InputLog {
            id_key,
            id,
            tick: 0,
            from: 1,
            inputs: Vec::with_capacity(chunk + 8),
            events: Vec::new(),
            chunk,
        }
    }

    /// The inputs that took the state from `self.tick` to `tick`; returns
    /// how many ticks were expected and how many inputs came, when they
    /// differ (a `gap` the recording notes).
    pub fn advance(&mut self, tick: u32, inputs: &[I]) -> Option<(usize, usize)> {
        if tick <= self.tick {
            return None;
        }
        let n = (tick - self.tick) as usize;
        self.inputs.extend_from_slice(inputs);
        self.tick = tick;
        (inputs.len() != n).then_some((n, inputs.len()))
    }

    /// Whether a line is due (a chunk of inputs waits).
    pub fn due(&self) -> bool {
        self.inputs.len() >= self.chunk
    }

    /// The inputs not yet written as a `ticks` line; `None` when there are
    /// none. `encode` writes the inputs; `check` is the state's hash after
    /// the last of them and a fragment of further fields (`,"k":v,…`),
    /// given when the state is at hand at that tick.
    pub fn line(
        &mut self,
        t: f64,
        encode: &dyn Fn(&[I], &mut String),
        check: Option<(u64, &str)>,
    ) -> Option<String> {
        if self.inputs.is_empty() {
            return None;
        }
        let mut l = String::with_capacity(self.inputs.len() * 12 + 512);
        let to = self.from + self.inputs.len() as u32 - 1;
        let _ = write!(
            l,
            "{{\"t\":{t:.4},\"type\":\"ticks\",\"{}\":{},\"from\":{},\"tick\":{to}",
            self.id_key, self.id, self.from
        );
        if let Some((hash, extra)) = check {
            let _ = write!(l, ",\"hash\":\"{hash:016x}\"");
            l.push_str(",\"in\":\"");
            encode(&self.inputs, &mut l);
            l.push('"');
            l.push_str(extra);
        } else {
            l.push_str(",\"in\":\"");
            encode(&self.inputs, &mut l);
            l.push('"');
        }
        if !self.events.is_empty() {
            l.push_str(",\"ev\":");
            let ev: Vec<Value> = self.events.iter().map(|(k, e)| json!([k, e])).collect();
            l.push_str(&Value::Array(ev).to_string());
        }
        l.push('}');
        self.from = to + 1;
        self.inputs.clear();
        self.events.clear();
        Some(l)
    }
}

/// What the rest of the client is doing, read once a frame.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Seen2<'w, 's> {
    status: Res<'w, LoadStatus>,
    ft: Res<'w, FrameTimes>,
    mode: Res<'w, RecorderMode>,
    overlay: Option<Res<'w, Overlay>>,
    adapter: Option<Res<'w, RenderAdapterInfo>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

/// The frame's timing, every second's summary, the changes, and the game's
/// lines. A game system that writes [`RecordLine`]s runs `.before(
/// recorder::frame)` in `Last` to have them in the same frame.
#[allow(clippy::too_many_arguments)]
pub fn frame(
    mut rec: ResMut<Recorder>,
    w: Seen2,
    mut lines: MessageReader<RecordLine>,
    mut lost: MessageReader<FocusLost>,
    mut gained: MessageReader<FocusGained>,
    mut pads: MessageReader<GamepadConnectionEvent>,
    mut exit: MessageReader<AppExit>,
) {
    let rec = &mut *rec;
    if rec.ended {
        return;
    }
    let t = now();
    let st = &*w.status;
    let ft = *w.ft;
    let mode = || {
        w.mode
            .0
            .clone()
            .unwrap_or_else(|| st.state.name().to_string())
    };
    rec.frames += 1;
    // The first frames' intervals are the start-up's.
    if st.frames > 2 {
        let s = &mut rec.sec;
        s.frames.push(ft.frame_ms);
        s.main_sum += f64::from(ft.main_ms);
        s.main_max = s.main_max.max(ft.main_ms);
        s.render_sum += f64::from(ft.render_ms);
        s.render_max = s.render_max.max(ft.render_ms);
        // `ms` is the interval that ended as this frame began; `main_ms`
        // this frame's own work, which shows in the next frame's interval.
        if ft.frame_ms > SLOW_MS || ft.main_ms > SLOW_MS {
            s.slow += 1;
            rec.slow += 1;
            rec.write(
                t,
                "slow_frame",
                Some(json!({
                    "frame": st.frames,
                    "ms": r2(ft.frame_ms),
                    "main_ms": r2(ft.main_ms),
                    "render_ms": r2(ft.render_ms),
                    "mode": mode(),
                    "pipelines_waiting": st.pipelines_waiting,
                })),
            );
        }
    }
    if t - rec.sec.start >= 1.0 {
        let mut s = std::mem::take(&mut rec.sec);
        let n = s.frames.len();
        if n > 0 {
            let sum: f64 = s.frames.iter().map(|x| f64::from(*x)).sum();
            let min = s.frames.iter().copied().fold(f32::INFINITY, f32::min);
            let max = s.frames.iter().copied().fold(0.0, f32::max);
            let k = ((n as f64 * 0.99).ceil() as usize).clamp(1, n) - 1;
            let p99 = *s.frames.select_nth_unstable_by(k, f32::total_cmp).1;
            rec.write(
                t,
                "frames",
                Some(json!({
                    "n": n,
                    "fps": round3(n as f64 / (t - s.start)),
                    "mean": round3(sum / n as f64),
                    "min": r2(min),
                    "p99": r2(p99),
                    "max": r2(max),
                    "main_mean": round3(s.main_sum / n as f64),
                    "main_max": r2(s.main_max),
                    "render_mean": round3(s.render_sum / n as f64),
                    "render_max": r2(s.render_max),
                    "slow": s.slow,
                    "ticks": s.ticks,
                    "mode": mode(),
                    "pipelines_waiting": st.pipelines_waiting,
                    "late_frames": st.late_frames,
                })),
            );
        }
        s.frames.clear();
        rec.sec = Second {
            start: t,
            frames: s.frames,
            ..default()
        };
    }
    // The loading status.
    if rec.seen.status != Some(st.state) || st.ready != rec.seen.ready {
        rec.seen.status = Some(st.state);
        rec.seen.ready = st.ready;
        rec.write(
            t,
            "status",
            Some(json!({
                "state": st.state.name(),
                "ready": st.ready,
                "frame": st.frames,
                "scenes": st.scenes,
                "error": st.error,
            })),
        );
    }
    let m = mode();
    if m != rec.seen.mode {
        rec.write(t, "mode", Some(json!({"mode": m, "frame": st.frames})));
        rec.seen.mode = m;
    }
    if !rec.seen.adapter
        && let Some(a) = &w.adapter
    {
        rec.seen.adapter = true;
        rec.write(
            t,
            "adapter",
            Some(json!({
                "name": a.name,
                "backend": format!("{:?}", a.backend),
                "device_type": format!("{:?}", a.device_type),
                "driver": a.driver,
                "driver_info": a.driver_info,
            })),
        );
    }
    for _ in lost.read() {
        rec.write(t, "focus", Some(json!({"focused": false})));
    }
    for _ in gained.read() {
        rec.write(t, "focus", Some(json!({"focused": true})));
    }
    for p in pads.read() {
        rec.write(
            t,
            "gamepad",
            Some(json!({"gamepad": format!("{:?}", p.gamepad), "connection": format!("{:?}", p.connection)})),
        );
    }
    if let Some(ov) = &w.overlay
        && rec.seen.overlay != Some(ov.on)
    {
        rec.seen.overlay = Some(ov.on);
        rec.write(t, "overlay", Some(json!({"on": ov.on})));
    }
    // The game's lines.
    for l in lines.read() {
        rec.write(t, &l.kind, Some(l.json.clone()));
    }
    // The log's lines since the last frame.
    let logs = std::mem::take(&mut *LOG.lock().unwrap_or_else(|e| e.into_inner()));
    for (lt, kind, target, msg) in logs {
        rec.write(
            lt,
            "log",
            Some(json!({"level": kind, "target": target, "msg": msg})),
        );
    }
    // Four times a second: the window, by what it says about itself.
    if t - rec.polled_at >= 0.25 {
        rec.polled_at = t;
        if let Ok(win) = w.windows.single() {
            let sig = format!(
                "{}x{} {:.2} {:?} {}",
                win.physical_width(),
                win.physical_height(),
                win.scale_factor(),
                win.mode,
                win.focused
            );
            if sig != rec.seen.window {
                rec.write(
                    t,
                    "window",
                    Some(json!({
                        "width": win.physical_width(),
                        "height": win.physical_height(),
                        "scale": win.scale_factor(),
                        "mode": format!("{:?}", win.mode),
                        "focused": win.focused,
                    })),
                );
                rec.seen.window = sig;
            }
        }
    }
    if exit.read().next().is_some() {
        rec.end("exit");
        return;
    }
    if t - rec.flushed_at >= 1.0 {
        rec.flushed_at = t;
        rec.flush();
    }
}

/// The recording so far (the page's `recording()`).
#[cfg(target_arch = "wasm32")]
pub fn recording_text() -> String {
    WEB.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `ticks` line: the id under its key, the range, the hash and the
    /// extra fields when checked, the events; the next line starts after.
    #[test]
    fn ticks_lines_chunk_and_check() {
        let mut log: InputLog<u8> = InputLog::new("run", 3, 4);
        assert_eq!(log.advance(2, &[1, 2]), None);
        assert_eq!(log.advance(4, &[3]), Some((2, 1)));
        assert!(!log.due());
        log.advance(5, &[4]);
        assert!(log.due());
        log.events.push((5, "Start".into()));
        let enc = |v: &[u8], out: &mut String| {
            for x in v {
                out.push((b'a' + x) as char);
            }
        };
        let l = log.line(1.5, &enc, Some((0xabc, ",\"p\":1"))).unwrap();
        assert_eq!(
            l,
            "{\"t\":1.5000,\"type\":\"ticks\",\"run\":3,\"from\":1,\"tick\":4,\"hash\":\"0000000000000abc\",\"in\":\"bcde\",\"p\":1,\"ev\":[[5,\"Start\"]]}"
        );
        assert_eq!(log.from, 5);
        assert!(log.line(2.0, &enc, None).is_none());
        log.advance(6, &[0]);
        assert_eq!(
            log.line(2.0, &enc, None).unwrap(),
            "{\"t\":2.0000,\"type\":\"ticks\",\"run\":3,\"from\":5,\"tick\":5,\"in\":\"a\"}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_date_is_a_civil_date() {
        let (stamp, date) = utc_now();
        assert_eq!(stamp.len(), 15);
        assert!(date.ends_with('Z') && date.starts_with("20"), "{date}");
        let root = std::path::Path::new("/r");
        assert_eq!(
            native_path("1", root, "coast", "20261005-221500"),
            PathBuf::from("/r/recordings/20261005-221500Z-coast.jsonl")
        );
        assert_eq!(
            native_path("x.jsonl", root, "coast", "s"),
            PathBuf::from("x.jsonl")
        );
        assert_eq!(
            native_path("out", root, "coast", "s"),
            PathBuf::from("out/sZ-coast.jsonl")
        );
    }

    #[test]
    fn record_params() {
        use crate::params::Params;
        assert_eq!(
            wanted(&Params::from_query("record=1")).as_deref(),
            Some("1")
        );
        assert_eq!(wanted(&Params::from_query("record=0")), None);
        assert_eq!(wanted(&Params::from_query("x=1")), None);
    }
}
