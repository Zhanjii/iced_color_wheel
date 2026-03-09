//! colors.rs
//!
//! Centralized color constants for consistent theming.
//!
//! Provides a single source of truth for all colors used in the application.
//! Update these values to change the application's color scheme.
//!
//! # Usage
//!
//! ```rust
//! use colors::Colors;
//!
//! let primary_bg = Colors::BG_PRIMARY;
//! let text_color = Colors::TEXT_PRIMARY;
//! ```

/// Application color constants
///
/// Organized by category for easy reference and modification.
/// All colors are hex strings (e.g., "#FF0000").
pub struct Colors;

impl Colors {
    // =========================================================================
    // BACKGROUND COLORS
    // =========================================================================

    /// Main window background
    pub const BG_PRIMARY: &'static str = "#1a1a2e";
    /// Secondary panels, sidebars
    pub const BG_SECONDARY: &'static str = "#16213e";
    /// Card/frame backgrounds
    pub const BG_CARD: &'static str = "#1f2940";
    /// Input field backgrounds
    pub const BG_INPUT: &'static str = "#2d3a5a";
    /// Hover state for interactive elements
    pub const BG_HOVER: &'static str = "#3d4f6f";

    // =========================================================================
    // TEXT COLORS
    // =========================================================================

    /// Main text (headings, important content)
    pub const TEXT_PRIMARY: &'static str = "#ffffff";
    /// Secondary text (descriptions, labels)
    pub const TEXT_SECONDARY: &'static str = "#8892a0";
    /// Disabled/inactive text
    pub const TEXT_DISABLED: &'static str = "#5a6270";
    /// Placeholder text in inputs
    pub const TEXT_PLACEHOLDER: &'static str = "#6c757d";
    /// Link text
    pub const TEXT_LINK: &'static str = "#4a90d9";

    // =========================================================================
    // ACCENT COLORS
    // =========================================================================

    /// Primary accent (buttons, links, focus)
    pub const ACCENT_PRIMARY: &'static str = "#4a90d9";
    /// Secondary accent
    pub const ACCENT_SECONDARY: &'static str = "#6c5ce7";
    /// Informational accent
    pub const ACCENT_INFO: &'static str = "#17a2b8";

    // =========================================================================
    // STATUS COLORS
    // =========================================================================

    /// Success/positive states
    pub const STATUS_SUCCESS: &'static str = "#28a745";
    /// Warning states
    pub const STATUS_WARNING: &'static str = "#ffc107";
    /// Error/danger states
    pub const STATUS_ERROR: &'static str = "#dc3545";
    /// Info states
    pub const STATUS_INFO: &'static str = "#17a2b8";

    // =========================================================================
    // BUTTON COLORS
    // =========================================================================

    /// Primary button background
    pub const BUTTON_PRIMARY: &'static str = "#4a90d9";
    /// Primary button hover
    pub const BUTTON_PRIMARY_HOVER: &'static str = "#3a7fc8";
    /// Primary button disabled
    pub const BUTTON_PRIMARY_DISABLED: &'static str = "#3d5a80";

    /// Secondary button background
    pub const BUTTON_SECONDARY: &'static str = "#6c757d";
    /// Secondary button hover
    pub const BUTTON_SECONDARY_HOVER: &'static str = "#5a6268";

    /// Danger button background
    pub const BUTTON_DANGER: &'static str = "#dc3545";
    /// Danger button hover
    pub const BUTTON_DANGER_HOVER: &'static str = "#c82333";

    /// Success button background
    pub const BUTTON_SUCCESS: &'static str = "#28a745";
    /// Success button hover
    pub const BUTTON_SUCCESS_HOVER: &'static str = "#218838";

    // =========================================================================
    // PROGRESS BAR
    // =========================================================================

    /// Progress bar track background
    pub const PROGRESS_BG: &'static str = "#2d3a5a";
    /// Progress bar fill
    pub const PROGRESS_FILL: &'static str = "#4a90d9";

    // =========================================================================
    // BORDER COLORS
    // =========================================================================

    /// Default border color
    pub const BORDER_DEFAULT: &'static str = "#3d4f6f";
    /// Focused element border
    pub const BORDER_FOCUS: &'static str = "#4a90d9";
    /// Error state border
    pub const BORDER_ERROR: &'static str = "#dc3545";

    // =========================================================================
    // SCROLLBAR
    // =========================================================================

    /// Scrollbar track
    pub const SCROLLBAR_BG: &'static str = "#1f2940";
    /// Scrollbar thumb
    pub const SCROLLBAR_FG: &'static str = "#3d4f6f";
    /// Scrollbar thumb hover
    pub const SCROLLBAR_HOVER: &'static str = "#4a5a7f";

    // =========================================================================
    // TAB COLORS
    // =========================================================================

    /// Active tab background
    pub const TAB_ACTIVE: &'static str = "#4a90d9";
    /// Inactive tab background
    pub const TAB_INACTIVE: &'static str = "#2d3a5a";
    /// Tab hover state
    pub const TAB_HOVER: &'static str = "#3d4f6f";

    // =========================================================================
    // DIALOG/OVERLAY
    // =========================================================================

    /// Overlay background (for dialogs)
    pub const OVERLAY_BG: &'static str = "rgba(0, 0, 0, 0.5)";
    /// Dialog background
    pub const DIALOG_BG: &'static str = "#1f2940";

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    /// Get color for a status string
    pub fn get_status_color(status: &str) -> &'static str {
        match status.to_lowercase().as_str() {
            "success" => Self::STATUS_SUCCESS,
            "warning" => Self::STATUS_WARNING,
            "error" => Self::STATUS_ERROR,
            "info" => Self::STATUS_INFO,
            _ => Self::TEXT_PRIMARY,
        }
    }

    /// Get button colors for a variant
    ///
    /// Returns tuple of (bg_color, hover_color)
    pub fn get_button_colors(variant: &str) -> (&'static str, &'static str) {
        match variant.to_lowercase().as_str() {
            "primary" => (Self::BUTTON_PRIMARY, Self::BUTTON_PRIMARY_HOVER),
            "secondary" => (Self::BUTTON_SECONDARY, Self::BUTTON_SECONDARY_HOVER),
            "danger" => (Self::BUTTON_DANGER, Self::BUTTON_DANGER_HOVER),
            "success" => (Self::BUTTON_SUCCESS, Self::BUTTON_SUCCESS_HOVER),
            _ => (Self::BUTTON_PRIMARY, Self::BUTTON_PRIMARY_HOVER),
        }
    }
}

// =============================================================================
// LIGHT THEME (optional alternative)
// =============================================================================

/// Light theme color constants
///
/// Alternative color scheme for light mode.
pub struct LightColors;

impl LightColors {
    // Background colors
    pub const BG_PRIMARY: &'static str = "#f5f5f5";
    pub const BG_SECONDARY: &'static str = "#ffffff";
    pub const BG_CARD: &'static str = "#ffffff";
    pub const BG_INPUT: &'static str = "#f0f0f0";

    // Text colors
    pub const TEXT_PRIMARY: &'static str = "#212121";
    pub const TEXT_SECONDARY: &'static str = "#757575";
    pub const TEXT_DISABLED: &'static str = "#bdbdbd";

    // Accent colors
    pub const ACCENT_PRIMARY: &'static str = "#1976d2";
    pub const ACCENT_SECONDARY: &'static str = "#7c4dff";

    // Status colors
    pub const STATUS_SUCCESS: &'static str = "#4caf50";
    pub const STATUS_WARNING: &'static str = "#ff9800";
    pub const STATUS_ERROR: &'static str = "#f44336";

    // Button colors
    pub const BUTTON_PRIMARY: &'static str = "#1976d2";
    pub const BUTTON_PRIMARY_HOVER: &'static str = "#1565c0";
}

// =============================================================================
// THEME MANAGER (optional)
// =============================================================================

/// Manage application theme
pub struct ThemeManager {
    dark_mode: bool,
}

impl ThemeManager {
    /// Create a new theme manager (default: dark mode)
    pub fn new() -> Self {
        Self { dark_mode: true }
    }

    /// Check if dark mode is active
    pub fn is_dark_mode(&self) -> bool {
        self.dark_mode
    }

    /// Switch to dark mode
    pub fn set_dark_mode(&mut self) {
        self.dark_mode = true;
    }

    /// Switch to light mode
    pub fn set_light_mode(&mut self) {
        self.dark_mode = false;
    }

    /// Toggle between dark and light mode
    pub fn toggle(&mut self) {
        self.dark_mode = !self.dark_mode;
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_colors() {
        assert_eq!(Colors::get_status_color("success"), Colors::STATUS_SUCCESS);
        assert_eq!(Colors::get_status_color("error"), Colors::STATUS_ERROR);
        assert_eq!(Colors::get_status_color("unknown"), Colors::TEXT_PRIMARY);
    }

    #[test]
    fn test_button_colors() {
        let (bg, hover) = Colors::get_button_colors("primary");
        assert_eq!(bg, Colors::BUTTON_PRIMARY);
        assert_eq!(hover, Colors::BUTTON_PRIMARY_HOVER);
    }

    #[test]
    fn test_theme_manager() {
        let mut theme = ThemeManager::new();
        assert!(theme.is_dark_mode());

        theme.set_light_mode();
        assert!(!theme.is_dark_mode());

        theme.toggle();
        assert!(theme.is_dark_mode());
    }
}
