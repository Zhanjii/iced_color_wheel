//! settings_export.rs
//!
//! Settings export/import functionality for backing up and restoring application settings.
//!
//! Features:
//! - Export all settings to a ZIP bundle (.your_app_settings file)
//! - Import settings from bundle with automatic backup
//! - Manifest with version info and file metadata
//! - Path traversal protection during extraction
//! - JSON validation before importing configs
//! - Suspicious content detection (code injection prevention)
//! - Version compatibility checking
//! - Export preview (see what will be included)
//!
//! The bundle format:
//!     your_app_settings_20250109_143022.yas  (ZIP file)
//!     ├── manifest.json          # Export metadata
//!     ├── configs/               # JSON config files
//!     │   ├── config.json
//!     │   ├── credentials.json
//!     │   └── ...
//!     └── resources/             # Additional resources
//!         └── templates/
//!             └── custom_template.txt
//!
//! # Example
//!
//! ```rust
//! use settings_export::SettingsExporter;
//! use std::path::Path;
//!
//! let exporter = SettingsExporter::new(Path::new("~/.your_app"));
//!
//! // Export settings
//! let export_path = exporter.export_all_settings(None)?;
//! println!("Exported to: {:?}", export_path);
//!
//! // Preview before export
//! let preview = exporter.get_export_preview()?;
//! println!("Will export {} config files", preview.configs.len());
//!
//! // Import settings (with automatic backup)
//! exporter.import_settings(Path::new("path/to/export.yas"), true)?;
//! ```

use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use zip::{ZipArchive, ZipWriter};

// =============================================================================
// CONSTANTS
// =============================================================================

/// File extension for settings bundles
const BUNDLE_EXTENSION: &str = ".yas";

/// Maximum file size for config validation (10MB)
const MAX_CONFIG_SIZE: u64 = 10 * 1024 * 1024;

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Path traversal detected: {0}")]
    PathTraversal(String),

    #[error("Invalid bundle: {0}")]
    InvalidBundle(String),

    #[error("Incompatible version: {0}")]
    IncompatibleVersion(String),

    #[error("Suspicious content: {0}")]
    SuspiciousContent(String),
}

pub type Result<T> = std::result::Result<T, SettingsError>;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Manifest for settings bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub app_version: String,
    pub export_date: String,
    pub platform: String,
    pub configs: Vec<FileInfo>,
    pub resources: Vec<ResourceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub file: String,
    pub size: u64,
    pub modified: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub category: String,
    pub file: String,
    pub size: u64,
}

/// Export preview information
#[derive(Debug, Clone)]
pub struct ExportPreview {
    pub configs: Vec<ConfigPreview>,
    pub resources: Vec<ResourcePreview>,
    pub total_size_kb: f64,
}

#[derive(Debug, Clone)]
pub struct ConfigPreview {
    pub name: String,
    pub size_kb: f64,
}

#[derive(Debug, Clone)]
pub struct ResourcePreview {
    pub category: String,
    pub name: String,
    pub size_kb: f64,
}

// =============================================================================
// SETTINGS EXPORTER
// =============================================================================

/// Handles exporting and importing application settings
pub struct SettingsExporter {
    config_dir: PathBuf,
    version: String,
    additional_resource_dirs: HashMap<String, PathBuf>,
}

impl SettingsExporter {
    /// Create a new settings exporter
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
            version: "1.0.0".to_string(),
            additional_resource_dirs: HashMap::new(),
        }
    }

    /// Add an additional resource directory to export
    pub fn add_resource_dir(&mut self, name: impl Into<String>, path: impl AsRef<Path>) {
        self.additional_resource_dirs
            .insert(name.into(), path.as_ref().to_path_buf());
    }

    /// Export all application settings to a bundle file
    pub fn export_all_settings(&self, export_path: Option<PathBuf>) -> Result<PathBuf> {
        let export_path = export_path.unwrap_or_else(|| {
            let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
            PathBuf::from(format!("your_app_settings_{}{}", timestamp, BUNDLE_EXTENSION))
        });

        let temp_dir = tempfile::tempdir()?;

        // Create manifest
        let mut manifest = Manifest {
            version: self.version.clone(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            export_date: Local::now().to_rfc3339(),
            platform: std::env::consts::OS.to_string(),
            configs: Vec::new(),
            resources: Vec::new(),
        };

        // Copy all JSON config files
        if self.config_dir.exists() {
            for entry in fs::read_dir(&self.config_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let relative = path.file_name().unwrap();
                    let dest = temp_dir.path().join("configs").join(relative);
                    fs::create_dir_all(dest.parent().unwrap())?;
                    fs::copy(&path, &dest)?;

                    let metadata = path.metadata()?;
                    manifest.configs.push(FileInfo {
                        file: relative.to_string_lossy().to_string(),
                        size: metadata.len(),
                        modified: metadata.modified().ok().and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .ok()
                                .map(|d| d.as_secs_f64())
                        }),
                    });
                }
            }
        }

        // Export additional resources
        for (resource_name, resource_dir) in &self.additional_resource_dirs {
            if !resource_dir.exists() {
                continue;
            }

            for entry in walkdir::WalkDir::new(resource_dir) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let relative = entry.path().strip_prefix(resource_dir).unwrap();
                    let dest = temp_dir
                        .path()
                        .join("resources")
                        .join(resource_name)
                        .join(relative);

                    fs::create_dir_all(dest.parent().unwrap())?;
                    fs::copy(entry.path(), &dest)?;

                    let metadata = entry.metadata()?;
                    manifest.resources.push(ResourceInfo {
                        category: resource_name.clone(),
                        file: relative.to_string_lossy().to_string(),
                        size: metadata.len(),
                    });
                }
            }
        }

        // Save manifest
        let manifest_path = temp_dir.path().join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        fs::write(&manifest_path, manifest_json)?;

        // Create ZIP bundle
        let zip_file = File::create(&export_path)?;
        let mut zip = ZipWriter::new(zip_file);

        for entry in walkdir::WalkDir::new(temp_dir.path()) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let path = entry.path();
                let name = path.strip_prefix(temp_dir.path()).unwrap();

                zip.start_file(name.to_string_lossy().to_string(), Default::default())?;
                let mut file = File::open(path)?;
                std::io::copy(&mut file, &mut zip)?;
            }
        }

        zip.finish()?;

        Ok(export_path)
    }

    /// Import settings from a bundle file
    pub fn import_settings(&self, import_path: impl AsRef<Path>, backup: bool) -> Result<()> {
        let import_path = import_path.as_ref();

        if !import_path.exists() {
            return Err(SettingsError::InvalidBundle("File not found".to_string()));
        }

        // Backup current settings if requested
        if backup {
            let backup_path = self.backup_current_settings()?;
            eprintln!("Backed up current settings to {:?}", backup_path);
        }

        // Extract to temporary directory
        let temp_dir = tempfile::tempdir()?;
        self.safe_extract_all(import_path, temp_dir.path())?;

        // Validate manifest
        let manifest_path = temp_dir.path().join("manifest.json");
        if !manifest_path.exists() {
            return Err(SettingsError::InvalidBundle(
                "Missing manifest".to_string(),
            ));
        }

        let manifest_content = fs::read_to_string(&manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&manifest_content)?;

        // Check version compatibility
        if !self.check_version_compatibility(&manifest.version) {
            return Err(SettingsError::IncompatibleVersion(manifest.version));
        }

        // Copy config files with validation
        let configs_dir = temp_dir.path().join("configs");
        if configs_dir.exists() {
            fs::create_dir_all(&self.config_dir)?;

            for entry in fs::read_dir(&configs_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if !self.validate_config_file(&path)? {
                        eprintln!("Skipping invalid config file: {:?}", path.file_name());
                        continue;
                    }

                    let dest = self.config_dir.join(path.file_name().unwrap());
                    fs::copy(&path, &dest)?;
                }
            }
        }

        Ok(())
    }

    /// Create a backup of current settings
    fn backup_current_settings(&self) -> Result<PathBuf> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = self.config_dir.join("backups");
        fs::create_dir_all(&backup_dir)?;
        let backup_path = backup_dir.join(format!("settings_backup_{}{}", timestamp, BUNDLE_EXTENSION));
        self.export_all_settings(Some(backup_path.clone()))?;
        Ok(backup_path)
    }

    /// Check if export version is compatible
    fn check_version_compatibility(&self, export_version: &str) -> bool {
        let current_major: u32 = self
            .version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let export_major: u32 = export_version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        current_major == export_major
    }

    /// Safely extract all files from a zip archive
    fn safe_extract_all(&self, zip_path: impl AsRef<Path>, target_dir: impl AsRef<Path>) -> Result<()> {
        let file = File::open(zip_path)?;
        let mut archive = ZipArchive::new(file)?;
        let target_dir = target_dir.as_ref().canonicalize()?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = target_dir.join(file.name());

            // Check for path traversal
            if contains_path_traversal(file.name()) {
                return Err(SettingsError::PathTraversal(file.name().to_string()));
            }

            // Ensure path is within target directory
            let canonical = outpath.canonicalize().unwrap_or(outpath.clone());
            if !canonical.starts_with(&target_dir) {
                return Err(SettingsError::PathTraversal(file.name().to_string()));
            }

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }

        Ok(())
    }

    /// Validate a config file before importing
    fn validate_config_file(&self, config_file: impl AsRef<Path>) -> Result<bool> {
        let config_file = config_file.as_ref();

        // Check file size
        let metadata = fs::metadata(config_file)?;
        if metadata.len() > MAX_CONFIG_SIZE {
            return Ok(false);
        }

        // Read and parse JSON
        let content = fs::read_to_string(config_file)?;

        // Check for suspicious patterns
        let suspicious_patterns = [
            "__import__",
            "eval(",
            "exec(",
            "compile(",
            "subprocess",
            "os.system",
            "os.popen",
            "<script",
            "javascript:",
        ];

        let content_lower = content.to_lowercase();
        for pattern in &suspicious_patterns {
            if content_lower.contains(&pattern.to_lowercase()) {
                return Err(SettingsError::SuspiciousContent(pattern.to_string()));
            }
        }

        // Validate JSON structure
        let parsed: serde_json::Value = serde_json::from_str(&content)?;
        if !parsed.is_object() && !parsed.is_array() {
            return Ok(false);
        }

        Ok(true)
    }

    /// Get a preview of what will be exported
    pub fn get_export_preview(&self) -> Result<ExportPreview> {
        let mut configs = Vec::new();
        let mut resources = Vec::new();
        let mut total_size = 0u64;

        // Preview config files
        if self.config_dir.exists() {
            for entry in fs::read_dir(&self.config_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let metadata = path.metadata()?;
                    let size = metadata.len();
                    total_size += size;

                    configs.push(ConfigPreview {
                        name: path.file_name().unwrap().to_string_lossy().to_string(),
                        size_kb: size as f64 / 1024.0,
                    });
                }
            }
        }

        // Preview resource files
        for (resource_name, resource_dir) in &self.additional_resource_dirs {
            if !resource_dir.exists() {
                continue;
            }

            for entry in walkdir::WalkDir::new(resource_dir) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let metadata = entry.metadata()?;
                    let size = metadata.len();
                    total_size += size;

                    let relative = entry.path().strip_prefix(resource_dir).unwrap();
                    resources.push(ResourcePreview {
                        category: resource_name.clone(),
                        name: relative.to_string_lossy().to_string(),
                        size_kb: size as f64 / 1024.0,
                    });
                }
            }
        }

        Ok(ExportPreview {
            configs,
            resources,
            total_size_kb: total_size as f64 / 1024.0,
        })
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Check if a path contains path traversal attempts
fn contains_path_traversal(path: &str) -> bool {
    let dangerous_patterns = ["..\\", "../", ".."];

    for pattern in &dangerous_patterns {
        if path.contains(pattern) {
            return true;
        }
    }

    // Check for absolute paths
    if path.starts_with('/') || (path.len() > 1 && path.chars().nth(1) == Some(':')) {
        return true;
    }

    false
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_traversal_detection() {
        assert!(contains_path_traversal("../etc/passwd"));
        assert!(contains_path_traversal("..\\windows\\system32"));
        assert!(contains_path_traversal("/etc/passwd"));
        assert!(!contains_path_traversal("config.json"));
    }

    #[test]
    fn test_version_compatibility() {
        let exporter = SettingsExporter::new("test");
        assert!(exporter.check_version_compatibility("1.0.0"));
        assert!(exporter.check_version_compatibility("1.5.2"));
        assert!(!exporter.check_version_compatibility("2.0.0"));
    }
}
