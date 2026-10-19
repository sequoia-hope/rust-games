//! The expectation model the generator scores its candidates with
//! (research/RULES.md G9): a variable-order PPM model with escape method C
//! and interpolated smoothing, a long-term model (LTM, the genre's
//! vocabulary: the listener's enculturation) and a short-term one (STM,
//! learning as the song plays: how a returning hook becomes predictable),
//! combined by entropy-weighted geometric mean (b = 2). Each event's
//! surprise is its information content IC = −log2 p; the uncertainty before
//! it is the entropy H of the prediction. Both are scored as z-scores
//! against the model's own corpus, so the targets of Gold et al. 2019 and
//! the placement rule of Cheung et al. 2019 apply in standard units (G9.3,
//! G9.5, G9.7).
//!
//! Also the two syncopation indices of G9.6: Longuet-Higgins and Lee per
//! layer, Witek et al.'s polyphonic index per kit loop.

use std::collections::BTreeMap;

/// A PPM model over symbols `0..alphabet`, contexts up to `order` events
/// (G9.3: escape C, interpolated, recursing to a uniform order −1).
#[derive(Clone, Debug)]
pub struct Ppm {
    pub order: usize,
    pub alphabet: usize,
    /// Context → (symbol → count), in symbol order.
    counts: BTreeMap<Vec<u32>, BTreeMap<u32, u32>>,
}

impl Ppm {
    pub fn new(order: usize, alphabet: usize) -> Ppm {
        Ppm {
            order,
            alphabet,
            counts: BTreeMap::new(),
        }
    }

    /// Counts every event of `seq` after each of its contexts (orders 0 to
    /// the bound).
    pub fn train(&mut self, seq: &[u32]) {
        for i in 0..seq.len() {
            self.observe(&seq[..i], seq[i]);
        }
    }

    /// Counts one event after its history.
    pub fn observe(&mut self, history: &[u32], e: u32) {
        for k in 0..=self.order.min(history.len()) {
            let ctx = history[history.len() - k..].to_vec();
            *self.counts.entry(ctx).or_default().entry(e).or_insert(0) += 1;
        }
    }

    /// The predictive distribution after `history`.
    pub fn dist(&self, history: &[u32]) -> Vec<f64> {
        let a = self.alphabet.max(1);
        let mut p = vec![1.0 / a as f64; a];
        let longest = self.order.min(history.len());
        for k in 0..=longest {
            let ctx = &history[history.len() - k..];
            let Some(c) = self.counts.get(ctx) else {
                // An unseen context adds nothing; longer ones are unseen too.
                break;
            };
            let n: f64 = c.values().map(|v| *v as f64).sum();
            let t = c.len() as f64;
            let mut q: Vec<f64> = p.iter().map(|lower| t * lower).collect();
            for (s, v) in c {
                if (*s as usize) < a {
                    q[*s as usize] += *v as f64;
                }
            }
            for x in &mut q {
                *x /= n + t;
            }
            p = q;
        }
        p
    }
}

/// Entropy in bits.
pub fn entropy(p: &[f64]) -> f64 {
    -p.iter()
        .filter(|x| **x > 0.0)
        .map(|x| x * x.log2())
        .sum::<f64>()
}

/// The weighted geometric mean of distributions (G9.3), weights
/// `(H / log2 |A|)^−b`.
pub fn combine(ps: &[Vec<f64>], b: f64) -> Vec<f64> {
    let a = ps[0].len();
    let hmax = (a as f64).log2().max(1e-9);
    let ws: Vec<f64> = ps
        .iter()
        .map(|p| (entropy(p) / hmax).max(1e-6).powf(-b))
        .collect();
    let wsum: f64 = ws.iter().sum();
    let mut out: Vec<f64> = (0..a)
        .map(|i| {
            let lg: f64 = ps
                .iter()
                .zip(&ws)
                .map(|(p, w)| w * p[i].max(1e-12).ln())
                .sum();
            (lg / wsum).exp()
        })
        .collect();
    let z: f64 = out.iter().sum();
    for x in &mut out {
        *x /= z;
    }
    out
}

/// The surprise and uncertainty of each event of a sequence, with the STM
/// learning as it goes (from `stm`, the song so far).
pub fn profile(ltm: &Ppm, stm: &Ppm, seq: &[u32], context: &[u32]) -> Vec<(f64, f64)> {
    let mut stm = stm.clone();
    let mut hist: Vec<u32> = context.to_vec();
    let mut out = Vec::with_capacity(seq.len());
    for &e in seq {
        let p = combine(&[ltm.dist(&hist), stm.dist(&hist)], 2.0);
        let h = entropy(&p);
        let ic = -p
            .get(e as usize)
            .copied()
            .unwrap_or(1e-12)
            .max(1e-12)
            .log2();
        out.push((ic, h));
        stm.observe(&hist, e);
        hist.push(e);
    }
    out
}

/// Mean and spread of IC and H over a corpus, for z-scores (G9.3,
/// "Calibration").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calib {
    pub ic: (f64, f64),
    pub h: (f64, f64),
}

impl Calib {
    /// From the model's own corpus, each sequence scored with a fresh STM.
    pub fn of(ltm: &Ppm, corpus: &[Vec<u32>]) -> Calib {
        let empty = Ppm::new(ltm.order, ltm.alphabet);
        let all: Vec<(f64, f64)> = corpus
            .iter()
            .flat_map(|s| profile(ltm, &empty, s, &[]))
            .collect();
        let stats = |f: &dyn Fn(&(f64, f64)) -> f64| {
            let n = all.len().max(1) as f64;
            let mu = all.iter().map(f).sum::<f64>() / n;
            let sd = (all.iter().map(|x| (f(x) - mu).powi(2)).sum::<f64>() / n).sqrt();
            (mu, sd.max(0.25))
        };
        Calib {
            ic: stats(&|x| x.0),
            h: stats(&|x| x.1),
        }
    }

    pub fn z(&self, (ic, h): (f64, f64)) -> (f64, f64) {
        ((ic - self.ic.0) / self.ic.1, (h - self.h.0) / self.h.1)
    }
}

/// A candidate's scores (G9.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    /// Mean z(IC): complexity.
    pub ic: f64,
    /// Mean z(H): uncertainty.
    pub h: f64,
    /// Σ z(IC) · (−z(H)) / n: surprises where the listener is sure,
    /// expected events where they are not (Cheung et al.).
    pub placement: f64,
}

impl Score {
    pub fn of(z: &[(f64, f64)]) -> Score {
        let n = z.len().max(1) as f64;
        Score {
            ic: z.iter().map(|x| x.0).sum::<f64>() / n,
            h: z.iter().map(|x| x.1).sum::<f64>() / n,
            placement: z.iter().map(|x| x.0 * -x.1).sum::<f64>() / n,
        }
    }

    /// The fitness against a target band for mean z(IC) (G9.7: melody −1.0
    /// to 0.0) and mean z(H) (−0.75 to −0.25): placement, minus the squared
    /// distance outside each band. Below `reject` or above `ceiling` mean IC
    /// is a hard no (`-inf`).
    pub fn fitness(&self, ic_band: (f64, f64), h_band: (f64, f64), reject: (f64, f64)) -> f64 {
        if self.ic < reject.0 || self.ic > reject.1 {
            return f64::NEG_INFINITY;
        }
        let out = |x: f64, (lo, hi): (f64, f64)| {
            if x < lo {
                lo - x
            } else if x > hi {
                x - hi
            } else {
                0.0
            }
        };
        self.placement - out(self.ic, ic_band).powi(2) - out(self.h, h_band).powi(2)
    }
}

/// LHL metrical weights on the 16-step grid (G9.6).
const LHL: [i32; 16] = [
    0, -4, -3, -4, -2, -4, -3, -4, -1, -4, -3, -4, -2, -4, -3, -4,
];

/// The Longuet-Higgins and Lee syncopation of a one-bar pattern (onsets on
/// steps 0..16), with wrap: for each onset, the strongest silent step
/// before the next onset, if stronger, adds the difference.
pub fn lhl(on: &[bool; 16]) -> i32 {
    let onsets: Vec<usize> = (0..16).filter(|i| on[*i]).collect();
    let mut s = 0;
    for (k, &i) in onsets.iter().enumerate() {
        let next = onsets.get(k + 1).copied().unwrap_or(onsets[0] + 16);
        let w_max = (i + 1..next).map(|j| LHL[j % 16]).max();
        if let Some(w) = w_max
            && w > LHL[i]
        {
            s += w - LHL[i];
        }
    }
    s
}

/// Witek et al.'s weights (G9.6): downbeat 0, other quarters −1, 8ths −2,
/// 16ths −3.
fn witek_w(step: usize) -> i32 {
    match step % 16 {
        0 => 0,
        4 | 8 | 12 => -1,
        s if s % 2 == 0 => -2,
        _ => -3,
    }
}

/// Witek et al.'s polyphonic syncopation index of a kit loop (kick, snare,
/// hat over the same number of steps, a multiple of 16), with wrap. Only
/// the kick and snare are scored (the hat keeps time): a kick or snare note
/// N whose next onset anywhere, at position R, falls on a position at least
/// as strong and is not its own instrument's is a polyphonic syncopation,
/// w(R) − w(N) + I, I = 2 for a kick with both others on R, 1 for a snare
/// with both others, 5 with one other; a silent position stronger than N
/// before R is a monophonic one, w − w(N).
pub fn witek(kick: &[bool], snare: &[bool], hat: &[bool]) -> i32 {
    let n = kick.len();
    let streams = [kick, snare, hat];
    let mut total = 0;
    for si in 0..2 {
        let s = streams[si];
        for i in 0..n {
            if !s[i] {
                continue;
            }
            let wn = witek_w(i);
            let mut j = (i + 1) % n;
            let mut silent_max: Option<i32> = None;
            for _ in 0..n {
                if streams.iter().any(|x| x[j]) {
                    break;
                }
                silent_max = Some(silent_max.map_or(witek_w(j), |m: i32| m.max(witek_w(j))));
                j = (j + 1) % n;
            }
            if let Some(m) = silent_max
                && m > wn
            {
                total += m - wn;
                continue;
            }
            let wr = witek_w(j);
            if s[j] || wr < wn {
                continue;
            }
            let others = (0..3).filter(|o| *o != si && streams[*o][j]).count();
            let inst = match (si, others) {
                (0, 2) => 2,
                (1, 2) => 1,
                _ => 5,
            };
            total += wr - wn + inst;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(steps: &[usize]) -> [bool; 16] {
        let mut b = [false; 16];
        for s in steps {
            b[s - 1] = true;
        }
        b
    }

    #[test]
    fn lhl_matches_the_worked_values() {
        // RULES.md G9.6's table (steps counted from 1).
        assert_eq!(lhl(&bar(&[1, 5, 9, 13])), 0);
        assert_eq!(lhl(&bar(&[3, 7, 11, 15])), 7);
        assert_eq!(lhl(&bar(&[1, 7, 13])), 2);
        assert_eq!(lhl(&bar(&[1, 4, 7, 9, 12, 15])), 4);
        assert_eq!(lhl(&bar(&[1, 4, 7, 11, 13])), 4);
        assert_eq!(lhl(&bar(&[2, 4, 6, 8, 10, 12, 14, 16])), 15);
    }

    #[test]
    fn four_on_the_floor_has_no_polyphonic_syncopation() {
        let k = bar(&[1, 5, 9, 13]);
        let s = bar(&[5, 13]);
        let h = bar(&[3, 7, 11, 15]);
        assert_eq!(witek(&k, &s, &h), 0);
        let eighths = bar(&[1, 3, 5, 7, 9, 11, 13, 15]);
        assert_eq!(
            witek(&bar(&[1, 9]), &bar(&[5, 13]), &eighths),
            0,
            "a rock beat"
        );
        let pushed = witek(&bar(&[1, 4, 11]), &bar(&[5, 13]), &eighths);
        let broken = witek(&bar(&[1, 3, 11, 12]), &bar(&[5, 8, 10, 13, 16]), &eighths);
        eprintln!("pushed {pushed} broken {broken}");
        assert!(
            pushed > 0 && broken > pushed,
            "a breakbeat syncopates more: {pushed} < {broken}"
        );
    }

    #[test]
    fn ppm_learns_and_the_stm_makes_a_return_predictable() {
        // A corpus of stepwise walks over 6 symbols (a varied style).
        let mut x = 12345u32;
        let mut rnd = move || {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (x >> 16) % 6
        };
        let corpus: Vec<Vec<u32>> = (0..60)
            .map(|_| {
                let mut v = vec![rnd()];
                for _ in 0..15 {
                    let step = [5, 1, 1, 0][(rnd() % 4) as usize];
                    v.push((v[v.len() - 1] + step) % 6);
                }
                v
            })
            .collect();
        let mut ltm = Ppm::new(2, 6);
        for s in &corpus {
            ltm.train(s);
        }
        let p = ltm.dist(&[2]);
        assert!(
            p[3] > p[0] && p[1] > p[5],
            "steps are likelier than leaps: {p:?}"
        );
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        let calib = Calib::of(&ltm, &corpus);
        let zs = |stm: &Ppm, seq: &[u32]| {
            Score::of(
                &profile(&ltm, stm, seq, &[])
                    .into_iter()
                    .map(|x| calib.z(x))
                    .collect::<Vec<_>>(),
            )
        };
        // Leaps the style never makes.
        let odd = [0, 3, 0, 3, 5, 2, 5, 2];
        let first = zs(&Ppm::new(2, 6), &odd);
        let mut stm = Ppm::new(2, 6);
        stm.train(&odd);
        stm.train(&odd);
        let again = zs(&stm, &odd);
        let walk = zs(&Ppm::new(2, 6), &corpus[0][..8]);
        assert!(
            first.ic > walk.ic + 1.0,
            "leaps surprise: {first:?} vs {walk:?}"
        );
        assert!(
            again.ic < first.ic - 0.5,
            "less on their return: {again:?} vs {first:?}"
        );
    }
}
