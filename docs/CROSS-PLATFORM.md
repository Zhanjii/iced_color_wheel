# Cross-Platform Compatibility

This document describes patterns for building Rust desktop applications that work on Windows, macOS, and Linux.

## Platform Detection

### Compile-Time with `cfg`

Rust's `cfg` attributes and macros handle platform-specific code at compile time, with zero runtime cost:

```rust
// Compile-time conditional compilation
#[cfg(target_os = "windows")]
fn platform_specific_setup() {
    // Only compiled on Windows
}

#[cfg(target_os = "macos")]
fn platform_specific_setup() {
    // Only compiled on macOS
}

#[cfg(target_os = "linux")]
fn platform_specific_setup() {
    // Only compiled on Linux
}
```

### Runtime Detection

For cases where you need runtime branching (e.g., logging, config paths):

```rust
use std::env::consts::OS;

fn get_platform() -> &'static str {
    match OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        other => other,
    }
}

// Or use the cfg! macro for boolean checks
fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

fn is_linux() -> bool {
    cfg!(target_os = "linux")
}
```

### Conditional Code Blocks

```rust
if cfg!(target_os = "windows") {
    // Windows-specific logic (still compiled on all platforms, but branch
    // is const-folded away). Use #[cfg] for platform-specific imports.
} else if cfg!(target_os = "macos") {
    // macOS-specific logic
} else {
    // Linux/other
}
```

> **Note:** `cfg!()` evaluates at compile time but the code in both branches must still type-check on all platforms. Use `#[cfg(...)]` on items/functions when the code itself won't compile on other platforms (e.g., platform-specific FFI).

## Windows-Specific

### Taskbar Icon / AppUserModelID

**CRITICAL**: Set the AppUserModelID early in `main()` before any window creation. Without this, your app shares the default Rust/terminal icon in the Windows taskbar.

```rust
#[cfg(target_os = "windows")]
fn set_app_user_model_id() {
    use windows::core::w;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(
            w!("com.yourcompany.appname")
        );
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    set_app_user_model_id();

    // Now safe to create windows / GUI
}
```

### Window Icon (winapi)

For iced apps using raw Win32:

```rust
#[cfg(target_os = "windows")]
fn set_window_icon(hwnd: *mut std::ffi::c_void, icon_path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::winuser::{LoadImageW, SendMessageW, IMAGE_ICON, WM_SETICON};
    use winapi::um::winuser::{LR_LOADFROMFILE, LR_DEFAULTSIZE, ICON_SMALL, ICON_BIG};

    let wide_path: Vec<u16> = icon_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hicon = LoadImageW(
            std::ptr::null_mut(),
            wide_path.as_ptr(),
            IMAGE_ICON,
            0, 0,
            LR_LOADFROMFILE | LR_DEFAULTSIZE,
        );

        if !hicon.is_null() {
            SendMessageW(hwnd as _, WM_SETICON, ICON_SMALL as _, hicon as _);
            SendMessageW(hwnd as _, WM_SETICON, ICON_BIG as _, hicon as _);
        }
    }
}
```

For iced apps, icons are bundled via `include_bytes!` or loaded from `assets/` (see [BUILD-DISTRIBUTION.md](BUILD-DISTRIBUTION.md)).

### File Paths

Rust's `std::path::Path` and `PathBuf` handle path separators automatically:

```rust
use std::path::{Path, PathBuf};

// Always use Path/PathBuf -- they handle separators automatically
let config_dir = dirs::config_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("myapp");

let file_path = config_dir.join("settings.json");

// Reading the file -- no string conversion needed
let data = std::fs::read_to_string(&file_path)?;
```

### Long Path Support

Windows has a 260 character path limit by default. Rust's standard library handles this transparently on Windows by using the `\\?\` extended-length prefix internally when needed. However, if you pass paths to external tools via string, you may need to add the prefix manually:

```rust
#[cfg(target_os = "windows")]
fn ensure_long_path(path: &str) -> String {
    if !path.starts_with(r"\\?\") && path.len() > 240 {
        format!(r"\\?\{path}")
    } else {
        path.to_string()
    }
}
```

### Hide Console Window (GUI Apps)

Prevent the console window from appearing on Windows for GUI applications:

```rust
// At the top of main.rs
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
```

This hides the console in release builds while keeping it visible during development for debug output.

## macOS-Specific

### Code Signing and Gatekeeper

Unsigned binaries will be blocked by macOS Gatekeeper with "app is damaged" or "can't be opened." Users can bypass with right-click > Open, but this is a poor experience.

For production distribution:
1. **Sign** the app with an Apple Developer certificate
2. **Notarize** with Apple's notarization service
3. Package as a `.dmg` for clean distribution

```bash
# Sign the binary or app bundle
codesign --deep --force --verify --verbose \
    --sign "Developer ID Application: Your Name (TEAM_ID)" \
    target/release/your-app-name

# For .app bundles (e.g., via cargo-bundle)
codesign --deep --force --verify --verbose \
    --sign "Developer ID Application: Your Name (TEAM_ID)" \
    target/release/bundle/macos/YourApp.app

# Create a zip for notarization
ditto -c -k --keepParent target/release/bundle/macos/YourApp.app YourApp.zip

# Submit for notarization
xcrun notarytool submit YourApp.zip \
    --apple-id "you@email.com" \
    --team-id "TEAM_ID" \
    --password "app-specific-password" \
    --wait

# Staple the notarization ticket
xcrun stapler staple target/release/bundle/macos/YourApp.app
```

Without signing, instruct users to remove the quarantine flag:
```bash
xattr -cr /Applications/YourApp.app
# Or for a bare binary:
xattr -d com.apple.quarantine your-app-name
```

### Universal Binaries (Intel + Apple Silicon)

Build a universal binary that runs natively on both Intel and Apple Silicon Macs:

```bash
# Install both targets
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Build for both
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Combine into universal binary
lipo -create \
    target/x86_64-apple-darwin/release/your-app-name \
    target/aarch64-apple-darwin/release/your-app-name \
    -output target/release/your-app-name-universal
```

### macOS Native Features (cocoa/objc)

Access macOS-specific APIs via the `cocoa` and `objc` crates:

```rust
#[cfg(target_os = "macos")]
fn set_macos_app_name(name: &str) {
    use cocoa::appkit::NSApp;
    use cocoa::base::nil;
    use cocoa::foundation::NSString;
    use objc::runtime::Object;

    unsafe {
        let app = NSApp();
        let name = NSString::alloc(nil).init_str(name);
        // Set the app name shown in the menu bar
        let _: () = msg_send![app, setActivationPolicy: 0i64]; // NSApplicationActivationPolicyRegular
    }
}
```

### System Tray on macOS

The `tray-icon` crate handles cross-platform system trays:

```rust
use tray_icon::{TrayIconBuilder, menu::Menu};

fn create_tray() -> Result<(), Box<dyn std::error::Error>> {
    let menu = Menu::new();
    // Add menu items...

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("YourApp")
        .with_icon(load_icon()?)
        .build()?;

    Ok(())
}
```

### macOS Build Checklist

Before distributing a macOS build:

- [ ] Universal binary built (Intel + Apple Silicon) or target architecture specified
- [ ] App signed and notarized (or quarantine removal documented)
- [ ] Tested on both Intel and Apple Silicon Macs
- [ ] Tested on Sonoma (14) and Sequoia (15)
- [ ] `.app` bundle includes correct `Info.plist` (use `cargo-bundle` to generate)

## Linux-Specific

### Desktop Environment Detection

```rust
fn get_desktop_environment() -> String {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

fn is_x11() -> bool {
    std::env::var("DISPLAY").is_ok() && !is_wayland()
}
```

### System Tray on Linux

System tray availability depends on the desktop environment and whether `libappindicator` is installed. The `tray-icon` crate works on most Linux DEs:

```rust
fn is_linux_tray_available() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }

    // Check for libappindicator at runtime
    // Most modern DEs (GNOME with extension, KDE, XFCE) support it
    std::process::Command::new("which")
        .arg("libappindicator")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

### Linux System Dependencies

Iced apps may require system libraries for graphics and tray support. Document these for users or provide a deb/AppImage:

```bash
# Ubuntu/Debian
sudo apt install libappindicator3-dev librsvg2-dev

# Fedora
sudo dnf install libappindicator-gtk3-devel librsvg2-devel

# Arch
sudo pacman -S libappindicator-gtk3 librsvg
```

### Config Directories

Use the `dirs` crate to follow platform conventions (XDG on Linux, `~/Library` on macOS, `%APPDATA%` on Windows):

```rust
use std::path::PathBuf;

fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("myapp")
}

fn get_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("myapp")
}

fn get_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("myapp")
}
```

The `dirs` crate returns:

| Function | Windows | macOS | Linux |
|----------|---------|-------|-------|
| `config_dir()` | `%APPDATA%` | `~/Library/Application Support` | `$XDG_CONFIG_HOME` or `~/.config` |
| `data_dir()` | `%LOCALAPPDATA%` | `~/Library/Application Support` | `$XDG_DATA_HOME` or `~/.local/share` |
| `cache_dir()` | `%LOCALAPPDATA%` | `~/Library/Caches` | `$XDG_CACHE_HOME` or `~/.cache` |

## Platform-Conditional Dependencies

In `Cargo.toml`, use `[target.'cfg(...)'.dependencies]`:

```toml
[dependencies]
# Cross-platform
serde = { version = "1", features = ["derive"] }
dirs = "5"
tray-icon = "0.19"

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["wincon", "shellapi", "winuser"] }
windows = { version = "0.58", features = ["Win32_UI_Shell"] }

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.26"
objc = "0.2"

[target.'cfg(target_os = "linux")'.dependencies]
# Linux-specific crates if needed
```

## Secure Credential Storage

Use the `keyring` crate for cross-platform credential storage (Windows Credential Manager, macOS Keychain, Linux Secret Service):

```rust
use keyring::Entry;

fn store_credential(
    service: &str,
    username: &str,
    password: &str,
) -> Result<(), keyring::Error> {
    let entry = Entry::new(service, username)?;
    entry.set_password(password)
}

fn get_credential(
    service: &str,
    username: &str,
) -> Result<String, keyring::Error> {
    let entry = Entry::new(service, username)?;
    entry.get_password()
}

fn delete_credential(
    service: &str,
    username: &str,
) -> Result<(), keyring::Error> {
    let entry = Entry::new(service, username)?;
    entry.delete_credential()
}
```

> **Note:** On macOS, first access may prompt for Keychain permission. On Linux, requires `libsecret` / Secret Service (GNOME Keyring, KDE Wallet).

## Testing Across Platforms

### Conditional Tests with `cfg`

```rust
#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_icon_loading() {
        // Only compiled and run on Windows
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_keychain_access() {
        // Only compiled and run on macOS
    }

    #[test]
    fn test_config_dir_exists() {
        let dir = super::get_config_dir();
        // This test runs on all platforms
        assert!(dir.to_str().is_some());
    }
}
```

### CI Matrix Testing

Test across platforms in GitHub Actions:

```yaml
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --verbose
```

## Common Gotchas

### 1. Line Endings

Git can mangle line endings. Use `.gitattributes`:

```
* text=auto
*.rs text eol=lf
*.toml text eol=lf
*.md text eol=lf
*.json text eol=lf
*.bat text eol=crlf
*.ps1 text eol=crlf
```

`rustfmt` normalizes line endings in `.rs` files automatically.

### 2. Case Sensitivity

Rust module names follow the file system. Windows/macOS are case-insensitive, Linux is case-sensitive. Always use lowercase `snake_case` for file names (Rust convention):

```rust
// GOOD - consistent snake_case file names
mod my_module;  // my_module.rs

// BAD - may fail on case-sensitive file systems
mod MyModule;   // MyModule.rs -- don't do this
```

### 3. Temp Files

Use the `tempfile` crate for cross-platform temporary files:

```rust
use tempfile::{NamedTempFile, TempDir};

// Cross-platform temp file
let mut temp_file = NamedTempFile::new()?;
temp_file.write_all(b"data")?;

// Cross-platform temp directory
let temp_dir = TempDir::new()?;
let file_path = temp_dir.path().join("output.txt");
```

Or use `std::env::temp_dir()` for the platform temp directory:

```rust
let temp_dir = std::env::temp_dir().join("myapp");
std::fs::create_dir_all(&temp_dir)?;
```

### 4. Dynamic Linking

On Linux, Rust binaries link dynamically to glibc by default. This can cause "GLIBC_2.XX not found" errors on older distributions. Solutions:

- **Use musl target** for fully static binaries: `cargo build --target x86_64-unknown-linux-musl`
- **Build on an older distribution** (e.g., Ubuntu 20.04 in CI)
- **Use `cross`** for containerized builds (see [BUILD-DISTRIBUTION.md](BUILD-DISTRIBUTION.md))

### 5. File Permissions

Windows does not support Unix-style file permissions. Use `cfg` when setting permissions:

```rust
#[cfg(unix)]
fn make_executable(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) -> std::io::Result<()> {
    // No-op on Windows -- executability is determined by file extension
    Ok(())
}
```

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [BUILD-DISTRIBUTION.md](BUILD-DISTRIBUTION.md) - Build and distribution
- [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) - Configuration storage
