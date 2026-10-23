//! The native client: `cargo run --release -p mw_demo -- [--mode impact]
//! [--policy <blob>]...`, one lane per policy. (On the web the library is
//! the entry point; this binary is native only.)

#[cfg(not(target_arch = "wasm32"))]
fn main() -> bevy::app::AppExit {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut mode = mw_demo::Mode::Viewer;
    let mut lane = 0;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mode" => mode = mw_demo::Mode::parse(it.next().map_or("", String::as_str)),
            "--policy" => {
                let Some(path) = it.next() else {
                    eprintln!("--policy needs a path");
                    std::process::exit(2);
                };
                let bytes = std::fs::read(path).unwrap_or_else(|e| {
                    eprintln!("{path}: {e}");
                    std::process::exit(2);
                });
                mw_demo::hand_policy(lane, path.clone(), bytes);
                lane += 1;
            }
            other => {
                eprintln!(
                    "unknown argument {other} (--mode viewer|impact|damage|calibration, --policy <blob>)"
                );
                std::process::exit(2);
            }
        }
    }
    mw_demo::app(mode).run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
