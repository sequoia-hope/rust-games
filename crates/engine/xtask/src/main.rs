//! `cargo xtask <command>`: the kit's own tasks, from `pk_xtask`
//! (`check-deps` over `deps.toml`, `imgdiff`). A game's xtask is the same
//! few lines plus a `WebBuild` and its own commands (`template/xtask`).

fn main() {
    if let Err(e) = pk_xtask::main_with(std::env::args().skip(1), &[]) {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
}
