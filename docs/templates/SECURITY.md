# Security Model

This application uses a **remote PIN activation** security model for internal team distribution.

## How It Works

1. **First Run**: Users enter a team-shared activation PIN
2. **Validation**: The PIN is validated against a remote configuration hosted on GitHub
3. **Storage**: A SHA256 hash of the PIN is stored locally (never the PIN itself)
4. **Ongoing**: The app can be remotely disabled or require updates via config changes

## Security Features

### Remote Kill Switch

The app can be instantly disabled for all users by setting `enabled: false` in the remote configuration.

### Minimum Version Enforcement

Force users to update by setting `min_app_version` in the remote configuration. Users on older versions will see a block message.

### Offline Grace Period

Previously activated installations can run offline for 7 days. After that, they must connect to verify activation status.

## What Is NOT a Security Issue

The following items are **intentionally bundled** and are NOT security vulnerabilities:

### Bundled in Application:

- **Encryption keys** in `keys/` directory
- **OAuth client credentials** (client_id, client_secret)
- **API configuration files**
- **Encrypted credential files** (*.enc)

### Why This Is Intentional:

- **Remote PIN is the security gate** - not local encryption
- **All team members need the same API access** - bundling avoids individual setup
- **Kill switch provides central control** - can revoke access instantly
- **Version gating ensures updates** - can force security patches

## Security Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Remote Config (GitHub)                │
│                                                          │
│  {                                                       │
│    "enabled": true,              ← Kill switch           │
│    "activation_code": "PIN",     ← Team PIN              │
│    "min_app_version": "1.0.0",   ← Version gate          │
│    "block_message": "..."        ← Custom message        │
│  }                                                       │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│                    App Activation                        │
│                                                          │
│  1. Fetch remote config                                  │
│  2. Verify enabled == true                               │
│  3. Verify version >= min_app_version                    │
│  4. Verify stored hash matches activation_code           │
│  5. Grant access (or show activation dialog)             │
└─────────────────────────────────────────────────────────┘
```

## Remote Configuration

### Repository Structure

```
your-org/your-app-config/
├── app_access.json    # Activation settings
└── README.md          # Documentation
```

### app_access.json Schema

```json
{
    "enabled": true,
    "activation_code": "your-secret-pin",
    "min_app_version": "1.0.0",
    "block_message": "This version is no longer supported.",
    "invalid_code_message": "Invalid activation code.",
    "success_message": "Activated successfully!"
}
```

## Revoking Access

### Disable All Users

```json
{
    "enabled": false,
    "block_message": "This application has been discontinued."
}
```

### Force Update

```json
{
    "min_app_version": "2.0.0",
    "block_message": "Please update to version 2.0.0 or later."
}
```

### Change PIN

```json
{
    "activation_code": "new-secret-pin",
    "invalid_code_message": "Your activation code has changed. Please contact admin for the new code."
}
```

## Reporting Security Issues

If you discover a security vulnerability that could allow **unauthorized access** to the application or its resources, please report it to [security contact].

**Note**: Issues related to bundled credentials are NOT security vulnerabilities per this security model.
