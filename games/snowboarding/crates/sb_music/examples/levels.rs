//! How loud the station plays (RMS and peak, dBFS) at a few energies and
//! wall times: for setting the music's level under the announcers.
//! `cargo run --release -p sb_music --example levels`
use sb_music::player::Player;
use sb_music::radio::{EPOCH, wall_parts};

fn main() {
    let sr = 48000.0;
    for energy in [1.0f32, 0.55] {
        for k in 0..4 {
            let wall = EPOCH + 9.0 * 86400.0 + 1000.0 + k as f64 * 3600.0;
            let (day, sec) = wall_parts(wall);
            let mut p = Player::new(2026, sr);
            p.set_params(0.0, day as f32, sec as f32, 1.0, energy);
            let (mut l, mut r) = (vec![0f32; 128], vec![0f32; 128]);
            let (mut sum, mut n, mut peak) = (0f64, 0usize, 0f32);
            for b in 0..(30.0 * sr / 128.0) as usize {
                p.process(&mut l, &mut r);
                if b as f64 * 128.0 > 3.0 * sr {
                    for x in l.iter().chain(&r) {
                        sum += (*x as f64).powi(2);
                        peak = peak.max(x.abs());
                        n += 1;
                    }
                }
            }
            let db = |x: f64| 20.0 * x.log10();
            println!(
                "energy {energy}: hour {k}: rms {:.1} dBFS, peak {:.1} dBFS",
                db((sum / n as f64).sqrt()),
                db(peak as f64)
            );
        }
    }
}
