//! The new generator's mix balance: in a synthwave chorus the arp and the
//! lead are heard against the bass and the pads stay a bed under it,
//! measured K-weighted as LUFS meters do (ITU-R BS.1770).

use music::generator::{Settings, generate};
use music::song::SongPlayer;

/// A biquad, direct form I.
struct Bq {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Bq {
    fn new(b: [f64; 3], a: [f64; 2]) -> Bq {
        Bq {
            b,
            a,
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// BS.1770's K-weighting at 48 kHz: the head's shelf, then the RLB
/// high-pass.
fn k_weighting() -> (Bq, Bq) {
    (
        Bq::new(
            [1.53512485958697, -2.69169618940638, 1.19839281085285],
            [-1.69065929318241, 0.73248077421585],
        ),
        Bq::new([1.0, -2.0, 1.0], [-1.99004745483398, 0.99007225036621]),
    )
}

/// A layer's loudness (LUFS, ungated) over 4 bars from the first chorus or
/// main section; `None` when the song has no such layer.
fn loudness(genre: &str, seed: u32, solo: &str) -> Option<f64> {
    let s = generate(genre, seed, &Settings::default()).unwrap();
    s.layers.iter().find(|l| l.name == solo)?;
    let chorus = s
        .sections
        .iter()
        .find(|x| matches!(x.name.as_str(), "chorus" | "main"))
        .unwrap()
        .bar;
    let mut pl = SongPlayer::new(5, 48000.0);
    let n = (4.0 * 240.0 / s.bpm * 48000.0 / 128.0) as usize;
    pl.set_song(s);
    pl.set_solo(Some(solo.to_owned()));
    pl.play(chorus + 2, 0.0);
    let (mut l, mut r) = (vec![0f32; 128], vec![0f32; 128]);
    let (mut kl, mut kr) = (k_weighting(), k_weighting());
    let mut e = 0.0;
    for _ in 0..n {
        pl.process(&mut l, &mut r);
        for (a, b) in l.iter().zip(&r) {
            let a = kl.1.run(kl.0.run(*a as f64));
            let b = kr.1.run(kr.0.run(*b as f64));
            e += a * a + b * b;
        }
    }
    Some(-0.691 + 10.0 * (e / (n * 128) as f64).log10())
}

#[test]
fn the_synthwave_hook_is_heard_against_the_bass() {
    for genre in ["synthwave", "outrun"] {
        for seed in [1, 2] {
            let bass = loudness(genre, seed, "bass").unwrap();
            let rel = |name: &str| loudness(genre, seed, name).map(|l| l - bass);
            let arp = rel("arp").unwrap();
            assert!(
                (-10.0..=0.5).contains(&arp),
                "{genre} {seed}: arp {arp:.1} LU"
            );
            let pads = rel("chords").unwrap();
            assert!(
                (-16.0..=-3.0).contains(&pads),
                "{genre} {seed}: pads {pads:.1} LU"
            );
            // Not every song has a lead.
            if let Some(lead) = rel("lead") {
                assert!(
                    (-8.0..=1.0).contains(&lead),
                    "{genre} {seed}: lead {lead:.1} LU"
                );
            }
        }
    }
}

#[test]
fn the_house_chords_and_lead_sit_with_the_bass() {
    for genre in ["chicago", "deep"] {
        for seed in [1, 2] {
            let bass = loudness(genre, seed, "bass").unwrap();
            let rel = |name: &str| loudness(genre, seed, name).map(|l| l - bass);
            let chords = rel("chords").unwrap();
            assert!(
                (-12.0..=3.0).contains(&chords),
                "{genre} {seed}: chords {chords:.1} LU"
            );
            if let Some(drone) = rel("drone") {
                assert!(
                    (-26.0..=-6.0).contains(&drone),
                    "{genre} {seed}: drone {drone:.1} LU"
                );
            }
            if let Some(lead) = rel("lead") {
                assert!(
                    (-10.0..=4.0).contains(&lead),
                    "{genre} {seed}: lead {lead:.1} LU"
                );
            }
        }
    }
}
