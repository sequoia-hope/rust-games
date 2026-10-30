//! Renders a song to a WAV file, with no game and no browser:
//! `cargo run --release -p music --example render -- <genre> <seed> [out.wav] [--classic] [--secs N]`.
//! A genre of the new generator (`synthwave`, `outrun`, `chicago`, `deep`)
//! or, with `--classic`, of the classic one (`house`, `techno`, …). Prints
//! the song's line, its loudness and peak, and its layers.

use music::dsp::Reverb;
use music::engine::Engine;
use music::generator::{Settings, generate};
use music::instruments::{TriggerOpt, make_instrument};
use music::patches::bpatch;
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
    let patches_arg: Option<String> = flag("--patch").and_then(|i| args.get(i + 1)).cloned();
    let pos: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            !a.starts_with("--")
                && flag("--secs").is_none_or(|s| *i != s + 1)
                && flag("--patch").is_none_or(|p| *i != p + 1)
        })
        .map(|(_, a)| a)
        .collect();

    if let Some(patch_list) = patches_arg {
        let out = pos
            .first()
            .map_or_else(|| "patches.wav".to_string(), |s| (*s).clone());
        let mut data = Vec::new();
        for name in patch_list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let start_s = data.len() as f64 / 2.0 / SR;
            render_patch_audition(name, &mut data);
            let end_s = data.len() as f64 / 2.0 / SR;
            println!("  {:<16} {:>5.1}s .. {:>5.1}s", name, start_s, end_s);
        }
        write_wav(&out, &data);
        return;
    }
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

fn render_patch_audition(name: &str, data: &mut Vec<f32>) {
    let lab = bpatch(name).unwrap_or_else(|| panic!("unknown patch: {name}"));
    let mut inst = make_instrument(lab, 7, SR);
    let mut rev = Reverb::new(7, SR);
    rev.set(1.6, 0.45);
    let step_s = 0.125; // 120 BPM 16th note
    // Each event: (at_16th, dur_16th, midis, vel, accent, slide)
    let events: Vec<(f64, f64, Vec<f64>, f64, bool, bool)> = match name {
        "acid" | "sawBass" | "pluckBass" | "moogBass" | "reese" | "sub" | "wobble" | "dxBass"
        | "fmBass" | "organBass" | "fingerBass" | "uprightBass" | "darkBass" | "roundBass"
        | "tranceBass" | "psyBass" | "rumble" | "contrabass" => vec![
            (0.0, 1.8, vec![33.0], 0.92, true, false),
            (2.0, 0.9, vec![33.0], 0.68, false, false),
            (3.0, 1.8, vec![45.0], 0.85, true, true),
            (5.0, 0.9, vec![40.0], 0.72, false, false),
            (6.0, 1.8, vec![36.0], 0.88, false, false),
            (8.0, 2.8, vec![38.0], 0.90, true, false),
            (11.0, 1.0, vec![40.0], 0.75, false, true),
            (12.0, 3.5, vec![33.0], 0.94, true, false),
        ],
        "acousticGuitar" | "nylonGuitar" | "leadGuitar" | "surfGuitar" | "wahGuitar"
        | "rhythmGuitar" | "telecaster" | "baritoneGuitar" | "banjo" | "mandolin" => vec![
            // Bar 1: Fingerstyle arpeggio with open ringing strings (Am9 -> Fmaj7#11)
            (0.0, 7.6, vec![45.0], 0.88, false, false),
            (1.02, 6.2, vec![52.0], 0.68, false, false),
            (2.04, 5.2, vec![57.0], 0.74, false, false),
            (3.01, 4.2, vec![60.0], 0.78, false, false),
            (3.98, 4.0, vec![64.0], 0.86, false, false),
            (5.03, 3.0, vec![60.0], 0.64, false, false),
            (6.02, 2.5, vec![67.0], 0.82, false, false),
            (7.04, 2.0, vec![64.0], 0.66, false, false),
            (8.0, 7.6, vec![41.0], 0.86, false, false),
            (9.03, 6.2, vec![48.0], 0.68, false, false),
            (10.02, 5.2, vec![57.0], 0.75, false, false),
            (11.01, 4.2, vec![60.0], 0.78, false, false),
            (12.0, 4.0, vec![64.0], 0.90, true, false),
            (13.52, 2.5, vec![62.0], 0.76, false, true),
            (15.02, 2.0, vec![60.0], 0.66, false, false),
            // Bar 2: Human strummed chords (downstrokes + upstrokes) + melodic turnaround
            (
                16.0,
                3.6,
                vec![48.0, 52.0, 55.0, 60.0, 64.0],
                0.88,
                false,
                false,
            ),
            (
                18.96,
                1.4,
                vec![48.0, 52.0, 55.0, 60.0, 64.0],
                0.66,
                false,
                false,
            ),
            (
                20.0,
                2.8,
                vec![43.0, 47.0, 50.0, 55.0, 59.0, 67.0],
                0.90,
                false,
                false,
            ),
            (
                22.02,
                1.6,
                vec![43.0, 47.0, 50.0, 55.0, 59.0, 67.0],
                0.68,
                false,
                false,
            ),
            (24.0, 2.2, vec![64.0], 0.84, false, false),
            (25.48, 1.8, vec![67.0], 0.80, false, true),
            (27.0, 2.2, vec![69.0], 0.92, true, true),
            (
                29.0,
                6.5,
                vec![45.0, 52.0, 57.0, 60.0, 64.0],
                0.88,
                false,
                false,
            ),
        ],
        "superPad" | "warmPad" | "darkPad" | "superPadWide" | "choir" | "supersaw" | "hoover"
        | "stringSection" | "celloPad" | "brassSection" | "stab" | "hit" | "dubChord" => vec![
            (
                0.0,
                6.5,
                vec![45.0, 52.0, 57.0, 60.0, 64.0],
                0.85,
                false,
                false,
            ),
            (
                8.0,
                6.5,
                vec![41.0, 48.0, 53.0, 57.0, 60.0],
                0.85,
                false,
                false,
            ),
            (
                16.0,
                6.5,
                vec![48.0, 55.0, 60.0, 64.0, 67.0],
                0.88,
                false,
                false,
            ),
        ],
        "piano" | "honkyPiano" | "ep" | "organ" | "comboOrgan" | "bell" | "glass" | "pluck"
        | "sqArp" | "pluckTrance" | "modernPluck" => vec![
            (0.0, 3.8, vec![45.0, 57.0, 60.0, 64.0], 0.85, false, false),
            (2.0, 1.8, vec![67.0], 0.78, false, false),
            (4.0, 3.8, vec![41.0, 53.0, 57.0, 60.0], 0.82, false, false),
            (6.0, 1.8, vec![64.0], 0.84, false, false),
            (8.0, 3.8, vec![48.0, 55.0, 60.0, 64.0], 0.88, false, false),
            (10.0, 1.8, vec![67.0], 0.80, false, false),
            (12.0, 4.5, vec![43.0, 55.0, 59.0, 62.0], 0.86, false, false),
        ],
        _ => vec![
            (0.0, 2.8, vec![57.0], 0.85, false, false),
            (3.0, 1.0, vec![60.0], 0.78, false, false),
            (4.0, 3.5, vec![64.0], 0.90, true, true),
            (8.0, 2.0, vec![67.0], 0.88, false, false),
            (10.0, 2.0, vec![64.0], 0.80, false, true),
            (12.0, 1.8, vec![62.0], 0.82, false, false),
            (14.0, 5.0, vec![57.0], 0.90, true, true),
        ],
    };
    let last_step = events
        .iter()
        .map(|(at, dur, _, _, _, _)| at + dur)
        .fold(16.0f64, f64::max)
        + 6.0;
    let total_samples = (last_step * step_s * SR) as usize;
    let mut ev_idx = 0;
    let mut prev_midi = None;
    for s in 0..total_samples {
        let t_s = s as f64 / SR;
        while ev_idx < events.len() && events[ev_idx].0 * step_s <= t_s {
            let (_, dur, ref midis, vel, accent, slide) = events[ev_idx];
            inst.trigger(
                midis,
                vel,
                dur * step_s * SR,
                &TriggerOpt {
                    accent,
                    glide_from: if slide { prev_midi } else { None },
                },
            );
            prev_midi = midis.first().copied();
            ev_idx += 1;
        }
        let mut dry = [0.0; 2];
        inst.run(&mut dry);
        let mut wet = [0.0; 2];
        rev.run(dry[0], dry[1], &mut wet);
        let sl = (dry[0] * 0.85 + wet[0] * 0.14).clamp(-1.0, 1.0) as f32;
        let sr = (dry[1] * 0.85 + wet[1] * 0.14).clamp(-1.0, 1.0) as f32;
        data.push(sl);
        data.push(sr);
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
