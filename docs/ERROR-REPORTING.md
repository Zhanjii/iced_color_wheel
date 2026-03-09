# Error Reporting & Logging

This document describes the logging system, error reporting, and crash handling architecture for Rust applications using the `tracing` ecosystem.

## Key Features

- **Structured Logging** - `tracing` crate with spans, events, and structured fields
- **Local JSON Logs** - All errors saved locally with file rotation
- **Sensitive Data Filtering** - API keys, passwords, emails automatically redacted via custom `tracing` layer
- **Action Tracking** - Last N user actions included in error context
- **Panic Handling** - Custom panic hook with `std::panic::set_hook` and optional `catch_unwind`
- **Backtrace Capture** - `std::backtrace::Backtrace` for detailed error context

---

## Architecture Overview

```
+----------------------------------------------------------------+
|                    Error Handling System                         |
+----------------------------------------------------------------+
|                                                                  |
|  +------------------+    +------------------+                   |
|  |  tracing setup   |    |  ErrorReporter   |                   |
|  | (thread-safe)    |    | (crash reports)  |                   |
|  +--------+---------+    +--------+---------+                   |
|           |                       |                              |
|  +--------+---------+    +--------+---------+                   |
|  | stdout layer     |    | Local JSON logs  |                   |
|  | file layer       |    | Email / webhook  |                   |
|  | memory layer     |    | Action tracking  |                   |
|  +------------------+    +------------------+                   |
|                                                                  |
|  +---------------------------------------------+               |
|  |        Custom Error Hierarchy (thiserror)    |               |
|  |  AppError -> ConfigError                     |               |
|  |            -> NetworkError                   |               |
|  |            -> ValidationError                |               |
|  +---------------------------------------------+               |
|                                                                  |
|  +---------------------------------------------+               |
|  |         Error Collection Utilities           |               |
|  |  ThreadSafeErrorCollector (Arc<Mutex<_>>)    |               |
|  |  BatchErrorCollector                         |               |
|  +---------------------------------------------+               |
|                                                                  |
+----------------------------------------------------------------+
```

## Error Reporting Services

For production applications, consider sending error reports to a reporting service or webhook rather than email. Common approaches:

### Option A: Webhook (Slack, Discord, Custom)

Send JSON error reports to a webhook endpoint:

```rust
use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorWebhookPayload {
    app_version: String,
    error_type: String,
    message: String,
    backtrace: String,
    platform: String,
    recent_actions: Vec<String>,
}

async fn send_error_webhook(
    client: &Client,
    webhook_url: &str,
    payload: &ErrorWebhookPayload,
) -> Result<(), reqwest::Error> {
    client
        .post(webhook_url)
        .json(payload)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
```

### Option B: Email via `lettre`

For email-based error reporting, use the `lettre` crate:

```rust
use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};

fn send_error_email(
    smtp_server: &str,
    smtp_port: u16,
    sender: &str,
    password: &str,
    recipient: &str,
    subject: &str,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let email = Message::builder()
        .from(sender.parse()?)
        .to(recipient.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())?;

    let creds = Credentials::new(sender.to_string(), password.to_string());

    let mailer = SmtpTransport::relay(smtp_server)?
        .port(smtp_port)
        .credentials(creds)
        .build();

    mailer.send(&email)?;
    Ok(())
}
```

### SMTP Password Storage

Store the SMTP password securely:

**Option A: Environment Variable** (simplest)
```bash
# Windows
set YOUR_APP_SMTP_PASSWORD=your-16-char-app-password

# Linux/macOS
export YOUR_APP_SMTP_PASSWORD=your-16-char-app-password
```

**Option B: Keyring** (more secure)
```rust
use keyring::Entry;

let entry = Entry::new("your_app_name", "smtp_password")?;
entry.set_password("your-16-char-app-password")?;

// Later, retrieve it:
let password = entry.get_password()?;
```

### Error Report Contents

When an error occurs, a report includes:

1. **Timestamp, app version, platform info** (OS, architecture, Rust version)
2. **Error type, message, and full backtrace**
3. **Context** (what operation was in progress)
4. **Last 20 user actions** for reproducing the issue

### Example Report

```
============================================================
Application Error Report
============================================================

Timestamp: 2024-01-15T14:32:18.456789Z
App Version: 1.2.3
Rust: 1.75.0
Platform: x86_64-pc-windows-msvc (Windows 11)

----------------------------------------
ERROR DETAILS
----------------------------------------
Type: ConfigError
Message: Invalid file format

Backtrace:
   0: your_app::config::load_config
             at src/config.rs:45
   1: your_app::main
             at src/main.rs:23
   ...

----------------------------------------
RECENT ACTIONS (last 20)
----------------------------------------
[14:31:45] User opened Settings dialog
[14:31:52] User selected Dark theme
[14:32:15] User clicked Process button
[14:32:18] Processing file: corrupted.dat

============================================================
This is an automated error report.
Full JSON report attached.
============================================================
```

---

## Tracing Setup

Initialize `tracing` with multiple layers (console, file, memory):

```rust
use std::path::Path;
use tracing::Level;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};
use tracing_appender::rolling;

/// Initialize the tracing/logging subsystem.
///
/// Call this early in `main()`, before any tracing macros are used.
pub fn init_tracing(log_dir: &Path, is_dev: bool) {
    let env_filter = if is_dev {
        EnvFilter::new("debug")
            .add_directive("hyper=warn".parse().unwrap())
            .add_directive("reqwest=warn".parse().unwrap())
    } else {
        EnvFilter::new("info")
            .add_directive("hyper=warn".parse().unwrap())
            .add_directive("reqwest=warn".parse().unwrap())
    };

    // Console layer: human-readable, colored
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_timer(fmt::time::uptime());

    // File layer: daily rotation, JSON format for machine parsing
    let file_appender = rolling::daily(log_dir, "app.log");
    let file_layer = fmt::layer()
        .json()
        .with_writer(file_appender)
        .with_target(true)
        .with_span_events(fmt::format::FmtSpan::CLOSE);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();
}
```

### Sensitive Data Redaction Layer

Create a custom `tracing` layer that redacts sensitive data before it reaches log output:

```rust
use regex::Regex;
use std::sync::LazyLock;

static REDACTION_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(), "[GITHUB_TOKEN]"),
        (Regex::new(r"gho_[a-zA-Z0-9]{36}").unwrap(), "[GITHUB_OAUTH]"),
        (Regex::new(r"sk-[a-zA-Z0-9]{48}").unwrap(), "[OPENAI_KEY]"),
        (Regex::new(r"Bearer\s+[a-zA-Z0-9\-._~+/]+=*").unwrap(), "Bearer [REDACTED]"),
        (Regex::new(r"(?i)api[_-]?key\s*[:=]\s*\S+").unwrap(), "api_key=[REDACTED]"),
        (Regex::new(r"(?i)password\s*[:=]\s*\S+").unwrap(), "password=[REDACTED]"),
        (Regex::new(r"(?i)secret\s*[:=]\s*\S+").unwrap(), "secret=[REDACTED]"),
        (Regex::new(r"[\w.\-]+@[\w.\-]+\.\w+").unwrap(), "[EMAIL]"),
    ]
});

pub fn redact_sensitive(text: &str) -> String {
    let mut result = text.to_string();
    for (pattern, replacement) in REDACTION_PATTERNS.iter() {
        result = pattern.replace_all(&result, *replacement).to_string();
    }
    result
}
```

Use it in tracing fields:

```rust
tracing::error!(
    message = redact_sensitive(&error_message),
    "Operation failed"
);
```

## ErrorReporter

Captures errors and writes structured JSON reports:

```rust
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::error;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorReport {
    pub timestamp: String,
    pub app_version: String,
    pub rust_version: String,
    pub platform: PlatformInfo,
    pub error: ErrorDetails,
    pub context: String,
    pub recent_actions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub error_type: String,
    pub message: String,
    pub backtrace: String,
}

pub struct ErrorReporter {
    error_dir: PathBuf,
    recent_actions: Mutex<Vec<String>>,
    max_actions: usize,
    app_version: String,
}

impl ErrorReporter {
    pub fn new(error_dir: &Path, app_version: &str) -> Self {
        std::fs::create_dir_all(error_dir).ok();
        Self {
            error_dir: error_dir.to_path_buf(),
            recent_actions: Mutex::new(Vec::new()),
            max_actions: 20,
            app_version: app_version.to_string(),
        }
    }

    // ==================== Action Logging ====================

    /// Log a user action for error context.
    pub fn log_action(&self, action: &str) {
        if let Ok(mut actions) = self.recent_actions.lock() {
            let timestamp = Utc::now().format("%H:%M:%S").to_string();
            actions.push(format!("[{timestamp}] {action}"));
            if actions.len() > self.max_actions {
                actions.remove(0);
            }
        }
    }

    // ==================== Error Capture ====================

    /// Capture an error and create a report.
    ///
    /// Returns the path to the saved report, or `None` if saving failed.
    pub fn capture_error(
        &self,
        err: &dyn std::error::Error,
        context: &str,
    ) -> Option<PathBuf> {
        let backtrace = std::backtrace::Backtrace::force_capture();

        let report = ErrorReport {
            timestamp: Utc::now().to_rfc3339(),
            app_version: self.app_version.clone(),
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
            platform: PlatformInfo {
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
            },
            error: ErrorDetails {
                error_type: format!("{:?}", err),
                message: err.to_string(),
                backtrace: format!("{backtrace}"),
            },
            context: context.to_string(),
            recent_actions: self
                .recent_actions
                .lock()
                .map(|a| a.clone())
                .unwrap_or_default(),
        };

        // Sanitize sensitive data
        let report = self.sanitize_report(report);

        self.save_report(&report)
    }

    fn sanitize_report(&self, mut report: ErrorReport) -> ErrorReport {
        report.error.backtrace = sanitize_paths(&report.error.backtrace);
        report.error.message = redact_sensitive(&report.error.message);
        report.context = redact_sensitive(&report.context);
        report.recent_actions = report
            .recent_actions
            .into_iter()
            .map(|a| redact_sensitive(&a))
            .collect();
        report
    }

    fn save_report(&self, report: &ErrorReport) -> Option<PathBuf> {
        let filename = format!(
            "errors_{}.json",
            Utc::now().format("%Y%m%d")
        );
        let filepath = self.error_dir.join(&filename);

        // Load existing errors for today
        let mut errors: Vec<serde_json::Value> = if filepath.exists() {
            std::fs::read_to_string(&filepath)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Add new error (keep last 100)
        if let Ok(value) = serde_json::to_value(report) {
            errors.push(value);
            if errors.len() > 100 {
                errors.drain(..errors.len() - 100);
            }
        }

        // Save
        match serde_json::to_string_pretty(&errors) {
            Ok(json) => {
                std::fs::write(&filepath, json).ok();
                Some(filepath)
            }
            Err(e) => {
                error!("Failed to serialize error report: {e}");
                None
            }
        }
    }

    // ==================== Recent Errors ====================

    /// Get recent error reports.
    pub fn get_recent_errors(
        &self,
        count: usize,
    ) -> Vec<ErrorReport> {
        let mut errors = Vec::new();

        let mut files: Vec<_> = std::fs::read_dir(&self.error_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("errors_") && n.ends_with(".json"))
                    .unwrap_or(false)
            })
            .collect();

        files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

        for entry in files {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if let Ok(file_errors) =
                    serde_json::from_str::<Vec<ErrorReport>>(&content)
                {
                    errors.extend(file_errors);
                    if errors.len() >= count {
                        break;
                    }
                }
            }
        }

        errors.truncate(count);
        errors
    }
}

/// Sanitize file paths to remove usernames.
fn sanitize_paths(text: &str) -> String {
    let patterns = [
        (r"C:\\Users\\[^\\]+", r"C:\Users\[USER]"),
        (r"/Users/[^/]+", "/Users/[USER]"),
        (r"/home/[^/]+", "/home/[USER]"),
    ];

    let mut result = text.to_string();
    for (pattern, replacement) in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            result = re.replace_all(&result, *replacement).to_string();
        }
    }
    result
}
```

## Panic Handler

Install a custom panic hook at app startup to capture panics before the process aborts:

```rust
use std::panic;
use std::sync::Arc;

/// Install the global panic handler.
///
/// CRITICAL: Call this FIRST in `main()`, before any other initialization.
pub fn install_panic_handler(reporter: Arc<ErrorReporter>) {
    panic::set_hook(Box::new(move |panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        tracing::error!(
            message = %message,
            location = %location,
            "PANIC"
        );

        // Capture as error report
        let err = PanicError {
            message: message.clone(),
            location,
        };
        reporter.capture_error(&err, "Panic in application");

        // Attempt to show error dialog (best-effort)
        show_crash_dialog(&message);
    }));
}

/// A wrapper to make panic info into a std::error::Error.
#[derive(Debug)]
struct PanicError {
    message: String,
    location: String,
}

impl std::fmt::Display for PanicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "panic at {}: {}", self.location, self.message)
    }
}

impl std::error::Error for PanicError {}

fn show_crash_dialog(message: &str) {
    // Platform-specific crash dialog (best-effort)
    #[cfg(target_os = "windows")]
    {
        // Use MessageBoxW for a basic crash dialog
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;

        let text: Vec<u16> = OsStr::new(&format!(
            "The application encountered a fatal error:\n\n{message}\n\n\
             An error report has been saved."
        ))
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

        let caption: Vec<u16> = OsStr::new("Fatal Error")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            winapi::um::winuser::MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                0x10, // MB_ICONERROR
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("FATAL ERROR: {message}");
        eprintln!("An error report has been saved.");
    }
}
```

### Using `catch_unwind` for Recoverable Panics

For operations that should not crash the entire application (e.g., processing user files), wrap them in `catch_unwind`:

```rust
use std::panic;

fn process_file_safely(
    path: &std::path::Path,
    reporter: &ErrorReporter,
) -> Result<(), Box<dyn std::error::Error>> {
    match panic::catch_unwind(panic::AssertUnwindSafe(|| {
        process_file(path)
    })) {
        Ok(result) => result,
        Err(panic_payload) => {
            let message = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic during file processing".to_string()
            };

            let err = PanicError {
                message,
                location: "process_file".to_string(),
            };
            reporter.capture_error(
                &err,
                &format!("Panic while processing {}", path.display()),
            );

            Err("File processing failed due to internal error".into())
        }
    }
}
```

> **Note:** `catch_unwind` only works when `panic = "unwind"` (the default). If you set `panic = "abort"` in `[profile.release]`, panics will terminate the process immediately. Consider using `panic = "unwind"` in release if you need recoverable panics.

## Custom Error Hierarchy

Use `thiserror` for structured, typed errors:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("File operation error: {0}")]
    FileOperation(#[from] std::io::Error),

    #[error("Network error: {message}")]
    Network {
        message: String,
        #[source]
        source: Option<reqwest::Error>,
    },

    #[error("Network timeout after {timeout_seconds}s: {message}")]
    NetworkTimeout {
        message: String,
        timeout_seconds: f64,
    },

    #[error("Validation error on field '{field}': {message}")]
    Validation {
        message: String,
        field: String,
    },

    #[error("Processing error in '{process_type}': {message}")]
    Processing {
        message: String,
        process_type: String,
    },

    #[error("Authentication error: {0}")]
    Authentication(String),
}
```

Using errors with `anyhow` for application-level code:

```rust
use anyhow::{Context, Result};

fn load_config(path: &std::path::Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config from {}", path.display()))?;

    let config: Config = toml::from_str(&content)
        .context("Failed to parse config TOML")?;

    Ok(config)
}
```

## Error Collection Utilities

### ThreadSafeErrorCollector

```rust
use std::sync::Mutex;

/// Collect errors from multiple threads.
pub struct ThreadSafeErrorCollector {
    errors: Mutex<Vec<(String, String)>>,
}

impl ThreadSafeErrorCollector {
    pub fn new() -> Self {
        Self {
            errors: Mutex::new(Vec::new()),
        }
    }

    pub fn add_error(&self, context: &str, error: &dyn std::error::Error) {
        if let Ok(mut errors) = self.errors.lock() {
            errors.push((context.to_string(), error.to_string()));
        }
    }

    pub fn has_errors(&self) -> bool {
        self.errors
            .lock()
            .map(|e| !e.is_empty())
            .unwrap_or(false)
    }

    pub fn get_errors(&self) -> Vec<(String, String)> {
        self.errors
            .lock()
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut errors) = self.errors.lock() {
            errors.clear();
        }
    }
}
```

### BatchErrorCollector

```rust
use std::collections::HashMap;

/// Collect errors during batch operations.
pub struct BatchErrorCollector {
    errors: HashMap<String, Vec<String>>,
}

impl BatchErrorCollector {
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
        }
    }

    pub fn add_error(&mut self, item_id: &str, error: &str) {
        self.errors
            .entry(item_id.to_string())
            .or_default()
            .push(error.to_string());
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.errors.values().map(|v| v.len()).sum()
    }

    pub fn failed_items(&self) -> Vec<&str> {
        self.errors.keys().map(|s| s.as_str()).collect()
    }

    pub fn summary(&self) -> String {
        if self.errors.is_empty() {
            "No errors".to_string()
        } else {
            format!(
                "Failed: {} items with {} errors",
                self.errors.len(),
                self.error_count()
            )
        }
    }

    pub fn clear(&mut self) {
        self.errors.clear();
    }
}
```

## Usage in Main

```rust
use std::path::Path;
use std::sync::Arc;

fn main() {
    // 1. FIRST: Initialize tracing
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| ".".into())
        .join("myapp")
        .join("logs");
    let is_dev = cfg!(debug_assertions);
    init_tracing(&log_dir, is_dev);

    // 2. Initialize error reporter
    let error_dir = dirs::data_dir()
        .unwrap_or_else(|| ".".into())
        .join("myapp")
        .join("error_logs");
    let reporter = Arc::new(ErrorReporter::new(
        &error_dir,
        env!("CARGO_PKG_VERSION"),
    ));

    // 3. Install panic handler
    install_panic_handler(Arc::clone(&reporter));

    tracing::info!("Application starting");

    // 4. Run the application
    if let Err(e) = run_app(&reporter) {
        tracing::error!(error = %e, "Fatal error during startup");
        reporter.capture_error(e.as_ref(), "Startup failure");
        std::process::exit(1);
    }
}

fn run_app(
    reporter: &ErrorReporter,
) -> Result<(), Box<dyn std::error::Error>> {
    // Application initialization and main loop...
    Ok(())
}
```

## Error Dialog UI

A user-friendly dialog that displays when unhandled errors occur.

### Dialog Layout

```
+-------------------------------------------------------------+
|                  Something went wrong                         |
|                                                               |
|  A required file could not be found.                          |
|                                                               |
|  +-------------------------------------------------------+   |
|  | Technical Details:                                     |   |
|  | +-------------------------------------------------+   |   |
|  | |    0: your_app::config::load_config              |   |   |
|  | |           at src/config.rs:45                    |   |   |
|  | |    1: your_app::main                             |   |   |
|  | |           at src/main.rs:23                      |   |   |
|  | |  ConfigError: config.json not found              |   |   |
|  | +-------------------------------------------------+   |   |
|  +-------------------------------------------------------+   |
|                                                               |
|  [Copy Details]  [Send Report]                    [Close]     |
|                                                               |
|  Error report automatically sent to development team.         |
+-------------------------------------------------------------+
```

### User-Friendly Messages

The dialog translates common error types:

| Error Type | User Message |
|------------|--------------|
| `std::io::Error (NotFound)` | "A required file could not be found." |
| `std::io::Error (PermissionDenied)` | "Permission denied while accessing a file or resource." |
| `reqwest::Error` (connect) | "Could not connect to the network or server." |
| `NetworkTimeout` | "The operation timed out. Please try again." |
| `Validation` | "An invalid value was encountered." |
| `Config` | "A required configuration value is missing or invalid." |

### Iced Popup Window Error Dialog

In iced daemon apps, show error dialogs as popup windows:

```rust
use iced::window;
use iced::Size;

// Error dialog as iced daemon popup window
Message::ShowError(report) => {
    let (id, task) = window::open(window::Settings {
        size: Size::new(500.0, 350.0),
        resizable: false,
        exit_on_close_request: false,
        ..Default::default()
    });
    self.windows.insert(id, WindowKind::ErrorDialog(report));
    task.discard()
}
```

For simple error popups without needing a full iced window, use `rfd::MessageDialog`:

```rust
use rfd::MessageDialog;
use rfd::MessageLevel;

fn show_simple_error(title: &str, message: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(MessageLevel::Error)
        .show();
}
```

### User-Friendly Message Helper

```rust
fn user_friendly_message(err: &dyn std::error::Error) -> String {
    let msg = err.to_string().to_lowercase();
    if msg.contains("not found") || msg.contains("no such file") {
        "A required file could not be found.".to_string()
    } else if msg.contains("permission denied") {
        "Permission denied while accessing a file or resource.".to_string()
    } else if msg.contains("connect") || msg.contains("network") {
        "Could not connect to the network or server.".to_string()
    } else if msg.contains("timeout") || msg.contains("timed out") {
        "The operation timed out. Please try again.".to_string()
    } else {
        format!("An unexpected error occurred: {}", err)
    }
}
```

---

## Related Documentation

- [RESILIENCE.md](RESILIENCE.md) - Circuit breakers, fallbacks, and health monitoring
- [TESTING.md](TESTING.md) - Testing error handling
- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) - Configuration for error reporting
