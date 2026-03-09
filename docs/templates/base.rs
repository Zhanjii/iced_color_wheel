//! base.rs
//!
//! Base application skeleton for an iced daemon (multi-window).
//!
//! Features:
//! - `iced::daemon` entry point with theme, subscription, run_with
//! - App struct with `HashMap<window::Id, WindowKind>` window registry
//! - `update()` message dispatch
//! - `view()` dispatch by window ID
//! - Deferred initialization (background tasks after startup)
//! - Window lifecycle (open, close, focus)
//! - Logging setup
//!
//! # Example
//!
//! ```rust
//! fn main() -> iced::Result {
//!     setup_logging();
//!
//!     iced::daemon("My App", App::update, App::view)
//!         .theme(App::theme)
//!         .subscription(App::subscription)
//!         .run_with(App::new)
//! }
//! ```

use iced::widget::{button, center, column, container, row, text};
use iced::window;
use iced::{Element, Size, Subscription, Task, Theme};
use std::collections::HashMap;

// =============================================================================
// APPLICATION CONFIGURATION
// =============================================================================

/// Application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Application name
    pub name: String,
    /// Version string
    pub version: String,
    /// Whether running in development mode
    pub dev_mode: bool,
    /// Whether to enable system tray
    pub enable_tray: bool,
    /// Whether to close to tray instead of quitting
    pub close_to_tray: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            dev_mode: cfg!(debug_assertions),
            enable_tray: true,
            close_to_tray: false,
        }
    }
}

// =============================================================================
// WINDOW KIND
// =============================================================================

/// Identifies what kind of content a window shows.
///
/// The `view()` method dispatches on this to render the correct UI.
/// Add your own variants as you add window types.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowKind {
    /// Primary application window
    Main,
    /// Settings/preferences window
    Settings,
    /// About dialog
    About,
    /// Log viewer popup
    LogViewer,
    // Add more window types here as needed
}

// =============================================================================
// MESSAGE
// =============================================================================

/// Top-level application message.
#[derive(Debug, Clone)]
pub enum Message {
    // -- Window lifecycle --
    /// A window was closed by the user or OS
    WindowClosed(window::Id),
    /// Open a new window of the given kind
    OpenWindow(WindowKind),

    // -- Deferred init --
    /// Fired once after startup to run background initialization
    DeferredInit,

    // -- Main window messages --
    /// Placeholder for main window UI events
    MainAction(String),

    // -- Settings window messages --
    /// Placeholder for settings window events
    SettingsSaved,

    // -- Tray events (if using system_tray module) --
    // TrayEvent(system_tray::TrayEvent),

    // -- Add your own messages here --
}

// =============================================================================
// APPLICATION STATE
// =============================================================================

/// Main application state for the iced daemon.
pub struct App {
    /// Window registry: maps each open window ID to its kind
    windows: HashMap<window::Id, WindowKind>,
    /// Application configuration
    config: AppConfig,
    /// Whether deferred initialization has completed
    initialized: bool,
}

impl App {
    /// Create the application and open the main window.
    ///
    /// Called by `iced::daemon(...).run_with(App::new)`.
    fn new() -> (Self, Task<Message>) {
        let config = AppConfig::default();

        // Open the main window
        let main_settings = window::Settings {
            size: Size::new(1200.0, 800.0),
            min_size: Some(Size::new(600.0, 400.0)),
            ..window::Settings::default()
        };

        let (main_id, open_main) = window::open(main_settings);

        let mut windows = HashMap::new();
        windows.insert(main_id, WindowKind::Main);

        log::info!("{} v{} starting", config.name, config.version);

        let app = Self {
            windows,
            config,
            initialized: false,
        };

        // Chain: open main window, then fire deferred init after a short delay
        let tasks = Task::batch(vec![
            open_main.discard(),
            Task::perform(
                async { tokio::time::sleep(std::time::Duration::from_millis(100)).await },
                |_| Message::DeferredInit,
            ),
        ]);

        (app, tasks)
    }

    // =========================================================================
    // UPDATE
    // =========================================================================

    /// Central update function -- all messages flow through here.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // -- Window lifecycle ------------------------------------------
            Message::WindowClosed(id) => {
                let kind = self.windows.remove(&id);
                log::debug!("Window closed: {:?} (kind: {:?})", id, kind);

                if self.windows.is_empty() {
                    if self.config.close_to_tray {
                        // Daemon mode: stay alive for tray icon
                        log::info!("All windows closed, staying in tray");
                        Task::none()
                    } else {
                        // No tray: exit the application
                        iced::exit()
                    }
                } else {
                    Task::none()
                }
            }

            Message::OpenWindow(kind) => {
                // Prevent duplicate windows of the same kind
                if let Some((&existing_id, _)) = self.windows.iter()
                    .find(|(_, k)| **k == kind)
                {
                    return window::gain_focus(existing_id);
                }

                let settings = match &kind {
                    WindowKind::Main => window::Settings {
                        size: Size::new(1200.0, 800.0),
                        ..Default::default()
                    },
                    WindowKind::Settings => window::Settings {
                        size: Size::new(500.0, 400.0),
                        ..Default::default()
                    },
                    WindowKind::About => window::Settings {
                        size: Size::new(350.0, 250.0),
                        resizable: false,
                        ..Default::default()
                    },
                    WindowKind::LogViewer => window::Settings {
                        size: Size::new(700.0, 500.0),
                        ..Default::default()
                    },
                };

                let (id, open_task) = window::open(settings);
                self.windows.insert(id, kind);
                open_task.discard()
            }

            // -- Deferred init ---------------------------------------------
            Message::DeferredInit => {
                if self.initialized {
                    return Task::none();
                }
                self.initialized = true;
                log::info!("Deferred initialization complete");

                // TODO: Load session state, start background watchers, etc.
                Task::none()
            }

            // -- Main window -----------------------------------------------
            Message::MainAction(action) => {
                log::debug!("Main action: {}", action);
                Task::none()
            }

            // -- Settings --------------------------------------------------
            Message::SettingsSaved => {
                log::info!("Settings saved");
                Task::none()
            }
        }
    }

    // =========================================================================
    // VIEW
    // =========================================================================

    /// Dispatch view rendering by window ID.
    ///
    /// Each window kind gets its own view function. Unknown window IDs
    /// (which should not happen) render an error placeholder.
    fn view(&self, id: window::Id) -> Element<Message> {
        match self.windows.get(&id) {
            Some(WindowKind::Main) => self.view_main(),
            Some(WindowKind::Settings) => self.view_settings(),
            Some(WindowKind::About) => self.view_about(),
            Some(WindowKind::LogViewer) => self.view_log_viewer(),
            None => {
                // Window ID not in registry (should not happen)
                center(text("Unknown window").size(16)).into()
            }
        }
    }

    // -- Per-window views --------------------------------------------------

    fn view_main(&self) -> Element<Message> {
        let title = if self.config.dev_mode {
            format!("{} v{} [DEV]", self.config.name, self.config.version)
        } else {
            format!("{} v{}", self.config.name, self.config.version)
        };

        let content = column![
            text(title).size(20),
            row![
                button("Settings")
                    .on_press(Message::OpenWindow(WindowKind::Settings)),
                button("Logs")
                    .on_press(Message::OpenWindow(WindowKind::LogViewer)),
                button("About")
                    .on_press(Message::OpenWindow(WindowKind::About)),
            ]
            .spacing(8),
        ]
        .spacing(16)
        .padding(20);

        container(content).into()
    }

    fn view_settings(&self) -> Element<Message> {
        let content = column![
            text("Settings").size(18),
            button("Save").on_press(Message::SettingsSaved),
        ]
        .spacing(12)
        .padding(20);

        container(content).into()
    }

    fn view_about(&self) -> Element<Message> {
        let content = column![
            text(&self.config.name).size(20),
            text(format!("Version {}", self.config.version)).size(14),
        ]
        .spacing(8)
        .padding(20);

        center(content).into()
    }

    fn view_log_viewer(&self) -> Element<Message> {
        let content = column![
            text("Log Viewer").size(18),
            text("(log content would go here)").size(12),
        ]
        .spacing(8)
        .padding(20);

        container(content).into()
    }

    // =========================================================================
    // THEME
    // =========================================================================

    /// Return the theme for each window.
    fn theme(&self, _id: window::Id) -> Theme {
        Theme::Dark
    }

    // =========================================================================
    // SUBSCRIPTION
    // =========================================================================

    /// Global subscriptions: window close events, tray polling, etc.
    fn subscription(&self) -> Subscription<Message> {
        let close_events = window::close_events().map(Message::WindowClosed);

        // Add more subscriptions here:
        // let tray = system_tray::tray_events().map(Message::TrayEvent);

        Subscription::batch(vec![
            close_events,
            // tray,
        ])
    }
}

// =============================================================================
// ENTRY POINT
// =============================================================================

/// Application entry point.
///
/// Uses `iced::daemon` so the process stays alive even when all windows
/// are closed (required for system tray integration).
fn main() -> iced::Result {
    setup_logging();

    iced::daemon("My App", App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .run_with(App::new)
}

/// Initialize logging.
fn setup_logging() {
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    env_logger::Builder::from_default_env()
        .filter_level(level)
        .init();
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window", "canvas", "tokio"] }
// log = "0.4"
// env_logger = "0.11"
// tokio = { version = "1", features = ["time"] }

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(!config.name.is_empty());
        assert!(!config.version.is_empty());
        assert_eq!(config.dev_mode, cfg!(debug_assertions));
    }

    #[test]
    fn test_window_kind_equality() {
        assert_eq!(WindowKind::Main, WindowKind::Main);
        assert_ne!(WindowKind::Main, WindowKind::Settings);
        assert_ne!(WindowKind::Settings, WindowKind::About);
    }

    #[test]
    fn test_version() {
        let version = env!("CARGO_PKG_VERSION").to_string();
        assert!(!version.is_empty());
    }
}
