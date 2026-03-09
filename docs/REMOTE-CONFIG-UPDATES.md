# Remote Configuration & Update Checking

This guide covers setting up a remote configuration system for:
- App activation with PIN verification
- Version update checking
- Encrypted download URLs
- Remote kill switch capability

## Overview

The system uses a public JSON config file (e.g., on GitHub) that contains:
- **Hashed PIN** for activation (not plaintext)
- **Latest version** for update notifications
- **Encrypted URLs** for download links (decrypted using PIN)
- **Kill switch** to disable compromised versions

## Quick Setup

### 1. Generate Secrets

Use the `remote_access_crypto` module to generate your config secrets:

```rust
use your_app::utils::remote_access_crypto::generate_remote_config_secrets;

fn main() {
    let secrets = generate_remote_config_secrets("1234");
    println!("{:#?}", secrets);
    // RemoteConfigSecrets {
    //     pin_hash: "a1b2c3...",
    //     pin_salt: "base64salt==",
    //     encryption_salt: "differentbase64salt==",
    // }
}
```

Or run the binary directly:
```bash
cargo run --bin remote-access-crypto
# Interactive prompt to generate secrets and encrypt fields
```

### 2. Create Remote Config

Create `app_access.json` in a public GitHub repo:

```json
{
    "enabled": true,
    "min_app_version": "1.0.0",
    "latest_version": "1.2.0",

    "pin_hash": "a1b2c3d4e5f6...",
    "pin_salt": "base64encodedsalt==",
    "encryption_salt": "differentbase64salt==",

    "block_message": "Please update to continue.",
    "activation_prompt": "Enter your activation PIN:",
    "invalid_code_message": "Invalid PIN.",
    "success_message": "Activated!",

    "release_notes": "Bug fixes and improvements.",

    "download_url_win": "encrypted-base64-string...",
    "download_url_mac": "encrypted-base64-string..."
}
```

### 3. Encrypt Download URLs

```rust
use your_app::utils::remote_access_crypto::encrypt_for_config;

fn main() -> anyhow::Result<()> {
    let encrypted_win = encrypt_for_config(
        "https://example.com/app-setup.exe",
        "1234",
        "differentbase64salt==",
    )?;
    println!(r#""download_url_win": "{encrypted_win}""#);
    Ok(())
}
```

### 4. Integrate in Your App

```rust
use your_app::utils::app_activation::{ActivationManager, ActivationStatus};

fn main() -> anyhow::Result<()> {
    let config = ConfigManager::instance();
    let manager = ActivationManager::new(config);
    let status = manager.check_activation()?;

    match status {
        ActivationStatus::Activated => {
            // Check for updates
            if let Some(update_info) = manager.check_for_updates()? {
                show_update_notification(
                    &format!("Version {} available!", update_info.latest_version),
                    &update_info.release_notes,
                    &update_info.download_url,
                );
            }
        }
        ActivationStatus::CodeRequired => {
            // Show activation dialog
            let pin = show_pin_dialog();
            if manager.activate_with_code(&pin)? {
                // Activated!
            }
        }
        ActivationStatus::VersionBlocked => {
            show_error(&manager.get_message(&status));
            std::process::exit(1);
        }
        _ => {}
    }

    Ok(())
}
```

## Remote Config Fields

| Field | Required | Description |
|-------|----------|-------------|
| `enabled` | Yes | `false` to disable all app instances (kill switch) |
| `min_app_version` | Yes | Minimum allowed version (blocks older versions) |
| `latest_version` | No | Latest available version (for update notifications) |
| `pin_hash` | Yes | SHA-256 hash of PIN with salt |
| `pin_salt` | Yes | Base64-encoded salt for PIN hashing |
| `encryption_salt` | No | Base64-encoded salt for key derivation |
| `block_message` | No | Message shown when app is blocked |
| `activation_prompt` | No | Custom prompt for PIN entry |
| `invalid_code_message` | No | Message for wrong PIN |
| `success_message` | No | Message after successful activation |
| `release_notes` | No | Notes about latest version |
| `download_url_win` | No | Windows download URL (can be encrypted) |
| `download_url_mac` | No | macOS download URL (can be encrypted) |

## Security Model

### What's Protected

1. **PIN is never stored in plaintext**
   - Remote config contains salted SHA-256 hash
   - Local storage contains hash of user's entered PIN
   - Original PIN cannot be recovered from hashes

2. **Download URLs are encrypted**
   - AES-256-GCM encryption using PIN-derived key (via `ring` or `aes-gcm` crate)
   - Only users with valid PIN can decrypt
   - URLs not exposed in public config

3. **Version enforcement**
   - `min_app_version` blocks outdated versions
   - Can force updates for security fixes

4. **Kill switch**
   - Set `enabled: false` to disable all instances
   - Useful for compromised versions or end-of-life

### What's NOT Protected

- The config file is public (anyone can read non-encrypted fields)
- PIN hashes could theoretically be brute-forced (use strong PINs)
- Local activation can work offline for grace period

## Update Checking Flow

```
┌──────────────┐     ┌─────────────────┐     ┌──────────────┐
│   App Start  │────>│  Fetch Remote   │────>│ Compare      │
│              │     │  Config         │     │ Versions     │
└──────────────┘     └─────────────────┘     └──────┬───────┘
                                                    │
                     ┌──────────────────────────────┴───────┐
                     │                                      │
              ┌──────▼──────┐                       ┌───────▼──────┐
              │ Up to Date  │                       │ Update       │
              │ (continue)  │                       │ Available    │
              └─────────────┘                       └───────┬──────┘
                                                           │
                                              ┌────────────▼────────────┐
                                              │ Show Update Dialog      │
                                              │ - Version info          │
                                              │ - Release notes         │
                                              │ - Download button       │
                                              └─────────────────────────┘
```

## Example: Complete Integration

```rust
// main.rs
use anyhow::Result;
use semver::Version;

use your_app::config::ConfigManager;
use your_app::utils::app_activation::{ActivationManager, ActivationStatus};
use your_app::ui::dialogs::{ActivationDialog, UpdateDialog};

fn main() -> Result<()> {
    let config = ConfigManager::instance();
    let activation = ActivationManager::new(config);

    // Check activation status
    let status = activation.check_activation()?;

    match status {
        ActivationStatus::AppDisabled => {
            show_error("This app has been disabled.");
            std::process::exit(1);
        }
        ActivationStatus::VersionBlocked => {
            show_error(&activation.get_message(&status));
            std::process::exit(1);
        }
        ActivationStatus::CodeRequired => {
            let dialog = ActivationDialog::new();
            let pin = dialog.get_pin();

            match pin {
                Some(ref p) if activation.activate_with_code(p)? => {}
                _ => {
                    show_error(&activation.get_message(&ActivationStatus::CodeInvalid));
                    std::process::exit(1);
                }
            }
        }
        ActivationStatus::NetworkError => {
            // Allow offline use within grace period
            if !activation.has_valid_local_activation()? {
                show_error("Please connect to the internet to verify activation.");
                std::process::exit(1);
            }
        }
        ActivationStatus::Activated => {}
    }

    // App is activated - check for updates
    if let Some(update_info) = activation.check_for_updates()? {
        UpdateDialog::new(
            &update_info.current_version,
            &update_info.latest_version,
            &update_info.release_notes,
            &update_info.download_url,
        )
        .show();
    }

    // Continue with app startup...
    run_app()
}
```

## Key Crates

| Crate | Purpose |
|-------|---------|
| `reqwest` | HTTP client for fetching remote config |
| `serde` / `serde_json` | JSON deserialization of config |
| `ring` or `sha2` | SHA-256 PIN hashing |
| `aes-gcm` or `chacha20poly1305` | Symmetric encryption for download URLs |
| `base64` | Encoding/decoding salts and encrypted data |
| `semver` | Semantic version comparison |

Add to `Cargo.toml`:
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "blocking"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ring = "0.17"
base64 = "0.22"
semver = "1"
anyhow = "1"
```

## Hosting Options

### GitHub (Recommended)

Host in a public repo for free, reliable access:

```
https://raw.githubusercontent.com/your-org/your-app-config/main/app_access.json
```

### Google Drive

1. Upload JSON file to Google Drive
2. Set sharing to "Anyone with the link"
3. Get direct download URL:
   ```
   https://drive.google.com/uc?export=download&id=FILE_ID
   ```

### Your Own Server

Any HTTPS endpoint that returns JSON:
```
https://api.yourcompany.com/app/config.json
```

## Updating the Remote Config

### To release a new version:

1. Update `latest_version` in config
2. Add `release_notes`
3. Encrypt new download URLs if needed
4. Commit and push

### To block an old version:

1. Update `min_app_version` to the minimum safe version
2. Update `block_message` with instructions

### To disable all instances (emergency):

1. Set `enabled: false`
2. Optionally update `block_message`

### To change the activation PIN:

1. Generate new secrets with new PIN
2. Update `pin_hash`, `pin_salt`, `encryption_salt`
3. Re-encrypt any encrypted fields with new key
4. Users will need to re-enter the new PIN

## Template Files

- `templates/app_activation.rs` - Activation manager with update checking
- `templates/remote_access_crypto.rs` - PIN hashing and encryption utilities
- `templates/app_access.json` - Example remote config structure
