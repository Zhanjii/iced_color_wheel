//! remote_config.rs
//!
//! Remote configuration sync with GitHub repositories.
//!
//! Provides bidirectional synchronization of configuration files with a GitHub repository,
//! enabling centralized config management, remote updates, and versioning.
//!
//! # Example
//!
//! ```rust
//! use remote_config::{RemoteConfigSync, ConfigSyncManager};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Direct sync with GitHub
//!     let sync = RemoteConfigSync::new(
//!         "ghp_xxx",
//!         "owner/config-repo",
//!         "app_settings.json",
//!     );
//!
//!     // Pull remote config
//!     let config = sync.pull().await?;
//!
//!     // Push local changes
//!     sync.push(&config, Some("Update settings")).await?;
//!
//!     // Higher-level manager with local caching
//!     let manager = ConfigSyncManager::new(
//!         PathBuf::from("config/settings.json"),
//!         Some(sync),
//!     );
//!     let config = manager.load_and_sync().await?;
//!     Ok(())
//! }
//! ```

use base64::{engine::general_purpose, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs;
use tracing::{error, info};

// =============================================================================
// CONSTANTS
// =============================================================================

const GITHUB_API: &str = "https://api.github.com";

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur during remote config operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}

/// Result type for config operations.
pub type ConfigResult<T> = Result<T, ConfigError>;

// =============================================================================
// GITHUB API TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
struct GitHubFileResponse {
    sha: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct GitHubPutRequest {
    message: String,
    content: String,
    branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<String>,
}

// =============================================================================
// REMOTE CONFIG SYNC
// =============================================================================

/// Synchronize configuration with a GitHub repository.
#[derive(Debug, Clone)]
pub struct RemoteConfigSync {
    github_token: String,
    repo: String,
    config_file: String,
    branch: String,
    file_sha: Option<String>,
}

impl RemoteConfigSync {
    /// Creates a new remote config sync.
    pub fn new(
        github_token: impl Into<String>,
        repo: impl Into<String>,
        config_file: impl Into<String>,
    ) -> Self {
        Self {
            github_token: github_token.into(),
            repo: repo.into(),
            config_file: config_file.into(),
            branch: "main".to_string(),
            file_sha: None,
        }
    }

    /// Sets a custom branch (default: main).
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = branch.into();
        self
    }

    /// Pulls configuration from remote repository.
    pub async fn pull(&mut self) -> ConfigResult<Value> {
        let client = Client::new();
        let url = format!(
            "{}/repos/{}/contents/{}",
            GITHUB_API, self.repo, self.config_file
        );

        let response = client
            .get(&url)
            .header("Authorization", format!("token {}", self.github_token))
            .header("Accept", "application/vnd.github.v3+json")
            .query(&[("ref", &self.branch)])
            .send()
            .await?;

        if response.status() == 404 {
            return Err(ConfigError::NotFound(self.config_file.clone()));
        }

        let file_data: GitHubFileResponse = response.json().await?;

        // Store SHA for later updates
        self.file_sha = Some(file_data.sha);

        // Decode base64 content
        let content = general_purpose::STANDARD
            .decode(&file_data.content.replace('\n', ""))
            .map_err(|e| ConfigError::HttpError(format!("Base64 decode error: {}", e)))?;

        let config: Value = serde_json::from_slice(&content)?;

        info!("Pulled config from {}/{}", self.repo, self.config_file);
        Ok(config)
    }

    /// Pushes configuration to remote repository.
    pub async fn push(&mut self, config: &Value, message: Option<&str>) -> ConfigResult<()> {
        let client = Client::new();
        let url = format!(
            "{}/repos/{}/contents/{}",
            GITHUB_API, self.repo, self.config_file
        );

        // Encode content as base64
        let content_json = serde_json::to_string_pretty(config)?;
        let content_b64 = general_purpose::STANDARD.encode(content_json.as_bytes());

        let payload = GitHubPutRequest {
            message: message
                .unwrap_or(&format!("Update config - {}", chrono::Utc::now()))
                .to_string(),
            content: content_b64,
            branch: self.branch.clone(),
            sha: self.file_sha.clone(),
        };

        let response = client
            .put(&url)
            .header("Authorization", format!("token {}", self.github_token))
            .header("Accept", "application/vnd.github.v3+json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(ConfigError::HttpError(error_text));
        }

        let result: GitHubFileResponse = response.json().await?;
        self.file_sha = Some(result.sha);

        info!("Pushed config to {}/{}", self.repo, self.config_file);
        Ok(())
    }

    /// Checks if the remote config file exists.
    pub async fn file_exists(&self) -> bool {
        let client = Client::new();
        let url = format!(
            "{}/repos/{}/contents/{}",
            GITHUB_API, self.repo, self.config_file
        );

        client
            .get(&url)
            .header("Authorization", format!("token {}", self.github_token))
            .query(&[("ref", &self.branch)])
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

// =============================================================================
// CONFIG SYNC MANAGER
// =============================================================================

/// High-level configuration manager with local caching.
#[derive(Debug)]
pub struct ConfigSyncManager {
    local_path: PathBuf,
    remote_sync: Option<RemoteConfigSync>,
    config: Value,
}

impl ConfigSyncManager {
    /// Creates a new config sync manager.
    pub fn new(local_path: PathBuf, remote_sync: Option<RemoteConfigSync>) -> Self {
        Self {
            local_path,
            remote_sync,
            config: Value::Object(serde_json::Map::new()),
        }
    }

    /// Loads configuration from local file.
    pub async fn load_local(&mut self) -> ConfigResult<&Value> {
        if self.local_path.exists() {
            let content = fs::read_to_string(&self.local_path).await?;
            self.config = serde_json::from_str(&content)?;
        } else {
            self.config = Value::Object(serde_json::Map::new());
        }

        Ok(&self.config)
    }

    /// Saves configuration to local file.
    pub async fn save_local(&self) -> ConfigResult<()> {
        if let Some(parent) = self.local_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.local_path, content).await?;
        Ok(())
    }

    /// Loads local config and syncs with remote.
    pub async fn load_and_sync(&mut self) -> ConfigResult<&Value> {
        self.load_local().await?;

        if let Some(ref mut sync) = self.remote_sync {
            match sync.pull().await {
                Ok(remote_config) => {
                    self.config = self.merge_configs(&self.config, &remote_config);
                    self.save_local().await?;
                    info!("Config synced with remote");
                }
                Err(e) => {
                    error!("Remote sync failed, using local: {}", e);
                }
            }
        }

        Ok(&self.config)
    }

    /// Saves config locally and pushes to remote.
    pub async fn save_and_sync(&mut self, config: Value) -> ConfigResult<()> {
        self.config = config;
        self.save_local().await?;

        if let Some(ref mut sync) = self.remote_sync {
            sync.push(&self.config, Some("Sync config")).await?;
        }

        Ok(())
    }

    /// Gets a config value by key (supports dot notation).
    pub fn get(&self, key: &str) -> Option<&Value> {
        let keys: Vec<&str> = key.split('.').collect();
        let mut value = &self.config;

        for k in keys {
            value = value.get(k)?;
        }

        Some(value)
    }

    /// Sets a config value by key (supports dot notation).
    pub fn set(&mut self, key: &str, value: Value) {
        let keys: Vec<&str> = key.split('.').collect();
        let mut current = &mut self.config;

        for k in &keys[..keys.len() - 1] {
            if !current.is_object() {
                *current = Value::Object(serde_json::Map::new());
            }
            current = current
                .as_object_mut()
                .unwrap()
                .entry(k.to_string())
                .or_insert(Value::Object(serde_json::Map::new()));
        }

        if let Some(obj) = current.as_object_mut() {
            obj.insert(keys.last().unwrap().to_string(), value);
        }
    }

    /// Gets reference to current config.
    pub fn config(&self) -> &Value {
        &self.config
    }

    // Helper: merge two configs (remote wins for conflicts)
    fn merge_configs(&self, local: &Value, remote: &Value) -> Value {
        match (local, remote) {
            (Value::Object(local_map), Value::Object(remote_map)) => {
                let mut result = local_map.clone();
                for (key, value) in remote_map {
                    result.insert(key.clone(), value.clone());
                }
                Value::Object(result)
            }
            _ => remote.clone(),
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_creation() {
        let sync = RemoteConfigSync::new("token", "owner/repo", "config.json");
        assert_eq!(sync.repo, "owner/repo");
        assert_eq!(sync.config_file, "config.json");
        assert_eq!(sync.branch, "main");
    }

    #[test]
    fn test_custom_branch() {
        let sync =
            RemoteConfigSync::new("token", "owner/repo", "config.json").with_branch("develop");
        assert_eq!(sync.branch, "develop");
    }

    #[tokio::test]
    async fn test_manager_set_get() {
        let mut manager = ConfigSyncManager::new(PathBuf::from("/tmp/test.json"), None);

        manager.set("app.name", Value::String("TestApp".to_string()));
        manager.set("app.version", Value::String("1.0.0".to_string()));

        assert_eq!(
            manager.get("app.name"),
            Some(&Value::String("TestApp".to_string()))
        );
        assert_eq!(
            manager.get("app.version"),
            Some(&Value::String("1.0.0".to_string()))
        );
        assert_eq!(manager.get("missing.key"), None);
    }
}
