//! Touch test: starts the ILI9341 in landscape and draws a dot wherever the XPT2046 reports a press.

#![no_std]
#![no_main]

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
    text::Text,
};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
    Blocking,
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
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
/// Time between touch readings.
const POLL_MS: u32 = 20;

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

    display
        .clear(Rgb565::BLACK)
        .expect("the display accepts a fill");
    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    Text::new("Touch the screen", Point::new(20, 30), text_style)
        .draw(&mut display)
        .expect("the display accepts text");
    let dot_style = PrimitiveStyle::with_fill(Rgb565::YELLOW);
    println!("touch test: ready");

    loop {
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
