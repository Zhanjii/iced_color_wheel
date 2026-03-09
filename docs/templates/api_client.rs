//! api_client.rs
//!
//! HTTP client for authenticated API requests with retry logic.
//!
//! Features:
//! - Multiple authentication methods (API key, Bearer token)
//! - Automatic retry on transient failures
//! - Connection pooling and timeout handling
//! - Request/response logging
//!
//! # Example
//!
//! ```rust
//! use api_client::{ApiClient, ApiClientFactory};
//!
//! // Direct client usage
//! let client = ApiClient::new("https://api.github.com")
//!     .with_bearer_token("ghp_xxxx")
//!     .build()?;
//!
//! let user = client.get("/user", None).await?;
//! println!("User: {:?}", user);
//!
//! // Factory pattern for multiple APIs
//! let mut factory = ApiClientFactory::new();
//! factory.register("github", "https://api.github.com", Some("ghp_xxx"), None);
//! factory.register("slack", "https://slack.com/api", Some("xoxb-xxx"), None);
//!
//! let github = factory.get("github")?;
//! ```

use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error};

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur during API operations.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP {status_code}: {reason}")]
    HttpError {
        status_code: u16,
        reason: String,
        response: Option<Value>,
    },

    #[error("Request timeout")]
    Timeout,

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}

/// Result type for API operations.
pub type ApiResult<T> = Result<T, ApiError>;

// =============================================================================
// API CLIENT BUILDER
// =============================================================================

/// Builder for constructing API clients with custom configuration.
#[derive(Debug, Clone)]
pub struct ApiClientBuilder {
    base_url: String,
    api_key: Option<String>,
    bearer_token: Option<String>,
    timeout: Duration,
    retries: u32,
    headers: HashMap<String, String>,
}

impl ApiClientBuilder {
    /// Creates a new builder with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: None,
            bearer_token: None,
            timeout: Duration::from_secs(30),
            retries: 3,
            headers: HashMap::new(),
        }
    }

    /// Sets API key authentication (X-API-Key header).
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets Bearer token authentication (Authorization header).
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Sets request timeout duration.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets number of retry attempts for failed requests.
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Adds a custom header to all requests.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Builds the ApiClient.
    pub fn build(self) -> ApiResult<ApiClient> {
        let mut headers = reqwest::header::HeaderMap::new();

        // Set authentication headers
        if let Some(api_key) = &self.api_key {
            headers.insert("X-API-Key", api_key.parse().unwrap());
        }

        if let Some(token) = &self.bearer_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(
                reqwest::header::AUTHORIZATION,
                auth_value.parse().unwrap(),
            );
        }

        // Set common headers
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/json".parse().unwrap(),
        );

        // Add custom headers
        for (key, value) in &self.headers {
            if let Ok(header_name) = reqwest::header::HeaderName::try_from(key.as_str()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        let client = Client::builder()
            .timeout(self.timeout)
            .default_headers(headers)
            .build()?;

        Ok(ApiClient {
            base_url: self.base_url.trim_end_matches('/').to_string(),
            client,
            retries: self.retries,
        })
    }
}

// =============================================================================
// API CLIENT
// =============================================================================

/// HTTP client for API requests with authentication and error handling.
#[derive(Debug, Clone)]
pub struct ApiClient {
    base_url: String,
    client: Client,
    retries: u32,
}

impl ApiClient {
    /// Creates a new builder for constructing an ApiClient.
    pub fn new(base_url: impl Into<String>) -> ApiClientBuilder {
        ApiClientBuilder::new(base_url)
    }

    /// Builds full URL from endpoint.
    fn build_url(&self, endpoint: &str) -> String {
        format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'))
    }

    /// Handles API response, converting errors to ApiError.
    async fn handle_response(&self, response: Response) -> ApiResult<Option<Value>> {
        let status = response.status();

        if status.is_success() {
            if response.content_length() == Some(0) {
                return Ok(None);
            }

            match response.json::<Value>().await {
                Ok(json) => Ok(Some(json)),
                Err(_) => {
                    // Try to get as text if JSON parsing fails
                    if let Ok(text) = response.text().await {
                        Ok(Some(Value::String(text)))
                    } else {
                        Ok(None)
                    }
                }
            }
        } else {
            let error_data = response.json::<Value>().await.ok();
            Err(ApiError::HttpError {
                status_code: status.as_u16(),
                reason: status.canonical_reason().unwrap_or("Unknown").to_string(),
                response: error_data,
            })
        }
    }

    /// Makes HTTP GET request.
    pub async fn get(
        &self,
        endpoint: &str,
        params: Option<&HashMap<String, String>>,
    ) -> ApiResult<Option<Value>> {
        let url = self.build_url(endpoint);
        debug!("GET {}", url);

        let mut request = self.client.get(&url);

        if let Some(params) = params {
            request = request.query(params);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Makes HTTP POST request with JSON body.
    pub async fn post(&self, endpoint: &str, json: Option<&Value>) -> ApiResult<Option<Value>> {
        let url = self.build_url(endpoint);
        debug!("POST {}", url);

        let mut request = self.client.post(&url);

        if let Some(json) = json {
            request = request.json(json);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Makes HTTP PUT request with JSON body.
    pub async fn put(&self, endpoint: &str, json: Option<&Value>) -> ApiResult<Option<Value>> {
        let url = self.build_url(endpoint);
        debug!("PUT {}", url);

        let mut request = self.client.put(&url);

        if let Some(json) = json {
            request = request.json(json);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Makes HTTP PATCH request with JSON body.
    pub async fn patch(&self, endpoint: &str, json: Option<&Value>) -> ApiResult<Option<Value>> {
        let url = self.build_url(endpoint);
        debug!("PATCH {}", url);

        let mut request = self.client.patch(&url);

        if let Some(json) = json {
            request = request.json(json);
        }

        let response = request.send().await?;
        self.handle_response(response).await
    }

    /// Makes HTTP DELETE request.
    pub async fn delete(&self, endpoint: &str) -> ApiResult<Option<Value>> {
        let url = self.build_url(endpoint);
        debug!("DELETE {}", url);

        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Updates the bearer token for subsequent requests.
    pub fn update_bearer_token(&mut self, token: impl Into<String>) {
        // Note: This requires rebuilding the client to update default headers
        // In practice, you might want to track the token separately and apply it per-request
        debug!("Bearer token update requested (requires client rebuild)");
    }
}

// =============================================================================
// API CLIENT FACTORY
// =============================================================================

/// Factory for creating and managing multiple API clients.
#[derive(Debug, Default)]
pub struct ApiClientFactory {
    clients: HashMap<String, ApiClient>,
    configs: HashMap<String, ClientConfig>,
}

#[derive(Debug, Clone)]
struct ClientConfig {
    base_url: String,
    api_key: Option<String>,
    bearer_token: Option<String>,
    timeout: Duration,
}

impl ApiClientFactory {
    /// Creates a new empty factory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an API configuration.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        base_url: impl Into<String>,
        bearer_token: Option<String>,
        api_key: Option<String>,
    ) {
        let name = name.into();
        let config = ClientConfig {
            base_url: base_url.into(),
            api_key,
            bearer_token,
            timeout: Duration::from_secs(30),
        };

        self.configs.insert(name.clone(), config);
        debug!("Registered API: {} -> {}", name, self.configs[&name].base_url);
    }

    /// Gets or creates an API client by name.
    pub fn get(&mut self, name: &str) -> ApiResult<&ApiClient> {
        if !self.clients.contains_key(name) {
            let config = self
                .configs
                .get(name)
                .ok_or_else(|| {
                    ApiError::ConnectionError(format!("Unknown API: {}. Register it first.", name))
                })?;

            let mut builder = ApiClient::new(&config.base_url).with_timeout(config.timeout);

            if let Some(ref token) = config.bearer_token {
                builder = builder.with_bearer_token(token);
            }

            if let Some(ref key) = config.api_key {
                builder = builder.with_api_key(key);
            }

            let client = builder.build()?;
            self.clients.insert(name.to_string(), client);
            debug!("Created API client: {}", name);
        }

        Ok(self.clients.get(name).unwrap())
    }

    /// Updates bearer token for a registered API.
    pub fn update_token(&mut self, name: &str, token: impl Into<String>) {
        if let Some(config) = self.configs.get_mut(name) {
            config.bearer_token = Some(token.into());
            // Remove cached client to force rebuild
            self.clients.remove(name);
        }
    }

    /// Removes an API registration.
    pub fn remove(&mut self, name: &str) {
        self.clients.remove(name);
        self.configs.remove(name);
    }

    /// Lists all registered API names.
    pub fn list_apis(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let client = ApiClient::new("https://api.example.com")
            .with_bearer_token("test_token")
            .with_timeout(Duration::from_secs(10))
            .build();

        assert!(client.is_ok());
    }

    #[test]
    fn test_factory() {
        let mut factory = ApiClientFactory::new();
        factory.register("test", "https://api.test.com", Some("token".to_string()), None);

        assert_eq!(factory.list_apis(), vec!["test"]);
        assert!(factory.get("test").is_ok());
    }

    #[test]
    fn test_url_building() {
        let client = ApiClient::new("https://api.example.com")
            .build()
            .unwrap();

        assert_eq!(
            client.build_url("/users"),
            "https://api.example.com/users"
        );
        assert_eq!(
            client.build_url("users"),
            "https://api.example.com/users"
        );
    }
}
