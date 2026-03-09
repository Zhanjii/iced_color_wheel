//! main.rs
//!
//! Iced daemon application entry point.
//!
//! Features:
//! - Multi-window support via `iced::daemon`
//! - Popup windows (settings, about, color picker, etc.)
//! - Window lifecycle management (open, close, track)
//! - System tray integration
//! - Session persistence (window geometry)
//!
//! Iced daemon vs iced::application:
//! - `daemon` does NOT exit when the last window closes
//! - Each window gets its own `view()` dispatch via window ID
//! - Windows are opened/closed with `window::open()` / `window::close()`
//! - Ideal for tray-resident apps and multi-window workflows

use std::collections::HashMap;

use iced::widget::{button, column, container, horizontal_space, row, text};
use iced::window;
use iced::{daemon, Element, Size, Task, Theme};

// Import your modules
// mod version;
// mod utils;
// mod ui;

// =============================================================================
// MAIN FUNCTION
// =============================================================================

fn main() -> iced::Result {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(if cfg!(debug_assertions) { "debug" } else { "info" })
        .init();

    tracing::info!("Starting application v{}", env!("CARGO_PKG_VERSION"));

    daemon("App Name", App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .run_with(App::new)
}

// =============================================================================
// APPLICATION STATE
// =============================================================================

/// Root application state.
///
/// The daemon owns all window state and routes messages/views by window ID.
struct App {
    /// Tracked windows: ID -> kind + per-window state.
    windows: HashMap<window::Id, WindowKind>,
    /// ID of the main window (first opened).
    main_id: window::Id,
    /// Shared application state.
    config: ConfigManager,
    session: SessionManager,
}

/// Discriminant for each open window.
enum WindowKind {
    Main(MainState),
    Settings(SettingsState),
    About,
    // Add more popup kinds here:
    // ColorPicker(ColorPickerState),
    // LogViewer(LogViewerState),
}

// =============================================================================
// PER-WINDOW STATE
// =============================================================================

#[derive(Default)]
struct MainState {
    working_directory: Option<std::path::PathBuf>,
    status: String,
}

#[derive(Default)]
struct SettingsState {
    check_updates: bool,
    close_to_tray: bool,
    theme: String,
    dirty: bool,
}

// =============================================================================
// MESSAGES
// =============================================================================

#[derive(Debug, Clone)]
enum Message {
    // Window lifecycle
    OpenSettings,
    OpenAbout,
    CloseWindow(window::Id),
    WindowClosed(window::Id),

    // Main window
    BrowseDirectory,
    DirectorySelected(Option<std::path::PathBuf>),

    // Settings
    SettingsCheckUpdatesToggled(bool),
    SettingsCloseToTrayToggled(bool),
    SettingsThemeChanged(String),
    SettingsSave,
    SettingsCancel,

    // Global
    Quit,
}

// =============================================================================
// APPLICATION LOGIC
// =============================================================================

impl App {
    /// Create the application and open the main window.
    fn new() -> (Self, Task<Message>) {
        let session = SessionManager::load();

        // Open the main window with restored geometry
        let size = session
            .window_size
            .unwrap_or(Size::new(1024.0, 768.0));

        let (main_id, open_main) = window::open(window::Settings {
            size,
            min_size: Some(Size::new(800.0, 600.0)),
            exit_on_close_request: false,
            ..window::Settings::default()
        });

        let app = Self {
            windows: HashMap::from([(main_id, WindowKind::Main(MainState::default()))]),
            main_id,
            config: ConfigManager::load_or_default(),
            session,
        };

        (app, open_main.discard())
    }

    /// Handle all messages.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // ---- Window lifecycle ----------------------------------------

            Message::OpenSettings => {
                // Prevent duplicate settings windows
                if self.has_window_kind(|w| matches!(w, WindowKind::Settings(_))) {
                    return Task::none();
                }
                let (id, open) = window::open(window::Settings {
                    size: Size::new(500.0, 400.0),
                    resizable: false,
                    exit_on_close_request: false,
                    ..window::Settings::default()
                });
                self.windows.insert(
                    id,
                    WindowKind::Settings(SettingsState {
                        check_updates: self.config.get_bool("check_updates"),
                        close_to_tray: self.config.get_bool("close_to_tray"),
                        theme: self.config.get_string("theme"),
                        dirty: false,
                    }),
                );
                open.discard()
            }

            Message::OpenAbout => {
                if self.has_window_kind(|w| matches!(w, WindowKind::About)) {
                    return Task::none();
                }
                let (id, open) = window::open(window::Settings {
                    size: Size::new(400.0, 300.0),
                    resizable: false,
                    exit_on_close_request: false,
                    ..window::Settings::default()
                });
                self.windows.insert(id, WindowKind::About);
                open.discard()
            }

            Message::CloseWindow(id) => {
                self.windows.remove(&id);
                window::close(id)
            }

            Message::WindowClosed(id) => {
                self.windows.remove(&id);
                // If main window closed, quit
                if id == self.main_id {
                    return self.shutdown();
                }
                Task::none()
            }

            // ---- Main window --------------------------------------------

            Message::BrowseDirectory => {
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Select Directory")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::DirectorySelected,
                )
            }

            Message::DirectorySelected(path) => {
                if let Some(WindowKind::Main(state)) = self.windows.get_mut(&self.main_id) {
                    if let Some(ref p) = path {
                        state.status = format!("Selected: {}", p.display());
                    }
                    state.working_directory = path;
                }
                Task::none()
            }

            // ---- Settings -----------------------------------------------

            Message::SettingsCheckUpdatesToggled(val) => {
                self.with_settings(|s| {
                    s.check_updates = val;
                    s.dirty = true;
                });
                Task::none()
            }

            Message::SettingsCloseToTrayToggled(val) => {
                self.with_settings(|s| {
                    s.close_to_tray = val;
                    s.dirty = true;
                });
                Task::none()
            }

            Message::SettingsThemeChanged(theme) => {
                self.with_settings(|s| {
                    s.theme = theme;
                    s.dirty = true;
                });
                Task::none()
            }

            Message::SettingsSave => {
                if let Some((id, settings)) = self.find_settings() {
                    self.config.set_bool("check_updates", settings.check_updates);
                    self.config.set_bool("close_to_tray", settings.close_to_tray);
                    self.config.set_string("theme", &settings.theme);
                    let _ = self.config.save();

                    self.windows.remove(&id);
                    return window::close(id);
                }
                Task::none()
            }

            Message::SettingsCancel => {
                if let Some((id, _)) = self.find_settings() {
                    self.windows.remove(&id);
                    return window::close(id);
                }
                Task::none()
            }

            // ---- Global -------------------------------------------------

            Message::Quit => self.shutdown(),
        }
    }

    /// Route view rendering to the correct window.
    fn view(&self, id: window::Id) -> Element<Message> {
        match self.windows.get(&id) {
            Some(WindowKind::Main(state)) => self.view_main(state),
            Some(WindowKind::Settings(state)) => self.view_settings(state, id),
            Some(WindowKind::About) => self.view_about(id),
            None => text("Unknown window").into(),
        }
    }

    /// Per-window theme (all windows share the same theme here).
    fn theme(&self, _id: window::Id) -> Theme {
        Theme::Dark
    }

    /// Subscriptions: listen for window close events.
    fn subscription(&self) -> iced::Subscription<Message> {
        window::close_events().map(Message::WindowClosed)
    }
}

// =============================================================================
// VIEWS
// =============================================================================

impl App {
    fn view_main(&self, state: &MainState) -> Element<Message> {
        let title = text(format!("App Name v{}", env!("CARGO_PKG_VERSION")))
            .size(24);

        let toolbar = row![
            horizontal_space(),
            button("Settings").on_press(Message::OpenSettings),
            button("About").on_press(Message::OpenAbout),
        ]
        .spacing(8);

        let dir_label = text(
            state
                .working_directory
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "No directory selected".into()),
        );

        let body = column![
            row![title, horizontal_space(), toolbar].spacing(16),
            iced::widget::horizontal_rule(1),
            row![dir_label, button("Browse...").on_press(Message::BrowseDirectory)]
                .spacing(8),
            text(&state.status),
        ]
        .spacing(12)
        .padding(20);

        container(body)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }

    fn view_settings(&self, state: &SettingsState, id: window::Id) -> Element<Message> {
        let content = column![
            text("Settings").size(20),
            iced::widget::horizontal_rule(1),
            iced::widget::checkbox("Check for updates on startup", state.check_updates)
                .on_toggle(Message::SettingsCheckUpdatesToggled),
            iced::widget::checkbox("Minimize to tray on close", state.close_to_tray)
                .on_toggle(Message::SettingsCloseToTrayToggled),
            iced::widget::horizontal_rule(1),
            row![
                button("Save").on_press(Message::SettingsSave),
                button("Cancel").on_press(Message::SettingsCancel),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(20);

        container(content)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }

    fn view_about(&self, id: window::Id) -> Element<Message> {
        let content = column![
            text("App Name").size(24),
            text(format!("Version {}", env!("CARGO_PKG_VERSION"))),
            text(env!("CARGO_PKG_DESCRIPTION")),
            iced::widget::vertical_space().height(16),
            button("Close").on_press(Message::CloseWindow(id)),
        ]
        .spacing(8)
        .padding(20)
        .align_x(iced::Alignment::Center);

        container(content)
            .center(iced::Length::Fill)
            .into()
    }
}

// =============================================================================
// HELPERS
// =============================================================================

impl App {
    /// Check if any open window matches a predicate.
    fn has_window_kind(&self, f: impl Fn(&WindowKind) -> bool) -> bool {
        self.windows.values().any(f)
    }

    /// Mutate the settings state (if a settings window is open).
    fn with_settings(&mut self, f: impl FnOnce(&mut SettingsState)) {
        for kind in self.windows.values_mut() {
            if let WindowKind::Settings(state) = kind {
                f(state);
                return;
            }
        }
    }

    /// Find the settings window ID and state.
    fn find_settings(&self) -> Option<(window::Id, &SettingsState)> {
        self.windows.iter().find_map(|(id, kind)| {
            if let WindowKind::Settings(state) = kind {
                Some((*id, state))
            } else {
                None
            }
        })
    }

    /// Orderly shutdown: save state, close all windows, exit.
    fn shutdown(&mut self) -> Task<Message> {
        tracing::info!("Application shutting down");
        let _ = self.session.save();
        let _ = self.config.save();
        iced::exit()
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.13", features = ["multi-window", "tokio"] }
// rfd = "0.15"
// serde = { version = "1", features = ["derive"] }
// serde_json = "1"
// tracing = "0.1"
// tracing-subscriber = { version = "0.3", features = ["env-filter"] }

// =============================================================================
// STUB TYPES (replace with your real modules)
// =============================================================================

// These stubs let the template compile standalone.
// Replace with your actual config/session modules.

#[derive(Default)]
struct ConfigManager;
impl ConfigManager {
    fn load_or_default() -> Self { Self }
    fn get_bool(&self, _key: &str) -> bool { false }
    fn get_string(&self, _key: &str) -> String { "dark".into() }
    fn set_bool(&mut self, _key: &str, _val: bool) {}
    fn set_string(&mut self, _key: &str, _val: &str) {}
    fn save(&self) -> Result<(), std::io::Error> { Ok(()) }
}

#[derive(Default)]
struct SessionManager {
    window_size: Option<Size>,
}
impl SessionManager {
    fn load() -> Self { Self::default() }
    fn save(&self) -> Result<(), std::io::Error> { Ok(()) }
}
