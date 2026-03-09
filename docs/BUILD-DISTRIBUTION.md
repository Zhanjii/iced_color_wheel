# Build & Distribution

This guide covers building Rust applications for release, configuring Cargo release profiles, setting up CI/CD with GitHub Actions, cross-compilation with `cross`, and implementing auto-update functionality.

## Overview

Distribution involves:
- Compiling a release binary with `cargo build --release`
- Bundling resources (icons, configs, assets) via `include_bytes!`
- Setting up automated multi-platform builds
- Implementing version checking and updates

## Cargo Release Configuration

### Cargo.toml

```toml
[package]
name = "your-app-name"
version = "1.0.0"
edition = "2021"
authors = ["Your Name <you@example.com>"]
description = "Your application description"
license = "MIT"
repository = "https://github.com/owner/repo"

# Compile-time metadata
[package.metadata]
app_name = "YourAppName"
bundle_identifier = "com.yourcompany.appname"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
semver = "1"
dirs = "5"

# Platform-specific dependencies
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["wincon", "shellapi", "winuser"] }

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.26"
objc = "0.2"

[profile.release]
opt-level = 3
lto = true            # Link-time optimization for smaller, faster binary
codegen-units = 1     # Single codegen unit for maximum optimization
strip = true          # Strip debug symbols from binary
panic = "abort"       # Abort on panic (smaller binary, no unwinding overhead)

[profile.release-with-debug]
inherits = "release"
strip = false
debug = true          # Keep debug info for profiling release builds
```

### Iced Build Considerations

For iced desktop apps, no special bundler is needed -- `cargo build --release` produces the final binary. Configure iced features in `Cargo.toml`:

```toml
[dependencies]
iced = { version = "0.14", features = ["multi-window", "canvas", "tokio"] }
iced_color_wheel = "0.1"
rfd = "0.15"       # Native file/message dialogs
```

Resources (icons, assets) are typically embedded via `include_bytes!` or loaded from an `assets/` directory at runtime. For `.app`/`.deb`/`.msi` packaging, use `cargo-bundle`:

```bash
cargo install cargo-bundle
cargo bundle --release
```

### Windows Executable Metadata

For iced desktop apps, embed Windows version info via a build script.

Create `build.rs`:

```rust
fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/app.ico");
        res.set("FileDescription", "Your App Description");
        res.set("ProductName", "YourAppName");
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("CompanyName", "Your Company");
        res.set("LegalCopyright", "Copyright (c) 2024");
        res.compile().expect("Failed to compile Windows resources");
    }
}
```

Add to `Cargo.toml`:

```toml
[build-dependencies]
winresource = "0.1"
```

## Build Workflow

### Cargo Make (Makefile.toml)

For projects that need more than bare `cargo build --release`, use `cargo-make`:

```toml
# Makefile.toml

[tasks.clean]
command = "cargo"
args = ["clean"]

[tasks.format]
command = "cargo"
args = ["fmt"]

[tasks.lint]
command = "cargo"
args = ["clippy", "--", "-D", "warnings"]

[tasks.test]
command = "cargo"
args = ["test"]

[tasks.build-release]
command = "cargo"
args = ["build", "--release"]
dependencies = ["format", "lint", "test"]

[tasks.ci]
dependencies = ["format", "lint", "test", "build-release"]
```

Run with:

```bash
cargo install cargo-make
cargo make ci              # Full pipeline: format, lint, test, release build
cargo make build-release   # Release build only (still runs format, lint, test first)
```

## Cross-Compilation with `cross`

[`cross`](https://github.com/cross-rs/cross) uses Docker to cross-compile Rust binaries for different targets without installing platform-specific toolchains.

### Setup

```bash
cargo install cross --git https://github.com/cross-rs/cross

# Build for Linux from Windows/macOS
cross build --release --target x86_64-unknown-linux-gnu

# Build for Linux ARM (Raspberry Pi)
cross build --release --target aarch64-unknown-linux-gnu

# Build for musl (statically linked, no glibc dependency)
cross build --release --target x86_64-unknown-linux-musl
```

### Cross.toml Configuration

```toml
# Cross.toml
[target.x86_64-unknown-linux-gnu]
image = "ghcr.io/cross-rs/x86_64-unknown-linux-gnu:main"

[target.aarch64-unknown-linux-gnu]
image = "ghcr.io/cross-rs/aarch64-unknown-linux-gnu:main"

[build.env]
passthrough = [
    "RUST_LOG",
    "APP_VERSION",
]
```

### Common Targets

| Target | Platform |
|--------|----------|
| `x86_64-pc-windows-msvc` | Windows x86-64 |
| `x86_64-unknown-linux-gnu` | Linux x86-64 (glibc) |
| `x86_64-unknown-linux-musl` | Linux x86-64 (static) |
| `aarch64-unknown-linux-gnu` | Linux ARM64 |
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `universal-apple-darwin` | macOS Universal Binary |

> **Note:** macOS targets cannot be cross-compiled from Linux/Windows without an Xcode SDK. Use native macOS runners in CI for macOS builds.

## GitHub Actions CI/CD

### Complete Workflow

```yaml
# .github/workflows/build.yml
name: Build and Release

on:
  push:
    branches: [main, master]
    tags:
      - 'v*'
  pull_request:
    branches: [main, master]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo registry and build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: ${{ runner.os }}-cargo-

      - name: Check formatting
        run: cargo fmt --check

      - name: Run clippy
        run: cargo clippy -- -D warnings

      - name: Run tests
        run: cargo test --verbose

  build:
    name: Build ${{ matrix.os }}
    needs: test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: your-app-name.exe
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: your-app-name
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: your-app-name

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }}

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.os }}-build
          path: target/${{ matrix.target }}/release/${{ matrix.artifact }}
          retention-days: 5

  release:
    name: Create Release
    needs: build
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-latest
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Get version
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          name: v${{ steps.version.outputs.VERSION }}
          draft: false
          prerelease: ${{ contains(github.ref, 'dev') || contains(github.ref, 'beta') }}
          files: |
            artifacts/windows-latest-build/your-app-name.exe
            artifacts/macos-latest-build/your-app-name
            artifacts/ubuntu-latest-build/your-app-name
```

### Secrets Configuration

Required GitHub repository secrets:

```
# For code signing (optional)
WINDOWS_CERTIFICATE_BASE64
WINDOWS_CERTIFICATE_PASSWORD
APPLE_CERTIFICATE_BASE64
APPLE_CERTIFICATE_PASSWORD
APPLE_TEAM_ID

# Automatically provided
GITHUB_TOKEN
```

## Auto-Update System

### Version Checker (Standalone Rust)

```rust
//! auto_update.rs - Check for and download updates from GitHub releases.

use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{error, info};

const GITHUB_API: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GitHubAsset>,
    prerelease: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub release_notes: String,
    pub download_url: String,
    pub published_at: Option<String>,
}

pub struct AutoUpdater {
    current_version: Version,
    github_repo: String,
    include_prerelease: bool,
    client: Client,
    latest_release: Option<GitHubRelease>,
}

impl AutoUpdater {
    /// Create a new auto-updater.
    ///
    /// # Arguments
    /// * `current_version` - Current app version (e.g., "1.0.0")
    /// * `github_repo` - GitHub repository in "owner/repo" format
    /// * `include_prerelease` - Whether to include pre-release versions
    pub fn new(
        current_version: &str,
        github_repo: &str,
        include_prerelease: bool,
    ) -> Result<Self, semver::Error> {
        Ok(Self {
            current_version: Version::parse(current_version)?,
            github_repo: github_repo.to_string(),
            include_prerelease,
            client: Client::builder()
                .user_agent(concat!(
                    env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")
                ))
                .build()
                .expect("Failed to build HTTP client"),
            latest_release: None,
        })
    }

    /// Check if a newer version is available.
    pub async fn check_for_update(
        &mut self,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let release = self.fetch_latest_release().await?;

        let tag = release.tag_name.trim_start_matches('v');
        let latest_version = Version::parse(tag)?;

        if latest_version > self.current_version {
            info!("Update available: {latest_version}");
            self.latest_release = Some(release);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get information about the available update.
    pub fn update_info(&self) -> Option<UpdateInfo> {
        let release = self.latest_release.as_ref()?;
        Some(UpdateInfo {
            version: release.tag_name.trim_start_matches('v').to_string(),
            release_notes: release.body.clone().unwrap_or_default(),
            download_url: self
                .platform_download_url(release)
                .unwrap_or_default(),
            published_at: release.published_at.clone(),
        })
    }

    /// Download the update to a temporary file.
    pub async fn download_update(
        &self,
        on_progress: impl Fn(u64, u64),
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let release = self
            .latest_release
            .as_ref()
            .ok_or("No update available")?;

        let url = self
            .platform_download_url(release)
            .ok_or("No download URL for current platform")?;

        let response = self.client.get(&url).send().await?.error_for_status()?;
        let total_size = response.content_length().unwrap_or(0);

        let file_name = url.rsplit('/').next().unwrap_or("update");
        let temp_path = std::env::temp_dir().join(file_name);

        let mut file = fs::File::create(&temp_path).await?;
        let mut downloaded: u64 = 0;

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            on_progress(downloaded, total_size);
        }

        file.flush().await?;
        Ok(temp_path)
    }

    /// Launch the downloaded update and exit the current process.
    pub fn launch_update(installer_path: &Path) -> Result<(), std::io::Error> {
        use std::process::Command;

        #[cfg(target_os = "windows")]
        Command::new(installer_path).spawn()?;

        #[cfg(target_os = "macos")]
        Command::new("open").arg(installer_path).spawn()?;

        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(installer_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(installer_path, perms)?;
            Command::new(installer_path).spawn()?;
        }

        std::process::exit(0);
    }

    async fn fetch_latest_release(
        &self,
    ) -> Result<GitHubRelease, Box<dyn std::error::Error>> {
        let url = if self.include_prerelease {
            format!(
                "{GITHUB_API}/repos/{}/releases?per_page=1",
                self.github_repo
            )
        } else {
            format!(
                "{GITHUB_API}/repos/{}/releases/latest",
                self.github_repo
            )
        };

        let response = self.client.get(&url).send().await?.error_for_status()?;

        if self.include_prerelease {
            let releases: Vec<GitHubRelease> = response.json().await?;
            releases
                .into_iter()
                .next()
                .ok_or_else(|| "No releases found".into())
        } else {
            Ok(response.json().await?)
        }
    }

    fn platform_download_url(&self, release: &GitHubRelease) -> Option<String> {
        let patterns: &[&str] = if cfg!(target_os = "windows") {
            &[".exe", ".msi", "windows"]
        } else if cfg!(target_os = "macos") {
            &[".app", ".dmg", "macos", "darwin"]
        } else {
            &["linux", ".appimage", ".deb"]
        };

        release.assets.iter().find_map(|asset| {
            let name = asset.name.to_lowercase();
            patterns
                .iter()
                .any(|p| name.contains(p))
                .then(|| asset.browser_download_url.clone())
        })
    }
}

/// Spawn a background update check on app startup.
pub fn check_update_on_startup(
    current_version: &str,
    github_repo: &str,
    on_update_available: impl FnOnce(UpdateInfo) + Send + 'static,
) {
    let version = current_version.to_string();
    let repo = github_repo.to_string();

    tokio::spawn(async move {
        let mut updater = match AutoUpdater::new(&version, &repo, false) {
            Ok(u) => u,
            Err(e) => {
                error!("Failed to create updater: {e}");
                return;
            }
        };

        match updater.check_for_update().await {
            Ok(true) => {
                if let Some(info) = updater.update_info() {
                    on_update_available(info);
                }
            }
            Ok(false) => {}
            Err(e) => error!("Update check failed: {e}"),
        }
    });
}
```

### Update Dialog (iced)

For iced daemon apps, show the update dialog as a popup window:

```rust
use iced::window;
use iced::Size;

pub enum UpdateAction {
    Download,
    Skip,
}

// Open an update dialog as a new iced window
Message::ShowUpdateDialog(version, release_notes) => {
    let (id, task) = window::open(window::Settings {
        size: Size::new(450.0, 350.0),
        resizable: false,
        exit_on_close_request: false,
        ..Default::default()
    });
    self.windows.insert(id, WindowKind::UpdateDialog { version, release_notes });
    task.discard()
}
```

For simpler update notifications, use `rfd::MessageDialog`:

```rust
use rfd::MessageDialog;
use rfd::MessageButtons;

fn show_update_prompt(version: &str, release_notes: &str) -> bool {
    MessageDialog::new()
        .set_title("Update Available")
        .set_description(&format!(
            "Version {version} is available!\n\n{release_notes}\n\nDownload now?"
        ))
        .set_buttons(MessageButtons::YesNo)
        .show()
}
```

## Dependency Management

### Cargo.toml Dependency Organization

```toml
[package]
name = "your-app-name"
version = "1.0.0"
edition = "2021"
rust-version = "1.75"  # Minimum supported Rust version (MSRV)

[dependencies]
# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Logging / Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Error handling
thiserror = "2"
anyhow = "1"

# Versioning
semver = "1"

# Platform directories
dirs = "5"

# Platform-specific
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["wincon", "shellapi"] }
windows = { version = "0.58", features = ["Win32_UI_Shell"] }

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.26"

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
mockall = "0.13"

[build-dependencies]
winresource = "0.1"
```

## Distribution Checklist

### Pre-Build
- [ ] Update version in `Cargo.toml`
- [ ] Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
- [ ] Update changelog
- [ ] Verify `Cargo.lock` is committed

### Build
- [ ] Run `cargo build --release`
- [ ] Test release binary on clean machine
- [ ] Verify all bundled resources are accessible
- [ ] Check binary size is reasonable

### Release
- [ ] Create git tag (`git tag v1.0.0`)
- [ ] Push tag to trigger CI build (`git push origin v1.0.0`)
- [ ] Verify CI builds succeed on all platforms
- [ ] Check release assets uploaded to GitHub

### Post-Release
- [ ] Verify auto-update works from previous version
- [ ] Monitor error reports
- [ ] Update documentation if needed

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [VERSIONING.md](VERSIONING.md) - Version management
- [CROSS-PLATFORM.md](CROSS-PLATFORM.md) - Cross-platform compatibility
