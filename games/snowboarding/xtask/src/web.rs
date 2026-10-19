//! `cargo xtask web [--release] [--only webgpu|webgl2]`: build the web
//! client into `dist/web/`: the WebGPU build and the WebGL2 one, which the
//! page picks between. `tools/serve.py` lays `dist/web/` over the root, so
//! the game is at `/` on the registered server. Nothing here knows or
//! writes a port.
//!
//! Borrowed from midnight-playground's `xtask web`: the build is made in
//! `dist/web.staging/` and swapped in whole once done, so `dist/web/` keeps
//! serving the last complete build while a new one compiles.

use crate::{Result, cargo, exec, output, root, target_dir, workspace_root};
use std::path::Path;
use std::process::Command;

pub const TARGET: &str = "wasm32-unknown-unknown";

/// Features the wasm32 target enables by default in current stable Rust;
/// wasm-opt must be told, or it rejects the module.
const WASM_FEATURES: &[&str] = &[
    "--enable-bulk-memory",
    "--enable-nontrapping-float-to-int",
    "--enable-sign-ext",
    "--enable-mutable-globals",
    "--enable-reference-types",
    "--enable-multivalue",
];

/// One build of the client. Bevy picks its backend at compile time, so the
/// WebGL2 fallback is a second wasm file, built with the `sb_webgl2` cfg in
/// its own target directory so neither build's cache is thrown away.
struct Backend {
    name: &'static str,
    out_name: &'static str,
    target_dir: Option<&'static str>,
    cfg: Option<&'static str>,
}

const BACKENDS: [Backend; 2] = [
    Backend {
        name: "webgpu",
        out_name: "sb_game",
        target_dir: None,
        cfg: None,
    },
    Backend {
        name: "webgl2",
        out_name: "sb_game_webgl2",
        target_dir: Some("webgl2"),
        cfg: Some("sb_webgl2"),
    },
];

/// Files the page loads at run time, copied beside it: (source, dest).
const RUNTIME_FILES: &[(&str, &str)] = &[
    (
        "assets/fonts/rajdhani/Rajdhani-SemiBold.ttf",
        "assets/fonts/rajdhani/Rajdhani-SemiBold.ttf",
    ),
    (
        "assets/fonts/rajdhani/Rajdhani-Bold.ttf",
        "assets/fonts/rajdhani/Rajdhani-Bold.ttf",
    ),
    (
        "assets/fonts/rajdhani/OFL.txt",
        "assets/fonts/rajdhani/OFL.txt",
    ),
];

pub fn run(args: &[String]) -> Result {
    let mut release = false;
    let mut only: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--release" => release = true,
            "--only" => only = Some(it.next().ok_or("--only needs webgpu or webgl2")?.clone()),
            bad => {
                return Err(format!(
                    "unknown option `{bad}` (--release, --only webgpu|webgl2)"
                ));
            }
        }
    }
    if let Some(o) = &only
        && !BACKENDS.iter().any(|b| b.name == o)
    {
        return Err(format!("--only {o}: webgpu or webgl2"));
    }
    let root = root();
    let profile = if release { "web-release" } else { "dev" };
    check_bindgen_version()?;

    let live = root.join("dist").join("web");
    let out = root.join("dist").join("web.staging");
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("clearing {}: {e}", out.display()))?;
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    // `--only` replaces one build's files and keeps the other's.
    if only.is_some() && live.is_dir() {
        copy_dir(&live, &out)?;
    }

    let mut built = Vec::new();
    for b in BACKENDS
        .iter()
        .filter(|b| only.as_deref().is_none_or(|o| o == b.name))
    {
        build_backend(b, release, profile, &out)?;
        built.push(b.name);
    }
    build_music(release, &out)?;
    copy_dir(&root.join("crates/sb_game/web"), &out)?;
    // The announcers' clips, so the build is whole on its own.
    let clips = root.join("audio/announcers");
    if clips.is_dir() {
        let dest = out.join("audio/announcers");
        std::fs::create_dir_all(&dest).map_err(|e| format!("creating {}: {e}", dest.display()))?;
        copy_dir(&clips, &dest)?;
    }
    for (from, to) in RUNTIME_FILES {
        let dest = out.join(to);
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        std::fs::copy(root.join(from), &dest).map_err(|e| format!("copying {from}: {e}"))?;
    }

    let backends = BACKENDS
        .iter()
        .filter(|b| out.join(format!("{}_bg.wasm", b.out_name)).exists())
        .map(|b| format!("\"{}\"", b.name))
        .collect::<Vec<_>>()
        .join(", ");
    let commit = output(Command::new("git").args(["rev-parse", "--short=7", "HEAD"]))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "uncommitted".into());
    let rustc = output(Command::new("rustc").arg("-V"))
        .map(|s| s.trim().trim_start_matches("rustc ").to_string())
        .unwrap_or_default();
    let date = utc_date(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    );
    std::fs::write(
        out.join("build.json"),
        format!(
            "{{\"profile\": \"{profile}\", \"backends\": [{backends}], \"commit\": \"{commit}\", \"date\": \"{date}\", \"rustc\": \"{}\"}}\n",
            rustc.replace('"', "'")
        ),
    )
    .map_err(|e| format!("writing build.json: {e}"))?;
    swap_in(&out, &live)?;
    println!(
        "web: built {} ({profile}) into dist/web/\n\
         \x20    open / on the registered server (proj up snowboarding, or ./serve.sh)",
        built.join(" and ")
    );
    Ok(())
}

/// `cargo xtask size`: what the page downloads.
pub fn size() -> Result {
    let dir = root().join("dist").join("web");
    for b in &BACKENDS {
        let wasm = dir.join(format!("{}_bg.wasm", b.out_name));
        let Ok(meta) = std::fs::metadata(&wasm) else {
            println!("{}: not built", b.name);
            continue;
        };
        let mb = |n: u64| n as f64 / 1048576.0;
        let gz = std::fs::metadata(wasm.with_extension("wasm.gz")).map(|m| m.len());
        let br = std::fs::metadata(wasm.with_extension("wasm.br")).map(|m| m.len());
        println!(
            "{}: {:.2} MB raw, {} gzip, {} brotli",
            b.name,
            mb(meta.len()),
            gz.map_or("-".into(), |n| format!("{:.2} MB", mb(n))),
            br.map_or("-".into(), |n| format!("{:.2} MB", mb(n)))
        );
    }
    Ok(())
}

/// `YYYY-MM-DD` for a Unix time (UTC), by the civil-from-days rule.
fn utc_date(secs: u64) -> String {
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

/// Builds one backend's wasm and runs wasm-bindgen (and, for a release,
/// wasm-opt and precompression) into `out`.
fn build_backend(b: &Backend, release: bool, profile: &str, out: &Path) -> Result {
    let target_root = match b.target_dir {
        Some(d) => target_dir().join(d),
        None => target_dir(),
    };
    let mut build = cargo();
    build.args([
        "build",
        "-p",
        "sb_game",
        "--lib",
        "--target",
        TARGET,
        "--profile",
        profile,
    ]);
    if b.target_dir.is_some() {
        build.arg("--target-dir").arg(&target_root);
    }
    if let Some(cfg) = b.cfg {
        add_cfg(&mut build, cfg);
    }
    println!("web: building the {} client", b.name);
    exec(&mut build)?;

    let profile_dir = if release { "web-release" } else { "debug" };
    let wasm = target_root
        .join(TARGET)
        .join(profile_dir)
        .join("sb_game.wasm");
    exec(
        Command::new("wasm-bindgen")
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
    if release {
        let bg = format!("{}_bg.wasm", b.out_name);
        exec(
            Command::new("wasm-opt")
                .arg("-Oz")
                .args(WASM_FEATURES)
                .arg(out.join(&bg))
                .arg("-o")
                .arg(out.join(&bg)),
        )
        .map_err(|e| format!("{e}\n(install with: cargo install wasm-opt --locked)"))?;
        precompress(out, &[&bg, &format!("{}.js", b.out_name)])?;
    }
    Ok(())
}

/// The music: `sb_music` as its own small wasm with the C-ABI exports of
/// its `worklet` feature, which `web/music-worklet.js` (copied with the
/// page) runs on the audio thread. Optimised even for a debug build (the
/// `release` profile): it runs per sample. Borrowed from midnight-
/// playground's `build_worklet`.
fn build_music(release: bool, out: &Path) -> Result {
    let profile = if release { "web-release" } else { "release" };
    let mut build = cargo();
    build.args([
        "build",
        "-p",
        "sb_music",
        "--lib",
        "--features",
        "worklet",
        "--target",
        TARGET,
        "--profile",
        profile,
    ]);
    println!("web: building the music worklet");
    exec(&mut build)?;
    let name = "sb_music.wasm";
    let wasm = target_dir().join(TARGET).join(profile).join(name);
    let dest = out.join(name);
    std::fs::copy(&wasm, &dest).map_err(|e| format!("copying {}: {e}", wasm.display()))?;
    if release {
        exec(
            Command::new("wasm-opt")
                .arg("-O3")
                .args(WASM_FEATURES)
                .arg(&dest)
                .arg("-o")
                .arg(&dest),
        )
        .map_err(|e| format!("{e}\n(install with: cargo install wasm-opt --locked)"))?;
        precompress(out, &[name])?;
    }
    Ok(())
}

/// Adds `--cfg <cfg>` to a cargo command's rustflags, keeping the caller's.
fn add_cfg(cmd: &mut Command, cfg: &str) {
    if let Ok(enc) = std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        let sep = if enc.is_empty() { "" } else { "\x1f" };
        cmd.env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!("{enc}{sep}--cfg\x1f{cfg}"),
        );
    } else {
        let flags = std::env::var("RUSTFLAGS").unwrap_or_default();
        cmd.env("RUSTFLAGS", format!("{flags} --cfg {cfg}").trim());
    }
}

/// The wasm-bindgen CLI must be the same version as the crate in Cargo.lock,
/// or the glue it writes does not match the module.
fn check_bindgen_version() -> Result {
    let lock = std::fs::read_to_string(workspace_root().join("Cargo.lock"))
        .map_err(|e| format!("reading Cargo.lock: {e}"))?;
    let want = locked_version(&lock, "wasm-bindgen").ok_or("wasm-bindgen is not in Cargo.lock")?;
    let have = output(Command::new("wasm-bindgen").arg("--version")).map_err(|_| {
        format!("wasm-bindgen CLI not found; install: cargo install wasm-bindgen-cli --version {want} --locked")
    })?;
    let have = have.split_whitespace().nth(1).unwrap_or("").to_owned();
    if have != want {
        return Err(format!(
            "wasm-bindgen CLI is {have}, the crate is {want}; install: \
             cargo install wasm-bindgen-cli --version {want} --locked"
        ));
    }
    Ok(())
}

fn locked_version(lock: &str, name: &str) -> Option<String> {
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

/// Writes `<file>.br` and `<file>.gz` beside the given files; `tools/serve.py`
/// sends them to browsers that take them.
fn precompress(out: &Path, names: &[&str]) -> Result {
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;
    for name in names {
        let src = out.join(name);
        let bytes = std::fs::read(&src).map_err(|e| format!("reading {}: {e}", src.display()))?;
        let mut gz = GzEncoder::new(Vec::new(), Compression::best());
        gz.write_all(&bytes).map_err(|e| e.to_string())?;
        let gz = gz.finish().map_err(|e| e.to_string())?;
        std::fs::write(out.join(format!("{name}.gz")), gz).map_err(|e| e.to_string())?;
        let mut br = Vec::new();
        let params = brotli::enc::BrotliEncoderParams {
            quality: 11,
            lgwin: 24,
            size_hint: bytes.len(),
            ..Default::default()
        };
        brotli::BrotliCompress(&mut &bytes[..], &mut br, &params).map_err(|e| e.to_string())?;
        std::fs::write(out.join(format!("{name}.br")), br).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Puts the finished build in place: two renames, so `dist/web/` is missing
/// for a moment rather than incomplete for minutes.
fn swap_in(staging: &Path, live: &Path) -> Result {
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

fn copy_dir(from: &Path, to: &Path) -> Result {
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

#[cfg(test)]
mod tests {
    #[test]
    fn utc_dates() {
        assert_eq!(super::utc_date(0), "1970-01-01");
        assert_eq!(super::utc_date(951_782_400), "2000-02-29");
    }
}
