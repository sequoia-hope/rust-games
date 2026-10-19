//! Tricks: what a jump was (spin, flips, grab, air time), its name in the
//! words riders use, and its points.

use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grab {
    Indy,
    Melon,
    Nose,
    Tail,
    Stalefish,
    Method,
    Mute,
    Japan,
}

impl Grab {
    /// The grab for where the stick points as the button goes down.
    pub fn from_stick(x: f32, y: f32) -> Grab {
        let (l, r, up, down) = (x < -0.4, x > 0.4, y > 0.4, y < -0.4);
        match (l, r, up, down) {
            (true, _, true, _) => Grab::Japan,
            (_, true, true, _) => Grab::Mute,
            (true, _, _, true) => Grab::Method,
            (_, true, _, true) => Grab::Stalefish,
            (true, _, _, _) => Grab::Melon,
            (_, true, _, _) => Grab::Mute,
            (_, _, true, _) => Grab::Nose,
            (_, _, _, true) => Grab::Tail,
            _ => Grab::Indy,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Grab::Indy => "Indy",
            Grab::Melon => "Melon",
            Grab::Nose => "Nose Grab",
            Grab::Tail => "Tail Grab",
            Grab::Stalefish => "Stalefish",
            Grab::Method => "Method",
            Grab::Mute => "Mute",
            Grab::Japan => "Japan",
        }
    }

    /// Points for the grab itself, before it is held.
    fn base(self) -> f32 {
        match self {
            Grab::Indy | Grab::Mute => 120.0,
            Grab::Melon | Grab::Nose | Grab::Tail => 140.0,
            Grab::Stalefish | Grab::Japan => 170.0,
            Grab::Method => 200.0,
        }
    }
}

/// How a landing went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landing {
    Clean,
    Sloppy,
    Crash,
}

/// What happened in the air, as the rider kept it.
#[derive(Clone, Debug, Default)]
pub struct AirLog {
    pub time: f32,
    /// The spin the rider made (radians, signed), not counting the pipe's
    /// automatic turn.
    pub spin: f32,
    /// The flip (radians, signed; positive forward).
    pub flip: f32,
    /// The grab held longest, and for how long.
    pub grab: Option<(Grab, f32)>,
    pub switch_takeoff: bool,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trick {
    pub name: String,
    pub points: u32,
    pub spin_deg: u32,
    pub flips: u32,
}

/// Names and scores a landed jump; `None` for a hop not worth a word.
pub fn evaluate(air: &AirLog, landing: Landing) -> Option<Trick> {
    if landing == Landing::Crash {
        return None;
    }
    let halves = (air.spin.abs() / PI + 0.2).floor() as u32;
    let spin_deg = halves * 180;
    let flips = (air.flip.abs() / TAU + 0.25).floor() as u32;
    let mut points = 0.0f32;
    if air.time > 0.5 {
        points += 45.0 * air.time;
    }
    if halves > 0 {
        points += 140.0 * (halves as f32).powf(1.35);
    }
    points += 520.0 * flips as f32 * (1.0 + 0.4 * flips.saturating_sub(1) as f32);
    if let Some((g, t)) = air.grab {
        points += g.base() + 260.0 * t.min(1.6);
    }
    if halves >= 2 && air.grab.is_some() {
        points *= 1.25;
    }
    if flips > 0 && halves >= 2 {
        points *= 1.3;
    }
    if air.switch_takeoff && (halves > 0 || flips > 0) {
        points *= 1.15;
    }
    if landing == Landing::Sloppy {
        points *= 0.5;
    }
    if points < 60.0 {
        return None;
    }

    let mut words: Vec<String> = Vec::new();
    if air.switch_takeoff && (halves > 0 || flips > 0) {
        words.push("Switch".into());
    }
    let forward = air.flip > 0.0;
    if flips > 0 && halves >= 2 {
        let prefix = match flips {
            1 => "",
            2 => "Double ",
            _ => "Triple ",
        };
        words.push(format!(
            "{prefix}{}",
            if forward { "Misty" } else { "Rodeo" }
        ));
        words.push(spin_deg.to_string());
    } else {
        if halves > 0 {
            words.push(
                if air.spin > 0.0 {
                    "Frontside"
                } else {
                    "Backside"
                }
                .into(),
            );
            words.push(spin_deg.to_string());
        }
        if flips > 0 {
            let prefix = match flips {
                1 => "",
                2 => "Double ",
                _ => "Triple ",
            };
            words.push(format!(
                "{prefix}{}",
                if forward { "Front Flip" } else { "Back Flip" }
            ));
        }
    }
    match air.grab {
        Some((g, _)) if words.is_empty() => {
            let n = g.name();
            words.push(if n.ends_with("Grab") {
                n.into()
            } else {
                format!("{n} Grab")
            });
        }
        Some((g, _)) => words.push(g.name().replace(" Grab", "")),
        None if words.is_empty() => {
            words.push(if air.time > 1.6 { "Big Air" } else { "Air" }.into())
        }
        None => {}
    }
    Some(Trick {
        name: words.join(" "),
        points: (points / 10.0).round() as u32 * 10,
        spin_deg,
        flips,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air(spin_deg: f32, flip_deg: f32, grab: Option<(Grab, f32)>) -> AirLog {
        AirLog {
            time: 1.5,
            spin: spin_deg.to_radians(),
            flip: flip_deg.to_radians(),
            grab,
            switch_takeoff: false,
            height: 4.0,
        }
    }

    #[test]
    fn names_tricks() {
        let t = evaluate(&air(-700.0, 0.0, Some((Grab::Melon, 0.8))), Landing::Clean).unwrap();
        assert_eq!(t.name, "Backside 720 Melon");
        let t = evaluate(&air(0.0, -350.0, None), Landing::Clean).unwrap();
        assert_eq!(t.name, "Back Flip");
        let t = evaluate(&air(540.0, -360.0, Some((Grab::Indy, 0.5))), Landing::Clean).unwrap();
        assert_eq!(t.name, "Rodeo 540 Indy");
        let t = evaluate(&air(0.0, 0.0, Some((Grab::Method, 1.0))), Landing::Clean).unwrap();
        assert_eq!(t.name, "Method Grab");
        let t = evaluate(&air(1060.0, 0.0, None), Landing::Clean).unwrap();
        assert_eq!(t.name, "Frontside 1080");
        assert!(evaluate(&air(1080.0, 0.0, None), Landing::Crash).is_none());
    }

    #[test]
    fn bigger_is_worth_more() {
        let p = |deg| {
            evaluate(&air(deg, 0.0, None), Landing::Clean)
                .unwrap()
                .points
        };
        assert!(p(360.0) < p(720.0));
        assert!(p(720.0) < p(1080.0));
        let clean = evaluate(&air(360.0, 0.0, None), Landing::Clean)
            .unwrap()
            .points;
        let sloppy = evaluate(&air(360.0, 0.0, None), Landing::Sloppy)
            .unwrap()
            .points;
        assert!(sloppy < clean);
    }
}
