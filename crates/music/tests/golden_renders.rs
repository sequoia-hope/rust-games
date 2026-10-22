//! The engine against the lab's renders (`tests/golden/renders.json`,
//! written by `node tools/music-lab/test/golden.mjs`): a few tracks from a
//! given bar, as RMS per 2400 samples and whole 256-sample windows. The lab
//! uses `Math.*` and this crate the platform's `f64` functions, which may
//! differ in the last bit; the filters are stable, so the renders agree to
//! far below anything audible. A run that differs by much more is a port
//! bug: find the first differing window sample and work back.

use music::engine::Engine;
use music::genres::genre;
use serde_json::Value;
use std::path::PathBuf;

fn file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/renders.json")
}

/// Base64 of little-endian `f32`s.
fn b64(s: &str) -> Vec<f32> {
    let val = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a') as u32 + 26,
            b'0'..=b'9' => (c - b'0') as u32 + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("bad base64"),
        }
    };
    let s = s.trim_end_matches('=').as_bytes();
    let mut bytes = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c) << (18 - 6 * i);
        }
        let take = chunk.len() - 1;
        for i in 0..take {
            bytes.push((n >> (16 - 8 * i)) as u8);
        }
    }
    bytes
        .chunks(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn to_b64(xs: &[f32]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::with_capacity(xs.len() * 4);
    for x in xs {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let b0 = c[0] as u32;
        let b1 = c.get(1).copied().unwrap_or(0) as u32;
        let b2 = c.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

const RMS: usize = 2400;
/// RMS within 0.2 % relative or 2e-5 absolute, whichever is larger.
const RMS_REL: f64 = 0.002;
const RMS_ABS: f64 = 2e-5;
/// Window samples within 2e-4 of full scale.
const WIN_ABS: f64 = 2e-4;

/// The worst differences of one run: (relative RMS, where), (absolute
/// window sample, where).
struct Worst {
    rms: (f64, String),
    win: (f64, String),
    fail: Vec<String>,
}

fn check(run: &mut Value, regen: bool) -> Worst {
    let g = run["genre"].as_str().unwrap().to_owned();
    let seed = run["seed"].as_u64().unwrap() as u32;
    let bar = run["bar"].as_u64().unwrap() as u32;
    let energy = run["energy"].as_f64().unwrap();
    let engine_seed = run["engineSeed"].as_u64().unwrap() as u32;
    let rate = run["rate"].as_f64().unwrap();
    let n = run["samples"].as_u64().unwrap() as usize;
    let name = format!("{g} {seed} bar {bar} energy {energy}");

    let t = (genre(&g).unwrap_or_else(|| panic!("genre {g}")).make)(seed, None);
    let mut e = Engine::new(engine_seed, rate);
    e.set_track(&t);
    e.set_energy(energy);
    e.play(bar, 0.02);
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    let mut i = 0;
    while i < n {
        e.process(&mut l[i..i + 128], &mut r[i..i + 128]);
        i += 128;
    }

    let mut w = Worst {
        rms: (0.0, String::new()),
        win: (0.0, String::new()),
        fail: Vec::new(),
    };
    if regen {
        for (side, x) in [("rmsL", &l), ("rmsR", &r)] {
            let blocks: Vec<f32> = (0..n / RMS)
                .map(|k| {
                    let s = &x[k * RMS..(k + 1) * RMS];
                    (s.iter().map(|&v| v as f64 * v as f64).sum::<f64>() / RMS as f64).sqrt() as f32
                })
                .collect();
            run[side] = Value::String(to_b64(&blocks));
        }
        for win in run["windows"].as_array_mut().unwrap() {
            let start = win["start"].as_u64().unwrap() as usize;
            win["l"] = Value::String(to_b64(&l[start..start + 256]));
            win["r"] = Value::String(to_b64(&r[start..start + 256]));
        }
        return w;
    }
    for (side, x) in [("rmsL", &l), ("rmsR", &r)] {
        let want = b64(run[side].as_str().unwrap());
        assert_eq!(want.len(), n / RMS, "{name}: {side} blocks");
        for (k, &want) in want.iter().enumerate() {
            let s = &x[k * RMS..(k + 1) * RMS];
            let got = (s.iter().map(|&v| v as f64 * v as f64).sum::<f64>() / RMS as f64).sqrt();
            let want = want as f64;
            let diff = (got - want).abs();
            let rel = diff / want.max(1e-12);
            let at = format!(
                "{name}: {side} block {k} ({:.2} s): {got} vs {want}",
                (k * RMS) as f64 / rate
            );
            if rel > w.rms.0 {
                w.rms = (rel, at.clone());
            }
            if diff > (RMS_REL * want).max(RMS_ABS) {
                w.fail.push(at);
            }
        }
    }
    for win in run["windows"].as_array().unwrap() {
        let start = win["start"].as_u64().unwrap() as usize;
        for (side, x) in [("l", &l), ("r", &r)] {
            let want = b64(win[side].as_str().unwrap());
            assert_eq!(want.len(), 256, "{name}: window length");
            for (k, &v) in want.iter().enumerate() {
                let got = x[start + k];
                let diff = (got as f64 - v as f64).abs();
                let at = format!(
                    "{name}: {side}[{}] ({:.4} s): {got} vs {v}",
                    start + k,
                    (start + k) as f64 / rate
                );
                if diff > w.win.0 {
                    w.win = (diff, at.clone());
                }
                if diff > WIN_ABS {
                    w.fail.push(at);
                }
            }
        }
    }
    w
}

#[test]
fn every_render_matches_the_lab() {
    let regen = std::env::var_os("REGEN_GOLDENS").is_some();
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(file()).unwrap()).unwrap();
    let runs = doc.as_array_mut().unwrap();
    assert_eq!(runs.len(), 19, "the lab's render runs");
    let mut worst_rms = (0.0, String::new());
    let mut worst_win = (0.0, String::new());
    let mut fails = Vec::new();
    for run in runs.iter_mut() {
        let w = check(run, regen);
        eprintln!(
            "{} {} bar {}: worst RMS {:.3e} rel, worst window {:.3e}{}",
            run["genre"].as_str().unwrap(),
            run["seed"],
            run["bar"],
            w.rms.0,
            w.win.0,
            if w.fail.is_empty() {
                String::new()
            } else {
                format!(" ({} out of tolerance, first: {})", w.fail.len(), w.fail[0])
            }
        );
        if w.rms.0 > worst_rms.0 {
            worst_rms = w.rms;
        }
        if w.win.0 > worst_win.0 {
            worst_win = w.win;
        }
        fails.extend(w.fail);
    }
    if regen {
        std::fs::write(file(), serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }
    eprintln!("worst RMS: {} ({:.3e} relative)", worst_rms.1, worst_rms.0);
    eprintln!("worst window sample: {} ({:.3e})", worst_win.1, worst_win.0);
    assert!(
        fails.is_empty(),
        "{} values out of tolerance; first: {}\nworst RMS: {}\nworst window: {}",
        fails.len(),
        fails[0],
        worst_rms.1,
        worst_win.1
    );
}
