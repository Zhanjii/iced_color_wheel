//! A circular HSV color wheel widget for Iced.
//!
//! Renders a hue/saturation wheel (angle = hue, distance = saturation) with
//! click-and-drag interaction. Pair with a brightness slider for full HSV control.
//!
//! # Usage
//!
//! ```rust,no_run
//! use iced::widget::canvas;
//! use iced_color_wheel::WheelProgram;
//!
//! // In your view function:
//! let wheel = canvas(WheelProgram::new(hue, saturation, value, |h, s| {
//!     MyMessage::HueSatChanged(h, s)
//! }))
//! .width(250)
//! .height(250);
//! ```

use iced::widget::canvas::{self, Action, Frame, Geometry, Path};
use iced::{mouse, Color, Event, Point, Rectangle, Renderer, Size, Theme};
use std::f32::consts::PI;

// Rendering resolution
const HUE_STEPS: usize = 360;
const SAT_STEPS: usize = 128;

// ---------------------------------------------------------------------------
// HSV <-> RGB helpers
// ---------------------------------------------------------------------------

/// Convert HSV (h: 0-360, s: 0-1, v: 0-1) to iced::Color.
pub fn hsv_to_color(h: f32, s: f32, v: f32) -> Color {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);

    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Color::from_rgb(r + m, g + m, b + m)
}

/// Convert iced::Color to HSV (h: 0-360, s: 0-1, v: 0-1).
pub fn color_to_hsv(c: Color) -> (f32, f32, f32) {
    let r = c.r;
    let g = c.g;
    let b = c.b;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;

    let v = max;
    let s = if max.abs() < 1e-6 { 0.0 } else { d / max };

    let h = if d.abs() < 1e-6 {
        0.0
    } else if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / d) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };

    let h = ((h % 360.0) + 360.0) % 360.0;
    (h, s, v)
}

/// Convert HSV to hex string (#RRGGBB).
pub fn hsv_to_hex(h: f32, s: f32, v: f32) -> String {
    let c = hsv_to_color(h, s, v);
    format!(
        "#{:02X}{:02X}{:02X}",
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8,
    )
}

/// Parse a hex string (#RRGGBB or RRGGBB) to iced::Color.
pub fn hex_to_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::from_rgb8(r, g, b))
}

// ---------------------------------------------------------------------------
// Canvas program — the wheel rendering + interaction
// ---------------------------------------------------------------------------

/// A circular HSV color wheel canvas program for Iced.
///
/// Generic over any message type. Provide a callback that maps `(hue, saturation)`
/// to your app's message type.
///
/// - `hue`: 0-360 degrees
/// - `saturation`: 0.0-1.0
/// - `value`: 0.0-1.0 (brightness, controls wheel appearance)
pub struct WheelProgram<Message> {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    on_change: Box<dyn Fn(f32, f32) -> Message>,
}

impl<Message> WheelProgram<Message> {
    /// Create a new wheel program.
    ///
    /// `on_change` is called with `(hue, saturation)` when the user clicks or drags.
    pub fn new(
        hue: f32,
        saturation: f32,
        value: f32,
        on_change: impl Fn(f32, f32) -> Message + 'static,
    ) -> Self {
        Self {
            hue,
            saturation,
            value,
            on_change: Box::new(on_change),
        }
    }
}

/// Internal canvas state (persists across frames).
#[derive(Default)]
pub struct WheelState {
    is_dragging: bool,
}

impl<Message: Clone> canvas::Program<Message> for WheelProgram<Message> {
    type State = WheelState;

    fn update(
        &self,
        state: &mut WheelState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    if let Some((h, s)) = wheel_hit_test(pos, bounds.size()) {
                        state.is_dragging = true;
                        return Some(
                            Action::publish((self.on_change)(h, s)).and_capture(),
                        );
                    }
                }
                None
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.is_dragging {
                    if let Some(pos) = cursor.position_in(bounds) {
                        let (h, s) = wheel_position_to_hs(pos, bounds.size());
                        return Some(
                            Action::publish((self.on_change)(h, s)).and_capture(),
                        );
                    }
                }
                None
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.is_dragging {
                    state.is_dragging = false;
                    return Some(Action::capture());
                }
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &WheelState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        draw_wheel(&mut frame, bounds.size(), self.value);
        draw_selector(&mut frame, bounds.size(), self.hue, self.saturation);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &WheelState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.is_dragging {
            return mouse::Interaction::Grabbing;
        }
        if let Some(pos) = cursor.position_in(bounds) {
            if wheel_hit_test(pos, bounds.size()).is_some() {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

fn wheel_geometry(size: Size) -> (f32, f32, f32) {
    let side = size.width.min(size.height);
    let radius = side / 2.0 - 4.0;
    let cx = size.width / 2.0;
    let cy = size.height / 2.0;
    (cx, cy, radius)
}

fn wheel_hit_test(pos: Point, size: Size) -> Option<(f32, f32)> {
    let (cx, cy, radius) = wheel_geometry(size);
    let dx = pos.x - cx;
    let dy = pos.y - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= radius {
        let hue = (dy.atan2(dx).to_degrees() + 360.0) % 360.0;
        let sat = (dist / radius).min(1.0);
        Some((hue, sat))
    } else {
        None
    }
}

fn wheel_position_to_hs(pos: Point, size: Size) -> (f32, f32) {
    let (cx, cy, radius) = wheel_geometry(size);
    let dx = pos.x - cx;
    let dy = pos.y - cy;
    let hue = (dy.atan2(dx).to_degrees() + 360.0) % 360.0;
    let dist = (dx * dx + dy * dy).sqrt();
    let sat = (dist / radius).clamp(0.0, 1.0);
    (hue, sat)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw_wheel(frame: &mut Frame, size: Size, value: f32) {
    let (cx, cy, radius) = wheel_geometry(size);

    let angle_step = 2.0 * PI / HUE_STEPS as f32;
    let r_step = radius / SAT_STEPS as f32;

    for h_idx in 0..HUE_STEPS {
        let a0 = h_idx as f32 * angle_step;
        let a1 = (h_idx + 1) as f32 * angle_step;
        let hue = h_idx as f32 * (360.0 / HUE_STEPS as f32);

        let cos0 = a0.cos();
        let sin0 = a0.sin();
        let cos1 = a1.cos();
        let sin1 = a1.sin();

        for s_idx in 0..SAT_STEPS {
            let r_inner = s_idx as f32 * r_step;
            let r_outer = (s_idx + 1) as f32 * r_step;
            let sat = (s_idx as f32 + 0.5) / SAT_STEPS as f32;

            let color = hsv_to_color(hue, sat, value);

            let path = Path::new(|b| {
                b.move_to(Point::new(cx + r_inner * cos0, cy + r_inner * sin0));
                b.line_to(Point::new(cx + r_inner * cos1, cy + r_inner * sin1));
                b.line_to(Point::new(cx + r_outer * cos1, cy + r_outer * sin1));
                b.line_to(Point::new(cx + r_outer * cos0, cy + r_outer * sin0));
                b.close();
            });

            frame.fill(&path, color);
        }
    }

    let border = Path::circle(Point::new(cx, cy), radius);
    frame.stroke(
        &border,
        canvas::Stroke::default()
            .with_color(Color::from_rgb(0.4, 0.4, 0.4))
            .with_width(1.0),
    );
}

fn draw_selector(frame: &mut Frame, size: Size, hue: f32, saturation: f32) {
    let (cx, cy, radius) = wheel_geometry(size);

    let angle = hue.to_radians();
    let dist = saturation * radius;
    let sx = cx + dist * angle.cos();
    let sy = cy + dist * angle.sin();

    let outer = Path::circle(Point::new(sx, sy), 7.0);
    frame.stroke(
        &outer,
        canvas::Stroke::default()
            .with_color(Color::from_rgb(0.1, 0.1, 0.1))
            .with_width(2.0),
    );

    let inner = Path::circle(Point::new(sx, sy), 5.0);
    frame.stroke(
        &inner,
        canvas::Stroke::default()
            .with_color(Color::WHITE)
            .with_width(2.0),
    );
}
