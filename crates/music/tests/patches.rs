//! The patch table against the lab: the canonical JSON of every `BPATCH`
//! entry, hashed, is the lab's (`node tools/music-lab/test/dump-tables.mjs`
//! prints it).

use music::json::{Val, canon};
use music::patches::{LAB_SHA256, all};
use sha2::{Digest, Sha256};

#[test]
fn the_table_is_the_labs() {
    let obj = Val::Obj(
        all()
            .iter()
            .map(|(n, l)| ((*n).to_owned(), l.to_val()))
            .collect(),
    );
    let text = canon(&obj);
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    assert_eq!(hash, LAB_SHA256, "{text}");
}

#[test]
fn every_patch_round_trips_through_json() {
    for (name, lab) in music::patches::all() {
        let back = music::track::Lab::from_val(&lab.to_val());
        assert_eq!(&back, lab, "{name}");
    }
}
