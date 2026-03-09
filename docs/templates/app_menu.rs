//! app_menu.rs
//!
//! Application toolbar / button bar for iced daemon applications.
//!
//! Iced (v0.14) does not expose a native OS menu bar API. Instead, this module
//! provides a horizontal toolbar widget with dropdown-style menus built from
//! `pick_list` and plain buttons. Keyboard shortcuts are handled via an iced
//! `Subscription` that listens for key events.
//!
//! Features:
//! - Cross-platform toolbar (File, Edit, Help)
//! - Platform-specific keyboard shortcuts (Cmd on macOS, Ctrl elsewhere)
//! - Dropdown actions via `pick_list`
//! - Keyboard shortcut subscription
//!
//! # Example
//!
//! ```rust
//! use app_menu::{view_toolbar, MenuMessage, keyboard_shortcut_subscription};
//! use iced::Element;
//!
//! // In your daemon's view():
//! let toolbar = view_toolbar();
//! let page_content = /* ... */;
//! column![toolbar, page_content].into()
//!
//! // In your daemon's subscription():
//! keyboard_shortcut_subscription().map(Message::Menu)
//! ```

use iced::event;
use iced::keyboard;
use iced::widget::{button, container, pick_list, row, text, Row};
use iced::{Alignment, Element, Length, Padding, Subscription};

// =============================================================================
// CONFIGURATION - Customize for your app
// =============================================================================

/// Documentation URL
const DOCS_URL: &str = "https://docs.yourcompany.com/";

/// Application name (from Cargo.toml)
const APP_NAME: &str = env!("CARGO_PKG_NAME");

// =============================================================================
// MESSAGES
// =============================================================================

/// Messages emitted by the toolbar / menu system
#[derive(Debug, Clone)]
pub enum MenuMessage {
    // -- File actions --------------------------------------------------------
    /// Open the Settings / Preferences panel
    ShowSettings,
    /// Close the currently focused window
    CloseWindow,
    /// Quit the entire application
    Quit,

    // -- Edit actions --------------------------------------------------------
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,

    // -- Help actions --------------------------------------------------------
    /// Open documentation in the default browser
    OpenDocumentation,
    /// Trigger an update check
    CheckForUpdates,
    /// Show the changelog
    ViewChangelog,
    /// Open the About popup
    ShowAbout,

    // -- Internal (pick_list plumbing) ----------------------------------------
    /// A File menu item was picked
    FileMenuPicked(FileAction),
    /// An Edit menu item was picked
    EditMenuPicked(EditAction),
    /// A Help menu item was picked
    HelpMenuPicked(HelpAction),
}

// =============================================================================
// PICK-LIST ACTION ENUMS
// =============================================================================

/// Actions available in the File dropdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    Settings,
    CloseWindow,
    Quit,
}

impl std::fmt::Display for FileAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings => {
                if cfg!(target_os = "macos") {
                    write!(f, "Preferences...  (Cmd+,)")
                } else {
                    write!(f, "Settings  (Ctrl+,)")
                }
            }
            Self::CloseWindow => write!(f, "Close Window"),
            Self::Quit => {
                if cfg!(target_os = "macos") {
                    write!(f, "Quit  (Cmd+Q)")
                } else {
                    write!(f, "Exit  (Alt+F4)")
                }
            }
        }
    }
}

/// All file actions for the pick_list
const FILE_ACTIONS: &[FileAction] = &[
    FileAction::Settings,
    FileAction::CloseWindow,
    FileAction::Quit,
];

/// Actions available in the Edit dropdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditAction {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl std::fmt::Display for EditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let modifier = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        match self {
            Self::Undo => write!(f, "Undo  ({}+Z)", modifier),
            Self::Redo => write!(f, "Redo  ({}+Shift+Z)", modifier),
            Self::Cut => write!(f, "Cut  ({}+X)", modifier),
            Self::Copy => write!(f, "Copy  ({}+C)", modifier),
            Self::Paste => write!(f, "Paste  ({}+V)", modifier),
            Self::SelectAll => write!(f, "Select All  ({}+A)", modifier),
        }
    }
}

/// All edit actions for the pick_list
const EDIT_ACTIONS: &[EditAction] = &[
    EditAction::Undo,
    EditAction::Redo,
    EditAction::Cut,
    EditAction::Copy,
    EditAction::Paste,
    EditAction::SelectAll,
];

/// Actions available in the Help dropdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpAction {
    Documentation,
    CheckForUpdates,
    ViewChangelog,
    About,
}

impl std::fmt::Display for HelpAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Documentation => write!(f, "Documentation"),
            Self::CheckForUpdates => write!(f, "Check for Updates"),
            Self::ViewChangelog => write!(f, "View Changelog"),
            Self::About => write!(f, "About {}", APP_NAME),
        }
    }
}

/// All help actions for the pick_list
const HELP_ACTIONS: &[HelpAction] = &[
    HelpAction::Documentation,
    HelpAction::CheckForUpdates,
    HelpAction::ViewChangelog,
    HelpAction::About,
];

// =============================================================================
// TOOLBAR VIEW
// =============================================================================

/// Render the application toolbar as a horizontal row of dropdown menus.
///
/// Each "menu" is an `iced::widget::pick_list` that resets to `None` after
/// selection so it always shows the category label.
///
/// # Returns
///
/// An `Element<MenuMessage>` to place at the top of your window layout.
pub fn view_toolbar<'a>() -> Element<'a, MenuMessage> {
    let file_menu: Element<'a, MenuMessage> = pick_list(
        FILE_ACTIONS,
        None::<FileAction>,
        MenuMessage::FileMenuPicked,
    )
    .placeholder("File")
    .width(Length::Shrink)
    .into();

    let edit_menu: Element<'a, MenuMessage> = pick_list(
        EDIT_ACTIONS,
        None::<EditAction>,
        MenuMessage::EditMenuPicked,
    )
    .placeholder("Edit")
    .width(Length::Shrink)
    .into();

    let help_menu: Element<'a, MenuMessage> = pick_list(
        HELP_ACTIONS,
        None::<HelpAction>,
        MenuMessage::HelpMenuPicked,
    )
    .placeholder("Help")
    .width(Length::Shrink)
    .into();

    let toolbar: Row<'a, MenuMessage> = row![file_menu, edit_menu, help_menu]
        .spacing(4)
        .align_y(Alignment::Center)
        .padding(Padding::from([4, 8]));

    container(toolbar)
        .width(Length::Fill)
        .into()
}

// =============================================================================
// PICK-LIST -> SEMANTIC MESSAGE TRANSLATION
// =============================================================================

/// Translate a raw pick-list selection into the semantic `MenuMessage`.
///
/// Call this at the top of your daemon `update()` to normalize menu messages:
///
/// ```rust
/// Message::Menu(menu_msg) => {
///     let resolved = resolve_menu_message(menu_msg);
///     // now match on resolved...
/// }
/// ```
pub fn resolve_menu_message(msg: MenuMessage) -> MenuMessage {
    match msg {
        MenuMessage::FileMenuPicked(action) => match action {
            FileAction::Settings => MenuMessage::ShowSettings,
            FileAction::CloseWindow => MenuMessage::CloseWindow,
            FileAction::Quit => MenuMessage::Quit,
        },
        MenuMessage::EditMenuPicked(action) => match action {
            EditAction::Undo => MenuMessage::Undo,
            EditAction::Redo => MenuMessage::Redo,
            EditAction::Cut => MenuMessage::Cut,
            EditAction::Copy => MenuMessage::Copy,
            EditAction::Paste => MenuMessage::Paste,
            EditAction::SelectAll => MenuMessage::SelectAll,
        },
        MenuMessage::HelpMenuPicked(action) => match action {
            HelpAction::Documentation => MenuMessage::OpenDocumentation,
            HelpAction::CheckForUpdates => MenuMessage::CheckForUpdates,
            HelpAction::ViewChangelog => MenuMessage::ViewChangelog,
            HelpAction::About => MenuMessage::ShowAbout,
        },
        other => other,
    }
}

// =============================================================================
// UPDATE HELPER
// =============================================================================

/// Handle a resolved `MenuMessage`.
///
/// Opens URLs, exits the app, etc. Returns an `iced::Task` that the caller
/// should map into their top-level `Message`.
pub fn handle_menu_message(msg: &MenuMessage) -> iced::Task<MenuMessage> {
    match msg {
        MenuMessage::Quit => {
            std::process::exit(0);
        }
        MenuMessage::OpenDocumentation => {
            let _ = open::that(DOCS_URL);
            iced::Task::none()
        }
        // CloseWindow, ShowSettings, ShowAbout, CheckForUpdates, ViewChangelog
        // and Edit actions should be handled by the caller in its own update().
        _ => iced::Task::none(),
    }
}

// =============================================================================
// KEYBOARD SHORTCUT SUBSCRIPTION
// =============================================================================

/// Return an iced `Subscription` that maps common keyboard shortcuts to
/// `MenuMessage` variants.
///
/// Handles platform-aware modifier keys (Cmd on macOS, Ctrl elsewhere).
pub fn keyboard_shortcut_subscription() -> Subscription<MenuMessage> {
    event::listen_with(|event, _status, _id| {
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            ..
        }) = &event
        {
            let primary = if cfg!(target_os = "macos") {
                modifiers.command()
            } else {
                modifiers.control()
            };

            if primary {
                return match key.as_ref() {
                    keyboard::Key::Character("z") if modifiers.shift() => {
                        Some(MenuMessage::Redo)
                    }
                    keyboard::Key::Character("z") => Some(MenuMessage::Undo),
                    keyboard::Key::Character("x") => Some(MenuMessage::Cut),
                    keyboard::Key::Character("c") => Some(MenuMessage::Copy),
                    keyboard::Key::Character("v") => Some(MenuMessage::Paste),
                    keyboard::Key::Character("a") => Some(MenuMessage::SelectAll),
                    keyboard::Key::Character(",") => Some(MenuMessage::ShowSettings),
                    keyboard::Key::Character("q") if cfg!(target_os = "macos") => {
                        Some(MenuMessage::Quit)
                    }
                    _ => None,
                };
            }
        }

        None
    })
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window"] }
// open = "5.0"

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_action_display() {
        let s = format!("{}", FileAction::Settings);
        assert!(s.contains("Settings") || s.contains("Preferences"));
    }

    #[test]
    fn test_edit_action_display() {
        let s = format!("{}", EditAction::Undo);
        assert!(s.contains("Undo"));
    }

    #[test]
    fn test_help_action_display() {
        let s = format!("{}", HelpAction::About);
        assert!(s.contains("About"));
        assert!(s.contains(APP_NAME));
    }

    #[test]
    fn test_resolve_file_menu() {
        let resolved = resolve_menu_message(MenuMessage::FileMenuPicked(FileAction::Quit));
        assert!(matches!(resolved, MenuMessage::Quit));
    }

    #[test]
    fn test_resolve_edit_menu() {
        let resolved = resolve_menu_message(MenuMessage::EditMenuPicked(EditAction::Copy));
        assert!(matches!(resolved, MenuMessage::Copy));
    }

    #[test]
    fn test_resolve_help_menu() {
        let resolved = resolve_menu_message(MenuMessage::HelpMenuPicked(HelpAction::Documentation));
        assert!(matches!(resolved, MenuMessage::OpenDocumentation));
    }

    #[test]
    fn test_resolve_passthrough() {
        let resolved = resolve_menu_message(MenuMessage::Quit);
        assert!(matches!(resolved, MenuMessage::Quit));
    }
}
