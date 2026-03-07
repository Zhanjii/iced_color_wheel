//! Basic example: a color wheel with a brightness slider and hex display.

use iced::widget::{canvas, column, container, slider, text};
use iced::{Center, Color, Element, Fill, Task, Theme};
use iced_color_wheel::{hsv_to_color, hsv_to_hex, WheelProgram};

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title("Color Wheel Example")
        .theme(App::theme)
        .window_size((400.0, 500.0))
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    HueSatChanged(f32, f32),
    ValueChanged(f32),
}

struct App {
    hue: f32,
    saturation: f32,
    value: f32,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        (
            Self {
                hue: 0.0,
                saturation: 1.0,
                value: 1.0,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::HueSatChanged(h, s) => {
                self.hue = h;
                self.saturation = s;
            }
            Message::ValueChanged(v) => {
                self.value = v;
            }
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn view(&self) -> Element<'_, Message> {
        let wheel = canvas(WheelProgram::new(
            self.hue,
            self.saturation,
            self.value,
            Message::HueSatChanged,
        ))
        .width(250)
        .height(250);

        let brightness = slider(0.0..=1.0, self.value, Message::ValueChanged).step(0.005);

        let color = hsv_to_color(self.hue, self.saturation, self.value);
        let hex = hsv_to_hex(self.hue, self.saturation, self.value);

        let lum = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
        let text_color = if lum > 0.5 { Color::BLACK } else { Color::WHITE };

        let hex_display = container(
            text(hex).size(20).color(text_color),
        )
        .width(250)
        .padding(12)
        .style(move |_theme: &Theme| container::Style {
            background: Some(color.into()),
            border: iced::Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .center_x(Fill);

        let content = column![wheel, brightness, hex_display]
            .spacing(16)
            .align_x(Center)
            .padding(20);

        container(content)
            .center_x(Fill)
            .center_y(Fill)
            .into()
    }
}
