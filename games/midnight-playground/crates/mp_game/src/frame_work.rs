//! The work each frame does, counted (docs/vision/peninsula.md, stage 6a;
//! DECISIONS D1238): the mesh, image and material assets added and their
//! bytes, the streamed sections' entities spawned and despawned, the time
//! the stream took on the main thread, the pipelines compiling, and what
//! the stream did (a section asked for, built, up, dropped).
//!
//! The stutter suite (`tools/parity/e2e/stutter.test.mjs`) reads it off
//! `window.__mp.frameWork`, an array of one entry a frame the page drains,
//! to say why a frame was late; the native work-budget tests read the
//! resource (`crate::stream::tests`), where no GPU times anything.

use bevy::prelude::*;

use crate::render::ThreeMaterial;
use crate::status::Status;

/// One frame's work. Reset at the start of the frame (`First`), counted
/// through it, published at its end (`Last`).
#[derive(Resource, Default, Debug, Clone)]
pub struct FrameWork {
    /// The frame (`Status::frames`, the page's `__mp.frames`).
    pub frame: u64,
    pub meshes: u32,
    pub mesh_bytes: u64,
    pub images: u32,
    pub image_bytes: u64,
    pub materials: u32,
    /// Instanced meshes' instance streams a streamed section made (not
    /// assets, so not in `mesh_bytes`).
    pub instance_bytes: u64,
    /// Entities a streamed section spawned this frame, and its dropped
    /// sections' (their roots' children).
    pub spawned: u32,
    pub despawned: u32,
    /// Milliseconds in the stream's systems: all of `stream::drive`, and
    /// within it the spawning, the worker's sections read back and the
    /// build stepped here.
    pub stream_ms: f32,
    pub spawn_ms: f32,
    pub unpack_ms: f32,
    pub step_ms: f32,
    /// Render pipelines still compiling (`Status::pipelines_waiting`).
    pub pipelines_waiting: u32,
    /// What the stream did: `ask 7`, `built 7`, `up 7`, `drop 3`, `warm 7`
    /// (its stand-ins went), `ring 6`.
    pub events: Vec<String>,
}

impl FrameWork {
    pub fn event(&mut self, e: impl Into<String>) {
        self.events.push(e.into());
    }

    /// Whether anything but the frame number is set.
    pub fn busy(&self) -> bool {
        self.meshes > 0
            || self.images > 0
            || self.materials > 0
            || self.spawned > 0
            || self.despawned > 0
            || self.stream_ms >= 0.5
            || self.pipelines_waiting > 0
            || !self.events.is_empty()
    }
}

/// The bytes a mesh hands the GPU: its vertex attributes and indices.
pub fn mesh_bytes(m: &Mesh) -> u64 {
    let v: usize = m.attributes().map(|(_, a)| a.get_bytes().len()).sum();
    let i = m.indices().map_or(0, |i| match i {
        bevy::mesh::Indices::U16(v) => v.len() * 2,
        bevy::mesh::Indices::U32(v) => v.len() * 4,
    });
    (v + i) as u64
}

fn begin(mut w: ResMut<FrameWork>, status: Res<Status>) {
    *w = FrameWork {
        frame: status.frames,
        ..FrameWork::default()
    };
}

/// The assets added this frame (before the render world extracts them,
/// which may drop the main world's copy).
fn count_assets(
    mut w: ResMut<FrameWork>,
    status: Res<Status>,
    mut mesh_ev: MessageReader<AssetEvent<Mesh>>,
    mut image_ev: MessageReader<AssetEvent<Image>>,
    mut material_ev: MessageReader<AssetEvent<ThreeMaterial>>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
) {
    for e in mesh_ev.read() {
        if let AssetEvent::Added { id } = e {
            w.meshes += 1;
            w.mesh_bytes += meshes.get(*id).map_or(0, mesh_bytes);
        }
    }
    for e in image_ev.read() {
        if let AssetEvent::Added { id } = e {
            w.images += 1;
            w.image_bytes += images
                .get(*id)
                .and_then(|i| i.data.as_ref())
                .map_or(0, |d| d.len() as u64);
        }
    }
    w.materials += material_ev
        .read()
        .filter(|e| matches!(e, AssetEvent::Added { .. }))
        .count() as u32;
    w.pipelines_waiting = status.pipelines_waiting as u32;
    w.frame = status.frames;
}

pub fn plugin(app: &mut App) {
    app.init_resource::<FrameWork>()
        .add_systems(First, begin)
        .add_systems(Last, count_assets);
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Last, web::publish.after(count_assets));
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::FrameWork;
    use crate::bridge;
    use bevy::prelude::*;
    use js_sys::{Array, Object, Reflect};
    use wasm_bindgen::{JsCast, JsValue};

    /// Entries kept when the page does not drain them.
    const KEEP: u32 = 1200;

    fn set(o: &Object, k: &str, v: impl Into<JsValue>) {
        let _ = Reflect::set(o, &JsValue::from_str(k), &v.into());
    }

    /// Appends the frame's entry to `__mp.frameWork` when it did anything
    /// (the page drains the array with `splice(0)`); the menu's hold
    /// (D1237) as `__mp.holding`, and the stream's sections as
    /// `__mp.stream` ({ at, sections: { k: state } }) when they change.
    pub fn publish(w: Res<FrameWork>, stream: Res<crate::stream::Stream>, mut last: Local<String>) {
        bridge::set("holding", crate::preview::holding());
        if stream.running() {
            let (at, list) = stream.states();
            let key = format!("{at:?}{list:?}{}", stream.started());
            if *last != key {
                *last = key;
                let o = Object::new();
                set(&o, "at", at.map_or(JsValue::NULL, |k| (k as f64).into()));
                set(&o, "started", stream.started());
                let secs = Object::new();
                for (k, s) in list {
                    set(&secs, &k.to_string(), s);
                }
                set(&o, "sections", secs);
                bridge::set("stream", o);
            }
        }
        if !w.busy() {
            return;
        }
        let list = match bridge::get("frameWork").and_then(|v| v.dyn_into::<Array>().ok()) {
            Some(a) => a,
            None => {
                let a = Array::new();
                bridge::set("frameWork", a.clone());
                a
            }
        };
        if list.length() >= KEEP {
            list.shift();
        }
        let o = Object::new();
        set(&o, "f", w.frame as f64);
        let nz = |o: &Object, k: &str, v: f64| {
            if v != 0.0 {
                set(o, k, v);
            }
        };
        nz(&o, "meshes", w.meshes.into());
        nz(&o, "meshKB", (w.mesh_bytes as f64 / 1024.0).round());
        nz(&o, "images", w.images.into());
        nz(&o, "imageKB", (w.image_bytes as f64 / 1024.0).round());
        nz(&o, "materials", w.materials.into());
        nz(&o, "instanceKB", (w.instance_bytes as f64 / 1024.0).round());
        nz(&o, "spawned", w.spawned.into());
        nz(&o, "despawned", w.despawned.into());
        let ms = |v: f32| (f64::from(v) * 10.0).round() / 10.0;
        nz(&o, "streamMs", ms(w.stream_ms));
        nz(&o, "spawnMs", ms(w.spawn_ms));
        nz(&o, "unpackMs", ms(w.unpack_ms));
        nz(&o, "stepMs", ms(w.step_ms));
        nz(&o, "pipelines", w.pipelines_waiting.into());
        if !w.events.is_empty() {
            let ev = Array::new();
            for e in &w.events {
                ev.push(&JsValue::from_str(e));
            }
            set(&o, "events", ev);
        }
        list.push(&o);
    }
}
