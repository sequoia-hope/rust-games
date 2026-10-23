//! The native client: `cargo run --release -p mw_demo -- [--mode impact]
//! [--policy <blob> [--estimator <blob>]]...`, one lane per policy (an
//! estimator joins the lane of the policy before it). `--stage <json>` queues
//! a command; `--shot <png> [--after <secs>]` saves a screenshot and exits. (On the web the library is
//! the entry point; this binary is native only.)

#[cfg(not(target_arch = "wasm32"))]
fn main() -> bevy::app::AppExit {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut mode = mw_demo::Mode::Viewer;
    let mut lane = 0;
    let mut shot: Option<(String, f32)> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--stage" => mw_demo::hand_command(it.next().cloned().unwrap_or_default()),
            "--shot" => {
                let path = it.next().cloned().unwrap_or_else(|| "shot.png".into());
                shot = Some((path, shot.map_or(5.0, |s| s.1)));
            }
            "--after" => {
                let secs = it.next().and_then(|s| s.parse().ok()).unwrap_or(5.0);
                shot = Some((shot.map_or("shot.png".into(), |s| s.0), secs));
            }
            "--mode" => mode = mw_demo::Mode::parse(it.next().map_or("", String::as_str)),
            flag @ ("--policy" | "--estimator") => {
                let Some(path) = it.next() else {
                    eprintln!("{flag} needs a path");
                    std::process::exit(2);
                };
                let bytes = std::fs::read(path).unwrap_or_else(|e| {
                    eprintln!("{path}: {e}");
                    std::process::exit(2);
                });
                if flag == "--policy" {
                    mw_demo::hand_policy(lane, path.clone(), bytes);
                    lane += 1;
                } else {
                    mw_demo::hand_policy(lane.saturating_sub(1), path.clone(), bytes);
                }
            }
            other => {
                eprintln!(
                    "unknown argument {other} (--mode viewer|impact|damage|calibration, --policy <blob>, --estimator <blob>, --stage <json>, --shot <png>, --after <secs>)"
                );
                std::process::exit(2);
            }
        }
    }
    let mut app = mw_demo::app(mode);
    if let Some((path, secs)) = shot {
        mw_demo::shot_after(&mut app, path, secs);
    }
    app.run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
