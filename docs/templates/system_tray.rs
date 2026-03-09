//! system_tray.rs
//!
//! System tray integration using the `tray-icon` crate (standalone).
//!
//! Designed for iced daemon applications where the app stays alive after
//! all windows close. The tray icon runs independently via `tray-icon`,
//! and menu events are polled through an `iced::Subscription`.
//!
//! Features:
//! - TrayManager with start/stop/update_tooltip
//! - Menu creation with tray_icon::menu::Menu
//! - Event handling with MenuEvent::receiver()
//! - iced Subscription integration for polling tray events
//! - Cross-platform support (Windows, macOS, Linux)
//!
//! # Example
//!
//! ```rust
//! use system_tray::TrayManager;
//!
//! // In your App::new() or deferred init:
//! let tray = TrayManager::start("My App", include_bytes!("../icons/tray.png"))?;
//!
//! // In your App::subscription():
//! fn subscription(&self) -> iced::Subscription<Message> {
//!     system_tray::tray_events().map(Message::TrayEvent)
//! }
//! ```

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

// =============================================================================
// TRAY MENU IDS
// =============================================================================

/// Well-known menu item IDs for matching in event handlers.
pub mod menu_ids {
    pub const SHOW: &str = "show";
    pub const HIDE: &str = "hide";
    pub const SETTINGS: &str = "settings";
    pub const ABOUT: &str = "about";
    pub const QUIT: &str = "quit";
}

// =============================================================================
// TRAY EVENT (iced integration)
// =============================================================================

/// Events emitted by the tray subscription.
///
/// Match on these in your `App::update()` to react to tray menu clicks.
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// User clicked "Show Window"
    ShowWindow,
    /// User clicked "Hide Window"
    HideWindow,
    /// User clicked "Settings"
    OpenSettings,
    /// User clicked "About"
    OpenAbout,
    /// User clicked "Quit"
    Quit,
    /// An unknown menu item was clicked (carries the raw ID string)
    Unknown(String),
}

// =============================================================================
// TRAY MANAGER
// =============================================================================

/// Manages the system tray icon and menu.
///
/// Create with `TrayManager::start()`, which builds the icon and menu.
/// Drop the manager (or call `stop()`) to remove the tray icon.
pub struct TrayManager {
    _tray: TrayIcon,
}

impl TrayManager {
    /// Start the system tray with a tooltip and icon.
    ///
    /// # Arguments
    ///
    /// * `tooltip` - Hover text for the tray icon
    /// * `icon_rgba` - RGBA image bytes for the tray icon (PNG decoded)
    /// * `icon_width` - Icon width in pixels
    /// * `icon_height` - Icon height in pixels
    ///
    /// # Errors
    ///
    /// Returns an error if the icon or tray cannot be created.
    ///
    /// # Example
    ///
    /// ```rust
    /// // Decode a PNG at build time or runtime, then pass RGBA bytes:
    /// let icon_image = image::load_from_memory(include_bytes!("../icons/tray.png"))
    ///     .unwrap()
    ///     .into_rgba8();
    /// let (w, h) = icon_image.dimensions();
    /// let tray = TrayManager::start("My App", icon_image.into_raw(), w, h)?;
    /// ```
    pub fn start(
        tooltip: &str,
        icon_rgba: Vec<u8>,
        icon_width: u32,
        icon_height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let icon = Icon::from_rgba(icon_rgba, icon_width, icon_height)?;
        let menu = Self::build_menu();

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .build()?;

        Ok(Self { _tray: tray })
    }

    /// Build the default tray menu.
    fn build_menu() -> Menu {
        let menu = Menu::new();

        let show = MenuItem::with_id(menu_ids::SHOW, "Show Window", true, None);
        let hide = MenuItem::with_id(menu_ids::HIDE, "Hide Window", true, None);
        let settings = MenuItem::with_id(menu_ids::SETTINGS, "Settings", true, None);
        let about = MenuItem::with_id(menu_ids::ABOUT, "About", true, None);
        let quit = MenuItem::with_id(menu_ids::QUIT, "Quit", true, None);

        let _ = menu.append(&show);
        let _ = menu.append(&hide);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&settings);
        let _ = menu.append(&about);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);

        menu
    }

    /// Update the tray tooltip text.
    ///
    /// Useful for showing status information on hover (e.g. "3 tasks running").
    pub fn update_tooltip(&self, tooltip: &str) -> Result<(), tray_icon::Error> {
        self._tray.set_tooltip(Some(tooltip))
    }

    /// Stop and remove the tray icon.
    ///
    /// This is also called automatically when the TrayManager is dropped.
    pub fn stop(self) {
        // Dropping self._tray removes the icon from the system tray.
        drop(self);
    }
}

// =============================================================================
// ICED SUBSCRIPTION
// =============================================================================

/// Create an iced `Subscription` that polls `tray-icon` menu events.
///
/// Use this in your `App::subscription()` method:
///
/// ```rust
/// fn subscription(&self) -> iced::Subscription<Message> {
///     iced::Subscription::batch(vec![
///         system_tray::tray_events().map(Message::TrayEvent),
///         // ... other subscriptions
///     ])
/// }
/// ```
///
/// In your `App::update()`, handle `Message::TrayEvent(event)`:
///
/// ```rust
/// Message::TrayEvent(TrayEvent::ShowWindow) => {
///     // Open or focus your main window
///     if let Some(&id) = self.windows.iter()
///         .find(|(_, kind)| matches!(kind, WindowKind::Main))
///         .map(|(id, _)| id)
///     {
///         return window::gain_focus(id);
///     }
/// }
/// Message::TrayEvent(TrayEvent::Quit) => {
///     // Close all windows and exit
///     return iced::exit();
/// }
/// ```
pub fn tray_events() -> iced::Subscription<TrayEvent> {
    iced::time::every(std::time::Duration::from_millis(100))
        .map(|_| {
            // Drain all pending menu events
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                let id_str = event.id.0.as_ref();
                match id_str {
                    id if id == menu_ids::SHOW => TrayEvent::ShowWindow,
                    id if id == menu_ids::HIDE => TrayEvent::HideWindow,
                    id if id == menu_ids::SETTINGS => TrayEvent::OpenSettings,
                    id if id == menu_ids::ABOUT => TrayEvent::OpenAbout,
                    id if id == menu_ids::QUIT => TrayEvent::Quit,
                    other => TrayEvent::Unknown(other.to_string()),
                }
            } else {
                // No event pending -- caller should filter this out
                TrayEvent::Unknown(String::new())
            }
        })
}

// NOTE: The subscription above emits TrayEvent::Unknown("") on every tick
// when no menu event is pending. In practice, filter these out in update():
//
//   Message::TrayEvent(TrayEvent::Unknown(id)) if id.is_empty() => {
//       // Ignore polling ticks with no event
//       iced::Task::none()
//   }
//
// A more sophisticated approach uses iced::subscription::channel to build
// a proper async stream. The polling approach above is simpler to template.

// =============================================================================
// CLOSE-TO-TRAY HELPER
// =============================================================================

/// Decide whether the app should hide to tray instead of exiting.
///
/// Call this from your `window::close_events()` handler:
///
/// ```rust
/// Message::WindowClosed(id) => {
///     self.windows.remove(&id);
///     if self.windows.is_empty() && should_close_to_tray() {
///         // All windows closed, but we stay alive (daemon mode).
///         // The tray icon keeps the event loop running.
///         iced::Task::none()
///     } else if self.windows.is_empty() {
///         iced::exit()
///     } else {
///         window::close(id)
///     }
/// }
/// ```
pub fn should_close_to_tray() -> bool {
    // TODO: Read from application config / settings file
    true
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window", "tokio"] }
// tray-icon = "0.19"
// image = "0.25"     # For decoding PNG to RGBA bytes

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_event_variants() {
        // Ensure all variants can be constructed
        let events = vec![
            TrayEvent::ShowWindow,
            TrayEvent::HideWindow,
            TrayEvent::OpenSettings,
            TrayEvent::OpenAbout,
            TrayEvent::Quit,
            TrayEvent::Unknown("custom_id".to_string()),
        ];
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn test_menu_ids_are_distinct() {
        let ids = [
            menu_ids::SHOW,
            menu_ids::HIDE,
            menu_ids::SETTINGS,
            menu_ids::ABOUT,
            menu_ids::QUIT,
        ];
        // All IDs should be unique
        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "Menu IDs must be unique");
                }
            }
        }
    }

    #[test]
    fn test_should_close_to_tray_default() {
        // Default behavior is to close to tray
        assert!(should_close_to_tray());
    }
}
