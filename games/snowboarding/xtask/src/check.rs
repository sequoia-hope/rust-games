//! `cargo xtask check [--web]`: the whole test suite, stopping at the
//! first failure.
//!
//! Always (no server needed):
//! - `cargo test` on this game's crates: the simulation (physics, races, a seeded
//!   masher on every course and mode), the music engine and station, the
//!   announcers' script and director, and the game itself headless
//!   (`sb_game/src/tests.rs`: every screen change, key mashing, touch,
//!   the announcers in a run; `sb_game/tests/leak.rs`: memory across runs).
//! - `cargo clippy` on this game's crates and on the web build, warnings
//!   denied. (The whole rust-games workspace is checked by `scripts/land.sh`.)
//!
//! With `--web`, in headless Chrome on the GPU through the registered
//! server (`proj up snowboarding`; the tools find its port):
//! - the release web build (`cargo xtask web --release`);
//! - `tools/stress.mjs`: seeded random screen changes, WebGPU and WebGL2,
//!   and touch at phone size, failing on any error, panic, freeze or a
//!   heap that keeps growing;
//! - `tools/heap-probe.mjs`: the heap across rides, restarts and course
//!   changes;
//! - `tools/courses.mjs`: every course ridden, WebGPU, WebGL2 and at phone
//!   size, failing on a heap that grows or GPU buffers re-made every frame
//!   (what got Avalanche Bowl's tab killed on an iPhone);
//! - `tools/radio-check.mjs`: the music sounds and the announcers talk
//!   (also on a page over plain http, where the music has no worklet).

use crate::{Result, cargo, exec, root};
use std::process::Command;

/// This game's crates, as `-p` arguments: the workspace holds other games.
const PACKAGES: &[&str] = &[
    "-p", "sb_sim", "-p", "sb_radio", "-p", "sb_game", "-p", "sb_xtask",
];

pub fn run(args: &[String]) -> Result {
    let web = match args {
        [] => false,
        [a] if a == "--web" => true,
        _ => return Err("usage: cargo xtask check [--web]".into()),
    };
    step("tests", cargo().arg("test").args(PACKAGES))?;
    step(
        "lints",
        cargo()
            .arg("clippy")
            .args(PACKAGES)
            .args(["--all-targets", "--", "-D", "warnings"]),
    )?;
    step(
        "lints (web)",
        cargo().args([
            "clippy",
            "-p",
            "sb_game",
            "--target",
            crate::web::TARGET,
            "--",
            "-D",
            "warnings",
        ]),
    )?;
    if !web {
        println!("check: ok (add --web for the browser checks)");
        return Ok(());
    }
    println!("check: the release web build");
    crate::web::run(&["--release".to_string()])?;
    let node = |script: &str, extra: &[&str]| {
        let mut c = Command::new("node");
        c.current_dir(root()).arg(script).args(extra);
        c
    };
    step(
        "stress, WebGPU",
        &mut node("tools/stress.mjs", &["--steps", "150", "--seed", "1"]),
    )?;
    step(
        "stress, WebGL2",
        &mut node(
            "tools/stress.mjs",
            &["--steps", "150", "--seed", "2", "--backend", "webgl2"],
        ),
    )?;
    step(
        "stress, phone",
        &mut node(
            "tools/stress.mjs",
            &[
                "--steps", "100", "--seed", "3", "--touch", "--size", "844x390",
            ],
        ),
    )?;
    step(
        "heap",
        &mut node(
            "tools/heap-probe.mjs",
            &["webgl2", "ride", "restart", "course"],
        ),
    )?;
    step("courses, WebGPU", &mut node("tools/courses.mjs", &[]))?;
    step(
        "courses, WebGL2",
        &mut node("tools/courses.mjs", &["--backend", "webgl2"]),
    )?;
    step(
        "courses, phone",
        &mut node(
            "tools/courses.mjs",
            &["--backend", "webgl2", "--touch", "--size", "390x844"],
        ),
    )?;
    step("radio, WebGPU", &mut node("tools/radio-check.mjs", &[]))?;
    step(
        "radio, WebGL2",
        &mut node("tools/radio-check.mjs", &["--backend", "webgl2"]),
    )?;
    step(
        "radio, an insecure page",
        &mut node(
            "tools/radio-check.mjs",
            &["--backend", "webgl2", "--insecure"],
        ),
    )?;
    println!("check: ok, browser checks included");
    Ok(())
}

fn step(what: &str, cmd: &mut Command) -> Result {
    println!("check: {what}");
    exec(cmd).map_err(|e| format!("{what}: {e}"))
}
