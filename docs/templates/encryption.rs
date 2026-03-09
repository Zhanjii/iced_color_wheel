//! encryption.rs
//!
//! File-based encryption for bundled credentials.
//!
//! This module provides symmetric encryption for storing
//! credentials in files that can be bundled with your application.
//!
//! Use this when you need to:
//! - Bundle API credentials with a distributed app
//! - Store team-shared secrets in encrypted files
//! - Encrypt configuration files
//!
//! # Security Note
//!
//! The encryption key is bundled with the app, so this provides
//! obfuscation rather than true security. Combine with remote
//! PIN activation for proper access control.
//!
//! # Usage
//!
//! ```rust
//! use encryption::EncryptionManager;
//! use std::path::Path;
//!
//! // First time: Generate a key (do this once, save the key file)
//! let key = EncryptionManager::generate_key(Some(Path::new("keys/encryption.key")));
//!
//! // Runtime: Load key and encrypt/decrypt
//! let enc = EncryptionManager::from_key_file(Path::new("keys/encryption.key")).unwrap();
//!
//! // Encrypt credentials to a file
//! let creds = serde_json::json!({"api_key": "secret123"});
//! enc.encrypt_to_file(&creds, Path::new("keys/credentials.enc")).unwrap();
//!
//! // Decrypt credentials from file
//! let decrypted = enc.decrypt_from_file(Path::new("keys/credentials.enc")).unwrap();
//! ```

use base64::{Engine as _, engine::general_purpose};
use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM};
use ring::error::Unspecified;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::Value;
use std::fs;
use std::path::Path;

const NONCE_LEN: usize = 12;

// =============================================================================
// NONCE SEQUENCE
// =============================================================================

struct CounterNonceSequence(u32);

impl NonceSequence for CounterNonceSequence {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        let mut nonce_bytes = vec![0u8; NONCE_LEN];
        let bytes = self.0.to_le_bytes();
        nonce_bytes[..4].copy_from_slice(&bytes);
        self.0 += 1;
        Nonce::try_assume_unique_for_key(&nonce_bytes)
    }
}

// =============================================================================
// ENCRYPTION MANAGER
// =============================================================================

/// Manages encryption for credential files using AES-256-GCM
pub struct EncryptionManager {
    key: Vec<u8>,
    rng: SystemRandom,
}

impl EncryptionManager {
    /// Initialize encryption manager from key bytes
    pub fn from_key(key: Vec<u8>) -> Result<Self, String> {
        if key.len() != 32 {
            return Err("Key must be 32 bytes for AES-256".to_string());
        }

        Ok(Self {
            key,
            rng: SystemRandom::new(),
        })
    }

    /// Initialize encryption manager from key file
    pub fn from_key_file<P: AsRef<Path>>(key_path: P) -> Result<Self, String> {
        let key = fs::read(key_path.as_ref())
            .map_err(|e| format!("Failed to read key file: {}", e))?;

        Self::from_key(key)
    }

    /// Generate a new encryption key
    pub fn generate_key<P: AsRef<Path>>(output_path: Option<P>) -> Vec<u8> {
        let rng = SystemRandom::new();
        let mut key = vec![0u8; 32];
        rng.fill(&mut key).expect("Failed to generate random key");

        if let Some(path) = output_path {
            let _ = fs::create_dir_all(path.as_ref().parent().unwrap());
            fs::write(path.as_ref(), &key).expect("Failed to write key file");
        }

        key
    }

    /// Encrypt bytes
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.key)
            .map_err(|_| "Failed to create encryption key".to_string())?;

        let nonce_sequence = CounterNonceSequence(0);
        let mut sealing_key = SealingKey::new(unbound_key, nonce_sequence);

        let mut in_out = data.to_vec();
        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .map_err(|_| "Encryption failed".to_string())?;

        Ok(in_out)
    }

    /// Decrypt bytes
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, String> {
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.key)
            .map_err(|_| "Failed to create decryption key".to_string())?;

        let nonce_sequence = CounterNonceSequence(0);
        let mut opening_key = OpeningKey::new(unbound_key, nonce_sequence);

        let mut in_out = encrypted.to_vec();
        let decrypted = opening_key
            .open_in_place(Aad::empty(), &mut in_out)
            .map_err(|_| "Decryption failed: Invalid key or corrupted data".to_string())?;

        Ok(decrypted.to_vec())
    }

    /// Encrypt a JSON value and save to file
    pub fn encrypt_to_file<P: AsRef<Path>>(&self, data: &Value, path: P) -> Result<(), String> {
        let json_str = serde_json::to_string_pretty(data)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;

        let encrypted = self.encrypt(json_str.as_bytes())?;

        fs::create_dir_all(path.as_ref().parent().unwrap())
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        fs::write(path.as_ref(), encrypted)
            .map_err(|e| format!("Failed to write encrypted file: {}", e))?;

        Ok(())
    }

    /// Decrypt a file to JSON value
    pub fn decrypt_from_file<P: AsRef<Path>>(&self, path: P) -> Result<Value, String> {
        let encrypted = fs::read(path.as_ref())
            .map_err(|e| format!("Failed to read encrypted file: {}", e))?;

        let decrypted = self.decrypt(&encrypted)?;

        let json_str = String::from_utf8(decrypted)
            .map_err(|e| format!("Decrypted data is not valid UTF-8: {}", e))?;

        serde_json::from_str(&json_str)
            .map_err(|e| format!("Decrypted data is not valid JSON: {}", e))
    }

    /// Encrypt a string and return base64-encoded result
    pub fn encrypt_string(&self, data: &str) -> Result<String, String> {
        let encrypted = self.encrypt(data.as_bytes())?;
        Ok(general_purpose::STANDARD.encode(encrypted))
    }

    /// Decrypt a base64-encoded encrypted string
    pub fn decrypt_string(&self, encrypted: &str) -> Result<String, String> {
        let encrypted_bytes = general_purpose::STANDARD
            .decode(encrypted)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;

        let decrypted = self.decrypt(&encrypted_bytes)?;

        String::from_utf8(decrypted)
            .map_err(|e| format!("Decrypted data is not valid UTF-8: {}", e))
    }
}

/// Ensure encryption key exists, creating if necessary
pub fn setup_encryption_key<P: AsRef<Path>>(key_dir: P, key_filename: &str) -> Result<std::path::PathBuf, String> {
    let key_path = key_dir.as_ref().join(key_filename);

    if !key_path.exists() {
        EncryptionManager::generate_key(Some(&key_path));
    }

    Ok(key_path)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_encryption_roundtrip() {
        let key = EncryptionManager::generate_key(None::<&Path>);
        let enc = EncryptionManager::from_key(key).unwrap();

        let plaintext = b"Hello, World!";
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_file_encryption() {
        let temp_dir = tempdir().unwrap();
        let key_path = temp_dir.path().join("key.bin");
        let data_path = temp_dir.path().join("data.enc");

        let key = EncryptionManager::generate_key(Some(&key_path));
        let enc = EncryptionManager::from_key(key).unwrap();

        let data = serde_json::json!({
            "api_key": "secret_key_123",
            "api_secret": "secret_value_456"
        });

        enc.encrypt_to_file(&data, &data_path).unwrap();

        let decrypted = enc.decrypt_from_file(&data_path).unwrap();
        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_string_encryption() {
        let key = EncryptionManager::generate_key(None::<&Path>);
        let enc = EncryptionManager::from_key(key).unwrap();

        let plaintext = "secret message";
        let encrypted = enc.encrypt_string(plaintext).unwrap();
        let decrypted = enc.decrypt_string(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}
