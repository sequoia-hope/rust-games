//! Prints the announcers' script as JSON, for `tools/announcers/render.py`:
//! `cargo run -p sb_radio --example script > tools/announcers/lines.json`.
fn main() {
    print!("{}", sb_radio::script_json());
}
