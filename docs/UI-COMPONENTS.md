# Reusable UI Components

This guide covers reusable UI components for building consistent, professional interfaces in Rust desktop applications using iced.

## Overview

| Component | Purpose | Template File |
|-----------|---------|---------------|
| ProgressBar | Progress indicator with label and status | `widgets/progress.rs` |
| HelpPanel | Collapsible help/instructions panel | `widgets/help_panel.rs` |
| TabDragManager | Chrome-style drag-and-drop tab reordering | `widgets/tab_drag.rs` |
| ColorPicker | HSV color wheel picker (popup or inline) | `widgets/color_picker.rs` |
| MessageBox | Native message dialogs (rfd) + iced modal | `widgets/messagebox.rs` |
| TitleBar | Windows DWM dark/light title bar matching | `title_bar.rs` |
| Theme | Color constants and iced Theme customization | `theme.rs` |

---

## Component Architecture

All reusable components follow the iced component pattern:

```
+-------------------------------------------------------------+
|              Component (struct + impl)                       |
|                                                              |
|  +-------------------------------------------------------+  |
|  |  Private State                                        |  |
|  |  - progress: f32, label: String, etc.                 |  |
|  |  - No public fields; access via methods               |  |
|  +-------------------------------------------------------+  |
|                                                              |
|  Public Methods:                                             |
|  - new()                          - Constructor / builder    |
|  - view(&self) -> Element<Msg>    - Render the component    |
|  - update(&mut self, msg)         - Handle state changes     |
|  - set_*()                        - Configure options        |
|  - reset()                        - Reset to initial state   |
|                                                              |
|  Message Routing:                                            |
|  - Parent maps via .map(Message::Component)                  |
|                                                              |
|  Read-Only Accessors:                                        |
|  - current(), total(), percentage(), etc.                    |
+-------------------------------------------------------------+
```

### Design Principles

1. **Self-contained**: Struct owns its state; `view` produces an `Element`; `update` handles messages
2. **Configurable**: Accept configuration at construction; expose setter methods and builder pattern
3. **Consistent styling**: Use shared `Theme` color constants
4. **Type-safe**: Strong types; no stringly-typed configuration
5. **Documented**: Doc comments with usage examples on public items
6. **Message-based**: `update(&mut self, msg) -> Option<Action>` for state changes that the parent may need to act on

---

## ProgressBar

A progress bar with label, percentage/count display, and status states.

### Basic Usage

```rust
use crate::ui::widgets::progress::ProgressBar;

// Create progress bar
let mut progress = ProgressBar::new("Processing Files")
    .show_percentage(true);

// Update progress (current, total)
progress.update(50, 100);
// Displays: "Processing Files: 50.0%"

// Or with count display
let mut progress = ProgressBar::new("Downloading")
    .show_count(true);
progress.update(5, 20);
// Displays: "Downloading: 5/20"

// In your application's view method, embed the progress bar:
progress.view().map(Message::Progress)
```

### Implementation

```rust
// src/ui/widgets/progress.rs

use iced::widget::{column, progress_bar, row, text};
use iced::{Element, Length};
use crate::ui::theme::Theme;

/// Status state of the progress bar.
#[derive(Clone, PartialEq)]
pub enum ProgressStatus {
    Normal,
    Success,
    Error(String),
    Indeterminate(String),
}

/// A styled progress bar with label, count/percentage, and status states.
pub struct ProgressBar {
    label: String,
    current: usize,
    total: usize,
    status: ProgressStatus,
    show_percentage: bool,
    show_count: bool,
    custom_status_text: Option<String>,
    color_override: Option<iced::Color>,
}

impl ProgressBar {
    /// Create a new progress bar with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            current: 0,
            total: 0,
            status: ProgressStatus::Normal,
            show_percentage: false,
            show_count: false,
            custom_status_text: None,
            color_override: None,
        }
    }

    // Builder methods

    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    pub fn show_count(mut self, show: bool) -> Self {
        self.show_count = show;
        self
    }

    // Update methods

    /// Update progress values.
    pub fn update(&mut self, current: usize, total: usize) {
        self.current = current;
        self.total = total;
        self.custom_status_text = None;
    }

    /// Update progress with custom status text.
    pub fn update_with_status(&mut self, current: usize, total: usize, status: impl Into<String>) {
        self.current = current;
        self.total = total;
        self.custom_status_text = Some(status.into());
    }

    /// Set success state (green, 100%).
    pub fn set_success(&mut self) {
        self.status = ProgressStatus::Success;
        self.current = self.total;
    }

    /// Set error state (red).
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = ProgressStatus::Error(message.into());
    }

    /// Set indeterminate state.
    pub fn set_indeterminate(&mut self, message: impl Into<String>) {
        self.status = ProgressStatus::Indeterminate(message.into());
    }

    /// Override the progress bar color.
    pub fn set_color(&mut self, color: iced::Color) {
        self.color_override = Some(color);
    }

    /// Change the label text.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Reset to zero.
    pub fn reset(&mut self) {
        self.current = 0;
        self.total = 0;
        self.status = ProgressStatus::Normal;
        self.custom_status_text = None;
        self.color_override = None;
    }

    // Read-only accessors

    pub fn current(&self) -> usize { self.current }
    pub fn total(&self) -> usize { self.total }
    pub fn percentage(&self) -> f32 {
        if self.total == 0 { 0.0 } else { self.current as f32 / self.total as f32 }
    }

    /// Render the progress bar as an iced Element.
    pub fn view(&self) -> Element<'_, ()> {
        let fraction = self.percentage();

        column![
            row![
                text(&self.label),
                iced::widget::horizontal_space(),
                text(self.status_text()),
            ],
            progress_bar(0.0..=1.0, fraction).width(Length::Fill),
        ]
        .spacing(4)
        .into()
    }

    fn status_text(&self) -> String {
        if let Some(ref text) = self.custom_status_text {
            return text.clone();
        }

        match &self.status {
            ProgressStatus::Success => "Complete".to_string(),
            ProgressStatus::Error(msg) => msg.clone(),
            ProgressStatus::Indeterminate(msg) => msg.clone(),
            ProgressStatus::Normal => {
                if self.show_count {
                    format!("{}/{}", self.current, self.total)
                } else if self.show_percentage {
                    format!("{:.1}%", self.percentage() * 100.0)
                } else {
                    String::new()
                }
            }
        }
    }
}
```

---

## HelpPanel

A collapsible help panel with structured help items, built with iced `column` layout and a toggle button for expand/collapse.

### Basic Usage

```rust
use crate::ui::widgets::help_panel::{HelpPanel, HelpItem};

let mut help = HelpPanel::new("How to Use")
    .collapsible(true)
    .initially_expanded(false)
    .items(vec![
        HelpItem::new("Step 1", "Select your input folder using the Browse button"),
        HelpItem::new("Step 2", "Configure processing options as needed"),
        HelpItem::new("Step 3", "Click Start to begin processing"),
    ]);

// In your application's view method:
help.view().map(Message::Help)

// In your application's update method:
Message::Help(msg) => self.help.update(msg),
```

### Implementation

```rust
// src/ui/widgets/help_panel.rs

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};
use crate::ui::theme::Theme;

/// A single help item with a title and description.
#[derive(Clone)]
pub struct HelpItem {
    pub title: String,
    pub description: String,
    pub prefix: Option<String>,
}

impl HelpItem {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            prefix: None,
        }
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }
}

/// Messages produced by the HelpPanel.
#[derive(Debug, Clone)]
pub enum HelpPanelMsg {
    ToggleExpanded,
}

/// A collapsible help section that displays structured help items.
pub struct HelpPanel {
    title: String,
    items: Vec<HelpItem>,
    collapsible: bool,
    expanded: bool,
}

impl HelpPanel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
            collapsible: true,
            expanded: true,
        }
    }

    // Builder methods

    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    pub fn initially_expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn items(mut self, items: Vec<HelpItem>) -> Self {
        self.items = items;
        self
    }

    // Mutation methods

    pub fn add_item(&mut self, item: HelpItem) {
        self.items.push(item);
    }

    pub fn clear_items(&mut self) {
        self.items.clear();
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn expand(&mut self) {
        self.expanded = true;
    }

    pub fn collapse(&mut self) {
        self.expanded = false;
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Handle a HelpPanel message.
    pub fn update(&mut self, msg: HelpPanelMsg) {
        match msg {
            HelpPanelMsg::ToggleExpanded => {
                self.expanded = !self.expanded;
            }
        }
    }

    /// Render the help panel as an iced Element.
    pub fn view(&self) -> Element<'_, HelpPanelMsg> {
        let mut content = column![].spacing(4);

        if self.collapsible {
            let toggle_label = if self.expanded {
                format!("[-] {}", &self.title)
            } else {
                format!("[+] {}", &self.title)
            };

            content = content.push(
                button(text(toggle_label).style(Theme::TEXT_PRIMARY))
                    .on_press(HelpPanelMsg::ToggleExpanded)
                    .style(iced::widget::button::text),
            );

            if self.expanded {
                content = self.push_items(content);
            }
        } else {
            content = content.push(
                text(&self.title).style(Theme::TEXT_PRIMARY),
            );
            content = self.push_items(content);
        }

        container(content)
            .padding(10)
            .style(|_theme| container::Style {
                background: Some(Theme::BG_CARD.into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .width(Length::Fill)
            .into()
    }

    fn push_items<'a>(
        &'a self,
        mut col: iced::widget::Column<'a, HelpPanelMsg>,
    ) -> iced::widget::Column<'a, HelpPanelMsg> {
        for item in &self.items {
            let mut item_row = row![].spacing(4);

            if let Some(prefix) = &item.prefix {
                item_row = item_row.push(
                    text(prefix).style(Theme::ACCENT_PRIMARY),
                );
            }

            item_row = item_row.push(
                text(&item.title).style(Theme::TEXT_PRIMARY),
            );
            item_row = item_row.push(
                text(&item.description).style(Theme::TEXT_SECONDARY),
            );

            col = col.push(item_row);
        }
        col
    }
}

/// Factory: create a standard help panel with numbered steps and notes.
pub fn standard_help_panel(
    panel_name: &str,
    steps: &[&str],
    notes: &[&str],
) -> HelpPanel {
    let mut items: Vec<HelpItem> = steps
        .iter()
        .enumerate()
        .map(|(i, &step)| {
            HelpItem::new(format!("Step {}", i + 1), step)
                .with_prefix(format!("{}.", i + 1))
        })
        .collect();

    for &note in notes {
        items.push(HelpItem::new("Note", note).with_prefix("*"));
    }

    HelpPanel::new(format!("How to Use {panel_name}"))
        .collapsible(true)
        .initially_expanded(false)
        .items(items)
}
```

### Programmatic Control

```rust
// Expand/collapse
help_panel.expand();
help_panel.collapse();

// Check state
if help_panel.is_expanded() {
    tracing::info!("Panel is visible");
}

// Update title
help_panel.set_title("Updated Help");

// Add items dynamically
help_panel.add_item(HelpItem::new("Tip", "Use keyboard shortcuts for faster navigation"));

// Clear all items
help_panel.clear_items();
```

---

## Theme System

Centralized color constants and iced `Theme` customization for consistent theming across the application.

### Color Constants

```rust
// src/ui/theme.rs

use iced::Color;

/// Application theme color constants.
///
/// Use these constants throughout UI code instead of hardcoded colors
/// to ensure consistency and easy theme changes.
pub struct Theme;

impl Theme {
    // Background colors
    pub const BG_PRIMARY: Color     = Color::from_rgb(0x1a as f32 / 255.0, 0x1a as f32 / 255.0, 0x2e as f32 / 255.0);
    pub const BG_SECONDARY: Color   = Color::from_rgb(0x16 as f32 / 255.0, 0x21 as f32 / 255.0, 0x3e as f32 / 255.0);
    pub const BG_CARD: Color        = Color::from_rgb(0x1f as f32 / 255.0, 0x29 as f32 / 255.0, 0x40 as f32 / 255.0);
    pub const BG_INPUT: Color       = Color::from_rgb(0x2d as f32 / 255.0, 0x3a as f32 / 255.0, 0x5a as f32 / 255.0);

    // Text colors
    pub const TEXT_PRIMARY: Color   = Color::from_rgb(1.0, 1.0, 1.0);
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0x88 as f32 / 255.0, 0x92 as f32 / 255.0, 0xa0 as f32 / 255.0);
    pub const TEXT_DISABLED: Color  = Color::from_rgb(0x5a as f32 / 255.0, 0x62 as f32 / 255.0, 0x70 as f32 / 255.0);

    // Accent colors
    pub const ACCENT_PRIMARY: Color   = Color::from_rgb(0x4a as f32 / 255.0, 0x90 as f32 / 255.0, 0xd9 as f32 / 255.0);
    pub const ACCENT_SECONDARY: Color = Color::from_rgb(0x6c as f32 / 255.0, 0x5c as f32 / 255.0, 0xe7 as f32 / 255.0);
    pub const ACCENT_INFO: Color      = Color::from_rgb(0x17 as f32 / 255.0, 0xa2 as f32 / 255.0, 0xb8 as f32 / 255.0);

    // Status colors
    pub const STATUS_SUCCESS: Color = Color::from_rgb(0x28 as f32 / 255.0, 0xa7 as f32 / 255.0, 0x45 as f32 / 255.0);
    pub const STATUS_WARNING: Color = Color::from_rgb(0xff as f32 / 255.0, 0xc1 as f32 / 255.0, 0x07 as f32 / 255.0);
    pub const STATUS_ERROR: Color   = Color::from_rgb(0xdc as f32 / 255.0, 0x35 as f32 / 255.0, 0x45 as f32 / 255.0);

    // Button colors
    pub const BUTTON_PRIMARY: Color       = Color::from_rgb(0x4a as f32 / 255.0, 0x90 as f32 / 255.0, 0xd9 as f32 / 255.0);
    pub const BUTTON_PRIMARY_HOVER: Color = Color::from_rgb(0x3a as f32 / 255.0, 0x7f as f32 / 255.0, 0xc8 as f32 / 255.0);
    pub const BUTTON_DANGER: Color        = Color::from_rgb(0xdc as f32 / 255.0, 0x35 as f32 / 255.0, 0x45 as f32 / 255.0);
    pub const BUTTON_DANGER_HOVER: Color  = Color::from_rgb(0xc8 as f32 / 255.0, 0x23 as f32 / 255.0, 0x33 as f32 / 255.0);

    // Progress bar
    pub const PROGRESS_BG: Color   = Color::from_rgb(0x2d as f32 / 255.0, 0x3a as f32 / 255.0, 0x5a as f32 / 255.0);
    pub const PROGRESS_FILL: Color = Color::from_rgb(0x4a as f32 / 255.0, 0x90 as f32 / 255.0, 0xd9 as f32 / 255.0);

    // Border colors
    pub const BORDER_DEFAULT: Color = Color::from_rgb(0x3d as f32 / 255.0, 0x4f as f32 / 255.0, 0x6f as f32 / 255.0);
    pub const BORDER_FOCUS: Color   = Color::from_rgb(0x4a as f32 / 255.0, 0x90 as f32 / 255.0, 0xd9 as f32 / 255.0);
}
```

### Creating a Custom iced Theme

```rust
use crate::ui::theme::Theme;

impl App {
    fn theme(&self, _id: window::Id) -> iced::Theme {
        iced::Theme::custom("App Theme".into(), iced::theme::Palette {
            background: Theme::BG_PRIMARY,
            text: Theme::TEXT_PRIMARY,
            primary: Theme::ACCENT_PRIMARY,
            success: Theme::STATUS_SUCCESS,
            danger: Theme::STATUS_ERROR,
        })
    }
}
```

### Using Theme Colors in Widgets

```rust
use iced::widget::{container, row, text};
use crate::ui::theme::Theme;

fn render_status<'a>(label: &'a str, ok: bool) -> Element<'a, Message> {
    let color = if ok { Theme::STATUS_SUCCESS } else { Theme::STATUS_ERROR };
    text(label).style(color).into()
}

fn render_card<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(12)
        .style(|_theme| container::Style {
            background: Some(Theme::BG_CARD.into()),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: Theme::BORDER_DEFAULT,
            },
            ..Default::default()
        })
        .into()
}
```

---

## Creating Custom Components

### Template Structure

```rust
// src/ui/widgets/my_widget.rs

use iced::widget::{row, text, text_input};
use iced::Element;
use crate::ui::theme::Theme;

/// Messages produced by MyWidget.
#[derive(Debug, Clone)]
pub enum MyWidgetMsg {
    ValueChanged(String),
}

/// Configuration for MyWidget.
pub struct MyWidgetConfig {
    pub label: String,
    pub show_icon: bool,
}

impl Default for MyWidgetConfig {
    fn default() -> Self {
        Self {
            label: "Default".to_string(),
            show_icon: true,
        }
    }
}

/// A reusable widget that displays a labeled input field.
///
/// # Example
/// ```rust,no_run
/// let mut widget = MyWidget::new("Name");
/// // In view:
/// widget.view().map(Message::MyWidget)
/// // In update:
/// Message::MyWidget(msg) => self.widget.update(msg),
/// ```
pub struct MyWidget {
    config: MyWidgetConfig,
    value: String,
}

impl MyWidget {
    /// Create with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            config: MyWidgetConfig {
                label: label.into(),
                ..Default::default()
            },
            value: String::new(),
        }
    }

    /// Builder: set whether to show the icon.
    pub fn show_icon(mut self, show: bool) -> Self {
        self.config.show_icon = show;
        self
    }

    /// Get the current value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Set the value programmatically.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }

    /// Change the label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.config.label = label.into();
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.value.clear();
    }

    /// Render the widget as an iced Element.
    pub fn view(&self) -> Element<'_, MyWidgetMsg> {
        row![
            text(&self.config.label),
            text_input("", &self.value)
                .on_input(MyWidgetMsg::ValueChanged),
        ]
        .spacing(8)
        .into()
    }

    /// Handle a MyWidget message.
    pub fn update(&mut self, msg: MyWidgetMsg) {
        match msg {
            MyWidgetMsg::ValueChanged(val) => self.value = val,
        }
    }
}
```

### Component Checklist

When creating new components:

- [ ] Owns its own state as a struct
- [ ] `view(&self) -> Element<Msg>` for rendering
- [ ] `update(&mut self, msg)` for state changes
- [ ] Uses `Theme` constants for all colors
- [ ] Private fields; public methods for interaction
- [ ] Doc comments with examples on public items
- [ ] Provides `reset()` to return to initial state
- [ ] Immutable accessors for read-only state

---

## Integration with Panels

### Using Components in a Panel

```rust
use crate::ui::widgets::progress::ProgressBar;
use crate::ui::widgets::help_panel::{standard_help_panel, HelpPanel, HelpPanelMsg};

#[derive(Debug, Clone)]
pub enum ProcessorMsg {
    Help(HelpPanelMsg),
    // ... other messages
}

pub struct ProcessorPanel {
    progress: ProgressBar,
    help: HelpPanel,
}

impl ProcessorPanel {
    pub fn new() -> Self {
        Self {
            progress: ProgressBar::new("Progress")
                .show_count(true),
            help: standard_help_panel(
                "File Processor",
                &[
                    "Select input folder",
                    "Configure options",
                    "Click Process",
                ],
                &["Large files may take longer to process"],
            ),
        }
    }

    pub fn view(&self) -> Element<'_, ProcessorMsg> {
        column![
            // Help section at top
            self.help.view().map(ProcessorMsg::Help),

            iced::widget::horizontal_rule(1),

            // ... other widgets ...

            // Progress bar at bottom
            self.progress.view().map(|_| ProcessorMsg::ProgressTick),
        ]
        .spacing(8)
        .into()
    }

    pub fn update(&mut self, msg: ProcessorMsg) {
        match msg {
            ProcessorMsg::Help(help_msg) => self.help.update(help_msg),
            // ...
        }
    }

    pub fn process_files(&mut self, files: &[std::path::PathBuf]) {
        let total = files.len();

        for (i, _file) in files.iter().enumerate() {
            // Process file...
            self.progress.update(i + 1, total);
        }

        self.progress.set_success();
    }
}
```

---

## Tab Drag-and-Drop

> **Full guide:** [tab-drag-reorder.md](tab-drag-reorder.md) -- architecture, ghost rendering rationale, persistence patterns, and complete API reference.

A horizontal tab bar with Chrome-style drag-and-drop reordering using iced mouse events and overlay layers for the ghost tab.

### Basic Usage

```rust
use crate::ui::widgets::tab_drag::TabDragManager;

let mut tabs = TabDragManager::new()
    .drag_threshold(6.0)
    .ghost_opacity(0.8);

let mut tab_labels = vec!["General", "Audio", "Video", "Export"];
let mut selected = 0usize;

// In your view method, the tab bar produces TabDragMsg messages:
tabs.view(&tab_labels, selected).map(Message::TabDrag)

// In your update method:
Message::TabDrag(msg) => {
    if let Some(action) = tabs.update(msg, &mut tab_labels, &mut selected) {
        match action {
            TabDragAction::Reordered(new_order) => {
                tracing::info!("Tabs reordered: {:?}", new_order);
                // Persist the new order to config here
            }
        }
    }
}
```

### How It Works

1. Each tab is rendered as a styled button with press and drag detection
2. On press, the drag state records the source index and pointer offset
3. Once pointer movement exceeds the configurable `drag_threshold`, the drag becomes active
4. A ghost tab is drawn via an iced overlay layer, following the pointer
5. On each frame, hit-testing checks if the ghost crosses a neighbor's midpoint; if so, labels are swapped live
6. On pointer release, the reorder action fires (if any swap occurred)

### Configuration

```rust
let tabs = TabDragManager::new()
    .drag_threshold(10.0)           // px before drag activates (default: 6.0)
    .ghost_opacity(0.7)             // ghost alpha 0.0..=1.0 (default: 0.8)
    .active_bg(Theme::TAB_ACTIVE)   // selected tab color
    .inactive_bg(Theme::TAB_INACTIVE)
    .ghost_bg(Theme::ACCENT_PRIMARY);
```

---

## Color Picker

A color picker widget using the `iced_color_wheel` crate for an HSV color wheel, with brightness slider, hex input, contrast-aware swatch, and popup or inline modes.

### Inline Usage

```rust
use crate::ui::widgets::color_picker::ColorPicker;

let mut picker = ColorPicker::new("Accent Color")
    .initial_color(Theme::ACCENT_PRIMARY);

// In your view method:
picker.view().map(Message::ColorPicker)

// In your update method:
Message::ColorPicker(msg) => {
    if let Some(color) = picker.update(msg) {
        // Color was confirmed by the user
        config.set_accent_color(color);
    }
}
```

### Popup Usage

```rust
use crate::ui::widgets::color_picker::{PopupColorPicker, PopupResult};

let mut popup = PopupColorPicker::new("Choose Theme Color");

// Open the popup
popup.open(current_color);

// In your view, the popup renders as an overlay when open:
popup.view().map(Message::ColorPopup)

// In your update:
Message::ColorPopup(msg) => {
    match popup.update(msg) {
        PopupResult::Confirmed(color) => {
            config.set_color(color);
        }
        PopupResult::Cancelled => {
            // User cancelled -- original color preserved
        }
        PopupResult::Open => {
            // Still open, do nothing
        }
    }
}
```

### Color Wheel Features

The color picker template in `widgets/color_picker.rs` provides:

- **Circular HSV wheel** via the `iced_color_wheel` crate for intuitive hue/saturation selection
- **Brightness slider** below the wheel for value adjustment
- **Hex input field** accepting `#RRGGBB`, `RRGGBB`, `#RGB`, and `RGB` formats; invalid input is flagged and reverts on focus loss
- **Contrast-aware text** on the color swatch -- automatically chooses black or white text based on background luminance (ITU-R BT.601)
- **OK/Cancel buttons** in popup mode for explicit confirmation or dismissal

### Programmatic Control

```rust
// Get current state
let color = picker.color();
let hex = picker.hex();   // e.g. "#4A90D9"

// Set color from code
picker.set_color(iced::Color::from_rgb(1.0, 0.0, 0.0));

// Reset to white
picker.reset();
```

---

## MessageBox (rfd + iced)

Message dialog utilities with two variants: native OS dialogs via the `rfd` crate, and an iced window-based modal alternative.

### Native Dialogs (rfd)

The `rfd` crate works with any Rust GUI framework. These are blocking OS-native dialogs.

| Function | Level | Buttons | Returns |
|----------|-------|---------|---------|
| `show_info(title, msg)` | Info | OK | `()` |
| `show_warning(title, msg)` | Warning | OK | `()` |
| `show_error(title, msg)` | Error | OK | `()` |
| `ask_yes_no(title, msg)` | Info | Yes / No | `bool` |
| `ask_ok_cancel(title, msg)` | Info | OK / Cancel | `bool` |
| `ask_yes_no_with_level(title, msg, level)` | Custom | Yes / No | `bool` |

```rust
use crate::ui::widgets::messagebox::{show_info, show_error, ask_yes_no};

show_info("Welcome", "Application started successfully.");

if ask_yes_no("Confirm", "Discard unsaved changes?") {
    discard_changes();
}
```

### iced Modal Alternative

For non-blocking dialogs that live inside the iced application, use a window-based modal pattern. This opens a dialog-sized popup window managed by iced's multi-window support.

```rust
use crate::ui::widgets::messagebox::{ModalMessage, ModalButtons, ModalIcon, ModalResponse};

#[derive(Debug, Clone)]
pub enum Message {
    Modal(ModalMsg),
    // ...
}

let mut modal = ModalMessage::new("Confirm Delete")
    .message("This action cannot be undone.")
    .icon(ModalIcon::Warning)
    .buttons(ModalButtons::YesNo);

// Open the modal (requests a new window)
modal.open();

// In your view, render the modal window content when open:
modal.view().map(Message::Modal)

// In your update:
Message::Modal(msg) => {
    match modal.update(msg) {
        ModalResponse::Confirmed => { /* delete */ }
        ModalResponse::Denied => { /* keep */ }
        ModalResponse::Cancelled => { /* cancelled */ }
        ModalResponse::Open => { /* still open */ }
    }
}
```

### Convenience Constructors

```rust
use crate::ui::widgets::messagebox::{modal_info, modal_warning, modal_error, modal_confirm};

let mut m = modal_info("Done", "Export complete.");
let mut m = modal_warning("Caution", "Large file detected.");
let mut m = modal_error("Failed", "Could not write to disk.");
let mut m = modal_confirm("Delete?", "Remove selected items?");
```

---

## Title Bar (Windows DWM)

Windows title bar dark/light mode styling using the DWM API. Compile-time no-op on non-Windows platforms.

### Basic Usage

```rust
use crate::ui::title_bar::set_title_bar_style;

// Apply dark title bar to match your iced dark theme
set_title_bar_style(hwnd, true)?;

// Switch to light title bar
set_title_bar_style(hwnd, false)?;
```

### iced + winit Convenience

With iced's winit backend, you can obtain the HWND from the window handle to apply title bar styling:

```rust
use crate::ui::title_bar::set_title_bar_style;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

/// Apply dark title bar styling to an iced window.
/// Call this after the window is created (e.g., in a startup subscription).
#[cfg(target_os = "windows")]
pub fn apply_dark_title_bar(window: &impl HasRawWindowHandle) {
    if let RawWindowHandle::Win32(handle) = window.raw_window_handle() {
        let hwnd = handle.hwnd as isize;
        let _ = set_title_bar_style(hwnd, true);
    }
}
```

### How It Works

1. Calls `DwmSetWindowAttribute` with `DWMWA_USE_IMMERSIVE_DARK_MODE` (attribute 20)
2. If that fails (older Windows 10), falls back to attribute 19
3. On non-Windows platforms, the function is a no-op returning `Ok(())`
4. Uses `#[cfg(target_os = "windows")]` for zero-cost compile-time platform selection

### Platform Dependencies

```toml
# Cargo.toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_Graphics_Dwm",
]}
```

---

## See Also

- [UI-ARCHITECTURE.md](UI-ARCHITECTURE.md) - Window, panel, and dialog patterns
- [UTILITIES.md](UTILITIES.md) - Utility systems
- [tab-drag-reorder.md](tab-drag-reorder.md) - Tab drag-and-drop deep-dive guide
- [templates/](templates/) - Rust template source files
- [templates/tab_drag.rs](templates/tab_drag.rs) - Tab drag-and-drop source
- [templates/color_picker.rs](templates/color_picker.rs) - Color picker source
- [templates/messagebox.rs](templates/messagebox.rs) - Message dialog source
- [templates/title_bar.rs](templates/title_bar.rs) - Title bar styling source
