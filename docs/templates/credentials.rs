//! credentials.rs
//!
//! Secure credential management using system keyring.
//!
//! This module provides a simple interface for storing and retrieving
//! credentials using the operating system's secure credential storage
//! (Windows Credential Manager, macOS Keychain, Linux Secret Service).
//!
//! # Usage
//!
//! ```rust
//! use credentials::CredentialManager;
//!
//! // Create manager for your app
//! let creds = CredentialManager::new("my_app_name");
//!
//! // Store credentials
//! creds.store("api_key", "secret_value_123").unwrap();
//! creds.store("oauth_token", "token_abc").unwrap();
//!
//! // Retrieve credentials
//! let api_key = creds.get("api_key").unwrap();
//!
//! // Delete credentials
//! creds.delete("api_key").unwrap();
//! ```

use keyring::{Entry, Error as KeyringError};
use std::fmt;

// =============================================================================
// ERROR TYPE
// =============================================================================

/// Error type for credential operations
#[derive(Debug)]
pub enum CredentialError {
    /// Keyring backend error
    KeyringError(KeyringError),
    /// Credential not found
    NotFound(String),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyringError(e) => write!(f, "Keyring error: {}", e),
            Self::NotFound(key) => write!(f, "Credential not found: {}", key),
        }
    }
}

impl std::error::Error for CredentialError {}

impl From<KeyringError> for CredentialError {
    fn from(error: KeyringError) -> Self {
        Self::KeyringError(error)
    }
}

// =============================================================================
// CREDENTIAL MANAGER
// =============================================================================

/// Manages secure credential storage using system keyring
///
/// The keyring library uses the OS-native secure storage:
/// - Windows: Credential Manager
/// - macOS: Keychain
/// - Linux: Secret Service (GNOME Keyring, KWallet)
pub struct CredentialManager {
    service_name: String,
}

impl CredentialManager {
    /// Initialize credential manager
    ///
    /// # Arguments
    ///
    /// * `service_name` - Unique identifier for your app in the keyring.
    ///                    Use a consistent name across your app.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Store a credential securely
    pub fn store(&self, key: &str, value: &str) -> Result<(), CredentialError> {
        let entry = Entry::new(&self.service_name, key)?;
        entry.set_password(value)?;
        Ok(())
    }

    /// Retrieve a credential
    pub fn get(&self, key: &str) -> Result<String, CredentialError> {
        let entry = Entry::new(&self.service_name, key)?;
        match entry.get_password() {
            Ok(password) => Ok(password),
            Err(KeyringError::NoEntry) => Err(CredentialError::NotFound(key.to_string())),
            Err(e) => Err(CredentialError::KeyringError(e)),
        }
    }

    /// Remove a credential from storage
    pub fn delete(&self, key: &str) -> Result<(), CredentialError> {
        let entry = Entry::new(&self.service_name, key)?;
        match entry.delete_password() {
            Ok(()) => Ok(()),
            Err(KeyringError::NoEntry) => Ok(()), // Already deleted - consider success
            Err(e) => Err(CredentialError::KeyringError(e)),
        }
    }

    /// Check if a credential exists
    pub fn exists(&self, key: &str) -> bool {
        self.get(key).is_ok()
    }

    /// Clear multiple credentials
    ///
    /// Returns the number of credentials successfully deleted
    pub fn clear_all(&self, keys: &[&str]) -> usize {
        let mut deleted = 0;
        for key in keys {
            if self.delete(key).is_ok() {
                deleted += 1;
            }
        }
        deleted
    }

    /// Update an existing credential (same as store, but for clarity)
    pub fn update(&self, key: &str, value: &str) -> Result<(), CredentialError> {
        self.store(key, value)
    }
}

// =============================================================================
// STANDARD CREDENTIAL KEYS
// =============================================================================

/// Standard credential key names
pub struct CredentialKeys;

impl CredentialKeys {
    pub const API_KEY: &'static str = "api_key";
    pub const API_SECRET: &'static str = "api_secret";
    pub const ACCESS_TOKEN: &'static str = "access_token";
    pub const REFRESH_TOKEN: &'static str = "refresh_token";
    pub const OAUTH_TOKEN: &'static str = "oauth_token";
    pub const GITHUB_TOKEN: &'static str = "github_token";
    pub const PASSWORD: &'static str = "password";
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_storage() {
        let creds = CredentialManager::new("test_app_credentials");
        let test_key = "test_credential";

        // Clean up any existing test credential
        let _ = creds.delete(test_key);

        // Store
        creds.store(test_key, "test_value_123").unwrap();

        // Retrieve
        let value = creds.get(test_key).unwrap();
        assert_eq!(value, "test_value_123");

        // Check existence
        assert!(creds.exists(test_key));

        // Update
        creds.update(test_key, "new_value_456").unwrap();
        let updated = creds.get(test_key).unwrap();
        assert_eq!(updated, "new_value_456");

        // Delete
        creds.delete(test_key).unwrap();
        assert!(!creds.exists(test_key));

        // Verify deletion
        let result = creds.get(test_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_clear_all() {
        let creds = CredentialManager::new("test_app_clear_all");

        // Store multiple credentials
        creds.store("key1", "value1").unwrap();
        creds.store("key2", "value2").unwrap();
        creds.store("key3", "value3").unwrap();

        // Clear all
        let deleted = creds.clear_all(&["key1", "key2", "key3", "nonexistent"]);
        assert_eq!(deleted, 3);

        // Verify deletion
        assert!(!creds.exists("key1"));
        assert!(!creds.exists("key2"));
        assert!(!creds.exists("key3"));
    }
}
