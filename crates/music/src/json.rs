//! Canonical JSON, as `tools/music-lab/test/golden.mjs`'s `canon` writes
//! it: object keys sorted, `undefined` members dropped, numbers as JS
//! prints them (`String(n)`: integers without a fraction, otherwise the
//! shortest round-trip decimal; `-0` is `0`). A track or an event list
//! becomes a [`Val`] tree (`to_val`) and then this text, so its SHA-256 can
//! be compared with the lab's.

/// A JSON value with its object keys in insertion order.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Val>),
    Obj(Vec<(String, Val)>),
}

impl Val {
    /// An object from `(key, value)` pairs; a `None` value is a missing
    /// key, as `JSON.stringify` drops an `undefined` member.
    pub fn obj(pairs: Vec<(&str, Option<Val>)>) -> Val {
        Val::Obj(
            pairs
                .into_iter()
                .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v)))
                .collect(),
        )
    }
    pub fn num(x: f64) -> Option<Val> {
        Some(Val::Num(x))
    }
    pub fn onum(x: Option<f64>) -> Option<Val> {
        x.map(Val::Num)
    }
    pub fn str(s: &str) -> Option<Val> {
        Some(Val::Str(s.to_owned()))
    }
    pub fn ostr(s: &Option<String>) -> Option<Val> {
        s.as_ref().map(|s| Val::Str(s.clone()))
    }
    pub fn obool(b: Option<bool>) -> Option<Val> {
        b.map(Val::Bool)
    }
    /// An object of string pairs (a pattern table, a progression table).
    pub fn strs(pairs: &[(String, String)]) -> Val {
        Val::Obj(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), Val::Str(v.clone())))
                .collect(),
        )
    }
    /// An object of number pairs (`lay`).
    pub fn nums(pairs: &[(String, f64)]) -> Val {
        Val::Obj(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), Val::Num(*v)))
                .collect(),
        )
    }
    pub fn pair(p: &[f64; 2]) -> Val {
        Val::Arr(vec![Val::Num(p[0]), Val::Num(p[1])])
    }
}

/// `String(n)` for a finite double.
pub fn js_num(x: f64) -> String {
    if x == 0.0 {
        return "0".to_owned();
    }
    if x.fract() == 0.0 && x.abs() < 1e21 {
        return format!("{}", x as i64);
    }
    let a = x.abs();
    if a >= 1e21 || a < 1e-6 {
        // JS switches to exponent form here; Rust's `{:e}` prints the
        // shortest round-trip mantissa too, with the exponent as `e-7`.
        let s = format!("{:e}", x);
        let (m, e) = s.split_once('e').expect("an exponent");
        let e: i32 = e.parse().expect("an exponent");
        return format!("{m}e{}{}", if e < 0 { "-" } else { "+" }, e.abs());
    }
    // Rust's Display is the shortest decimal that round-trips, no exponent.
    format!("{}", x)
}

fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The canonical text.
pub fn canon(v: &Val) -> String {
    let mut out = String::new();
    write(v, &mut out);
    out
}

fn write(v: &Val, out: &mut String) {
    match v {
        Val::Null => out.push_str("null"),
        Val::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Val::Num(x) => {
            if x.is_finite() {
                out.push_str(&js_num(*x));
            } else {
                out.push_str("null");
            }
        }
        Val::Str(s) => escape(s, out),
        Val::Arr(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(x, out);
            }
            out.push(']');
        }
        Val::Obj(o) => {
            let mut keys: Vec<&(String, Val)> = o.iter().collect();
            // JS `sort()` compares UTF-16 code units; the keys are ASCII.
            keys.sort_by(|a, b| a.0.cmp(&b.0));
            out.push('{');
            for (i, (k, x)) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                escape(k, out);
                out.push(':');
                write(x, out);
            }
            out.push('}');
        }
    }
}

/// Parses JSON text (as written by [`canon`] or by hand: the station
/// tables games pass in). Numbers are `f64`; object keys keep their order.
pub fn parse(text: &str) -> Result<Val, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("trailing text"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> String {
        format!("JSON: {what} at byte {}", self.i)
    }
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn eat(&mut self, b: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&b) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn lit(&mut self, word: &str, v: Val) -> Result<Val, String> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err("unexpected word"))
        }
    }
    fn value(&mut self) -> Result<Val, String> {
        self.ws();
        match self.s.get(self.i) {
            None => Err(self.err("unexpected end")),
            Some(b'{') => {
                self.i += 1;
                let mut pairs = Vec::new();
                if self.eat(b'}') {
                    return Ok(Val::Obj(pairs));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    if !self.eat(b':') {
                        return Err(self.err("expected ':'"));
                    }
                    pairs.push((k, self.value()?));
                    if self.eat(b'}') {
                        return Ok(Val::Obj(pairs));
                    }
                    if !self.eat(b',') {
                        return Err(self.err("expected ',' or '}'"));
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                if self.eat(b']') {
                    return Ok(Val::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    if self.eat(b']') {
                        return Ok(Val::Arr(items));
                    }
                    if !self.eat(b',') {
                        return Err(self.err("expected ',' or ']'"));
                    }
                }
            }
            Some(b'"') => Ok(Val::Str(self.string()?)),
            Some(b't') => self.lit("true", Val::Bool(true)),
            Some(b'f') => self.lit("false", Val::Bool(false)),
            Some(b'n') => self.lit("null", Val::Null),
            Some(_) => {
                let start = self.i;
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[start..self.i])
                    .ok()
                    .and_then(|t| t.parse::<f64>().ok())
                    .map(Val::Num)
                    .ok_or_else(|| self.err("bad number"))
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        if self.s.get(self.i) != Some(&b'"') {
            return Err(self.err("expected a string"));
        }
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let Some(&b) = self.s.get(self.i) else {
                return Err(self.err("unterminated string"));
            };
            self.i += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.s.get(self.i) else {
                        return Err(self.err("unterminated string"));
                    };
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(e),
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'u' => {
                            let mut c = self.hex4()?;
                            if (0xD800..0xDC00).contains(&c) && self.s[self.i..].starts_with(b"\\u")
                            {
                                self.i += 2;
                                let lo = self.hex4()?;
                                c = 0x10000
                                    + ((c - 0xD800) << 10)
                                    + (lo.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            let ch = char::from_u32(c).ok_or_else(|| self.err("bad \\u escape"))?;
                            out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                        }
                        _ => return Err(self.err("bad escape")),
                    }
                }
                _ => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("bad UTF-8"))
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let h = self
            .s
            .get(self.i..self.i + 4)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .ok_or_else(|| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(h)
    }
}

impl Val {
    /// An object's member.
    pub fn get(&self, key: &str) -> Option<&Val> {
        match self {
            Val::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Val::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Val::Num(x) => Some(*x),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Val]> {
        match self {
            Val::Arr(a) => Some(a),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_as_js_does() {
        assert_eq!(js_num(1.0), "1");
        assert_eq!(js_num(-0.0), "0");
        assert_eq!(js_num(0.75), "0.75");
        assert_eq!(js_num(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(js_num(124.0), "124");
        assert_eq!(js_num(-3.5), "-3.5");
        assert_eq!(js_num(1e-7), "1e-7");
        assert_eq!(js_num(1e21), "1e+21");
        assert_eq!(js_num(0.0001), "0.0001");
    }

    #[test]
    fn objects_sort_and_drop_missing() {
        let v = Val::obj(vec![
            ("b", Val::num(2.0)),
            ("a", None),
            ("c", Val::str("x\"y")),
            ("aa", Some(Val::Arr(vec![Val::Null, Val::Bool(true)]))),
        ]);
        assert_eq!(canon(&v), r#"{"aa":[null,true],"b":2,"c":"x\"y"}"#);
    }

    #[test]
    fn parses_what_canon_writes() {
        let v = Val::obj(vec![
            ("a", Val::num(1.5)),
            (
                "b",
                Some(Val::Arr(vec![
                    Val::Str("x\"é\n".into()),
                    Val::Null,
                    Val::Bool(true),
                ])),
            ),
            ("c", Some(Val::Obj(vec![]))),
            ("d", Val::num(-2e-7)),
        ]);
        assert_eq!(parse(&canon(&v)).unwrap(), v);
        assert_eq!(
            parse(" [ 1 , {\"k\" : \"\\u00e9\\ud83c\\udfb5\"} ] ").unwrap(),
            Val::Arr(vec![
                Val::Num(1.0),
                Val::Obj(vec![("k".into(), Val::Str("é🎵".into()))])
            ])
        );
        assert!(parse("[1,]").is_err());
        assert!(parse("{\"a\":1} x").is_err());
    }
}
