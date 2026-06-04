//! `hexforge-core` — vanity Ethereum address search engine.
//!
//! Generates wallets, derives their addresses, and searches for a chosen hex
//! word at the start, anywhere, or the end of the address. The CPU engine
//! produces full wallets backed by a BIP-39 mnemonic.
//!
//! The crate performs no file or network I/O and never logs secret material;
//! persistence and presentation are the caller's responsibility.

mod derive;
mod matcher;
mod search;

pub use derive::{
    address_from_private_key, derive_from_phrase, DeriveError, Derived, DEFAULT_DERIVATION_PATH,
};
pub use matcher::Target;
pub use search::{search, FoundWallet, Progress, SearchConfig, SearchError};

use thiserror::Error;

/// Number of hex characters in an Ethereum address, excluding the `0x` prefix.
pub const ADDRESS_HEX_LEN: usize = 40;

/// Where the target word must appear inside the address (the 40 hex chars after `0x`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    /// Anywhere in the address.
    Anywhere,
    /// Right after the `0x` prefix.
    Prefix,
    /// At the very end of the address.
    Suffix,
}

/// Reasons a target word is rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidateError {
    /// The word was empty after trimming.
    #[error("empty target")]
    Empty,
    /// The word contains a character that is not a hex digit (`0-9a-f`).
    #[error("non-hex character '{0}' (addresses use only 0-9, a-f)")]
    NonHex(char),
    /// The word is longer than an address can hold.
    #[error("target too long: {len} chars (max {max})")]
    TooLong {
        /// Length of the supplied word.
        len: usize,
        /// Maximum searchable length ([`ADDRESS_HEX_LEN`]).
        max: usize,
    },
}

/// Validates and normalizes a target word.
///
/// An Ethereum address is hexadecimal, so a searchable word may contain only
/// `0-9` and `a-f`. The word is trimmed and lowercased; the normalized form is
/// returned on success.
///
/// # Errors
///
/// Returns [`ValidateError`] if the word is empty, contains a non-hex
/// character, or is longer than [`ADDRESS_HEX_LEN`].
pub fn validate_target(word: &str) -> Result<String, ValidateError> {
    let normalized = word.trim().to_lowercase();

    if normalized.is_empty() {
        return Err(ValidateError::Empty);
    }
    if normalized.len() > ADDRESS_HEX_LEN {
        return Err(ValidateError::TooLong {
            len: normalized.len(),
            max: ADDRESS_HEX_LEN,
        });
    }
    if let Some(bad) = normalized.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(ValidateError::NonHex(bad));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{validate_target, ValidateError, ADDRESS_HEX_LEN};

    #[test]
    fn accepts_and_normalizes_hex_words() {
        assert_eq!(validate_target("DeadBeef").unwrap(), "deadbeef");
        assert_eq!(validate_target("  cafe ").unwrap(), "cafe");
        assert_eq!(validate_target("c0ffee").unwrap(), "c0ffee");
    }

    #[test]
    fn rejects_non_hex_reporting_first_bad_char() {
        assert_eq!(validate_target("rustok"), Err(ValidateError::NonHex('r')));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(validate_target("   "), Err(ValidateError::Empty));
    }

    #[test]
    fn rejects_too_long() {
        let word = "a".repeat(ADDRESS_HEX_LEN + 1);
        assert_eq!(
            validate_target(&word),
            Err(ValidateError::TooLong {
                len: ADDRESS_HEX_LEN + 1,
                max: ADDRESS_HEX_LEN,
            })
        );
    }

    #[test]
    fn accepts_word_of_exactly_address_length() {
        let word = "a".repeat(ADDRESS_HEX_LEN);
        assert!(validate_target(&word).is_ok());
    }
}
