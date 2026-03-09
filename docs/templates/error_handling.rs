//! error_handling.rs
//!
//! Custom exception hierarchy and error collection utilities.
//!
//! This module provides:
//! - ApplicationError and variants for domain-specific errors
//! - ThreadSafeErrorCollector for multi-threaded error collection
//! - BatchErrorCollector for batch operation error tracking
//! - ErrorTracker for error history with querying

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

// =============================================================================
// CUSTOM ERROR HIERARCHY
// =============================================================================

/// Base error type for all application errors
#[derive(Debug, Clone)]
pub enum ApplicationError {
    /// Configuration-related errors (missing files, invalid values)
    Config { message: String, details: String },
    /// File system operation errors
    FileOperation { message: String, details: String },
    /// Network-related errors
    Network { message: String, details: String },
    /// Network timeout errors
    NetworkTimeout { message: String, timeout_seconds: f64 },
    /// Input validation errors
    Validation { message: String, field: String },
    /// Processing/business logic errors
    Processing { message: String, process_type: String },
    /// Authentication/authorization errors
    Authentication { message: String, details: String },
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config { message, details } => {
                if details.is_empty() {
                    write!(f, "Config error: {}", message)
                } else {
                    write!(f, "Config error: {}: {}", message, details)
                }
            }
            Self::FileOperation { message, details } => {
                if details.is_empty() {
                    write!(f, "File operation error: {}", message)
                } else {
                    write!(f, "File operation error: {}: {}", message, details)
                }
            }
            Self::Network { message, details } => {
                if details.is_empty() {
                    write!(f, "Network error: {}", message)
                } else {
                    write!(f, "Network error: {}: {}", message, details)
                }
            }
            Self::NetworkTimeout { message, timeout_seconds } => {
                write!(f, "{} (timeout: {}s)", message, timeout_seconds)
            }
            Self::Validation { message, field } => {
                if field.is_empty() {
                    write!(f, "Validation error: {}", message)
                } else {
                    write!(f, "Validation error [{}]: {}", field, message)
                }
            }
            Self::Processing { message, process_type } => {
                if process_type.is_empty() {
                    write!(f, "Processing error: {}", message)
                } else {
                    write!(f, "Processing error [{}]: {}", process_type, message)
                }
            }
            Self::Authentication { message, details } => {
                if details.is_empty() {
                    write!(f, "Authentication error: {}", message)
                } else {
                    write!(f, "Authentication error: {}: {}", message, details)
                }
            }
        }
    }
}

impl std::error::Error for ApplicationError {}

// =============================================================================
// ERROR COLLECTORS
// =============================================================================

/// Collect errors from multiple threads safely
pub struct ThreadSafeErrorCollector {
    errors: Arc<Mutex<Vec<(String, String)>>>,
}

impl ThreadSafeErrorCollector {
    /// Create a new error collector
    pub fn new() -> Self {
        Self {
            errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add an error with context
    pub fn add_error(&self, context: impl Into<String>, error: impl fmt::Display) {
        let mut errors = self.errors.lock().unwrap();
        errors.push((context.into(), error.to_string()));
    }

    /// Check if any errors were collected
    pub fn has_errors(&self) -> bool {
        !self.errors.lock().unwrap().is_empty()
    }

    /// Get all collected errors
    pub fn get_errors(&self) -> Vec<(String, String)> {
        self.errors.lock().unwrap().clone()
    }

    /// Get the number of errors
    pub fn get_error_count(&self) -> usize {
        self.errors.lock().unwrap().len()
    }

    /// Clear all errors
    pub fn clear(&self) {
        self.errors.lock().unwrap().clear();
    }
}

impl Default for ThreadSafeErrorCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Collect errors during batch operations
pub struct BatchErrorCollector {
    errors: HashMap<String, Vec<String>>,
}

impl BatchErrorCollector {
    /// Create a new batch error collector
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
        }
    }

    /// Add an error for an item
    pub fn add_error(&mut self, item_id: impl Into<String>, error: impl Into<String>) {
        self.errors
            .entry(item_id.into())
            .or_insert_with(Vec::new)
            .push(error.into());
    }

    /// Check if any errors were collected
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get total number of errors across all items
    pub fn get_error_count(&self) -> usize {
        self.errors.values().map(|v| v.len()).sum()
    }

    /// Get list of item IDs that had errors
    pub fn get_failed_items(&self) -> Vec<String> {
        self.errors.keys().cloned().collect()
    }

    /// Get errors for a specific item
    pub fn get_errors_for_item(&self, item_id: &str) -> Vec<String> {
        self.errors.get(item_id).cloned().unwrap_or_default()
    }

    /// Get a summary of errors
    pub fn get_summary(&self) -> String {
        if self.errors.is_empty() {
            return "No errors".to_string();
        }

        let item_count = self.errors.len();
        let error_count = self.get_error_count();
        format!("Failed: {} items with {} total errors", item_count, error_count)
    }

    /// Clear all errors
    pub fn clear(&mut self) {
        self.errors.clear();
    }
}

impl Default for BatchErrorCollector {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ERROR TRACKER
// =============================================================================

/// An error tracked with context and timestamp
#[derive(Debug, Clone)]
pub struct TrackedError {
    pub timestamp: DateTime<Utc>,
    pub error_type: String,
    pub message: String,
    pub context: String,
    pub details: HashMap<String, String>,
}

/// Track error history with querying capabilities
pub struct ErrorTracker {
    max_history: usize,
    errors: Arc<Mutex<Vec<TrackedError>>>,
}

impl ErrorTracker {
    /// Create a new error tracker
    pub fn new(max_history: usize) -> Self {
        Self {
            max_history,
            errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Track an error
    pub fn track(
        &self,
        error: &dyn std::error::Error,
        context: impl Into<String>,
        details: Option<HashMap<String, String>>,
    ) {
        let tracked = TrackedError {
            timestamp: Utc::now(),
            error_type: std::any::type_name_of_val(error).to_string(),
            message: error.to_string(),
            context: context.into(),
            details: details.unwrap_or_default(),
        };

        let mut errors = self.errors.lock().unwrap();
        errors.push(tracked);
        if errors.len() > self.max_history {
            errors.remove(0);
        }
    }

    /// Get most recent errors
    pub fn get_recent_errors(&self, count: usize) -> Vec<TrackedError> {
        let errors = self.errors.lock().unwrap();
        let start = if errors.len() > count {
            errors.len() - count
        } else {
            0
        };
        errors[start..].to_vec()
    }

    /// Get errors of a specific type
    pub fn get_errors_by_type(&self, error_type: &str) -> Vec<TrackedError> {
        let errors = self.errors.lock().unwrap();
        errors
            .iter()
            .filter(|e| e.error_type == error_type)
            .cloned()
            .collect()
    }

    /// Get errors with matching context (case-insensitive)
    pub fn get_errors_by_context(&self, context: &str) -> Vec<TrackedError> {
        let context_lower = context.to_lowercase();
        let errors = self.errors.lock().unwrap();
        errors
            .iter()
            .filter(|e| e.context.to_lowercase().contains(&context_lower))
            .cloned()
            .collect()
    }

    /// Get error counts by type
    pub fn get_error_summary(&self) -> HashMap<String, usize> {
        let errors = self.errors.lock().unwrap();
        let mut counts = HashMap::new();
        for error in errors.iter() {
            *counts.entry(error.error_type.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Clear error history
    pub fn clear(&self) {
        self.errors.lock().unwrap().clear();
    }
}

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

/// Execute a function, returning default on error
pub fn safe_execute<T, F>(func: F, default: T) -> T
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>>,
{
    func().unwrap_or(default)
}

/// Convert an error to a user-friendly message
pub fn user_friendly_message(error: &dyn std::error::Error) -> String {
    let error_str = error.to_string();

    if error_str.contains("Connection") {
        "Unable to connect to the server. Please check your internet connection.".to_string()
    } else if error_str.contains("timeout") {
        "The operation timed out. Please try again.".to_string()
    } else if error_str.contains("not found") {
        format!("File not found: {}", error_str)
    } else if error_str.contains("Permission denied") {
        "Permission denied. Please check file permissions.".to_string()
    } else {
        format!("An error occurred: {}", error_str)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_safe_collector() {
        let collector = ThreadSafeErrorCollector::new();
        collector.add_error("task1", "Error message");
        assert!(collector.has_errors());
        assert_eq!(collector.get_error_count(), 1);

        let errors = collector.get_errors();
        assert_eq!(errors[0].0, "task1");
    }

    #[test]
    fn test_batch_collector() {
        let mut collector = BatchErrorCollector::new();
        collector.add_error("item1", "Error 1");
        collector.add_error("item1", "Error 2");
        collector.add_error("item2", "Error 3");

        assert_eq!(collector.get_error_count(), 3);
        assert_eq!(collector.get_failed_items().len(), 2);
        assert_eq!(collector.get_errors_for_item("item1").len(), 2);
    }

    #[test]
    fn test_error_tracker() {
        let tracker = ErrorTracker::new(50);
        let error = ApplicationError::Network {
            message: "Connection failed".to_string(),
            details: String::new(),
        };

        tracker.track(&error, "Testing", None);
        let recent = tracker.get_recent_errors(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "Network error: Connection failed");
    }
}
