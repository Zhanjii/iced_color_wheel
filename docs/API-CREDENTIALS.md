# API & Credentials Management

This guide covers secure credential storage, API client patterns, and OAuth integration for Rust desktop applications.

## Overview

Desktop apps often need to:
- Store API keys securely (not in plain text)
- Authenticate with OAuth providers (Google, etc.)
- Manage multiple credential sets (dev vs production)
- Handle credential refresh and expiration

## Architecture

```
src/
├── utils/
│   ├── credentials.rs      # Keyring integration (keyring crate)
│   ├── encryption.rs       # AES-GCM encryption for files
│   ├── oauth_client.rs     # OAuth2 flow handler
│   └── api_client.rs       # API client factory
└── keys/                   # Bundled encrypted credentials (optional)
    ├── api_credentials.enc # Encrypted API keys
    └── oauth_config.json   # OAuth client config
```

## Credential Storage Options

### Option 1: System Keyring (Recommended for User Credentials)

Use the system's secure credential storage via the `keyring` crate:

```rust
use keyring::Entry;

// Store credential
let entry = Entry::new("my_app", "api_key")?;
entry.set_password("secret_value")?;

// Retrieve credential
let api_key = entry.get_password()?;

// Delete credential
entry.delete_credential()?;
```

**Pros:**
- OS-level security (Windows Credential Manager, macOS Keychain, Linux Secret Service)
- User-specific storage
- No encryption key to manage

**Cons:**
- Requires user interaction on some systems
- Not suitable for bundled app credentials

### Option 2: AES-GCM Encryption (For Bundled Credentials)

Encrypt credentials with a bundled key using the `aes-gcm` crate:

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::aead::rand_core::RngCore;

// Generate key once (store in keys/encryption.key)
let mut key_bytes = [0u8; 32];
OsRng.fill_bytes(&mut key_bytes);

// Encrypt
let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
let mut nonce_bytes = [0u8; 12];
OsRng.fill_bytes(&mut nonce_bytes);
let nonce = Nonce::from_slice(&nonce_bytes);
let ciphertext = cipher.encrypt(nonce, b"api_secret".as_ref())?;

// Decrypt
let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())?;
```

**Pros:**
- Can bundle encrypted files with app
- Works offline
- Suitable for team-shared credentials

**Cons:**
- Key is bundled with app (security through obscurity)
- Should be combined with remote PIN activation

### Option 3: Environment Variables (For Development)

```rust
use std::env;

let api_key = env::var("MY_APP_API_KEY").ok();
```

**Pros:**
- Simple for development
- CI/CD friendly

**Cons:**
- Not suitable for distributed apps

## Keyring Manager Pattern

```rust
//! credentials.rs - Secure credential management with system keyring.

use keyring::Entry;
use tracing::{error, info};

/// Manages secure credential storage using system keyring.
///
/// # Usage
/// ```
/// let creds = CredentialManager::new("my_app");
/// creds.store("api_key", "secret123")?;
/// let key = creds.get("api_key")?;
/// ```
pub struct CredentialManager {
    service_name: String,
}

impl CredentialManager {
    /// Create a new credential manager.
    ///
    /// `service_name` is the unique identifier for your app in the keyring.
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
        }
    }

    /// Store a credential securely.
    pub fn store(&self, key: &str, value: &str) -> Result<(), CredentialError> {
        let entry = Entry::new(&self.service_name, key)
            .map_err(|e| CredentialError::Keyring(e.to_string()))?;

        entry
            .set_password(value)
            .map_err(|e| CredentialError::Keyring(e.to_string()))?;

        info!("Stored credential: {key}");
        Ok(())
    }

    /// Retrieve a credential.
    pub fn get(&self, key: &str) -> Result<Option<String>, CredentialError> {
        let entry = Entry::new(&self.service_name, key)
            .map_err(|e| CredentialError::Keyring(e.to_string()))?;

        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => {
                error!("Failed to retrieve credential {key}: {e}");
                Err(CredentialError::Keyring(e.to_string()))
            }
        }
    }

    /// Remove a credential.
    pub fn delete(&self, key: &str) -> Result<(), CredentialError> {
        let entry = Entry::new(&self.service_name, key)
            .map_err(|e| CredentialError::Keyring(e.to_string()))?;

        match entry.delete_credential() {
            Ok(()) => {
                info!("Deleted credential: {key}");
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()), // Already gone
            Err(e) => {
                error!("Failed to delete credential {key}: {e}");
                Err(CredentialError::Keyring(e.to_string()))
            }
        }
    }

    /// Clear multiple credentials.
    pub fn clear_all(&self, keys: &[&str]) {
        for key in keys {
            let _ = self.delete(key);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("Keyring error: {0}")]
    Keyring(String),
}
```

## Encryption Manager Pattern

```rust
//! encryption.rs - File-based encryption for bundled credentials.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::Path;
use tracing::{error, info};

/// Manages encrypted credential files.
///
/// # Usage
/// ```no_run
/// let enc = EncryptionManager::from_key_file("keys/encryption.key")?;
///
/// // Encrypt credentials to file
/// enc.encrypt_to_file(&json!({"api_key": "secret"}), "keys/creds.enc")?;
///
/// // Decrypt from file
/// let creds: serde_json::Value = enc.decrypt_from_file("keys/creds.enc")?;
/// ```
pub struct EncryptionManager {
    cipher: Aes256Gcm,
}

/// Nonce is prepended to ciphertext in file format: [12-byte nonce][ciphertext...]
const NONCE_LEN: usize = 12;

impl EncryptionManager {
    /// Create from raw key bytes (32 bytes for AES-256).
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        Self { cipher }
    }

    /// Create from a key file on disk.
    pub fn from_key_file(key_path: &Path) -> Result<Self, EncryptionError> {
        let key_bytes = fs::read(key_path)
            .map_err(|e| EncryptionError::Io(e.to_string()))?;

        if key_bytes.len() != 32 {
            return Err(EncryptionError::InvalidKey(
                "Key must be exactly 32 bytes".into(),
            ));
        }

        let key: [u8; 32] = key_bytes.try_into().unwrap();
        Ok(Self::new(&key))
    }

    /// Generate a new random 32-byte encryption key.
    pub fn generate_key(output_path: Option<&Path>) -> Result<[u8; 32], EncryptionError> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        if let Some(path) = output_path {
            fs::write(path, &key)
                .map_err(|e| EncryptionError::Io(e.to_string()))?;
            info!("Generated key saved to {}", path.display());
        }

        Ok(key)
    }

    /// Encrypt bytes. Returns nonce + ciphertext.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| EncryptionError::EncryptFailed)?;

        let mut output = nonce_bytes.to_vec();
        output.extend(ciphertext);
        Ok(output)
    }

    /// Decrypt bytes (expects nonce + ciphertext format).
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        if data.len() < NONCE_LEN {
            return Err(EncryptionError::InvalidData("Data too short".into()));
        }

        let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| EncryptionError::DecryptFailed)
    }

    /// Encrypt a serializable value to a file.
    pub fn encrypt_to_file<T: Serialize>(
        &self,
        data: &T,
        path: &Path,
    ) -> Result<(), EncryptionError> {
        let json = serde_json::to_string(data)
            .map_err(|e| EncryptionError::Serialize(e.to_string()))?;

        let encrypted = self.encrypt(json.as_bytes())?;
        fs::write(path, &encrypted)
            .map_err(|e| EncryptionError::Io(e.to_string()))?;

        info!("Encrypted data to {}", path.display());
        Ok(())
    }

    /// Decrypt a file into a deserialized value.
    pub fn decrypt_from_file<T: DeserializeOwned>(
        &self,
        path: &Path,
    ) -> Result<T, EncryptionError> {
        let encrypted = fs::read(path)
            .map_err(|e| EncryptionError::Io(e.to_string()))?;

        let plaintext = self.decrypt(&encrypted)?;
        let json_str = String::from_utf8(plaintext)
            .map_err(|e| EncryptionError::InvalidData(e.to_string()))?;

        serde_json::from_str(&json_str)
            .map_err(|e| EncryptionError::Serialize(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Encryption failed")]
    EncryptFailed,
    #[error("Decryption failed (invalid key or corrupted data)")]
    DecryptFailed,
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Serialization error: {0}")]
    Serialize(String),
}
```

## OAuth2 Integration (Google Example)

```rust
//! oauth_client.rs - Google OAuth2 flow for desktop apps.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use tracing::{error, info};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Tokens returned from the OAuth flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<u64>,
}

/// Google OAuth2 client for desktop applications.
///
/// # Usage
/// ```no_run
/// let oauth = GoogleOAuthClient::new(
///     "your_client_id",
///     "your_client_secret",
///     &["https://www.googleapis.com/auth/gmail.readonly"],
/// );
///
/// // Start auth flow (opens browser)
/// let tokens = oauth.authenticate()?;
///
/// // Later, refresh the token
/// let new_tokens = oauth.refresh_token(&tokens.refresh_token.unwrap())?;
/// ```
pub struct GoogleOAuthClient {
    client_id: String,
    client_secret: String,
    scopes: Vec<String>,
    redirect_port: u16,
}

impl GoogleOAuthClient {
    pub fn new(client_id: &str, client_secret: &str, scopes: &[&str]) -> Self {
        Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
            redirect_port: 8089,
        }
    }

    /// Create client from a Google OAuth config JSON file.
    pub fn from_config_file(
        config_path: &std::path::Path,
        scopes: &[&str],
    ) -> Result<Self, OAuthError> {
        let contents = std::fs::read_to_string(config_path)
            .map_err(|e| OAuthError::Config(e.to_string()))?;

        let config: serde_json::Value = serde_json::from_str(&contents)
            .map_err(|e| OAuthError::Config(e.to_string()))?;

        let installed = config
            .get("installed")
            .or_else(|| config.get("web"))
            .ok_or_else(|| OAuthError::Config("Missing 'installed' or 'web' key".into()))?;

        let client_id = installed["client_id"]
            .as_str()
            .ok_or_else(|| OAuthError::Config("Missing client_id".into()))?;
        let client_secret = installed["client_secret"]
            .as_str()
            .ok_or_else(|| OAuthError::Config("Missing client_secret".into()))?;

        Ok(Self::new(client_id, client_secret, scopes))
    }

    /// Generate the authorization URL.
    pub fn get_auth_url(&self) -> String {
        let redirect_uri = format!("http://localhost:{}", self.redirect_port);
        format!(
            "{GOOGLE_AUTH_URL}?\
            client_id={}&\
            redirect_uri={redirect_uri}&\
            response_type=code&\
            scope={}&\
            access_type=offline&\
            prompt=consent",
            self.client_id,
            self.scopes.join(" "),
        )
    }

    /// Start OAuth flow: opens browser, listens for callback, exchanges code.
    pub fn authenticate(&self) -> Result<OAuthTokens, OAuthError> {
        let redirect_uri = format!("http://localhost:{}", self.redirect_port);

        // Start local TCP listener
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.redirect_port))
            .map_err(|e| OAuthError::Server(e.to_string()))?;

        // Open browser
        let auth_url = self.get_auth_url();
        open::that(&auth_url).map_err(|e| OAuthError::Browser(e.to_string()))?;
        info!("Opened browser for authentication");

        // Wait for callback
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| OAuthError::Server(e.to_string()))?;

        let reader = BufReader::new(&stream);
        let request_line = reader
            .lines()
            .next()
            .ok_or_else(|| OAuthError::NoCode)?
            .map_err(|e| OAuthError::Server(e.to_string()))?;

        // Parse auth code from GET request
        let code = Self::extract_code(&request_line)?;

        // Send success response to browser
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><body>\
            <h1>Authorization Successful!</h1>\
            <p>You can close this window and return to the application.</p>\
            <script>window.close();</script>\
            </body></html>";
        stream.write_all(response.as_bytes()).ok();

        // Exchange code for tokens
        self.exchange_code(&code, &redirect_uri)
    }

    fn extract_code(request_line: &str) -> Result<String, OAuthError> {
        // Parse "GET /?code=xxx&... HTTP/1.1"
        let path = request_line
            .split_whitespace()
            .nth(1)
            .ok_or(OAuthError::NoCode)?;

        let url = url::Url::parse(&format!("http://localhost{path}"))
            .map_err(|_| OAuthError::NoCode)?;

        url.query_pairs()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.to_string())
            .ok_or(OAuthError::NoCode)
    }

    fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<OAuthTokens, OAuthError> {
        let client = Client::new();
        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
        ];

        let resp = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;

        resp.json::<OAuthTokens>()
            .map_err(|e| OAuthError::TokenExchange(e.to_string()))
    }

    /// Refresh an access token.
    pub fn refresh_token(&self, refresh_token: &str) -> Result<OAuthTokens, OAuthError> {
        let client = Client::new();
        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let resp = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;

        resp.json::<OAuthTokens>()
            .map_err(|e| OAuthError::TokenExchange(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("Config error: {0}")]
    Config(String),
    #[error("Server error: {0}")]
    Server(String),
    #[error("Browser error: {0}")]
    Browser(String),
    #[error("No authorization code received")]
    NoCode,
    #[error("Token exchange failed: {0}")]
    TokenExchange(String),
}
```

## API Client Factory Pattern

```rust
//! api_client.rs - Factory for creating authenticated API clients.

use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;
use tracing::error;

/// Base API client with authentication and error handling.
///
/// # Usage
/// ```no_run
/// let client = ApiClient::new("https://api.example.com")
///     .api_key("your_key");
///
/// let response: serde_json::Value = client.get("/users")?;
/// client.post("/users", &json!({"name": "John"}))?;
/// ```
pub struct ApiClient {
    base_url: String,
    client: Client,
    timeout: Duration,
}

impl ApiClient {
    /// Create a new API client for the given base URL.
    pub fn new(base_url: &str) -> ApiClientBuilder {
        ApiClientBuilder {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: None,
            bearer_token: None,
            timeout: Duration::from_secs(30),
        }
    }

    fn request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
    ) -> Result<T, ApiError> {
        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));

        let resp = self
            .client
            .request(method, &url)
            .timeout(self.timeout)
            .send()
            .map_err(|e| ApiError::Request(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Http(status.as_u16(), status.to_string()));
        }

        resp.json::<T>()
            .map_err(|e| ApiError::Deserialize(e.to_string()))
    }

    /// GET request.
    pub fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, ApiError> {
        self.request(reqwest::Method::GET, endpoint)
    }

    /// POST request with JSON body.
    pub fn post<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));

        let resp = self
            .client
            .post(&url)
            .json(body)
            .timeout(self.timeout)
            .send()
            .map_err(|e| ApiError::Request(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Http(status.as_u16(), status.to_string()));
        }

        resp.json::<T>()
            .map_err(|e| ApiError::Deserialize(e.to_string()))
    }

    /// PUT request with JSON body.
    pub fn put<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let url = format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'));

        let resp = self
            .client
            .put(&url)
            .json(body)
            .timeout(self.timeout)
            .send()
            .map_err(|e| ApiError::Request(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Http(status.as_u16(), status.to_string()));
        }

        resp.json::<T>()
            .map_err(|e| ApiError::Deserialize(e.to_string()))
    }

    /// DELETE request.
    pub fn delete<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, ApiError> {
        self.request(reqwest::Method::DELETE, endpoint)
    }
}

/// Builder for `ApiClient`.
pub struct ApiClientBuilder {
    base_url: String,
    api_key: Option<String>,
    bearer_token: Option<String>,
    timeout: Duration,
}

impl ApiClientBuilder {
    /// Set an API key for the `X-API-Key` header.
    pub fn api_key(mut self, key: &str) -> Self {
        self.api_key = Some(key.to_string());
        self
    }

    /// Set a bearer token for the `Authorization` header.
    pub fn bearer_token(mut self, token: &str) -> Self {
        self.bearer_token = Some(token.to_string());
        self
    }

    /// Set request timeout (default: 30s).
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Build the `ApiClient`.
    pub fn build(self) -> Result<ApiClient, ApiError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        if let Some(ref key) = self.api_key {
            headers.insert(
                "X-API-Key",
                HeaderValue::from_str(key)
                    .map_err(|e| ApiError::Config(e.to_string()))?,
            );
        }

        if let Some(ref token) = self.bearer_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|e| ApiError::Config(e.to_string()))?,
            );
        }

        let client = Client::builder()
            .default_headers(headers)
            .timeout(self.timeout)
            .build()
            .map_err(|e| ApiError::Config(e.to_string()))?;

        Ok(ApiClient {
            base_url: self.base_url,
            client,
            timeout: self.timeout,
        })
    }
}

/// Factory for creating pre-configured API clients.
///
/// # Usage
/// ```no_run
/// let mut factory = ApiClientFactory::new();
/// factory.register("github", "https://api.github.com", |b| b.bearer_token("ghp_xxx"));
///
/// let github = factory.get("github")?;
/// let repos: serde_json::Value = github.get("/user/repos")?;
/// ```
pub struct ApiClientFactory {
    clients: HashMap<String, ApiClient>,
    builders: HashMap<String, ApiClientBuilder>,
}

impl ApiClientFactory {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            builders: HashMap::new(),
        }
    }

    /// Register an API configuration.
    pub fn register<F>(&mut self, name: &str, base_url: &str, configure: F)
    where
        F: FnOnce(ApiClientBuilder) -> ApiClientBuilder,
    {
        let builder = ApiClient::new(base_url);
        self.builders
            .insert(name.to_string(), configure(builder));
    }

    /// Get or create an API client.
    pub fn get(&mut self, name: &str) -> Result<&ApiClient, ApiError> {
        if !self.clients.contains_key(name) {
            let builder = self
                .builders
                .remove(name)
                .ok_or_else(|| ApiError::Config(format!("Unknown API: {name}")))?;

            self.clients.insert(name.to_string(), builder.build()?);
        }

        Ok(&self.clients[name])
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Config error: {0}")]
    Config(String),
    #[error("Request failed: {0}")]
    Request(String),
    #[error("HTTP error {0}: {1}")]
    Http(u16, String),
    #[error("Deserialization error: {0}")]
    Deserialize(String),
}
```

## Complete Integration Example

```rust
//! Example: Setting up credentials for a new app.

use std::path::Path;

// 1. Generate encryption key (do once, bundle with app)
let key = EncryptionManager::generate_key(Some(Path::new("keys/encryption.key")))?;

// 2. Encrypt your API credentials (do during build)
let enc = EncryptionManager::new(&key);
enc.encrypt_to_file(
    &serde_json::json!({
        "github_token": "ghp_xxxx",
        "slack_webhook": "https://hooks.slack.com/xxx"
    }),
    Path::new("keys/api_credentials.enc"),
)?;

// 3. At runtime, decrypt and use
let enc = EncryptionManager::from_key_file(Path::new("keys/encryption.key"))?;
let bundled_creds: serde_json::Value =
    enc.decrypt_from_file(Path::new("keys/api_credentials.enc"))?;

// Load user-specific credentials from keyring
let user_creds = CredentialManager::new("my_app");
let user_token = user_creds.get("oauth_token")?;

// Create API clients
let mut factory = ApiClientFactory::new();
factory.register("github", "https://api.github.com", |b| {
    b.bearer_token(bundled_creds["github_token"].as_str().unwrap())
});

// Use the client
let github = factory.get("github")?;
let user: serde_json::Value = github.get("/user")?;
```

## Cargo.toml Dependencies

The patterns in this document use these crates:

```toml
[dependencies]
keyring = "3"           # System keyring integration
aes-gcm = "0.10"       # AES-256-GCM encryption
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["blocking", "json"] }
url = "2"               # URL parsing for OAuth callback
open = "5"              # Open browser for OAuth
tracing = "0.1"         # Logging
thiserror = "2"         # Error derive macro
chrono = { version = "0.4", features = ["serde"] }
```

## Security Considerations

1. **Never commit unencrypted credentials** to git
2. **Use .gitignore** for key files during development
3. **Bundle encrypted files** in production builds (via `include_bytes!` or resource embedding)
4. **Combine with remote PIN** for distributed apps
5. **Clear credentials on version change** if security model changes
6. **Use keyring for user-specific** secrets (OAuth tokens)
7. **Use AES-GCM encryption for bundled** team credentials

## See Also

- [SECURITY-MODEL.md](SECURITY-MODEL.md) - Remote PIN activation
- [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) - Configuration management
- [templates/credentials.rs](templates/credentials.rs) - Full implementation
- [templates/encryption.rs](templates/encryption.rs) - Encryption utilities
- [templates/oauth_client.rs](templates/oauth_client.rs) - OAuth2 client
