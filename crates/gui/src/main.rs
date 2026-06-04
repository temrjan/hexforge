//! `hexforge` — GUI entry point.
//!
//! Status: scaffold. The egui interface lands in PR2; for now this binary runs
//! a tiny search to confirm the engine works end-to-end. It prints addresses
//! only — never the mnemonic or private key.

use std::sync::atomic::AtomicBool;

use hexforge_core::{search, MatchMode, SearchConfig, Target, DEFAULT_DERIVATION_PATH};

fn main() {
    println!(
        "hexforge {} — vanity Ethereum address forge",
        env!("CARGO_PKG_VERSION")
    );

    let config = SearchConfig {
        targets: vec![Target::new("a", MatchMode::Suffix).expect("\"a\" is a valid target")],
        threads: 0,
        derivation_path: DEFAULT_DERIVATION_PATH.to_string(),
    };
    let stop = AtomicBool::new(false);

    match search(&config, &stop, |_| {}, |_| {}) {
        Ok(found) => {
            for wallet in &found {
                println!("found ...{} -> {}", wallet.target_word, wallet.address);
            }
        }
        Err(error) => eprintln!("search error: {error}"),
    }

    println!("(scaffold demo - full GUI lands in PR2)");
}
