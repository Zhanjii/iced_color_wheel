# System Tray Integration

This document describes how to integrate system tray functionality using the `tray-icon` crate with iced daemon applications.

## Overview

System tray integration allows your app to:
- Minimize to tray instead of closing
- Show a context menu with quick actions
- Display notifications (platform-dependent)
- Run in the background

## Platform Considerations

| Platform | Support | Notes |
|----------|---------|-------|
| Windows  | Full    | Works reliably with `tray-icon` |
| Linux    | Full    | Requires `libayatana-appindicator3-dev` or `libappindicator3-dev` |
| macOS    | Full    | Native support via `tray-icon` |

## TrayManager with `tray-icon` Crate

### TrayManager Implementation

```rust
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder, Icon,
};

/// Whether tray is available on this platform.
fn tray_available() -> bool {
    // tray-icon supports Windows, macOS, and Linux with appindicator
    cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
    ))
}

/// Callbacks set by the application.
#[derive(Default)]
pub struct TrayCallbacks {
    pub on_show: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_quit: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_settings: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_about: Option<Box<dyn Fn() + Send + Sync>>,
}

pub struct TrayManager {
    icon: Option<TrayIcon>,
    running: bool,
    callbacks: Arc<Mutex<TrayCallbacks>>,
    // Menu item IDs for matching events
    show_id: Option<tray_icon::menu::MenuId>,
    settings_id: Option<tray_icon::menu::MenuId>,
    about_id: Option<tray_icon::menu::MenuId>,
    quit_id: Option<tray_icon::menu::MenuId>,
}

static INSTANCE: OnceLock<Mutex<TrayManager>> = OnceLock::new();

impl TrayManager {
    /// Get the singleton instance.
    pub fn instance() -> &'static Mutex<TrayManager> {
        INSTANCE.get_or_init(|| {
            Mutex::new(TrayManager {
                icon: None,
                running: false,
                callbacks: Arc::new(Mutex::new(TrayCallbacks::default())),
                show_id: None,
                settings_id: None,
                about_id: None,
                quit_id: None,
            })
        })
    }

    /// Reset singleton (for testing).
    pub fn reset() {
        if let Some(mgr) = INSTANCE.get() {
            let mut tray = mgr.lock().unwrap();
            tray.stop();
        }
    }

    pub fn is_available(&self) -> bool {
        tray_available()
    }

    /// Start the system tray icon.
    pub fn start(&mut self, icon_path: Option<&Path>) -> bool {
        if !tray_available() || self.running {
            return self.running;
        }

        // Load icon
        let icon = match icon_path {
            Some(path) if path.exists() => load_icon_from_file(path),
            _ => create_default_icon(),
        };

        let icon = match icon {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Failed to load tray icon: {e}");
                return false;
            }
        };

        // Build menu
        let show_item = MenuItem::new("Show", true, None);
        let settings_item = MenuItem::new("Settings", true, None);
        let about_item = MenuItem::new("About", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        self.show_id = Some(show_item.id().clone());
        self.settings_id = Some(settings_item.id().clone());
        self.about_id = Some(about_item.id().clone());
        self.quit_id = Some(quit_item.id().clone());

        let menu = Menu::new();
        let _ = menu.append(&show_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&settings_item);
        let _ = menu.append(&about_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        // Create tray icon
        match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("App Name")
            .with_icon(icon)
            .build()
        {
            Ok(tray_icon) => {
                self.icon = Some(tray_icon);
                self.running = true;
                true
            }
            Err(e) => {
                eprintln!("Failed to start tray: {e}");
                false
            }
        }
    }

    /// Stop the tray icon.
    pub fn stop(&mut self) {
        self.icon.take(); // Drop removes the tray icon
        self.running = false;
    }

    /// Update the tray icon tooltip.
    pub fn update_tooltip(&mut self, text: &str) {
        if let Some(ref icon) = self.icon {
            let _ = icon.set_tooltip(Some(text));
        }
    }

    /// Handle a menu event by dispatching to the appropriate callback.
    pub fn handle_menu_event(&self, event: &MenuEvent) {
        let cbs = self.callbacks.lock().unwrap();
        if Some(&event.id) == self.show_id.as_ref() {
            if let Some(ref cb) = cbs.on_show {
                cb();
            }
        } else if Some(&event.id) == self.settings_id.as_ref() {
            if let Some(ref cb) = cbs.on_settings {
                cb();
            }
        } else if Some(&event.id) == self.about_id.as_ref() {
            if let Some(ref cb) = cbs.on_about {
                cb();
            }
        } else if Some(&event.id) == self.quit_id.as_ref() {
            if let Some(ref cb) = cbs.on_quit {
                cb();
            }
        }
    }

    /// Access the callbacks for setting them from the app.
    pub fn callbacks(&self) -> Arc<Mutex<TrayCallbacks>> {
        Arc::clone(&self.callbacks)
    }
}

fn load_icon_from_file(path: &Path) -> Result<Icon, Box<dyn std::error::Error>> {
    let image = image::open(path)?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}

fn create_default_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    // Create a small 64x64 blue square
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[0] = 52;  // R
        pixel[1] = 152; // G
        pixel[2] = 219; // B
        pixel[3] = 255; // A
    }
    Ok(Icon::from_rgba(rgba, size, size)?)
}
```

### Processing Menu Events

With `tray-icon`, menu events arrive on a channel. Poll them in your event loop:

```rust
use tray_icon::menu::MenuEvent;

// In your main event loop (e.g., winit event loop)
if let Ok(event) = MenuEvent::receiver().try_recv() {
    let tray = TrayManager::instance().lock().unwrap();
    tray.handle_menu_event(&event);
}
```

## Integration with Iced Daemon

The `tray-icon` crate integrates naturally with iced daemon since daemon mode
keeps the app alive even when all windows are closed — exactly what tray-resident
apps need.

### Initialization (Deferred)

Initialize the tray after the main window is created, typically in your `App::new()`:

```rust
use std::path::PathBuf;

fn init_system_tray() {
    let tray = TrayManager::instance();
    let mut mgr = tray.lock().unwrap();

    if !mgr.is_available() {
        return;
    }

    // Set callbacks
    let callbacks = mgr.callbacks();
    {
        let mut cbs = callbacks.lock().unwrap();
        cbs.on_show = Some(Box::new(|| {
            // Signal the main window to show itself
            // In iced daemon, you'd send a message to re-open the main window
        }));
        cbs.on_quit = Some(Box::new(|| {
            std::process::exit(0);
        }));
    }

    // Start tray
    let icon_path = PathBuf::from("assets/icon/app.png");
    mgr.start(Some(&icon_path));
}
```

### Polling Tray Events in Iced

Use an `iced::Subscription` to poll tray menu events each frame:

```rust
use iced::Subscription;
use tray_icon::menu::MenuEvent;

fn subscription(&self) -> Subscription<Message> {
    Subscription::batch([
        // Window close events
        iced::window::close_events().map(Message::WindowClosed),
        // Tray menu events (polled each frame)
        iced::time::every(std::time::Duration::from_millis(100))
            .map(|_| Message::PollTrayEvents),
    ])
}

// In update():
Message::PollTrayEvents => {
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        let tray = TrayManager::instance().lock().unwrap();
        tray.handle_menu_event(&event);
    }
    Task::none()
}
```

### Close-to-Tray Behavior

With iced daemon, close-to-tray is natural — just close the window without
calling `iced::exit()`:

```rust
Message::WindowClosed(id) => {
    self.windows.remove(&id);
    if id == self.main_id {
        let close_to_tray = self.config.get_bool("close_to_tray");
        if close_to_tray {
            // Just close the window; daemon keeps running
            return Task::none();
        }
        // Actually quit
        return self.shutdown();
    }
    Task::none()
}
```

To re-show the main window from the tray "Show" action:

```rust
Message::ShowMainWindow => {
    if !self.windows.contains_key(&self.main_id) {
        // Re-open the main window
        let (id, task) = window::open(window::Settings {
            size: self.session.window_size.unwrap_or(Size::new(1024.0, 768.0)),
            exit_on_close_request: false,
            ..Default::default()
        });
        self.main_id = id;
        self.windows.insert(id, WindowKind::Main(MainState::default()));
        return task.discard();
    }
    Task::none()
}
```

## Shutdown Handling

Ensure tray is stopped on app exit:

```rust
fn shutdown_application() {
    let tray = TrayManager::instance();
    let mut mgr = tray.lock().unwrap();
    mgr.stop();
    // ... other cleanup ...
}
```

## Icon Requirements

- **Windows**: `.ico` format preferred, 16x16 to 256x256 multi-resolution
- **Linux**: `.png` format, 22x22 or 24x24 typical
- **macOS**: `.png` format, template images work best

Recommended: Include both `.ico` and `.png` versions:

```rust
fn get_icon_path() -> PathBuf {
    let base = PathBuf::from("assets/icon");

    if cfg!(target_os = "windows") {
        base.join("app.ico")
    } else {
        base.join("app.png")
    }
}
```

## Dependencies

### For standalone `tray-icon` approach

Add to `Cargo.toml`:

```toml
[dependencies]
tray-icon = "0.19"
image = "0.25"          # For loading icon images

# Linux only
[target.'cfg(target_os = "linux")'.dependencies]
# Requires system package: libayatana-appindicator3-dev
```

### For iced daemon apps

The `tray-icon` crate is the recommended approach. Iced daemon provides the
"keep alive" behavior that system tray apps need.

## Testing Considerations

Reset tray singleton between tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Tray tests are inherently platform-dependent.
    // Mock the TrayManager or skip in CI where no display is available.

    #[test]
    fn test_tray_availability() {
        let tray = TrayManager::instance();
        let mgr = tray.lock().unwrap();
        // On CI without a display, this may be false
        let _available = mgr.is_available();
    }
}
```

## Notifications

For desktop notifications independent of the tray, use the `notify-rust` crate:

```rust
use notify_rust::Notification;

fn show_notification(title: &str, message: &str) {
    let _ = Notification::new()
        .summary(title)
        .body(message)
        .timeout(5000) // milliseconds
        .show();
}
```

Add to `Cargo.toml`:
```toml
[dependencies]
notify-rust = "4"
```

## Related Documentation

- [UI-ARCHITECTURE.md](UI-ARCHITECTURE.md) - Window management
- [CROSS-PLATFORM.md](CROSS-PLATFORM.md) - Platform-specific handling
- [ARCHITECTURE.md](ARCHITECTURE.md) - Project structure
