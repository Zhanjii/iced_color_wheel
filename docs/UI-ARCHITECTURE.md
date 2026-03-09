# UI Architecture: Iced Daemon, Multi-Window, and Panels

This document describes the UI architecture patterns for Rust desktop applications using the iced daemon model -- a single Rust binary that manages multiple windows through a centralized update/view loop.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Iced Daemon Model                                              │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Single Rust Binary                                       │  │
│  │                                                           │  │
│  │  iced::daemon() entry point                               │  │
│  │  - App::update() handles all messages centrally           │  │
│  │  - App::view(id) dispatches rendering per window          │  │
│  │  - App::subscription() manages background events          │  │
│  │                                                           │  │
│  │  Multi-window via window::open() / window::close()        │  │
│  │  HashMap<window::Id, WindowKind> tracks all windows       │  │
│  │  View dispatch by window ID                               │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  Features: multi-window, canvas, tokio                          │
│  iced = "0.14"                                                  │
│  iced_color_wheel = "0.1"                                       │
└─────────────────────────────────────────────────────────────────┘
```

The daemon model differs from a standard iced application in one critical way: the process does **not** exit when the last window closes. This makes it suitable for tray-resident applications that persist in the background. All windows set `exit_on_close_request: false` so the application handles close events itself.

---

## Daemon Entry Point

The daemon is launched with a builder chain that wires up all four pillars of the Elm architecture -- `update`, `view`, `theme`, and `subscription` -- plus an initialization function:

```rust
// src/main.rs

use iced;

fn main() -> iced::Result {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    iced::daemon("App Name", App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .run_with(App::new)
}
```

`run_with` calls `App::new` once at startup. The returned `(App, Task<Message>)` tuple initializes state and can fire an initial task (such as opening the main window).

---

## Window Lifecycle

### Window Registry

The application tracks every open window in a `HashMap`. Each entry maps a `window::Id` to a `WindowKind` enum describing the window's role:

```rust
use std::collections::HashMap;
use iced::window;

#[derive(Debug, Clone, PartialEq)]
pub enum WindowKind {
    Main,
    Settings,
    ColorPicker,
    About,
}

pub struct App {
    windows: HashMap<window::Id, WindowKind>,
    main_id: window::Id,
    config: ConfigManager,
    session: SessionManager,
    // Panel state structs...
    settings_panel: SettingsPanel,
    color_picker_panel: ColorPickerPanel,
}
```

### Opening Windows

Use `window::open()` to create new windows. The function returns a `(window::Id, Task<Message>)` pair. Register the ID immediately in the window map:

```rust
use iced::window;

impl App {
    fn open_window(&mut self, kind: WindowKind, settings: window::Settings) -> Task<Message> {
        let (id, task) = window::open(settings);
        self.windows.insert(id, kind);
        task.discard()
    }
}
```

To open specific window types from `update`:

```rust
Message::OpenSettings => {
    if self.has_window_kind(&WindowKind::Settings) {
        return Task::none(); // Prevent duplicate
    }
    self.open_window(
        WindowKind::Settings,
        WindowSettings::popup(400.0, 500.0, "Settings"),
    )
}

Message::OpenColorPicker => {
    if self.has_window_kind(&WindowKind::ColorPicker) {
        return Task::none();
    }
    self.open_window(
        WindowKind::ColorPicker,
        WindowSettings::popup(350.0, 400.0, "Color Picker"),
    )
}
```

### Preventing Duplicate Popups

A helper method checks whether a window of a given kind is already open:

```rust
impl App {
    fn has_window_kind(&self, kind: &WindowKind) -> bool {
        self.windows.values().any(|k| k == kind)
    }
}
```

### Closing Windows

Close windows by dispatching `window::close(id)`. Remove the entry from the registry when the close event arrives:

```rust
Message::CloseWindow(id) => {
    self.windows.remove(&id);
    window::close(id)
}

Message::WindowClosed(id) => {
    self.windows.remove(&id);
    Task::none()
}
```

### Handling Close Events

Subscribe to `window::close_events()` so the application can clean up state and remove entries from the registry when a window is closed externally (e.g., the user clicks the OS close button):

```rust
impl App {
    fn subscription(&self) -> iced::Subscription<Message> {
        window::close_events().map(Message::WindowClosed)
    }
}
```

Because every window is created with `exit_on_close_request: false`, closing a window never terminates the process. The `App` must explicitly handle the close event and decide whether to shut down.

---

## Main Window Setup

The `new()` function is called once by `run_with`. It opens the main window and returns the initial application state along with the task that creates the window:

```rust
use iced::{Task, window};

impl App {
    fn new() -> (Self, Task<Message>) {
        let session = SessionManager::load();

        let (main_id, open_task) = window::open(WindowSettings::main(
            1024.0,
            768.0,
            "App Name",
        ));

        let mut windows = HashMap::new();
        windows.insert(main_id, WindowKind::Main);

        let app = Self {
            windows,
            main_id,
            config: ConfigManager::load_or_default(),
            session,
            settings_panel: SettingsPanel::new(),
            color_picker_panel: ColorPickerPanel::new(),
        };

        (app, open_task.discard())
    }
}
```

---

## View Dispatch

The `view` function receives a `window::Id` and must return the correct UI for that window. Look up the window kind in the registry and dispatch accordingly:

```rust
use iced::Element;

impl App {
    fn view(&self, id: window::Id) -> Element<Message> {
        match self.windows.get(&id) {
            Some(WindowKind::Main) => self.view_main(),
            Some(WindowKind::Settings) => self.settings_panel.view(),
            Some(WindowKind::ColorPicker) => self.color_picker_panel.view(),
            Some(WindowKind::About) => self.view_about(),
            None => iced::widget::text("Unknown window").into(),
        }
    }
}
```

Each branch can delegate to a method on `App` or to a `view()` method on a panel struct. The key constraint is that `view` is called with `&self` (immutable) -- all mutation happens in `update`.

---

## Panel Pattern

Each popup or panel is its own state struct with `view()` and `update()` methods. The parent `App` owns the panel struct and forwards messages to it.

### Settings Popup Example

```rust
// src/ui/panels/settings.rs

use iced::widget::{button, checkbox, column, row, text, text_input, scrollable};
use iced::Element;

pub struct SettingsPanel {
    auto_save: bool,
    confirm_on_exit: bool,
    recent_files_limit: String,
    dirty: bool,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    AutoSaveToggled(bool),
    ConfirmOnExitToggled(bool),
    RecentFilesLimitChanged(String),
    Save,
    Cancel,
}

impl SettingsPanel {
    pub fn new() -> Self {
        Self {
            auto_save: false,
            confirm_on_exit: true,
            recent_files_limit: "10".to_string(),
            dirty: false,
        }
    }

    pub fn update(&mut self, message: SettingsMessage, config: &mut ConfigManager) {
        match message {
            SettingsMessage::AutoSaveToggled(val) => {
                self.auto_save = val;
                self.dirty = true;
            }
            SettingsMessage::ConfirmOnExitToggled(val) => {
                self.confirm_on_exit = val;
                self.dirty = true;
            }
            SettingsMessage::RecentFilesLimitChanged(val) => {
                self.recent_files_limit = val;
                self.dirty = true;
            }
            SettingsMessage::Save => {
                self.save_to(config);
                self.dirty = false;
            }
            SettingsMessage::Cancel => {
                self.load_from(config);
                self.dirty = false;
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let content = column![
            text("Settings").size(24),
            checkbox("Auto-save", self.auto_save)
                .on_toggle(|v| Message::Settings(SettingsMessage::AutoSaveToggled(v))),
            checkbox("Confirm on exit", self.confirm_on_exit)
                .on_toggle(|v| Message::Settings(SettingsMessage::ConfirmOnExitToggled(v))),
            row![
                text("Recent files limit:"),
                text_input("10", &self.recent_files_limit)
                    .on_input(|v| Message::Settings(SettingsMessage::RecentFilesLimitChanged(v))),
            ].spacing(10),
            row![
                button("Save").on_press_maybe(
                    self.dirty.then_some(Message::Settings(SettingsMessage::Save))
                ),
                button("Cancel").on_press(Message::Settings(SettingsMessage::Cancel)),
            ].spacing(10),
        ]
        .spacing(12)
        .padding(20);

        scrollable(content).into()
    }

    fn save_to(&self, config: &mut ConfigManager) {
        config.set("general.auto_save", self.auto_save.into());
        config.set("general.confirm_on_exit", self.confirm_on_exit.into());
        if let Ok(limit) = self.recent_files_limit.parse::<u32>() {
            config.set("general.recent_files_limit", limit.into());
        }
        let _ = config.persist();
    }

    fn load_from(&mut self, config: &ConfigManager) {
        self.auto_save = config.get_bool("general.auto_save").unwrap_or(false);
        self.confirm_on_exit = config.get_bool("general.confirm_on_exit").unwrap_or(true);
        self.recent_files_limit = config
            .get_u32("general.recent_files_limit")
            .unwrap_or(10)
            .to_string();
    }
}
```

The parent `App::update` forwards messages:

```rust
Message::Settings(msg) => {
    self.settings_panel.update(msg, &mut self.config);
    Task::none()
}
```

---

## Window Settings Helpers

Centralize window configuration in a `base_frame.rs` template. Each helper returns a `window::Settings` with sensible defaults. All helpers set `exit_on_close_request: false` so the daemon manages the lifecycle.

```rust
// src/ui/base_frame.rs

use iced::window;

pub struct WindowSettings;

impl WindowSettings {
    /// Main application window -- resizable, centered, with minimum size.
    pub fn main(width: f32, height: f32, title: &str) -> window::Settings {
        window::Settings {
            size: iced::Size::new(width, height),
            min_size: Some(iced::Size::new(800.0, 600.0)),
            position: window::Position::Centered,
            exit_on_close_request: false,
            ..window::Settings::default()
        }
    }

    /// Popup window -- fixed size, not resizable, centered.
    pub fn popup(width: f32, height: f32, title: &str) -> window::Settings {
        window::Settings {
            size: iced::Size::new(width, height),
            resizable: false,
            position: window::Position::Centered,
            exit_on_close_request: false,
            ..window::Settings::default()
        }
    }

    /// Dialog window -- small, not resizable, centered.
    pub fn dialog(width: f32, height: f32, title: &str) -> window::Settings {
        window::Settings {
            size: iced::Size::new(width, height),
            resizable: false,
            decorations: true,
            position: window::Position::Centered,
            exit_on_close_request: false,
            ..window::Settings::default()
        }
    }
}
```

Reference `base_frame.rs` whenever opening a new window to keep sizing and behavior consistent across the application.

---

## UI Lock Pattern

Disable buttons during long-running operations. In iced, this is achieved with `on_press_maybe` -- passing `None` disables the button, passing `Some(msg)` enables it:

```rust
pub struct ProcessorState {
    processing: bool,
    progress: f32,
    status: String,
}

impl ProcessorState {
    pub fn is_locked(&self) -> bool {
        self.processing
    }

    pub fn lock(&mut self, status: &str) {
        self.processing = true;
        self.progress = 0.0;
        self.status = status.to_string();
    }

    pub fn unlock(&mut self) {
        self.processing = false;
        self.status.clear();
    }
}
```

Usage in a view function:

```rust
use iced::widget::{button, column, progress_bar, text};

fn view_processor(&self) -> Element<Message> {
    let start_button = button(
        if self.processor.is_locked() { "Processing..." } else { "Start Processing" }
    )
    .on_press_maybe(
        (!self.processor.is_locked()).then_some(Message::StartProcessing)
    );

    let mut content = column![start_button].spacing(10);

    if self.processor.is_locked() {
        content = content
            .push(progress_bar(0.0..=1.0, self.processor.progress))
            .push(text(&self.processor.status))
            .push(button("Cancel").on_press(Message::CancelProcessing));
    }

    content.into()
}
```

---

## Progress with Background Tasks

Use `Task::perform()` for async work that produces a single result. Use `iced::Subscription` for ongoing progress streaming.

### Single-Result Async Task

```rust
Message::StartProcessing => {
    self.processor.lock("Starting...");

    let directory = self.working_directory.clone();
    Task::perform(
        async move {
            process_files(&directory).await
        },
        Message::ProcessingComplete,
    )
}

Message::ProcessingComplete(result) => {
    self.processor.unlock();
    match result {
        Ok(summary) => self.result = Some(summary),
        Err(err) => self.result = Some(format!("Error: {err}")),
    }
    Task::none()
}
```

### Streaming Progress with Subscription

For tasks that report incremental progress, use a subscription that yields messages over time:

```rust
use iced::Subscription;

impl App {
    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            window::close_events().map(Message::WindowClosed),
        ];

        if self.processor.is_locked() {
            subscriptions.push(
                self.progress_subscription()
            );
        }

        Subscription::batch(subscriptions)
    }

    fn progress_subscription(&self) -> Subscription<Message> {
        iced::subscription::channel("file-processing", 100, |mut sender| async move {
            // This runs in a tokio task.
            // Send progress updates through the channel.
            for i in 0..100 {
                let _ = sender
                    .send(Message::ProgressUpdate {
                        current: i + 1,
                        total: 100,
                        status: format!("Processing file {}/100", i + 1),
                    })
                    .await;
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            let _ = sender.send(Message::ProcessingComplete(Ok("Done".to_string()))).await;

            // Keep the future alive (subscription ends when dropped)
            std::future::pending().await
        })
    }
}
```

Handle progress messages in `update`:

```rust
Message::ProgressUpdate { current, total, status } => {
    self.processor.progress = current as f32 / total as f32;
    self.processor.status = status;
    Task::none()
}
```

---

## Dialog Helpers

Use the `rfd` crate for native file and message dialogs. These are framework-agnostic and work with any Rust GUI toolkit:

```rust
// src/ui/dialogs.rs

use rfd::{FileDialog, MessageDialog, MessageLevel, MessageButtons};

/// Show an error dialog.
pub fn show_error(title: &str, message: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(MessageLevel::Error)
        .show();
}

/// Show a warning dialog.
pub fn show_warning(title: &str, message: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(MessageLevel::Warning)
        .show();
}

/// Show an info dialog.
pub fn show_info(title: &str, message: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(MessageLevel::Info)
        .show();
}

/// Show a confirmation dialog. Returns `true` if confirmed.
pub fn show_confirm(title: &str, message: &str) -> bool {
    MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(MessageLevel::Info)
        .set_buttons(MessageButtons::YesNo)
        .show()
}

/// Show a directory picker. Returns the selected path or `None`.
pub fn pick_directory(title: &str) -> Option<std::path::PathBuf> {
    FileDialog::new()
        .set_title(title)
        .pick_folder()
}

/// Show a file picker with optional filters.
pub fn pick_file(title: &str, extensions: &[&str]) -> Option<std::path::PathBuf> {
    let mut dialog = FileDialog::new().set_title(title);
    if !extensions.is_empty() {
        dialog = dialog.add_filter("Files", extensions);
    }
    dialog.pick_file()
}

/// Show a save-file dialog.
pub fn save_file(title: &str, default_name: &str, extensions: &[&str]) -> Option<std::path::PathBuf> {
    let mut dialog = FileDialog::new()
        .set_title(title)
        .set_file_name(default_name);
    if !extensions.is_empty() {
        dialog = dialog.add_filter("Files", extensions);
    }
    dialog.save_file()
}
```

For async contexts (e.g., inside `Task::perform`), use `rfd::AsyncFileDialog` and `rfd::AsyncMessageDialog` instead.

---

## Color Picker Popup

Open a color picker as a daemon popup window using the `iced_color_wheel` crate (version 0.1). Reference the `color_picker.rs` template for the full implementation.

### Panel State

```rust
// src/ui/panels/color_picker.rs

use iced::widget::{button, column, row, text};
use iced::{Color, Element};
use iced_color_wheel::ColorWheel;

pub struct ColorPickerPanel {
    current_color: Color,
    original_color: Color,
}

#[derive(Debug, Clone)]
pub enum ColorPickerMessage {
    ColorChanged(Color),
    Confirm,
    Cancel,
}

impl ColorPickerPanel {
    pub fn new() -> Self {
        Self {
            current_color: Color::WHITE,
            original_color: Color::WHITE,
        }
    }

    pub fn open_with(&mut self, color: Color) {
        self.current_color = color;
        self.original_color = color;
    }

    pub fn update(&mut self, message: ColorPickerMessage) -> Option<ColorPickerResult> {
        match message {
            ColorPickerMessage::ColorChanged(color) => {
                self.current_color = color;
                None
            }
            ColorPickerMessage::Confirm => {
                Some(ColorPickerResult::Confirmed(self.current_color))
            }
            ColorPickerMessage::Cancel => {
                Some(ColorPickerResult::Cancelled)
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let wheel = ColorWheel::new(
            self.current_color,
            |color| Message::ColorPicker(ColorPickerMessage::ColorChanged(color)),
        );

        column![
            text("Pick a Color").size(20),
            wheel,
            row![
                button("OK").on_press(
                    Message::ColorPicker(ColorPickerMessage::Confirm)
                ),
                button("Cancel").on_press(
                    Message::ColorPicker(ColorPickerMessage::Cancel)
                ),
            ].spacing(10),
        ]
        .spacing(12)
        .padding(20)
        .into()
    }
}

pub enum ColorPickerResult {
    Confirmed(Color),
    Cancelled,
}
```

### Wiring in App::update

```rust
Message::ColorPicker(msg) => {
    if let Some(result) = self.color_picker_panel.update(msg) {
        // Close the color picker window
        let picker_id = self.window_id_for_kind(&WindowKind::ColorPicker);
        let close_task = if let Some(id) = picker_id {
            self.windows.remove(&id);
            window::close(id)
        } else {
            Task::none()
        };

        match result {
            ColorPickerResult::Confirmed(color) => {
                self.apply_selected_color(color);
            }
            ColorPickerResult::Cancelled => {}
        }

        close_task
    } else {
        Task::none()
    }
}
```

The helper `window_id_for_kind` finds the ID for a given window kind:

```rust
impl App {
    fn window_id_for_kind(&self, kind: &WindowKind) -> Option<window::Id> {
        self.windows.iter()
            .find(|(_, k)| *k == kind)
            .map(|(id, _)| *id)
    }
}
```

---

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
- [UI-COMPONENTS.md](UI-COMPONENTS.md) - Reusable widgets and theming
- [SYSTEM-TRAY.md](SYSTEM-TRAY.md) - System tray integration
- [CROSS-PLATFORM.md](CROSS-PLATFORM.md) - Platform-specific UI handling
