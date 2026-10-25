//! Synthetic simulation telemetry traces (`AudioFrame` sequences at 50 Hz policy rate)
//! for offline rendering, automated tests, and ear-verification of `mw_audio`.
//!
//! Each trace models the physical numbers produced by `mw_sim` across the four
//! tech-demo scenarios in `games/mech-works/docs/SPEC.md`:
//! - [`walk_trace`]: Demo 2 & Demo 4 — bipedal walking gait (configurable body mass
//!   so nominal 4 t vs heavy 5.6 t torso replacement can be compared directly).
//! - [`limp_and_fault_trace`]: Demo 3 — actuator damage injection on the left knee,
//!   asymmetric limping gait, winding heat rise into thermal derate (90–120 °C),
//!   120 °C thermal trip clunk + cool-down hiss, and a weak-driver overcurrent fault.
//! - [`combat_and_fall_trace`]: Demo 1 — projectile impacts (light rounds on limbs,
//!   heavy shell on torso causing foot slide and recovery step, and an overwhelming
//!   salvo that topples the mech into a fall crash).

use std::f32::consts::TAU;

use crate::{
    AMBIENT_C, AudioFrame, FootFrame, ImpactEvent, JOINTS, JointFrame, MechAudio, StruckLink,
    TRIP_C, UiEvent,
};

/// Policy-rate step frequency (Hz) used by trace generators.
pub const TRACE_HZ: usize = 50;

/// Render a sequence of [`AudioFrame`]s at [`TRACE_HZ`] into stereo `[L, R]` samples.
pub fn render_trace(frames: &[AudioFrame], sample_rate: f32, seed: u64) -> Vec<[f32; 2]> {
    let mut audio = MechAudio::new(seed as u32, f64::from(sample_rate));
    let samples_per_frame = (sample_rate / TRACE_HZ as f32).round() as usize;
    let mut out = Vec::with_capacity(frames.len() * samples_per_frame);
    let mut buf_l = vec![0.0f32; samples_per_frame];
    let mut buf_r = vec![0.0f32; samples_per_frame];
    for frame in frames {
        audio.push_frame(frame.clone());
        audio.process(&mut buf_l, &mut buf_r);
        for (&l, &r) in buf_l.iter().zip(&buf_r) {
            out.push([l, r]);
        }
    }
    out
}

/// Demo 2 & Demo 4: Bipedal walking gait at ~1.25 Hz step cadence (0.8 s per step,
/// 1.6 s full stride cycle) for `duration_s` seconds with configurable `body_mass_kg`.
pub fn walk_trace(duration_s: f32, body_mass_kg: f32, dust_storm: f32) -> Vec<AudioFrame> {
    let total_frames = (duration_s * TRACE_HZ as f32).round() as usize;
    let dt = 1.0 / TRACE_HZ as f32;
    let stride_freq = 0.625; // 1.25 steps/s
    let weight_n = body_mass_kg * 9.81;
    let mass_scale = body_mass_kg / 4000.0;

    let mut frames = Vec::with_capacity(total_frames);
    for i in 0..total_frames {
        let t = i as f32 * dt;
        let phase = (t * stride_freq).fract(); // 0..1 full stride

        // Left foot stance in [0.0, 0.55), swing in [0.55, 1.0)
        // Right foot stance in [0.50, 1.0) U [0.0, 0.05), swing in [0.05, 0.50)
        let left_phase = phase;
        let right_phase = (phase + 0.5).fract();

        let foot_state = |p: f32, pan: f32| -> FootFrame {
            let touching = p < 0.55;
            if !touching {
                FootFrame {
                    touching: false,
                    normal_force: 0.0,
                    slip_speed: 0.0,
                    pan,
                }
            } else {
                // Impact spike at touchdown (p in 0..0.06), double-hump stance force
                let stance_u = p / 0.55;
                let impact_spike = if stance_u < 0.10 {
                    1.55 * (1.0 - stance_u / 0.10)
                } else {
                    0.0
                };
                let arch = (stance_u * std::f32::consts::PI).sin();
                let normal_force = weight_n * (0.55 + 0.55 * arch + impact_spike);
                // Small scuff on touchdown and push-off
                let slip_speed = if stance_u < 0.08 {
                    0.35 * (1.0 - stance_u / 0.08) * mass_scale.sqrt()
                } else if stance_u > 0.88 {
                    0.22 * ((stance_u - 0.88) / 0.12)
                } else {
                    0.0
                };
                FootFrame {
                    touching: true,
                    normal_force,
                    slip_speed,
                    pan,
                }
            }
        };

        let mut joints = [JointFrame::default(); JOINTS];
        for leg in 0..2 {
            let p = if leg == 0 { left_phase } else { right_phase };
            let theta = p * TAU;
            let base = leg * 6;

            // Hip pitch (2) and knee (3) carry the main sagittal gait load;
            // hip roll (1) stabilizes stance; ankle pitch (4) pushes off.
            let hip_pitch_vel = 2.6 * theta.cos();
            let knee_vel = -3.8 * (theta * 2.0).sin();
            let ankle_vel = 2.1 * (theta + 0.8).cos();
            let hip_roll_vel = 0.9 * theta.sin();

            let stance_load = if p < 0.55 { 1.0 } else { 0.35 };
            let impact_load = if p < 0.06 { 1.45 } else { 1.0 };

            // Hip roll
            joints[base + 1] = JointFrame {
                torque: 420.0 * stance_load * mass_scale * theta.sin().abs().max(0.4),
                current: 42.0 * stance_load * mass_scale,
                max_current: 90.0,
                torque_limit: 900.0,
                temperature: 38.0 + 4.0 * mass_scale,
                health: 1.0,
                angle: 0.08 * theta.cos(),
                velocity: hip_roll_vel,
                tripped: false,
                fault_left: 0.0,
            };
            // Hip pitch
            joints[base + 2] = JointFrame {
                torque: 680.0 * stance_load * impact_load * mass_scale,
                current: 58.0 * stance_load * impact_load * mass_scale,
                max_current: 110.0,
                torque_limit: 1200.0,
                temperature: 42.0 + 6.0 * mass_scale,
                health: 1.0,
                angle: 0.42 * theta.sin(),
                velocity: hip_pitch_vel,
                tripped: false,
                fault_left: 0.0,
            };
            // Knee (works hardest on touchdown absorption and stance extension)
            joints[base + 3] = JointFrame {
                torque: 680.0 * stance_load * impact_load * mass_scale,
                current: 58.0 * stance_load * impact_load * mass_scale,
                max_current: 110.0,
                torque_limit: 1200.0,
                temperature: 46.0 + 8.0 * mass_scale,
                health: 1.0,
                angle: -0.65 * (0.5 - 0.5 * theta.cos()),
                velocity: knee_vel,
                tripped: false,
                fault_left: 0.0,
            };
            // Ankle pitch
            joints[base + 4] = JointFrame {
                torque: 360.0 * stance_load * mass_scale,
                current: 36.0 * stance_load * mass_scale,
                max_current: 80.0,
                torque_limit: 700.0,
                temperature: 36.0 + 4.0 * mass_scale,
                health: 1.0,
                angle: 0.25 * theta.sin(),
                velocity: ankle_vel,
                tripped: false,
                fault_left: 0.0,
            };
        }

        // Torso sways and bobs twice per stride
        let sway = (phase * TAU).cos();
        let bob = (phase * TAU * 2.0).sin();

        frames.push(AudioFrame {
            joints,
            feet: [foot_state(left_phase, -0.35), foot_state(right_phase, 0.35)],
            torso_vel: [1.35, 0.22 * sway, 0.28 * bob * mass_scale],
            torso_ang_vel: [0.26 * sway, 0.32 * bob * mass_scale, 0.14 * sway],
            body_mass_kg,
            terminated: false,
            dust_storm,
            impacts: Vec::new(),
            ui: Vec::new(),
            rocks: Vec::new(),
            rubble_slide: 0.0,
        });
    }
    frames
}

/// Demo 3: Fault & damage tolerance trace (~7.0 s).
/// - `0.0..1.5 s`: normal walk
/// - `1.5 s`: UI Damage click reduces left knee (`joint 3`) health to `0.45`; gait becomes
///   an asymmetric limp (left foot lands early/hard with a slide; right knee works overtime).
/// - `1.8..4.5 s`: overloaded right knee (`joint 9`) heats from 55 °C into the 90–120 °C
///   derating zone (audible thermal buzz + harmonic strain).
/// - `4.5 s`: right knee hits 120 °C and trips (`tripped = true`, metallic breaker clunk +
///   cool-down hiss).
/// - `5.4 s`: weak driver on left hip pitch (`joint 2`) hits a 250 ms overcurrent fault
///   (`fault_left = 0.25`, sharp electrical pop + dropout).
pub fn limp_and_fault_trace(duration_s: f32) -> Vec<AudioFrame> {
    let mut frames = walk_trace(duration_s, 4000.0, 0.2);
    let dt = 1.0 / TRACE_HZ as f32;
    let mut right_knee_temp = 48.0_f32;
    let mut right_knee_tripped = false;

    for (i, frame) in frames.iter_mut().enumerate() {
        let t = i as f32 * dt;

        // Inject UI damage click at t = 1.50 s
        if i == (1.50 * TRACE_HZ as f32).round() as usize {
            frame.ui.push(UiEvent::Damage);
        }

        if t >= 1.50 {
            // Left knee (joint 3) damaged: lower torque limit, higher strain & slip
            let lk = &mut frame.joints[3];
            lk.health = 0.45;
            lk.torque_limit = 540.0;
            lk.torque = lk.torque.min(530.0);
            lk.current = 88.0; // straining against degraded torque limit
            lk.temperature = 78.0;

            // Asymmetric limp: left foot slaps down harder and scrapes
            if frame.feet[0].touching {
                frame.feet[0].normal_force *= 1.28;
                frame.feet[0].slip_speed = (frame.feet[0].slip_speed + 0.45).min(1.2);
            }
            // Torso lurches on the damaged side
            frame.torso_ang_vel[0] *= 1.85;
            frame.torso_ang_vel[1] *= 1.65;

            // Right knee (joint 9) compensates for the damaged left leg and heats rapidly
            if !right_knee_tripped {
                right_knee_temp += 26.5 * dt;
                if right_knee_temp >= TRIP_C {
                    right_knee_temp = TRIP_C;
                    right_knee_tripped = true;
                }
            } else {
                // Cooling down while tripped
                right_knee_temp = (right_knee_temp - 12.0 * dt).max(AMBIENT_C);
            }

            let rk = &mut frame.joints[9];
            rk.temperature = right_knee_temp;
            rk.tripped = right_knee_tripped;
            if right_knee_tripped {
                rk.torque = 0.0;
                rk.current = 0.0;
                rk.torque_limit = 0.0;
            } else {
                // Derating between 90°C and 120°C squeezes torque_limit down while current stays high
                let derate = ((right_knee_temp - 90.0) / 30.0).clamp(0.0, 0.92);
                rk.torque_limit = 1200.0 * (1.0 - derate);
                rk.torque = (rk.torque * 1.35).min(rk.torque_limit * 0.98);
                rk.current = 104.0;
                rk.max_current = 110.0;
            }
        }

        // Weak driver overcurrent fault on left hip pitch (joint 2) at t = 5.40..5.65 s
        if (5.40..5.65).contains(&t) {
            let lhp = &mut frame.joints[2];
            lhp.fault_left = 5.65 - t;
            lhp.torque = 0.0;
            lhp.current = 0.0;
        }
    }

    frames
}

/// Demo 1: Standing balance under projectile fire, recovery step, and eventual fall (~6.5 s).
/// - `0.0..1.0 s`: braced standing in hillside wind (`dust_storm = 0.55`)
/// - `1.0 s`: light projectile strikes left shin (`impulse = 220 N·s`) — bright high hull ring
/// - `2.2 s`: medium projectile strikes right thigh (`impulse = 520 N·s`) — mid hull ring + servo brace
/// - `3.4 s`: heavy cannon shell strikes torso (`impulse = 1150 N·s`) — deep torso ring,
///   feet slide across gravel (`slip_speed = 1.4 m/s`), left foot lifts and takes a heavy
///   recovery stomp at `3.85 s`.
/// - `5.0 s`: overwhelming double salvo on torso (`1450 N·s` + `1600 N·s`), joints strain at
///   saturation, and at `5.45 s` the mech topples (`terminated = true` fall crash).
pub fn combat_and_fall_trace(duration_s: f32) -> Vec<AudioFrame> {
    let total_frames = (duration_s * TRACE_HZ as f32).round() as usize;
    let dt = 1.0 / TRACE_HZ as f32;
    let body_mass_kg = 4000.0;
    let weight_n = body_mass_kg * 9.81;

    let mut frames = Vec::with_capacity(total_frames);
    let mut terminated = false;

    for i in 0..total_frames {
        let t = i as f32 * dt;
        let mut joints = [JointFrame::default(); JOINTS];
        let mut feet = [
            FootFrame {
                touching: true,
                normal_force: weight_n * 0.5,
                slip_speed: 0.0,
                pan: -0.35,
            },
            FootFrame {
                touching: true,
                normal_force: weight_n * 0.5,
                slip_speed: 0.0,
                pan: 0.35,
            },
        ];
        let mut impacts = Vec::new();
        let mut torso_vel = [0.0_f32; 3];
        let mut torso_ang_vel = [0.05 * (t * 2.0).sin(), 0.04 * (t * 1.7).cos(), 0.0];

        // Light hit at 1.0 s (left shin)
        if i == (1.00 * TRACE_HZ as f32).round() as usize {
            impacts.push(ImpactEvent {
                impulse: 240.0,
                link: StruckLink::Shin,
                pan: -0.45,
            });
        }
        // Medium hit at 2.2 s (right thigh)
        if i == (2.20 * TRACE_HZ as f32).round() as usize {
            impacts.push(ImpactEvent {
                impulse: 560.0,
                link: StruckLink::Thigh,
                pan: 0.40,
            });
        }
        // Heavy shell at 3.4 s (torso)
        if i == (3.40 * TRACE_HZ as f32).round() as usize {
            impacts.push(ImpactEvent {
                impulse: 1180.0,
                link: StruckLink::Torso,
                pan: 0.10,
            });
        }
        // Overwhelming salvo at 5.0 s and 5.12 s
        if i == (5.00 * TRACE_HZ as f32).round() as usize {
            impacts.push(ImpactEvent {
                impulse: 1450.0,
                link: StruckLink::Torso,
                pan: -0.15,
            });
        }
        if i == (5.12 * TRACE_HZ as f32).round() as usize {
            impacts.push(ImpactEvent {
                impulse: 1600.0,
                link: StruckLink::Torso,
                pan: 0.20,
            });
        }

        // Post-hit corrective servo bursts & foot reaction
        if (1.00..1.35).contains(&t) {
            let decay = 1.0 - (t - 1.00) / 0.35;
            torso_ang_vel[1] = 0.8 * decay;
            for j in [2, 3, 4] {
                joints[j].torque = 620.0 * decay;
                joints[j].current = 55.0 * decay;
                joints[j].velocity = 2.4 * decay * ((t * 28.0).sin());
            }
        } else if (2.20..2.70).contains(&t) {
            let decay = 1.0 - (t - 2.20) / 0.50;
            torso_vel[0] = -0.65 * decay;
            torso_ang_vel[0] = -1.2 * decay;
            feet[1].slip_speed = 0.55 * decay;
            for j in [7, 8, 9, 10] {
                joints[j].torque = 890.0 * decay;
                joints[j].current = 82.0 * decay;
                joints[j].velocity = 3.2 * decay * ((t * 22.0).cos());
            }
        } else if (3.40..4.30).contains(&t) {
            // Heavy shell pushes torso back, both feet skid on gravel, then left foot
            // lifts (3.55..3.85 s) and stomps down hard at 3.85 s to catch balance.
            let elapsed = t - 3.40;
            let decay = (1.0 - elapsed / 0.90).max(0.0);
            torso_vel[0] = -1.45 * decay;
            torso_ang_vel[1] = -2.1 * decay;
            if elapsed < 0.22 {
                feet[0].slip_speed = 1.35 * (1.0 - elapsed / 0.22);
                feet[1].slip_speed = 1.50 * (1.0 - elapsed / 0.22);
            }
            if (0.15..0.45).contains(&elapsed) {
                // Left foot in the air swinging back to catch the mech
                feet[0].touching = false;
                feet[0].normal_force = 0.0;
                feet[1].normal_force = weight_n * 1.15;
            } else if (0.45..0.58).contains(&elapsed) {
                // Heavy recovery stomp!
                feet[0].touching = true;
                feet[0].normal_force = weight_n * 1.75;
                feet[0].slip_speed = 0.65 * (1.0 - (elapsed - 0.45) / 0.13);
            }
            for (idx, j) in joints.iter_mut().enumerate() {
                j.torque = 1080.0 * decay;
                j.current = 98.0 * decay;
                j.max_current = 110.0;
                j.torque_limit = 1200.0;
                j.velocity = 4.2 * decay * ((t * 18.0 + idx as f32).sin());
                j.temperature = 52.0 + 18.0 * (1.0 - decay);
            }
        } else if (5.00..5.45).contains(&t) {
            // Desperate saturation before toppling
            torso_vel = [-2.4, 0.9, -1.8 * ((t - 5.0) / 0.45)];
            torso_ang_vel = [2.8, -3.5, 1.1];
            feet[0].slip_speed = 1.6;
            feet[1].touching = false;
            feet[1].normal_force = 0.0;
            for (idx, j) in joints.iter_mut().enumerate() {
                j.torque = 1180.0;
                j.current = 109.0;
                j.max_current = 110.0;
                j.torque_limit = 1200.0;
                j.velocity = 5.0 * ((t * 25.0 + idx as f32).cos());
                j.temperature = 84.0;
            }
        } else if t >= 5.45 {
            // Fall crash on the ground
            terminated = true;
            feet[0].touching = false;
            feet[0].normal_force = 0.0;
            feet[1].touching = false;
            feet[1].normal_force = 0.0;
            let post_fall = (1.0 - (t - 5.45) / 0.6).max(0.0);
            torso_vel = [-2.8 * post_fall, 0.8 * post_fall, -3.2 * post_fall];
            torso_ang_vel = [3.0 * post_fall, -4.0 * post_fall, 0.5 * post_fall];
        }

        frames.push(AudioFrame {
            joints,
            feet,
            torso_vel,
            torso_ang_vel,
            body_mass_kg,
            terminated,
            dust_storm: 0.55,
            impacts,
            ui: Vec::new(),
            rocks: Vec::new(),
            rubble_slide: 0.0,
        });
    }

    frames
}
