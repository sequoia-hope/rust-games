//! The game's radio stations (docs/vision/radio.md 7), in dial order: the
//! index in [`STATIONS`] is the radio node's `station` param. The table is
//! `stations.json` beside this crate's `Cargo.toml`, the one copy the game,
//! its worklet (handed the bytes at start-up) and the Radio page
//! (`tools/radio.html`) all read; the music engine plays whatever table it
//! is given.
//!
//! - **The Tide** (Marisol): house (the classic grammar's, and the new
//!   generator's deep and Chicago house), garage and drum & bass on the
//!   coast.
//! - **Ridgeline Radio** (Kit): synthwave and outrun (the new generator's),
//!   techno, psytrance, trance and eurobeat for the mountain roads.
//! - **Radio Pacífico** (Teo): chicha, with some house and garage of a
//!   morning (D1154).
//! - **The Roadhouse** (Tess, D1184): honky-tonk, the shuffle, bluegrass
//!   and outlaw (sound.md 3.5.1), with a little chicha from down the
//!   highway.
//! - **The Loft** (Dez, D1221): the city's club station, fast house (with
//!   the new generator's Chicago and deep house), techno, garage and drum &
//!   bass from a loft south of Market; the city levels' default, as the
//!   others' slow DJs don't fit downtown at speed.

use music::radio::Station;
use std::sync::LazyLock;

/// The table as JSON, for the web worklet and the Radio page.
pub const STATIONS_JSON: &str = include_str!("../stations.json");

/// The stations, in dial order.
pub static STATIONS: LazyLock<Vec<Station>> =
    LazyLock::new(|| Station::parse_list(STATIONS_JSON).expect("stations.json is a station table"));

/// A station by key.
pub fn station(key: &str) -> Option<&'static Station> {
    STATIONS.iter().find(|s| s.key == key)
}

/// The station a level tunes to by default (radio.md 7: as `LEVEL_TRACK`
/// picks a song today).
pub fn level_station(level: &str) -> &'static str {
    match level {
        "coast" | "seaside" => "tide",
        "sierra" => "ridgeline",
        // Downtown and the night cruise: the city's club station.
        "streets" | "cruise" => "loft",
        // The old highway, the freight train, the dry lake bed: country.
        "desert" => "roadhouse",
        _ => "tide",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_every_level_has_a_station() {
        assert_eq!(STATIONS.len(), 5);
        for level in [
            "coast", "seaside", "sierra", "streets", "cruise", "desert", "other",
        ] {
            assert!(station(level_station(level)).is_some(), "{level}");
        }
    }
}
