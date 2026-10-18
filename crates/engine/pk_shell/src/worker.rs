//! A Web Worker executor (Midnight Playground DECISIONS D1215; its
//! `worker.rs` and `web/builder-worker.js`): a script that loads a small
//! wasm of its own and builds things off the main thread, asked with
//! generation-stamped messages and answered through an inbox a system
//! drains each frame.
//!
//! The protocol, which the game's worker script speaks:
//!
//! ```text
//! { type: 'plan',  generation, ...fields }  → { type: 'planned', generation } | { type: 'failed', generation, error }
//! { type: 'build', generation, key }        → { type: 'built', generation, key, ms, bytes } | { type: 'failed', generation, key, error }
//! ```
//!
//! `generation` stamps one job's requests, so a stale answer (a job that
//! was left) is told apart and dropped. One worker for the page's life,
//! started on the first [`Worker::available`]. The script failing to load
//! (not found, a syntax error), or its wasm trapping, marks it gone: every
//! request after fails at once, and the caller builds on its own thread.

use std::cell::RefCell;
use std::sync::Mutex;

use bevy::log::warn;
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;
use web_sys::MessageEvent;

/// What the worker answered.
pub enum Msg {
    Planned(u64),
    Built {
        generation: u64,
        key: i64,
        /// The worker's own time for the build, ms.
        ms: f64,
        bytes: Vec<u8>,
    },
    Failed {
        generation: u64,
        /// None: the plan failed (or the worker is gone).
        key: Option<i64>,
        error: String,
    },
}

struct Handle {
    worker: web_sys::Worker,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
}

thread_local! {
    static WORKER: RefCell<Option<Result<Handle, String>>> = const { RefCell::new(None) };
}
static INBOX: Mutex<Vec<Msg>> = Mutex::new(Vec::new());
/// The worker's script failed: nothing more from it.
static GONE: Mutex<bool> = Mutex::new(false);

/// The executor: the script's URL, relative to the page.
pub struct Worker {
    pub script: &'static str,
}

fn get(o: &JsValue, k: &str) -> JsValue {
    Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
}

fn num(o: &JsValue, k: &str) -> f64 {
    get(o, k).as_f64().unwrap_or(0.0)
}

fn push(m: Msg) {
    INBOX.lock().unwrap_or_else(|e| e.into_inner()).push(m);
}

fn start(script: &str) -> Result<Handle, String> {
    let worker =
        web_sys::Worker::new(script).map_err(|e| format!("starting the worker {script}: {e:?}"))?;
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let d = e.data();
        let generation = num(&d, "generation") as u64;
        match get(&d, "type").as_string().as_deref() {
            Some("planned") => push(Msg::Planned(generation)),
            Some("built") => {
                let bytes = js_sys::Uint8Array::new(&get(&d, "bytes")).to_vec();
                push(Msg::Built {
                    generation,
                    key: num(&d, "k") as i64,
                    ms: num(&d, "ms"),
                    bytes,
                });
            }
            Some("failed") => push(Msg::Failed {
                generation,
                key: get(&d, "k").as_f64().map(|k| k as i64),
                error: get(&d, "error")
                    .as_string()
                    .unwrap_or_else(|| "unknown".into()),
            }),
            _ => {}
        }
    });
    let script_name = script.to_owned();
    let on_error = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |e: web_sys::ErrorEvent| {
        // The script itself failed (not found, a syntax error): every
        // generation's requests are off.
        push(Msg::Failed {
            generation: 0,
            key: None,
            error: format!("{script_name}: {}", e.message()),
        });
        *GONE.lock().unwrap_or_else(|e| e.into_inner()) = true;
    });
    worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    Ok(Handle {
        worker,
        _on_message: on_message,
        _on_error: on_error,
    })
}

impl Worker {
    /// Whether the worker is there to ask (started on the first call).
    pub fn available(&self) -> bool {
        if *GONE.lock().unwrap_or_else(|e| e.into_inner()) {
            return false;
        }
        WORKER.with(|w| {
            let mut w = w.borrow_mut();
            if w.is_none() {
                let r = start(self.script);
                if let Err(e) = &r {
                    warn!("worker: {e}");
                }
                *w = Some(r);
            }
            w.as_ref().is_some_and(|r| r.is_ok())
        })
    }

    /// Asks the worker to plan (for this generation), with the game's own
    /// fields (which job).
    pub fn plan(&self, generation: u64, fields: &[(&str, &str)]) {
        let o = message("plan", generation);
        for (k, v) in fields {
            let _ = Reflect::set(&o, &JsValue::from_str(k), &JsValue::from_str(v));
        }
        post(&o);
    }

    /// Asks for the item under `key`.
    pub fn build(&self, generation: u64, key: i64) {
        let o = message("build", generation);
        let _ = Reflect::set(&o, &JsValue::from_str("k"), &JsValue::from_f64(key as f64));
        post(&o);
    }

    /// What the worker has answered since the last call.
    pub fn take(&self) -> Vec<Msg> {
        std::mem::take(&mut *INBOX.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

fn post(msg: &Object) {
    WORKER.with(|w| {
        if let Some(Ok(h)) = w.borrow().as_ref()
            && let Err(e) = h.worker.post_message(msg)
        {
            warn!("worker: posting: {e:?}");
        }
    });
}

fn message(kind: &str, generation: u64) -> Object {
    let o = Object::new();
    let _ = Reflect::set(&o, &JsValue::from_str("type"), &JsValue::from_str(kind));
    let _ = Reflect::set(
        &o,
        &JsValue::from_str("generation"),
        &JsValue::from_f64(generation as f64),
    );
    o
}
