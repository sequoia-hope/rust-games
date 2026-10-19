//! The options: `?course=timber&mode=race&rider=1&go=1&auto=1` on the web,
//! `--course timber --mode race --rider 1 --go --auto` natively.
//!
//! - `course`: crystal, timber, avalanche or pipe.
//! - `mode`: race, time or trick.
//! - `rider`: 0 to 3.
//! - `go`: straight into the run, past the menu.
//! - `auto`: the player's rider rides itself (a demo, and the tests).
//! - `touch`: 1 forces the touch controls on, 0 off.
//! - `assist`: the landing assist on (1) or off (0); unset, it comes on
//!   when the screen is touched.
//! - `radio`: all, music, voices or off (unset: as last chosen).
//! - natively, `--screenshot <file> --frames <n>` saves frame n and exits,
//!   and `--size WxH` sets the window.

use sb_sim::course::CourseId;
use sb_sim::race::{CHARACTERS, Mode};

#[derive(Clone, Debug)]
pub struct Options {
    pub course: CourseId,
    pub mode: Mode,
    pub character: usize,
    pub go: bool,
    pub auto: bool,
    pub touch: Option<bool>,
    /// The landing assist; unset, it comes on with touch.
    pub assist: Option<bool>,
    pub radio: Option<crate::radio::Radio>,
    pub width: u32,
    pub height: u32,
    pub screenshot: Option<(String, u32)>,
    /// Seconds to fast-forward the run at its start (tests and pictures).
    pub skip: f32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            course: CourseId::Crystal,
            mode: Mode::Race,
            character: 0,
            go: false,
            auto: false,
            touch: None,
            assist: None,
            radio: None,
            width: 1280,
            height: 720,
            screenshot: None,
            skip: 0.0,
        }
    }
}

fn truthy(v: &str) -> bool {
    !matches!(v, "0" | "false" | "no" | "off")
}

impl Options {
    pub fn set(&mut self, k: &str, v: &str) {
        match k {
            "course" => {
                if let Some(c) = CourseId::from_key(v) {
                    self.course = c;
                }
            }
            "mode" => {
                if let Some(m) = Mode::from_key(v) {
                    self.mode = m;
                }
            }
            // Any number picks a rider: the menu indexes the riders by it.
            "rider" => self.character = v.parse::<usize>().unwrap_or(0) % CHARACTERS.len(),
            "go" => self.go = truthy(v),
            "auto" => self.auto = truthy(v),
            "touch" => self.touch = Some(truthy(v)),
            "assist" => self.assist = Some(truthy(v)),
            "skip" => self.skip = v.parse().unwrap_or(0.0),
            "radio" => self.radio = crate::radio::Radio::from_key(v),
            "size" => {
                if let Some((w, h)) = v.split_once('x') {
                    self.width = w.parse().unwrap_or(self.width);
                    self.height = h.parse().unwrap_or(self.height);
                }
            }
            _ => {}
        }
    }

    /// From a query string (`?a=1&b`).
    pub fn from_query(q: &str) -> Options {
        let mut o = Options::default();
        for pair in q
            .trim_start_matches('?')
            .split('&')
            .filter(|p| !p.is_empty())
        {
            let (k, v) = pair.split_once('=').unwrap_or((pair, "1"));
            o.set(k, v);
        }
        o
    }

    /// From command-line arguments.
    pub fn from_args(args: impl Iterator<Item = String>) -> Options {
        let mut o = Options::default();
        let args: Vec<String> = args.collect();
        let mut i = 0;
        let mut frames = 240;
        let mut shot = None;
        while i < args.len() {
            let k = args[i].trim_start_matches('-');
            let next = args.get(i + 1).filter(|n| !n.starts_with("--")).cloned();
            match (k, next) {
                ("screenshot", Some(v)) => {
                    shot = Some(v);
                    i += 1;
                }
                ("frames", Some(v)) => {
                    frames = v.parse().unwrap_or(frames);
                    i += 1;
                }
                (k, Some(v)) if !matches!(k, "go" | "auto" | "assist") => {
                    o.set(k, &v);
                    i += 1;
                }
                (k, _) => o.set(k, "1"),
            }
            i += 1;
        }
        o.screenshot = shot.map(|s| (s, frames));
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_queries_and_args() {
        let o = Options::from_query("?course=timber&mode=trick&rider=2&go&auto=0");
        assert_eq!(o.course, CourseId::Timber);
        assert_eq!(o.mode, Mode::TrickAttack);
        assert_eq!(o.character, 2);
        assert!(o.go && !o.auto);
        let o = Options::from_args(
            [
                "--course",
                "pipe",
                "--go",
                "--screenshot",
                "a.png",
                "--frames",
                "9",
            ]
            .into_iter()
            .map(String::from),
        );
        assert_eq!(o.course, CourseId::Pipe);
        assert!(o.go);
        assert_eq!(o.screenshot, Some(("a.png".into(), 9)));
    }
}
