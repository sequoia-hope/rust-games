//! `cargo xtask <command>`: build tooling.
//!
//! Commands:
//!   web [--release] [--only webgpu|webgl2]   build the web client into dist/web/
//!   size                                      report the built wasm's size
//!   check [--web]                             the test suite (see check.rs)

mod check;
mod web;

use std::path::PathBuf;
use std::process::{Command, ExitCode};

/// An error that ends the command with a message and a non-zero exit.
type Result<T = ()> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprintln!("{}", usage());
        return ExitCode::FAILURE;
    };
    let rest = &args[1..];
    let result = match cmd.as_str() {
        "web" => web::run(rest),
        "size" => web::size(),
        "check" => check::run(rest),
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n\n{}", usage())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask {cmd}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> &'static str {
    "usage: cargo xtask <command>\n\
     \n\
     commands:\n\
     \x20 web [--release] [--only webgpu|webgl2]   build the web client into dist/web/\n\
     \x20 size                                      the built wasm's size (raw, gzip, brotli)\n\
     \x20 check [--web]                             the whole test suite (--web: and in the browser)"
}

/// The game's root: the folder above this xtask. In rust-games it is not
/// the workspace root (see [`workspace_root`]).
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the game's folder")
        .to_path_buf()
}

/// The workspace root, where `Cargo.lock` is: the directory of the
/// manifest `cargo locate-project --workspace` names.
fn workspace_root() -> PathBuf {
    let out = cargo()
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .output()
        .expect("cargo locate-project runs");
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
        .parent()
        .expect("the workspace manifest has a directory")
        .to_path_buf()
}

/// Cargo's build directory: `$CARGO_TARGET_DIR`, else the workspace's
/// `target/`.
fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

/// `cargo`, as the one running us.
fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

/// Runs a command, failing on a non-zero exit.
fn exec(cmd: &mut Command) -> Result {
    let status = cmd
        .status()
        .map_err(|e| format!("running {:?}: {e}", cmd.get_program()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{:?} failed ({status})", cmd.get_program()))
    }
}

/// Runs a command and returns its standard output.
fn output(cmd: &mut Command) -> Result<String> {
    let out = cmd
        .output()
        .map_err(|e| format!("running {:?}: {e}", cmd.get_program()))?;
    if !out.status.success() {
        return Err(format!("{:?} failed ({})", cmd.get_program(), out.status));
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}
