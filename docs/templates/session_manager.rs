//! session_manager.rs
//!
//! Session state management for iced daemon applications.
//!
//! Features:
//! - Window geometry persistence (position, size, maximized)
//! - Tab/view state tracking
//! - Recent items lists
//! - Generic key-value storage
//! - Thread-safe singleton pattern
//! - Conversion to/from iced window::Settings
//!
//! # Example
//!
//! ```rust
//! use session_manager::SessionManager;
//!
//! let mut session = SessionManager::instance();
//!
//! // Save window geometry
//! session.save_window_state("main", &geometry);
//!
//! // Restore on next launch
//! if let Some(geo) = session.get_window_state("main") {
//!     let settings = geo.to_window_settings();
//!     // Use settings when opening window via window::open(settings)
//! }
//!
//! // Generic key-value storage
//! session.set("last_tab", "settings");
//! let tab = session.get("last_tab").unwrap_or("home");
//! ```

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

// =============================================================================
// WINDOW GEOMETRY
// =============================================================================

/// Window geometry for persistence.
///
/// Framework-agnostic struct that stores raw window dimensions and position.
/// Convert to iced `window::Settings` via `to_window_settings()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Window width in logical pixels
    pub width: f32,
    /// Window height in logical pixels
    pub height: f32,
    /// Window x position in logical pixels
    pub x: i32,
    /// Window y position in logical pixels
    pub y: i32,
    /// Whether window is maximized
    pub maximized: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            width: 1024.0,
            height: 768.0,
            x: 100,
            y: 100,
            maximized: false,
        }
    }
}

impl WindowGeometry {
    /// Create geometry from explicit values.
    pub fn new(width: f32, height: f32, x: i32, y: i32, maximized: bool) -> Self {
        Self { width, height, x, y, maximized }
    }

    /// Convert to iced `window::Settings` for use with `window::open()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use iced::window;
    ///
    /// if let Some(geo) = get_window_state("editor") {
    ///     let (id, open_cmd) = window::open(geo.to_window_settings());
    ///     // track id in your HashMap<window::Id, WindowKind>
    /// }
    /// ```
    pub fn to_window_settings(&self) -> iced::window::Settings {
        iced::window::Settings {
            size: iced::Size::new(self.width, self.height),
            position: iced::window::Position::Specific(iced::Point::new(
                self.x as f32,
                self.y as f32,
            )),
            // Note: iced 0.14 window::Settings does not have a `maximized` field.
            // To maximize after opening, send a window::maximize(id) command in update().
            ..iced::window::Settings::default()
        }
    }

    /// Create geometry from iced size and position values.
    ///
    /// Call this when the window emits a resize or move event so you can
    /// persist the latest geometry before the app exits.
    pub fn from_iced_values(
        size: iced::Size,
        position: iced::Point,
        maximized: bool,
    ) -> Self {
        Self {
            width: size.width,
            height: size.height,
            x: position.x as i32,
            y: position.y as i32,
            maximized,
        }
    }
}

// =============================================================================
// SESSION DATA
// =============================================================================

/// Session data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionData {
    /// Window geometries keyed by window label
    windows: HashMap<String, WindowGeometry>,
    /// Active tab/view for each section
    active_tabs: HashMap<String, String>,
    /// Recent items lists
    recent_items: HashMap<String, Vec<String>>,
    /// Generic key-value storage
    data: HashMap<String, serde_json::Value>,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            windows: HashMap::new(),
            active_tabs: HashMap::new(),
            recent_items: HashMap::new(),
            data: HashMap::new(),
        }
    }
}

// =============================================================================
// SESSION MANAGER
// =============================================================================

/// Thread-safe singleton session manager.
///
/// Persists window geometry, tab state, recent items, and arbitrary
/// key-value data to a JSON file in the platform data directory.
pub struct SessionManager {
    data: Arc<RwLock<SessionData>>,
    file_path: PathBuf,
}

static SESSION_MANAGER: Lazy<Arc<RwLock<Option<SessionManager>>>> =
    Lazy::new(|| Arc::new(RwLock::new(None)));

impl SessionManager {
    /// Initialize the session manager.
    ///
    /// # Arguments
    ///
    /// * `session_file` - Path to session JSON file
    pub fn initialize(session_file: PathBuf) -> Result<(), std::io::Error> {
        let session_data = if session_file.exists() {
            let content = fs::read_to_string(&session_file)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            // Create parent directory if needed
            if let Some(parent) = session_file.parent() {
                fs::create_dir_all(parent)?;
            }
            SessionData::default()
        };

        let manager = SessionManager {
            data: Arc::new(RwLock::new(session_data)),
            file_path: session_file,
        };

        *SESSION_MANAGER.write().unwrap() = Some(manager);

        Ok(())
    }

    /// Get the singleton instance.
    ///
    /// Panics if not initialized.
    pub fn instance() -> Arc<RwLock<SessionData>> {
        SESSION_MANAGER
            .read()
            .unwrap()
            .as_ref()
            .expect("SessionManager not initialized")
            .data
            .clone()
    }

    /// Save session to file.
    pub fn save() -> Result<(), std::io::Error> {
        let session_guard = SESSION_MANAGER.read().unwrap();
        let session = session_guard.as_ref().expect("SessionManager not initialized");

        let data = session.data.read().unwrap();
        let json = serde_json::to_string_pretty(&*data)?;

        fs::write(&session.file_path, json)?;

        Ok(())
    }

    /// Get session file path.
    pub fn get_session_file() -> PathBuf {
        get_session_file_path()
    }
}

// =============================================================================
// WINDOW STATE HELPERS
// =============================================================================

/// Save window geometry.
pub fn save_window_state(label: &str, geometry: WindowGeometry) {
    let data = SessionManager::instance();
    let mut data = data.write().unwrap();
    data.windows.insert(label.to_string(), geometry);
}

/// Get window geometry.
pub fn get_window_state(label: &str) -> Option<WindowGeometry> {
    let data = SessionManager::instance();
    let data = data.read().unwrap();
    data.windows.get(label).cloned()
}

// =============================================================================
// TAB STATE HELPERS
// =============================================================================

/// Save active tab.
pub fn save_active_tab(section: &str, tab: &str) {
    let data = SessionManager::instance();
    let mut data = data.write().unwrap();
    data.active_tabs.insert(section.to_string(), tab.to_string());
}

/// Get active tab.
pub fn get_active_tab(section: &str) -> Option<String> {
    let data = SessionManager::instance();
    let data = data.read().unwrap();
    data.active_tabs.get(section).cloned()
}

// =============================================================================
// RECENT ITEMS HELPERS
// =============================================================================

/// Add item to recent list.
pub fn add_recent_item(list: &str, item: &str, max_items: usize) {
    let data = SessionManager::instance();
    let mut data = data.write().unwrap();

    let recent = data.recent_items.entry(list.to_string())
        .or_insert_with(Vec::new);

    // Remove if already exists
    recent.retain(|i| i != item);

    // Add to front
    recent.insert(0, item.to_string());

    // Trim to max
    recent.truncate(max_items);
}

/// Get recent items.
pub fn get_recent_items(list: &str, limit: usize) -> Vec<String> {
    let data = SessionManager::instance();
    let data = data.read().unwrap();

    data.recent_items
        .get(list)
        .map(|items| items.iter().take(limit).cloned().collect())
        .unwrap_or_default()
}

/// Clear recent items list.
pub fn clear_recent_items(list: &str) {
    let data = SessionManager::instance();
    let mut data = data.write().unwrap();
    data.recent_items.remove(list);
}

// =============================================================================
// GENERIC KEY-VALUE STORAGE
// =============================================================================

/// Set a value.
pub fn set<T: Serialize>(key: &str, value: T) {
    let data = SessionManager::instance();
    let mut data = data.write().unwrap();

    if let Ok(json_value) = serde_json::to_value(value) {
        data.data.insert(key.to_string(), json_value);
    }
}

/// Get a value.
pub fn get<T: for<'de> Deserialize<'de>>(key: &str) -> Option<T> {
    let data = SessionManager::instance();
    let data = data.read().unwrap();

    data.data.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Delete a value.
pub fn delete(key: &str) {
    let data = SessionManager::instance();
    let mut data = data.write().unwrap();
    data.data.remove(key);
}

/// Clear all session data.
pub fn clear() {
    let data = SessionManager::instance();
    let mut data = data.write().unwrap();
    *data = SessionData::default();
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Get default session file path.
fn get_session_file_path() -> PathBuf {
    let app_dir = if cfg!(target_os = "windows") {
        dirs::data_local_dir()
    } else if cfg!(target_os = "macos") {
        dirs::data_dir()
    } else {
        dirs::data_dir()
    }
    .expect("Could not find data directory");

    app_dir
        .join(env!("CARGO_PKG_NAME"))
        .join("session.json")
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window"] }
// serde = { version = "1.0", features = ["derive"] }
// serde_json = "1.0"
// once_cell = "1.19"
// dirs = "5.0"

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_geometry_serialization() {
        let geo = WindowGeometry {
            width: 1024.0,
            height: 768.0,
            x: 100,
            y: 100,
            maximized: false,
        };

        let json = serde_json::to_string(&geo).unwrap();
        let deserialized: WindowGeometry = serde_json::from_str(&json).unwrap();

        assert_eq!(geo.width, deserialized.width);
        assert_eq!(geo.height, deserialized.height);
        assert_eq!(geo.x, deserialized.x);
        assert_eq!(geo.y, deserialized.y);
        assert_eq!(geo.maximized, deserialized.maximized);
    }

    #[test]
    fn test_window_geometry_default() {
        let geo = WindowGeometry::default();
        assert_eq!(geo.width, 1024.0);
        assert_eq!(geo.height, 768.0);
        assert!(!geo.maximized);
    }

    #[test]
    fn test_session_data_default() {
        let data = SessionData::default();
        assert!(data.windows.is_empty());
        assert!(data.active_tabs.is_empty());
        assert!(data.recent_items.is_empty());
    }
}
