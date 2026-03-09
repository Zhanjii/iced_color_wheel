//! auto_update.rs
//!
//! Automatic update checker and installer for desktop applications.
//!
//! Checks GitHub releases for newer versions and can download and install updates.
//!
//! # Example
//!
//! ```rust
//! use auto_update::AutoUpdater;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let updater = AutoUpdater::new("1.0.0", "owner/repo");
//!
//!     if updater.check_for_update().await? {
//!         let info = updater.get_update_info().await?;
//!         println!("New version available: {}", info.version);
//!         updater.download_and_install(None).await?;
//!     }
//!     Ok(())
//! }
//! ```

use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

// =============================================================================
// CONSTANTS
// =============================================================================

const GITHUB_API: &str = "https://api.github.com";

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur during update operations.
#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("No update available")]
    NoUpdateAvailable,

    #[error("Invalid version: {0}")]
    InvalidVersion(String),

    #[error("No download available for current platform")]
    NoPlatformDownload,

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}

/// Result type for update operations.
pub type UpdateResult<T> = Result<T, UpdateError>;

// =============================================================================
// GITHUB API TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    html_url: String,
    assets: Vec<GitHubAsset>,
    prerelease: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

// =============================================================================
// UPDATE INFO
// =============================================================================

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub release_notes: String,
    pub download_url: String,
    pub published_at: Option<String>,
    pub html_url: String,
}

// =============================================================================
// AUTO UPDATER
// =============================================================================

/// Checks for and installs application updates from GitHub releases.
#[derive(Debug, Clone)]
pub struct AutoUpdater {
    current_version: String,
    github_repo: String,
    prerelease: bool,
    github_token: Option<String>,
    latest_release: Option<GitHubRelease>,
}

impl AutoUpdater {
    /// Creates a new auto-updater.
    pub fn new(current_version: impl Into<String>, github_repo: impl Into<String>) -> Self {
        Self {
            current_version: current_version.into(),
            github_repo: github_repo.into(),
            prerelease: false,
            github_token: None,
            latest_release: None,
        }
    }

    /// Enables checking for pre-release versions.
    pub fn with_prerelease(mut self, prerelease: bool) -> Self {
        self.prerelease = prerelease;
        self
    }

    /// Sets GitHub token for private repositories.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.github_token = Some(token.into());
        self
    }

    /// Checks if a newer version is available.
    pub async fn check_for_update(&mut self) -> UpdateResult<bool> {
        let release = self.fetch_latest_release().await?;

        let latest_version = release.tag_name.trim_start_matches('v');
        let current_version = self.current_version.trim_start_matches('v');

        let is_newer = Self::compare_versions(latest_version, current_version)?;

        if is_newer {
            self.latest_release = Some(release);
            info!("Update available: {} -> {}", current_version, latest_version);
            Ok(true)
        } else {
            info!("Current version {} is up to date", current_version);
            Ok(false)
        }
    }

    /// Gets information about the available update.
    pub async fn get_update_info(&self) -> UpdateResult<UpdateInfo> {
        let release = self
            .latest_release
            .as_ref()
            .ok_or(UpdateError::NoUpdateAvailable)?;

        let download_url = self.get_platform_download_url(release)?;

        Ok(UpdateInfo {
            version: release.tag_name.trim_start_matches('v').to_string(),
            release_notes: release.body.clone().unwrap_or_default(),
            download_url,
            published_at: release.published_at.clone(),
            html_url: release.html_url.clone(),
        })
    }

    /// Downloads the update and launches installer.
    pub async fn download_and_install(
        &self,
        on_progress: Option<Box<dyn Fn(u64, u64) + Send>>,
    ) -> UpdateResult<()> {
        let release = self
            .latest_release
            .as_ref()
            .ok_or(UpdateError::NoUpdateAvailable)?;

        let download_url = self.get_platform_download_url(release)?;

        info!("Downloading update from {}", download_url);
        let installer_path = self.download_file(&download_url, on_progress).await?;

        info!("Launching installer...");
        self.launch_installer(&installer_path)?;

        Ok(())
    }

    // Private methods

    async fn fetch_latest_release(&self) -> UpdateResult<GitHubRelease> {
        let client = Client::new();
        let url = if self.prerelease {
            format!("{}/repos/{}/releases", GITHUB_API, self.github_repo)
        } else {
            format!("{}/repos/{}/releases/latest", GITHUB_API, self.github_repo)
        };

        let mut request = client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json");

        if let Some(ref token) = self.github_token {
            request = request.header("Authorization", format!("token {}", token));
        }

        let response = request.send().await?;

        if self.prerelease {
            let releases: Vec<GitHubRelease> = response.json().await?;
            releases
                .into_iter()
                .next()
                .ok_or(UpdateError::NoUpdateAvailable)
        } else {
            Ok(response.json().await?)
        }
    }

    fn get_platform_download_url(&self, release: &GitHubRelease) -> UpdateResult<String> {
        let patterns = Self::get_platform_patterns();

        for asset in &release.assets {
            let name_lower = asset.name.to_lowercase();
            if patterns.iter().any(|p| name_lower.contains(p)) {
                return Ok(asset.browser_download_url.clone());
            }
        }

        // Fallback: return first asset
        release
            .assets
            .first()
            .map(|a| a.browser_download_url.clone())
            .ok_or(UpdateError::NoPlatformDownload)
    }

    fn get_platform_patterns() -> Vec<&'static str> {
        if cfg!(target_os = "windows") {
            vec![".exe", "windows", "win64", "win32"]
        } else if cfg!(target_os = "macos") {
            vec![".dmg", ".app", "macos", "darwin", "osx"]
        } else {
            vec![".appimage", "linux", ".deb", ".rpm"]
        }
    }

    async fn download_file(
        &self,
        url: &str,
        on_progress: Option<Box<dyn Fn(u64, u64) + Send>>,
    ) -> UpdateResult<PathBuf> {
        let client = Client::new();
        let response = client.get(url).send().await?;
        let total_size = response.content_length().unwrap_or(0);

        let temp_dir = env::temp_dir();
        let file_name = url.split('/').last().unwrap_or("update");
        let file_path = temp_dir.join(file_name);

        let mut file = fs::File::create(&file_path).await?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if let Some(ref callback) = on_progress {
                callback(downloaded, total_size);
            }
        }

        info!("Downloaded to: {:?}", file_path);
        Ok(file_path)
    }

    fn launch_installer(&self, installer_path: &PathBuf) -> UpdateResult<()> {
        #[cfg(target_os = "windows")]
        {
            Command::new(installer_path).spawn()?;
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(installer_path).spawn()?;
        }

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(installer_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(installer_path, perms)?;
            Command::new(installer_path).spawn()?;
        }

        // Exit current application
        info!("Exiting for update installation");
        std::process::exit(0);
    }

    fn compare_versions(v1: &str, v2: &str) -> UpdateResult<bool> {
        // Simple semantic version comparison
        let parse_version = |v: &str| -> Result<Vec<u32>, UpdateError> {
            v.split('.')
                .map(|s| {
                    s.parse::<u32>()
                        .map_err(|_| UpdateError::InvalidVersion(v.to_string()))
                })
                .collect()
        };

        let ver1 = parse_version(v1)?;
        let ver2 = parse_version(v2)?;

        for (a, b) in ver1.iter().zip(ver2.iter()) {
            if a > b {
                return Ok(true);
            } else if a < b {
                return Ok(false);
            }
        }

        Ok(ver1.len() > ver2.len())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(AutoUpdater::compare_versions("1.2.0", "1.1.0").unwrap());
        assert!(!AutoUpdater::compare_versions("1.1.0", "1.2.0").unwrap());
        assert!(!AutoUpdater::compare_versions("1.0.0", "1.0.0").unwrap());
        assert!(AutoUpdater::compare_versions("2.0.0", "1.9.9").unwrap());
    }

    #[test]
    fn test_platform_patterns() {
        let patterns = AutoUpdater::get_platform_patterns();
        assert!(!patterns.is_empty());
    }
}
