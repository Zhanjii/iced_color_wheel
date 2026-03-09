# Application Architecture Guide

This document describes the recommended project structure and architecture patterns for building professional Rust desktop applications using Cargo workspaces and iced.

## Project Structure

```
your_app_name/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── SECURITY.md
├── .gitignore
├── rust-toolchain.toml
│
├── src/
│   ├── main.rs                # daemon entry point
│   ├── lib.rs                 # Library root
│   ├── version.rs             # VERSION const + is_development_mode()
│   │
│   ├── ui/                    # User interface layer
│   │   ├── mod.rs
│   │   ├── app.rs             # App struct, update(), view() dispatch
│   │   ├── windows.rs         # WindowKind enum, window registry
│   │   ├── theme.rs           # Theme colors + iced Theme customization
│   │   ├── components/        # Reusable UI components
│   │   │   ├── mod.rs
│   │   │   ├── progress.rs
│   │   │   └── color_picker.rs
│   │   └── popups/            # Popup window views
│   │       ├── mod.rs
│   │       ├── settings.rs
│   │       └── about.rs
│   │
│   ├── core/                  # Business logic
│   │   ├── mod.rs
│   │   └── ...
│   │
│   ├── utils/                 # Utilities and infrastructure
│   │   ├── mod.rs
│   │   ├── config.rs
│   │   ├── config_paths.rs
│   │   ├── session.rs
│   │   ├── logging.rs
│   │   ├── error_reporter.rs
│   │   └── errors.rs
│   │
│   └── keys/
│       ├── secret.key
│       └── *.enc
│
├── assets/
│   └── icon/
│       ├── app.ico
│       └── app.png
│
├── tests/
│   ├── common/
│   │   └── mod.rs
│   ├── test_config.rs
│   └── test_api.rs
│
├── benches/
│   └── benchmarks.rs
│
└── .github/
    └── workflows/
        └── ci.yml
```

### Cargo Workspace (Multi-Crate)

For larger projects, split into a workspace:

```
your_app_name/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── app/                   # Binary crate (UI + main)
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── core/                  # Library crate (business logic)
│   │   ├── Cargo.toml
│   │   └── src/
│   └── utils/                 # Library crate (shared utilities)
│       ├── Cargo.toml
│       └── src/
└── tests/                     # Workspace-level integration tests
```

Workspace `Cargo.toml`:
```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
tracing = "0.1"
```

## Initialization Sequence

The application starts in a specific order to ensure dependencies are available when needed:

```rust
// main.rs - Startup Order

fn main() -> anyhow::Result<()> {
    // 1. FIRST: Install panic hook (before anything can fail)
    install_panic_hook();

    // 2. Initialize core systems
    init_application()?;

    // 3. Check activation and updates
    if !check_app_activation()? {
        return Ok(());
    }
    if check_pending_update()? {
        return Ok(());
    }

    // 4. Launch iced daemon
    launch_ui()
}

fn launch_ui() -> anyhow::Result<()> {
    iced::daemon(App::title, App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .run_with(App::new)?;
    Ok(())
}

fn init_application() -> anyhow::Result<()> {
    // Logging first - everything else may need to log
    init_logging(if is_development_mode() { "debug" } else { "info" });

    // Platform info for diagnostics
    init_platform_info();

    // Configuration system
    init_config()?;

    // Credentials from keyring
    init_credentials()?;

    // Error handling (reporter, collectors)
    init_error_handling()?;

    // Remote config sync (background)
    init_remote_config();

    Ok(())
}
```

## Key Architecture Patterns

### 1. Singleton via OnceLock

Used for managers that need global access with thread safety:

```rust
use std::sync::{Mutex, OnceLock};

pub struct ConfigManager {
    data: std::collections::HashMap<String, String>,
}

static INSTANCE: OnceLock<Mutex<ConfigManager>> = OnceLock::new();

impl ConfigManager {
    pub fn instance() -> &'static Mutex<ConfigManager> {
        INSTANCE.get_or_init(|| {
            Mutex::new(ConfigManager {
                data: std::collections::HashMap::new(),
            })
        })
    }
}
```

### 2. Observer Pattern for Config Changes

```rust
use std::sync::{Arc, Mutex};

type Observer = Box<dyn Fn(&str, &str) + Send + Sync>;

pub struct ConfigManager {
    data: std::collections::HashMap<String, String>,
    observers: Vec<Arc<Observer>>,
}

impl ConfigManager {
    pub fn add_observer(&mut self, callback: impl Fn(&str, &str) + Send + Sync + 'static) {
        self.observers.push(Arc::new(Box::new(callback)));
    }

    fn notify_observers(&self, key: &str, value: &str) {
        for callback in &self.observers {
            callback(key, value);
        }
    }

    pub fn set(&mut self, key: String, value: String) {
        self.notify_observers(&key, &value);
        self.data.insert(key, value);
    }
}
```

### 3. Error Types with thiserror

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Activation failed: {0}")]
    Activation(String),
}
```

### 4. Popup State Structs with view()/update()

Each popup window is a self-contained state struct with its own `view()` and `update()` methods. The main `App` dispatches to these based on the `WindowKind` registry.

```rust
use iced::widget::{button, column, text};
use iced::Element;

/// Each popup owns its state and knows how to render/update itself.
pub struct SettingsPopup {
    theme_choice: String,
    dirty: bool,
}

impl SettingsPopup {
    pub fn new(current_theme: &str) -> Self {
        Self {
            theme_choice: current_theme.to_owned(),
            dirty: false,
        }
    }

    pub fn update(&mut self, message: SettingsMessage) {
        match message {
            SettingsMessage::ThemeChanged(t) => {
                self.theme_choice = t;
                self.dirty = true;
            }
            SettingsMessage::Save => {
                // persist via ConfigManager
                self.dirty = false;
            }
        }
    }

    pub fn view(&self) -> Element<'_, SettingsMessage> {
        column![
            text(format!("Theme: {}", self.theme_choice)),
            button("Save").on_press(SettingsMessage::Save),
        ]
        .into()
    }
}
```

## Window Lifecycle (iced daemon)

### Creation

Open new windows via `window::open` in the `update()` method. The daemon keeps running as long as at least one window exists (or until an explicit exit).

```rust
use iced::window;

fn update(&mut self, message: Message) -> iced::Task<Message> {
    match message {
        Message::OpenSettings => {
            let settings = window::Settings {
                size: iced::Size::new(500.0, 400.0),
                position: window::Position::Centered,
                ..Default::default()
            };
            let (id, task) = window::open(settings);
            self.windows.insert(id, WindowKind::Settings(SettingsPopup::new("dark")));
            task.map(|_| Message::Noop)
        }
        // ...
    }
}
```

### Close Handling

Subscribe to `window::close_events()` so the app can save state and clean up resources when a window is closed.

```rust
fn subscription(&self) -> iced::Subscription<Message> {
    window::close_events().map(Message::WindowClosed)
}

fn update(&mut self, message: Message) -> iced::Task<Message> {
    match message {
        Message::WindowClosed(id) => {
            if let Some(kind) = self.windows.remove(&id) {
                tracing::info!("Window closed: {kind:?}");
            }

            // If no windows remain, shut down
            if self.windows.is_empty() {
                return self.shutdown();
            }
            iced::Task::none()
        }
        // ...
    }
}
```

### Shutdown

Use `iced::exit()` to terminate the daemon after running cleanup logic.

```rust
impl App {
    fn shutdown(&mut self) -> iced::Task<Message> {
        self.session.save_window_state();
        tracing::info!("Application closing");
        iced::exit()
    }
}
```

## Shutdown Sequence

Orderly shutdown with timeouts to prevent hanging:

```rust
fn shutdown_application() {
    let services: &[(&str, fn())] = &[
        ("tray", stop_tray),
        ("remote_sync", stop_remote_sync),
        ("monitoring", stop_monitoring),
    ];

    for (name, stop_func) in services {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(stop_func));
        if let Err(e) = result {
            tracing::warn!("Error stopping {name}: {e:?}");
        }
    }
}
```

## Directory Conventions

| Directory | Purpose |
|-----------|---------|
| `src/` | All application source code |
| `src/ui/` | iced views, popup windows, components |
| `src/core/` | Business logic (no UI dependencies) |
| `src/utils/` | Infrastructure utilities |
| `tests/` | Integration tests |
| `benches/` | Performance benchmarks |
| `assets/` | Static files (icons, images) |
| `keys/` | Bundled credentials (if using remote PIN security) |

## Related Documentation

- [VERSIONING.md](VERSIONING.md) - Version management and dev mode
- [UI-ARCHITECTURE.md](UI-ARCHITECTURE.md) - Window and dialog patterns
- [CONFIG-SYSTEM.md](CONFIG-SYSTEM.md) - Configuration management
- [QUICK-START.md](QUICK-START.md) - Step-by-step new app guide
