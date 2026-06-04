//! Target words and address matching.

use crate::{validate_target, MatchMode, ValidateError};

/// A validated search target: a hex word plus where it must appear.
#[derive(Debug, Clone)]
pub struct Target {
    word: String,
    mode: MatchMode,
}

impl Target {
    /// Creates a target from a word and match mode, validating the word.
    ///
    /// # Errors
    ///
    /// Returns [`ValidateError`] if `word` is not a valid hex pattern (see
    /// [`validate_target`]).
    pub fn new(word: &str, mode: MatchMode) -> Result<Self, ValidateError> {
        Ok(Self {
            word: validate_target(word)?,
            mode,
        })
    }

    /// The normalized (lowercase, hex) target word.
    #[must_use]
    pub fn word(&self) -> &str {
        &self.word
    }

    /// Where the word must appear in the address.
    #[must_use]
    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    /// Returns `true` if `address_hex` (the 40 hex chars after `0x`) matches.
    #[must_use]
    pub fn matches(&self, address_hex: &str) -> bool {
        match self.mode {
            MatchMode::Anywhere => address_hex.contains(&self.word),
            MatchMode::Prefix => address_hex.starts_with(&self.word),
            MatchMode::Suffix => address_hex.ends_with(&self.word),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Target;
    use crate::MatchMode;

    // 40 hex chars (no 0x) ending in deadbeef.
    const ADDR: &str = "132e8e18719627002091d24bb8406191deadbeef";

    #[test]
    fn suffix_matches_only_at_end() {
        let target = Target::new("deadbeef", MatchMode::Suffix).unwrap();
        assert!(target.matches(ADDR));
        assert!(!target.matches("deadbeef00000000000000000000000000000000"));
    }

    #[test]
    fn prefix_matches_only_at_start() {
        let target = Target::new("132e", MatchMode::Prefix).unwrap();
        assert!(target.matches(ADDR));
        assert!(!target.matches("0000132e00000000000000000000000000000000"));
    }

    #[test]
    fn anywhere_matches_in_the_middle() {
        let target = Target::new("d24b", MatchMode::Anywhere).unwrap();
        assert!(target.matches(ADDR));
        assert!(!target.matches("000000000000000000000000000000000000abcd"));
    }

    #[test]
    fn rejects_non_hex_word() {
        assert!(Target::new("xyz", MatchMode::Anywhere).is_err());
    }
}
