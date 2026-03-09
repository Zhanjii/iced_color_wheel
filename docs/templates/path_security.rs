//! path_security.rs
//!
//! Path security utilities to prevent path traversal attacks.
//!
//! Provides functions to sanitize and validate file paths to prevent
//! path traversal attacks and other path-related security issues.
//!
//! # Usage
//!
//! ```rust
//! use path_security::{sanitize_filename, safe_join, is_safe_path};
//!
//! // Sanitize user input
//! let safe_name = sanitize_filename("../../../etc/passwd");
//!
//! // Safe path joining
//! let safe_path = safe_join("/base/dir", &["user_input", "file.txt"]).unwrap();
//! ```

use regex::Regex;
use std::path::{Path, PathBuf};
use std::fmt;

// =============================================================================
// ERRORS
// =============================================================================

/// Path security error
#[derive(Debug)]
pub struct PathSecurityError {
    message: String,
}

impl fmt::Display for PathSecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Path security error: {}", self.message)
    }
}

impl std::error::Error for PathSecurityError {}

impl PathSecurityError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

// =============================================================================
// FILENAME SANITIZATION
// =============================================================================

/// Sanitize a filename by removing potentially dangerous characters
///
/// Removes or replaces:
/// - Path separators (/ and \)
/// - Parent directory references (..)
/// - Null bytes
/// - Control characters
/// - Windows reserved characters
pub fn sanitize_filename(filename: &str) -> String {
    if filename.is_empty() {
        return String::new();
    }

    let mut result = filename.to_string();

    // Remove null bytes
    result = result.replace('\0', "");

    // Remove path separators
    result = result.replace('/', "_").replace('\\', "_");

    // Remove parent directory references
    result = result.replace("..", "_");

    // Remove control characters (0x00-0x1F and 0x7F)
    let control_chars = Regex::new(r"[\x00-\x1f\x7f]").unwrap();
    result = control_chars.replace_all(&result, "").to_string();

    // Remove Windows reserved characters: < > : " | ? *
    let reserved = Regex::new(r#"[<>:"|?*]"#).unwrap();
    result = reserved.replace_all(&result, "_").to_string();

    // Remove leading/trailing whitespace and dots
    result = result.trim_matches(|c| c == '.' || c == ' ' || c == '\t' || c == '\n' || c == '\r').to_string();

    // Prevent Windows reserved names
    let reserved_names = [
        "CON", "PRN", "AUX", "NUL",
        "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let name_without_ext = result.split('.').next().unwrap_or("").to_uppercase();
    if reserved_names.contains(&name_without_ext.as_str()) {
        result = format!("_{}", result);
    }

    result
}

/// Sanitize a single path component (directory or file name)
pub fn sanitize_path_component(component: &str) -> String {
    sanitize_filename(component)
}

// =============================================================================
// PATH VALIDATION
// =============================================================================

/// Check if a path is safely within a base directory
///
/// Prevents path traversal attacks by ensuring the resolved path
/// is actually within the allowed base directory.
pub fn is_safe_path<P: AsRef<Path>, B: AsRef<Path>>(path: P, base_dir: B) -> bool {
    let base_path = match base_dir.as_ref().canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let target_path = base_path.join(path.as_ref());
    let resolved_path = match target_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // If path doesn't exist, check if its parent does
            if let Some(parent) = target_path.parent() {
                if let Ok(p) = parent.canonicalize() {
                    p.join(target_path.file_name().unwrap_or_default())
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
    };

    resolved_path.starts_with(&base_path)
}

/// Safely join path components to a base directory
///
/// Sanitizes each component and validates the result is within
/// the base directory.
pub fn safe_join<P: AsRef<Path>>(base_dir: P, components: &[&str]) -> Result<PathBuf, PathSecurityError> {
    let base_path = base_dir.as_ref().canonicalize()
        .map_err(|e| PathSecurityError::new(format!("Invalid base directory: {}", e)))?;

    let mut result = base_path.clone();
    for component in components {
        if !component.is_empty() {
            let sanitized = sanitize_path_component(component);
            result = result.join(sanitized);
        }
    }

    // Validate the result is still within base directory
    if let Ok(resolved) = result.canonicalize() {
        if !resolved.starts_with(&base_path) {
            return Err(PathSecurityError::new(
                "Path traversal detected: resulting path is outside base directory"
            ));
        }
    }

    Ok(result)
}

/// Validate and sanitize user-provided path input
pub fn validate_path_input<P: AsRef<Path>>(
    path_input: &str,
    base_dir: P,
    must_exist: bool,
) -> Result<PathBuf, PathSecurityError> {
    if path_input.is_empty() {
        return Err(PathSecurityError::new("Path cannot be empty"));
    }

    // Remove null bytes and control characters
    let control_chars = Regex::new(r"[\x00-\x1f\x7f]").unwrap();
    let clean_path = control_chars.replace_all(path_input, "");

    let input_path = Path::new(clean_path.as_ref());

    // Reject absolute paths
    if input_path.is_absolute() {
        return Err(PathSecurityError::new("Absolute paths not allowed"));
    }

    let base_resolved = base_dir.as_ref().canonicalize()
        .map_err(|e| PathSecurityError::new(format!("Invalid base directory: {}", e)))?;

    let resolved = base_resolved.join(input_path);

    // Check for path traversal
    if let Ok(canonical) = resolved.canonicalize() {
        if !canonical.starts_with(&base_resolved) {
            return Err(PathSecurityError::new(
                "Path traversal detected: path escapes base directory"
            ));
        }

        if must_exist && !canonical.exists() {
            return Err(PathSecurityError::new("Path does not exist"));
        }

        Ok(canonical)
    } else {
        if must_exist {
            return Err(PathSecurityError::new("Path does not exist"));
        }
        Ok(resolved)
    }
}

/// Check if a path string contains path traversal sequences
pub fn contains_path_traversal(path_string: &str) -> bool {
    let traversal_patterns = [
        "..",
        "..\\",
        "../",
        "%2e%2e",
        "%252e%252e",
        "....\\",
        "....//",
    ];

    let path_lower = path_string.to_lowercase();
    for pattern in &traversal_patterns {
        if path_lower.contains(pattern) {
            return true;
        }
    }

    path_string.contains('\0')
}

// =============================================================================
// TEMP FILE UTILITIES
// =============================================================================

/// Get a safe temporary file path
pub fn get_safe_temp_path(prefix: &str, suffix: &str) -> PathBuf {
    let safe_prefix = if prefix.is_empty() {
        "app_"
    } else {
        prefix
    };

    let safe_suffix = sanitize_filename(suffix);

    let temp_dir = std::env::temp_dir();
    let filename = format!("{}{}", sanitize_filename(safe_prefix), safe_suffix);
    temp_dir.join(filename)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal.txt"), "normal.txt");
        assert_eq!(sanitize_filename("../../../etc/passwd"), "______etc_passwd");
        assert_eq!(sanitize_filename("file<>name"), "file__name");
        assert!(!sanitize_filename("CON").starts_with("CON"));
    }

    #[test]
    fn test_is_safe_path() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        assert!(is_safe_path("safe/file.txt", base));
        assert!(!is_safe_path("../../../etc/passwd", base));
    }

    #[test]
    fn test_safe_join() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        let result = safe_join(base, &["user", "documents", "file.txt"]);
        assert!(result.is_ok());

        let result = safe_join(base, &["..", "..", "etc", "passwd"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_contains_path_traversal() {
        assert!(contains_path_traversal("../etc/passwd"));
        assert!(contains_path_traversal("..\\windows\\system32"));
        assert!(contains_path_traversal("file\x00name"));
        assert!(!contains_path_traversal("normal/path"));
    }
}
