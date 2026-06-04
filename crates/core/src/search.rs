//! Multi-threaded vanity search engine (CPU, mnemonic-backed).

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

use coins_bip32::path::DerivationPath;
use coins_bip39::{English, Mnemonic};
use rand::rngs::OsRng;
use rayon::prelude::*;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::derive::{
    address_bytes, derive_signing_key, private_key_bytes, DEFAULT_DERIVATION_PATH,
};
use crate::matcher::Target;

/// How often (in attempts) each thread flushes its counter and thread 0 polls
/// the progress throttle. Keeps atomic traffic off the hot path.
const COUNTER_BATCH: u64 = 4096;

/// Minimum wall-clock gap between [`Progress`] callbacks.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// Hard upper bound on worker threads, guarding against resource exhaustion.
/// More threads than cores never helps a CPU-bound PBKDF2 workload.
const MAX_THREADS: usize = 256;

/// Configuration for a vanity search.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Words to look for (each with its own match mode).
    pub targets: Vec<Target>,
    /// Worker threads; `0` means "use all available cores".
    pub threads: usize,
    /// BIP-32 derivation path (defaults to [`DEFAULT_DERIVATION_PATH`]).
    pub derivation_path: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
            threads: 0,
            derivation_path: DEFAULT_DERIVATION_PATH.to_string(),
        }
    }
}

/// Progress snapshot reported periodically during a search.
#[derive(Debug, Clone, Copy)]
pub struct Progress {
    /// Total wallets generated so far (approximate; batched per thread).
    pub attempts: u64,
    /// Time elapsed since the search started.
    pub elapsed: Duration,
    /// How many distinct targets have been found.
    pub found: usize,
    /// Total number of targets being searched.
    pub total: usize,
}

/// A wallet that matched one of the targets.
///
/// Holds secret material ([`Self::mnemonic`], [`Self::private_key`]) in
/// zeroizing buffers; its [`fmt::Debug`] redacts them.
pub struct FoundWallet {
    /// The target word this wallet matched.
    pub target_word: String,
    /// `0x`-prefixed lowercase address.
    pub address: String,
    /// 12-word BIP-39 mnemonic, zeroized on drop.
    pub mnemonic: Zeroizing<String>,
    /// secp256k1 private key (32 bytes), zeroized on drop.
    pub private_key: Zeroizing<[u8; 32]>,
}

impl FoundWallet {
    /// Returns the private key as a `0x`-prefixed hex string (zeroized on drop).
    ///
    /// The hex is written directly into the zeroizing buffer (pre-sized to avoid
    /// reallocation) so no un-wiped plaintext copy of the key is left behind.
    #[must_use]
    pub fn private_key_hex(&self) -> Zeroizing<String> {
        use std::fmt::Write as _;
        let mut out = Zeroizing::new(String::with_capacity(2 + self.private_key.len() * 2));
        out.push_str("0x");
        for &byte in &*self.private_key {
            // Writing to a String is infallible.
            let _ = write!(out, "{byte:02x}");
        }
        out
    }
}

impl fmt::Debug for FoundWallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FoundWallet")
            .field("target_word", &self.target_word)
            .field("address", &self.address)
            .field("mnemonic", &"<redacted>")
            .field("private_key", &"<redacted>")
            .finish()
    }
}

/// Errors that prevent a search from starting.
#[derive(Debug, Error)]
pub enum SearchError {
    /// No targets were provided.
    #[error("no targets provided")]
    NoTargets,
    /// The derivation path is not a valid BIP-32 path.
    #[error("invalid derivation path")]
    InvalidPath,
    /// The worker thread pool could not be created.
    #[error("failed to start thread pool")]
    ThreadPool,
}

/// Searches for wallets whose address matches the configured targets.
///
/// Runs until every target is found or `stop` is set. `on_progress` is called
/// periodically (throttled), and `on_found` is called once per matched target
/// as it is discovered. All matched wallets are also returned.
///
/// # Errors
///
/// Returns [`SearchError`] if no targets are given, the derivation path is
/// invalid, or the worker pool cannot be created.
pub fn search(
    config: &SearchConfig,
    stop: &AtomicBool,
    on_progress: impl Fn(Progress) + Sync,
    on_found: impl Fn(&FoundWallet) + Sync,
) -> Result<Vec<FoundWallet>, SearchError> {
    if config.targets.is_empty() {
        return Err(SearchError::NoTargets);
    }
    // Validate the path once so an invalid path fails fast instead of per attempt.
    DerivationPath::from_str(&config.derivation_path).map_err(|_| SearchError::InvalidPath)?;

    let thread_count = if config.threads == 0 {
        available_parallelism().map_or(1, std::num::NonZeroUsize::get)
    } else {
        config.threads
    }
    .clamp(1, MAX_THREADS);

    let targets = &config.targets;
    let total = targets.len();
    let found_flags: Vec<AtomicBool> = (0..total).map(|_| AtomicBool::new(false)).collect();
    let total_attempts = AtomicU64::new(0);
    let results: Mutex<Vec<FoundWallet>> = Mutex::new(Vec::new());
    let start = Instant::now();
    let last_progress = Mutex::new(start);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build()
        .map_err(|_| SearchError::ThreadPool)?;

    pool.install(|| {
        (0..thread_count).into_par_iter().for_each(|thread_id| {
            let mut rng = OsRng;
            let mut local: u64 = 0;

            loop {
                if stop.load(Ordering::Relaxed)
                    || found_flags.iter().all(|f| f.load(Ordering::Relaxed))
                {
                    break;
                }

                local += 1;
                if local >= COUNTER_BATCH {
                    total_attempts.fetch_add(local, Ordering::Relaxed);
                    local = 0;
                    if thread_id == 0 {
                        report_progress(
                            &last_progress,
                            &total_attempts,
                            &found_flags,
                            start,
                            total,
                            &on_progress,
                        );
                    }
                }

                let Ok(mnemonic) = Mnemonic::<English>::new_with_count(&mut rng, 12) else {
                    continue;
                };
                let Ok(signing_key) = derive_signing_key(&mnemonic, &config.derivation_path) else {
                    continue;
                };

                let mut hex_buf = [0u8; 40];
                if hex::encode_to_slice(address_bytes(&signing_key), &mut hex_buf).is_err() {
                    continue;
                }
                let Ok(address_hex) = std::str::from_utf8(&hex_buf) else {
                    continue;
                };

                for (index, target) in targets.iter().enumerate() {
                    if found_flags[index].load(Ordering::Relaxed) || !target.matches(address_hex) {
                        continue;
                    }
                    // Only the first thread to flip the flag records the wallet.
                    if found_flags[index].swap(true, Ordering::Relaxed) {
                        continue;
                    }
                    let wallet = FoundWallet {
                        target_word: target.word().to_string(),
                        address: format!("0x{address_hex}"),
                        mnemonic: Zeroizing::new(mnemonic.to_phrase()),
                        private_key: private_key_bytes(&signing_key),
                    };
                    on_found(&wallet);
                    results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(wallet);
                }
            }
        });
    });

    Ok(results
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// Emits a throttled progress callback (called only from thread 0).
fn report_progress(
    last_progress: &Mutex<Instant>,
    total_attempts: &AtomicU64,
    found_flags: &[AtomicBool],
    start: Instant,
    total: usize,
    on_progress: &impl Fn(Progress),
) {
    let now = Instant::now();
    let mut last = last_progress
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if now.duration_since(*last) < PROGRESS_INTERVAL {
        return;
    }
    *last = now;
    let found = found_flags
        .iter()
        .filter(|f| f.load(Ordering::Relaxed))
        .count();
    on_progress(Progress {
        attempts: total_attempts.load(Ordering::Relaxed),
        elapsed: now.duration_since(start),
        found,
        total,
    });
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use zeroize::Zeroizing;

    use super::{search, FoundWallet, SearchConfig, SearchError};
    use crate::derive::{derive_from_phrase, DEFAULT_DERIVATION_PATH};
    use crate::matcher::Target;
    use crate::MatchMode;

    fn config_for(word: &str, mode: MatchMode, threads: usize) -> SearchConfig {
        SearchConfig {
            targets: vec![Target::new(word, mode).unwrap()],
            threads,
            derivation_path: DEFAULT_DERIVATION_PATH.to_string(),
        }
    }

    #[test]
    fn private_key_hex_is_0x_prefixed_and_zero_padded() {
        let mut key = [0u8; 32];
        key[0] = 0xab;
        key[31] = 0x05; // low byte must render as "05", not "5"
        let wallet = FoundWallet {
            target_word: "x".to_string(),
            address: "0x0".to_string(),
            mnemonic: Zeroizing::new(String::new()),
            private_key: Zeroizing::new(key),
        };

        let hex = wallet.private_key_hex();
        assert_eq!(hex.len(), 66);
        assert!(hex.starts_with("0xab"));
        assert!(hex.ends_with("05"));
    }

    #[test]
    fn finds_trivial_target_streams_and_round_trips() {
        let config = config_for("a", MatchMode::Anywhere, 0);
        let stop = AtomicBool::new(false);
        let streamed = AtomicUsize::new(0);

        let found = search(
            &config,
            &stop,
            |_| {},
            |_| {
                streamed.fetch_add(1, Ordering::Relaxed);
            },
        )
        .unwrap();

        assert_eq!(found.len(), 1);
        assert!(streamed.load(Ordering::Relaxed) >= 1);

        let wallet = &found[0];
        assert!(wallet.address.contains('a'));
        // Round-trip: the mnemonic re-derives to exactly this address.
        let rederived = derive_from_phrase(&wallet.mnemonic, DEFAULT_DERIVATION_PATH).unwrap();
        assert_eq!(rederived.address, wallet.address);
        assert_eq!(rederived.private_key.as_ref(), wallet.private_key.as_ref());
    }

    #[test]
    fn empty_targets_is_an_error() {
        let stop = AtomicBool::new(false);
        let result = search(&SearchConfig::default(), &stop, |_| {}, |_| {});
        assert!(matches!(result, Err(SearchError::NoTargets)));
    }

    #[test]
    fn invalid_path_is_an_error() {
        let mut config = config_for("a", MatchMode::Anywhere, 1);
        config.derivation_path = "not/a/path".to_string();
        let stop = AtomicBool::new(false);
        assert!(matches!(
            search(&config, &stop, |_| {}, |_| {}),
            Err(SearchError::InvalidPath)
        ));
    }

    #[test]
    fn preset_stop_flag_returns_no_results() {
        let config = config_for("deadbeef", MatchMode::Suffix, 1);
        let stop = AtomicBool::new(true);
        let found = search(&config, &stop, |_| {}, |_| {}).unwrap();
        assert!(found.is_empty());
    }
}
