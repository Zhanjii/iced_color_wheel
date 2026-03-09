//! about_dialog.rs
//!
//! About dialog as an iced daemon popup window.
//!
//! Features:
//! - App version from Cargo.toml
//! - System information for debugging
//! - Support links and metadata
//! - Popup window via `window::open()` with Close button
//!
//! # Example
//!
//! ```rust
//! use about_dialog::{AboutInfo, view_about, Message};
//! use iced::window;
//!
//! // In your daemon's update():
//! Message::ShowAbout => {
//!     let (id, open) = window::open(window::Settings {
//!         size: iced::Size::new(420.0, 520.0),
//!         resizable: false,
//!         ..Default::default()
//!     });
//!     self.windows.insert(id, WindowKind::About);
//!     return open.map(|_| Message::Noop);
//! }
//!
//! // In your daemon's view():
//! WindowKind::About => view_about(id).map(Message::from),
//! ```

use iced::widget::{button, column, container, horizontal_rule, row, text, Column};
use iced::window;
use iced::{Alignment, Element, Length, Padding};
use serde::{Deserialize, Serialize};

// =============================================================================
// CONFIGURATION - Update these for your project
// =============================================================================

/// Application name
const APP_NAME: &str = env!("CARGO_PKG_NAME");

/// Application description
const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

/// Copyright text
const COPYRIGHT_TEXT: &str = "2025 Your Company. All rights reserved.";

/// Developer credits
const DEVELOPER_CREDITS: &str = "Developed by Your Name";

/// Support email
const SUPPORT_EMAIL: &str = "support@yourcompany.com";

/// Company website
const COMPANY_WEBSITE: &str = "https://yourcompany.com";

/// Documentation URL
const DOCS_URL: &str = "https://docs.yourcompany.com/";

/// Changelog URL
const CHANGELOG_URL: &str = "https://docs.yourcompany.com/CHANGELOG";

/// GitHub repository URL
const GITHUB_REPO_URL: &str = "https://github.com/yourcompany/your-app";

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Application information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AboutInfo {
    /// Application name
    pub name: String,
    /// Application version
    pub version: String,
    /// Application description
    pub description: String,
    /// Copyright text
    pub copyright: String,
    /// Developer credits
    pub credits: String,
    /// Support email
    pub email: String,
    /// Company website
    pub website: String,
    /// Documentation URL
    pub docs_url: String,
    /// Changelog URL
    pub changelog_url: String,
    /// GitHub repository URL
    pub github_url: String,
}

/// System information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Application version
    pub app_version: String,
    /// Rust version used to compile
    pub rust_version: String,
    /// Operating system
    pub os: String,
    /// OS version
    pub os_version: String,
    /// CPU architecture
    pub arch: String,
    /// Formatted display string
    pub display: String,
}

// =============================================================================
// MESSAGES
// =============================================================================

/// Messages emitted by the about dialog
#[derive(Debug, Clone)]
pub enum AboutMessage {
    /// Close the about popup window
    CloseWindow(window::Id),
    /// Open a URL in the default browser
    OpenUrl(String),
    /// Copy system info text to clipboard
    CopySystemInfo,
}

// =============================================================================
// DATA CONSTRUCTORS
// =============================================================================

/// Build the about info from compile-time constants
pub fn get_about_info() -> AboutInfo {
    AboutInfo {
        name: APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: APP_DESCRIPTION.to_string(),
        copyright: COPYRIGHT_TEXT.to_string(),
        credits: DEVELOPER_CREDITS.to_string(),
        email: SUPPORT_EMAIL.to_string(),
        website: COMPANY_WEBSITE.to_string(),
        docs_url: DOCS_URL.to_string(),
        changelog_url: CHANGELOG_URL.to_string(),
        github_url: GITHUB_REPO_URL.to_string(),
    }
}

/// Gather system information for debugging
pub fn get_system_info() -> SystemInfo {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let os_version = get_os_version();

    let display = format!(
        "App Version: {}\nRust: {}\nPlatform: {} {}\nArch: {}",
        env!("CARGO_PKG_VERSION"),
        rustc_version_runtime::version(),
        os,
        os_version,
        arch
    );

    SystemInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        rust_version: rustc_version_runtime::version().to_string(),
        os: os.to_string(),
        os_version,
        arch: arch.to_string(),
        display,
    }
}

// =============================================================================
// VIEW
// =============================================================================

/// Render the about dialog as an iced popup window view.
///
/// # Arguments
///
/// * `id` - The `window::Id` of this popup, used by the Close button.
///
/// # Returns
///
/// An `Element<AboutMessage>` suitable for returning from your daemon's `view()`.
pub fn view_about(id: window::Id) -> Element<'static, AboutMessage> {
    let info = get_about_info();
    let sys = get_system_info();

    let header = column![
        text(&info.name).size(24),
        text(format!("Version {}", info.version)).size(14),
        text(&info.description).size(14),
    ]
    .spacing(4)
    .align_x(Alignment::Center);

    let credits = column![
        text(&info.copyright).size(12),
        text(&info.credits).size(12),
    ]
    .spacing(2)
    .align_x(Alignment::Center);

    let links = column![
        link_row("Website", &info.website),
        link_row("Documentation", &info.docs_url),
        link_row("Changelog", &info.changelog_url),
        link_row("Source Code", &info.github_url),
        link_row("Support", &format!("mailto:{}", info.email)),
    ]
    .spacing(4);

    let system_section = column![
        text("System Information").size(14),
        text(&sys.display).size(12),
        button(text("Copy System Info").size(12))
            .on_press(AboutMessage::CopySystemInfo)
            .padding(Padding::from([4, 8])),
    ]
    .spacing(4);

    let close_btn = button(text("Close").size(14))
        .on_press(AboutMessage::CloseWindow(id))
        .padding(Padding::from([6, 20]));

    let content: Column<'_, AboutMessage> = column![
        header,
        horizontal_rule(1),
        credits,
        horizontal_rule(1),
        links,
        horizontal_rule(1),
        system_section,
        close_btn,
    ]
    .spacing(12)
    .padding(20)
    .align_x(Alignment::Center)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// Helper: render one link row with a label and an "Open" button
fn link_row(label: &str, url: &str) -> Element<'static, AboutMessage> {
    let url_owned = url.to_owned();
    row![
        text(format!("{}:", label)).size(12).width(Length::Fixed(110.0)),
        button(text("Open").size(11))
            .on_press(AboutMessage::OpenUrl(url_owned))
            .padding(Padding::from([2, 8])),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

// =============================================================================
// UPDATE HELPER
// =============================================================================

/// Handle an `AboutMessage` in your daemon's `update()`.
///
/// Returns an `iced::Task` -- the caller should map it into their top-level
/// `Message` type.
///
/// # Example
///
/// ```rust
/// Message::About(msg) => {
///     return handle_about_message(msg).map(Message::from);
/// }
/// ```
pub fn handle_about_message(msg: AboutMessage) -> iced::Task<AboutMessage> {
    match msg {
        AboutMessage::CloseWindow(id) => window::close(id).map(|_| AboutMessage::CopySystemInfo),
        AboutMessage::OpenUrl(url) => {
            let _ = open::that(&url);
            iced::Task::none()
        }
        AboutMessage::CopySystemInfo => {
            let sys = get_system_info();
            copy_to_clipboard(sys.display);
            iced::Task::none()
        }
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Copy text to the system clipboard
fn copy_to_clipboard(text: String) {
    use clipboard::{ClipboardContext, ClipboardProvider};

    if let Ok(mut ctx) = ClipboardContext::new() {
        let _ = ctx.set_contents(text);
    }
}

/// Get OS version string
fn get_os_version() -> String {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(current_version) =
            hklm.open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
        {
            if let Ok(version) = current_version.get_value::<String, _>("DisplayVersion") {
                return version;
            }
        }
        "Unknown".to_string()
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        if let Ok(output) = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            if let Ok(version) = String::from_utf8(output.stdout) {
                return version.trim().to_string();
            }
        }
        "Unknown".to_string()
    }

    #[cfg(target_os = "linux")]
    {
        use std::fs;

        if let Ok(contents) = fs::read_to_string("/etc/os-release") {
            for line in contents.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    return line
                        .trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string();
                }
            }
        }
        "Unknown".to_string()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "Unknown".to_string()
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window"] }
// serde = { version = "1.0", features = ["derive"] }
// open = "5.0"
// clipboard = "0.5"
// rustc_version_runtime = "0.3"
//
// [target.'cfg(windows)'.dependencies]
// winreg = "0.52"

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_about_info() {
        let info = get_about_info();
        assert!(!info.name.is_empty());
        assert!(!info.version.is_empty());
        assert_eq!(info.name, APP_NAME);
    }

    #[test]
    fn test_get_system_info() {
        let info = get_system_info();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(!info.display.is_empty());
    }

    #[test]
    fn test_get_os_version() {
        let version = get_os_version();
        assert!(!version.is_empty());
    }
}
