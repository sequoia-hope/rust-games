//! The native client: `cargo run -p sb_game -- --course timber --go`.
//! (On the web the library is the entry point; this binary is native only.)

#[cfg(not(target_arch = "wasm32"))]
fn main() -> bevy::app::AppExit {
    sb_game::run_native()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
