# Quick Start: Building a Desktop App

This guide walks you through creating a new Rust desktop application using iced (daemon mode) with the architectural patterns in this template.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Project Setup](#step-1-project-setup)
3. [Version Management](#step-2-version-management)
4. [Create Main Window](#step-3-create-main-window)
5. [Add Utility Modules](#step-4-add-utility-modules)
6. [Add Settings Dialog](#step-5-add-settings-dialog)
7. [PIN Activation (Optional)](#step-6-add-pin-activation-optional)
8. [System Tray (Optional)](#step-7-add-system-tray-optional)
9. [API & Credentials](#step-8-api--credentials-optional)
10. [Testing Setup](#step-9-add-tests)
11. [Changelog & GitHub Pages](#step-10-changelog--github-pages)
12. [Build & Distribute](#step-11-build--distribute)
13. [GitHub Actions CI/CD](#step-12-github-actions-cicd)
14. [Next Steps](#next-steps)

---

## Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Git
- A code editor (VS Code with rust-analyzer recommended)
- GitHub account (for CI/CD and Pages)

---

## Step 1: Project Setup

```bash
# Create project
cargo new my-app
cd my-app

# Initialize git (cargo new does this automatically)
git init  # only if needed

# Create directory structure
mkdir -p src/core
mkdir -p src/utils
mkdir -p src/ui
mkdir -p assets/icon
mkdir -p tests/common
mkdir -p benches
mkdir -p docs
mkdir -p .github/workflows
```

### Cargo.toml

```toml
[package]
name = "my-app"
version = "1.0.0-dev.0"
edition = "2021"
description = "My Desktop Application"
license = "MIT"
rust-version = "1.75"

[dependencies]
# GUI
iced = { version = "0.14", features = ["multi-window", "canvas", "tokio"] }
iced_color_wheel = "0.1"

# Core utilities
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
directories = "5"
semver = "1"

# Async / HTTP
reqwest = { version = "0.12", features = ["json"] }

# File dialogs
rfd = "0.15"

# System tray (optional)
tray-icon = "0.19"
image = "0.25"

# Notifications (optional)
notify-rust = "4"

[dev-dependencies]
tempfile = "3"
mockall = "0.13"
criterion = "0.5"
serial_test = "3"

[[bench]]
name = "benchmarks"
harness = false

[profile.release]
opt-level = "z"
lto = true
strip = true
codegen-units = 1
```

### Build the Project

```bash
cargo build
cargo run
```

---

## Step 2: Version Management

Manage version in `Cargo.toml` (the single source of truth in Rust):

```toml
[package]
version = "1.0.0-dev.0"
```

Create `src/version.rs` for runtime version checks:

```rust
/// The application version, read from Cargo.toml at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if running in development mode.
///
/// Development mode is indicated by a pre-release suffix (e.g., "1.0.0-dev.0").
/// This affects:
/// - Skip pushing config changes to remote
/// - Skip clearing credentials on version change
/// - Show [DEV] in window title
pub fn is_development_mode() -> bool {
    VERSION.contains("-dev")
}
```

**Version Conventions:**
- `1.0.0-dev.0` - Development version
- `1.0.0` - Production release
- `1.0.1` - Patch release (bug fixes)
- `1.1.0` - Minor release (new features)
- `2.0.0` - Major release (breaking changes)

---

## Step 3: Create Main Window

Create `src/main.rs`:

```rust
use std::collections::HashMap;
use iced::widget::{button, column, container, horizontal_space, row, text};
use iced::window;
use iced::{daemon, Element, Size, Task, Theme};

mod version;
mod utils;

use version::{VERSION, is_development_mode};

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(if is_development_mode() { "debug" } else { "info" })
        .init();

    tracing::info!("Starting application v{VERSION}");

    daemon("My App", App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .run_with(App::new)
}

struct App {
    windows: HashMap<window::Id, WindowKind>,
    main_id: window::Id,
    config: ConfigManager,
    session: SessionManager,
}

enum WindowKind {
    Main,
    Settings(SettingsState),
    About,
}

#[derive(Debug, Clone)]
enum Message {
    OpenSettings,
    OpenAbout,
    CloseWindow(window::Id),
    WindowClosed(window::Id),
    Quit,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let session = SessionManager::load().unwrap_or_default();
        let size = session.window_size.unwrap_or(Size::new(1024.0, 768.0));

        let (main_id, open) = window::open(window::Settings {
            size,
            min_size: Some(Size::new(800.0, 600.0)),
            exit_on_close_request: false,
            ..Default::default()
        });

        let app = Self {
            windows: HashMap::from([(main_id, WindowKind::Main)]),
            main_id,
            config: ConfigManager::load_or_default(),
            session,
        };

        (app, open.discard())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenSettings => {
                let (id, task) = window::open(window::Settings {
                    size: Size::new(500.0, 400.0),
                    resizable: false,
                    exit_on_close_request: false,
                    ..Default::default()
                });
                self.windows.insert(id, WindowKind::Settings(SettingsState::default()));
                task.discard()
            }
            Message::OpenAbout => {
                let (id, task) = window::open(window::Settings {
                    size: Size::new(400.0, 300.0),
                    resizable: false,
                    exit_on_close_request: false,
                    ..Default::default()
                });
                self.windows.insert(id, WindowKind::About);
                task.discard()
            }
            Message::CloseWindow(id) => {
                self.windows.remove(&id);
                window::close(id)
            }
            Message::WindowClosed(id) => {
                self.windows.remove(&id);
                if id == self.main_id {
                    let _ = self.session.save();
                    return iced::exit();
                }
                Task::none()
            }
            Message::Quit => {
                let _ = self.session.save();
                iced::exit()
            }
        }
    }

    fn view(&self, id: window::Id) -> Element<Message> {
        match self.windows.get(&id) {
            Some(WindowKind::Main) => {
                let title = if is_development_mode() {
                    format!("My App v{VERSION} [DEV]")
                } else {
                    format!("My App v{VERSION}")
                };

                container(
                    column![
                        row![
                            text(title).size(24),
                            horizontal_space(),
                            button("Settings").on_press(Message::OpenSettings),
                        ].spacing(8),
                        iced::widget::horizontal_rule(1),
                        text("Welcome to My App!"),
                    ]
                    .spacing(12)
                    .padding(20)
                )
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into()
            }
            Some(WindowKind::Settings(_state)) => {
                container(
                    column![
                        text("Settings").size(20),
                        text("Configure your app here"),
                        button("Close").on_press(Message::CloseWindow(id)),
                    ]
                    .spacing(12)
                    .padding(20)
                )
                .into()
            }
            Some(WindowKind::About) => {
                container(
                    column![
                        text("My App").size(24),
                        text(format!("Version {VERSION}")),
                        button("Close").on_press(Message::CloseWindow(id)),
                    ]
                    .spacing(8)
                    .padding(20)
                )
                .into()
            }
            None => text("Unknown window").into(),
        }
    }

    fn theme(&self, _id: window::Id) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        window::close_events().map(Message::WindowClosed)
    }
}
```

---

## Step 4: Add Utility Modules

Create modules under `src/utils/`:

| Template File | Destination | Purpose |
|--------------|-------------|---------|
| `config.rs` | `utils/config.rs` | Configuration management (serde + JSON) |
| `session_manager.rs` | `utils/session.rs` | Window state persistence |
| `logging_setup.rs` | `utils/logging.rs` | Tracing subscriber setup |
| `error_reporter.rs` | `utils/error_reporter.rs` | Crash reporting |
| `error_handling.rs` | `utils/errors.rs` | Custom error types with `thiserror` |
| `system_tray.rs` | `utils/system_tray.rs` | Tray icon manager |

Create `src/utils/mod.rs`:

```rust
pub mod config;
pub mod session;
pub mod logging;
pub mod error_reporter;
pub mod errors;
pub mod system_tray;
```

---

## Step 5: Add Settings Dialog

Settings are displayed in a separate popup window using `window::open()`. The `App::update` handler already opens a settings window via `Message::OpenSettings`. Extend the settings window view with your configuration fields:

```rust
// src/ui/settings.rs

use iced::widget::{button, checkbox, column, container, pick_list, row, text};
use iced::{Element, window};

use crate::utils::config::ConfigManager;

#[derive(Debug, Clone, Default)]
pub struct SettingsState {
    pub check_updates: bool,
    pub close_to_tray: bool,
    pub theme: ThemeChoice,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
    System,
}

impl std::fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dark => write!(f, "Dark"),
            Self::Light => write!(f, "Light"),
            Self::System => write!(f, "System"),
        }
    }
}

impl ThemeChoice {
    pub const ALL: &'static [Self] = &[Self::Dark, Self::Light, Self::System];
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    ToggleCheckUpdates(bool),
    ToggleCloseToTray(bool),
    ThemeChanged(ThemeChoice),
    Save,
    Cancel,
}

impl SettingsState {
    pub fn from_config(config: &ConfigManager) -> Self {
        Self {
            check_updates: config.get_bool("check_updates", true),
            close_to_tray: config.get_bool("close_to_tray", false),
            theme: ThemeChoice::Dark,
        }
    }

    pub fn view(&self, window_id: window::Id) -> Element<crate::Message> {
        container(
            column![
                text("General").size(20),
                checkbox("Check for updates on startup", self.check_updates),
                checkbox("Minimize to system tray on close", self.close_to_tray),
                text("Appearance").size(20),
                row![
                    text("Theme:"),
                    pick_list(ThemeChoice::ALL, Some(self.theme), |choice| {
                        crate::Message::SettingsChanged(SettingsMessage::ThemeChanged(choice))
                    }),
                ].spacing(8),
                row![
                    button("Save").on_press(crate::Message::SettingsChanged(SettingsMessage::Save)),
                    button("Cancel").on_press(crate::Message::CloseWindow(window_id)),
                ].spacing(8),
            ]
            .spacing(12)
            .padding(20)
        )
        .into()
    }

    pub fn apply_to_config(&self, config: &mut ConfigManager) {
        config.set_bool("check_updates", self.check_updates);
        config.set_bool("close_to_tray", self.close_to_tray);
        config.set_string("theme", &self.theme.to_string());
    }
}
```

To integrate this, add `SettingsChanged(SettingsMessage)` to your `Message` enum and handle it in `App::update`. The settings window is opened via `window::open()` and closed via `window::close()`, keeping it consistent with the multi-window daemon pattern.

---

## Step 6: Add PIN Activation (Optional)

For remote PIN-based activation (useful for team-distributed apps):

### 6.1 Create Config Repository

1. Create a new GitHub repository (e.g., `my-app-config`)
2. Add `app_access.json`:

```json
{
    "enabled": true,
    "activation_code": "your-secret-pin-here",
    "min_app_version": "1.0.0",
    "block_message": "This version is no longer supported.",
    "invalid_code_message": "Invalid activation code.",
    "success_message": "Application activated successfully!"
}
```

### 6.2 Add Activation Module

Copy `templates/app_activation.rs` to `src/utils/app_activation.rs`.

Update the config URL:
```rust
const CONFIG_URL: &str =
    "https://raw.githubusercontent.com/your-org/my-app-config/main/app_access.json";
```

### 6.3 Add Activation Check to Main

```rust
fn main() -> iced::Result {
    // 1. Initialize logging
    init_logging();

    // 2. Initialize config
    let config = ConfigManager::load_or_default();

    // 3. Check activation
    let activation = ActivationManager::new(&config);
    if !activation.check_activation().unwrap_or(false) {
        // Show activation dialog or exit
        return Ok(());
    }

    // 4. Create and run app via daemon
    daemon("My App", App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .run_with(App::new)
}
```

See [SECURITY-MODEL.md](SECURITY-MODEL.md) for full details.

---

## Step 7: Add System Tray (Optional)

Copy `templates/system_tray.rs` to `src/utils/system_tray.rs`.

The tray icon uses the `tray-icon` crate directly. Ensure you have an icon file at `assets/icon/app.png` (or `app.ico` for Windows).

Because the app runs in daemon mode, it does not exit when all windows are closed. This pairs naturally with a system tray: closing the main window hides the app to the tray, and the user can reopen it from the tray menu. Adjust the `Message::WindowClosed` handler to skip calling `iced::exit()` when `close_to_tray` is enabled, and instead just remove the window from the `windows` map.

See [SYSTEM-TRAY.md](SYSTEM-TRAY.md) for full details.

---

## Step 8: API & Credentials (Optional)

For apps that need to store API keys or authenticate with services:

### 8.1 Keyring for User Credentials

Use the `keyring` crate for OS-native credential storage:

```rust
use keyring::Entry;

// Store API key securely
let entry = Entry::new("my_app", "api_key")?;
entry.set_password("user_provided_key")?;

// Retrieve later
let api_key = entry.get_password()?;
```

Add to `Cargo.toml`:
```toml
[dependencies]
keyring = "3"
```

### 8.2 Encrypted Bundled Credentials

For credentials bundled with the app:

```rust
use your_app::utils::encryption::EncryptionManager;
use std::path::Path;

// 1. Generate encryption key (do once, at build time)
EncryptionManager::generate_key(Path::new("keys/encryption.key"))?;

// 2. Encrypt your credentials
let enc = EncryptionManager::new(Path::new("keys/encryption.key"))?;
enc.encrypt_to_file(
    &serde_json::json!({"api_key": "secret", "api_secret": "secret2"}),
    Path::new("keys/credentials.enc"),
)?;

// 3. Load at runtime
let creds: serde_json::Value = enc.decrypt_from_file(
    Path::new("keys/credentials.enc"),
)?;
```

### 8.3 OAuth Integration

Copy `templates/oauth_client.rs` to `src/utils/oauth_client.rs`.

See [API-CREDENTIALS.md](API-CREDENTIALS.md) for full details.

---

## Step 9: Add Tests

### 9.1 Create Inline Unit Tests

Add `#[cfg(test)]` modules directly in your source files:

```rust
// src/utils/config.rs

// ... (implementation code) ...

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config() -> (ConfigManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let config = ConfigManager::new(dir.path());
        (config, dir) // dir must live as long as config
    }

    #[test]
    fn test_get_default() {
        let (config, _dir) = test_config();
        assert_eq!(config.get_or_default("nonexistent", "default"), "default");
    }

    #[test]
    fn test_set_and_get() {
        let (mut config, _dir) = test_config();
        config.set_string("key", "value");
        assert_eq!(config.get_string("key", ""), "value");
    }
}
```

### 9.2 Create Integration Tests

Create `tests/common/mod.rs` for shared helpers:

```rust
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct TestFixture {
    pub temp_dir: TempDir,
}

impl TestFixture {
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().unwrap(),
        }
    }

    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }
}
```

### 9.3 Run Tests

```bash
cargo test                    # Run all tests
cargo test -- --nocapture     # With stdout output
cargo test --lib              # Unit tests only
cargo test --test '*'         # Integration tests only
cargo tarpaulin --out html    # With coverage
```

See [TESTING.md](TESTING.md) for full testing patterns.

---

## Step 10: Changelog & GitHub Pages

### 10.1 Setup Commit Convention

Use conventional commits for automatic changelog generation:

```bash
# Features
git commit -m "feat: Add dark mode toggle"

# Bug fixes
git commit -m "fix: Correct file path on Windows"

# Performance
git commit -m "perf: Speed up image loading"

# Internal (excluded from changelog)
git commit -m "docs: Update README"
git commit -m "test: Add unit tests"
git commit -m "chore: Update dependencies"
```

### 10.2 Add Changelog Script

Copy `templates/generate_changelog.rs` to `src/bin/generate_changelog.rs`, or use [git-cliff](https://git-cliff.org/) for automated changelog generation.

Add to `Cargo.toml`:
```toml
[[bin]]
name = "generate-changelog"
path = "src/bin/generate_changelog.rs"
```

Test it:
```bash
cargo run --bin generate-changelog -- --dry-run
```

### 10.3 Setup GitHub Pages

1. **Create Personal Access Token:**
   - Go to https://github.com/settings/tokens
   - Generate new token with `repo` scope
   - Copy the token

2. **Create Public Docs Repository:**
   - Create `your-username/my-app-docs` (public)
   - Enable GitHub Pages: Settings > Pages > Source: main

3. **Add Token to Source Repo:**
   - Go to your app repo > Settings > Secrets > Actions
   - Add secret: `DOCS_DEPLOY_TOKEN` with the token

4. **Add Workflow:**

Copy `templates/sync-docs.yml` to `.github/workflows/sync-docs.yml`.

### 10.4 Create a Release

```bash
# Update version in Cargo.toml to 1.0.0
# Commit
git add Cargo.toml
git commit -m "chore(release): Bump version to 1.0.0"

# Create tag
git tag v1.0.0
git push origin main
git push origin v1.0.0
```

The workflow automatically generates changelog and publishes to GitHub Pages.

See [CHANGELOG-SETUP.md](CHANGELOG-SETUP.md) for full details.

---

## Step 11: Build & Distribute

### 11.1 Release Build

```bash
# Optimized release build
cargo build --release

# Output is at: target/release/my-app (or my-app.exe on Windows)
```

### 11.2 Cross-Compilation

Use [cross](https://github.com/cross-rs/cross) for building on other platforms:

```bash
cargo install cross
cross build --release --target x86_64-pc-windows-gnu
cross build --release --target x86_64-apple-darwin
cross build --release --target x86_64-unknown-linux-gnu
```

### 11.3 Packaging

For distributing as platform-native installers, use [`cargo-bundle`](https://github.com/burtonageo/cargo-bundle):

```bash
cargo install cargo-bundle
cargo bundle --release
# Produces: .app (macOS), .deb (Linux)
```

For Windows `.msi` installers, use [WiX Toolset](https://wixtoolset.org/) or [NSIS](https://nsis.sourceforge.io/) with manual packaging scripts. For `.AppImage` on Linux, see [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy).

Alternatively, package manually by placing the release binary alongside your assets in a zip or tarball for each target platform.

See [BUILD-DISTRIBUTION.md](BUILD-DISTRIBUTION.md) for full details including auto-update.

---

## Step 12: GitHub Actions CI/CD

Create `.github/workflows/test.yml`:

```yaml
name: CI

on:
  push:
    branches: [main, master, develop]
  pull_request:
    branches: [main, master]

jobs:
  check:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
        rust: [stable, "1.75"]  # MSRV + stable

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
          components: clippy, rustfmt

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Run tests
        run: cargo test --verbose

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-tarpaulin
      - run: cargo tarpaulin --out xml
      - uses: codecov/codecov-action@v4
```

---

## Next Steps

Your app is now set up with:
- [x] Version management with dev mode
- [x] Main window with session restore (iced daemon, multi-window)
- [x] Configuration system (serde + JSON)
- [x] Logging with tracing
- [x] Error reporting
- [x] Settings as a popup window
- [x] System tray (optional, pairs with daemon mode)
- [x] Testing setup (cargo test + criterion)
- [x] Changelog generation
- [x] GitHub Pages documentation
- [x] CI/CD pipeline
- [x] Release build configuration

### What to Add Next

1. **Your Application Features** in `src/core/`
2. **Additional Windows** via `window::open()` and the `WindowKind` enum
3. **More Settings** in the settings popup window
4. **Auto-Update** - see `templates/auto_update.rs`
5. **Notifications** - see `templates/notifications.rs`
6. **Plugin System** - see `templates/plugin_loader.rs`

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Project structure overview |
| [VERSIONING.md](VERSIONING.md) | Version management patterns |
| [SECURITY-MODEL.md](SECURITY-MODEL.md) | PIN activation system |
| [UI-ARCHITECTURE.md](UI-ARCHITECTURE.md) | Window and dialog patterns |
| [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) | Configuration management |
| [SYSTEM-TRAY.md](SYSTEM-TRAY.md) | Tray icon integration |
| [CROSS-PLATFORM.md](CROSS-PLATFORM.md) | Multi-OS support |
| [TESTING.md](TESTING.md) | Testing patterns |
| [ERROR-REPORTING.md](ERROR-REPORTING.md) | Logging and crash reports |
| [API-CREDENTIALS.md](API-CREDENTIALS.md) | Credential management |
| [BUILD-DISTRIBUTION.md](BUILD-DISTRIBUTION.md) | Build and release |
| [CHANGELOG-SETUP.md](CHANGELOG-SETUP.md) | Changelog and GitHub Pages |
| [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md) | Plugins, notifications, etc. |
