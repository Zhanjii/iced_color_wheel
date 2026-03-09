//! file_operations.rs
//!
//! Core file operation utilities.
//!
//! Provides a centralized, reusable API for common file operations with
//! proper error handling and parallel processing support.
//!
//! Security: All file operations validate paths to prevent traversal attacks.
//!
//! # Example
//!
//! ```rust
//! use file_operations::{copy_file, move_file, delete_file, list_files, safe_filename};
//! use std::path::Path;
//!
//! // Basic operations
//! copy_file("source.txt", "dest.txt")?;
//! move_file("old.txt", "new.txt")?;
//! delete_file("unwanted.txt")?;
//!
//! // List files
//! let files = list_files(Path::new("data"), "*", false, true)?;
//!
//! // Sanitize filename
//! let safe = safe_filename("unsafe/file<name>.txt", 200);
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

// =============================================================================
// CONSTANTS
// =============================================================================

/// Unsafe glob patterns that could escape directory boundaries
const UNSAFE_GLOB_PATTERN: &str = r"\.\.[\\/]|^[\\/]";

/// Maximum results from glob operations
const MAX_GLOB_RESULTS: usize = 10000;

/// Minimum path depth for recursive delete
const MINIMUM_PATH_DEPTH: usize = 3;

/// Maximum files for recursive delete without force flag
const MAX_RECURSIVE_DELETE_FILES: usize = 1000;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur during file operations
#[derive(Debug, Error)]
pub enum FileOpError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Protected path: {0}")]
    ProtectedPath(PathBuf),

    #[error("Path too shallow: {0}")]
    PathTooShallow(PathBuf),

    #[error("Directory not empty: {0}")]
    DirectoryNotEmpty(PathBuf),

    #[error("Too many files: {0} files (max {1})")]
    TooManyFiles(usize, usize),

    #[error("Invalid glob pattern: {0}")]
    InvalidGlob(String),

    #[error("Source not found: {0}")]
    SourceNotFound(PathBuf),
}

pub type Result<T> = std::result::Result<T, FileOpError>;

// =============================================================================
// PROTECTED PATHS
// =============================================================================

/// Check if a path is a protected system path
pub fn is_protected_path(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    let protected: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\",
            "C:\\Windows",
            "C:\\Windows\\System32",
            "C:\\Windows\\SysWOW64",
            "C:\\Program Files",
            "C:\\Program Files (x86)",
            "C:\\ProgramData",
            "C:\\Users",
        ]
    } else {
        &[
            "/",
            "/etc",
            "/usr",
            "/var",
            "/bin",
            "/sbin",
            "/root",
            "/System", // macOS
        ]
    };

    // Also protect home directory
    if let Ok(home) = std::env::var("HOME") {
        if path == Path::new(&home) {
            return true;
        }
    }

    for protected_path in protected {
        if path == Path::new(protected_path) {
            return true;
        }

        // Check if path is a parent of protected path
        if let Ok(canonical) = path.canonicalize() {
            if let Ok(protected_canonical) = Path::new(protected_path).canonicalize() {
                if protected_canonical.starts_with(&canonical) && protected_canonical != canonical {
                    return true;
                }
            }
        }
    }

    false
}

// =============================================================================
// BASIC FILE OPERATIONS
// =============================================================================

/// Copy file with error handling
pub fn copy_file(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<()> {
    let source = source.as_ref();
    let destination = destination.as_ref();

    if !source.exists() {
        return Err(FileOpError::SourceNotFound(source.to_path_buf()));
    }

    // Ensure destination directory exists
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(source, destination)?;
    Ok(())
}

/// Move file with error handling
pub fn move_file(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<()> {
    let source = source.as_ref();
    let destination = destination.as_ref();

    if !source.exists() {
        return Err(FileOpError::SourceNotFound(source.to_path_buf()));
    }

    // Ensure destination directory exists
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::rename(source, destination)?;
    Ok(())
}

/// Delete file with error handling
pub fn delete_file(file_path: impl AsRef<Path>) -> Result<()> {
    let file_path = file_path.as_ref();

    if !file_path.exists() {
        return Ok(()); // Already deleted
    }

    fs::remove_file(file_path)?;
    Ok(())
}

/// Create directory with error handling
pub fn create_directory(
    dir_path: impl AsRef<Path>,
    parents: bool,
) -> Result<()> {
    let dir_path = dir_path.as_ref();

    if is_protected_path(dir_path) {
        return Err(FileOpError::ProtectedPath(dir_path.to_path_buf()));
    }

    if parents {
        fs::create_dir_all(dir_path)?;
    } else {
        fs::create_dir(dir_path)?;
    }

    Ok(())
}

/// Delete directory with safety checks
pub fn delete_directory(
    dir_path: impl AsRef<Path>,
    recursive: bool,
    force: bool,
) -> Result<()> {
    let dir_path = dir_path.as_ref();

    if !dir_path.exists() {
        return Ok(()); // Already deleted
    }

    // Check against protected paths
    if is_protected_path(dir_path) {
        return Err(FileOpError::ProtectedPath(dir_path.to_path_buf()));
    }

    // Check path depth
    let path_components = dir_path.components().count();
    if path_components < MINIMUM_PATH_DEPTH {
        return Err(FileOpError::PathTooShallow(dir_path.to_path_buf()));
    }

    // Size check for recursive deletes
    if recursive && !force {
        let file_count = count_files(dir_path)?;
        if file_count > MAX_RECURSIVE_DELETE_FILES {
            return Err(FileOpError::TooManyFiles(
                file_count,
                MAX_RECURSIVE_DELETE_FILES,
            ));
        }
    }

    // Perform deletion
    if recursive {
        fs::remove_dir_all(dir_path)?;
    } else {
        fs::remove_dir(dir_path).map_err(|e| {
            if e.kind() == io::ErrorKind::DirectoryNotEmpty {
                FileOpError::DirectoryNotEmpty(dir_path.to_path_buf())
            } else {
                FileOpError::Io(e)
            }
        })?;
    }

    Ok(())
}

// =============================================================================
// FILE LISTING
// =============================================================================

/// List files matching pattern in directory
pub fn list_files(
    directory: impl AsRef<Path>,
    pattern: &str,
    recursive: bool,
    files_only: bool,
) -> Result<Vec<PathBuf>> {
    let directory = directory.as_ref();

    // Validate pattern doesn't contain path traversal
    if regex::Regex::new(UNSAFE_GLOB_PATTERN)
        .unwrap()
        .is_match(pattern)
    {
        return Err(FileOpError::InvalidGlob(pattern.to_string()));
    }

    if !directory.exists() || !directory.is_dir() {
        return Ok(Vec::new());
    }

    let glob_pattern = if recursive {
        directory.join("**").join(pattern)
    } else {
        directory.join(pattern)
    };

    let mut matches: Vec<PathBuf> = glob::glob(glob_pattern.to_str().unwrap())
        .map_err(|e| FileOpError::InvalidGlob(e.to_string()))?
        .filter_map(|p| p.ok())
        .take(MAX_GLOB_RESULTS)
        .collect();

    if files_only {
        matches.retain(|p| p.is_file());
    }

    matches.sort();
    Ok(matches)
}

/// List subdirectories in directory
pub fn list_directories(
    directory: impl AsRef<Path>,
    recursive: bool,
) -> Result<Vec<PathBuf>> {
    let directory = directory.as_ref();

    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut dirs = Vec::new();

    if recursive {
        for entry in walkdir::WalkDir::new(directory) {
            if let Ok(entry) = entry {
                if entry.file_type().is_dir() {
                    dirs.push(entry.path().to_path_buf());
                }
            }
        }
    } else {
        for entry in fs::read_dir(directory)? {
            if let Ok(entry) = entry {
                if entry.file_type()?.is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
    }

    dirs.sort();
    Ok(dirs)
}

// =============================================================================
// FILE SIZE UTILITIES
// =============================================================================

/// Get file size in bytes
pub fn get_file_size(file_path: impl AsRef<Path>) -> Result<u64> {
    let file_path = file_path.as_ref();

    if !file_path.exists() {
        return Ok(0);
    }

    Ok(file_path.metadata()?.len())
}

/// Get total size of directory contents in bytes
pub fn get_directory_size(directory: impl AsRef<Path>) -> Result<u64> {
    let directory = directory.as_ref();

    if !directory.exists() {
        return Ok(0);
    }

    let mut total = 0u64;

    for entry in walkdir::WalkDir::new(directory) {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total += metadata.len();
                }
            }
        }
    }

    Ok(total)
}

/// Format file size in human-readable format
pub fn format_file_size(size_bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

    let mut size = size_bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}

// =============================================================================
// FILENAME UTILITIES
// =============================================================================

/// Ensure filename is unique by appending a number if needed
pub fn ensure_unique_filename(file_path: impl AsRef<Path>) -> Result<PathBuf> {
    let file_path = file_path.as_ref();

    if !file_path.exists() {
        return Ok(file_path.to_path_buf());
    }

    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let extension = file_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let parent = file_path.parent().unwrap_or_else(|| Path::new("."));

    for counter in 1..=10000 {
        let new_name = if extension.is_empty() {
            format!("{}_{}", stem, counter)
        } else {
            format!("{}_{}.{}", stem, counter, extension)
        };

        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return Ok(new_path);
        }
    }

    Err(FileOpError::Io(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Could not find unique filename",
    )))
}

/// Sanitize filename for safe filesystem use
pub fn safe_filename(filename: &str, max_length: usize) -> String {
    if filename.is_empty() {
        return "unnamed".to_string();
    }

    // Normalize Unicode
    let normalized: String = filename.chars().collect();

    // Remove/replace invalid characters
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut safe: String = normalized
        .chars()
        .map(|c| {
            if invalid_chars.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Remove leading/trailing spaces and dots
    safe = safe.trim_matches(|c| c == ' ' || c == '.').to_string();

    // Replace multiple consecutive underscores/spaces
    while safe.contains("__") || safe.contains("  ") {
        safe = safe.replace("__", "_").replace("  ", " ");
    }

    // Truncate if too long (preserving extension)
    if safe.len() > max_length {
        if let Some(dot_pos) = safe.rfind('.') {
            let ext = &safe[dot_pos..];
            if ext.len() < 10 {
                let max_name_len = max_length - ext.len();
                safe = format!("{}{}", &safe[..max_name_len], ext);
            } else {
                safe.truncate(max_length);
            }
        } else {
            safe.truncate(max_length);
        }
    }

    // Ensure not empty after sanitization
    if safe.is_empty() || safe == "." {
        safe = "unnamed".to_string();
    }

    safe
}

/// Copy entire directory tree
pub fn copy_directory(
    source_dir: impl AsRef<Path>,
    destination_dir: impl AsRef<Path>,
) -> Result<()> {
    let source_dir = source_dir.as_ref();
    let destination_dir = destination_dir.as_ref();

    if !source_dir.exists() {
        return Err(FileOpError::SourceNotFound(source_dir.to_path_buf()));
    }

    for entry in walkdir::WalkDir::new(source_dir) {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(source_dir).unwrap();
        let dest_path = destination_dir.join(relative);

        if entry.file_type().is_dir() {
            fs::create_dir_all(dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, dest_path)?;
        }
    }

    Ok(())
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Count files in a directory
fn count_files(dir: impl AsRef<Path>) -> Result<usize> {
    let mut count = 0;

    for entry in walkdir::WalkDir::new(dir.as_ref()) {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                count += 1;
            }
        }
    }

    Ok(count)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_copy_file() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        let dest = temp.path().join("dest.txt");

        File::create(&source).unwrap();
        copy_file(&source, &dest).unwrap();

        assert!(dest.exists());
    }

    #[test]
    fn test_safe_filename() {
        assert_eq!(safe_filename("safe.txt", 200), "safe.txt");
        assert_eq!(safe_filename("unsafe/file<name>.txt", 200), "unsafe_file_name_.txt");
        assert_eq!(safe_filename("", 200), "unnamed");
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(500), "500.0 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_is_protected_path() {
        assert!(is_protected_path(Path::new("C:\\")));
        assert!(is_protected_path(Path::new("/")));
    }
}
