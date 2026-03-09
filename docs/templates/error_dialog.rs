//! error_dialog.rs
//!
//! Error handling and popup dialog for iced daemon applications.
//!
//! Features:
//! - User-friendly error messages
//! - Error popup window via `window::open()`
//! - Stack trace capture
//! - Copy-to-clipboard and Close buttons
//! - Error persistence to log files
//!
//! # Example
//!
//! ```rust
//! use error_dialog::{ErrorReport, ErrorDialogState, view_error, ErrorMessage};
//! use iced::window;
//!
//! // Create an error report
//! let report = ErrorReport::new(
//!     "FileNotFound",
//!     "Configuration file not found",
//!     Some("config.json".to_string()),
//! );
//!
//! // In your daemon's update(), open the error popup:
//! let (id, open) = window::open(window::Settings {
//!     size: iced::Size::new(480.0, 400.0),
//!     resizable: false,
//!     ..Default::default()
//! });
//! self.error_windows.insert(id, report);
//! return open.map(|_| Message::Noop);
//!
//! // In your daemon's view():
//! if let Some(report) = self.error_windows.get(&id) {
//!     return view_error(id, report).map(Message::Error);
//! }
//! ```

use iced::widget::{button, column, container, horizontal_rule, row, scrollable, text};
use iced::window;
use iced::{Alignment, Element, Length, Padding};
use serde::{Deserialize, Serialize};
use std::backtrace::Backtrace;

// =============================================================================
// ERROR STRUCTURES
// =============================================================================

/// Error report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    /// Error type/category
    pub error_type: String,
    /// User-friendly error message
    pub message: String,
    /// Technical details
    pub details: Option<String>,
    /// Stack trace
    pub stack_trace: Option<String>,
    /// Timestamp
    pub timestamp: String,
    /// Application version
    pub app_version: String,
}

impl ErrorReport {
    /// Create a new error report
    pub fn new(
        error_type: impl Into<String>,
        message: impl Into<String>,
        details: Option<String>,
    ) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            details,
            stack_trace: None,
            timestamp: chrono::Local::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create error report with backtrace
    pub fn with_backtrace(
        error_type: impl Into<String>,
        message: impl Into<String>,
        details: Option<String>,
    ) -> Self {
        let backtrace = Backtrace::capture();
        let stack_trace = if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
            Some(format!("{:?}", backtrace))
        } else {
            None
        };

        Self {
            error_type: error_type.into(),
            message: message.into(),
            details,
            stack_trace,
            timestamp: chrono::Local::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create from a std::error::Error
    pub fn from_error<E: std::error::Error>(error: &E, context: Option<String>) -> Self {
        Self::with_backtrace(
            std::any::type_name::<E>(),
            error.to_string(),
            context,
        )
    }

    /// Format the full error details as a copyable string
    pub fn to_clipboard_text(&self) -> String {
        format!(
            "Error: {}\nMessage: {}\nDetails: {}\nTimestamp: {}\nVersion: {}\n\nStack Trace:\n{}",
            self.error_type,
            self.message,
            self.details.as_deref().unwrap_or("None"),
            self.timestamp,
            self.app_version,
            self.stack_trace.as_deref().unwrap_or("Not available"),
        )
    }
}

/// Friendly error messages for common error types
pub fn get_friendly_message(error_type: &str) -> &'static str {
    match error_type {
        "FileNotFound" | "std::io::Error" => "A required file could not be found.",
        "PermissionDenied" => "Permission denied while accessing a file or resource.",
        "ConnectionError" => "Could not connect to the network or server.",
        "Timeout" => "The operation timed out. Please try again.",
        "ValueError" => "An invalid value was encountered.",
        "ParseError" => "Failed to parse data. The format may be corrupted.",
        "SerdeError" => "Failed to serialize or deserialize data.",
        _ => "An unexpected error occurred.",
    }
}

// =============================================================================
// MESSAGES
// =============================================================================

/// Messages emitted by the error dialog
#[derive(Debug, Clone)]
pub enum ErrorMessage {
    /// Close the error popup window
    CloseWindow(window::Id),
    /// Copy the full error details to the clipboard
    CopyToClipboard(String),
}

// =============================================================================
// VIEW
// =============================================================================

/// Render the error dialog popup view.
///
/// # Arguments
///
/// * `id` - The `window::Id` for this error popup
/// * `report` - The `ErrorReport` to display
///
/// # Returns
///
/// An `Element<ErrorMessage>` suitable for returning from your daemon's `view()`.
pub fn view_error<'a>(id: window::Id, report: &ErrorReport) -> Element<'a, ErrorMessage> {
    let friendly = get_friendly_message(&report.error_type);

    let header = column![
        text("Error").size(22),
        text(friendly).size(14),
    ]
    .spacing(4);

    let details_section = {
        let mut col = column![
            text(format!("Type: {}", report.error_type)).size(12),
            text(&report.message).size(13),
        ]
        .spacing(4);

        if let Some(ref details) = report.details {
            col = col.push(text(format!("Details: {}", details)).size(12));
        }

        if let Some(ref trace) = report.stack_trace {
            col = col.push(text("Stack Trace:").size(12));
            col = col.push(
                scrollable(
                    text(trace).size(10),
                )
                .height(Length::Fixed(120.0)),
            );
        }

        col = col.push(text(format!("Timestamp: {}", report.timestamp)).size(11));
        col = col.push(text(format!("Version: {}", report.app_version)).size(11));

        col
    };

    let clipboard_text = report.to_clipboard_text();

    let buttons = row![
        button(text("Copy to Clipboard").size(13))
            .on_press(ErrorMessage::CopyToClipboard(clipboard_text))
            .padding(Padding::from([6, 14])),
        button(text("Close").size(13))
            .on_press(ErrorMessage::CloseWindow(id))
            .padding(Padding::from([6, 14])),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let content = column![
        header,
        horizontal_rule(1),
        details_section,
        horizontal_rule(1),
        buttons,
    ]
    .spacing(12)
    .padding(20)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

// =============================================================================
// UPDATE HELPER
// =============================================================================

/// Handle an `ErrorMessage` in your daemon's `update()`.
///
/// Returns an `iced::Task` the caller should map into their top-level `Message`.
pub fn handle_error_message(msg: ErrorMessage) -> iced::Task<ErrorMessage> {
    match msg {
        ErrorMessage::CloseWindow(id) => {
            window::close(id).map(|_| ErrorMessage::CopyToClipboard(String::new()))
        }
        ErrorMessage::CopyToClipboard(text_content) => {
            if !text_content.is_empty() {
                copy_to_clipboard(text_content);
            }
            iced::Task::none()
        }
    }
}

// =============================================================================
// ERROR LOGGING
// =============================================================================

/// Log an error and optionally persist it to disk
pub fn report_error(report: &ErrorReport) {
    log::error!(
        "Error [{}]: {} - {:?}",
        report.error_type,
        report.message,
        report.details
    );

    // Persist to file (best-effort)
    let _ = save_error_to_file(report);
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

/// Save error to log file
fn save_error_to_file(error: &ErrorReport) -> std::io::Result<()> {
    use std::fs::{create_dir_all, OpenOptions};
    use std::io::Write;

    let log_dir = get_error_log_dir()?;
    create_dir_all(&log_dir)?;

    let log_file = log_dir.join(format!(
        "errors_{}.log",
        chrono::Local::now().format("%Y-%m-%d")
    ));

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;

    writeln!(
        file,
        "[{}] {} - {}: {:?}",
        error.timestamp, error.error_type, error.message, error.details
    )?;

    if let Some(ref stack) = error.stack_trace {
        writeln!(file, "Stack trace:\n{}", stack)?;
    }

    writeln!(file, "---")?;

    Ok(())
}

/// Get error log directory
fn get_error_log_dir() -> std::io::Result<std::path::PathBuf> {
    let app_dir = if cfg!(target_os = "windows") {
        dirs::data_local_dir()
    } else {
        dirs::data_dir()
    }
    .ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find data directory",
        )
    })?;

    Ok(app_dir.join(env!("CARGO_PKG_NAME")).join("errors"))
}

// =============================================================================
// RESULT EXTENSION TRAIT
// =============================================================================

/// Extension trait for `Result` to build an `ErrorReport` from any `Err`.
pub trait ResultExt<T, E> {
    /// If `Err`, log the error and return `None`; otherwise return `Some(value)`.
    fn log_and_discard(self, context: &str) -> Option<T>;
}

impl<T, E: std::error::Error> ResultExt<T, E> for Result<T, E> {
    fn log_and_discard(self, context: &str) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                let report = ErrorReport::from_error(&error, Some(context.to_string()));
                report_error(&report);
                None
            }
        }
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window"] }
// serde = { version = "1.0", features = ["derive"] }
// serde_json = "1.0"
// chrono = { version = "0.4", features = ["serde"] }
// log = "0.4"
// clipboard = "0.5"
// dirs = "5.0"

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_report_creation() {
        let error = ErrorReport::new(
            "TestError",
            "Test message",
            Some("Additional details".to_string()),
        );

        assert_eq!(error.error_type, "TestError");
        assert_eq!(error.message, "Test message");
        assert!(error.details.is_some());
    }

    #[test]
    fn test_friendly_messages() {
        assert_eq!(
            get_friendly_message("FileNotFound"),
            "A required file could not be found."
        );
        assert_eq!(
            get_friendly_message("Unknown"),
            "An unexpected error occurred."
        );
    }

    #[test]
    fn test_error_report_serialization() {
        let error = ErrorReport::new("Test", "Message", None);
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: ErrorReport = serde_json::from_str(&json).unwrap();

        assert_eq!(error.error_type, deserialized.error_type);
        assert_eq!(error.message, deserialized.message);
    }

    #[test]
    fn test_clipboard_text_format() {
        let error = ErrorReport::new("TestType", "Test msg", Some("ctx".to_string()));
        let text = error.to_clipboard_text();
        assert!(text.contains("TestType"));
        assert!(text.contains("Test msg"));
        assert!(text.contains("ctx"));
    }
}
