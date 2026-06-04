//! Key derivation: BIP-39 mnemonic → BIP-32 path → secp256k1 key → Ethereum address.
//!
//! The derivation matches MetaMask and Foundry: a BIP-39 seed (empty
//! passphrase) is derived along [`DEFAULT_DERIVATION_PATH`], the secp256k1
//! public key is hashed with Keccak-256, and the last 20 bytes form the address.

use std::fmt;

use coins_bip32::prelude::XPriv;
use coins_bip39::{English, Mnemonic};
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

/// Standard Ethereum derivation path for account 0 (MetaMask / Foundry default).
pub const DEFAULT_DERIVATION_PATH: &str = "m/44'/60'/0'/0/0";

/// A derived wallet: its `0x`-prefixed lowercase address and the private key.
///
/// The private key is held in a [`Zeroizing`] buffer and wiped on drop.
pub struct Derived {
    /// `0x`-prefixed lowercase address.
    pub address: String,
    /// secp256k1 private key (32 bytes), zeroized on drop.
    pub private_key: Zeroizing<[u8; 32]>,
}

impl fmt::Debug for Derived {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Derived")
            .field("address", &self.address)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

/// Errors from key derivation.
#[derive(Debug, Error)]
pub enum DeriveError {
    /// The mnemonic phrase is not valid BIP-39.
    #[error("invalid mnemonic phrase")]
    Mnemonic,
    /// The derivation path is not a valid BIP-32 path.
    #[error("invalid derivation path")]
    Path,
    /// BIP-32 / secp256k1 key derivation failed.
    #[error("key derivation failed")]
    Derive,
}

/// Derives the signing key for `mnemonic` along `path`.
pub(crate) fn derive_signing_key(
    mnemonic: &Mnemonic<English>,
    path: &str,
) -> Result<SigningKey, DeriveError> {
    let seed = Zeroizing::new(
        mnemonic
            .to_seed(Some(""))
            .map_err(|_| DeriveError::Mnemonic)?,
    );
    let root = XPriv::root_from_seed(&*seed, None).map_err(|_| DeriveError::Derive)?;
    let child = root.derive_path(path).map_err(|_| DeriveError::Path)?;
    let signing_key: &SigningKey = child.as_ref();
    Ok(signing_key.clone())
}

/// Computes the 20 raw address bytes from a signing key (no allocation).
pub(crate) fn address_bytes(signing_key: &SigningKey) -> [u8; 20] {
    let encoded = signing_key.verifying_key().to_encoded_point(false);
    // `encoded` is the uncompressed point `0x04 || X || Y`; hash X || Y.
    let hash = Keccak256::digest(&encoded.as_bytes()[1..]);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

/// Formats 20 address bytes as a `0x`-prefixed lowercase string.
fn address_hex(bytes: &[u8; 20]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    s.push_str(&hex::encode(bytes));
    s
}

/// Extracts the private key bytes of a signing key into a zeroizing buffer.
pub(crate) fn private_key_bytes(signing_key: &SigningKey) -> Zeroizing<[u8; 32]> {
    // `to_bytes()` returns a fresh secret copy; wipe it after copying out.
    let mut field = signing_key.to_bytes();
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(field.as_ref());
    field[..].zeroize();
    key
}

/// Derives a wallet (address + private key) from a BIP-39 phrase.
///
/// # Errors
///
/// Returns [`DeriveError`] if the phrase is not valid BIP-39 or `path` is not a
/// valid BIP-32 derivation path.
pub fn derive_from_phrase(phrase: &str, path: &str) -> Result<Derived, DeriveError> {
    let mnemonic =
        Mnemonic::<English>::new_from_phrase(phrase).map_err(|_| DeriveError::Mnemonic)?;
    let signing_key = derive_signing_key(&mnemonic, path)?;
    Ok(Derived {
        address: address_hex(&address_bytes(&signing_key)),
        private_key: private_key_bytes(&signing_key),
    })
}

/// Computes the `0x`-prefixed lowercase address for a raw secp256k1 private key.
///
/// # Errors
///
/// Returns [`DeriveError::Derive`] if the bytes do not form a valid secp256k1
/// private key (zero or out of range).
pub fn address_from_private_key(key: &[u8; 32]) -> Result<String, DeriveError> {
    let field_bytes = k256::FieldBytes::from(*key);
    let signing_key = SigningKey::from_bytes(&field_bytes).map_err(|_| DeriveError::Derive)?;
    Ok(address_hex(&address_bytes(&signing_key)))
}

#[cfg(test)]
mod tests {
    use super::{address_from_private_key, derive_from_phrase, DEFAULT_DERIVATION_PATH};

    /// Canonical Foundry/Anvil/Hardhat test mnemonic and its account-0 wallet.
    const ANVIL_PHRASE: &str = "test test test test test test test test test test test junk";
    const ANVIL_ADDRESS: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    const ANVIL_PRIVATE: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn decode32(hex_str: &str) -> [u8; 32] {
        let bytes = hex::decode(hex_str).unwrap();
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        out
    }

    #[test]
    fn derives_anvil_vector_address_and_key() {
        let derived = derive_from_phrase(ANVIL_PHRASE, DEFAULT_DERIVATION_PATH).unwrap();
        assert_eq!(derived.address, ANVIL_ADDRESS);
        assert_eq!(hex::encode(derived.private_key.as_ref()), ANVIL_PRIVATE);
    }

    #[test]
    fn address_from_private_key_matches_vector() {
        let key = decode32(ANVIL_PRIVATE);
        assert_eq!(address_from_private_key(&key).unwrap(), ANVIL_ADDRESS);
    }

    #[test]
    fn rejects_invalid_phrase() {
        assert!(derive_from_phrase("definitely not a mnemonic", DEFAULT_DERIVATION_PATH).is_err());
    }

    #[test]
    fn rejects_invalid_path() {
        assert!(derive_from_phrase(ANVIL_PHRASE, "not/a/path").is_err());
    }
}
