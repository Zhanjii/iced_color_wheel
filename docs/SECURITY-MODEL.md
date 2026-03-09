# Remote PIN Activation Security Model

This document describes the security architecture for internally-distributed desktop applications using remote PIN-based activation.

## Security Philosophy

**Remote PIN activation is the primary security layer, not local encryption.**

This model is designed for:
- Internal team tools distributed as executables
- Apps where all team members need the same API access
- Situations where you need a kill switch capability

### What This Means

The following are **intentionally bundled** and are NOT security issues:
- Encryption keys with encrypted API credentials
- OAuth client_secret stored in JSON files
- API configuration in bundled files

Security comes from:
1. Remote PIN validation before app can be used
2. Kill switch via GitHub config change
3. Minimum version enforcement
4. Offline grace period limits

## Remote Config Repository

Create a **public** GitHub repository for your app configuration:

```
your-org/your-app-config/
├── app_access.json    # Activation settings
└── README.md          # Optional documentation
```

### app_access.json Structure

```json
{
    "enabled": true,
    "activation_code": "your-secret-pin",
    "min_app_version": "1.0.0",
    "block_message": "This version is no longer supported. Please update.",
    "invalid_code_message": "Invalid activation code. Please contact support.",
    "success_message": "App activated successfully!"
}
```

## Activation States

```rust
/// Possible activation states for the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationStatus {
    /// Successfully activated.
    Activated,
    /// First run, needs activation code.
    CodeRequired,
    /// Stored code no longer matches remote.
    CodeInvalid,
    /// App version below minimum.
    VersionBlocked,
    /// App globally disabled via kill switch.
    AppDisabled,
    /// Cannot reach GitHub (may use grace period).
    NetworkError,
}
```

## Activation Flow

```
App Start
    │
    ▼
┌─────────────────────────┐
│ Fetch remote config     │──── Network Error ───► Check Grace Period
│ from GitHub             │                              │
└───────────┬─────────────┘                              ▼
            │                                    ┌───────────────┐
            ▼                                    │ Within 7 days │
      ┌───────────┐                              │ of last       │
      │ enabled?  │──── No ───► APP_DISABLED     │ activation?   │
      └─────┬─────┘                              └───────┬───────┘
            │ Yes                                   Yes │ No
            ▼                                        │  │
      ┌─────────────────┐                            │  ▼
      │ version >= min? │── No ──► VERSION_BLOCKED   │ NETWORK_ERROR
      └───────┬─────────┘                            │
              │ Yes                                  ▼
              ▼                                 ACTIVATED
       ┌────────────────┐                      (offline mode)
       │ Has stored     │
       │ activation?    │
       └───────┬────────┘
          Yes  │  No
               │   │
               │   ▼
               │  CODE_REQUIRED
               │  (show input dialog)
               ▼
       ┌────────────────┐
       │ Stored hash    │
       │ matches remote?│
       └───────┬────────┘
          Yes  │  No
               │   │
               ▼   ▼
         ACTIVATED  CODE_INVALID
```

## Implementation

### Activation Manager

```rust
use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Remote configuration fetched from GitHub.
#[derive(Debug, Deserialize)]
struct RemoteConfig {
    enabled: Option<bool>,
    activation_code: Option<String>,
    min_app_version: Option<String>,
    block_message: Option<String>,
    invalid_code_message: Option<String>,
    success_message: Option<String>,
}

/// Manages app activation via remote PIN validation.
pub struct ActivationManager {
    remote_config_url: String,
    grace_period_days: i64,
    config: ConfigManager,
    remote: Option<RemoteConfig>,
}

impl ActivationManager {
    const DEFAULT_GRACE_PERIOD_DAYS: i64 = 7;

    pub fn new(config: ConfigManager, remote_url: &str) -> Self {
        Self {
            remote_config_url: remote_url.to_string(),
            grace_period_days: Self::DEFAULT_GRACE_PERIOD_DAYS,
            config,
            remote: None,
        }
    }

    /// Check current activation status.
    pub fn check_activation(&mut self) -> ActivationStatus {
        let app_version = env!("CARGO_PKG_VERSION");

        // Try to fetch remote config
        let remote = match self.fetch_remote_config() {
            Some(r) => r,
            None => return self.check_grace_period(),
        };

        // Check kill switch
        if !remote.enabled.unwrap_or(true) {
            self.remote = Some(remote);
            return ActivationStatus::AppDisabled;
        }

        // Check minimum version
        let min_version = remote.min_app_version.as_deref().unwrap_or("0.0.0");
        if !version_allowed(app_version, min_version) {
            self.remote = Some(remote);
            return ActivationStatus::VersionBlocked;
        }

        // Check activation code
        let stored_hash = self.config.get::<String>("activation_code_hash");
        let stored_hash = match stored_hash {
            Some(h) => h,
            None => {
                self.remote = Some(remote);
                return ActivationStatus::CodeRequired;
            }
        };

        // Verify stored hash matches current remote code
        let remote_code = remote.activation_code.as_deref().unwrap_or("");
        let expected_hash = hash_code(remote_code);

        if stored_hash != expected_hash {
            self.remote = Some(remote);
            return ActivationStatus::CodeInvalid;
        }

        // Update activation timestamp for grace period
        self.config.set("activation_timestamp", &Utc::now().to_rfc3339());
        self.remote = Some(remote);
        ActivationStatus::Activated
    }

    /// Attempt activation with provided code.
    pub fn activate_with_code(&mut self, code: &str) -> bool {
        let Some(ref remote) = self.remote else {
            return false;
        };

        let expected_code = remote.activation_code.as_deref().unwrap_or("");

        if code == expected_code {
            self.config.set("activation_code_hash", &hash_code(code));
            self.config.set("activation_timestamp", &Utc::now().to_rfc3339());
            true
        } else {
            false
        }
    }

    fn fetch_remote_config(&self) -> Option<RemoteConfig> {
        let client = Client::new();
        let resp = client
            .get(&self.remote_config_url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .ok()?;

        resp.json::<RemoteConfig>().ok()
    }

    fn check_grace_period(&self) -> ActivationStatus {
        let timestamp: Option<String> = self.config.get("activation_timestamp");
        let Some(ts) = timestamp else {
            return ActivationStatus::NetworkError;
        };

        let Ok(last_activation) = DateTime::parse_from_rfc3339(&ts) else {
            return ActivationStatus::NetworkError;
        };

        let grace_end = last_activation + Duration::days(self.grace_period_days);
        if Utc::now() < grace_end {
            ActivationStatus::Activated
        } else {
            ActivationStatus::NetworkError
        }
    }

    /// Get user-facing message for status.
    pub fn get_message(&self, status: &ActivationStatus) -> String {
        let Some(ref remote) = self.remote else {
            return match status {
                ActivationStatus::NetworkError => {
                    "Unable to verify activation. Please check your internet connection.".into()
                }
                _ => String::new(),
            };
        };

        match status {
            ActivationStatus::AppDisabled => remote
                .block_message
                .clone()
                .unwrap_or_else(|| "This application has been disabled.".into()),
            ActivationStatus::VersionBlocked => remote
                .block_message
                .clone()
                .unwrap_or_else(|| "Please update to the latest version.".into()),
            ActivationStatus::CodeInvalid => remote
                .invalid_code_message
                .clone()
                .unwrap_or_else(|| "Your activation code is no longer valid.".into()),
            ActivationStatus::Activated => remote
                .success_message
                .clone()
                .unwrap_or_else(|| "Activated successfully!".into()),
            _ => String::new(),
        }
    }
}

/// Create SHA-256 hash of activation code.
fn hash_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Check if current version meets minimum requirement.
fn version_allowed(current: &str, minimum: &str) -> bool {
    let normalize = |v: &str| -> Option<Vec<u32>> {
        let v = v.split('-').next()?; // Strip pre-release suffix
        v.split('.').map(|s| s.parse().ok()).collect()
    };

    match (normalize(current), normalize(minimum)) {
        (Some(curr), Some(min)) => curr >= min,
        _ => false,
    }
}
```

### Activation Dialog (iced daemon)

In an iced daemon application, the activation dialog is shown as a popup window:

```rust
use iced::window;
use iced::Size;

// Open activation dialog as a new iced window
Message::ShowActivationDialog => {
    let (id, task) = window::open(window::Settings {
        size: Size::new(400.0, 250.0),
        resizable: false,
        exit_on_close_request: false,
        ..Default::default()
    });
    self.windows.insert(id, WindowKind::ActivationDialog);
    task.discard()
}

// Handle activation code submission
Message::SubmitActivationCode(code) => {
    let mut mgr = self.activation_manager.lock().unwrap();
    if mgr.activate_with_code(&code) {
        self.status_message = Some(mgr.get_message(&ActivationStatus::Activated));
        // Close activation window, open main window
    } else {
        self.status_message = Some(mgr.get_message(&ActivationStatus::CodeInvalid));
    }
    Task::none()
}
```

## Kill Switch Usage

To disable the app for all users:

1. Edit `app_access.json` in your config repo
2. Set `"enabled": false`
3. Commit and push

All running instances will fail activation on next check.

## Version Blocking

To force users to update:

1. Edit `app_access.json`
2. Set `"min_app_version": "2.0.0"` (or desired minimum)
3. Commit and push

Users on older versions will see the block message.

## SECURITY.md Template

Include this in your project root:

```markdown
# Security Model

This application uses a **remote PIN activation** security model.

## How It Works

1. On first run, users enter a team-shared activation PIN
2. The PIN is validated against a remote configuration
3. A hash of the PIN is stored locally (never the PIN itself)
4. The app can be remotely disabled via configuration change

## What Is NOT a Security Issue

The following are intentionally bundled for team access:

- Encryption keys in `keys/` directory
- OAuth client credentials
- API configuration files

Security is enforced via:
- Remote PIN validation (not local encryption)
- Kill switch capability
- Minimum version enforcement
- 7-day offline grace period

## Reporting Issues

If you discover a security vulnerability that could affect
unauthorized access, please report it to [security contact].
```

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [VERSIONING.md](VERSIONING.md) - Version management
- [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) - Configuration storage
