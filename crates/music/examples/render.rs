//! Renders a song to a WAV file, with no game and no browser:
//! `cargo run --release -p music --example render -- <genre> <seed> [out.wav] [--classic] [--secs N]`.
//! A genre of the new generator (`synthwave`, `outrun`, `chicago`, `deep`)
//! or, with `--classic`, of the classic one (`house`, `techno`, …). Prints
//! the song's line, its loudness and peak, and its layers.

use music::engine::Engine;
use music::generator::{Settings, generate};
use music::song::SongPlayer;

const SR: f64 = 48000.0;
const BLOCK: usize = 128;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().position(|a| a == f);
    let classic = flag("--classic").is_some();
    let secs: Option<f64> = flag("--secs")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());
    let pos: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && flag("--secs").is_none_or(|s| *i != s + 1))
        .map(|(_, a)| a)
        .collect();
    let genre = pos.first().map_or("synthwave", |s| s.as_str());
    let seed: u32 = pos.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let out = pos
        .get(2)
        .map_or_else(|| format!("{genre}-{seed}.wav"), |s| s.to_string());

    let mut data = Vec::new();
    let mut l = vec![0f32; BLOCK];
    let mut r = vec![0f32; BLOCK];
    if !classic && let Some(s) = generate(genre, seed, &Settings::default()) {
        println!(
            "{} · {} · {} bpm, swing {} %, {} bars",
            s.title, s.style, s.bpm, s.swing, s.bars
        );
        for layer in &s.layers {
            let entry = layer.clips.iter().map(|c| c.bar + 1).min().unwrap_or(0);
            println!(
                "  {:<7} enters bar {:>3}, {} clips",
                layer.name,
                entry,
                layer.clips.len()
            );
        }
        let len = secs.unwrap_or(s.secs());
        let mut p = SongPlayer::new(5, SR);
        p.set_song(s);
        p.play(0, 0.0);
        for _ in 0..(len * SR / BLOCK as f64) as usize {
            p.process(&mut l, &mut r);
            interleave(&mut data, &l, &r);
        }
    } else {
        let t = music::lab::generate(genre, seed, None).expect("a known genre");
        println!(
            "{} · {} · {} bpm",
            t.title.as_deref().unwrap_or(""),
            t.style.as_deref().unwrap_or(""),
            t.bpm
        );
        let len = secs.unwrap_or(t.bars() as f64 * 16.0 * 60.0 / t.bpm / 4.0);
        let mut e = Engine::new(5, SR);
        e.set_track(&t);
        e.play(0, 0.0);
        for _ in 0..(len * SR / BLOCK as f64) as usize {
            e.process(&mut l, &mut r);
            interleave(&mut data, &l, &r);
        }
    }
    let rms =
        (data.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / data.len().max(1) as f64).sqrt();
    let peak = data.iter().fold(0f32, |m, x| m.max(x.abs()));
    println!(
        "rms {:.1} dBFS, peak {:.1} dBFS, {:.0} s → {out}",
        20.0 * rms.log10(),
        20.0 * (peak as f64).log10(),
        data.len() as f64 / 2.0 / SR
    );
    write_wav(&out, &data);
}

fn interleave(data: &mut Vec<f32>, l: &[f32], r: &[f32]) {
    for i in 0..l.len() {
        data.push(l[i]);
        data.push(r[i]);
    }
}

fn write_wav(path: &str, data: &[f32]) {
    let mut b = Vec::with_capacity(44 + data.len() * 2);
    let bytes = (data.len() * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + bytes).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&(SR as u32).to_le_bytes());
    b.extend_from_slice(&(SR as u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&bytes.to_le_bytes());
    for x in data {
        let s = (x.clamp(-1.0, 1.0) * 32767.0) as i16;
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, b).expect("write the wav");
}
