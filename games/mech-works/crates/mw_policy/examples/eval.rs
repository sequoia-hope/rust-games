//! A policy walking the simulation in closed loop, all in Rust:
//! `cargo run --release -p mw_policy --example eval -- <blob> [forward m/s]`.
//! Several episodes at nominal parameters, then with damage and pushes;
//! reports distance, speed and falls.
use mw_policy::Policy;
use mw_sim::env::{Command, Curriculum, EPISODE_STEPS, Env, POLICY_DT};
use mw_sim::mech::JOINTS;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: eval <blob> [forward]");
    let forward: f32 = args.get(1).map_or(1.0, |s| s.parse().expect("a speed"));
    let mut policy =
        Policy::from_bytes(&std::fs::read(path).expect("read the blob")).expect("a policy");
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
            "bodies + pushes + damage",
            Curriculum {
                body: 1.0,
                damage: 1.0,
                pushes: 1.0,
                speed: 1.0,
            },
        ),
    ] {
        let (mut falls, mut dist, mut secs) = (0, 0.0, 0.0);
        let episodes = 8;
        for seed in 0..episodes {
            let mut env = Env::new(seed);
            env.curriculum = c;
            env.hold_command = true;
            env.reset();
            env.command = Command { forward, yaw: 0.0 };
            let x0 = env.sim.torso().translation().x;
            let mut action = [0.0; JOINTS];
            for _ in 0..EPISODE_STEPS {
                policy.act(&env.observe(), &mut action);
                let r = env.step(&action);
                secs += POLICY_DT;
                if r.terminated {
                    falls += 1;
                    break;
                }
            }
            dist += env.sim.torso().translation().x - x0;
        }
        println!(
            "{name:26}: {falls}/{episodes} fell, {:.1} m per episode, {:.2} m/s (asked {forward})",
            dist / episodes as f32,
            dist / secs
        );
    }
}
