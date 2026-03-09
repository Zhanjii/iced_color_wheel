# Version Management & Development Mode

This document describes the versioning system and development mode detection patterns for Rust projects.

## Single Source of Truth

The version string lives in one place: `Cargo.toml`

```toml
[package]
name = "app-name"
version = "1.0.0"
edition = "2021"
description = "Description of your application."
```

At compile time, the version is embedded via the `env!` macro:

```rust
/// The application version, read from Cargo.toml at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if the application is running in development mode.
///
/// Development mode is indicated by a pre-release suffix in the version string
/// (e.g., `1.0.0-dev.0` is development, `1.0.0` is production).
pub fn is_development_mode() -> bool {
    VERSION.contains("-dev")
}

/// Parse version string into a comparable tuple.
///
/// Returns `(major, minor, patch)` or `None` if parsing fails.
pub fn get_version_tuple() -> Option<(u32, u32, u32)> {
    let base = VERSION.split('-').next()?;
    let mut parts = base.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}
```

## Version in Window Title

Display the version prominently in the window title. In an iced daemon application, set the title via the `daemon()` call:

```rust
// In iced daemon, set the title via window::Settings or daemon() call
daemon(
    &if is_development_mode() {
        format!("My App v{VERSION} [DEV]")
    } else {
        format!("My App v{VERSION}")
    },
    App::update,
    App::view,
)
```

## Development vs Production Behavior

Use `is_development_mode()` to change behavior:

### Skip Remote Config Push

```rust
fn save_config_to_remote(&self) -> bool {
    if is_development_mode() {
        tracing::info!("Dev mode: Skipping remote config push");
        return true;
    }

    // Actually push to GitHub in production
    self.push_to_github()
}
```

### More Verbose Logging

```rust
fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let filter = if is_development_mode() {
        // Debug logging in dev
        EnvFilter::new("debug")
    } else {
        // Quieter in production
        EnvFilter::new("warn,app_name=info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
}
```

### Skip Credential Clearing

```rust
fn check_version_upgrade_reset(&self) {
    if is_development_mode() {
        tracing::info!("Dev mode: Skipping credential reset check");
        return;
    }

    let stored_version: Option<String> = self.config.get("last_version");
    if stored_version.as_deref() != Some(VERSION) {
        self.clear_cached_credentials();
        self.config.set("last_version", VERSION);
    }
}
```

### Allow Unsafe Operations

```rust
fn delete_all_data(&self) -> Result<(), AppError> {
    if !is_development_mode() {
        return Err(AppError::Forbidden(
            "This operation is only available in dev mode".into(),
        ));
    }

    self.actually_delete_everything()
}
```

## Version Comparison

For minimum version enforcement:

```rust
use std::cmp::Ordering;

/// Compare two version strings.
///
/// Returns `Ordering::Less`, `Equal`, or `Greater`.
fn compare_versions(v1: &str, v2: &str) -> Ordering {
    let normalize = |v: &str| -> Vec<u32> {
        let base = v.split('-').next().unwrap_or(v);
        base.split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let n1 = normalize(v1);
    let n2 = normalize(v2);
    n1.cmp(&n2)
}

/// Check if current version meets minimum requirement.
fn is_version_allowed(current: &str, minimum: &str) -> bool {
    compare_versions(current, minimum) != Ordering::Less
}
```

## Release Workflow

### Development Cycle

```
1.0.0-dev.0  ->  Development starts
1.0.0-dev.1  ->  Feature additions
1.0.0-dev.2  ->  Bug fixes
1.0.0        ->  Release (remove -dev suffix)
1.0.1-dev.0  ->  Next development cycle
```

### Version Bump Script

Since Cargo.toml is the single source of truth, use a shell script or `cargo-edit`:

```bash
#!/usr/bin/env bash
# bump_version.sh - Bump version number in Cargo.toml
set -euo pipefail

CARGO_TOML="Cargo.toml"

current_version=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Current version: $current_version"

case "${1:-}" in
    major|minor|patch)
        # Use cargo-set-version from cargo-edit
        cargo set-version --bump "$1"
        ;;
    dev)
        # Increment dev number: 1.0.0-dev.0 -> 1.0.0-dev.1
        base=$(echo "$current_version" | sed 's/-dev\..*//')
        dev_num=$(echo "$current_version" | grep -oP 'dev\.\K\d+' || echo "-1")
        new_dev=$((dev_num + 1))
        new_version="${base}-dev.${new_dev}"
        sed -i "s/^version = \".*\"/version = \"${new_version}\"/" "$CARGO_TOML"
        echo "Version bumped to $new_version"
        ;;
    release)
        # Remove -dev suffix: 1.0.0-dev.2 -> 1.0.0
        new_version=$(echo "$current_version" | sed 's/-dev\..*//')
        sed -i "s/^version = \".*\"/version = \"${new_version}\"/" "$CARGO_TOML"
        echo "Version bumped to $new_version"
        ;;
    *)
        echo "Usage: bump_version.sh [major|minor|patch|dev|release]"
        exit 1
        ;;
esac
```

Alternatively, install `cargo-edit` for the `cargo set-version` command:

```bash
cargo install cargo-edit
cargo set-version --bump minor
```

## Using Version at Runtime

```rust
use crate::version::{VERSION, is_development_mode};

// In about dialog
let about_text = format!(
    "App Name\nVersion: {VERSION}\nMode: {}",
    if is_development_mode() { "Development" } else { "Production" }
);

// In error reports
let error_report = serde_json::json!({
    "app_version": VERSION,
    "dev_mode": is_development_mode(),
    // ... other fields
});

// In API requests (for debugging)
let client = reqwest::Client::builder()
    .user_agent(format!("AppName/{VERSION}"))
    .build()?;
```

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [SECURITY-MODEL.md](SECURITY-MODEL.md) - Minimum version enforcement
