//! Color picker popup template for iced daemon applications.
//!
//! Integrates the `iced_color_wheel` crate into a complete color picker
//! popup window with:
//! - Circular HSV color wheel (hue/saturation via click & drag)
//! - Brightness (value) slider with colored rail
//! - Hex input bar with contrast-aware text
//! - OK / Cancel buttons
//! - Popup window lifecycle via iced daemon `window::open()` / `window::close()`
//!
//! This is the Rust equivalent of the Python `CTkColorPicker` wrapper
//! in `docs/templates/color_picker.py`.
//!
//! # Popup Usage (iced daemon)
//!
//! ```rust
//! // In your app's WindowKind enum:
//! enum WindowKind {
//!     Main(MainState),
//!     ColorPicker(ColorPickerState),
//!     // ...
//! }
//!
//! // Opening the popup:
//! Message::OpenColorPicker => {
//!     let (id, task) = open_color_picker(Some(current_accent_color));
//!     self.windows.insert(id, WindowKind::ColorPicker(
//!         ColorPickerState::new(current_accent_color),
//!     ));
//!     task
//! }
//!
//! // In your view() dispatch:
//! Some(WindowKind::ColorPicker(state)) => state.view(id).map(Message::ColorPicker),
//!
//! // Handling the result:
//! Message::ColorPicker(ColorPickerMsg::Confirmed(color, id)) => {
//!     self.accent_color = color;
//!     self.windows.remove(&id);
//!     window::close(id)
//! }
//! Message::ColorPicker(ColorPickerMsg::Cancelled(id)) => {
//!     self.windows.remove(&id);
//!     window::close(id)
//! }
//! ```
//!
//! # Inline Usage
//!
//! ```rust
//! // Embed the picker directly in a panel (no popup window):
//! let picker_view = self.picker_state.view_inline().map(Message::ColorPicker);
//! ```

use iced::widget::{button, canvas, column, container, row, slider, text, text_input, Space};
use iced::window;
use iced::{Background, Border, Color, Element, Length, Size, Task, Theme};

use iced_color_wheel::{color_to_hsv, hsv_to_color, hsv_to_hex, hex_to_color, WheelProgram};

// =============================================================================
// COLOR PICKER STATE
// =============================================================================

/// State for a color picker popup window.
pub struct ColorPickerState {
    /// Current hue (0-360 degrees).
    pub hue: f32,
    /// Current saturation (0.0-1.0).
    pub saturation: f32,
    /// Current brightness/value (0.0-1.0).
    pub value: f32,
    /// Hex input field contents.
    pub hex: String,
    /// The color at the time the picker was opened (for Cancel).
    original_color: Color,
}

impl ColorPickerState {
    /// Create a new picker state from an initial color.
    pub fn new(initial: Color) -> Self {
        let (h, s, v) = color_to_hsv(initial);
        Self {
            hue: h,
            saturation: s,
            value: v,
            hex: hsv_to_hex(h, s, v),
            original_color: initial,
        }
    }

    /// Get the current color.
    pub fn color(&self) -> Color {
        hsv_to_color(self.hue, self.saturation, self.value)
    }

    /// Get the hex string.
    pub fn hex(&self) -> &str {
        &self.hex
    }
}

// =============================================================================
// MESSAGES
// =============================================================================

/// Messages emitted by the color picker popup.
#[derive(Debug, Clone)]
pub enum ColorPickerMsg {
    /// Hue and saturation changed (from wheel interaction).
    HueSatChanged(f32, f32),
    /// Brightness/value changed (from slider).
    ValueChanged(f32),
    /// Hex input field changed.
    HexChanged(String),
    /// User confirmed the color — includes the window ID for closing.
    Confirmed(Color, window::Id),
    /// User cancelled — includes the window ID for closing.
    Cancelled(window::Id),
}

// =============================================================================
// UPDATE
// =============================================================================

impl ColorPickerState {
    /// Handle a color picker message.
    ///
    /// Returns `Some((color, window_id))` when the user confirms or cancels,
    /// so the caller can close the window.
    pub fn update(&mut self, msg: ColorPickerMsg) -> Option<(Color, window::Id)> {
        match msg {
            ColorPickerMsg::HueSatChanged(h, s) => {
                self.hue = h;
                self.saturation = s;
                self.hex = hsv_to_hex(self.hue, self.saturation, self.value);
                None
            }
            ColorPickerMsg::ValueChanged(v) => {
                self.value = v;
                self.hex = hsv_to_hex(self.hue, self.saturation, self.value);
                None
            }
            ColorPickerMsg::HexChanged(input) => {
                self.hex = input;
                if let Some(color) = hex_to_color(&self.hex) {
                    let (h, s, v) = color_to_hsv(color);
                    self.hue = h;
                    self.saturation = s;
                    self.value = v;
                }
                None
            }
            ColorPickerMsg::Confirmed(color, id) => Some((color, id)),
            ColorPickerMsg::Cancelled(id) => Some((self.original_color, id)),
        }
    }
}

// =============================================================================
// VIEW
// =============================================================================

impl ColorPickerState {
    /// Render the full color picker popup (wheel + slider + hex + buttons).
    ///
    /// `window_id` is passed through to the Confirmed/Cancelled messages
    /// so the parent app can close this window.
    pub fn view(&self, window_id: window::Id) -> Element<'_, ColorPickerMsg> {
        let wheel_size = 250;

        // Color wheel canvas
        let wheel = container(
            canvas(WheelProgram::new(
                self.hue,
                self.saturation,
                self.value,
                ColorPickerMsg::HueSatChanged,
            ))
            .width(wheel_size)
            .height(wheel_size),
        )
        .width(Length::Fill)
        .center_x(Length::Fill);

        // Brightness slider with colored rail
        let current_color = self.color();
        let handle_color = current_color;
        let unfilled_color = Color::from_rgb(0.2, 0.2, 0.2);

        let brightness_slider = slider(0.0..=1.0, self.value, ColorPickerMsg::ValueChanged)
            .step(0.005)
            .width(Length::Fill)
            .style(move |_theme: &Theme, _status| {
                use iced::widget::slider::{Handle, HandleShape, Rail, Style};
                Style {
                    rail: Rail {
                        backgrounds: (
                            Background::Color(current_color),
                            Background::Color(unfilled_color),
                        ),
                        width: 10.0,
                        border: Border {
                            radius: 5.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                    },
                    handle: Handle {
                        shape: HandleShape::Rectangle {
                            width: 24,
                            border_radius: 12.0.into(),
                        },
                        background: Background::Color(handle_color),
                        border_width: 2.0,
                        border_color: Color::WHITE,
                    },
                }
            });

        // Hex input bar with contrast-aware text
        let preview_color = current_color;
        let luminance =
            0.299 * preview_color.r + 0.587 * preview_color.g + 0.114 * preview_color.b;
        let text_color = if luminance > 0.5 {
            Color::BLACK
        } else {
            Color::WHITE
        };

        let hex_input = text_input("#RRGGBB", &self.hex)
            .on_input(ColorPickerMsg::HexChanged)
            .width(Length::Fill)
            .size(18)
            .style(move |_theme: &Theme, _status| iced::widget::text_input::Style {
                background: Background::Color(preview_color),
                border: Border {
                    color: Color::from_rgb(0.3, 0.3, 0.3),
                    width: 1.0,
                    radius: 20.0.into(),
                },
                icon: text_color,
                placeholder: Color {
                    a: 0.5,
                    ..text_color
                },
                value: text_color,
                selection: Color::from_rgba(0.5, 0.5, 0.5, 0.3),
            })
            .padding([12, 16]);

        // OK / Cancel buttons
        let confirmed_color = self.color();
        let ok_button = button(
            container(text("OK").size(15))
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .on_press(ColorPickerMsg::Confirmed(confirmed_color, window_id))
        .width(Length::FillPortion(1))
        .padding([10, 0])
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            let base_bg = palette.primary.base.color;
            let bg = match status {
                iced::widget::button::Status::Hovered => Color {
                    r: (base_bg.r + 0.1).min(1.0),
                    g: (base_bg.g + 0.1).min(1.0),
                    b: (base_bg.b + 0.1).min(1.0),
                    a: base_bg.a,
                },
                _ => base_bg,
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                text_color: palette.primary.base.text,
                border: Border {
                    radius: 20.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: iced::Shadow::default(),
                snap: false,
            }
        });

        let cancel_button = button(
            container(text("Cancel").size(15))
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .on_press(ColorPickerMsg::Cancelled(window_id))
        .width(Length::FillPortion(1))
        .padding([10, 0])
        .style(|theme: &Theme, status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Color::from_rgb(0.35, 0.35, 0.35),
                _ => Color::from_rgb(0.25, 0.25, 0.25),
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                text_color: Color::WHITE,
                border: Border {
                    radius: 20.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: iced::Shadow::default(),
                snap: false,
            }
        });

        let button_row = row![ok_button, cancel_button].spacing(8);

        container(
            column![
                Space::new().height(8),
                wheel,
                Space::new().height(14),
                brightness_slider,
                Space::new().height(12),
                hex_input,
                Space::new().height(12),
                button_row,
            ]
            .spacing(0)
            .padding([12, 24])
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Render the picker inline (no OK/Cancel buttons, no window ID).
    ///
    /// Useful when embedding the picker directly in a settings panel
    /// rather than as a popup window.
    pub fn view_inline(&self) -> Element<'_, ColorPickerMsg> {
        let wheel_size = 200;

        let wheel = container(
            canvas(WheelProgram::new(
                self.hue,
                self.saturation,
                self.value,
                ColorPickerMsg::HueSatChanged,
            ))
            .width(wheel_size)
            .height(wheel_size),
        )
        .width(Length::Fill)
        .center_x(Length::Fill);

        let current_color = self.color();
        let unfilled_color = Color::from_rgb(0.2, 0.2, 0.2);

        let brightness_slider = slider(0.0..=1.0, self.value, ColorPickerMsg::ValueChanged)
            .step(0.005)
            .width(Length::Fill)
            .style(move |_theme: &Theme, _status| {
                use iced::widget::slider::{Handle, HandleShape, Rail, Style};
                Style {
                    rail: Rail {
                        backgrounds: (
                            Background::Color(current_color),
                            Background::Color(unfilled_color),
                        ),
                        width: 8.0,
                        border: Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                    },
                    handle: Handle {
                        shape: HandleShape::Rectangle {
                            width: 20,
                            border_radius: 10.0.into(),
                        },
                        background: Background::Color(current_color),
                        border_width: 2.0,
                        border_color: Color::WHITE,
                    },
                }
            });

        let hex_label = text(&self.hex).size(14);

        column![wheel, Space::new().height(8), brightness_slider, hex_label,]
            .spacing(4)
            .padding(8)
            .into()
    }
}

// =============================================================================
// POPUP WINDOW HELPERS
// =============================================================================

/// Open a color picker as a popup window.
///
/// Returns `(window_id, task)`. The caller must:
/// 1. Insert the window ID into the app's window registry with a `ColorPickerState`
/// 2. Return the task from `update()`
///
/// # Example
///
/// ```rust
/// let (id, task) = open_color_picker(Some(self.accent_color));
/// self.windows.insert(id, WindowKind::ColorPicker(
///     ColorPickerState::new(self.accent_color),
/// ));
/// task
/// ```
pub fn open_color_picker<M: 'static>(initial_color: Option<Color>) -> (window::Id, Task<M>) {
    let (id, task) = window::open(window::Settings {
        size: Size::new(340.0, 460.0),
        min_size: Some(Size::new(340.0, 460.0)),
        resizable: false,
        position: window::Position::Centered,
        exit_on_close_request: false,
        ..window::Settings::default()
    });
    (id, task.discard())
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
//
// [dependencies]
// iced = { version = "0.14", features = ["canvas", "multi-window"] }
// iced_color_wheel = "0.1"
// # or path dependency:
// # iced_color_wheel = { path = "../iced_color_wheel" }

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_picker_state_new() {
        let color = Color::from_rgb8(0x4A, 0x90, 0xD9);
        let state = ColorPickerState::new(color);

        // HSV should roundtrip approximately
        let restored = state.color();
        let diff_r = (color.r - restored.r).abs();
        let diff_g = (color.g - restored.g).abs();
        let diff_b = (color.b - restored.b).abs();
        assert!(diff_r < 0.02 && diff_g < 0.02 && diff_b < 0.02);
    }

    #[test]
    fn test_picker_hex() {
        let state = ColorPickerState::new(Color::from_rgb8(0xFF, 0x00, 0x00));
        assert_eq!(state.hex(), "#FF0000");
    }

    #[test]
    fn test_picker_update_hue_sat() {
        let mut state = ColorPickerState::new(Color::WHITE);
        let result = state.update(ColorPickerMsg::HueSatChanged(120.0, 0.8));
        assert!(result.is_none()); // No confirm/cancel
        assert!((state.hue - 120.0).abs() < 0.01);
        assert!((state.saturation - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_picker_update_value() {
        let mut state = ColorPickerState::new(Color::WHITE);
        state.update(ColorPickerMsg::ValueChanged(0.5));
        assert!((state.value - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_picker_update_hex() {
        let mut state = ColorPickerState::new(Color::WHITE);
        state.update(ColorPickerMsg::HexChanged("#00FF00".into()));
        // Should parse and update HSV
        assert!((state.hue - 120.0).abs() < 1.0);
        assert!((state.saturation - 1.0).abs() < 0.02);
    }

    #[test]
    fn test_picker_confirm() {
        let mut state = ColorPickerState::new(Color::WHITE);
        state.update(ColorPickerMsg::HueSatChanged(0.0, 1.0));
        state.update(ColorPickerMsg::ValueChanged(1.0));

        let id = window::Id::unique();
        let result = state.update(ColorPickerMsg::Confirmed(state.color(), id));
        assert!(result.is_some());
    }

    #[test]
    fn test_picker_cancel_restores_original() {
        let original = Color::from_rgb8(0x4A, 0x90, 0xD9);
        let mut state = ColorPickerState::new(original);

        // Change the color
        state.update(ColorPickerMsg::HueSatChanged(0.0, 1.0));

        // Cancel should return the original
        let id = window::Id::unique();
        let result = state.update(ColorPickerMsg::Cancelled(id));
        if let Some((color, _)) = result {
            assert!((color.r - original.r).abs() < 0.01);
            assert!((color.g - original.g).abs() < 0.01);
            assert!((color.b - original.b).abs() < 0.01);
        }
    }
}
