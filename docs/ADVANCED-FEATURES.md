# Advanced Features

This guide covers advanced patterns for trait-based plugins, notifications, remote configuration sync, scheduling, and other extensibility features in Rust applications.

## Trait-Based Plugin System

Define a plugin trait and discover/load plugins at runtime using dynamic dispatch or static registration:

### Plugin Trait

```rust
// src/plugins/mod.rs

use std::any::Any;
use std::collections::BTreeMap;

/// Metadata describing a plugin.
pub struct PluginInfo {
    pub name: &'static str,
    pub description: &'static str,
    /// Lower values load first (default 100).
    pub priority: u32,
    /// Whether the plugin requires a working directory to function.
    pub requires_directory: bool,
}

/// Trait that all plugins must implement.
pub trait Plugin: Send + Sync {
    /// Return metadata about this plugin.
    fn info(&self) -> PluginInfo;

    /// Called once when the plugin is loaded.
    fn on_load(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called when the plugin's tab/view becomes active.
    fn on_activate(&mut self) {}

    /// Called when the user navigates away from this plugin.
    fn on_deactivate(&mut self) {}

    /// Render the plugin's UI (iced popup windows or widget elements).
    fn render(&self, ctx: &mut dyn Any);

    /// Called on graceful shutdown.
    fn on_unload(&mut self) {}
}
```

### Static Plugin Registry

For compile-time known plugins, use a simple registry vec:

```rust
// src/plugins/registry.rs

use super::{Plugin, PluginInfo};

/// Build the list of all compiled-in plugins, sorted by priority.
pub fn discover_plugins() -> Vec<Box<dyn Plugin>> {
    let mut plugins: Vec<Box<dyn Plugin>> = vec![
        Box::new(super::export_plugin::ExportPlugin::new()),
        Box::new(super::import_plugin::ImportPlugin::new()),
        Box::new(super::analytics_plugin::AnalyticsPlugin::new()),
    ];

    plugins.sort_by_key(|p| p.info().priority);
    plugins
}
```

### Dynamic Plugin Loading (libloading)

For plugins distributed as shared libraries (`.dll` / `.so` / `.dylib`):

```rust
// src/plugins/dynamic_loader.rs

use std::path::{Path, PathBuf};
use libloading::{Library, Symbol};
use tracing::{info, warn, error};

use super::Plugin;

type CreatePluginFn = unsafe fn() -> Box<dyn Plugin>;

/// Holds a dynamically loaded plugin and its library handle.
struct LoadedPlugin {
    _library: Library,
    plugin: Box<dyn Plugin>,
}

/// Discovers and loads plugin shared libraries from a directory.
pub struct DynamicPluginLoader {
    plugins_dir: PathBuf,
    loaded: Vec<LoadedPlugin>,
}

impl DynamicPluginLoader {
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            loaded: Vec::new(),
        }
    }

    /// Scan the plugins directory and load all valid shared libraries.
    ///
    /// Each library must export a `create_plugin` symbol with signature
    /// `fn() -> Box<dyn Plugin>`.
    pub fn discover(&mut self) -> anyhow::Result<Vec<&dyn Plugin>> {
        let read_dir = match std::fs::read_dir(&self.plugins_dir) {
            Ok(rd) => rd,
            Err(e) => {
                warn!("Plugins directory not found: {}: {e}", self.plugins_dir.display());
                return Ok(Vec::new());
            }
        };

        let extension = std::env::consts::DLL_EXTENSION;

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(extension) {
                continue;
            }

            match self.load_library(&path) {
                Ok(loaded) => {
                    info!("Loaded plugin: {}", loaded.plugin.info().name);
                    self.loaded.push(loaded);
                }
                Err(e) => {
                    error!("Failed to load plugin {}: {e}", path.display());
                }
            }
        }

        // Sort by priority
        self.loaded.sort_by_key(|lp| lp.plugin.info().priority);

        Ok(self.loaded.iter().map(|lp| &*lp.plugin as &dyn Plugin).collect())
    }

    /// Load a single shared library and call its `create_plugin` entry point.
    ///
    /// # Safety
    /// The shared library must be built with the same Rust compiler version
    /// and ABI as the host application.
    fn load_library(&self, path: &Path) -> anyhow::Result<LoadedPlugin> {
        // SAFETY: We trust plugin libraries in the designated directory.
        let library = unsafe { Library::new(path)? };
        let create_fn: Symbol<CreatePluginFn> = unsafe { library.get(b"create_plugin")? };
        let plugin = unsafe { create_fn() };

        Ok(LoadedPlugin {
            _library: library,
            plugin,
        })
    }

    /// Get a loaded plugin by name.
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.loaded
            .iter()
            .find(|lp| lp.plugin.info().name == name)
            .map(|lp| &*lp.plugin as &dyn Plugin)
    }
}
```

### Plugin Crate Template (for dynamic plugins)

Each plugin crate exposes a single entry point:

```rust
// my_plugin/src/lib.rs

use app_core::plugins::{Plugin, PluginInfo};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "My Plugin",
            description: "Does something useful",
            priority: 50,
            requires_directory: true,
        }
    }

    fn on_load(&mut self) -> anyhow::Result<()> {
        tracing::info!("MyPlugin loaded");
        Ok(())
    }

    fn render(&self, _ctx: &mut dyn std::any::Any) {
        // Render UI elements
    }
}

/// Entry point called by the dynamic loader.
///
/// # Safety
/// Must be built with the same compiler/ABI as the host application.
#[no_mangle]
pub extern "Rust" fn create_plugin() -> Box<dyn Plugin> {
    Box::new(MyPlugin)
}
```

## Multi-Channel Notifications

Send notifications to Slack, SMS, webhooks, or desktop using a trait-based channel system:

```rust
// src/notifications.rs

use std::collections::HashMap;
use reqwest::blocking::Client;
use serde_json::json;
use tracing::{error, info};

/// Trait for notification delivery channels.
pub trait NotificationChannel: Send + Sync {
    /// Send a notification. Returns `true` on success.
    fn send(
        &self,
        title: &str,
        message: &str,
        extra: &HashMap<String, String>,
    ) -> bool;
}

/// Slack webhook notifications.
pub struct SlackChannel {
    webhook_url: String,
    client: Client,
}

impl SlackChannel {
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
        }
    }
}

impl NotificationChannel for SlackChannel {
    fn send(
        &self,
        title: &str,
        message: &str,
        extra: &HashMap<String, String>,
    ) -> bool {
        let mut blocks = vec![
            json!({
                "type": "header",
                "text": { "type": "plain_text", "text": title }
            }),
            json!({
                "type": "section",
                "text": { "type": "mrkdwn", "text": message }
            }),
        ];

        if !extra.is_empty() {
            let fields: Vec<_> = extra
                .iter()
                .map(|(k, v)| json!({ "type": "mrkdwn", "text": format!("*{k}*\n{v}") }))
                .collect();
            blocks.push(json!({ "type": "section", "fields": fields }));
        }

        match self.client.post(&self.webhook_url).json(&json!({ "blocks": blocks })).send() {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                error!("Slack notification failed: {e}");
                false
            }
        }
    }
}

/// Generic webhook notifications (Zapier, IFTTT, etc.).
pub struct WebhookChannel {
    webhook_url: String,
    client: Client,
}

impl WebhookChannel {
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
        }
    }
}

impl NotificationChannel for WebhookChannel {
    fn send(
        &self,
        title: &str,
        message: &str,
        extra: &HashMap<String, String>,
    ) -> bool {
        let mut payload = json!({
            "title": title,
            "message": message,
        });

        if let Some(obj) = payload.as_object_mut() {
            for (k, v) in extra {
                obj.insert(k.clone(), json!(v));
            }
        }

        match self.client.post(&self.webhook_url).json(&payload).send() {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                error!("Webhook notification failed: {e}");
                false
            }
        }
    }
}

/// Desktop notification via `notify-rust`.
pub struct DesktopChannel {
    app_name: String,
}

impl DesktopChannel {
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
        }
    }
}

impl NotificationChannel for DesktopChannel {
    fn send(
        &self,
        title: &str,
        message: &str,
        _extra: &HashMap<String, String>,
    ) -> bool {
        match notify_rust::Notification::new()
            .appname(&self.app_name)
            .summary(title)
            .body(message)
            .show()
        {
            Ok(_) => true,
            Err(e) => {
                error!("Desktop notification failed: {e}");
                false
            }
        }
    }
}

/// Manages multiple notification channels.
pub struct NotificationManager {
    channels: HashMap<String, Box<dyn NotificationChannel>>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    pub fn add_channel(&mut self, name: impl Into<String>, channel: Box<dyn NotificationChannel>) {
        self.channels.insert(name.into(), channel);
    }

    pub fn remove_channel(&mut self, name: &str) {
        self.channels.remove(name);
    }

    /// Send to a specific channel by name.
    pub fn notify(
        &self,
        channel_name: &str,
        title: &str,
        message: &str,
    ) -> bool {
        self.channels
            .get(channel_name)
            .map(|ch| ch.send(title, message, &HashMap::new()))
            .unwrap_or(false)
    }

    /// Send to all registered channels. Returns per-channel results.
    pub fn notify_all(
        &self,
        title: &str,
        message: &str,
    ) -> HashMap<String, bool> {
        self.channels
            .iter()
            .map(|(name, ch)| (name.clone(), ch.send(title, message, &HashMap::new())))
            .collect()
    }
}
```

## Remote Configuration Sync

Sync configuration with a remote GitHub repository:

```rust
// src/remote_config.rs

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tracing::{error, info};

const GITHUB_API: &str = "https://api.github.com";

/// Bidirectional config sync with a GitHub repository.
pub struct RemoteConfigSync {
    client: Client,
    repo: String,
    config_file: String,
    branch: String,
    last_sha: Option<String>,
}

#[derive(Deserialize)]
struct GitHubContent {
    sha: String,
    content: String,
}

#[derive(Deserialize)]
struct GitHubPutResponse {
    content: GitHubContentRef,
}

#[derive(Deserialize)]
struct GitHubContentRef {
    sha: String,
}

impl RemoteConfigSync {
    pub fn new(
        github_token: &str,
        repo: impl Into<String>,
        config_file: impl Into<String>,
        branch: impl Into<String>,
    ) -> Result<Self> {
        use reqwest::header::{self, HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("token {github_token}"))?,
        );
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static("application/vnd.github.v3+json"),
        );
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("rust-config-sync"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        Ok(Self {
            client,
            repo: repo.into(),
            config_file: config_file.into(),
            branch: branch.into(),
            last_sha: None,
        })
    }

    /// Pull configuration from the remote repository.
    pub fn pull(&mut self) -> Result<Option<Value>> {
        let url = format!(
            "{GITHUB_API}/repos/{}/contents/{}?ref={}",
            self.repo, self.config_file, self.branch
        );

        let response = self.client.get(&url).send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            info!("Remote config not found");
            return Ok(None);
        }

        let data: GitHubContent = response
            .error_for_status()?
            .json()
            .context("Failed to parse GitHub response")?;

        self.last_sha = Some(data.sha);

        // GitHub returns base64 with embedded newlines
        let cleaned = data.content.replace('\n', "");
        let decoded = BASE64.decode(cleaned)?;
        let content = String::from_utf8(decoded)?;
        let value: Value = serde_json::from_str(&content)?;

        Ok(Some(value))
    }

    /// Push configuration to the remote repository.
    pub fn push(&mut self, config: &Value, message: Option<&str>) -> Result<()> {
        let url = format!(
            "{GITHUB_API}/repos/{}/contents/{}",
            self.repo, self.config_file
        );

        let content = serde_json::to_string_pretty(config)?;
        let encoded = BASE64.encode(content.as_bytes());

        let commit_message = message
            .map(String::from)
            .unwrap_or_else(|| format!("Update config - {}", Utc::now().to_rfc3339()));

        let mut payload = serde_json::json!({
            "message": commit_message,
            "content": encoded,
            "branch": &self.branch,
        });

        if let Some(sha) = &self.last_sha {
            payload["sha"] = serde_json::json!(sha);
        }

        let resp: GitHubPutResponse = self
            .client
            .put(&url)
            .json(&payload)
            .send()?
            .error_for_status()?
            .json()?;

        self.last_sha = Some(resp.content.sha);
        info!("Config pushed to remote");
        Ok(())
    }

    /// Bidirectional sync: merge remote and local (remote wins on conflict).
    pub fn sync(&mut self, local_config: &Value) -> Result<Value> {
        let remote = self.pull()?.unwrap_or(Value::Object(Default::default()));

        // Simple merge strategy: overlay remote on top of local
        let merged = merge_json(local_config, &remote);

        self.push(&merged, None)?;
        Ok(merged)
    }
}

/// Shallow merge of two JSON objects. `overlay` values win on conflict.
fn merge_json(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            let mut merged = b.clone();
            for (k, v) in o {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        }
        (_, overlay) => overlay.clone(),
    }
}

/// Manages config sync with local file caching.
pub struct ConfigSyncManager {
    local_path: PathBuf,
    remote_sync: Option<RemoteConfigSync>,
    config: Value,
}

impl ConfigSyncManager {
    pub fn new(
        local_path: impl Into<PathBuf>,
        remote_sync: Option<RemoteConfigSync>,
    ) -> Self {
        Self {
            local_path: local_path.into(),
            remote_sync,
            config: Value::Object(Default::default()),
        }
    }

    /// Load local config and sync with remote if available.
    pub fn load_and_sync(&mut self) -> Result<&Value> {
        // Load local
        if self.local_path.exists() {
            let text = std::fs::read_to_string(&self.local_path)?;
            self.config = serde_json::from_str(&text)?;
        }

        // Sync with remote
        if let Some(ref mut remote) = self.remote_sync {
            match remote.sync(&self.config) {
                Ok(merged) => {
                    self.config = merged;
                    self.save_local()?;
                }
                Err(e) => {
                    error!("Remote sync failed, using local: {e}");
                }
            }
        }

        Ok(&self.config)
    }

    /// Save config locally and push to remote.
    pub fn save_and_sync(&mut self, config: Value) -> Result<()> {
        self.config = config;
        self.save_local()?;

        if let Some(ref mut remote) = self.remote_sync {
            remote.push(&self.config, None)?;
        }

        Ok(())
    }

    fn save_local(&self) -> Result<()> {
        if let Some(parent) = self.local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&self.local_path, text)?;
        Ok(())
    }
}
```

## Background Task Scheduler

Schedule recurring and one-shot tasks using `tokio`:

```rust
// src/scheduler.rs

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

type TaskFn = Arc<dyn Fn() + Send + Sync>;

struct TaskEntry {
    handle: JoinHandle<()>,
    cancel: Arc<Notify>,
}

/// Background task scheduler built on tokio.
///
/// # Example
/// ```rust,no_run
/// let scheduler = TaskScheduler::new();
///
/// scheduler.schedule_recurring("cleanup", Duration::from_secs(3600), false, || {
///     cleanup_old_files();
/// }).await;
///
/// scheduler.schedule_once("notify", Duration::from_secs(60), || {
///     send_notification();
/// }).await;
///
/// // Later: cancel or shut down
/// scheduler.cancel("cleanup").await;
/// scheduler.stop().await;
/// ```
pub struct TaskScheduler {
    tasks: Arc<Mutex<HashMap<String, TaskEntry>>>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Schedule a one-shot task after `delay`.
    pub async fn schedule_once(
        &self,
        name: impl Into<String>,
        delay: Duration,
        func: impl Fn() + Send + Sync + 'static,
    ) {
        let name = name.into();
        let cancel = Arc::new(Notify::new());
        let cancel_clone = cancel.clone();
        let tasks = self.tasks.clone();
        let task_name = name.clone();

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(delay) => {
                    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&func)) {
                        error!("Task {task_name} panicked: {e:?}");
                    }
                }
                _ = cancel_clone.notified() => {
                    debug!("Task {task_name} cancelled");
                }
            }
            tasks.lock().await.remove(&task_name);
        });

        self.tasks.lock().await.insert(name, TaskEntry { handle, cancel });
    }

    /// Schedule a recurring task at `interval`.
    ///
    /// If `immediate` is true, the function runs once before the first interval.
    pub async fn schedule_recurring(
        &self,
        name: impl Into<String>,
        interval: Duration,
        immediate: bool,
        func: impl Fn() + Send + Sync + 'static,
    ) {
        let name = name.into();
        let cancel = Arc::new(Notify::new());
        let cancel_clone = cancel.clone();
        let task_name = name.clone();
        let func = Arc::new(func) as TaskFn;

        let handle = tokio::spawn(async move {
            if immediate {
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (func)())) {
                    error!("Initial run of {task_name} panicked: {e:?}");
                }
            }

            let mut ticker = tokio::time::interval(interval);
            if !immediate {
                ticker.tick().await; // skip the immediate first tick
            }

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let f = func.clone();
                        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || (f)())) {
                            error!("Recurring task {task_name} panicked: {e:?}");
                        }
                    }
                    _ = cancel_clone.notified() => {
                        debug!("Recurring task {task_name} cancelled");
                        break;
                    }
                }
            }
        });

        self.tasks.lock().await.insert(name, TaskEntry { handle, cancel });
    }

    /// Cancel a specific task by name.
    pub async fn cancel(&self, name: &str) -> bool {
        if let Some(entry) = self.tasks.lock().await.remove(name) {
            entry.cancel.notify_one();
            debug!("Cancelled task: {name}");
            true
        } else {
            false
        }
    }

    /// Check whether a task is currently scheduled.
    pub async fn is_scheduled(&self, name: &str) -> bool {
        self.tasks.lock().await.contains_key(name)
    }

    /// Cancel all tasks and shut down the scheduler.
    pub async fn stop(&self) {
        let mut tasks = self.tasks.lock().await;
        for (name, entry) in tasks.drain() {
            entry.cancel.notify_one();
            debug!("Stopping task: {name}");
        }
        info!("Scheduler stopped");
    }
}
```

## Year Rotation Pattern

Manage year-based directory structures:

```rust
// src/year_rotation.rs

use std::path::{Path, PathBuf};
use anyhow::Result;
use chrono::Datelike;
use tracing::info;

/// Manages year-based directory structures for organizing files by year
/// (logs, exports, archives).
///
/// # Example
/// ```rust,no_run
/// let manager = YearRotationManager::new("/data/exports", true);
///
/// // Get current year directory (creates if missing)
/// let dir = manager.current_year_dir()?;
///
/// // Archive years older than 2 years
/// manager.archive_old_years(2, Some(Path::new("/data/archive")))?;
/// ```
pub struct YearRotationManager {
    base_path: PathBuf,
    create_if_missing: bool,
}

impl YearRotationManager {
    pub fn new(base_path: impl Into<PathBuf>, create_if_missing: bool) -> Self {
        Self {
            base_path: base_path.into(),
            create_if_missing,
        }
    }

    /// Get directory for the current year.
    pub fn current_year_dir(&self) -> Result<PathBuf> {
        self.year_dir(chrono::Utc::now().year())
    }

    /// Get directory for a specific year.
    pub fn year_dir(&self, year: i32) -> Result<PathBuf> {
        let dir = self.base_path.join(year.to_string());
        if self.create_if_missing {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    /// List all year directories, sorted ascending.
    pub fn list_year_dirs(&self) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();

        if !self.base_path.exists() {
            return Ok(dirs);
        }

        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.parse::<i32>().is_ok() {
                        dirs.push(path);
                    }
                }
            }
        }

        dirs.sort();
        Ok(dirs)
    }

    /// Collect all files matching a glob pattern across year directories.
    pub fn all_files(&self, pattern: &str, years: Option<&[i32]>) -> Result<Vec<PathBuf>> {
        let dirs = match years {
            Some(yrs) => yrs.iter().filter_map(|&y| self.year_dir(y).ok()).collect(),
            None => self.list_year_dirs()?,
        };

        let mut files = Vec::new();
        for dir in dirs {
            let glob_pattern = format!("{}/{pattern}", dir.display());
            for entry in glob::glob(&glob_pattern)? {
                if let Ok(path) = entry {
                    files.push(path);
                }
            }
        }

        files.sort();
        Ok(files)
    }

    /// Move or remove directories older than `keep_years`.
    pub fn archive_old_years(
        &self,
        keep_years: i32,
        archive_path: Option<&Path>,
    ) -> Result<Vec<PathBuf>> {
        let current_year = chrono::Utc::now().year();
        let cutoff = current_year - keep_years;
        let mut archived = Vec::new();

        for dir in self.list_year_dirs()? {
            let year: i32 = dir
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if year > 0 && year < cutoff {
                if let Some(archive) = archive_path {
                    let dest = archive.join(dir.file_name().unwrap());
                    std::fs::create_dir_all(archive)?;
                    std::fs::rename(&dir, &dest)?;
                    info!("Archived {} to {}", dir.display(), dest.display());
                } else {
                    std::fs::remove_dir_all(&dir)?;
                    info!("Removed old year directory: {}", dir.display());
                }
                archived.push(dir);
            }
        }

        Ok(archived)
    }

    /// Remove empty year directories.
    pub fn cleanup_empty_years(&self) -> Result<Vec<PathBuf>> {
        let mut removed = Vec::new();

        for dir in self.list_year_dirs()? {
            if std::fs::read_dir(&dir)?.next().is_none() {
                std::fs::remove_dir(&dir)?;
                info!("Removed empty directory: {}", dir.display());
                removed.push(dir);
            }
        }

        Ok(removed)
    }
}
```

## Feature Flags

Runtime feature flag system:

```rust
// src/feature_flags.rs

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use tracing::debug;

/// Thread-safe runtime feature flag manager.
///
/// # Example
/// ```rust,no_run
/// let flags = FeatureFlags::new();
/// flags.set_defaults(&[("new_ui", false), ("beta_export", false)]);
///
/// flags.enable("new_ui");
///
/// if flags.is_enabled("new_ui") {
///     show_new_ui();
/// }
///
/// // Load from remote config
/// flags.load_from_map(&remote_config.feature_flags);
/// ```
pub struct FeatureFlags {
    inner: Arc<RwLock<FlagsInner>>,
}

struct FlagsInner {
    flags: HashMap<String, bool>,
    defaults: HashMap<String, bool>,
    locked: HashSet<String>,
}

impl FeatureFlags {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(FlagsInner {
                flags: HashMap::new(),
                defaults: HashMap::new(),
                locked: HashSet::new(),
            })),
        }
    }

    /// Set default values for flags. Only applies to flags not already set.
    pub fn set_defaults(&self, defaults: &[(&str, bool)]) {
        let mut inner = self.inner.write().unwrap();
        for &(name, value) in defaults {
            inner.defaults.insert(name.to_string(), value);
            inner.flags.entry(name.to_string()).or_insert(value);
        }
    }

    pub fn is_enabled(&self, flag: &str) -> bool {
        let inner = self.inner.read().unwrap();
        inner
            .flags
            .get(flag)
            .or_else(|| inner.defaults.get(flag))
            .copied()
            .unwrap_or(false)
    }

    pub fn enable(&self, flag: &str) {
        let mut inner = self.inner.write().unwrap();
        if !inner.locked.contains(flag) {
            inner.flags.insert(flag.to_string(), true);
            debug!("Enabled flag: {flag}");
        }
    }

    pub fn disable(&self, flag: &str) {
        let mut inner = self.inner.write().unwrap();
        if !inner.locked.contains(flag) {
            inner.flags.insert(flag.to_string(), false);
            debug!("Disabled flag: {flag}");
        }
    }

    pub fn toggle(&self, flag: &str) -> bool {
        let mut inner = self.inner.write().unwrap();
        if inner.locked.contains(flag) {
            return inner.flags.get(flag).copied().unwrap_or(false);
        }
        let current = inner
            .flags
            .get(flag)
            .or_else(|| inner.defaults.get(flag))
            .copied()
            .unwrap_or(false);
        let new_value = !current;
        inner.flags.insert(flag.to_string(), new_value);
        debug!("Toggled flag: {flag} = {new_value}");
        new_value
    }

    pub fn lock(&self, flag: &str) {
        self.inner.write().unwrap().locked.insert(flag.to_string());
    }

    pub fn unlock(&self, flag: &str) {
        self.inner.write().unwrap().locked.remove(flag);
    }

    /// Load flags from a map (e.g., deserialized from remote config).
    pub fn load_from_map(&self, config: &HashMap<String, bool>) {
        let mut inner = self.inner.write().unwrap();
        for (flag, &value) in config {
            if !inner.locked.contains(flag.as_str()) {
                inner.flags.insert(flag.clone(), value);
            }
        }
        debug!("Loaded {} flags from config", config.len());
    }

    /// Export current flag state.
    pub fn to_map(&self) -> HashMap<String, bool> {
        self.inner.read().unwrap().flags.clone()
    }

    /// Reset all flags to their defaults.
    pub fn reset(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.flags = inner.defaults.clone();
        debug!("Reset all flags to defaults");
    }
}

impl Clone for FeatureFlags {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
```

## Integration Example

```rust
// src/main.rs — Wiring advanced features together

use std::path::PathBuf;
use std::time::Duration;

mod plugins;
mod notifications;
mod remote_config;
mod scheduler;
mod feature_flags;

use notifications::{NotificationManager, SlackChannel};
use remote_config::{RemoteConfigSync, ConfigSyncManager};
use scheduler::TaskScheduler;
use feature_flags::FeatureFlags;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::init();

    // 1. Discover plugins
    let plugins = plugins::registry::discover_plugins();
    tracing::info!("Loaded {} plugins", plugins.len());

    // 2. Setup notifications
    let mut notifier = NotificationManager::new();
    notifier.add_channel(
        "slack",
        Box::new(SlackChannel::new("https://hooks.slack.com/...")),
    );

    // 3. Setup config sync
    let remote_sync = RemoteConfigSync::new(
        "ghp_xxx",
        "owner/config-repo",
        "config.json",
        "main",
    )?;
    let mut config_manager = ConfigSyncManager::new(
        PathBuf::from("config/settings.json"),
        Some(remote_sync),
    );
    let config = config_manager.load_and_sync()?;

    // 4. Setup feature flags
    let flags = FeatureFlags::new();
    flags.set_defaults(&[("new_feature", false), ("beta_mode", false)]);
    if let Some(ff) = config.get("feature_flags").and_then(|v| v.as_object()) {
        let map: std::collections::HashMap<String, bool> = ff
            .iter()
            .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
            .collect();
        flags.load_from_map(&map);
    }

    // 5. Setup scheduler
    let scheduler = TaskScheduler::new();

    // Recurring config sync every 5 minutes
    scheduler
        .schedule_recurring("config_sync", Duration::from_secs(300), false, move || {
            // In production, use channels or shared state for the config manager
            tracing::info!("Config sync tick");
        })
        .await;

    // Daily status notification
    let notifier_ref = std::sync::Arc::new(notifier);
    let n = notifier_ref.clone();
    scheduler
        .schedule_recurring("daily_report", Duration::from_secs(86400), true, move || {
            n.notify("slack", "Daily Report", "All systems operational");
        })
        .await;

    // Application runs until shutdown signal...
    tokio::signal::ctrl_c().await?;
    scheduler.stop().await;

    Ok(())
}
```

## See Also

- [UI-ARCHITECTURE.md](UI-ARCHITECTURE.md) - Base window and component patterns
- [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) - Configuration management
- [API-CREDENTIALS.md](API-CREDENTIALS.md) - API authentication
- [templates/](templates/) - Rust template source files
