//! Message dialog utilities.
//!
//! Provides native OS message dialogs via `rfd` and an iced popup window
//! alternative for in-app modal messages.
//!
//! Features:
//! - `show_info`, `show_warning`, `show_error` for display-only native dialogs
//! - `ask_yes_no`, `ask_ok_cancel` for native confirmation dialogs
//! - `ModalMessage` struct for an iced popup window modal
//! - `ModalIcon`, `ModalButtons`, `ModalResponse` enums
//! - Consistent API across native and iced variants
//!
//! # Example
//!
//! ```rust
//! use messagebox::{show_info, show_error, ask_yes_no};
//!
//! // Native OS dialogs (blocking, thread-safe)
//! show_info("Welcome", "Application started successfully.");
//! show_error("Error", "Failed to open the file.");
//!
//! if ask_yes_no("Confirm", "Discard unsaved changes?") {
//!     // user clicked Yes
//! }
//! ```

// =============================================================================
// NATIVE DIALOGS (rfd)
// =============================================================================

/// Show an informational message dialog (OK button).
pub fn show_info(title: &str, message: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Show a warning message dialog (OK button).
pub fn show_warning(title: &str, message: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Warning)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Show an error message dialog (OK button).
pub fn show_error(title: &str, message: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Ask a Yes/No question. Returns `true` for Yes.
pub fn ask_yes_no(title: &str, message: &str) -> bool {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
}

/// Ask an OK/Cancel question. Returns `true` for OK.
pub fn ask_ok_cancel(title: &str, message: &str) -> bool {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::OkCancel)
        .show()
}

/// Ask a Yes/No question with a custom level. Returns `true` for Yes.
pub fn ask_yes_no_with_level(title: &str, message: &str, level: rfd::MessageLevel) -> bool {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(level)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
}

// =============================================================================
// ICED POPUP WINDOW MODAL
// =============================================================================

use iced::widget::{button, center, column, container, row, text, Space};
use iced::window;
use iced::{Color, Element, Length, Size, Task};

/// Icon style for the modal message.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ModalIcon {
    Info,
    Warning,
    Error,
    Question,
    None,
}

impl ModalIcon {
    /// Get the display character and color for this icon.
    fn display(&self) -> Option<(&str, Color)> {
        match self {
            Self::Info => Some(("i", Color::from_rgb(0.09, 0.64, 0.72))),
            Self::Warning => Some(("!", Color::from_rgb(1.0, 0.76, 0.03))),
            Self::Error => Some(("X", Color::from_rgb(0.86, 0.21, 0.27))),
            Self::Question => Some(("?", Color::from_rgb(0.29, 0.56, 0.85))),
            Self::None => None,
        }
    }
}

/// Button configuration for the modal.
#[derive(Clone, PartialEq, Debug)]
pub enum ModalButtons {
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
}

/// Result of a modal button click.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ModalResponse {
    /// Modal is still open; no button was clicked.
    Open,
    /// OK or Yes was clicked.
    Confirmed,
    /// No was clicked (only for YesNo / YesNoCancel).
    Denied,
    /// Cancel was clicked (or window closed).
    Cancelled,
}

/// Messages emitted by the modal popup window.
///
/// Wire these into your top-level `Message` enum:
///
/// ```rust
/// enum Message {
///     Modal(ModalMessage),
///     // ...
/// }
/// ```
#[derive(Debug, Clone)]
pub enum ModalMessage {
    /// User clicked Confirm / OK / Yes
    Confirmed,
    /// User clicked No
    Denied,
    /// User clicked Cancel or closed the window
    Cancelled,
}

/// An iced popup window modal dialog.
///
/// Instead of rendering inside an existing window (like overlay modals),
/// this opens a small separate window via `window::open()`. The parent
/// daemon app tracks the window ID and dispatches view/update to this state.
///
/// # Usage
///
/// ```rust
/// // 1. Create the modal
/// let mut modal = ModalMessage::new("Confirm Delete")
///     .message("This cannot be undone.")
///     .icon(ModalIcon::Warning)
///     .buttons(ModalButtons::YesNo);
///
/// // 2. Open it (returns a window ID and task)
/// let (id, task) = modal.open();
/// self.modal_window_id = Some(id);
///
/// // 3. In view() dispatch:
/// Some(WindowKind::Modal) => self.modal.view().map(Message::Modal),
///
/// // 4. In update(), handle the response:
/// Message::Modal(ModalMessage::Confirmed) => { /* proceed */ }
/// Message::Modal(ModalMessage::Cancelled) => { /* abort */ }
/// ```
pub struct ModalState {
    title: String,
    message: String,
    icon: ModalIcon,
    buttons: ModalButtons,
    window_id: Option<window::Id>,
}

impl ModalState {
    /// Create a new modal with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: String::new(),
            icon: ModalIcon::Info,
            buttons: ModalButtons::Ok,
            window_id: None,
        }
    }

    // -- Builder methods ---------------------------------------------------

    /// Set the message body.
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = msg.into();
        self
    }

    /// Set the icon style.
    pub fn icon(mut self, icon: ModalIcon) -> Self {
        self.icon = icon;
        self
    }

    /// Set the button configuration.
    pub fn buttons(mut self, buttons: ModalButtons) -> Self {
        self.buttons = buttons;
        self
    }

    // -- Control methods ---------------------------------------------------

    /// Open the modal as a popup window.
    ///
    /// Returns the window ID and a Task to execute. Register the window ID
    /// in your `HashMap<window::Id, WindowKind>` so view() dispatches here.
    pub fn open(&mut self) -> (window::Id, Task<ModalMessage>) {
        let settings = window::Settings {
            size: Size::new(380.0, 180.0),
            resizable: false,
            decorations: true,
            ..window::Settings::default()
        };

        let (id, open_task) = window::open(settings);
        self.window_id = Some(id);
        (id, open_task.discard())
    }

    /// Get the window ID if the modal is open.
    pub fn window_id(&self) -> Option<window::Id> {
        self.window_id
    }

    /// Whether the modal is currently open.
    pub fn is_open(&self) -> bool {
        self.window_id.is_some()
    }

    /// Mark the modal as closed (call when you receive WindowClosed for this ID).
    pub fn mark_closed(&mut self) {
        self.window_id = None;
    }

    /// Set the message text programmatically.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
    }

    // -- Rendering ---------------------------------------------------------

    /// Render the modal content for the popup window.
    ///
    /// Call this from your `App::view()` when the window ID matches.
    pub fn view(&self) -> Element<ModalMessage> {
        let mut content = column![].spacing(12).padding(20);

        // Icon + message row
        let mut msg_row = row![].spacing(12);

        if let Some((ch, color)) = self.icon.display() {
            let icon_label = text(ch)
                .size(18)
                .color(color);
            msg_row = msg_row.push(icon_label);
        }

        msg_row = msg_row.push(
            text(&self.message).size(14).width(Length::Fill),
        );

        content = content.push(msg_row);

        // Spacer
        content = content.push(Space::with_height(8));

        // Buttons
        let button_row = match &self.buttons {
            ModalButtons::Ok => {
                row![
                    Space::with_width(Length::Fill),
                    button("OK").on_press(ModalMessage::Confirmed),
                ]
            }
            ModalButtons::OkCancel => {
                row![
                    Space::with_width(Length::Fill),
                    button("OK").on_press(ModalMessage::Confirmed),
                    button("Cancel").on_press(ModalMessage::Cancelled),
                ]
            }
            ModalButtons::YesNo => {
                row![
                    Space::with_width(Length::Fill),
                    button("Yes").on_press(ModalMessage::Confirmed),
                    button("No").on_press(ModalMessage::Denied),
                ]
            }
            ModalButtons::YesNoCancel => {
                row![
                    Space::with_width(Length::Fill),
                    button("Yes").on_press(ModalMessage::Confirmed),
                    button("No").on_press(ModalMessage::Denied),
                    button("Cancel").on_press(ModalMessage::Cancelled),
                ]
            }
        }
        .spacing(8);

        content = content.push(button_row);

        center(container(content)).into()
    }
}

// =============================================================================
// CONVENIENCE CONSTRUCTORS
// =============================================================================

/// Create an info modal (OK button).
pub fn modal_info(title: impl Into<String>, message: impl Into<String>) -> ModalState {
    ModalState::new(title)
        .message(message)
        .icon(ModalIcon::Info)
        .buttons(ModalButtons::Ok)
}

/// Create a warning modal (OK button).
pub fn modal_warning(title: impl Into<String>, message: impl Into<String>) -> ModalState {
    ModalState::new(title)
        .message(message)
        .icon(ModalIcon::Warning)
        .buttons(ModalButtons::Ok)
}

/// Create an error modal (OK button).
pub fn modal_error(title: impl Into<String>, message: impl Into<String>) -> ModalState {
    ModalState::new(title)
        .message(message)
        .icon(ModalIcon::Error)
        .buttons(ModalButtons::Ok)
}

/// Create a confirmation modal (Yes/No buttons).
pub fn modal_confirm(title: impl Into<String>, message: impl Into<String>) -> ModalState {
    ModalState::new(title)
        .message(message)
        .icon(ModalIcon::Question)
        .buttons(ModalButtons::YesNo)
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// rfd = "0.15"                    # Native file/message dialogs
// iced = { version = "0.14", features = ["multi-window"] }

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_icon_display() {
        assert!(ModalIcon::Info.display().is_some());
        assert!(ModalIcon::Warning.display().is_some());
        assert!(ModalIcon::Error.display().is_some());
        assert!(ModalIcon::Question.display().is_some());
        assert!(ModalIcon::None.display().is_none());
    }

    #[test]
    fn test_modal_builder() {
        let modal = ModalState::new("Test")
            .message("Body text")
            .icon(ModalIcon::Warning)
            .buttons(ModalButtons::YesNo);

        assert_eq!(modal.title, "Test");
        assert_eq!(modal.message, "Body text");
        assert_eq!(modal.icon, ModalIcon::Warning);
        assert_eq!(modal.buttons, ModalButtons::YesNo);
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_mark_closed() {
        let mut modal = ModalState::new("Test");
        // Simulate having a window ID
        modal.window_id = Some(window::Id::unique());
        assert!(modal.is_open());

        modal.mark_closed();
        assert!(!modal.is_open());
        assert!(modal.window_id().is_none());
    }

    #[test]
    fn test_modal_set_message() {
        let mut modal = ModalState::new("Test");
        modal.set_message("Updated message");
        assert_eq!(modal.message, "Updated message");
    }

    #[test]
    fn test_convenience_constructors() {
        let m = modal_info("Info", "Message");
        assert!(!m.is_open());
        assert_eq!(m.icon, ModalIcon::Info);

        let m = modal_warning("Warn", "Message");
        assert_eq!(m.icon, ModalIcon::Warning);

        let m = modal_error("Err", "Message");
        assert_eq!(m.icon, ModalIcon::Error);

        let m = modal_confirm("Confirm", "Message");
        assert_eq!(m.icon, ModalIcon::Question);
        assert_eq!(m.buttons, ModalButtons::YesNo);
    }

    #[test]
    fn test_modal_response_equality() {
        assert_eq!(ModalResponse::Open, ModalResponse::Open);
        assert_ne!(ModalResponse::Confirmed, ModalResponse::Denied);
        assert_ne!(ModalResponse::Confirmed, ModalResponse::Cancelled);
    }
}
