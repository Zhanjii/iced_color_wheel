//! app_activation.rs
//!
//! Remote PIN-based activation system.
//!
//! Features:
//! - Remote configuration via GitHub
//! - PIN activation with hash storage
//! - Kill switch capability
//! - Minimum version enforcement
//! - Offline grace period

use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Update this URL for your config repository
const REMOTE_CONFIG_URL: &str =
    "https://raw.githubusercontent.com/your-org/your-app-config/main/app_access.json";

/// Days to allow offline use after successful activation
const GRACE_PERIOD_DAYS: i64 = 7;

// =============================================================================
// ACTIVATION STATUS
// =============================================================================

/// Possible activation states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationStatus {
    /// Successfully activated
    Activated,
    /// First run, needs activation code
    CodeRequired,
    /// Stored code no longer matches remote
    CodeInvalid,
    /// App version below minimum required
    VersionBlocked,
    /// App globally disabled via kill switch
    AppDisabled,
    /// Cannot reach remote config
    NetworkError,
}

// =============================================================================
// REMOTE CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RemoteConfig {
    enabled: bool,
    #[serde(default)]
    min_app_version: String,
    #[serde(default)]
    activation_code: String,
    #[serde(default)]
    latest_version: String,
    #[serde(default)]
    block_message: String,
    #[serde(default)]
    invalid_code_message: String,
    #[serde(default)]
    success_message: String,
    #[serde(default)]
    release_notes: String,
    #[serde(default)]
    download_url_win: String,
    #[serde(default)]
    download_url_mac: String,
}

// =============================================================================
// ACTIVATION MANAGER
// =============================================================================

/// Manages app activation via remote PIN validation
pub struct ActivationManager {
    config_manager: Arc<RwLock<dyn ConfigStore>>,
    remote_config: Arc<RwLock<Option<RemoteConfig>>>,
}

/// Trait for configuration storage (for dependency injection)
pub trait ConfigStore: Send + Sync {
    fn get(&self, key: &str, section: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: String, section: &str);
}

impl ActivationManager {
    /// Create a new activation manager
    pub fn new(config_manager: Arc<RwLock<dyn ConfigStore>>) -> Self {
        Self {
            config_manager,
            remote_config: Arc::new(RwLock::new(None)),
        }
    }

    /// Check current activation status
    pub fn check_activation(&self, app_version: &str) -> ActivationStatus {
        // Try to fetch remote config
        let remote = self.fetch_remote_config();

        if remote.is_none() {
            // Network error - check grace period
            return self.check_grace_period();
        }

        let remote = remote.unwrap();
        *self.remote_config.write().unwrap() = Some(remote.clone());

        // Check kill switch
        if !remote.enabled {
            return ActivationStatus::AppDisabled;
        }

        // Check minimum version
        if !Self::version_allowed(app_version, &remote.min_app_version) {
            return ActivationStatus::VersionBlocked;
        }

        // Check activation code
        let config = self.config_manager.read().unwrap();
        let stored_hash = config.get("activation_code_hash", "activation");

        if stored_hash.is_none() {
            return ActivationStatus::CodeRequired;
        }

        let stored_hash = stored_hash.unwrap();

        // Verify stored hash matches current remote code
        let expected_hash = Self::hash_code(&remote.activation_code);

        if stored_hash != expected_hash {
            return ActivationStatus::CodeInvalid;
        }

        // Update activation timestamp for grace period
        drop(config);
        let mut config_mut = self.config_manager.write().unwrap();
        config_mut.set(
            "activation_timestamp",
            Utc::now().to_rfc3339(),
            "activation",
        );

        ActivationStatus::Activated
    }

    /// Attempt activation with provided code
    pub fn activate_with_code(&self, code: &str) -> bool {
        let remote = self.remote_config.read().unwrap();
        if remote.is_none() {
            return false;
        }

        let remote = remote.as_ref().unwrap();
        if code != remote.activation_code {
            return false;
        }

        // Store hash, not plaintext
        let mut config = self.config_manager.write().unwrap();
        config.set(
            "activation_code_hash",
            Self::hash_code(code),
            "activation",
        );
        config.set(
            "activation_timestamp",
            Utc::now().to_rfc3339(),
            "activation",
        );

        true
    }

    /// Get user-facing message for status
    pub fn get_message(&self, status: ActivationStatus) -> String {
        let remote = self.remote_config.read().unwrap();

        if remote.is_none() {
            return match status {
                ActivationStatus::NetworkError => {
                    "Unable to verify activation. Please check your internet connection."
                        .to_string()
                }
                _ => String::new(),
            };
        }

        let remote = remote.as_ref().unwrap();

        match status {
            ActivationStatus::AppDisabled => remote.block_message.clone(),
            ActivationStatus::VersionBlocked => remote.block_message.clone(),
            ActivationStatus::CodeInvalid => remote.invalid_code_message.clone(),
            ActivationStatus::CodeRequired => "Please enter your activation code.".to_string(),
            ActivationStatus::Activated => remote.success_message.clone(),
            ActivationStatus::NetworkError => {
                "Unable to verify activation. Please check your internet connection.".to_string()
            }
        }
    }

    /// Check for available updates
    pub fn check_for_updates(&self, current_version: &str) -> Option<UpdateInfo> {
        let remote = self.remote_config.read().unwrap();
        if remote.is_none() {
            return None;
        }

        let remote = remote.as_ref().unwrap();
        let latest = &remote.latest_version;

        if Self::normalize_version(latest) > Self::normalize_version(current_version) {
            let platform_key = if cfg!(target_os = "windows") {
                &remote.download_url_win
            } else {
                &remote.download_url_mac
            };

            Some(UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: latest.clone(),
                release_notes: remote.release_notes.clone(),
                download_url: platform_key.clone(),
            })
        } else {
            None
        }
    }

    /// Clear stored activation (for testing or reset)
    pub fn clear_activation(&self) {
        let mut config = self.config_manager.write().unwrap();
        config.set("activation_code_hash", String::new(), "activation");
        config.set("activation_timestamp", String::new(), "activation");
    }

    // =========================================================================
    // PRIVATE HELPERS
    // =========================================================================

    fn fetch_remote_config(&self) -> Option<RemoteConfig> {
        let response = reqwest::blocking::get(REMOTE_CONFIG_URL)
            .ok()?
            .json::<RemoteConfig>()
            .ok()?;
        Some(response)
    }

    fn check_grace_period(&self) -> ActivationStatus {
        let config = self.config_manager.read().unwrap();
        let timestamp = config.get("activation_timestamp", "activation");

        if timestamp.is_none() {
            return ActivationStatus::NetworkError;
        }

        if let Ok(last_activation) = DateTime::parse_from_rfc3339(&timestamp.unwrap()) {
            let grace_end = last_activation + Duration::days(GRACE_PERIOD_DAYS);
            if Utc::now() < grace_end {
                return ActivationStatus::Activated;
            }
        }

        ActivationStatus::NetworkError
    }

    fn hash_code(code: &str) -> String {
        use ring::digest;
        let hash = digest::digest(&digest::SHA256, code.as_bytes());
        hex::encode(hash.as_ref())
    }

    fn version_allowed(current: &str, minimum: &str) -> bool {
        Self::normalize_version(current) >= Self::normalize_version(minimum)
    }

    fn normalize_version(v: &str) -> Vec<u32> {
        let re = Regex::new(r"\.dev\d+$").unwrap();
        let clean = re.replace(v, "");
        clean
            .split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    }
}

// =============================================================================
// UPDATE INFO
// =============================================================================

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: String,
    pub download_url: String,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    struct MockConfigStore {
        data: HashMap<String, String>,
    }

    impl MockConfigStore {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }
    }

    impl ConfigStore for MockConfigStore {
        fn get(&self, key: &str, _section: &str) -> Option<String> {
            self.data.get(key).cloned()
        }

        fn set(&mut self, key: &str, value: String, _section: &str) {
            self.data.insert(key.to_string(), value);
        }
    }

    #[test]
    fn test_version_normalization() {
        assert!(ActivationManager::normalize_version("1.2.3") < ActivationManager::normalize_version("1.2.4"));
        assert!(ActivationManager::normalize_version("1.2.0.dev0") < ActivationManager::normalize_version("1.2.1"));
    }

    #[test]
    fn test_hash_code() {
        let hash1 = ActivationManager::hash_code("1234");
        let hash2 = ActivationManager::hash_code("1234");
        assert_eq!(hash1, hash2);

        let hash3 = ActivationManager::hash_code("5678");
        assert_ne!(hash1, hash3);
    }
}
