//! Settings and records, as a browser game keeps them (Midnight Playground
//! SPEC 8.3, `ui/store.rs`; ECOSYSTEM.md row 1 on why not bevy_pkv).
//!
//! The JS game kept every setting in `localStorage` under `<prefix><key>`,
//! each value JSON-encoded (`store.set(k, v)` is `localStorage.setItem(
//! prefix + k, JSON.stringify(v))`, and `store.get(k, d)` gives `d` for a
//! missing key or one that does not parse). A Rust build served from the
//! same origin reads and writes the same keys in the same encoding, so a
//! player keeps their settings and records across the two; [`stringify`]
//! and [`js_number`] write numbers as `JSON.stringify` does (`1` not `1.0`,
//! `1e+21`, `1e-7`), so what the Rust build saves reads back in the JS game
//! too. Natively the same key → string pairs live in one JSON file in the
//! user's config directory ([`FileStore`]), written whole on every change
//! through a temporary file and a rename.
//!
//! A game's typed settings struct sits on top: it reads the store at start
//! with its defaults and checks, and writes a key when a control changes.

use bevy::prelude::Resource;
use serde_json::Value;

/// Where the strings live: `localStorage`'s `getItem` and `setItem`.
pub trait Backend: Send + Sync + 'static {
    fn get_item(&self, key: &str) -> Option<String>;
    fn set_item(&mut self, key: &str, value: &str);
}

/// Where a game's store lives.
#[derive(Clone, Copy, Debug)]
pub struct StoreConfig {
    /// The key prefix (`mr.` in Midnight Playground).
    pub prefix: &'static str,
    /// The folder under the user's config directory natively.
    pub app_dir: &'static str,
    /// An environment variable naming the file instead (tests, CI).
    pub env_var: &'static str,
    /// Earlier names of `app_dir`: the first time `app_dir` is missing, the
    /// files of the first of these that exists are copied into it, so a
    /// rename keeps the player's settings and records.
    pub legacy_dirs: &'static [&'static str],
}

/// `store.get` and `store.set`, over a backend.
#[derive(Resource)]
pub struct Store {
    backend: Box<dyn Backend>,
    prefix: &'static str,
}

impl Store {
    pub fn new(backend: impl Backend, prefix: &'static str) -> Store {
        Store {
            backend: Box::new(backend),
            prefix,
        }
    }

    /// An empty store in memory (tests, and where nothing persists).
    pub fn memory(prefix: &'static str) -> Store {
        Store::new(Memory::default(), prefix)
    }

    /// The platform's store: `localStorage` on the web, a file natively
    /// (memory where no config directory can be found).
    pub fn platform(config: &StoreConfig) -> Store {
        #[cfg(target_arch = "wasm32")]
        {
            Store::new(LocalStorage, config.prefix)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match FileStore::default_path(config) {
                Some(p) => Store::new(FileStore::open(p), config.prefix),
                None => Store::memory(config.prefix),
            }
        }
    }

    /// The key prefix.
    pub fn prefix(&self) -> &'static str {
        self.prefix
    }

    /// The raw string under `<prefix><k>`.
    pub fn raw(&self, k: &str) -> Option<String> {
        self.backend.get_item(&format!("{}{k}", self.prefix))
    }

    /// Writes the raw string under `<prefix><k>`.
    pub fn set_raw(&mut self, k: &str, v: &str) {
        self.backend.set_item(&format!("{}{k}", self.prefix), v);
    }

    /// `store.get(k, d)` without the default: the parsed value, or `None`
    /// when the key is missing or does not parse (`JSON.parse` threw).
    pub fn get(&self, k: &str) -> Option<Value> {
        serde_json::from_str(&self.raw(k)?).ok()
    }

    /// `store.set(k, v)`: `JSON.stringify(v)` under `<prefix><k>`.
    pub fn set(&mut self, k: &str, v: &Value) {
        let s = stringify(v);
        self.set_raw(k, &s);
    }

    /// A number, else `d` (a missing key, `null` or another type).
    pub fn num(&self, k: &str, d: f64) -> f64 {
        self.get(k).and_then(|v| v.as_f64()).unwrap_or(d)
    }

    /// A number or `null` (`store.get(k, null)` for best times).
    pub fn num_or_null(&self, k: &str) -> Option<f64> {
        self.get(k).and_then(|v| v.as_f64())
    }

    pub fn bool(&self, k: &str, d: bool) -> bool {
        self.get(k).and_then(|v| v.as_bool()).unwrap_or(d)
    }

    pub fn string(&self, k: &str, d: &str) -> String {
        match self.get(k) {
            Some(Value::String(s)) => s,
            _ => d.to_owned(),
        }
    }

    pub fn set_num(&mut self, k: &str, v: f64) {
        self.set(k, &num_value(v));
    }

    pub fn set_bool(&mut self, k: &str, v: bool) {
        self.set(k, &Value::Bool(v));
    }

    pub fn set_str(&mut self, k: &str, v: &str) {
        self.set(k, &Value::String(v.to_owned()));
    }
}

/// A number as a JSON value; NaN and the infinities are `null`, as
/// `JSON.stringify` writes them.
pub fn num_value(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
}

/// `JSON.stringify(v)`: no spaces, numbers as JS prints them
/// (`Number.prototype.toString`: `1` not `1.0`, `1e+21`, `1e-7`), strings
/// escaped as JSON does.
pub fn stringify(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v);
    out
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => out.push_str(&js_number(f)),
            _ => out.push_str(&n.to_string()),
        },
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_default()),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, x);
            }
            out.push(']');
        }
        Value::Object(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap_or_default());
                out.push(':');
                write_value(out, x);
            }
            out.push('}');
        }
    }
}

/// `Number.prototype.toString()` for a finite number (ECMA-262
/// Number::toString): the shortest digits that round-trip, written plainly
/// for exponents from -7 to 20 and in exponent form outside them. NaN and
/// the infinities are `null` (as JSON writes them).
pub fn js_number(x: f64) -> String {
    if !x.is_finite() {
        return "null".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x < 0.0 {
        return format!("-{}", js_number(-x));
    }
    // Rust's `{:e}` is the shortest round-trip digits: "1.2345e3".
    let e = format!("{x:e}");
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    // x = 0.digits × 10^n
    let n = exp.parse::<i32>().unwrap_or(0) + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let sign = if n - 1 < 0 { '-' } else { '+' };
        let rest = if k > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits.clone()
        };
        format!("{rest}e{sign}{}", (n - 1).abs())
    }
}

/// A store in memory, in insertion order.
#[derive(Default, Clone, Debug)]
pub struct Memory {
    pub items: Vec<(String, String)>,
}

impl Backend for Memory {
    fn get_item(&self, key: &str) -> Option<String> {
        self.items
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    fn set_item(&mut self, key: &str, value: &str) {
        match self.items.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value.to_owned(),
            None => self.items.push((key.to_owned(), value.to_owned())),
        }
    }
}

/// `window.localStorage`. Every access is wrapped as the JS wraps it: a
/// browser in private mode, or with storage blocked, throws, and the game
/// carries on with its defaults.
#[cfg(target_arch = "wasm32")]
pub struct LocalStorage;

#[cfg(target_arch = "wasm32")]
impl LocalStorage {
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }
}

#[cfg(target_arch = "wasm32")]
impl Backend for LocalStorage {
    fn get_item(&self, key: &str) -> Option<String> {
        Self::storage()?.get_item(key).ok()?
    }

    fn set_item(&mut self, key: &str, value: &str) {
        if let Some(s) = Self::storage() {
            let _ = s.set_item(key, value);
        }
    }
}

/// Natively: the same key → string pairs as `localStorage` holds, as one
/// JSON object in a file, rewritten on every change (a few hundred bytes).
#[cfg(not(target_arch = "wasm32"))]
pub struct FileStore {
    path: std::path::PathBuf,
    mem: Memory,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileStore {
    /// `$<env_var>`, else `storage.json` in the user's config directory
    /// (`$XDG_CONFIG_HOME` or `~/.config` on Linux, `~/Library/Application
    /// Support` on macOS, `%APPDATA%` on Windows) under `<app_dir>/`. The
    /// first time that folder is missing, the files of the first legacy
    /// folder that exists (this store and the window's state) are copied
    /// into it, so a rename keeps the player's settings and records.
    pub fn default_path(config: &StoreConfig) -> Option<std::path::PathBuf> {
        use std::path::PathBuf;
        if let Some(p) = std::env::var_os(config.env_var) {
            return Some(PathBuf::from(p));
        }
        Some(Self::path_in(&config_dir()?, config))
    }

    /// `storage.json` under `<dir>/<app_dir>/`, with the legacy folders'
    /// files copied in the first time (see [`FileStore::default_path`]).
    pub fn path_in(dir: &std::path::Path, config: &StoreConfig) -> std::path::PathBuf {
        let new = dir.join(config.app_dir);
        if !new.exists()
            && let Some(old) = config
                .legacy_dirs
                .iter()
                .map(|d| dir.join(d))
                .find(|d| d.is_dir())
            && std::fs::create_dir_all(&new).is_ok()
            && let Ok(entries) = std::fs::read_dir(&old)
        {
            for e in entries.flatten() {
                if e.path().is_file() {
                    let _ = std::fs::copy(e.path(), new.join(e.file_name()));
                }
            }
        }
        new.join("storage.json")
    }

    /// Reads the file if there is one (a missing or broken file is an empty
    /// store).
    pub fn open(path: std::path::PathBuf) -> FileStore {
        let mut mem = Memory::default();
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&text)
        {
            for (k, v) in m {
                if let Value::String(s) = v {
                    mem.items.push((k, s));
                }
            }
        }
        FileStore { path, mem }
    }

    /// The file.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn save(&self) -> std::io::Result<()> {
        // In insertion order, as `localStorage` keeps them.
        let mut text = String::from("{");
        for (i, (k, v)) in self.mem.items.iter().enumerate() {
            if i > 0 {
                text.push(',');
            }
            text.push_str(&stringify(&Value::String(k.clone())));
            text.push(':');
            text.push_str(&stringify(&Value::String(v.clone())));
        }
        text.push('}');
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }
}

/// The user's config directory: `$XDG_CONFIG_HOME` or `~/.config` on Linux,
/// `~/Library/Application Support` on macOS, `%APPDATA%` on Windows.
#[cfg(not(target_arch = "wasm32"))]
pub fn config_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    Some(if cfg!(windows) {
        env("APPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
    })
}

#[cfg(not(target_arch = "wasm32"))]
impl Backend for FileStore {
    fn get_item(&self, key: &str) -> Option<String> {
        self.mem.get_item(key)
    }

    fn set_item(&mut self, key: &str, value: &str) {
        self.mem.set_item(key, value);
        if let Err(e) = self.save() {
            bevy::log::warn!("settings: {}: {e}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const P: &str = "t.";

    fn seeded(pairs: &[(&str, &str)]) -> Store {
        let mut m = Memory::default();
        for (k, v) in pairs {
            m.set_item(k, v);
        }
        Store::new(m, P)
    }

    #[test]
    fn numbers_as_js_prints_them() {
        for (x, s) in [
            (0.7, "0.7"),
            (0.85, "0.85"),
            (1.0, "1"),
            (0.0, "0"),
            (-0.0, "0"),
            (12345.0, "12345"),
            (83.456_789_012_345_67, "83.45678901234567"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (1e20, "100000000000000000000"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (0.000001, "0.000001"),
            (-2.5, "-2.5"),
            (123.0e-2, "1.23"),
            (f64::NAN, "null"),
            (f64::INFINITY, "null"),
        ] {
            assert_eq!(js_number(x), s, "{x:e}");
        }
    }

    #[test]
    fn values_as_json_stringify_writes_them() {
        assert_eq!(stringify(&json!("sports")), "\"sports\"");
        assert_eq!(stringify(&json!(true)), "true");
        assert_eq!(stringify(&num_value(0.35)), "0.35");
        assert_eq!(stringify(&num_value(f64::NAN)), "null");
        assert_eq!(stringify(&json!("a\"b\\c\n\u{1}")), r#""a\"b\\c\n\u0001""#);
        assert_eq!(
            stringify(
                &json!({"Pad (STANDARD GAMEPAD)": {"left": [{"axis": 0, "dir": -1, "rest": 0}], "throttle": [{"button": 7}]}})
            ),
            r#"{"Pad (STANDARD GAMEPAD)":{"left":[{"axis":0,"dir":-1,"rest":0}],"throttle":[{"button":7}]}}"#
        );
    }

    /// What `store.set` writes is what the JS writes, under the prefix and
    /// JSON-encoded, and what the JS wrote reads back.
    #[test]
    fn writes_are_the_js_strings() {
        let mut store = Store::memory(P);
        store.set_str("hero", "blue");
        store.set_bool("mph", false);
        store.set_num("musicVol", 0.42);
        store.set_num("best.a", 183.456_666_666_666_7);
        store.set_num("score", 12345.0);
        assert_eq!(store.raw("hero").as_deref(), Some("\"blue\""));
        assert_eq!(store.raw("mph").as_deref(), Some("false"));
        assert_eq!(store.raw("musicVol").as_deref(), Some("0.42"));
        assert_eq!(store.raw("best.a").as_deref(), Some("183.4566666666667"));
        assert_eq!(store.raw("score").as_deref(), Some("12345"));
        assert_eq!(store.num_or_null("best.a"), Some(183.456_666_666_666_7));
        assert_eq!(store.num_or_null("best.b"), None);
        assert_eq!(store.prefix(), P);
    }

    /// `store.get(k, d)`'s default for a value of the wrong type, as the
    /// JS's typed reads do: a string is no number, a number no string,
    /// `null` nothing; a value that does not parse is missing.
    #[test]
    fn a_value_of_the_wrong_type_reads_as_the_default() {
        let store = seeded(&[
            ("t.a", "\"0.5\""),
            ("t.b", "0.5"),
            ("t.c", "null"),
            ("t.d", "1"),
            ("t.e", ""),
            ("t.f", "not json"),
        ]);
        assert_eq!(store.num("a", 2.0), 2.0);
        assert_eq!(store.num_or_null("a"), None);
        assert_eq!(store.string("b", "x"), "x");
        assert_eq!(store.num("c", 3.0), 3.0);
        assert_eq!(store.num_or_null("c"), None);
        assert!(store.bool("d", true), "1 is not true");
        assert!(!store.bool("d", false));
        assert_eq!(store.get("e"), None, "an empty string does not parse");
        assert_eq!(store.num("f", 0.7), 0.7);
        assert_eq!(store.num("missing", 4.0), 4.0);
        // A NaN saved is `null`, and reads back as missing.
        let mut store = Store::memory(P);
        store.set_num("best", f64::NAN);
        assert_eq!(store.raw("best").as_deref(), Some("null"));
        assert_eq!(store.num_or_null("best"), None);
    }

    /// Setting a key again replaces its value where it is: the order is
    /// the first insertion's, as `localStorage` keeps it.
    #[test]
    fn rewriting_a_key_keeps_its_place() {
        let mut m = Memory::default();
        m.set_item("a", "1");
        m.set_item("b", "2");
        m.set_item("a", "3");
        assert_eq!(
            m.items,
            [("a".to_string(), "3".to_string()), ("b".into(), "2".into())]
        );
        assert_eq!(m.get_item("a").as_deref(), Some("3"));
        assert_eq!(m.get_item("c"), None);
    }

    /// The native file holds the same key → string pairs, and a new store
    /// over it reads them back.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_file_store_round_trips() {
        let dir = std::env::temp_dir().join(format!("pk-store-test-{}", std::process::id()));
        let path = dir.join("storage.json");
        let _ = std::fs::remove_dir_all(&dir);
        {
            let mut s = Store::new(FileStore::open(path.clone()), P);
            s.set_str("hero", "green");
            s.set_num("tiltSens", 0.3);
            s.set_bool("hq", false);
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            r#"{"t.hero":"\"green\"","t.tiltSens":"0.3","t.hq":"false"}"#
        );
        let s = Store::new(FileStore::open(path.clone()), P);
        assert_eq!(s.raw("hero").as_deref(), Some("\"green\""));
        assert_eq!(s.raw("tiltSens").as_deref(), Some("0.3"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A broken file, a file that isn't an object, or values that aren't
    /// strings: whatever can't be read is left out, and the store works.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_file_store_survives_a_broken_file() {
        let dir = std::env::temp_dir().join(format!("pk-store-broken-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("storage.json");
        for text in ["{not json", "[1,2]", "", "null"] {
            std::fs::write(&path, text).unwrap();
            let s = Store::new(FileStore::open(path.clone()), P);
            assert_eq!(s.raw("hero"), None, "{text:?}");
        }
        std::fs::write(&path, r#"{"t.hero":"\"red\"","t.hq":false,"t.x":1}"#).unwrap();
        let mut s = Store::new(FileStore::open(path.clone()), P);
        assert_eq!(s.raw("hero").as_deref(), Some("\"red\""));
        assert_eq!(s.raw("hq"), None, "not a string: left out");
        s.set_str("hero", "gold");
        s.set_bool("mph", false);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"t.hero":"\"gold\"","t.mph":"false"}"#
        );
        // A store whose folder isn't there yet makes it.
        let deep = dir.join("a/b/storage.json");
        Store::new(FileStore::open(deep.clone()), P).set_num("sfxVol", 0.5);
        assert!(deep.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A missing app folder is filled from the first legacy folder that
    /// exists.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn legacy_folders_migrate() {
        let dir = std::env::temp_dir().join(format!("pk-store-legacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("old-name")).unwrap();
        std::fs::write(dir.join("old-name/storage.json"), "{}").unwrap();
        std::fs::write(dir.join("old-name/window-state.json"), "{}").unwrap();
        let cfg = StoreConfig {
            prefix: P,
            app_dir: "new-name",
            env_var: "PK_STORE_TEST_UNSET_VAR",
            legacy_dirs: &["older-name", "old-name"],
        };
        let p = FileStore::path_in(&dir, &cfg);
        assert_eq!(p, dir.join("new-name/storage.json"));
        assert!(dir.join("new-name/window-state.json").exists());
        // Once the folder exists, nothing more is copied.
        std::fs::write(dir.join("old-name/extra.json"), "{}").unwrap();
        FileStore::path_in(&dir, &cfg);
        assert!(!dir.join("new-name/extra.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
