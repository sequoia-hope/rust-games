//! `cargo xtask check-deps`: the crate dependency rules of a workspace,
//! read from `deps.toml` at its root and checked from `cargo tree` and
//! from the crates' sources.
//!
//! Every workspace member needs a rule, so a new crate cannot slip in
//! unchecked. A path dependency outside the workspace (a game's `pk_*`
//! crates from the sibling Playground Kit checkout) is not a member: it is
//! allowed by default and still subject to the `banned` tree scan.
//!
//! ```toml
//! # Named sets of banned crate patterns (a trailing `*` is a prefix).
//! [banned]
//! sim = ["bevy*", "wgpu*", "web-sys", "js-sys", "wasm-bindgen*", "rand*", "getrandom", "rayon*", "crossbeam*"]
//! no_bevy = ["bevy*"]
//!
//! [crates.my_sim]
//! allowed = ["my_math"]    # workspace crates it may depend on directly; ["*"] for any
//! banned = "sim"           # a set's name, or a list of patterns
//! pure = true              # #![forbid(unsafe_code)]; no clock, threads or hash maps in the source
//! exact_math = true        # no float methods that call the platform's libm (use the kernel)
//! ```

use crate::{Result, cargo, output, root};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    banned: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    crates: BTreeMap<String, RuleToml>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RuleToml {
    #[serde(default)]
    allowed: Vec<String>,
    #[serde(default)]
    banned: Banned,
    #[serde(default)]
    pure: bool,
    #[serde(default)]
    exact_math: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Banned {
    Named(String),
    List(Vec<String>),
}

impl Default for Banned {
    fn default() -> Self {
        Banned::List(Vec::new())
    }
}

/// One crate's rule.
pub struct Rule {
    pub krate: String,
    /// Workspace crates this one may depend on directly; `*` for any.
    pub allowed: Vec<String>,
    /// Crates that must not appear anywhere in its normal dependency tree,
    /// for any target, with default features. A trailing `*` is a prefix.
    pub banned: Vec<String>,
    /// Simulation rules: `#![forbid(unsafe_code)]`, and no clock, threads or
    /// hash maps in the source.
    pub pure: bool,
    /// No float methods that call the platform's math library (`x.sin()`,
    /// `f64::exp`, `mul_add`, ...): the math kernel is used instead.
    pub exact_math: bool,
}

/// Parses a `deps.toml`.
pub fn parse_rules(text: &str) -> Result<Vec<Rule>> {
    let file: File = toml::from_str(text).map_err(|e| format!("deps.toml: {e}"))?;
    let mut rules = Vec::new();
    for (krate, r) in file.crates {
        let banned = match r.banned {
            Banned::List(l) => l,
            Banned::Named(n) => file.banned.get(&n).cloned().ok_or_else(|| {
                format!("deps.toml: {krate}: banned = \"{n}\" names no [banned] set")
            })?,
        };
        rules.push(Rule {
            krate,
            allowed: r.allowed,
            banned,
            pure: r.pure,
            exact_math: r.exact_math,
        });
    }
    Ok(rules)
}

/// Float methods that are not exact on every platform, plus `mul_add`, which
/// changes rounding.
pub const INEXACT: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sin_cos", "exp", "exp2", "exp_m1", "ln",
    "ln_1p", "log", "log2", "log10", "powf", "powi", "sinh", "cosh", "tanh", "asinh", "acosh",
    "atanh", "hypot", "cbrt", "mul_add",
];

/// Source tokens the pure crates must not use.
pub const IMPURE: &[&str] = &[
    "std::time",
    "std::thread",
    "HashMap",
    "HashSet",
    "Instant",
    "SystemTime",
];

pub fn run(_args: &[String]) -> Result {
    let path = root().join("deps.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "reading {}: {e} (every workspace needs one)",
            path.display()
        )
    })?;
    let rules = parse_rules(&text)?;
    let dirs = members()?;
    let members: BTreeSet<String> = dirs.keys().cloned().collect();
    let mut problems = Vec::new();

    for m in &members {
        if !rules.iter().any(|r| r.krate == *m) {
            problems.push(format!("{m}: workspace member with no rule in deps.toml"));
        }
    }

    for rule in rules.iter().filter(|r| members.contains(&r.krate)) {
        check_tree(rule, &members, &mut problems)?;
        if rule.pure || rule.exact_math {
            check_source(rule, &dirs[&rule.krate], &mut problems)?;
        }
    }

    if problems.is_empty() {
        println!("check-deps: {} crates follow the rules", members.len());
        Ok(())
    } else {
        for p in &problems {
            eprintln!("  {p}");
        }
        Err(format!("{} violation(s)", problems.len()))
    }
}

/// Workspace members and their crate directories.
fn members() -> Result<BTreeMap<String, PathBuf>> {
    let json = output(cargo().args(["metadata", "--no-deps", "--format-version", "1"]))?;
    let meta: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("cargo metadata: {e}"))?;
    Ok(meta["packages"]
        .as_array()
        .ok_or("cargo metadata: no packages")?
        .iter()
        .filter_map(|p| {
            let name = p["name"].as_str()?.to_owned();
            let dir = Path::new(p["manifest_path"].as_str()?)
                .parent()?
                .to_path_buf();
            Some((name, dir))
        })
        .collect())
}

/// Crate names in `cargo tree` output, one per line, first token.
fn tree(krate: &str, depth: Option<u32>) -> Result<Vec<String>> {
    let mut cmd = cargo();
    cmd.args([
        "tree", "-p", krate, "-e", "normal", "--target", "all", "--prefix", "none",
    ]);
    if let Some(d) = depth {
        cmd.args(["--depth", &d.to_string()]);
    }
    let text = output(&mut cmd)?;
    Ok(text
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}

fn check_tree(rule: &Rule, members: &BTreeSet<String>, problems: &mut Vec<String>) -> Result {
    if !rule.allowed.iter().any(|a| a == "*") {
        for dep in tree(&rule.krate, Some(1))?.iter().skip(1) {
            if members.contains(dep) && !rule.allowed.contains(dep) {
                problems.push(format!(
                    "{}: may not depend on {dep} (deps.toml)",
                    rule.krate
                ));
            }
        }
    }
    let all: BTreeSet<String> = tree(&rule.krate, None)?.into_iter().skip(1).collect();
    for dep in &all {
        if rule.banned.iter().any(|b| matches(b, dep)) {
            problems.push(format!(
                "{}: {dep} is in its dependency tree (banned)",
                rule.krate
            ));
        }
    }
    Ok(())
}

pub fn matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    }
}

fn check_source(rule: &Rule, crate_dir: &Path, problems: &mut Vec<String>) -> Result {
    let dir = crate_dir.join("src");
    if rule.pure {
        let lib = std::fs::read_to_string(dir.join("lib.rs"))
            .map_err(|e| format!("{}: reading lib.rs: {e}", rule.krate))?;
        if !lib.contains("#![forbid(unsafe_code)]") {
            problems.push(format!(
                "{}: lib.rs lacks #![forbid(unsafe_code)]",
                rule.krate
            ));
        }
    }
    let mut files = Vec::new();
    rust_files(&dir, &mut files)?;
    for file in files {
        let text = std::fs::read_to_string(&file)
            .map_err(|e| format!("reading {}: {e}", file.display()))?;
        let rel = file
            .strip_prefix(root())
            .unwrap_or(&file)
            .display()
            .to_string();
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if rule.pure {
                for tok in IMPURE {
                    if code.contains(tok) {
                        problems.push(format!("{rel}:{}: `{tok}` in a pure crate", n + 1));
                    }
                }
            }
            if rule.exact_math {
                for m in INEXACT {
                    if calls_method(code, m) {
                        problems.push(format!(
                            "{rel}:{}: `{m}` calls the platform's math; use the kernel (pk_math)",
                            n + 1
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// `x.sin(` or `f64::sin(` (also `f32::`), but not `libm::sin(` or `kernel::sin(`.
pub fn calls_method(code: &str, name: &str) -> bool {
    let method = format!(".{name}(");
    let assoc64 = format!("f64::{name}(");
    let assoc32 = format!("f32::{name}(");
    code.contains(&method) || code.contains(&assoc64) || code.contains(&assoc32)
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        assert!(matches("bevy*", "bevy_render"));
        assert!(matches("rand*", "rand_core"));
        assert!(!matches("web-sys", "web-sys-extra"));
        assert!(matches("getrandom", "getrandom"));
    }

    #[test]
    fn method_calls() {
        assert!(calls_method("let y = x.sin();", "sin"));
        assert!(calls_method("f64::exp(x)", "exp"));
        assert!(calls_method("a.mul_add(b, c)", "mul_add"));
        assert!(!calls_method("libm::sin(x)", "sin"));
        assert!(!calls_method("kernel::sin(x)", "sin"));
        assert!(!calls_method("x.sinh_like()", "sin"));
    }

    #[test]
    fn parses_rules() {
        let text = r#"
[banned]
sim = ["bevy*", "rand*"]

[crates.a]
pure = true
exact_math = true
banned = "sim"

[crates.b]
allowed = ["a"]
banned = ["bevy*"]

[crates.xtask]
allowed = ["*"]
"#;
        let rules = parse_rules(text).unwrap();
        let a = rules.iter().find(|r| r.krate == "a").unwrap();
        assert!(a.pure && a.exact_math && a.allowed.is_empty());
        assert_eq!(a.banned, ["bevy*", "rand*"]);
        let b = rules.iter().find(|r| r.krate == "b").unwrap();
        assert!(!b.pure && b.allowed == ["a"] && b.banned == ["bevy*"]);
        let x = rules.iter().find(|r| r.krate == "xtask").unwrap();
        assert!(x.allowed == ["*"] && x.banned.is_empty());
    }

    #[test]
    fn unknown_set_and_unknown_field_fail() {
        assert!(parse_rules("[crates.a]\nbanned = \"nope\"\n").is_err());
        assert!(parse_rules("[crates.a]\nallowd = []\n").is_err());
    }
}
