//! `hexforge-core` — vanity Ethereum address search engine.
//!
//! Status: scaffold. The generation/derivation/search engine lands in PR1.
//! This module currently provides the shared types and input validation that
//! both the engine and the GUI build on.

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidateError {
    /// The word was empty after trimming.
    Empty,
    /// The word contains a character that is not a hex digit (`0-9a-f`).
    NonHex(char),
    /// The word is longer than an address can hold.
    TooLong { len: usize, max: usize },
}

impl std::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidateError::Empty => write!(f, "empty target"),
            ValidateError::NonHex(c) => {
                write!(f, "non-hex character '{c}' (addresses use only 0-9, a-f)")
            }
            ValidateError::TooLong { len, max } => {
                write!(f, "target too long: {len} chars (max {max})")
            }
        }
    }
}

impl std::error::Error for ValidateError {}

/// Validate and normalize a target word.
///
/// An Ethereum address is hexadecimal, so a searchable word may contain only
/// `0-9` and `a-f`. The word is trimmed and lowercased; the normalized form is
/// returned on success.
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
    use super::*;

    #[test]
    fn accepts_and_normalizes_hex_words() {
        assert_eq!(validate_target("DeadBeef").unwrap(), "deadbeef");
        assert_eq!(validate_target("  cafe ").unwrap(), "cafe");
        assert_eq!(validate_target("c0ffee").unwrap(), "c0ffee");
    }

    #[test]
    fn rejects_non_hex_reporting_first_bad_char() {
        // 'r' is not a hex digit.
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
