# Tab Drag-and-Drop Reordering for iced

A reusable widget for adding tab reordering to any iced application.
Create a `TabBar`, call `view()` in your view function, handle messages in `update()`, and you are done.

---

## Table of Contents

- [Overview](#overview)
- [How to Use](#how-to-use)
- [Architecture](#architecture)
- [Why Canvas for the Ghost](#why-canvas-for-the-ghost)
- [Tuning](#tuning)
- [Persisting Tab Order](#persisting-tab-order)
- [Builder Pattern](#builder-pattern)
- [Full API Reference](#full-api-reference)

---

## Overview

`TabBar` renders a horizontal tab bar with reordering support. The simplified version
uses move-button arrows for reordering; a full canvas-based drag-and-drop with ghost
rendering is also possible (see [Why Canvas for the Ghost](#why-canvas-for-the-ghost)).

In the button-based reorder mode:

1. Each tab is an iced button widget. Clicking a tab emits `TabBarMsg::TabClicked(usize)`.
2. Move buttons (left/right arrows) appear next to the selected tab, allowing the user
   to shift it in either direction.
3. When a tab is moved, `TabBarMsg::TabMoved(from, to)` fires, the labels swap, and
   the `TabBarAction::Reordered` action is returned from `update()`.
4. Clicking a tab returns `TabBarAction::Selected(idx)` so the parent can switch content.

Normal clicks work as expected. No drag threshold is needed in the button-based mode
since reordering is explicit via arrow buttons.

---

## How to Use

### Step 1: Add the dependency

```toml
# Cargo.toml
[dependencies]
iced = { version = "0.14" }
```

### Step 2: Copy the template

Copy `tab_drag.rs` from [templates/tab_drag.rs](templates/tab_drag.rs) into your
project (e.g., `src/ui/widgets/tab_drag.rs`). Add it to your module tree:

```rust
// src/ui/widgets/mod.rs
pub mod tab_drag;
```

### Step 3: Create the TabBar and wire up view/update

```rust
use crate::ui::widgets::tab_drag::{TabBar, TabBarMsg, TabBarAction};

pub struct MyApp {
    tabs: TabBar,
    tab_labels: Vec<String>,
    selected_tab: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabBar(TabBarMsg),
}

impl MyApp {
    pub fn new() -> Self {
        Self {
            tabs: TabBar::new(),
            tab_labels: vec![
                "General".into(),
                "Audio".into(),
                "Video".into(),
                "Export".into(),
            ],
            selected_tab: 0,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::TabBar(msg) => {
                let action = self.tabs.update(
                    msg,
                    &mut self.tab_labels,
                    &mut self.selected_tab,
                );
                match action {
                    Some(TabBarAction::Selected(idx)) => {
                        tracing::info!("Tab selected: {}", idx);
                    }
                    Some(TabBarAction::Reordered) => {
                        tracing::info!("Tabs reordered: {:?}", self.tab_labels);
                    }
                    None => {}
                }
            }
        }
    }

    pub fn view(&self) -> iced::Element<Message> {
        let tab_bar = self.tabs
            .view_with_move_buttons(&self.tab_labels, self.selected_tab)
            .map(Message::TabBar);

        let content = iced::widget::column![
            tab_bar,
            iced::widget::horizontal_rule(1),
            iced::widget::text(format!(
                "Content for: {}",
                self.tab_labels[self.selected_tab]
            )),
        ];

        content.into()
    }
}
```

That is all there is to it. No trait implementations, no custom widget registration,
no separate render pass.

---

## Architecture

### Struct layout

`TabBar` owns two things:

| Field | Type | Purpose |
|-------|------|---------|
| `config` | `TabBarConfig` | All tunable appearance and behavior values |
| `drag` | `DragState` | Mutable drag-in-progress state, reset on drop |

### DragState

```rust
struct DragState {
    dragging: Option<usize>,  // Index of tab being dragged (canvas mode)
    moved: bool,              // True if at least one swap occurred
}
```

All fields default to their zero/false/None values. The entire struct resets to
`DragState::default()` when a reorder sequence completes.

### Message and Action flow

```
view() / view_with_move_buttons() called each frame
    |
    +-- Render each tab as an iced button widget
    |   - On press: emit TabBarMsg::TabClicked(idx)
    |   - Move arrows (if view_with_move_buttons): emit TabBarMsg::TabMoved(from, to)
    |
    +-- Parent maps messages: .map(Message::TabBar)

update(msg, labels, selected) called when a TabBarMsg arrives
    |
    +-- TabBarMsg::TabClicked(idx):
    |   - Set *selected = idx
    |   - Return Some(TabBarAction::Selected(idx))
    |
    +-- TabBarMsg::TabMoved(from, to):
    |   - labels.swap(from, to)
    |   - Update *selected if it was affected
    |   - Return Some(TabBarAction::Reordered)
    |
    +-- Return None if no meaningful action
```

### View modes

- **`view()`** -- Click-only tab bar. Tabs are buttons, clicking selects. No reorder controls.
- **`view_with_move_buttons()`** -- Same as `view()` but adds left/right arrow buttons next to the selected tab for explicit reordering.

### Cleanup

Drag state resets atomically via `self.drag = DragState::default()`. There is no
multi-step cleanup, no deferred callbacks, and no way for state to leak between frames.

---

## Why Canvas for the Ghost

The simplified `TabBar` template uses button-based reordering (arrow buttons) which
requires no overlay rendering. However, a full Chrome-style drag-and-drop ghost tab
is possible using `iced::widget::canvas`, similar to how `iced_color_wheel` implements
its HSV wheel. Three reasons canvas works well for this:

1. **Floats above all widgets.** An iced canvas overlay can be layered on top of
   other content using `iced::widget::stack`. The ghost always appears above the tab
   bar, panels, and any overlapping content. No z-fighting, no clipping by parent
   containers.

2. **Does not affect layout.** A canvas overlay occupies its own layer in the stack
   and does not shift neighboring widgets. The tab bar layout is unchanged whether or
   not a ghost is visible.

3. **Can follow the pointer freely.** By tracking cursor position via canvas events,
   the ghost can be drawn at an arbitrary position each frame, giving smooth horizontal
   tracking without vertical jitter.

If you need full drag-and-drop ghost rendering, extend the template with an
`iced::widget::canvas` layer that draws the ghost tab during an active drag. The
`iced_color_wheel` crate (used in this stack for color pickers) demonstrates the
canvas event and draw patterns needed.

---

## Tuning

All tunable values live in `TabBarConfig`:

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `active_bg` | `Color` | `#4A90D9` | Background color for the selected tab. |
| `inactive_bg` | `Color` | `#2D3A5A` | Background color for unselected tabs. |
| `text_color` | `Color` | `WHITE` | Label text color for all tabs. |
| `dimmed_text_color` | `Color` | `#727272` | Text color for the source tab while dragging (canvas mode). Signals "this is the one being moved." |
| `rounding` | `f32` | `6.0` | Corner radius for tab buttons. |
| `tab_padding` | `Padding` | `(16.0, 6.0)` | Inner margin of each tab button (horizontal, vertical). |
| `spacing` | `f32` | `4.0` | Horizontal gap between tabs. |

To integrate with a shared `Theme` system, set colors at construction:

```rust
let tabs = TabBar::new()
    .active_bg(Theme::ACCENT_PRIMARY)
    .inactive_bg(Theme::BG_INPUT);
```

---

## Persisting Tab Order

To save and restore the user's tab order across app restarts, use `serde_json` with
a config file.

### Saving

In your `update()` handler when `TabBarAction::Reordered` is returned, serialize the label order:

```rust
use std::path::PathBuf;

fn save_tab_order(labels: &[String], config_path: &PathBuf) {
    let prefs: serde_json::Value = if config_path.exists() {
        std::fs::read_to_string(config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let mut prefs = prefs.as_object().cloned().unwrap_or_default();
    prefs.insert(
        "tab_order".to_string(),
        serde_json::json!(labels),
    );

    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(config_path, serde_json::to_string_pretty(&prefs).unwrap());
}
```

### Loading (before first `view()` call)

```rust
fn apply_saved_tab_order(labels: &mut Vec<String>, config_path: &PathBuf) {
    let Ok(contents) = std::fs::read_to_string(config_path) else { return };
    let Ok(prefs) = serde_json::from_str::<serde_json::Value>(&contents) else { return };

    let Some(saved) = prefs.get("tab_order").and_then(|v| v.as_array()) else { return };
    let saved_order: Vec<String> = saved
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    // Only apply if the saved names match current tabs exactly
    let mut current_set: std::collections::HashSet<&str> =
        labels.iter().map(|s| s.as_str()).collect();
    let saved_set: std::collections::HashSet<&str> =
        saved_order.iter().map(|s| s.as_str()).collect();

    if current_set != saved_set {
        return; // Tabs changed -- ignore stale order
    }

    *labels = saved_order;
}
```

### Wiring it up

```rust
let config_path = dirs::config_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("my_app")
    .join("prefs.json");

// At startup:
apply_saved_tab_order(&mut app.tab_labels, &config_path);

// In update(), after matching TabBarAction::Reordered:
save_tab_order(&app.tab_labels, &config_path);
```

The saved order is validated against current tab names so that adding or removing
tabs does not cause panics -- it simply resets to the default order.

---

## Builder Pattern

`TabBar` uses consuming builder methods (each takes `mut self` and returns
`Self`). Chain them at construction:

```rust
let tabs = TabBar::new()
    .active_bg(iced::Color::from_rgb8(0x4a, 0x90, 0xd9))
    .inactive_bg(iced::Color::from_rgb8(0x2d, 0x3a, 0x5a));
```

Available builder methods:

| Method | Parameter | Effect |
|--------|-----------|--------|
| `active_bg(color)` | `Color` | Set selected tab background |
| `inactive_bg(color)` | `Color` | Set unselected tab background |

After construction, configuration is only accessible through `self.config` (not public).
If you need runtime color changes, you would add `set_*(&mut self)` methods.

---

## Full API Reference

### `TabBarConfig`

```rust
pub struct TabBarConfig {
    pub active_bg: iced::Color,
    pub inactive_bg: iced::Color,
    pub text_color: iced::Color,
    pub dimmed_text_color: iced::Color,
    pub rounding: f32,
    pub tab_padding: iced::Padding,
    pub spacing: f32,
}
```

Implements `Default`. See [Tuning](#tuning) for default values.

### `DragState`

```rust
struct DragState {
    dragging: Option<usize>,
    moved: bool,
}
```

Private. Implements `Default` (derive). Not accessible outside the module.

### `TabBarMsg`

```rust
#[derive(Debug, Clone)]
pub enum TabBarMsg {
    TabClicked(usize),
    TabMoved(usize, usize),  // (from_index, to_index)
}
```

Messages emitted by `view()` and `view_with_move_buttons()`. The parent maps these
via `.map(Message::TabBar)`.

### `TabBarAction`

```rust
#[derive(Debug, Clone)]
pub enum TabBarAction {
    Selected(usize),
    Reordered,
}
```

Returned from `update()` to tell the parent what happened.

### `TabBar`

```rust
pub struct TabBar {
    config: TabBarConfig,
    drag: DragState,
}
```

Implements `Default` (delegates to `Self::new()`).

#### Methods

| Signature | Description |
|-----------|-------------|
| `pub fn new() -> Self` | Create with default config and no active drag. |
| `pub fn active_bg(self, color: Color) -> Self` | Builder: set active tab background. |
| `pub fn inactive_bg(self, color: Color) -> Self` | Builder: set inactive tab background. |
| `pub fn update<S: AsRef<str>>(&mut self, msg: TabBarMsg, labels: &mut Vec<S>, selected: &mut usize) -> Option<TabBarAction>` | Handle a tab bar message. Mutates `labels` and `selected` in place. Returns the resulting action, if any. |
| `pub fn view<S: AsRef<str>>(&self, labels: &[S], selected: usize) -> iced::Element<TabBarMsg>` | Render a click-only tab bar (no reorder controls). |
| `pub fn view_with_move_buttons<S: AsRef<str>>(&self, labels: &[S], selected: usize) -> iced::Element<TabBarMsg>` | Render a tab bar with left/right arrow buttons for reordering. |

### Dependency

```toml
[dependencies]
iced = { version = "0.14" }
```

---

## See Also

- [UI-COMPONENTS.md](UI-COMPONENTS.md) - Overview of all reusable UI components
- [UI-ARCHITECTURE.md](UI-ARCHITECTURE.md) - Window, panel, and dialog patterns
- [templates/tab_drag.rs](templates/tab_drag.rs) - Full template source
