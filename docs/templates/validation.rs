//! validation.rs
//!
//! Validation utilities for user inputs and form data.
//!
//! Provides functions for validating user inputs, form data,
//! and a standardized validation framework.
//!
//! # Usage
//!
//! ```rust
//! use validation::{validate_email, validate_required, FormValidator};
//!
//! // Individual validators
//! let result = validate_email("user@example.com", "Email");
//! assert!(result.is_ok());
//!
//! // Form validation
//! let mut validator = FormValidator::new();
//! validator.add_field("email", "user@example.com");
//! ```

use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

// =============================================================================
// TYPE DEFINITIONS
// =============================================================================

/// Result type for validation operations
pub type ValidationResult = Result<(), String>;

// =============================================================================
// INDIVIDUAL VALIDATORS
// =============================================================================

/// Validate that a value is not empty
pub fn validate_required<T: AsRef<str>>(value: T, field_name: &str) -> ValidationResult {
    let val = value.as_ref();
    if val.is_empty() {
        Err(format!("{} is required", field_name))
    } else {
        Ok(())
    }
}

/// Validate string length
pub fn validate_length(
    value: &str,
    min_length: usize,
    max_length: Option<usize>,
    field_name: &str,
) -> ValidationResult {
    if value.len() < min_length {
        return Err(format!(
            "{} must be at least {} characters",
            field_name, min_length
        ));
    }

    if let Some(max) = max_length {
        if value.len() > max {
            return Err(format!(
                "{} cannot exceed {} characters",
                field_name, max
            ));
        }
    }

    Ok(())
}

/// Validate that a path exists
pub fn validate_path_exists(path: &str, field_name: &str) -> ValidationResult {
    if path.is_empty() {
        return Err(format!("{} cannot be empty", field_name));
    }

    if !Path::new(path).exists() {
        return Err(format!("{} does not exist: {}", field_name, path));
    }

    Ok(())
}

/// Validate that a directory exists
pub fn validate_directory_exists(path: &str, field_name: &str) -> ValidationResult {
    validate_path_exists(path, field_name)?;

    if !Path::new(path).is_dir() {
        return Err(format!("{} is not a directory: {}", field_name, path));
    }

    Ok(())
}

/// Validate that a file exists
pub fn validate_file_exists(path: &str, field_name: &str) -> ValidationResult {
    validate_path_exists(path, field_name)?;

    if !Path::new(path).is_file() {
        return Err(format!("{} is not a file: {}", field_name, path));
    }

    Ok(())
}

/// Validate a number is within range
pub fn validate_number_range<T: PartialOrd + std::fmt::Display>(
    value: T,
    min_value: Option<T>,
    max_value: Option<T>,
    field_name: &str,
) -> ValidationResult {
    if let Some(min) = min_value {
        if value < min {
            return Err(format!("{} must be at least {}", field_name, min));
        }
    }

    if let Some(max) = max_value {
        if value > max {
            return Err(format!("{} cannot exceed {}", field_name, max));
        }
    }

    Ok(())
}

/// Validate a URL
pub fn validate_url(url: &str, field_name: &str, require_https: bool) -> ValidationResult {
    if url.is_empty() {
        return Err(format!("{} cannot be empty", field_name));
    }

    let parsed = url::Url::parse(url)
        .map_err(|_| format!("{} is not a valid URL: {}", field_name, url))?;

    if require_https && parsed.scheme() != "https" {
        return Err(format!("{} must use HTTPS protocol", field_name));
    }

    Ok(())
}

/// Validate an email address
pub fn validate_email(email: &str, field_name: &str) -> ValidationResult {
    if email.is_empty() {
        return Err(format!("{} cannot be empty", field_name));
    }

    let pattern = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();

    if !pattern.is_match(email) {
        return Err(format!(
            "{} is not a valid email address: {}",
            field_name, email
        ));
    }

    Ok(())
}

// =============================================================================
// HEX COLOR VALIDATION
// =============================================================================

/// Check if a string is a valid 6-digit hex color code
pub fn is_valid_hex_color(color: &str) -> bool {
    let pattern = Regex::new(r"^#[0-9A-Fa-f]{6}$").unwrap();
    pattern.is_match(color)
}

/// Validate a hex color code
pub fn validate_hex_color(color: &str, field_name: &str) -> ValidationResult {
    if color.is_empty() {
        return Err(format!("{} cannot be empty", field_name));
    }

    if !is_valid_hex_color(color) {
        return Err(format!(
            "{} must be a valid hex color (e.g., #FF0000)",
            field_name
        ));
    }

    Ok(())
}

/// Normalize a hex color to uppercase with # prefix
pub fn normalize_hex_color(color: &str) -> Result<String, String> {
    if color.is_empty() {
        return Err("Color cannot be empty".to_string());
    }

    let normalized = if color.starts_with('#') {
        color.to_string()
    } else {
        format!("#{}", color)
    };

    if !is_valid_hex_color(&normalized) {
        return Err(format!("Invalid hex color: {}", color));
    }

    Ok(normalized.to_uppercase())
}

// =============================================================================
// FILENAME VALIDATION
// =============================================================================

/// Validate a filename (no path)
pub fn validate_filename(filename: &str, field_name: &str) -> ValidationResult {
    if filename.is_empty() {
        return Err(format!("{} cannot be empty", field_name));
    }

    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    for ch in invalid_chars {
        if filename.contains(ch) {
            return Err(format!(
                "{} contains invalid character: '{}'",
                field_name, ch
            ));
        }
    }

    Ok(())
}

// =============================================================================
// FORM VALIDATOR
// =============================================================================

/// Form validation helper for UI forms
pub struct FormValidator {
    errors: HashMap<String, String>,
    fields: HashMap<String, String>,
}

impl FormValidator {
    /// Create a new form validator
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    /// Add a field to validate
    pub fn add_field(&mut self, field_name: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(field_name.into(), value.into());
    }

    /// Validate a field using the provided validators
    pub fn validate<F>(&mut self, field_name: &str, validators: Vec<F>) -> bool
    where
        F: Fn(&str, &str) -> ValidationResult,
    {
        let value = match self.fields.get(field_name) {
            Some(v) => v,
            None => {
                self.errors
                    .insert(field_name.to_string(), "Field not found".to_string());
                return false;
            }
        };

        for validator in validators {
            if let Err(error) = validator(value, field_name) {
                self.errors.insert(field_name.to_string(), error);
                return false;
            }
        }

        true
    }

    /// Check if all fields are valid
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get all validation errors
    pub fn get_errors(&self) -> &HashMap<String, String> {
        &self.errors
    }

    /// Get the error message for a specific field
    pub fn get_error_message(&self, field_name: &str) -> Option<&String> {
        self.errors.get(field_name)
    }

    /// Get a summary of all validation errors
    pub fn get_error_summary(&self) -> String {
        if self.errors.is_empty() {
            return "No validation errors".to_string();
        }

        let mut summary = String::from("Validation errors:\n");
        for (field, error) in &self.errors {
            summary.push_str(&format!("- {}: {}\n", field, error));
        }

        summary.trim().to_string()
    }

    /// Clear all fields and errors
    pub fn clear(&mut self) {
        self.fields.clear();
        self.errors.clear();
    }
}

impl Default for FormValidator {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_required() {
        assert!(validate_required("value", "Field").is_ok());
        assert!(validate_required("", "Field").is_err());
    }

    #[test]
    fn test_validate_length() {
        assert!(validate_length("hello", 3, Some(10), "Field").is_ok());
        assert!(validate_length("hi", 3, Some(10), "Field").is_err());
        assert!(validate_length("hello world!", 3, Some(10), "Field").is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("user@example.com", "Email").is_ok());
        assert!(validate_email("invalid", "Email").is_err());
        assert!(validate_email("", "Email").is_err());
    }

    #[test]
    fn test_hex_color_validation() {
        assert!(is_valid_hex_color("#FF0000"));
        assert!(is_valid_hex_color("#ffffff"));
        assert!(!is_valid_hex_color("#FFF"));
        assert!(!is_valid_hex_color("FF0000"));

        assert!(validate_hex_color("#FF0000", "Color").is_ok());
        assert!(validate_hex_color("#FFF", "Color").is_err());
    }

    #[test]
    fn test_normalize_hex_color() {
        assert_eq!(normalize_hex_color("#ff0000").unwrap(), "#FF0000");
        assert_eq!(normalize_hex_color("ff0000").unwrap(), "#FF0000");
        assert!(normalize_hex_color("invalid").is_err());
    }

    #[test]
    fn test_form_validator() {
        let mut validator = FormValidator::new();
        validator.add_field("email", "user@example.com");

        let is_valid = validator.validate(
            "email",
            vec![
                |v, n| validate_required(v, n),
                |v, n| validate_email(v, n),
            ],
        );

        assert!(is_valid);
        assert!(validator.is_valid());
    }
}
