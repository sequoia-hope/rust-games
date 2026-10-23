//! Offline WAV renderer for `mw_audio` simulation telemetry traces.
//!
//! Usage:
//!   cargo run -p mw_audio --release --example render_traces -- [out_dir]
//!
//! Renders four trace WAVs at 44.1 kHz stereo 16-bit PCM:
//! - `walk_nominal.wav`: Demo 2 nominal 4.0 t walking gait
//! - `walk_heavy.wav`: Demo 4 heavy 5.8 t torso replacement walking gait
//! - `limp_and_fault.wav`: Demo 3 knee damage limp, 90–120 °C derate buzz, 120 °C trip & cool-down hiss, and overcurrent fault pop
//! - `combat_and_fall.wav`: Demo 1 projectile impacts (shin, thigh, torso), gravel slide + recovery stomp, and fall crash

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use mw_audio::traces::{combat_and_fall_trace, limp_and_fault_trace, render_trace, walk_trace};

const SAMPLE_RATE: u32 = 44_100;

fn write_wav(path: &std::path::Path, samples: &[[f32; 2]]) -> std::io::Result<()> {
    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let byte_rate = SAMPLE_RATE * u32::from(num_channels) * u32::from(bits_per_sample / 8);
    let block_align = num_channels * (bits_per_sample / 8);
    let data_bytes = (samples.len() * 4) as u32;
    let riff_size = 36 + data_bytes;

    let mut buf = Vec::with_capacity(44 + data_bytes as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&riff_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&num_channels.to_le_bytes());
    buf.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_bytes.to_le_bytes());

    for &[l, r] in samples {
        let li = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
        let ri = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
        buf.extend_from_slice(&li.to_le_bytes());
        buf.extend_from_slice(&ri.to_le_bytes());
    }

    let mut file = fs::File::create(path)?;
    file.write_all(&buf)
}

fn stats(samples: &[[f32; 2]]) -> (f32, f32) {
    let mut peak = 0.0_f32;
    let mut sum_sq = 0.0_f64;
    for &[l, r] in samples {
        peak = peak.max(l.abs()).max(r.abs());
        sum_sq += (f64::from(l) * f64::from(l) + f64::from(r) * f64::from(r)) * 0.5;
    }
    let rms = (sum_sq / samples.len().max(1) as f64).sqrt() as f32;
    (peak, rms)
}

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/mw_audio"));
    fs::create_dir_all(&out_dir).expect("create output directory");

    let cases = [
        ("walk_nominal.wav", walk_trace(5.0, 4000.0, 0.15), 0xA001u64),
        ("walk_heavy.wav", walk_trace(5.0, 5800.0, 0.15), 0xA001u64),
        ("limp_and_fault.wav", limp_and_fault_trace(7.0), 0xB002u64),
        ("combat_and_fall.wav", combat_and_fall_trace(6.8), 0xC003u64),
    ];

    for (name, trace, seed) in cases {
        let samples = render_trace(&trace, SAMPLE_RATE as f32, seed);
        let (peak, rms) = stats(&samples);
        let path = out_dir.join(name);
        write_wav(&path, &samples).expect("write wav");
        println!(
            "wrote {} ({:.2}s, peak={:.3}, rms={:.4})",
            path.display(),
            samples.len() as f32 / SAMPLE_RATE as f32,
            peak,
            rms
        );
    }
}
