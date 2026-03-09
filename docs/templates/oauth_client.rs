//! oauth_client.rs
//!
//! Google OAuth2 flow handler for desktop applications.
//!
//! Implements the OAuth2 authorization code flow for desktop apps using a local HTTP server
//! to receive the callback.
//!
//! # Example
//!
//! ```rust
//! use oauth_client::{GoogleOAuthClient, GoogleScopes};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let oauth = GoogleOAuthClient::new(
//!         "your_client_id.apps.googleusercontent.com",
//!         "your_client_secret",
//!         vec![
//!             GoogleScopes::GMAIL_READONLY.to_string(),
//!             GoogleScopes::DRIVE_READONLY.to_string(),
//!         ],
//!     );
//!
//!     let tokens = oauth.authenticate().await?;
//!     println!("Access token: {}", tokens.access_token);
//!
//!     // Later: refresh the access token
//!     let new_tokens = oauth.refresh_token(&tokens.refresh_token.unwrap()).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Setup
//!
//! 1. Go to Google Cloud Console (https://console.cloud.google.com)
//! 2. Create a project or select existing
//! 3. Enable required APIs (Gmail, Drive, etc.)
//! 4. Create OAuth 2.0 credentials (Desktop application)
//! 5. Download credentials JSON or copy client_id/client_secret

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{error, info};
use url::Url;

// =============================================================================
// CONSTANTS
// =============================================================================

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur during OAuth operations.
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("No authorization code received")]
    NoAuthorizationCode,

    #[error("Server error: {0}")]
    ServerError(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    UrlError(#[from] url::ParseError),
}

/// Result type for OAuth operations.
pub type OAuthResult<T> = Result<T, OAuthError>;

// =============================================================================
// TOKEN TYPES
// =============================================================================

/// OAuth token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: String,
}

// =============================================================================
// GOOGLE OAUTH CLIENT
// =============================================================================

/// Google OAuth2 client for desktop applications.
#[derive(Debug, Clone)]
pub struct GoogleOAuthClient {
    client_id: String,
    client_secret: String,
    scopes: Vec<String>,
    redirect_port: u16,
}

impl GoogleOAuthClient {
    /// Creates a new OAuth client.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            scopes,
            redirect_port: 8089,
        }
    }

    /// Sets a custom redirect port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.redirect_port = port;
        self
    }

    /// Generates the authorization URL.
    pub fn get_auth_url(&self) -> String {
        let redirect_uri = format!("http://localhost:{}", self.redirect_port);
        let scope = self.scopes.join(" ");

        let params = [
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", &redirect_uri),
            ("response_type", "code"),
            ("scope", &scope),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ];

        let url = Url::parse_with_params(GOOGLE_AUTH_URL, &params).unwrap();
        url.to_string()
    }

    /// Starts OAuth flow and waits for user authorization.
    pub async fn authenticate(&self) -> OAuthResult<TokenResponse> {
        let auth_url = self.get_auth_url();

        info!("Opening browser for Google authentication...");

        // Open browser
        if let Err(e) = open::that(&auth_url) {
            error!("Failed to open browser: {}", e);
        }

        // Start local server and wait for callback
        let code = self.wait_for_callback()?;

        // Exchange code for tokens
        self.exchange_code(&code).await
    }

    /// Waits for OAuth callback on local HTTP server.
    fn wait_for_callback(&self) -> OAuthResult<String> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.redirect_port))?;
        listener.set_nonblocking(false)?;

        info!("Waiting for OAuth callback on port {}...", self.redirect_port);

        let result = Arc::new(Mutex::new(None));
        let result_clone = Arc::clone(&result);

        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0; 4096];
            if let Ok(size) = stream.read(&mut buffer) {
                let request = String::from_utf8_lossy(&buffer[..size]);

                // Parse URL from first line
                if let Some(first_line) = request.lines().next() {
                    if let Some(path) = first_line.split_whitespace().nth(1) {
                        if let Ok(url) = Url::parse(&format!("http://localhost{}", path)) {
                            let params: HashMap<_, _> = url.query_pairs().collect();

                            if let Some(code) = params.get("code") {
                                *result_clone.lock().unwrap() = Some(code.to_string());
                                let _ = stream.write_all(self.success_html().as_bytes());
                            } else if let Some(error) = params.get("error") {
                                let _ = stream.write_all(self.error_html(error).as_bytes());
                            }
                        }
                    }
                }
            }
        }

        result
            .lock()
            .unwrap()
            .take()
            .ok_or(OAuthError::NoAuthorizationCode)
    }

    /// Exchanges authorization code for tokens.
    async fn exchange_code(&self, code: &str) -> OAuthResult<TokenResponse> {
        let redirect_uri = format!("http://localhost:{}", self.redirect_port);
        let client = Client::new();

        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &redirect_uri),
        ];

        let response = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .await?;

        if response.status().is_success() {
            let tokens: TokenResponse = response.json().await?;
            info!("Token exchange successful");
            Ok(tokens)
        } else {
            let error_text = response.text().await?;
            Err(OAuthError::TokenExchangeFailed(error_text))
        }
    }

    /// Refreshes an expired access token.
    pub async fn refresh_token(&self, refresh_token: &str) -> OAuthResult<TokenResponse> {
        let client = Client::new();

        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let response = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .await?;

        if response.status().is_success() {
            let mut tokens: TokenResponse = response.json().await?;

            // Preserve refresh_token if not returned
            if tokens.refresh_token.is_none() {
                tokens.refresh_token = Some(refresh_token.to_string());
            }

            info!("Token refreshed successfully");
            Ok(tokens)
        } else {
            let error_text = response.text().await?;
            Err(OAuthError::TokenExchangeFailed(error_text))
        }
    }

    /// Revokes a token (access or refresh).
    pub async fn revoke_token(&self, token: &str) -> OAuthResult<bool> {
        let client = Client::new();

        let params = [("token", token)];

        let response = client
            .post(GOOGLE_REVOKE_URL)
            .form(&params)
            .send()
            .await?;

        let success = response.status().is_success() || response.status().as_u16() == 400;

        if success {
            info!("Token revoked successfully");
        }

        Ok(success)
    }

    /// HTML response for successful authentication.
    fn success_html(&self) -> String {
        r#"HTTP/1.1 200 OK
Content-Type: text/html

<!DOCTYPE html>
<html>
<head>
    <title>Authorization Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            text-align: center;
            background: white;
            padding: 40px 60px;
            border-radius: 10px;
            box-shadow: 0 10px 40px rgba(0,0,0,0.2);
        }
        h1 { color: #22c55e; margin-bottom: 10px; }
        p { color: #666; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Authorization Successful!</h1>
        <p>You can close this window and return to the application.</p>
    </div>
    <script>setTimeout(() => window.close(), 3000);</script>
</body>
</html>"#
            .to_string()
    }

    /// HTML response for failed authentication.
    fn error_html(&self, message: &str) -> String {
        format!(
            r#"HTTP/1.1 400 Bad Request
Content-Type: text/html

<!DOCTYPE html>
<html>
<head>
    <title>Authorization Failed</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: #fee2e2;
        }}
        .container {{
            text-align: center;
            background: white;
            padding: 40px 60px;
            border-radius: 10px;
            box-shadow: 0 10px 40px rgba(0,0,0,0.1);
        }}
        h1 {{ color: #ef4444; margin-bottom: 10px; }}
        p {{ color: #666; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Authorization Failed</h1>
        <p>{}</p>
        <p>Please close this window and try again.</p>
    </div>
</body>
</html>"#,
            message
        )
    }
}

// =============================================================================
// GOOGLE SCOPES
// =============================================================================

/// Common Google OAuth2 scopes.
pub struct GoogleScopes;

impl GoogleScopes {
    // Gmail
    pub const GMAIL_READONLY: &'static str = "https://www.googleapis.com/auth/gmail.readonly";
    pub const GMAIL_SEND: &'static str = "https://www.googleapis.com/auth/gmail.send";
    pub const GMAIL_MODIFY: &'static str = "https://www.googleapis.com/auth/gmail.modify";

    // Drive
    pub const DRIVE_READONLY: &'static str = "https://www.googleapis.com/auth/drive.readonly";
    pub const DRIVE_FILE: &'static str = "https://www.googleapis.com/auth/drive.file";
    pub const DRIVE_FULL: &'static str = "https://www.googleapis.com/auth/drive";

    // Calendar
    pub const CALENDAR_READONLY: &'static str =
        "https://www.googleapis.com/auth/calendar.readonly";
    pub const CALENDAR_EVENTS: &'static str = "https://www.googleapis.com/auth/calendar.events";

    // Sheets
    pub const SHEETS_READONLY: &'static str =
        "https://www.googleapis.com/auth/spreadsheets.readonly";
    pub const SHEETS_FULL: &'static str = "https://www.googleapis.com/auth/spreadsheets";

    // User info
    pub const USERINFO_EMAIL: &'static str = "https://www.googleapis.com/auth/userinfo.email";
    pub const USERINFO_PROFILE: &'static str = "https://www.googleapis.com/auth/userinfo.profile";
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_url_generation() {
        let client = GoogleOAuthClient::new(
            "test_client_id",
            "test_secret",
            vec![GoogleScopes::GMAIL_READONLY.to_string()],
        );

        let url = client.get_auth_url();
        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("gmail.readonly"));
    }

    #[test]
    fn test_custom_port() {
        let client = GoogleOAuthClient::new("id", "secret", vec![]).with_port(9000);
        assert_eq!(client.redirect_port, 9000);
    }
}
