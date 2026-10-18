//! `cargo xtask web [--release] [--profile <name>] [--only <backend>]`: the
//! web build. Bevy picks its GPU backend at compile time, so a game has one
//! wasm per backend (WebGPU, and WebGL2 as the fallback), each built with
//! its own `--cfg` in its own target directory so neither build's cache
//! throws the other's away; the page picks between them. Then wasm-bindgen
//! (`--target web`), wasm-opt (`-Oz`, release only), brotli and gzip
//! precompression, the page's files, the side wasms (AudioWorklets,
//! workers), the runtime files and `build.json`.
//!
//! The build is made in `<dist>.staging/` and swapped in whole by two
//! renames, so `<dist>/` keeps serving the last complete build for the
//! minutes a build takes. Nothing here knows or writes a port: the
//! registered dev server (`template/tools/serve.py`) lays `<dist>/` over
//! the repository root, and every URL the page uses is relative.

use crate::{Command, Result, cargo, copy_dir, exec, output, root};
use std::path::{Path, PathBuf};
use std::process::Command as Process;

pub const TARGET: &str = "wasm32-unknown-unknown";

/// Features the wasm32 target enables by default in current stable Rust;
/// wasm-opt must be told, or it rejects the module.
pub const WASM_FEATURES: &[&str] = &[
    "--enable-bulk-memory",
    "--enable-nontrapping-float-to-int",
    "--enable-sign-ext",
    "--enable-mutable-globals",
    "--enable-reference-types",
    "--enable-multivalue",
];

/// A game's extra `build.json` fields, given the workspace root.
pub type BuildJsonExtra = fn(&Path) -> Result<serde_json::Map<String, serde_json::Value>>;

/// One build of the client.
pub struct Backend {
    /// What the page calls it (`webgpu`, `webgl2`); `--only` names it.
    pub name: &'static str,
    /// wasm-bindgen's `--out-name`: `<out>.js` and `<out>_bg.wasm`.
    pub out_name: &'static str,
    /// Below `target/`; none for the default target directory.
    pub target_dir: Option<&'static str>,
    /// A `--cfg` for this build (`pk_webgl2`), added to rustflags.
    pub cfg: Option<&'static str>,
}

/// A crate that is also its own small wasm beside the page: an
/// AudioWorklet processor or a worker. Built with the C-ABI exports of a
/// feature, at `--release` (or `web-release` for a release web build) even
/// for a dev build of the client, since it runs per sample or builds a
/// kilometre of world at a time.
pub struct SideWasm {
    pub krate: &'static str,
    pub features: &'static [&'static str],
    /// wasm-opt's level for it (`-O3`: speed over size).
    pub opt_level: &'static str,
    /// The JS that loads it (a worklet shim, a worker script), as a path
    /// from the workspace root; copied beside the wasm and precompressed.
    pub shim_js: Option<&'static str>,
    /// What the log calls it.
    pub what: &'static str,
}

/// Repository files the page loads at run time, copied into the build so
/// it is whole wherever it is served: (source from the root, path under
/// the dist directory). A directory copies whole.
pub struct RuntimeFiles {
    pub required: &'static [(&'static str, &'static str)],
    /// Skipped with a note when the checkout lacks them.
    pub optional: &'static [(&'static str, &'static str)],
}

/// A game's web build.
pub struct WebBuild {
    /// The client crate (built with `--lib`).
    pub crate_name: &'static str,
    pub backends: &'static [Backend],
    /// The page's directory (`index.html`, `perf.js`, worker shims), copied
    /// whole into the build.
    pub page_dir: &'static str,
    /// The live build directory, from the root (`dist/next`).
    pub dist_dir: &'static str,
    pub runtime_files: RuntimeFiles,
    pub side_wasm: &'static [SideWasm],
    /// More `build.json` fields (the game's own), given the root.
    pub build_json_extra: Option<BuildJsonExtra>,
    /// The binaryen version `wasm-opt` must be (`"116"`); CI installs that
    /// one. Checked like wasm-bindgen's.
    pub wasm_opt: &'static str,
    /// `size --budget`'s default: the largest wasm after gzip, in bytes.
    pub size_budget_gzip: u64,
}

impl WebBuild {
    pub fn dist(&self) -> PathBuf {
        root().join(self.dist_dir)
    }

    pub fn backend(&self, name: &str) -> Option<&Backend> {
        self.backends.iter().find(|b| b.name == name)
    }

    fn backend_names(&self) -> String {
        self.backends
            .iter()
            .map(|b| b.name)
            .collect::<Vec<_>>()
            .join("|")
    }

    /// The release profile's wasm of a backend, as cargo wrote it.
    pub fn target_wasm(&self, b: &Backend, profile_dir: &str) -> PathBuf {
        let target_root = match b.target_dir {
            Some(d) => root().join("target").join(d),
            None => root().join("target"),
        };
        target_root
            .join(TARGET)
            .join(profile_dir)
            .join(format!("{}.wasm", self.crate_name))
    }
}

/// The `web`, `prof` and `size` commands of a build.
pub fn commands(web: &'static WebBuild) -> Vec<Command> {
    vec![
        Command::new(
            "web",
            "web [--release] [--profile <name>] [--only <backend>]\n\
             \x20                  build the web client into the dist directory",
            move |args| run(web, args),
        ),
        Command::new(
            "prof",
            "prof [--only <backend>]  dist/prof/: the release build with function names kept, for CPU profiles",
            move |args| crate::prof::run(web, args),
        ),
        Command::new(
            "size",
            "size [--budget [bytes]]  the built wasm's size (raw, gzip, brotli); --budget fails over the limit",
            move |args| crate::size::run(web, args),
        ),
    ]
}

struct Options {
    release: bool,
    profile: String,
    only: Option<String>,
}

fn parse(web: &WebBuild, args: &[String]) -> Result<Options> {
    let mut release = false;
    let mut profile: Option<String> = None;
    let mut only: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--release" => release = true,
            "--profile" => profile = Some(it.next().ok_or("--profile needs a name")?.clone()),
            "--only" => {
                only = Some(
                    it.next()
                        .ok_or_else(|| format!("--only needs {}", web.backend_names()))?
                        .clone(),
                )
            }
            bad => {
                return Err(format!(
                    "unknown option `{bad}` (--release, --profile <name>, --only {})",
                    web.backend_names()
                ));
            }
        }
    }
    if let Some(o) = &only
        && web.backend(o).is_none()
    {
        return Err(format!("--only {o}: {}", web.backend_names()));
    }
    // `--profile` names any cargo profile; `--release` is `web-release`,
    // and either one is optimised with wasm-opt and precompressed.
    let release = release || profile.is_some();
    let profile = profile.unwrap_or_else(|| if release { "web-release" } else { "dev" }.into());
    Ok(Options {
        release,
        profile,
        only,
    })
}

pub fn run(web: &WebBuild, args: &[String]) -> Result {
    let o = parse(web, args)?;
    let root = root();

    check_bindgen_version()?;
    if o.release {
        check_wasm_opt_version(web.wasm_opt)?;
    }

    let live = web.dist();
    let out = live.with_extension("staging");
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("clearing {}: {e}", out.display()))?;
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    // `--only` replaces one build's files and keeps the other's.
    if o.only.is_some() && live.is_dir() {
        copy_dir(&live, &out)?;
    }

    let mut built = Vec::new();
    for b in web
        .backends
        .iter()
        .filter(|b| o.only.as_deref().is_none_or(|n| n == b.name))
    {
        build_backend(web, b, &o, &out)?;
        built.push(b.name);
    }

    copy_dir(&root.join(web.page_dir), &out)?;
    for s in web.side_wasm {
        build_side(s, o.release, &out)?;
    }
    copy_runtime_files(&web.runtime_files, &root, &out)?;
    write_build_json(web, &o, &out, &root)?;
    swap_in(&out, &live)?;

    println!(
        "web: built {} ({}) into {}/\n\
         \x20    open / on the registered server (proj up <project>, or ./serve.sh)",
        built.join(" and "),
        o.profile,
        web.dist_dir
    );
    Ok(())
}

/// `build.json`, what the page's build line and the tools read: the
/// profile, the backends present, the commit (`+` when the tree is dirty),
/// the day, the compiler, the wasm-opt version (release builds), and the
/// game's own fields.
fn write_build_json(web: &WebBuild, o: &Options, out: &Path, root: &Path) -> Result {
    use serde_json::{Map, Value, json};
    let backends: Vec<Value> = web
        .backends
        .iter()
        .filter(|b| out.join(format!("{}_bg.wasm", b.out_name)).exists())
        .map(|b| json!(b.name))
        .collect();
    let commit = output(Process::new("git").args(["rev-parse", "--short=7", "HEAD"]))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let dirty = output(Process::new("git").args(["status", "--porcelain", "--untracked-files=no"]))
        .is_ok_and(|s| !s.trim().is_empty());
    let rustc = output(Process::new("rustc").arg("-V"))
        .map(|s| s.trim().trim_start_matches("rustc ").to_string())
        .unwrap_or_default();
    let date = utc_date(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    );
    let mut m = Map::new();
    m.insert("profile".into(), json!(o.profile));
    m.insert("backends".into(), Value::Array(backends));
    m.insert(
        "commit".into(),
        json!(format!("{commit}{}", if dirty { "+" } else { "" })),
    );
    m.insert("date".into(), json!(date));
    m.insert("rustc".into(), json!(rustc));
    if o.release {
        m.insert("wasm_opt".into(), json!(web.wasm_opt));
    }
    if let Some(extra) = web.build_json_extra {
        for (k, v) in extra(root)? {
            m.insert(k, v);
        }
    }
    let text = serde_json::to_string(&Value::Object(m)).map_err(|e| e.to_string())?;
    std::fs::write(out.join("build.json"), text + "\n")
        .map_err(|e| format!("writing build.json: {e}"))
}

/// `YYYY-MM-DD` for a Unix time (UTC), by the civil-from-days rule.
pub fn utc_date(secs: u64) -> String {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn copy_runtime_files(files: &RuntimeFiles, root: &Path, out: &Path) -> Result {
    let all = files
        .required
        .iter()
        .map(|f| (f, false))
        .chain(files.optional.iter().map(|f| (f, true)));
    for ((from, to), optional) in all {
        let (src, dest) = (root.join(from), out.join(to));
        if !src.exists() && optional {
            println!("note: {from} is not in this checkout; skipped");
            continue;
        }
        if src.is_dir() {
            std::fs::create_dir_all(&dest)
                .map_err(|e| format!("creating {}: {e}", dest.display()))?;
            copy_dir(&src, &dest)?;
        } else {
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("creating {}: {e}", dir.display()))?;
            }
            std::fs::copy(&src, &dest).map_err(|e| format!("copying {}: {e}", src.display()))?;
        }
    }
    Ok(())
}

/// Builds one backend's wasm and runs wasm-bindgen (and, for a release,
/// wasm-opt and the precompression) into `out`.
fn build_backend(web: &WebBuild, b: &Backend, o: &Options, out: &Path) -> Result {
    let root = root();
    let mut build = cargo();
    build.args([
        "build",
        "-p",
        web.crate_name,
        "--lib",
        "--target",
        TARGET,
        "--profile",
        &o.profile,
    ]);
    if let Some(d) = b.target_dir {
        build.arg("--target-dir").arg(root.join("target").join(d));
    }
    if let Some(cfg) = b.cfg {
        add_cfg(&mut build, cfg);
    }
    println!("web: building the {} client", b.name);
    exec(&mut build)?;

    // The dev profile's output directory is called debug.
    let profile_dir = if o.profile == "dev" {
        "debug"
    } else {
        &o.profile
    };
    let wasm = web.target_wasm(b, profile_dir);
    exec(
        Process::new("wasm-bindgen")
            .args([
                "--target",
                "web",
                "--no-typescript",
                "--out-name",
                b.out_name,
                "--out-dir",
            ])
            .arg(out)
            .arg(&wasm),
    )?;

    let bg = format!("{}_bg.wasm", b.out_name);
    if o.release {
        // -Oz: size first (about 1.3 MB less after gzip than -O3 for 0.1
        // to 0.2 ms more per CPU-bound frame, pictures identical).
        wasm_opt(&["-Oz"], &out.join(&bg))?;
        precompress(out, &[&bg, &format!("{}.js", b.out_name)])?;
    }
    Ok(())
}

fn build_side(s: &SideWasm, release: bool, out: &Path) -> Result {
    let root = root();
    let profile = if release { "web-release" } else { "release" };
    let mut build = cargo();
    build.args(["build", "-p", s.krate, "--lib"]);
    if !s.features.is_empty() {
        build.arg("--features").arg(s.features.join(","));
    }
    build.args(["--target", TARGET, "--profile", profile]);
    println!("web: building the {} wasm", s.what);
    exec(&mut build)?;
    let name = format!("{}.wasm", s.krate);
    let wasm = root.join("target").join(TARGET).join(profile).join(&name);
    let dest = out.join(&name);
    std::fs::copy(&wasm, &dest).map_err(|e| format!("copying {}: {e}", wasm.display()))?;
    let mut names = vec![name.clone()];
    if let Some(js) = s.shim_js {
        let src = root.join(js);
        let file = src
            .file_name()
            .ok_or_else(|| format!("{js}: no file name"))?
            .to_string_lossy()
            .into_owned();
        std::fs::copy(&src, out.join(&file))
            .map_err(|e| format!("copying {}: {e}", src.display()))?;
        names.push(file);
    }
    if release {
        wasm_opt(&[s.opt_level], &dest)?;
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        precompress(out, &names)?;
    }
    Ok(())
}

/// wasm-opt in place, with the target's features.
pub fn wasm_opt(flags: &[&str], wasm: &Path) -> Result {
    exec(
        Process::new("wasm-opt")
            .args(flags)
            .args(WASM_FEATURES)
            .arg(wasm)
            .arg("-o")
            .arg(wasm),
    )
    .map_err(|e| format!("{e}\n(install binaryen: cargo install wasm-opt --locked)"))
}

/// Adds `--cfg <cfg>` to a cargo command's rustflags, keeping the ones the
/// caller set (CI sets `-D warnings`).
pub fn add_cfg(cmd: &mut Process, cfg: &str) {
    let enc = std::env::var("CARGO_ENCODED_RUSTFLAGS").ok();
    let flags = std::env::var("RUSTFLAGS").unwrap_or_default();
    let (var, value) = add_cfg_value(enc.as_deref(), &flags, cfg);
    cmd.env(var, value);
}

/// The variable to set and its value: `CARGO_ENCODED_RUSTFLAGS` when cargo
/// set it (0x1f-separated), else `RUSTFLAGS` with the flag appended.
fn add_cfg_value(encoded: Option<&str>, flags: &str, cfg: &str) -> (&'static str, String) {
    match encoded {
        Some(enc) => {
            let sep = if enc.is_empty() { "" } else { "\x1f" };
            (
                "CARGO_ENCODED_RUSTFLAGS",
                format!("{enc}{sep}--cfg\x1f{cfg}"),
            )
        }
        None => (
            "RUSTFLAGS",
            format!("{flags} --cfg {cfg}").trim().to_owned(),
        ),
    }
}

/// The wasm-bindgen CLI must be the same version as the crate in Cargo.lock,
/// or the glue it writes does not match the module.
pub fn check_bindgen_version() -> Result {
    let lock = std::fs::read_to_string(root().join("Cargo.lock"))
        .map_err(|e| format!("reading Cargo.lock: {e}"))?;
    let want = locked_version(&lock, "wasm-bindgen").ok_or("wasm-bindgen is not in Cargo.lock")?;
    let have = output(Process::new("wasm-bindgen").arg("--version")).map_err(|_| {
        format!("wasm-bindgen CLI not found; install: cargo install wasm-bindgen-cli --version {want} --locked")
    })?;
    let have = bindgen_version(&have);
    if have != want {
        return Err(format!(
            "wasm-bindgen CLI is {have}, the crate is {want}; install: \
             cargo install wasm-bindgen-cli --version {want} --locked"
        ));
    }
    Ok(())
}

/// `wasm-bindgen 0.2.129` → `0.2.129`.
pub fn bindgen_version(text: &str) -> String {
    text.split_whitespace().nth(1).unwrap_or("").to_owned()
}

/// `wasm-opt` must be the binaryen version the build pins (`WebBuild::
/// wasm_opt`): another version optimises differently, and a build made
/// here would not match CI's.
pub fn check_wasm_opt_version(want: &str) -> Result {
    let have = output(Process::new("wasm-opt").arg("--version")).map_err(|_| {
        format!(
            "wasm-opt not found; install binaryen version {want} \
             (https://github.com/WebAssembly/binaryen/releases/tag/version_{want})"
        )
    })?;
    let have = wasm_opt_version(&have);
    if have != want {
        return Err(format!(
            "wasm-opt is binaryen {have}, the build pins {want}; install \
             https://github.com/WebAssembly/binaryen/releases/tag/version_{want}"
        ));
    }
    Ok(())
}

/// `wasm-opt version 116 (version_116)` → `116`; `wasm-opt version 123` → `123`.
pub fn wasm_opt_version(text: &str) -> String {
    text.split_whitespace()
        .skip_while(|w| *w != "version")
        .nth(1)
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_ascii_digit())
        .to_owned()
}

pub fn locked_version(lock: &str, name: &str) -> Option<String> {
    let needle = format!("name = \"{name}\"");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() == needle {
            let v = lines.next()?.trim();
            return Some(
                v.strip_prefix("version = \"")?
                    .strip_suffix('"')?
                    .to_owned(),
            );
        }
    }
    None
}

/// Writes `<file>.br` and `<file>.gz` beside the given files, for a server
/// that sends precompressed files, so load times on phones are as they will
/// be: brotli to browsers that take it (every current one, on https), gzip
/// to the rest.
pub fn precompress(out: &Path, names: &[&str]) -> Result {
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;
    for name in names {
        let src = out.join(name);
        let bytes = std::fs::read(&src).map_err(|e| format!("reading {}: {e}", src.display()))?;
        let mut gz = GzEncoder::new(Vec::new(), Compression::best());
        gz.write_all(&bytes).map_err(|e| e.to_string())?;
        let gz = gz.finish().map_err(|e| e.to_string())?;
        let dest = out.join(format!("{name}.gz"));
        std::fs::write(&dest, gz).map_err(|e| format!("writing {}: {e}", dest.display()))?;

        // Quality 11 with the largest standard window (16 MB): the wasm is
        // built once and downloaded many times.
        let mut br = Vec::new();
        let params = brotli::enc::BrotliEncoderParams {
            quality: 11,
            lgwin: 24,
            size_hint: bytes.len(),
            ..Default::default()
        };
        brotli::BrotliCompress(&mut &bytes[..], &mut br, &params).map_err(|e| e.to_string())?;
        let dest = out.join(format!("{name}.br"));
        std::fs::write(&dest, br).map_err(|e| format!("writing {}: {e}", dest.display()))?;
    }
    Ok(())
}

/// Puts the finished build in place: two renames, so the live directory
/// is missing for a moment rather than incomplete for minutes.
pub fn swap_in(staging: &Path, live: &Path) -> Result {
    let old = live.with_extension("old");
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(|e| format!("clearing {}: {e}", old.display()))?;
    }
    if live.exists() {
        std::fs::rename(live, &old).map_err(|e| format!("moving {} aside: {e}", live.display()))?;
    }
    std::fs::rename(staging, live)
        .map_err(|e| format!("moving {} into place: {e}", staging.display()))?;
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(|e| format!("removing {}: {e}", old.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(951_782_400), "2000-02-29");
        assert_eq!(utc_date(1_791_331_200), "2026-10-07");
    }

    #[test]
    fn reads_locked_version() {
        let lock = "[[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.129\"\n";
        assert_eq!(
            locked_version(lock, "wasm-bindgen").as_deref(),
            Some("0.2.129")
        );
        assert_eq!(locked_version(lock, "js-sys"), None);
    }

    #[test]
    fn version_parsers() {
        assert_eq!(bindgen_version("wasm-bindgen 0.2.129\n"), "0.2.129");
        assert_eq!(
            wasm_opt_version("wasm-opt version 116 (version_116)\n"),
            "116"
        );
        assert_eq!(wasm_opt_version("wasm-opt version 123\n"), "123");
        assert_eq!(wasm_opt_version("garbage"), "");
    }

    #[test]
    fn add_cfg_keeps_the_callers_flags() {
        assert_eq!(
            add_cfg_value(None, "-D warnings", "pk_webgl2"),
            ("RUSTFLAGS", "-D warnings --cfg pk_webgl2".to_owned())
        );
        assert_eq!(
            add_cfg_value(None, "", "pk_webgl2"),
            ("RUSTFLAGS", "--cfg pk_webgl2".to_owned())
        );
        assert_eq!(
            add_cfg_value(Some("-D\x1fwarnings"), "", "pk_webgl2"),
            (
                "CARGO_ENCODED_RUSTFLAGS",
                "-D\x1fwarnings\x1f--cfg\x1fpk_webgl2".to_owned()
            )
        );
        assert_eq!(
            add_cfg_value(Some(""), "", "x"),
            ("CARGO_ENCODED_RUSTFLAGS", "--cfg\x1fx".to_owned())
        );
    }
}
