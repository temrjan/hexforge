//! `hexforge` — GUI entry point.
//!
//! Status: scaffold. The egui interface lands in PR2; for now this binary only
//! confirms the workspace wires together and exercises `hexforge-core`.

fn main() {
    println!(
        "hexforge {} — vanity Ethereum address forge",
        env!("CARGO_PKG_VERSION")
    );

    // Sanity check that the core crate is linked and usable.
    match hexforge_core::validate_target("deadbeef") {
        Ok(word) => println!("core ok: '{word}' is a valid target"),
        Err(e) => eprintln!("core error: {e}"),
    }

    println!("GUI not built yet (scaffold). See README.md for the roadmap.");
}
