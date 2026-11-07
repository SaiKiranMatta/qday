//! The QDay wallet. This file only connects hardware to `qday-ui`, where every decision is made
//! and tested.

#![no_std]
#![no_main]

extern crate alloc;

#[cfg(feature = "atecc-probe")]
mod atecc;
mod hardware;
#[cfg(feature = "measure")]
mod measure;

use core::cell::RefCell;

use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    main,
    psram::{Psram, PsramConfig, PsramMode, SpiRamFreq},
    rmt::{Rmt, TxChannelConfig, TxChannelCreator},
    rng::{Trng, TrngSource},
    spi::{
        Mode,
        master::{Config, Spi},
    },
    time::Rate,
    usb::usb_serial_jtag::UsbSerialJtag,
};
use esp_storage::FlashStorage;
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
};
use qday_ui::app::{App, Light};
use qday_ui::fb::{Framebuffer, PIXELS};
#[cfg(feature = "atecc-probe")]
use qday_wallet::atecc::Atecc;
#[cfg(feature = "atecc-counter")]
use qday_wallet::atecc::ChipCounter;
use qday_wallet::counter::FlashCounter;
#[cfg(feature = "atecc-counter")]
use qday_wallet::counter::PairedCounter;
use qday_wallet::flash::{Aligned, layout};
use qday_wallet::keystore::KeyStore;
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::Passcode;
use qday_wallet::protocol::LineBuffer;
use qday_wallet::settings::Settings;
use qday_wallet::wallet::Wallet;

use hardware::{Board, HardwareRng, PartitionFlash, StatusLight, Touch, UsbWatch, now_ms};

esp_bootloader_esp_idf::esp_app_desc!();

/// Everything allocated lives in internal RAM, because flash operations and external RAM can't run
/// at once. Only the framebuffer is in PSRAM, and it's never in a flash operation's way: nothing
/// here runs in an interrupt, and the screen is only drawn between flash operations. `heap_peak` in
/// `qday-ui`'s tests measures what the heap has to cover, and the rest of RAM is left for the stack,
/// which ML-DSA needs at least 117 KB of.
const HEAP_BYTES: usize = 96 * 1024;
const RECLAIMED_HEAP_BYTES: usize = 73_744;
/// Over breadboard wires the picture turns to noise at 80 MHz.
const LCD_MHZ: u32 = 40;
const TOUCH_MHZ: u32 = 1;
/// The RMT clock, which sets the WS2812 pulse lengths.
const RMT_MHZ: u32 = 80;
const POLL_MS: u32 = 5;

#[main]
fn main() -> ! {
    #[cfg(feature = "measure")]
    measure::paint_stack();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    // The RAM the bootloader used is free now, and using it costs the stack nothing.
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: RECLAIMED_HEAP_BYTES);
    esp_alloc::heap_allocator!(size: HEAP_BYTES);
    let psram = Psram::new(
        peripherals.PSRAM,
        PsramConfig {
            mode: PsramMode::OctalSpi,
            ram_frequency: SpiRamFreq::Freq80m,
            ..Default::default()
        },
    );
    let (psram_start, psram_bytes) = psram.raw_parts();
    assert!(
        psram_bytes >= PIXELS * size_of::<u16>(),
        "the framebuffer needs PSRAM"
    );
    let psram_range = psram_start as usize..psram_start as usize + psram_bytes;
    // SAFETY: nothing else uses PSRAM, it's aligned, and it stays mapped for the whole program.
    let screen = unsafe { core::slice::from_raw_parts_mut(psram_start.cast::<u16>(), PIXELS) };
    let mut delay = Delay::new();

    let spi_config = Config::default()
        .with_frequency(Rate::from_mhz(LCD_MHZ))
        .with_mode(Mode::_0);
    let spi = Spi::new(peripherals.SPI2, spi_config)
        .expect("the LCD SPI config is valid")
        .with_sck(peripherals.GPIO9)
        .with_mosi(peripherals.GPIO10);
    let cs = Output::new(peripherals.GPIO13, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO11, Level::Low, OutputConfig::default());
    let reset = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());
    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).expect("the CS pin cannot fail");
    let mut buffer = [0_u8; 2048];
    let interface = SpiInterface::new(spi_device, dc, &mut buffer);
    let display = Builder::new(ILI9341Rgb565, interface)
        .reset_pin(reset)
        .color_order(ColorOrder::Bgr)
        .orientation(Orientation::new().rotate(Rotation::Deg90).flip_horizontal())
        .init(&mut delay)
        .expect("the display starts");
    let _backlight = Output::new(peripherals.GPIO46, Level::High, OutputConfig::default());

    let touch_config = Config::default()
        .with_frequency(Rate::from_mhz(TOUCH_MHZ))
        .with_mode(Mode::_0);
    let touch_spi = Spi::new(peripherals.SPI3, touch_config)
        .expect("the touch SPI config is valid")
        .with_sck(peripherals.GPIO8)
        .with_mosi(peripherals.GPIO17)
        .with_miso(peripherals.GPIO16);
    let touch_cs = Output::new(peripherals.GPIO18, Level::High, OutputConfig::default());
    let mut touch = Touch::new(touch_spi, touch_cs);

    let pull_up = InputConfig::default().with_pull(Pull::Up);
    let keys = [
        Input::new(peripherals.GPIO3, pull_up),
        Input::new(peripherals.GPIO6, pull_up),
        Input::new(peripherals.GPIO7, pull_up),
        Input::new(peripherals.GPIO15, pull_up),
    ];

    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(RMT_MHZ)).expect("the RMT clock is valid");
    let channel = rmt
        .channel0
        .configure_tx(&TxChannelConfig::default().with_clk_divider(1))
        .expect("the RMT channel config is valid")
        .with_pin(peripherals.GPIO48);
    let mut light = StatusLight::new(channel);
    light.set(Light::Off);

    // The hardware generator is only truly random while an entropy source runs, and the seed
    // comes from it, so this source stays alive for the whole program.
    let _entropy = TrngSource::new(peripherals.RNG, peripherals.ADC1);
    let rng = HardwareRng(Trng::try_new().expect("the entropy source is running"));

    let flash = RefCell::new(Aligned(PartitionFlash {
        storage: FlashStorage::new(peripherals.FLASH),
        psram: psram_range,
    }));
    #[cfg(feature = "atecc-probe")]
    let mut chip = Atecc::new(atecc::Wires::new(
        peripherals.I2C0,
        peripherals.GPIO5,
        peripherals.GPIO4,
    ));
    #[cfg(feature = "atecc-probe")]
    let mut chip_report = atecc::Report::run(&mut chip);
    // Off unless built with `atecc-counter`, and then signing stops if the chip stops answering.
    #[cfg(feature = "atecc-counter")]
    let counter = PairedCounter::new(
        ChipCounter::new(chip),
        FlashCounter::new(&flash, layout::COUNTER),
    );
    #[cfg(not(feature = "atecc-counter"))]
    let counter = FlashCounter::new(&flash, layout::COUNTER);
    let wallet = Wallet::open(
        KeyStore::new(&flash),
        Passcode::new(&flash, layout::PASSCODE),
        counter,
        Settings::new(&flash),
        MlKeyStore::new(&flash),
        now_ms(),
    )
    .expect("the qday partition can be read");
    let settings = Settings::new(&flash);

    let (mut usb_rx, usb_tx) = UsbSerialJtag::new(peripherals.USB_DEVICE).split();
    let mut board = Board {
        display,
        light,
        usb: usb_tx,
    };

    let hold_k1 = keys[0].is_low();
    #[allow(
        unused_mut,
        reason = "only the measuring build draws before the app starts"
    )]
    let mut fb = Framebuffer::placed(screen);
    #[cfg(feature = "measure")]
    let mut report = measure::Report::run(&mut fb, &mut board, &flash);
    let mut app = App::start(wallet, settings, fb, rng, hold_k1, &mut board);

    let mut lines = LineBuffer::default();
    let mut watch = UsbWatch::default();
    loop {
        while let Ok(byte) = usb_rx.read_byte() {
            if let Some(line) = lines.push(byte) {
                app.on_line(&mut board, &line);
            }
        }
        app.set_usb(watch.connected());
        let held = [
            keys[0].is_low(),
            keys[1].is_low(),
            keys[2].is_low(),
            keys[3].is_low(),
        ];
        app.poll(&mut board, held, touch.read());
        #[cfg(feature = "measure")]
        report.tick(&mut board.usb);
        #[cfg(feature = "atecc-probe")]
        chip_report.tick(&mut board.usb);
        delay.delay_millis(POLL_MS);
    }
}
