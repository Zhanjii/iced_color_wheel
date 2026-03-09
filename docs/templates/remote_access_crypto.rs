//! remote_access_crypto.rs
//!
//! PIN-derived cryptographic utilities for remote configuration.
//!
//! Provides secure hashing and encryption functions for:
//! - PIN verification using salted SHA-256 hashes
//! - Field encryption/decryption using AES-GCM with PIN-derived keys
//! - PBKDF2 key derivation for secure key generation from PINs
//!
//! # Usage
//!
//! ```rust
//! use remote_access_crypto::{hash_pin, verify_pin_hash, derive_key_from_pin};
//!
//! // Generate PIN hash for remote config
//! let (pin_hash, salt) = hash_pin("1234", None);
//! // Store pin_hash and salt in your remote config
//!
//! // Verify entered PIN against stored hash
//! let is_valid = verify_pin_hash("1234", &pin_hash, &salt);
//!
//! // Derive encryption key from PIN
//! let key = derive_key_from_pin("1234", &encryption_salt);
//! ```

use base64::{Engine as _, engine::general_purpose};
use ring::digest;
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use std::num::NonZeroU32;

/// PBKDF2 iterations for key derivation
/// OWASP recommends 600,000+ for SHA-256 as of 2023
const KEY_DERIVATION_ITERATIONS: u32 = 480_000;

/// Generate a cryptographically secure random salt
pub fn generate_salt(length: usize) -> Vec<u8> {
    let rng = SystemRandom::new();
    let mut salt = vec![0u8; length];
    rng.fill(&mut salt).expect("Failed to generate random salt");
    salt
}

/// Create a salted SHA-256 hash of the activation PIN
///
/// Use this when setting up the remote config to generate
/// the hash that will be stored publicly.
///
/// # Returns
///
/// Tuple of (hex_hash, base64_salt)
pub fn hash_pin(pin: &str, salt: Option<Vec<u8>>) -> (String, String) {
    let salt = salt.unwrap_or_else(|| generate_salt(16));

    // Combine salt and PIN, then hash
    let mut combined = salt.clone();
    combined.extend_from_slice(pin.trim().as_bytes());

    let hash = digest::digest(&digest::SHA256, &combined);
    let hex_hash = hex::encode(hash.as_ref());
    let salt_b64 = general_purpose::STANDARD.encode(&salt);

    (hex_hash, salt_b64)
}

/// Verify an entered PIN against a stored hash
pub fn verify_pin_hash(pin: &str, stored_hash: &str, salt_b64: &str) -> bool {
    let salt = match general_purpose::STANDARD.decode(salt_b64) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let mut combined = salt;
    combined.extend_from_slice(pin.trim().as_bytes());

    let computed_hash = digest::digest(&digest::SHA256, &combined);
    let computed_hex = hex::encode(computed_hash.as_ref());

    // Constant-time comparison to prevent timing attacks
    use subtle::ConstantTimeEq;
    computed_hex.as_bytes().ct_eq(stored_hash.as_bytes()).into()
}

/// Derive a 32-byte encryption key from a PIN using PBKDF2
///
/// This allows encrypting sensitive fields in the remote config
/// that can only be decrypted by users who know the PIN.
pub fn derive_key_from_pin(pin: &str, salt_b64: &str) -> Vec<u8> {
    let salt = general_purpose::STANDARD.decode(salt_b64)
        .unwrap_or_else(|_| salt_b64.as_bytes().to_vec());

    let iterations = NonZeroU32::new(KEY_DERIVATION_ITERATIONS).unwrap();
    let mut key = vec![0u8; 32];

    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        &salt,
        pin.trim().as_bytes(),
        &mut key,
    );

    key
}

/// Encrypt a field value using the derived key
pub fn encrypt_field(plaintext: &str, key: &[u8]) -> Result<String, String> {
    use ring::aead::{Aad, BoundKey, Nonce, SealingKey, UnboundKey, AES_256_GCM};

    if key.len() != 32 {
        return Err("Key must be 32 bytes".to_string());
    }

    let unbound_key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| "Failed to create encryption key".to_string())?;

    let nonce_sequence = super::encryption::CounterNonceSequence(0);
    let mut sealing_key = SealingKey::new(unbound_key, nonce_sequence);

    let mut in_out = plaintext.as_bytes().to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut in_out)
        .map_err(|_| "Encryption failed".to_string())?;

    Ok(general_purpose::STANDARD.encode(in_out))
}

/// Decrypt a field value using the derived key
pub fn decrypt_field(encrypted: &str, key: &[u8]) -> Result<String, String> {
    use ring::aead::{Aad, BoundKey, OpeningKey, UnboundKey, AES_256_GCM};

    if key.len() != 32 {
        return Err("Key must be 32 bytes".to_string());
    }

    let encrypted_bytes = general_purpose::STANDARD.decode(encrypted)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;

    let unbound_key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| "Failed to create decryption key".to_string())?;

    let nonce_sequence = super::encryption::CounterNonceSequence(0);
    let mut opening_key = OpeningKey::new(unbound_key, nonce_sequence);

    let mut in_out = encrypted_bytes;
    let decrypted = opening_key
        .open_in_place(Aad::empty(), &mut in_out)
        .map_err(|_| "Decryption failed: Invalid token or wrong key".to_string())?;

    String::from_utf8(decrypted.to_vec())
        .map_err(|e| format!("Decrypted data is not valid UTF-8: {}", e))
}

/// Generate all secrets needed for a remote config file
///
/// Run this locally to generate the values for your public config.
pub fn generate_remote_config_secrets(pin: &str) -> serde_json::Value {
    let (pin_hash, pin_salt) = hash_pin(pin, None);
    let encryption_salt = general_purpose::STANDARD.encode(generate_salt(16));

    serde_json::json!({
        "pin_hash": pin_hash,
        "pin_salt": pin_salt,
        "encryption_salt": encryption_salt,
        "_instructions": "Use these values in your remote config. To encrypt fields, use derive_key_from_pin then encrypt_field"
    })
}

/// Convenience function to encrypt a value for remote config
pub fn encrypt_for_config(value: &str, pin: &str, encryption_salt: &str) -> Result<String, String> {
    let key = derive_key_from_pin(pin, encryption_salt);
    encrypt_field(value, &key)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_hashing() {
        let pin = "1234";
        let (hash1, salt) = hash_pin(pin, None);

        assert!(verify_pin_hash(pin, &hash1, &salt));
        assert!(!verify_pin_hash("wrong", &hash1, &salt));
    }

    #[test]
    fn test_key_derivation() {
        let pin = "1234";
        let salt = general_purpose::STANDARD.encode(generate_salt(16));

        let key1 = derive_key_from_pin(pin, &salt);
        let key2 = derive_key_from_pin(pin, &salt);

        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_field_encryption() {
        let pin = "1234";
        let salt = general_purpose::STANDARD.encode(generate_salt(16));
        let key = derive_key_from_pin(pin, &salt);

        let plaintext = "secret-api-key";
        let encrypted = encrypt_field(plaintext, &key).unwrap();
        let decrypted = decrypt_field(&encrypted, &key).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_generate_secrets() {
        let secrets = generate_remote_config_secrets("1234");
        assert!(secrets["pin_hash"].is_string());
        assert!(secrets["pin_salt"].is_string());
        assert!(secrets["encryption_salt"].is_string());
    }
}
