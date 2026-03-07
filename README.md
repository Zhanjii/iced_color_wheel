# iced_color_wheel

A circular HSV color wheel widget for [Iced](https://github.com/iced-rs/iced).

![Iced 0.14](https://img.shields.io/badge/iced-0.14-blue)
![License: MIT](https://img.shields.io/badge/license-MIT-green)

No circular color wheel picker exists in the Iced ecosystem — this fills that gap.

## Features

- Circular hue/saturation wheel (angle = hue, distance from center = saturation)
- Smooth gradient rendering (360 hue steps x 128 saturation bands)
- Click and drag interaction with proper mouse cursor feedback
- Brightness (value) parameter controls wheel appearance
- HSV/RGB/hex conversion utilities included
- Generic over your app's message type

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
iced_color_wheel = { path = "../iced_color_wheel" }
# or from crates.io once published:
# iced_color_wheel = "0.1"
```

In your Iced app:

```rust
use iced::widget::canvas;
use iced_color_wheel::{WheelProgram, hsv_to_color, hsv_to_hex};

// In your view function:
let wheel = canvas(WheelProgram::new(
    self.hue,        // 0-360
    self.saturation,  // 0.0-1.0
    self.value,       // 0.0-1.0 (brightness)
    |h, s| MyMessage::HueSatChanged(h, s),
))
.width(250)
.height(250);
```

Pair with an Iced `slider` for brightness control — see the `basic` example for a complete setup.

## Examples

```bash
cargo run --example basic
```

## Conversion Utilities

```rust
use iced_color_wheel::{hsv_to_color, color_to_hsv, hsv_to_hex, hex_to_color};

let color = hsv_to_color(120.0, 0.8, 1.0);  // bright green
let (h, s, v) = color_to_hsv(color);
let hex = hsv_to_hex(120.0, 0.8, 1.0);       // "#33FF33"
let parsed = hex_to_color("#FF0000");          // Some(red)
```

## License

MIT
