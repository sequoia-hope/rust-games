//! The library behind a game's `cargo xtask` (PLAN.md section 4, layer 2):
//! the web build (`web`), the size report and budget (`size`), the
//! names-kept profiling copy (`prof`), the crate dependency rules
//! (`check-deps`, read from `deps.toml` at the workspace root) and the
//! screenshot metric (`imgdiff`).
//!
//! A game's `xtask/src/main.rs` is forty lines: a [`WebBuild`] describing
//! its client crate, backends, page and side wasms, then
//! [`main_with`] over [`web::commands`] plus its own [`Command`]s
//! (`template/xtask` in this repository). Nothing here knows a port or a
//! game name; every path is relative to the workspace root, found with
//! `cargo locate-project --workspace`.

pub mod deps;
pub mod imgdiff;
pub mod prof;
pub mod size;
pub mod web;

pub use serde_json;
pub use web::{Backend, RuntimeFiles, SideWasm, WebBuild};

use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::OnceLock;

/// An error that ends the command with a message and a non-zero exit.
pub type Result<T = ()> = std::result::Result<T, String>;

/// What a [`Command`] runs: the arguments after its name.
pub type Run = Box<dyn Fn(&[String]) -> Result>;

/// One `cargo xtask <name> [args]` subcommand.
pub struct Command {
    pub name: &'static str,
    /// One or two lines for the usage text, as `name [options]  what it does`.
    pub help: &'static str,
    pub run: Run,
}

impl Command {
    pub fn new(
        name: &'static str,
        help: &'static str,
        run: impl Fn(&[String]) -> Result + 'static,
    ) -> Command {
        Command {
            name,
            help,
            run: Box::new(run),
        }
    }
}

/// The commands every workspace gets: `check-deps` and `imgdiff`.
pub fn builtin_commands() -> Vec<Command> {
    vec![
        Command::new(
            "check-deps",
            "check-deps        enforce the crate dependency rules in deps.toml",
            deps::run,
        ),
        Command::new(
            "imgdiff",
            "imgdiff --a <dir> --b <dir> [--label name] [--out <dir>]\n\
             \x20                  compare two directories of screenshots (CIEDE2000); write a report",
            imgdiff::run,
        ),
    ]
}

/// Runs `cargo xtask <command>` over the built-in commands and `extra`
/// (a game's own, and [`web::commands`] for its web build). An `extra`
/// command with a built-in's name replaces it. `-h`, `--help` and `help`
/// print the usage.
pub fn main_with(args: impl Iterator<Item = String>, extra: &[Command]) -> Result {
    let args: Vec<String> = args.collect();
    let builtin = builtin_commands();
    let mut commands: Vec<&Command> = extra.iter().collect();
    for b in &builtin {
        if !commands.iter().any(|c| c.name == b.name) {
            commands.push(b);
        }
    }
    let usage = || {
        let mut s = String::from("usage: cargo xtask <command>\n\ncommands:\n");
        for c in &commands {
            s.push_str("  ");
            s.push_str(c.help);
            s.push('\n');
        }
        s
    };
    let Some(cmd) = args.first() else {
        return Err(usage());
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            print!("{}", usage());
            Ok(())
        }
        name => match commands.iter().find(|c| c.name == name) {
            Some(c) => (c.run)(&args[1..]).map_err(|e| format!("{name}: {e}")),
            None => Err(format!("unknown command `{name}`\n\n{}", usage())),
        },
    }
}

/// The workspace root: the directory of the `Cargo.toml` that
/// `cargo locate-project --workspace` names from the current directory,
/// found once.
pub fn root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let out = cargo()
            .args(["locate-project", "--workspace", "--message-format", "plain"])
            .output()
            .expect("cargo locate-project runs");
        let manifest = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        Path::new(&manifest)
            .parent()
            .expect("the workspace manifest has a directory")
            .to_path_buf()
    })
    .clone()
}

/// Runs a command in the workspace root, failing on a non-zero exit.
pub fn exec(cmd: &mut Process) -> Result {
    let status = cmd
        .current_dir(root())
        .status()
        .map_err(|e| format!("could not run {cmd:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?} failed ({status})"))
    }
}

/// Runs a command in the workspace root and returns its stdout.
pub fn output(cmd: &mut Process) -> Result<String> {
    let out = cmd
        .current_dir(root())
        .output()
        .map_err(|e| format!("could not run {cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{cmd:?} failed ({}):\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("{cmd:?}: output is not UTF-8: {e}"))
}

/// The `cargo` that runs us (`$CARGO`), so the toolchain matches.
pub fn cargo() -> Process {
    Process::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

/// Copies a directory tree.
pub fn copy_dir(from: &Path, to: &Path) -> Result {
    let entries =
        std::fs::read_dir(from).map_err(|e| format!("reading {}: {e}", from.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let dest = to.join(path.file_name().unwrap());
        if path.is_dir() {
            std::fs::create_dir_all(&dest)
                .map_err(|e| format!("creating {}: {e}", dest.display()))?;
            copy_dir(&path, &dest)?;
        } else {
            std::fs::copy(&path, &dest).map_err(|e| format!("copying {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// `--key value` and `--flag` out of an argument list.
pub fn opt(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

pub fn flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}
