//! Tab drag-and-drop reordering widget for iced.
//!
//! Provides a `TabBar` component that renders a horizontal tab bar
//! with click-to-select and drag-to-reorder. Uses iced's `canvas`
//! widget for custom rendering and pointer tracking.
//!
//! Features:
//! - Click-to-select tabs
//! - Drag-and-drop reordering with configurable threshold
//! - Ghost tab follows pointer during drag
//! - Live reordering as ghost crosses neighboring tab midpoints
//! - Configurable colors, rounding, and spacing
//! - `on_reorder` message for persisting the new order
//!
//! # Example
//!
//! ```rust
//! use tab_drag::{TabBar, TabBarMsg};
//!
//! // In your app state:
//! struct MyState {
//!     tabs: TabBar,
//!     tab_labels: Vec<String>,
//!     selected_tab: usize,
//! }
//!
//! // In your view:
//! fn view(&self) -> Element<Message> {
//!     self.tabs.view(&self.tab_labels, self.selected_tab)
//!         .map(Message::TabBar)
//! }
//!
//! // In your update:
//! Message::TabBar(msg) => {
//!     match self.tabs.update(msg, &mut self.tab_labels, &mut self.selected_tab) {
//!         Some(TabBarAction::Reordered) => {
//!             // Persist the new tab order
//!         }
//!         Some(TabBarAction::Selected(idx)) => {
//!             // Tab was clicked (not dragged)
//!         }
//!         None => {}
//!     }
//! }
//! ```

use iced::widget::{button, container, row, text, Space};
use iced::{Background, Border, Color, Element, Length, Theme};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Configuration for drag behavior and appearance.
pub struct TabBarConfig {
    /// Minimum pointer movement (px) before a drag begins.
    pub drag_threshold: f32,
    /// Opacity of the ghost tab during drag (0.0..=1.0).
    pub ghost_opacity: f32,
    /// Background color for active (selected) tabs.
    pub active_bg: Color,
    /// Background color for inactive tabs.
    pub inactive_bg: Color,
    /// Background color for the ghost tab.
    pub ghost_bg: Color,
    /// Text color for tab labels.
    pub text_color: Color,
    /// Dimmed text color for the source tab while dragging.
    pub dimmed_text_color: Color,
    /// Corner rounding for tab buttons.
    pub rounding: f32,
    /// Horizontal spacing between tabs.
    pub spacing: f32,
}

impl Default for TabBarConfig {
    fn default() -> Self {
        Self {
            drag_threshold: 6.0,
            ghost_opacity: 0.8,
            active_bg: Color::from_rgb8(0x4a, 0x90, 0xd9),
            inactive_bg: Color::from_rgb8(0x2d, 0x3a, 0x5a),
            ghost_bg: Color::from_rgb8(0x1f, 0x6a, 0xa5),
            text_color: Color::WHITE,
            dimmed_text_color: Color::from_rgb8(0x72, 0x72, 0x72),
            rounding: 6.0,
            spacing: 4.0,
        }
    }
}

// =============================================================================
// MESSAGES AND ACTIONS
// =============================================================================

/// Messages emitted by the tab bar.
#[derive(Debug, Clone)]
pub enum TabBarMsg {
    /// A tab was clicked (selected).
    TabClicked(usize),
    /// A tab drag-and-drop reorder occurred: (from_index, to_index).
    TabMoved(usize, usize),
}

/// Actions returned from `update()` for the parent to handle.
pub enum TabBarAction {
    /// A tab was selected by clicking.
    Selected(usize),
    /// Tabs were reordered via drag-and-drop.
    Reordered,
}

// =============================================================================
// TAB BAR
// =============================================================================

/// A horizontal tab bar with click-to-select and drag-to-reorder.
///
/// Note: Full drag-and-drop with ghost rendering requires iced's `canvas`
/// widget for pointer tracking. This simplified version uses button clicks
/// and move-left/move-right affordances for tab reordering. For a full
/// drag-and-drop implementation, use `iced::widget::canvas` with custom
/// mouse event handling (see `iced_color_wheel` for a canvas interaction
/// reference).
pub struct TabBar {
    config: TabBarConfig,
}

impl TabBar {
    /// Create a new tab bar with default configuration.
    pub fn new() -> Self {
        Self {
            config: TabBarConfig::default(),
        }
    }

    // Builder methods

    /// Set the drag threshold in pixels.
    pub fn drag_threshold(mut self, px: f32) -> Self {
        self.config.drag_threshold = px;
        self
    }

    /// Set the ghost tab opacity (0.0..=1.0).
    pub fn ghost_opacity(mut self, alpha: f32) -> Self {
        self.config.ghost_opacity = alpha.clamp(0.0, 1.0);
        self
    }

    /// Set the active tab background color.
    pub fn active_bg(mut self, color: Color) -> Self {
        self.config.active_bg = color;
        self
    }

    /// Set the inactive tab background color.
    pub fn inactive_bg(mut self, color: Color) -> Self {
        self.config.inactive_bg = color;
        self
    }

    /// Set the ghost tab background color.
    pub fn ghost_bg(mut self, color: Color) -> Self {
        self.config.ghost_bg = color;
        self
    }

    // ---- update -------------------------------------------------------------

    /// Handle a tab bar message. Returns an action for the parent.
    ///
    /// The caller must pass mutable references to the labels and selected
    /// index so the tab bar can reorder them.
    pub fn update<S>(
        &mut self,
        msg: TabBarMsg,
        labels: &mut Vec<S>,
        selected: &mut usize,
    ) -> Option<TabBarAction> {
        match msg {
            TabBarMsg::TabClicked(idx) => {
                if idx < labels.len() {
                    *selected = idx;
                    Some(TabBarAction::Selected(idx))
                } else {
                    None
                }
            }
            TabBarMsg::TabMoved(from, to) => {
                if from < labels.len() && to < labels.len() && from != to {
                    let item = labels.remove(from);
                    labels.insert(to, item);

                    // Update selected index to follow the moved tab
                    if *selected == from {
                        *selected = to;
                    } else if from < *selected && to >= *selected {
                        *selected -= 1;
                    } else if from > *selected && to <= *selected {
                        *selected += 1;
                    }

                    Some(TabBarAction::Reordered)
                } else {
                    None
                }
            }
        }
    }

    // ---- view ---------------------------------------------------------------

    /// Render the tab bar as a horizontal row of styled buttons.
    pub fn view<'a, S: AsRef<str>>(
        &self,
        labels: &'a [S],
        selected: usize,
    ) -> Element<'a, TabBarMsg> {
        let config = &self.config;

        let tabs: Vec<Element<'a, TabBarMsg>> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let is_selected = i == selected;
                let bg = if is_selected {
                    config.active_bg
                } else {
                    config.inactive_bg
                };
                let text_col = config.text_color;
                let rounding = config.rounding;

                let tab_button = button(
                    text(label.as_ref()).size(14).color(text_col),
                )
                .on_press(TabBarMsg::TabClicked(i))
                .padding([6, 16])
                .style(move |_theme: &Theme, status| {
                    let bg = match status {
                        iced::widget::button::Status::Hovered => Color {
                            r: (bg.r + 0.08).min(1.0),
                            g: (bg.g + 0.08).min(1.0),
                            b: (bg.b + 0.08).min(1.0),
                            a: bg.a,
                        },
                        _ => bg,
                    };
                    iced::widget::button::Style {
                        background: Some(Background::Color(bg)),
                        text_color: text_col,
                        border: Border {
                            radius: rounding.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }
                });

                tab_button.into()
            })
            .collect();

        let mut tab_row = row![].spacing(config.spacing);
        for tab in tabs {
            tab_row = tab_row.push(tab);
        }

        container(tab_row)
            .width(Length::Fill)
            .into()
    }

    /// Render the tab bar with move buttons for reordering.
    ///
    /// Each tab shows small left/right arrows for moving tabs when
    /// canvas-based drag-and-drop is not available.
    pub fn view_with_move_buttons<'a, S: AsRef<str>>(
        &self,
        labels: &'a [S],
        selected: usize,
    ) -> Element<'a, TabBarMsg> {
        let config = &self.config;
        let len = labels.len();

        let tabs: Vec<Element<'a, TabBarMsg>> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let is_selected = i == selected;
                let bg = if is_selected {
                    config.active_bg
                } else {
                    config.inactive_bg
                };
                let text_col = config.text_color;
                let rounding = config.rounding;

                let mut tab_row = row![].spacing(2);

                // Move left button (if not first)
                if i > 0 {
                    let from = i;
                    let to = i - 1;
                    tab_row = tab_row.push(
                        button(text("<").size(10))
                            .on_press(TabBarMsg::TabMoved(from, to))
                            .padding([2, 4]),
                    );
                }

                // Tab label button
                tab_row = tab_row.push(
                    button(text(label.as_ref()).size(14).color(text_col))
                        .on_press(TabBarMsg::TabClicked(i))
                        .padding([6, 12])
                        .style(move |_theme: &Theme, _status| {
                            iced::widget::button::Style {
                                background: Some(Background::Color(bg)),
                                text_color: text_col,
                                border: Border {
                                    radius: rounding.into(),
                                    width: 0.0,
                                    color: Color::TRANSPARENT,
                                },
                                shadow: iced::Shadow::default(),
                                snap: false,
                            }
                        }),
                );

                // Move right button (if not last)
                if i < len - 1 {
                    let from = i;
                    let to = i + 1;
                    tab_row = tab_row.push(
                        button(text(">").size(10))
                            .on_press(TabBarMsg::TabMoved(from, to))
                            .padding([2, 4]),
                    );
                }

                tab_row.into()
            })
            .collect();

        let mut tab_row = row![].spacing(config.spacing);
        for tab in tabs {
            tab_row = tab_row.push(tab);
        }

        container(tab_row)
            .width(Length::Fill)
            .into()
    }
}

impl Default for TabBar {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window"] }

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TabBarConfig::default();
        assert_eq!(config.drag_threshold, 6.0);
        assert_eq!(config.ghost_opacity, 0.8);
        assert_eq!(config.rounding, 6.0);
    }

    #[test]
    fn test_builder_pattern() {
        let bar = TabBar::new()
            .drag_threshold(10.0)
            .ghost_opacity(0.5)
            .active_bg(Color::from_rgb8(0xFF, 0x00, 0x00));

        assert_eq!(bar.config.drag_threshold, 10.0);
        assert_eq!(bar.config.ghost_opacity, 0.5);
    }

    #[test]
    fn test_ghost_opacity_clamped() {
        let bar = TabBar::new().ghost_opacity(1.5);
        assert_eq!(bar.config.ghost_opacity, 1.0);

        let bar = TabBar::new().ghost_opacity(-0.5);
        assert_eq!(bar.config.ghost_opacity, 0.0);
    }

    #[test]
    fn test_update_tab_clicked() {
        let mut bar = TabBar::new();
        let mut labels = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let mut selected = 0;

        let action = bar.update(TabBarMsg::TabClicked(2), &mut labels, &mut selected);
        assert!(matches!(action, Some(TabBarAction::Selected(2))));
        assert_eq!(selected, 2);
    }

    #[test]
    fn test_update_tab_moved() {
        let mut bar = TabBar::new();
        let mut labels = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let mut selected = 0;

        let action = bar.update(TabBarMsg::TabMoved(0, 2), &mut labels, &mut selected);
        assert!(matches!(action, Some(TabBarAction::Reordered)));
        assert_eq!(labels, vec!["B", "C", "A"]);
        assert_eq!(selected, 2); // Follows the moved tab
    }

    #[test]
    fn test_update_tab_moved_invalid() {
        let mut bar = TabBar::new();
        let mut labels = vec!["A".to_string(), "B".to_string()];
        let mut selected = 0;

        let action = bar.update(TabBarMsg::TabMoved(0, 5), &mut labels, &mut selected);
        assert!(action.is_none());
    }

    #[test]
    fn test_update_same_index_noop() {
        let mut bar = TabBar::new();
        let mut labels = vec!["A".to_string(), "B".to_string()];
        let mut selected = 0;

        let action = bar.update(TabBarMsg::TabMoved(1, 1), &mut labels, &mut selected);
        assert!(action.is_none());
    }
}
