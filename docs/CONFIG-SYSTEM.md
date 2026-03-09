# Configuration System

This document describes the configuration management architecture using a global `ConfigManager` with `OnceLock`-based initialization, file-backed storage via `serde`, and remote sync capabilities.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      ConfigManager                           │
│  (OnceLock global - thread-safe access to all config)       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │ ConfigRegistry  │    │ RemoteConfigMgr │                │
│  │ (Local Storage) │◄──►│ (GitHub Sync)   │                │
│  └─────────────────┘    └─────────────────┘                │
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │ SessionManager  │    │   Observers     │                │
│  │ (Window State)  │    │ (Change Notify) │                │
│  └─────────────────┘    └─────────────────┘                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## ConfigRegistry (Local Storage)

Handles reading/writing configuration to disk using `serde_json`:

```rust
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Registry-based configuration storage.
pub struct ConfigRegistry {
    config_dir: PathBuf,
    cache: RwLock<HashMap<String, Value>>,
}

impl ConfigRegistry {
    pub fn new(config_dir: &Path) -> Self {
        fs::create_dir_all(config_dir).ok();

        let registry = Self {
            config_dir: config_dir.to_path_buf(),
            cache: RwLock::new(HashMap::new()),
        };
        registry.load_all();
        registry
    }

    fn file_path(&self, section: &str) -> PathBuf {
        self.config_dir.join(format!("{section}.json"))
    }

    fn load_all(&self) {
        let Ok(entries) = fs::read_dir(&self.config_dir) else {
            return;
        };

        let mut cache = self.cache.write().unwrap();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                let section = path.file_stem().unwrap().to_string_lossy().to_string();
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(value) = serde_json::from_str(&contents) {
                        cache.insert(section, value);
                    }
                }
            }
        }
    }

    /// Get a configuration value from a section.
    pub fn get(&self, section: &str, key: &str) -> Option<Value> {
        let cache = self.cache.read().unwrap();
        cache
            .get(section)?
            .as_object()?
            .get(key)
            .cloned()
    }

    /// Set a configuration value in a section.
    pub fn set(&self, section: &str, key: &str, value: Value) {
        let mut cache = self.cache.write().unwrap();
        let section_data = cache
            .entry(section.to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));

        if let Some(obj) = section_data.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        drop(cache);
        self.save_section(section);
    }

    /// Get an entire section as a JSON object.
    pub fn get_section(&self, section: &str) -> Value {
        let cache = self.cache.read().unwrap();
        cache.get(section).cloned().unwrap_or(Value::Object(serde_json::Map::new()))
    }

    /// Replace an entire section.
    pub fn set_section(&self, section: &str, data: Value) {
        let mut cache = self.cache.write().unwrap();
        cache.insert(section.to_string(), data);
        drop(cache);
        self.save_section(section);
    }

    fn save_section(&self, section: &str) {
        let cache = self.cache.read().unwrap();
        let Some(data) = cache.get(section) else { return };

        let path = self.file_path(section);
        if let Ok(json) = serde_json::to_string_pretty(data) {
            fs::write(path, json).ok();
        }
    }

    /// Delete a key from a section.
    pub fn delete(&self, section: &str, key: &str) {
        let mut cache = self.cache.write().unwrap();
        if let Some(section_data) = cache.get_mut(section) {
            if let Some(obj) = section_data.as_object_mut() {
                obj.remove(key);
            }
        }
        drop(cache);
        self.save_section(section);
    }
}
```

## ConfigManager (Global Access)

Main configuration access point using `OnceLock` for safe global initialization:

```rust
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

type ObserverFn = Box<dyn Fn(Option<&str>, &Value, &str) + Send + Sync>;

/// Thread-safe global configuration manager.
pub struct ConfigManager {
    registry: ConfigRegistry,
    observers: Mutex<Vec<ObserverFn>>,
}

/// Global singleton instance.
static INSTANCE: OnceLock<ConfigManager> = OnceLock::new();

impl ConfigManager {
    /// Initialize the global ConfigManager. Must be called once at startup.
    pub fn initialize(config_dir: &Path) {
        let manager = ConfigManager {
            registry: ConfigRegistry::new(config_dir),
            observers: Mutex::new(Vec::new()),
        };

        INSTANCE
            .set(manager)
            .expect("ConfigManager already initialized");
    }

    /// Get the global ConfigManager instance.
    ///
    /// Panics if `initialize()` has not been called.
    pub fn instance() -> &'static ConfigManager {
        INSTANCE
            .get()
            .expect("ConfigManager not initialized -- call ConfigManager::initialize() first")
    }

    // ==================== Basic Access ====================

    /// Get a configuration value, deserialized to the target type.
    pub fn get<T: DeserializeOwned>(
        &self,
        key: &str,
        section: Option<&str>,
    ) -> Option<T> {
        let section = section.unwrap_or("settings");
        let value = self.registry.get(section, key)?;
        serde_json::from_value(value).ok()
    }

    /// Get a configuration value with a default fallback.
    pub fn get_or<T: DeserializeOwned>(
        &self,
        key: &str,
        default: T,
        section: Option<&str>,
    ) -> T {
        self.get(key, section).unwrap_or(default)
    }

    /// Set a configuration value and notify observers.
    pub fn set<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        section: Option<&str>,
    ) {
        let section = section.unwrap_or("settings");
        let json_value = serde_json::to_value(value).unwrap();
        self.registry.set(section, key, json_value.clone());
        self.notify_observers(Some(key), &json_value, section);
    }

    /// Delete a configuration value.
    pub fn delete(&self, key: &str, section: Option<&str>) {
        let section = section.unwrap_or("settings");
        self.registry.delete(section, key);
    }

    // ==================== Section Access ====================

    /// Get an entire section as a JSON value.
    pub fn get_section(&self, section: &str) -> Value {
        self.registry.get_section(section)
    }

    /// Replace an entire section.
    pub fn set_section(&self, section: &str, data: Value) {
        self.registry.set_section(section, data.clone());
        self.notify_observers(None, &data, section);
    }

    // ==================== Observer Pattern ====================

    /// Add an observer for config changes.
    ///
    /// The callback receives `(key, value, section)`.
    pub fn add_observer<F>(&self, callback: F)
    where
        F: Fn(Option<&str>, &Value, &str) + Send + Sync + 'static,
    {
        let mut observers = self.observers.lock().unwrap();
        observers.push(Box::new(callback));
    }

    fn notify_observers(&self, key: Option<&str>, value: &Value, section: &str) {
        let observers = self.observers.lock().unwrap();
        for callback in observers.iter() {
            // Don't let observer errors break config operations
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback(key, value, section);
            }))
            .ok();
        }
    }
}
```

## SessionManager (Window State)

Manages ephemeral window state (position, size, etc.):

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Manages window session state (position, size, etc.).
pub struct SessionManager {
    session: Mutex<HashMap<String, Value>>,
    session_file: PathBuf,
}

static SESSION_INSTANCE: OnceLock<SessionManager> = OnceLock::new();

impl SessionManager {
    /// Initialize the global SessionManager.
    pub fn initialize(session_file: &Path) {
        let mut session = HashMap::new();

        if session_file.exists() {
            if let Ok(contents) = fs::read_to_string(session_file) {
                if let Ok(data) = serde_json::from_str::<HashMap<String, Value>>(&contents) {
                    session = data;
                }
            }
        }

        let manager = SessionManager {
            session: Mutex::new(session),
            session_file: session_file.to_path_buf(),
        };

        SESSION_INSTANCE
            .set(manager)
            .expect("SessionManager already initialized");
    }

    /// Get the global SessionManager instance.
    pub fn instance() -> &'static SessionManager {
        SESSION_INSTANCE
            .get()
            .expect("SessionManager not initialized")
    }

    /// Save session to disk.
    pub fn save(&self) {
        let session = self.session.lock().unwrap();
        if let Ok(json) = serde_json::to_string_pretty(&*session) {
            fs::write(&self.session_file, json).ok();
        }
    }

    /// Get a session value, deserialized to the target type.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let session = self.session.lock().unwrap();
        let value = session.get(key)?;
        serde_json::from_value(value.clone()).ok()
    }

    /// Set a session value.
    pub fn set<T: Serialize>(&self, key: &str, value: &T) {
        let mut session = self.session.lock().unwrap();
        if let Ok(json_value) = serde_json::to_value(value) {
            session.insert(key.to_string(), json_value);
        }
    }

    // ==================== Window State ====================

    /// Save window geometry.
    pub fn save_window_geometry(&self, x: i32, y: i32, width: u32, height: u32, maximized: bool) {
        let mut session = self.session.lock().unwrap();
        session.insert("window_x".into(), Value::from(x));
        session.insert("window_y".into(), Value::from(y));
        session.insert("window_width".into(), Value::from(width));
        session.insert("window_height".into(), Value::from(height));
        session.insert("window_maximized".into(), Value::from(maximized));
        drop(session);
        self.save();
    }

    /// Restore window geometry. Returns `(x, y, width, height, maximized)`.
    pub fn restore_window_geometry(&self) -> Option<(i32, i32, u32, u32, bool)> {
        let session = self.session.lock().unwrap();
        Some((
            session.get("window_x")?.as_i64()? as i32,
            session.get("window_y")?.as_i64()? as i32,
            session.get("window_width")?.as_u64()? as u32,
            session.get("window_height")?.as_u64()? as u32,
            session.get("window_maximized")?.as_bool().unwrap_or(false),
        ))
    }
}
```

## Remote Config Sync

Sync configuration with a GitHub repository:

```rust
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Maps a config section to a GitHub file.
#[derive(Debug, Clone)]
pub struct RemoteConfigMapping {
    /// Local config section name.
    pub section_name: String,
    /// Raw GitHub URL for pulling.
    pub github_url: String,
    /// Path in repo for pushing.
    pub repo_path: String,
    /// Whether sync is enabled.
    pub enabled: bool,
    /// Whether pushing changes is enabled.
    pub push_enabled: bool,
}

/// Manages bidirectional config sync with GitHub.
pub struct RemoteConfigManager {
    mappings: Vec<RemoteConfigMapping>,
    sync_interval: Duration,
    running: Arc<AtomicBool>,
    github_token: Option<String>,
}

impl RemoteConfigManager {
    pub fn new(
        mappings: Vec<RemoteConfigMapping>,
        sync_interval: Duration,
    ) -> Self {
        Self {
            mappings,
            sync_interval,
            running: Arc::new(AtomicBool::new(false)),
            github_token: None,
        }
    }

    /// Start background sync.
    pub fn start(&mut self, github_token: Option<String>) {
        self.github_token = github_token;
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let mappings = self.mappings.clone();
        let interval = self.sync_interval;

        thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                sync_all(&mappings);
                thread::sleep(interval);
            }
        });
    }

    /// Stop background sync.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Push config to GitHub (requires token).
    pub fn push_config(&self, mapping: &RemoteConfigMapping) -> bool {
        if self.github_token.is_none() || !mapping.push_enabled {
            return false;
        }

        if crate::version::is_development_mode() {
            // Don't push in dev mode
            return false;
        }

        // Implementation depends on your GitHub setup
        // Use GitHub API to update file
        true
    }
}

fn sync_all(mappings: &[RemoteConfigMapping]) {
    let config = ConfigManager::instance();

    for mapping in mappings {
        if !mapping.enabled {
            continue;
        }

        if let Err(e) = pull_config(config, mapping) {
            tracing::warn!("Failed to sync {}: {e}", mapping.section_name);
        }
    }
}

fn pull_config(config: &ConfigManager, mapping: &RemoteConfigMapping) -> anyhow::Result<()> {
    let resp = reqwest::blocking::get(&mapping.github_url)?;
    if !resp.status().is_success() {
        return Ok(());
    }

    let remote_data: Value = resp.json()?;

    // Merge with local (remote wins for conflicts)
    let mut local_data = config.get_section(&mapping.section_name);
    if let (Some(local_obj), Some(remote_obj)) =
        (local_data.as_object_mut(), remote_data.as_object())
    {
        for (k, v) in remote_obj {
            local_obj.insert(k.clone(), v.clone());
        }
    }

    config.set_section(&mapping.section_name, local_data);
    Ok(())
}
```

## Using Configuration

### Basic Usage

```rust
// Get global instance
let config = ConfigManager::instance();

// Read values
let theme: String = config.get_or("theme", "dark".into(), None);
let window_width: u32 = config.get_or("window_width", 800, None);
let recent_files: Vec<String> = config.get_or("recent_files", vec![], None);

// Write values
config.set("theme", &"light", None);
config.set("window_width", &1024u32, None);

// Sections
let ui_settings = config.get_section("ui");
config.set("font_size", &14u32, Some("ui"));
```

### Observing Changes

```rust
struct SettingsPanel;

impl SettingsPanel {
    fn new() -> Self {
        let config = ConfigManager::instance();
        config.add_observer(|key, value, section| {
            if section == "ui" && key == Some("theme") {
                if let Some(theme) = value.as_str() {
                    apply_theme(theme);
                }
            }
        });

        Self
    }
}
```

### Initialization

```rust
fn init_config() {
    let config_dir = dirs::config_dir()
        .expect("No config directory available")
        .join("your-app-name");

    ConfigManager::initialize(&config_dir);
    SessionManager::initialize(&config_dir.join("session.json"));
}
```

## Configuration Sections

Recommended section organization:

| Section | Purpose | Example Keys |
|---------|---------|--------------|
| `settings` | General app settings | `theme`, `language`, `check_updates` |
| `ui` | UI preferences | `font_size`, `sidebar_width` |
| `directories` | Path settings | `default_directory`, `output_path` |
| `credentials` | Auth info (use keyring for secrets) | `username`, `api_endpoint` |
| `recent` | Recent items | `recent_files`, `recent_projects` |

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [SECURITY-MODEL.md](SECURITY-MODEL.md) - Secure credential storage
- [CROSS-PLATFORM.md](CROSS-PLATFORM.md) - Platform-specific paths
