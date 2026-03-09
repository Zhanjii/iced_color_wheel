//! test_helpers.rs
//!
//! Test helper functions and utilities for consistent testing across the project.
//!
//! This module provides:
//! - Platform detection utilities
//! - Temporary directory fixtures
//! - Mock configuration helpers
//! - Common test credentials
//!
//! # Usage
//!
//! ```rust
//! use test_helpers::{TempDir, mock_credentials};
//!
//! #[test]
//! fn test_with_temp_dir() {
//!     let temp = TempDir::new().unwrap();
//!     // Use temp.path() for testing
//!     // Automatically cleaned up when dropped
//! }
//! ```

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// =============================================================================
// PLATFORM DETECTION
// =============================================================================

/// Check if running on Windows
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Check if running on macOS
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Check if running on Linux
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

// =============================================================================
// SINGLETON RESET UTILITIES
// =============================================================================

/// Trait for resetting singleton instances (for test isolation)
pub trait Resettable {
    /// Reset the singleton instance to its initial state
    fn reset();
}

// =============================================================================
// TEMPORARY DIRECTORY FIXTURE
// =============================================================================

/// Temporary directory that is automatically cleaned up when dropped
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create a new temporary directory
    pub fn new() -> std::io::Result<Self> {
        let path = tempfile::tempdir()?.into_path();
        Ok(Self { path })
    }

    /// Get the path to the temporary directory
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Convert to PathBuf
    pub fn into_path(self) -> PathBuf {
        self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// =============================================================================
// MOCK CREDENTIALS
// =============================================================================

/// Standard test credentials for API testing
///
/// Never use real credentials in tests!
pub fn mock_credentials() -> std::collections::HashMap<String, String> {
    let mut creds = std::collections::HashMap::new();
    creds.insert("api_key".to_string(), "test_api_key_12345".to_string());
    creds.insert("api_secret".to_string(), "test_api_secret_67890".to_string());
    creds.insert("username".to_string(), "test_user".to_string());
    creds
}

// =============================================================================
// TEST MARKERS
// =============================================================================

/// Macro to skip test if not on Windows
#[macro_export]
macro_rules! skip_if_not_windows {
    () => {
        if !$crate::test_helpers::is_windows() {
            return;
        }
    };
}

/// Macro to skip test if not on macOS
#[macro_export]
macro_rules! skip_if_not_macos {
    () => {
        if !$crate::test_helpers::is_macos() {
            return;
        }
    };
}

/// Macro to skip test if not on Linux
#[macro_export]
macro_rules! skip_if_not_linux {
    () => {
        if !$crate::test_helpers::is_linux() {
            return;
        }
    };
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        // At least one should be true
        assert!(is_windows() || is_macos() || is_linux());
    }

    #[test]
    fn test_temp_dir() {
        let temp = TempDir::new().unwrap();
        assert!(temp.path().exists());
        let path = temp.path().to_path_buf();
        drop(temp);
        // Directory should be cleaned up after drop
        assert!(!path.exists());
    }

    #[test]
    fn test_mock_credentials() {
        let creds = mock_credentials();
        assert!(creds.contains_key("api_key"));
        assert!(creds.contains_key("api_secret"));
        assert_eq!(creds.get("username").unwrap(), "test_user");
    }
}
