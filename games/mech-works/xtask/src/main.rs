//! `cargo xtask <command>` in games/mech-works/: the web build, size,
//! profiling copy, dependency rules and image diff, all from Playground
//! Kit's `pk_xtask`; this file only says what this game's web build is.

use pk_xtask::{Backend, RuntimeFiles, SideWasm, WebBuild};
use std::path::Path;

static WEB: WebBuild = WebBuild {
    crate_name: "mw_demo",
    // One wasm per GPU backend; the page picks.
    backends: &[
        Backend {
            name: "webgpu",
            out_name: "mw_demo",
            target_dir: None,
            cfg: None,
        },
        Backend {
            name: "webgl2",
            out_name: "mw_demo_webgl2",
            target_dir: Some("webgl2"),
            cfg: Some("pk_webgl2"),
        },
    ],
    page_dir: "crates/mw_demo/web",
    dist_dir: "dist/web",
    // The shipped policies and their training record, so the build is
    // whole wherever it is served (the public site has no checkpoints/).
    runtime_files: RuntimeFiles {
        required: &[("assets/policies", "assets/policies")],
        optional: &[],
    },
    side_wasm: &[
        SideWasm {
            krate: "mw_audio",
            features: &["worklet"],
            opt_level: "-O3",
            shim_js: Some("crates/mw_demo/web/mw-audio-worklet.js"),
            what: "mech sound effects worklet",
        },
        SideWasm {
            krate: "music",
            features: &["worklet"],
            opt_level: "-O3",
            shim_js: Some("../../crates/music/web/music-worklet.js"),
            what: "orchestral score worklet",
        },
    ],
    build_json_extra: None,
    wasm_opt: "116",
    size_budget_gzip: pk_xtask::size::DEFAULT_BUDGET_GZIP,
};

fn main() {
    // The game's folder, not the workspace root: rust-games holds several.
    pk_xtask::set_root(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask/ is in the game's folder"),
    );
    let commands = pk_xtask::web::commands(&WEB);
    if let Err(e) = pk_xtask::main_with(std::env::args().skip(1), &commands) {
        eprintln!("xtask {e}");
        std::process::exit(1);
    }
}
