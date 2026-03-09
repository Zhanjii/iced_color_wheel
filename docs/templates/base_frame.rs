//! base_frame.rs
//!
//! Window management utilities for iced daemon applications.
//!
//! Features:
//! - Window creation helpers (main, popup, dialog)
//! - Window state persistence (position, size)
//! - Multi-window registry and lookup
//! - Popup deduplication (prevent opening the same popup twice)
//!
//! # Iced Daemon Window Model
//!
//! In `iced::daemon`, every window is identified by a `window::Id`.
//! The application owns a `HashMap<window::Id, WindowKind>` and routes
//! `view()` calls by ID. Windows are opened with `window::open()` and
//! closed with `window::close()`.
//!
//! This module provides helpers to standardize window settings and
//! persist/restore geometry across sessions.
//!
//! # Example
//!
//! ```rust
//! use base_frame::{WindowSettings, open_popup, WindowGeometry};
//!
//! // Open a popup window
//! let settings = WindowSettings::popup("Settings", 500.0, 400.0);
//! let (id, task) = open_popup(settings);
//!
//! // Persist window geometry
//! let geo = WindowGeometry::new(1024.0, 768.0, Some((100, 200)));
//! geo.save("main")?;
//! let restored = WindowGeometry::load("main");
//! ```

use iced::window;
use iced::Size;
use serde::{Deserialize, Serialize};

// =============================================================================
// WINDOW SETTINGS BUILDER
// =============================================================================

/// Convenience builder for `window::Settings`.
///
/// Wraps iced's `window::Settings` with named constructors for common
/// window types (main, popup, dialog).
pub struct WindowSettings {
    pub title: String,
    pub size: Size,
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
    pub resizable: bool,
    pub position: window::Position,
    pub visible: bool,
    pub decorations: bool,
    pub level: window::Level,
    pub exit_on_close_request: bool,
}

impl WindowSettings {
    /// Main application window with sensible defaults.
    pub fn main(title: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            title: title.into(),
            size: Size::new(width, height),
            min_size: Some(Size::new(800.0, 600.0)),
            max_size: None,
            resizable: true,
            position: window::Position::Centered,
            visible: true,
            decorations: true,
            level: window::Level::Normal,
            exit_on_close_request: false,
        }
    }

    /// Popup window (settings, preferences, etc.) — resizable, normal level.
    pub fn popup(title: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            title: title.into(),
            size: Size::new(width, height),
            min_size: Some(Size::new(width, height)),
            max_size: None,
            resizable: false,
            position: window::Position::Centered,
            visible: true,
            decorations: true,
            level: window::Level::Normal,
            exit_on_close_request: false,
        }
    }

    /// Dialog window — non-resizable, always on top.
    pub fn dialog(title: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            title: title.into(),
            size: Size::new(width, height),
            min_size: Some(Size::new(width, height)),
            max_size: Some(Size::new(width, height)),
            resizable: false,
            position: window::Position::Centered,
            visible: true,
            decorations: true,
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: false,
        }
    }

    // Builder modifiers

    /// Set minimum size constraint.
    pub fn with_min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Some(Size::new(width, height));
        self
    }

    /// Restore geometry from a saved session.
    pub fn with_geometry(mut self, geo: &WindowGeometry) -> Self {
        self.size = Size::new(geo.width, geo.height);
        if let Some((x, y)) = geo.position {
            self.position = window::Position::Specific(iced::Point::new(x as f32, y as f32));
        }
        self
    }

    /// Convert to iced's `window::Settings`.
    pub fn into_iced(self) -> window::Settings {
        window::Settings {
            size: self.size,
            min_size: self.min_size,
            max_size: self.max_size,
            resizable: self.resizable,
            position: self.position,
            visible: self.visible,
            decorations: self.decorations,
            level: self.level,
            exit_on_close_request: self.exit_on_close_request,
            ..window::Settings::default()
        }
    }
}

// =============================================================================
// WINDOW OPEN HELPERS
// =============================================================================

/// Open a window with the given settings. Returns (id, task).
///
/// The caller must insert the returned `window::Id` into the app's
/// window registry before the task completes.
pub fn open_window<M: 'static>(settings: WindowSettings) -> (window::Id, iced::Task<M>) {
    let (id, task) = window::open(settings.into_iced());
    (id, task.discard())
}

/// Close a window by ID. Returns a task that must be returned from `update()`.
pub fn close_window<M: 'static>(id: window::Id) -> iced::Task<M> {
    window::close(id)
}

// =============================================================================
// WINDOW GEOMETRY PERSISTENCE
// =============================================================================

/// Serializable window geometry for session persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub width: f32,
    pub height: f32,
    pub position: Option<(i32, i32)>,
    pub maximized: bool,
}

impl WindowGeometry {
    pub fn new(width: f32, height: f32, position: Option<(i32, i32)>) -> Self {
        Self {
            width,
            height,
            position,
            maximized: false,
        }
    }

    /// Save geometry to the session file under the given key.
    pub fn save(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = geometry_path(key);
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load geometry from the session file. Returns `None` if not found.
    pub fn load(key: &str) -> Option<Self> {
        let path = geometry_path(key);
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }
}

fn geometry_path(key: &str) -> std::path::PathBuf {
    // Use the directories crate in real code:
    // directories::ProjectDirs::from("", "", "AppName")
    //     .map(|d| d.data_dir().join(format!("window_{key}.json")))
    //     .unwrap_or_else(|| ...)
    std::path::PathBuf::from(format!("window_{key}.json"))
}

// =============================================================================
// POPUP DEDUPLICATION
// =============================================================================

/// Helper trait for the app's window registry to prevent duplicate popups.
///
/// # Example
///
/// ```rust
/// use std::collections::HashMap;
///
/// impl App {
///     fn open_settings(&mut self) -> Task<Message> {
///         if self.windows.has_kind(|w| matches!(w, WindowKind::Settings(_))) {
///             return Task::none();
///         }
///         let (id, task) = open_window(WindowSettings::popup("Settings", 500.0, 400.0));
///         self.windows.insert(id, WindowKind::Settings(SettingsState::default()));
///         task
///     }
/// }
/// ```
pub trait WindowRegistry<V> {
    /// Check if any window matches the predicate.
    fn has_kind(&self, f: impl Fn(&V) -> bool) -> bool;

    /// Find the first window matching the predicate, returning its ID.
    fn find_kind(&self, f: impl Fn(&V) -> bool) -> Option<window::Id>;
}

impl<V> WindowRegistry<V> for std::collections::HashMap<window::Id, V> {
    fn has_kind(&self, f: impl Fn(&V) -> bool) -> bool {
        self.values().any(f)
    }

    fn find_kind(&self, f: impl Fn(&V) -> bool) -> Option<window::Id> {
        self.iter()
            .find_map(|(id, v)| if f(v) { Some(*id) } else { None })
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.13", features = ["multi-window"] }
// serde = { version = "1", features = ["derive"] }
// serde_json = "1"

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_window_settings() {
        let settings = WindowSettings::main("Test App", 1024.0, 768.0);
        assert_eq!(settings.size, Size::new(1024.0, 768.0));
        assert!(settings.resizable);
        assert!(!settings.exit_on_close_request);
    }

    #[test]
    fn test_popup_settings() {
        let settings = WindowSettings::popup("Settings", 500.0, 400.0);
        assert_eq!(settings.size, Size::new(500.0, 400.0));
        assert!(!settings.resizable);
    }

    #[test]
    fn test_dialog_settings() {
        let settings = WindowSettings::dialog("Confirm", 300.0, 150.0);
        assert!(!settings.resizable);
        assert!(matches!(settings.level, window::Level::AlwaysOnTop));
    }

    #[test]
    fn test_geometry_roundtrip() {
        let geo = WindowGeometry::new(1024.0, 768.0, Some((100, 200)));
        let json = serde_json::to_string(&geo).unwrap();
        let restored: WindowGeometry = serde_json::from_str(&json).unwrap();
        assert_eq!(geo.width, restored.width);
        assert_eq!(geo.height, restored.height);
        assert_eq!(geo.position, restored.position);
    }

    #[test]
    fn test_with_geometry() {
        let geo = WindowGeometry::new(800.0, 600.0, Some((50, 75)));
        let settings = WindowSettings::main("Test", 1024.0, 768.0).with_geometry(&geo);
        assert_eq!(settings.size, Size::new(800.0, 600.0));
    }

    #[test]
    fn test_window_registry() {
        use std::collections::HashMap;

        let mut windows: HashMap<window::Id, &str> = HashMap::new();
        let id = window::Id::unique();
        windows.insert(id, "main");

        assert!(windows.has_kind(|w| *w == "main"));
        assert!(!windows.has_kind(|w| *w == "settings"));
        assert_eq!(windows.find_kind(|w| *w == "main"), Some(id));
    }
}
