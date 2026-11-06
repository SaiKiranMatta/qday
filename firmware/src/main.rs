//! Screen test: starts the ILI9341 in landscape, then cycles red, green, blue and a text page.

#![no_std]
#![no_main]

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
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

/// How long each colour stays on screen.
const COLOUR_MS: u32 = 1_000;
/// How long the text page stays on screen.
const TEXT_MS: u32 = 3_000;

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

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    loop {
        for (name, colour) in [
            ("red", Rgb565::RED),
            ("green", Rgb565::GREEN),
            ("blue", Rgb565::BLUE),
        ] {
            println!("screen test: {name}");
            display.clear(colour).expect("the display accepts a fill");
            delay.delay_millis(COLOUR_MS);
        }

        println!("screen test: text");
        display
            .clear(Rgb565::BLACK)
            .expect("the display accepts a fill");
        Text::new("QDay screen test", Point::new(20, 100), text_style)
            .draw(&mut display)
            .expect("the display accepts text");
        Text::new("320 x 240 landscape", Point::new(20, 130), text_style)
            .draw(&mut display)
            .expect("the display accepts text");
        delay.delay_millis(TEXT_MS);
    }
}
