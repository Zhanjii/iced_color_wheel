# Rust Desktop App Template

A comprehensive boilerplate for building professional Rust desktop applications using iced daemon (multi-window native GUI), with modern architecture patterns.

## Quick Start

1. **Read the guide:** Start with [QUICK-START.md](QUICK-START.md) for a step-by-step tutorial
2. **Copy templates:** Copy files from `templates/` to your project
3. **Customize:** Update crate names, `Cargo.toml` metadata, and configuration values

---

## Documentation

### Getting Started

| Document | Description |
|----------|-------------|
| [QUICK-START.md](QUICK-START.md) | Step-by-step guide to create a new app from scratch |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Project structure and initialization patterns |
| [VERSIONING.md](VERSIONING.md) | Version management and dev mode detection |

### Core Systems

| Document | Description |
|----------|-------------|
| [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) | Configuration management with file-based storage |
| [UI-ARCHITECTURE.md](UI-ARCHITECTURE.md) | Window, dialog, and component patterns (iced daemon) |
| [UI-COMPONENTS.md](UI-COMPONENTS.md) | Reusable UI components (progress indicators, theming) |
| [tab-drag-reorder.md](tab-drag-reorder.md) | Tab drag-and-drop reordering guide (iced, ghost, persistence) |
| [UTILITIES.md](UTILITIES.md) | Caching, async operations, validation, path security |
| [ERROR-REPORTING.md](ERROR-REPORTING.md) | Logging, crash reports, and error handling |
| [CROSS-PLATFORM.md](CROSS-PLATFORM.md) | Windows, macOS, and Linux compatibility |

### Features

| Document | Description |
|----------|-------------|
| [SYSTEM-TRAY.md](SYSTEM-TRAY.md) | System tray icon integration |
| [SECURITY-MODEL.md](SECURITY-MODEL.md) | Remote PIN activation for team distribution |
| [REMOTE-CONFIG-UPDATES.md](REMOTE-CONFIG-UPDATES.md) | Remote config, update checking, encrypted URLs |
| [API-CREDENTIALS.md](API-CREDENTIALS.md) | Keyring, encryption, and OAuth patterns |
| [ADVANCED-FEATURES.md](ADVANCED-FEATURES.md) | Plugins, notifications, remote sync, scheduler |

### Build & Deploy

| Document | Description |
|----------|-------------|
| [BUILD-DISTRIBUTION.md](BUILD-DISTRIBUTION.md) | Cargo build, CI/CD, and auto-update |
| [CHANGELOG-SETUP.md](CHANGELOG-SETUP.md) | Changelog generation and GitHub Pages |
| [GITHUB-PAGES-DARK-MODE.md](GITHUB-PAGES-DARK-MODE.md) | Just the Docs theme with dark mode setup |
| [TESTING.md](TESTING.md) | Cargo test setup, integration tests, and patterns |

---

## Template Files

All templates are in the `templates/` folder. Copy what you need to your project.

### Core Application

| File | Purpose |
|------|---------|
| `Cargo.toml` | Complete project config with platform-specific dependencies |
| `main.rs` | Entry point with tracing init and error handling |
| `app.rs` | Main application struct with window setup |
| `lib.rs` | Library root with version constant and `is_dev_mode()` |

### Utilities

| File | Purpose |
|------|---------|
| `config.rs` | ConfigManager singleton with observer pattern |
| `config_file.rs` | JSON/TOML file-based config storage |
| `session.rs` | Window state persistence |
| `logging.rs` | Tracing subscriber setup with sensitive data filtering |
| `error_reporter.rs` | Error capture with local logs and remote reporting |
| `errors.rs` | Custom error types with `thiserror` |
| `cache.rs` | Thread-safe caching with TTL and LRU eviction |
| `async_ops.rs` | Batch processing with progress tracking (tokio) |
| `validation.rs` | Input and form validation framework |
| `path_security.rs` | Path traversal prevention and sanitization |
| `file_ops.rs` | Safe file operations with batch support |
| `settings_export.rs` | ZIP-based settings backup/restore with validation |
| `health.rs` | System health checks (disk, memory, CPU, network) |

### UI Components

| File | Purpose |
|------|---------|
| `theme.rs` | Centralized theme color constants and iced styling |
| `widgets/progress.rs` | Custom progress bar widget |
| `widgets/help_panel.rs` | Collapsible help panel widget |
| `widgets/tab_drag.rs` | Chrome-style drag-and-drop tab reordering |
| `widgets/color_picker.rs` | HSV color wheel picker (iced_color_wheel + popup) |
| `widgets/messagebox.rs` | Native message dialogs (rfd) + iced popup modal |
| `title_bar.rs` | Windows DWM dark/light title bar matching |
| `panels/settings.rs` | Settings panel with tab layout |
| `panels/about.rs` | About dialog with version and system info |

### Features

| File | Purpose |
|------|---------|
| `tray.rs` | System tray manager using `tray-icon` |
| `activation.rs` | Remote PIN activation system |
| `credentials.rs` | Keyring-based credential storage |
| `encryption.rs` | AES-GCM encryption for bundled credentials |
| `oauth.rs` | OAuth2 desktop flow |
| `api_client.rs` | HTTP client with retry logic (reqwest) |
| `auto_update.rs` | GitHub release update checker |

### Advanced

| File | Purpose |
|------|---------|
| `plugins/mod.rs` | Plugin trait and static registry |
| `plugins/dynamic_loader.rs` | Dynamic plugin loading via `libloading` |
| `notifications.rs` | Slack, webhook, desktop notifications |
| `remote_config.rs` | GitHub-based config sync |
| `scheduler.rs` | Background task scheduling (tokio) |
| `feature_flags.rs` | Runtime feature toggles |

### Build & CI

| File | Purpose |
|------|---------|
| `Cargo.toml` (features) | iced feature flags and build configuration |
| `build.rs` | Cargo build script (embed version, resources) |
| `generate_changelog.rs` | Git-based changelog generator |
| `ci.yml` | GitHub Actions workflow for tests and releases |
| `test.yml` | CI workflow (tags-only test matrix, lint on every push) |
| `.cargo/config.toml` | Cargo workspace and target configuration |

### Security

| File | Purpose |
|------|---------|
| `SECURITY.md` | Security model documentation template |
| `app_access.json` | Sample remote config structure |

---

## Project Structure

After setup, your project should look like:

```
my-app/
├── Cargo.toml                 # Workspace/crate metadata and dependencies
├── Cargo.lock                 # Locked dependency versions
├── src/
│   ├── main.rs                # Entry point
│   ├── lib.rs                 # Library root, version, dev-mode detection
│   ├── app.rs                 # Application struct, window management
│   ├── ui/
│   │   ├── mod.rs             # UI module root
│   │   ├── components/        # Reusable widgets
│   │   │   ├── progress.rs
│   │   │   ├── help_panel.rs
│   │   │   └── menu.rs
│   │   ├── panels/            # Application panels/views
│   │   │   ├── settings.rs
│   │   │   ├── about.rs
│   │   │   └── log_viewer.rs
│   │   └── theme.rs           # Theme colors and styling
│   ├── core/                  # Business logic
│   │   └── mod.rs
│   └── utils/                 # Utilities from templates
│       ├── config.rs
│       ├── logging.rs
│       ├── error_reporter.rs
│       ├── settings_export.rs
│       ├── health.rs
│       ├── cache.rs
│       ├── validation.rs
│       ├── path_security.rs
│       └── mod.rs
├── tests/
│   ├── integration/
│   │   └── mod.rs
│   └── common/
│       └── mod.rs
├── benches/                   # Benchmarks (criterion)
├── build.rs                   # Build script
├── keys/                      # Encrypted credentials (optional)
├── logs/                      # Log files
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── sync-docs.yml
├── .cargo/
│   └── config.toml
├── rustfmt.toml               # Formatter config
├── clippy.toml                # Linter config
└── CHANGELOG.md
```

---

## Features Overview

### What This Template Provides

- **Version Management** - Single source of truth via `Cargo.toml` with dev mode detection
- **Window State** - Session restore (position, size, maximized state)
- **Configuration** - File-based storage with observer pattern
- **Logging** - Structured logging via `tracing` with sensitive data filtering
- **Error Reporting** - Local logs + remote reports + user-friendly error dialogs
- **System Tray** - Minimize to tray, context menu
- **Cross-Platform** - Windows, macOS, Linux support
- **Testing** - Cargo test with integration test structure
- **CI/CD** - GitHub Actions for tests, clippy, and releases
- **Auto-Update** - Check GitHub releases for updates
- **Changelog** - Auto-generated from git commits
- **GitHub Pages** - Documentation publishing
- **Caching** - Thread-safe with TTL, LRU eviction, and capacity limits
- **Batch Processing** - Async operations with progress and cancellation (tokio)
- **Validation** - Composable input validation
- **Path Security** - Traversal prevention and filename sanitization
- **UI Components** - Reusable progress indicators, help panels, theme system
- **Settings Panel** - Tabbed settings with save/cancel
- **About Dialog** - Version info, system details, support links
- **Log Viewer** - In-app log viewing
- **Settings Export** - ZIP backup/restore with security validation
- **Health Checks** - System health monitoring (disk, memory, CPU, network)

### Optional Features

- **PIN Activation** - Remote activation for team distribution
- **Credential Storage** - Keyring + encrypted file support
- **OAuth Integration** - OAuth2 for desktop apps
- **Plugin System** - Static registry + dynamic loading via `libloading`
- **Notifications** - Slack, webhooks, desktop notifications
- **Remote Config** - Sync settings with GitHub
- **Feature Flags** - Runtime feature toggles

---

## Commit Convention

Use conventional commits for automatic changelog categorization:

```bash
feat: Add new feature          # -> Added section
fix: Fix a bug                 # -> Fixed section
perf: Improve performance      # -> Improved section
refactor: Refactor code        # -> Changed section
security: Security fix         # -> Security section

# These are excluded from changelog:
docs: Update documentation
test: Add tests
chore: Maintenance
ci: CI/CD changes
```

---

## Requirements

- Rust 1.75+ (2021 edition minimum, 2024 edition recommended)
- Cargo
- Git
- GitHub account (for CI/CD, Pages, releases)

### Key Dependencies

The templates use these crates:

```toml
# Core
serde = { version = "1", features = ["derive"] }   # Serialization
serde_json = "1"                                    # JSON handling
anyhow = "1"                                        # Error handling (applications)
thiserror = "2"                                     # Error types (libraries)
tracing = "0.1"                                     # Structured logging
tracing-subscriber = "0.3"                          # Log output
tokio = { version = "1", features = ["full"] }      # Async runtime
reqwest = { version = "0.12", features = ["json"] } # HTTP client

# UI
iced = { version = "0.14", features = ["multi-window", "canvas", "tokio"] }  # Native GUI
iced_color_wheel = "0.1"       # HSV color wheel widget

# Optional
keyring = "3"                  # Secure credential storage
notify-rust = "4"              # Desktop notifications
tray-icon = "0.19"             # System tray
libloading = "0.8"             # Dynamic plugin loading
chrono = "0.4"                 # Date/time
glob = "0.3"                   # File pattern matching
base64 = "0.22"                # Base64 encoding
directories = "5"              # Platform config directories
sysinfo = "0.32"               # System health monitoring
```

---

## License

These templates are provided as-is for use in your projects. No attribution required.
