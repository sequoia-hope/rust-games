//! Every shipped policy gives what PyTorch gave for the test observations
//! its export recorded, to 1e-5.
use std::path::PathBuf;

use mw_policy::Policy;

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

#[test]
fn rejects_what_is_not_a_blob() {
    assert!(Policy::from_bytes(b"hello world, not a policy").is_err());
}
