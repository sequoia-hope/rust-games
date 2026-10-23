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
