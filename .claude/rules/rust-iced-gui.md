---
description: Iced 0.14 daemon GUI stack — entry point, multi-window patterns, real dependency block, iced gotchas, theme/style modules
paths: ['**/src/ui/**', '**/src/*ui*.rs', '**/app.rs', '**/main.rs']
---

# GUI Stack: Iced Daemon

All desktop apps use **iced v0.14** with the **daemon** entry point for multi-window support. Never introduce Tauri, egui, eframe, or web frontend dependencies — all UI is pure Rust via iced widgets and canvas.

## Key Patterns

- **Entry point**: `iced::daemon(boot, update, view).title(title).theme(theme_fn).subscription(subscription).settings(settings).run()` — `boot` is `fn() -> (App, Task<Message>)`; daemon owns no implicit window, so `boot` opens the main window itself via `window::open()`
- **Multi-window**: `HashMap<window::Id, WindowKind>` registry; dispatch `view()` by window ID
- **Popups**: `window::open(window::Settings { ... })` for settings, about, color picker, etc.
- **Close handling**: `window::close_events().map(Message::WindowClosed)` subscription
- **Daemon mode**: the app does NOT exit when the last window closes (pairs with system tray)
- **Color picker**: `iced_color_wheel` crate for the HSV color wheel widget

### Why Daemon (Not Application)

- `iced::application` exits when its single window closes
- `iced::daemon` keeps running — required for tray-resident apps and popup workflows
- Each window gets independent state and view dispatch

## Dependencies

```toml
# "advanced" is required by iced_draggable_tabs; there is no "multi-window"
# feature flag — daemon mode is multi-window natively in 0.14.
iced = { version = "=0.14", features = ["advanced", "tokio"] }
# Local path dep — patched 0.1.2 adds .equal_widths(); switch back to a
# crates.io version once the patch is published upstream. Path resolves to
# H:/BOOK.IO/02_ELEMENTS/10_SCRIPTS/iced_draggable_tabs — adjust depth per project.
iced_draggable_tabs = { path = "../../iced_draggable_tabs" }
iced_message_box = "=0.1.1"  # Themed message dialogs (NOT rfd — rfd is not in this stack)
iced_color_wheel = "=0.1.1"  # HSV color wheel widget
tray-icon = "0.19"           # System tray (optional)
```

## Iced Gotchas

- **`Task::perform` for async** — never `.await` inside `update()`, and never the pre-0.13 `Command` API. `Command` was renamed `Task` in iced 0.13 — return `Task::perform(async_fn, Message::Variant)` from `update()`.
- **`view()` returns a lifetime-bound `Element<'_, Message>`** — views are rebuilt on every update; never store or cache rendered elements.
- **No widget IDs** — iced is not the DOM. State is owned by the application struct, not the widgets.
- **Release console output vanishes** — `#![windows_subsystem = "windows"]` hides the console in release builds; use `tracing` macros for all operational output, never `println!`/`eprintln!`, or it is silently lost.

## Theme & Style Modules

- Keep a dedicated `src/ui/theme.rs` (custom iced theme + palette/surface tones) and `src/ui/style.rs` (reusable widget-style closures: buttons, cards, scrollbars, text inputs) — views compose these; never inline ad-hoc colors in `view()`.
- Full patterns read on demand from the templates repo: `RUST/docs/UI-ARCHITECTURE.md` (window lifecycle, panel pattern, UI lock, dialogs) and `RUST/docs/UI-COMPONENTS.md` (theme system, custom components, tab drag-and-drop).

## Visual Debugging via Screenshots

iced layout bugs are faster to show than describe — drop a screenshot of the misbehaving window into the prompt; Claude reads it directly. Use for overlapping widgets, padding asymmetry, scrollables that won't scroll, design-mock comparison, and before/after layout refactors. Be specific — bad: "fix this"; good: "the right column is cut off — is it `width(Fill)` vs `width(Shrink)` on the inner `Column`?". For new views, work mock-driven: screenshot first, then scaffold the `view()` function (widget hierarchy, `spacing`, `padding`, `align_x/align_y`) from it. Keep captures in `screenshots/` at repo root, gitignored.

## Reference Documentation

- iced repo + examples: https://github.com/iced-rs/iced · docs: https://docs.rs/iced/latest/iced/
- daemon entry point: https://docs.rs/iced/latest/iced/fn.daemon.html · window module: https://docs.rs/iced/latest/iced/window/index.html
- widgets: https://docs.rs/iced/latest/iced/widget/index.html · canvas: https://docs.rs/iced/latest/iced/widget/canvas/index.html
- Subscription (events, timers, channels): https://docs.rs/iced/latest/iced/struct.Subscription.html
- iced_color_wheel: https://github.com/zhanjii/iced_color_wheel · iced_message_box: https://docs.rs/iced_message_box/latest/iced_message_box/
- tray-icon: https://docs.rs/tray-icon/latest/tray_icon/ · Q&A/patterns: https://github.com/iced-rs/iced/discussions
