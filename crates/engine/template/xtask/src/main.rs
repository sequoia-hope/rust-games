//! `cargo xtask <command>`: web build, size, profiling copy, dependency
//! rules and image diff, all from Playground Kit's `pk_xtask`; this file
//! only says what this game's web build is. Add the game's own commands
//! to `commands` below.

use pk_xtask::{Backend, RuntimeFiles, WebBuild};

static WEB: WebBuild = WebBuild {
    // The client crate, built with `--lib` for the web.
    crate_name: "game",
    // Bevy picks its GPU backend at compile time: one wasm per backend,
    // the WebGL2 one with its own cfg in its own target directory. The
    // page (`web/index.html`) picks between them.
    backends: &[
        Backend {
            name: "webgpu",
            out_name: "game",
            target_dir: None,
            cfg: None,
        },
        Backend {
            name: "webgl2",
            out_name: "game_webgl2",
            target_dir: Some("webgl2"),
            cfg: Some("pk_webgl2"),
        },
    ],
    page_dir: "crates/game/web",
    dist_dir: "dist/next",
    // Repository files the page loads at run time, copied into the build
    // so it is whole wherever it is served.
    runtime_files: RuntimeFiles {
        required: &[],
        optional: &[],
    },
    // Crates that are also their own wasm beside the page (AudioWorklet
    // processors, workers): `SideWasm { krate, features, opt_level, shim_js, what }`.
    side_wasm: &[],
    build_json_extra: None,
    // binaryen's version; CI installs the same one.
    wasm_opt: "116",
    size_budget_gzip: pk_xtask::size::DEFAULT_BUDGET_GZIP,
};

fn main() {
    let commands = pk_xtask::web::commands(&WEB);
    // The game's own: `commands.push(pk_xtask::Command::new("name", "help", |args| ...))`.
    if let Err(e) = pk_xtask::main_with(std::env::args().skip(1), &commands) {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
}
