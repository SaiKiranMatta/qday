//! Touch and button test: starts the ILI9341 in landscape, draws a dot wherever the XPT2046 reports
//! a press, and lights a box on screen while each of the four buttons is held.

#![no_std]
#![no_main]

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
    Blocking,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    main,
    spi::{
        Mode,
        master::{Config, Spi},
    },
    time::Rate,
};
use esp_println::println;
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
};

esp_bootloader_esp_idf::esp_app_desc!();

const SCREEN_WIDTH: i32 = 320;
const SCREEN_HEIGHT: i32 = 240;

/// XPT2046 commands: read X, Y and the two pressure channels.
const READ_X: u8 = 0xD0;
const READ_Y: u8 = 0x90;
const READ_Z1: u8 = 0xB0;
const READ_Z2: u8 = 0xC0;

/// Pressure above this value counts as a press.
const PRESS_THRESHOLD: i32 = 400;
/// Pixels between each calibration point and the nearest screen edge.
const EDGE: i32 = 20;
/// Raw Y grows from left to right across the landscape screen.
const RAW_Y_LEFT: i32 = 567;
const RAW_Y_RIGHT: i32 = 3_673;
/// Raw X grows from top to bottom down the landscape screen.
const RAW_X_TOP: i32 = 523;
const RAW_X_BOTTOM: i32 = 3_523;
/// Readings averaged for each position, to steady the dot.
const SAMPLES: i32 = 4;
/// Time between readings. It also debounces the buttons.
const POLL_MS: u32 = 20;

/// Button names, left to right.
const KEY_NAMES: [&str; 4] = ["K1", "K2", "K3", "K4"];
/// Size and position of the button boxes along the bottom of the screen.
const KEY_BOX_SIZE: Size = Size::new(70, 40);
const KEY_BOX_LEFT: i32 = 8;
const KEY_BOX_STEP: i32 = 78;
const KEY_BOX_TOP: i32 = 192;

/// Reads one 12-bit value from the touch controller.
fn read_channel(spi: &mut Spi<'_, Blocking>, cs: &mut Output<'_>, command: u8) -> i32 {
    let mut frame = [command, 0, 0];
    cs.set_low();
    spi.transfer(&mut frame)
        .expect("the touch SPI bus accepts a transfer");
    cs.set_high();
    i32::from(u16::from_be_bytes([frame[1], frame[2]]) >> 3)
}

/// Converts a raw touch reading to a landscape screen position. The panel's axes are swapped
/// relative to the screen, so raw Y gives the screen's X.
fn to_screen(raw_x: i32, raw_y: i32) -> Point {
    let span_x = SCREEN_WIDTH - 2 * EDGE;
    let span_y = SCREEN_HEIGHT - 2 * EDGE;
    let x = EDGE + (raw_y - RAW_Y_LEFT) * span_x / (RAW_Y_RIGHT - RAW_Y_LEFT);
    let y = EDGE + (raw_x - RAW_X_TOP) * span_y / (RAW_X_BOTTOM - RAW_X_TOP);
    Point::new(x.clamp(0, SCREEN_WIDTH - 1), y.clamp(0, SCREEN_HEIGHT - 1))
}

/// Draws one button's box, lit while the button is held.
fn draw_key<D>(display: &mut D, slot: usize, name: &str, held: bool)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: core::fmt::Debug,
{
    let (fill, ink) = if held {
        (Rgb565::YELLOW, Rgb565::BLACK)
    } else {
        (Rgb565::CSS_DIM_GRAY, Rgb565::WHITE)
    };
    let left = KEY_BOX_LEFT + KEY_BOX_STEP * i32::try_from(slot).expect("there are four slots");
    let area = Rectangle::new(Point::new(left, KEY_BOX_TOP), KEY_BOX_SIZE);

    area.into_styled(PrimitiveStyle::with_fill(fill))
        .draw(display)
        .expect("the display accepts a box");
    Text::with_alignment(
        name,
        area.center() + Point::new(0, 6),
        MonoTextStyle::new(&FONT_10X20, ink),
        Alignment::Center,
    )
    .draw(display)
    .expect("the display accepts text");
}

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut delay = Delay::new();

    // 10 MHz is slow enough to be reliable over breadboard jumper wires.
    let spi_config = Config::default()
        .with_frequency(Rate::from_mhz(10))
        .with_mode(Mode::_0);
    let spi = Spi::new(peripherals.SPI2, spi_config)
        .expect("the SPI config is valid")
        .with_sck(peripherals.GPIO9)
        .with_mosi(peripherals.GPIO10);

    let cs = Output::new(peripherals.GPIO13, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO11, Level::Low, OutputConfig::default());
    let reset = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).expect("the CS pin cannot fail");
    let mut buffer = [0_u8; 512];
    let interface = SpiInterface::new(spi_device, dc, &mut buffer);

    let mut display = Builder::new(ILI9341Rgb565, interface)
        .reset_pin(reset)
        .color_order(ColorOrder::Bgr)
        .orientation(Orientation::new().rotate(Rotation::Deg90).flip_horizontal())
        .init(&mut delay)
        .expect("the display starts");

    let _backlight = Output::new(peripherals.GPIO46, Level::High, OutputConfig::default());

    // The touch controller has its own bus. It is limited to about 2 MHz.
    let touch_config = Config::default()
        .with_frequency(Rate::from_mhz(1))
        .with_mode(Mode::_0);
    let mut touch = Spi::new(peripherals.SPI3, touch_config)
        .expect("the SPI config is valid")
        .with_sck(peripherals.GPIO8)
        .with_mosi(peripherals.GPIO17)
        .with_miso(peripherals.GPIO16);
    let mut touch_cs = Output::new(peripherals.GPIO18, Level::High, OutputConfig::default());

    // Each button connects its pin to ground, so a held button reads low.
    let pull_up = InputConfig::default().with_pull(Pull::Up);
    let keys = [
        Input::new(peripherals.GPIO3, pull_up),
        Input::new(peripherals.GPIO6, pull_up),
        Input::new(peripherals.GPIO7, pull_up),
        Input::new(peripherals.GPIO15, pull_up),
    ];
    let mut held = [false; 4];

    display
        .clear(Rgb565::BLACK)
        .expect("the display accepts a fill");
    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    Text::new("Touch or press a button", Point::new(20, 30), text_style)
        .draw(&mut display)
        .expect("the display accepts text");
    for (slot, name) in KEY_NAMES.iter().enumerate() {
        draw_key(&mut display, slot, name, false);
    }
    let dot_style = PrimitiveStyle::with_fill(Rgb565::YELLOW);
    println!("input test: ready");

    loop {
        for (slot, key) in keys.iter().enumerate() {
            let is_held = key.is_low();
            if is_held != held[slot] {
                held[slot] = is_held;
                let name = KEY_NAMES[slot];
                println!("key: {name} {}", if is_held { "down" } else { "up" });
                draw_key(&mut display, slot, name, is_held);
            }
        }

        let z1 = read_channel(&mut touch, &mut touch_cs, READ_Z1);
        let z2 = read_channel(&mut touch, &mut touch_cs, READ_Z2);
        let pressure = z1 + 4_095 - z2;

        if pressure > PRESS_THRESHOLD {
            let (mut raw_x, mut raw_y) = (0, 0);
            for _ in 0..SAMPLES {
                raw_x += read_channel(&mut touch, &mut touch_cs, READ_X);
                raw_y += read_channel(&mut touch, &mut touch_cs, READ_Y);
            }
            raw_x /= SAMPLES;
            raw_y /= SAMPLES;

            let point = to_screen(raw_x, raw_y);
            println!(
                "touch: raw {raw_x},{raw_y} pressure {pressure} pixel {},{}",
                point.x, point.y
            );

            Circle::with_center(point, 7)
                .into_styled(dot_style)
                .draw(&mut display)
                .expect("the display accepts a dot");
        }

        delay.delay_millis(POLL_MS);
    }
}
