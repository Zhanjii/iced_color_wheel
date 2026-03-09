//! data_models.rs
//!
//! Data models and type definitions for the application.
//!
//! Provides structs, enums, and type configurations for structured data.
//!
//! This module demonstrates patterns for:
//! - Enum-based configuration types
//! - Struct configurations with defaults
//! - Type registries for extensible systems
//! - Serialization/deserialization patterns
//!
//! # Usage
//!
//! ```rust
//! use data_models::{APIAuthType, APITypeConfig, ProjectMetadata};
//!
//! // Get API configuration
//! let config = get_api_config("openai");
//! if let Some(cfg) = config {
//!     println!("Auth type: {:?}", cfg.auth_type);
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// ENUMS
// =============================================================================

/// Authentication types for API integrations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum APIAuthType {
    BearerToken,
    ApiKeySecret,
    OAuthToken,
    CustomHeaders,
    GoogleOAuth2,
    BasicAuth,
}

/// Types of projects supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    Default,
    Template,
    Archive,
}

impl Default for ProjectType {
    fn default() -> Self {
        Self::Default
    }
}

/// Status of a processing operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

// =============================================================================
// API TYPE CONFIGURATION
// =============================================================================

/// Configuration for different API types
///
/// Defines how to authenticate and test connections for various APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APITypeConfig {
    /// Human-readable name for the API
    pub name: String,
    /// Authentication method to use
    pub auth_type: APIAuthType,
    /// URL to test the connection
    pub test_endpoint: String,
    /// HTTP method for test request
    #[serde(default = "default_test_method")]
    pub test_method: String,
    /// Optional request body for test
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_body: Option<serde_json::Value>,
    /// Fields that must be provided
    #[serde(default)]
    pub required_fields: Vec<String>,
    /// Fields that can be provided
    #[serde(default)]
    pub optional_fields: Vec<String>,
    /// Name of custom test function if needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_test_function: Option<String>,
}

fn default_test_method() -> String {
    "GET".to_string()
}

impl APITypeConfig {
    /// Create a new API type configuration
    pub fn new(name: impl Into<String>, auth_type: APIAuthType, test_endpoint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            auth_type,
            test_endpoint: test_endpoint.into(),
            test_method: default_test_method(),
            test_body: None,
            required_fields: Vec::new(),
            optional_fields: Vec::new(),
            custom_test_function: None,
        }
    }

    /// Add required fields
    pub fn with_required_fields(mut self, fields: Vec<String>) -> Self {
        self.required_fields = fields;
        self
    }

    /// Add optional fields
    pub fn with_optional_fields(mut self, fields: Vec<String>) -> Self {
        self.optional_fields = fields;
        self
    }
}

/// Get API configuration by type name
pub fn get_api_config(api_type: &str) -> Option<APITypeConfig> {
    api_types().get(api_type).cloned()
}

/// Get the registry of all API types
pub fn api_types() -> HashMap<String, APITypeConfig> {
    let mut types = HashMap::new();

    types.insert(
        "openai".to_string(),
        APITypeConfig::new("OpenAI", APIAuthType::BearerToken, "https://api.openai.com/v1/models")
            .with_required_fields(vec!["api_key".to_string()]),
    );

    types.insert(
        "anthropic".to_string(),
        APITypeConfig {
            name: "Anthropic".to_string(),
            auth_type: APIAuthType::BearerToken,
            test_endpoint: "https://api.anthropic.com/v1/messages".to_string(),
            test_method: "POST".to_string(),
            test_body: Some(serde_json::json!({
                "model": "claude-3-sonnet-20240229",
                "messages": [{"role": "user", "content": "Hi"}],
                "max_tokens": 1
            })),
            required_fields: vec!["api_key".to_string()],
            optional_fields: Vec::new(),
            custom_test_function: None,
        },
    );

    types.insert(
        "github".to_string(),
        APITypeConfig::new("GitHub API", APIAuthType::BearerToken, "https://api.github.com/user")
            .with_required_fields(vec!["api_key".to_string()]),
    );

    types
}

// =============================================================================
// PROJECT METADATA
// =============================================================================

/// Metadata for a project
///
/// Stores project information with automatic timestamp tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Project name
    pub name: String,
    /// Project description
    #[serde(default)]
    pub description: String,
    /// Type of project
    #[serde(default)]
    pub project_type: ProjectType,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modification timestamp
    pub modified_at: DateTime<Utc>,
    /// Project version string
    #[serde(default = "default_version")]
    pub version: String,
    /// List of tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Dictionary for custom fields
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

impl ProjectMetadata {
    /// Create new project metadata
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            name: name.into(),
            description: String::new(),
            project_type: ProjectType::default(),
            created_at: now,
            modified_at: now,
            version: default_version(),
            tags: Vec::new(),
            custom: HashMap::new(),
        }
    }

    /// Update the modified timestamp
    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
    }
}

// =============================================================================
// PROCESSING RESULT
// =============================================================================

/// Result of a processing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult<T = serde_json::Value> {
    /// Whether the operation succeeded
    pub success: bool,
    /// Status message
    #[serde(default)]
    pub message: String,
    /// Optional result data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Processing duration in milliseconds
    #[serde(default)]
    pub duration_ms: f64,
}

impl<T> ProcessingResult<T> {
    /// Create a successful result
    pub fn ok(message: impl Into<String>, data: T) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
            error: None,
            duration_ms: 0.0,
        }
    }

    /// Create a failed result
    pub fn fail(error: impl Into<String>) -> Self {
        Self {
            success: false,
            message: "Failed".to_string(),
            data: None,
            error: Some(error.into()),
            duration_ms: 0.0,
        }
    }
}

// =============================================================================
// CONFIGURATION ITEM
// =============================================================================

/// A configuration item with validation
#[derive(Debug, Clone)]
pub struct ConfigItem<T> {
    /// Configuration key
    pub key: String,
    /// Current value
    pub value: T,
    /// Default value
    pub default: T,
    /// Human-readable description
    pub description: String,
    /// Whether the item is required
    pub required: bool,
}

impl<T: Clone> ConfigItem<T> {
    /// Create a new configuration item
    pub fn new(key: impl Into<String>, value: T, default: T) -> Self {
        Self {
            key: key.into(),
            value,
            default,
            description: String::new(),
            required: false,
        }
    }

    /// Reset value to default
    pub fn reset(&mut self) {
        self.value = self.default.clone();
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_config() {
        let config = get_api_config("openai").unwrap();
        assert_eq!(config.name, "OpenAI");
        assert_eq!(config.auth_type, APIAuthType::BearerToken);
    }

    #[test]
    fn test_project_metadata() {
        let mut meta = ProjectMetadata::new("test_project");
        assert_eq!(meta.name, "test_project");

        let original_time = meta.modified_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        meta.touch();
        assert!(meta.modified_at > original_time);
    }

    #[test]
    fn test_processing_result() {
        let result: ProcessingResult<String> = ProcessingResult::ok("Success", "data".to_string());
        assert!(result.success);
        assert_eq!(result.data.unwrap(), "data");

        let fail: ProcessingResult<String> = ProcessingResult::fail("Error occurred");
        assert!(!fail.success);
        assert!(fail.error.is_some());
    }
}
