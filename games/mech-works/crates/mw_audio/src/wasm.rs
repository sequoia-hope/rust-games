//! Standalone `wasm32-unknown-unknown` AudioWorklet entry points for `mw_audio`.
//!
//! No `wasm-bindgen` is used: the JS worklet passes packed `f32` telemetry frames
//! into linear memory and reads 128-frame stereo blocks back out.

use std::cell::RefCell;

use crate::{
    AudioFrame, FEET, FootFrame, ImpactEvent, JOINTS, JointFrame, MechAudio, RockEvent, StruckLink,
    UiEvent,
};

struct WorkletState {
    audio: MechAudio,
    frame: AudioFrame,
    out_l: [f32; 128],
    out_r: [f32; 128],
}

thread_local! {
    static STATE: RefCell<Option<WorkletState>> = const { RefCell::new(None) };
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_init(sample_rate: f32, seed: u32) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(WorkletState {
            audio: MechAudio::new(seed, f64::from(sample_rate)),
            frame: AudioFrame::default(),
            out_l: [0.0; 128],
            out_r: [0.0; 128],
        });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_set_body(
    mass_kg: f32,
    dust_storm: f32,
    terminated: u32,
    vx: f32,
    vy: f32,
    vz: f32,
    wx: f32,
    wy: f32,
    wz: f32,
) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.frame.body_mass_kg = mass_kg;
            st.frame.dust_storm = dust_storm;
            st.frame.terminated = terminated != 0;
            st.frame.torso_vel = [vx, vy, vz];
            st.frame.torso_ang_vel = [wx, wy, wz];
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_set_foot(
    foot: usize,
    touching: u32,
    normal_force: f32,
    slip_speed: f32,
    pan: f32,
) {
    if foot >= FEET {
        return;
    }
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.frame.feet[foot] = FootFrame {
                touching: touching != 0,
                normal_force,
                slip_speed,
                pan,
            };
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_set_joint(
    joint: usize,
    torque: f32,
    current: f32,
    max_current: f32,
    torque_limit: f32,
    temperature: f32,
    health: f32,
    angle: f32,
    velocity: f32,
    tripped: u32,
    fault_left: f32,
) {
    if joint >= JOINTS {
        return;
    }
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.frame.joints[joint] = JointFrame {
                torque,
                current,
                max_current,
                torque_limit,
                temperature,
                health,
                angle,
                velocity,
                tripped: tripped != 0,
                fault_left,
            };
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_push_impact(impulse: f32, link: u32, pan: f32) {
    let link = match link {
        0 => StruckLink::Torso,
        1 => StruckLink::Thigh,
        2 => StruckLink::Shin,
        _ => StruckLink::Foot,
    };
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.frame.impacts.push(ImpactEvent { impulse, link, pan });
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_push_ui(kind: u32) {
    let ev = match kind {
        1 => UiEvent::Damage,
        2 => UiEvent::PartSwap,
        3 => UiEvent::Objective,
        _ => UiEvent::Click,
    };
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.frame.ui.push(ev);
        }
    });
}

/// A rock knock on this step (the game's rubble).
#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_push_rock(impulse: f32, pan: f32) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.frame.rocks.push(RockEvent { impulse, pan });
        }
    });
}

/// How much loose rock is sliding and rolling, t·m/s at the listener.
#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_set_rubble(slide: f32) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.frame.rubble_slide = slide;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_commit_step() {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.audio.push_frame(st.frame.clone());
            st.frame.impacts.clear();
            st.frame.ui.clear();
            st.frame.rocks.clear();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_render_128() {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.audio.process(&mut st.out_l, &mut st.out_r);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_out_l_ptr() -> *const f32 {
    STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(std::ptr::null(), |st| st.out_l.as_ptr())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mw_audio_out_r_ptr() -> *const f32 {
    STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(std::ptr::null(), |st| st.out_r.as_ptr())
    })
}
