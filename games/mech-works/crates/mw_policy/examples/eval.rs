//! A policy walking the simulation in closed loop, all in Rust:
//! `cargo run --release -p mw_policy --example eval -- <blob> [forward m/s] [estimator blob]`
//! (`EPISODES=64` for more than 16 per set; they run on every core).
//! Several episodes at nominal parameters, then random bodies, then with
//! damage and pushes; reports distance, speed and falls. With an estimator,
//! each set runs three ways: the policy told its true body, walking on the
//! estimator's guess, and told the nominal body (what the estimator saves
//! it from).
use mw_policy::{Estimator, Policy};
use mw_sim::env::{Command, Curriculum, EPISODE_STEPS, Env, POLICY_DT, PROPRIO};
use mw_sim::mech::{BodyParams, JOINTS};

#[derive(Clone, Copy, PartialEq)]
enum Body {
    Told,
    Estimated,
    Nominal,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .expect("usage: eval <blob> [forward] [estimator]");
    let forward: f32 = args.get(1).map_or(1.0, |s| s.parse().expect("a speed"));
    let policy =
        Policy::from_bytes(&std::fs::read(path).expect("read the blob")).expect("a policy");
    let episodes: usize = std::env::var("EPISODES").map_or(16, |s| s.parse().expect("EPISODES"));
    let estimator = args.get(2).map(|p| {
        Estimator::from_bytes(&std::fs::read(p).expect("read the estimator")).expect("an estimator")
    });
    let ways: &[Body] = if estimator.is_some() {
        &[Body::Told, Body::Estimated, Body::Nominal]
    } else {
        &[Body::Told]
    };
    let nominal = BodyParams::default().to_vec();
    for (name, c) in [
        (
            "nominal",
            Curriculum {
                body: 0.0,
                damage: 0.0,
                pushes: 0.0,
                speed: 1.0,
            },
        ),
        (
            "random bodies",
            Curriculum {
                body: 1.0,
                damage: 0.0,
                pushes: 0.0,
                speed: 1.0,
            },
        ),
        (
            "bodies + damage",
            Curriculum {
                body: 1.0,
                damage: 1.0,
                pushes: 0.0,
                speed: 1.0,
            },
        ),
        (
            "bodies + pushes + damage",
            Curriculum {
                body: 1.0,
                damage: 1.0,
                pushes: 1.0,
                speed: 1.0,
            },
        ),
    ] {
        for &way in ways {
            let episodes = episodes as u64;
            let results: Vec<Episode> = std::thread::scope(|scope| {
                let threads = std::thread::available_parallelism().map_or(4, |n| n.get()) as u64;
                let handles: Vec<_> = (0..threads)
                    .map(|t| {
                        let (mut policy, mut estimator) = (policy.clone(), estimator.clone());
                        let nominal = &nominal;
                        scope.spawn(move || {
                            (t..episodes)
                                .step_by(threads as usize)
                                .map(|seed| {
                                    episode(
                                        &mut policy,
                                        estimator.as_mut(),
                                        nominal,
                                        c,
                                        seed,
                                        way,
                                        forward,
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .flat_map(|h| h.join().unwrap())
                    .collect()
            });
            let falls = results.iter().filter(|e| e.fell).count();
            let dist: f32 = results.iter().map(|e| e.dist).sum();
            let secs: f32 = results.iter().map(|e| e.secs).sum();
            let err: f32 = results.iter().map(|e| e.err).sum();
            let errs: usize = results.iter().map(|e| e.errs).sum();
            let label = match way {
                Body::Told => "told",
                Body::Estimated => "estimated",
                Body::Nominal => "told nominal",
            };
            let est = if errs > 0 {
                format!(", parameter guess rmse {:.2}", (err / errs as f32).sqrt())
            } else {
                String::new()
            };
            println!(
                "{name:26} {label:13}: {falls:2}/{episodes} fell, {:5.1} m per episode, {:.2} m/s (asked {forward}){est}",
                dist / episodes as f32,
                dist / secs
            );
        }
    }
}

struct Episode {
    fell: bool,
    dist: f32,
    secs: f32,
    err: f32,
    errs: usize,
}

fn episode(
    policy: &mut Policy,
    mut estimator: Option<&mut Estimator>,
    nominal: &[f32],
    c: Curriculum,
    seed: u64,
    way: Body,
    forward: f32,
) -> Episode {
    let mut env = Env::new(seed);
    env.curriculum = c;
    env.hold_command = true;
    env.reset();
    env.command = Command { forward, yaw: 0.0 };
    let x0 = env.sim.torso().translation().x;
    let mut action = [0.0; JOINTS];
    let mut out = Episode {
        fell: false,
        dist: 0.0,
        secs: 0.0,
        err: 0.0,
        errs: 0,
    };
    if let Some(e) = estimator.as_mut() {
        e.reset(&env.observe());
    }
    for step in 0..EPISODE_STEPS {
        let mut obs = env.observe();
        if let Some(e) = estimator.as_mut() {
            if step > 0 {
                e.push(&obs);
            }
            e.estimate();
            let truth = env.sim.params.to_vec();
            out.err += e
                .params()
                .iter()
                .zip(&truth)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f32>()
                / truth.len() as f32;
            out.errs += 1;
        }
        match way {
            Body::Told => policy.act(&obs, &mut action),
            Body::Estimated => {
                let e = estimator.as_ref().unwrap();
                policy.act_with_latent(&obs, e.latent(), &mut action);
            }
            Body::Nominal => {
                obs[PROPRIO..].copy_from_slice(nominal);
                policy.act(&obs, &mut action);
            }
        }
        let r = env.step(&action);
        out.secs += POLICY_DT;
        if r.terminated {
            out.fell = true;
            break;
        }
    }
    out.dist = env.sim.torso().translation().x - x0;
    out
}
