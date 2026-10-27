//! Every shipped policy and estimator gives what PyTorch gave for the test
//! inputs its export recorded, to 1e-5.
use std::path::PathBuf;

use mw_policy::{Estimator, Policy};

fn policies() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/policies");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    out.sort();
    out
}

#[test]
fn matches_pytorch() {
    let paths = policies();
    assert!(!paths.is_empty(), "no shipped policies");
    for path in paths {
        if mw_policy::arch(&std::fs::read(&path).unwrap()).unwrap() != "teacher_v1" {
            continue;
        }
        let mut policy = Policy::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        let side: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path.with_extension("bin.json")).unwrap(),
        )
        .unwrap();
        let test = &side["test"];
        let obs = test["obs"].as_array().unwrap();
        let want = test["action"].as_array().unwrap();
        let mut action = vec![0.0; policy.header.act_dim];
        let mut worst: f32 = 0.0;
        for (o, w) in obs.iter().zip(want) {
            let o: Vec<f32> = o
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap() as f32)
                .collect();
            policy.act(&o, &mut action);
            for (a, w) in action.iter().zip(w.as_array().unwrap()) {
                worst = worst.max((a - w.as_f64().unwrap() as f32).abs());
            }
        }
        assert!(worst < 1e-5, "{}: worst difference {worst}", path.display());
    }
}

fn floats(v: &serde_json::Value) -> Vec<f32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap() as f32)
        .collect()
}

#[test]
fn estimators_match_pytorch() {
    let mut checked = 0;
    for path in policies() {
        let bytes = std::fs::read(&path).unwrap();
        if mw_policy::arch(&bytes).unwrap() != "estimator_v1" {
            continue;
        }
        let mut est = Estimator::from_bytes(&bytes).unwrap();
        checked += 1;
        let side: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(path.with_extension("bin.json")).unwrap(),
        )
        .unwrap();
        let test = &side["test"];
        let mut worst: f32 = 0.0;
        for (i, hist) in test["history"].as_array().unwrap().iter().enumerate() {
            let steps = hist.as_array().unwrap();
            est.reset(&floats(&steps[0]));
            for s in &steps[1..] {
                est.push(&floats(s));
            }
            est.estimate();
            for (got, want) in [
                (est.latent(), &test["latent"][i]),
                (est.params(), &test["params"][i]),
            ] {
                for (a, w) in got.iter().zip(floats(want)) {
                    worst = worst.max((a - w).abs());
                }
            }
        }
        assert!(worst < 1e-5, "{}: worst difference {worst}", path.display());
    }
    assert!(checked > 0, "no shipped estimators");
}

#[test]
fn rejects_what_is_not_a_blob() {
    assert!(Policy::from_bytes(b"hello world, not a policy").is_err());
}

#[test]
fn deterministic_replay_and_terrain_path_walk() {
    use mw_sim::env::{Command, Env};
    use mw_sim::mech::{BodyParams, JOINTS};
    use mw_sim::terrain::Terrain;

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/policies");
    let mut policy =
        Policy::from_bytes(&std::fs::read(dir.join("teacher-v4.bin")).unwrap()).unwrap();
    let mut est =
        Estimator::from_bytes(&std::fs::read(dir.join("teacher-v4.estimator.bin")).unwrap())
            .unwrap();

    // 1. Deterministic replay check: two runs from the same seed and body produce
    // identical trajectories.
    let run_flat = |p: &mut Policy, e: &mut Estimator| -> Vec<[f32; 3]> {
        let mut env = Env::new(17);
        env.hold_command = true;
        env.command = Command {
            forward: 0.85,
            yaw: 0.0,
        };
        env.reset_with(BodyParams {
            torso_mass: 0.3,
            tool: [0.5, 0.8, 0.2],
            ..BodyParams::default()
        });
        e.reset(&env.observe());
        let mut action = [0.0; JOINTS];
        let mut poses = Vec::with_capacity(150);
        for step in 0..150 {
            let obs = env.observe();
            if step > 0 {
                e.push(&obs);
            }
            e.estimate();
            p.act_with_latent(&obs, e.latent(), &mut action);
            let r = env.step(&action);
            assert!(!r.terminated, "flat replay fell at step {step}");
            let t = env.sim.torso().translation();
            poses.push([t.x, t.y, t.z]);
        }
        poses
    };
    let traj_a = run_flat(&mut policy, &mut est);
    let traj_b = run_flat(&mut policy, &mut est);
    assert_eq!(traj_a, traj_b);
    assert!(traj_a.last().unwrap()[0] > 1.2, "walked forward on flat");

    // 2. M5 terrain & height scan check: walking along the uneven dirt path with
    // boulders, branches, and car wrecks adapts from the height scan.
    let mut env = Env::new(23);
    env.terrain = Terrain::path(200.0, 0.45, 0.65, 0.9, 23);
    env.hold_command = true;
    env.command = Command {
        forward: 0.75,
        yaw: 0.0,
    };
    env.reset_with(BodyParams::default());
    let mut action = [0.0; JOINTS];
    let mut max_scan: f32 = 0.0;
    for step in 0..450 {
        let scan = env.height_scan();
        for &h in &scan {
            max_scan = max_scan.max(h.abs());
        }
        let obs = env.observe();
        policy.act(&obs, &mut action);
        let r = env.step(&action);
        assert!(
            !r.terminated,
            "path walk fell at step {step} (x = {:.2})",
            env.sim.torso().translation().x
        );
    }
    let final_x = env.sim.torso().translation().x;
    assert!(
        final_x > 4.5,
        "expected mech to walk past obstacles on path, got x = {final_x:.2}"
    );
    assert!(
        max_scan > 0.03,
        "expected height scan to detect terrain/obstacles, got max_scan = {max_scan:.3}"
    );
}
