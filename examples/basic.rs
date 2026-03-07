//! Basic example: a color wheel with a brightness slider, hex bar, and OK button.
//! Styled to match a polished color picker popup.

use iced::widget::{button, canvas, column, container, slider, text, text_input, Space};
use iced::{Background, Border, Color, Element, Length, Task, Theme};
use iced_color_wheel::{hsv_to_color, hsv_to_hex, WheelProgram};

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title("Color Wheel Example")
        .theme(App::theme)
        .window_size((340.0, 460.0))
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    HueSatChanged(f32, f32),
    ValueChanged(f32),
    HexChanged(String),
    Submit,
}

struct App {
    hue: f32,
    saturation: f32,
    value: f32,
    hex: String,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        (
            Self {
                hue: 0.0,
                saturation: 1.0,
                value: 1.0,
                hex: "#FF0000".into(),
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::HueSatChanged(h, s) => {
                self.hue = h;
                self.saturation = s;
                self.hex = hsv_to_hex(self.hue, self.saturation, self.value);
            }
            Message::ValueChanged(v) => {
                self.value = v;
                self.hex = hsv_to_hex(self.hue, self.saturation, self.value);
            }
            Message::HexChanged(h) => {
                self.hex = h;
            }
            Message::Submit => {}
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn view(&self) -> Element<'_, Message> {
        let wheel_size = 250;

        // Large centered color wheel
        let wheel = container(
            canvas(WheelProgram::new(
                self.hue,
                self.saturation,
                self.value,
                Message::HueSatChanged,
            ))
            .width(wheel_size)
            .height(wheel_size),
        )
        .width(Length::Fill)
        .center_x(Length::Fill);

        // Brightness slider — colored rail + pill handle
        let current_color = hsv_to_color(self.hue, self.saturation, self.value);
        let handle_color = current_color;
        let unfilled_color = Color::from_rgb(0.2, 0.2, 0.2);

        let brightness_slider =
            slider(0.0..=1.0, self.value, Message::ValueChanged)
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

        // Hex bar — fully colored background with auto-contrast text
        let preview_color = current_color;
        let luminance =
            0.299 * preview_color.r + 0.587 * preview_color.g + 0.114 * preview_color.b;
        let text_color = if luminance > 0.5 {
            Color::BLACK
        } else {
            Color::WHITE
        };

        let color_preview = text_input("#RRGGBB", &self.hex)
            .on_input(Message::HexChanged)
            .width(Length::Fill)
            .size(18)
            .style(move |_theme: &Theme, _status| {
                iced::widget::text_input::Style {
                    background: Background::Color(preview_color),
                    border: Border {
                        color: Color::from_rgb(0.3, 0.3, 0.3),
                        width: 1.0,
                        radius: 20.0.into(),
                    },
                    icon: text_color,
                    placeholder: Color { a: 0.5, ..text_color },
                    value: text_color,
                    selection: Color::from_rgba(0.5, 0.5, 0.5, 0.3),
                }
            })
            .padding([12, 16]);

        // Full-width rounded OK button
        let ok_button = button(
            container(text("OK").size(15))
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .on_press(Message::Submit)
        .width(Length::Fill)
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

        container(
            column![
                Space::new().height(8),
                wheel,
                Space::new().height(14),
                brightness_slider,
                Space::new().height(12),
                color_preview,
                Space::new().height(12),
                ok_button,
            ]
            .spacing(0)
            .padding([12, 24])
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
