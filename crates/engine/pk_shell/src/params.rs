//! What the client was asked to do: the page's query string on the web, and
//! the same names as a `--query` string, bare `key=value` pairs or flags
//! natively (Midnight Playground SPEC 8.4, its `options.rs`).
//!
//! [`Params`] is the ordered list of decoded pairs, with `URLSearchParams`'s
//! decoding rules and JavaScript's `Number()` for the numeric ones, so a
//! link's parameters read the same in a browser and on a command line. A
//! game builds its own typed options from it, and keeps asking it for the
//! parameters its modules read themselves (`params.get("seed")`).

/// The command-line spelling of parameters: `--debug` for `debug=1`
/// (a flag), `--stage x` for `stage=x` (a valued flag), and valued flags
/// whose value is percent-encoded because it holds `?`, `&` or `#` (an
/// invitation link). `--query "a=1&b=2"` and bare `key=value` arguments are
/// always accepted.
#[derive(Clone, Copy, Debug, Default)]
pub struct Aliases {
    /// `(flag, key)`: the flag alone means `key=1`.
    pub flags: &'static [(&'static str, &'static str)],
    /// `(flag, key)`: the flag takes the next argument as `key`'s value.
    pub valued: &'static [(&'static str, &'static str)],
    /// As `valued`, the value percent-encoded first.
    pub escaped: &'static [(&'static str, &'static str)],
}

/// Query parameters, decoded, in the order given (a name may repeat; the
/// first wins on lookup, as `URLSearchParams.get` does).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params(Vec<(String, String)>);

impl Params {
    /// Splits `a=1&b=2` (a leading `?` is ignored) into decoded pairs.
    pub fn from_query(q: &str) -> Params {
        Params(parse_query(q))
    }

    /// From pairs already decoded.
    pub fn from_pairs(pairs: Vec<(String, String)>) -> Params {
        Params(pairs)
    }

    /// Native command line: `--query "stage=x&s=300"`, bare `key=value`
    /// pairs, and the game's aliases. Anything else is an error naming
    /// the argument (the game appends its usage).
    pub fn from_args(args: &[String], aliases: &Aliases) -> Result<Params, String> {
        let mut query: Vec<String> = Vec::new();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            let mut val = |name: &str| {
                it.next()
                    .cloned()
                    .ok_or_else(|| format!("{name} needs a value"))
            };
            let s = a.as_str();
            if s == "--query" {
                query.push(val("--query")?);
            } else if let Some((_, k)) = aliases.flags.iter().find(|(f, _)| *f == s) {
                query.push(format!("{k}=1"));
            } else if let Some((_, k)) = aliases.valued.iter().find(|(f, _)| *f == s) {
                query.push(format!("{k}={}", val(s)?));
            } else if let Some((_, k)) = aliases.escaped.iter().find(|(f, _)| *f == s) {
                query.push(format!("{k}={}", escape(&val(s)?)));
            } else if s.contains('=') && !s.starts_with('-') {
                query.push(s.to_owned());
            } else {
                return Err(format!("unknown argument `{s}`"));
            }
        }
        Ok(Params::from_query(&query.join("&")))
    }

    /// The first value under `k`.
    pub fn get(&self, k: &str) -> Option<&str> {
        self.0.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }

    /// Whether `k` is given.
    pub fn has(&self, k: &str) -> bool {
        self.get(k).is_some()
    }

    /// `k=1` or `k=true`: on; anything else, or absent, off.
    pub fn flag(&self, k: &str) -> bool {
        self.get(k).is_some_and(|v| v == "1" || v == "true")
    }

    /// `k=0|1|true|false` as given, `None` when absent.
    pub fn switch(&self, k: &str) -> Option<bool> {
        self.get(k).map(|v| v == "1" || v == "true")
    }

    /// `Number(v)` of the first value: NaN for one that does not parse.
    pub fn num(&self, k: &str) -> Option<f64> {
        self.get(k).map(number)
    }

    /// The value parsed as `T`, `None` when absent or not a `T`.
    pub fn parse<T: std::str::FromStr>(&self, k: &str) -> Option<T> {
        self.get(k).and_then(|v| v.parse().ok())
    }

    /// Every pair, in order.
    pub fn pairs(&self) -> &[(String, String)] {
        &self.0
    }

    /// Adds a pair at the end.
    pub fn push(&mut self, k: impl Into<String>, v: impl Into<String>) {
        self.0.push((k.into(), v.into()));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Splits `a=1&b=2` (a leading `?` is ignored) into decoded pairs, in order.
pub fn parse_query(q: &str) -> Vec<(String, String)> {
    q.trim_start_matches('?')
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

/// `%XX` and `+` decoding (`URLSearchParams`' rules).
pub fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = |c: u8| (c as char).to_digit(16);
                match (hex(b[i + 1]), hex(b[i + 2])) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 2;
                    }
                    _ => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encodes everything but RFC 3986's unreserved characters
/// (`encodeURIComponent`, stricter: `!'()*` are encoded too).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// JS `Number(x)` for the simple forms debug parameters use: NaN when it
/// does not parse (as `Number('abc')`), 0 for an empty string.
pub fn number(v: &str) -> f64 {
    let t = v.trim();
    if t.is_empty() {
        return 0.0;
    }
    t.parse().unwrap_or(f64::NAN)
}

/// `0`, `false`, `no` and `off` are false; anything else is true.
pub fn truthy(v: &str) -> bool {
    !matches!(v, "0" | "false" | "no" | "off")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALIASES: Aliases = Aliases {
        flags: &[("--debug", "debug"), ("--smoke-test", "smoke-test")],
        valued: &[("--stage", "stage"), ("--after", "after")],
        escaped: &[("--join", "join")],
    };

    #[test]
    fn queries_decode_in_order() {
        let p = Params::from_query("?stage=coast&s=300&scene=a%20b+c&x&s=1");
        assert_eq!(p.get("stage"), Some("coast"));
        assert_eq!(p.num("s"), Some(300.0));
        assert_eq!(p.get("scene"), Some("a b c"));
        assert_eq!(p.get("x"), Some(""));
        assert_eq!(p.len(), 5);
        assert!(!p.flag("x"));
        assert!(Params::from_query("hq=true").flag("hq"));
        assert_eq!(Params::from_query("hq=0").switch("hq"), Some(false));
        assert_eq!(Params::from_query("").switch("hq"), None);
        assert!(Params::from_query("n=abc").num("n").unwrap().is_nan());
        assert_eq!(Params::from_query("n=").num("n"), Some(0.0));
        assert_eq!(Params::from_query("n=7").parse::<u32>("n"), Some(7));
    }

    #[test]
    fn args_aliases_and_escaping() {
        let args: Vec<String> = [
            "--stage",
            "seaside",
            "--query",
            "s=10&v=30",
            "--after",
            "3",
            "--debug",
            "hero=gold",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let p = Params::from_args(&args, &ALIASES).unwrap();
        assert_eq!(p.get("stage"), Some("seaside"));
        assert_eq!(p.num("v"), Some(30.0));
        assert_eq!(p.parse::<u32>("after"), Some(3));
        assert!(p.flag("debug"));
        assert_eq!(p.get("hero"), Some("gold"));
        // An invitation keeps its `?`, `&` and `#`.
        let link = "https://x/?signal=wss%3A%2F%2Fs%2F&a=1#join=AAAA.BBBB";
        let p = Params::from_args(&["--join".into(), link.into()], &ALIASES).unwrap();
        assert_eq!(p.get("join"), Some(link));
        assert!(Params::from_args(&["--bogus".to_string()], &ALIASES).is_err());
        assert!(Params::from_args(&["--stage".to_string()], &ALIASES).is_err());
    }

    #[test]
    fn escape_is_rfc_3986() {
        assert_eq!(escape("a b&c=d/e~"), "a%20b%26c%3Dd%2Fe~");
        assert_eq!(decode(&escape("ü?#")), "ü?#");
        assert!(truthy("1") && truthy("yes") && !truthy("off") && !truthy("0"));
    }
}
