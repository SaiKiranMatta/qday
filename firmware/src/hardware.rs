//! The hardware behind `qday-ui`'s traits.

use embedded_graphics::{pixelcolor::Rgb565, pixelcolor::raw::RawU16};
use embedded_hal::digital::OutputPin;
use esp_hal::{
    Blocking,
    gpio::{Level, Output},
    peripherals::USB_DEVICE,
    rmt::{Channel, PulseCode, Tx},
    rng::Trng,
    spi::master::Spi,
    time::Instant,
    usb::usb_serial_jtag::UsbSerialJtagTx,
};
use esp_storage::FlashStorage;
use mipidsi::{Display, interface::Interface, models::Model};
use qday_ui::app::{Host, Light};
use qday_ui::fb::{Framebuffer, WIDTH};
use qday_wallet::flash::{FlashError, SECTOR, WordFlash};
use qday_wallet::rng::Rng;

/// Where the `qday` partition starts in flash.
const PARTITION: u32 = 0x31_0000;

/// XPT2046 commands: read X, Y and the two pressure channels.
const READ_X: u8 = 0xD0;
const READ_Y: u8 = 0x90;
const READ_Z1: u8 = 0xB0;
const READ_Z2: u8 = 0xC0;
/// T_IRQ isn't wired, so pressure above this counts as a press.
const PRESS_THRESHOLD: i32 = 400;
const TOUCH_SAMPLES: i32 = 4;

/// WS2812 bit lengths in 12.5 ns RMT ticks.
const T0_HIGH: u16 = 32;
const T0_LOW: u16 = 68;
const T1_HIGH: u16 = 64;
const T1_LOW: u16 = 36;
const LIGHT_BITS: usize = 24;

/// How long a USB write waits for the laptop before giving up.
const USB_STALL_MS: u64 = 1000;

/// How often to look for USB start-of-frame packets, which only arrive from a live host.
const USB_CHECK_MS: u64 = 500;

/// Milliseconds since start.
pub fn now_ms() -> u64 {
    Instant::now().duration_since_epoch().as_millis()
}

/// The `qday` partition, through raw NOR writes. The chip's convenience write erases whole
/// sectors behind the scenes, which would undo every power-cut guarantee, so it's never used.
pub struct PartitionFlash<'d>(pub FlashStorage<'d>);

impl WordFlash for PartitionFlash<'_> {
    fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), FlashError> {
        self.0
            .read(PARTITION + offset, buffer)
            .map_err(|_| FlashError)
    }

    fn write_words(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError> {
        self.0
            .write_nor(PARTITION + offset, data)
            .map_err(|_| FlashError)
    }

    fn erase(&mut self, offset: u32) -> Result<(), FlashError> {
        let start = PARTITION + offset;
        self.0.erase(start, start + SECTOR).map_err(|_| FlashError)
    }
}

/// The true random number generator.
pub struct HardwareRng(pub Trng);

impl Rng for HardwareRng {
    fn fill(&mut self, buffer: &mut [u8]) {
        self.0.read(buffer);
    }
}

/// The XPT2046 touch controller.
pub struct Touch<'d> {
    spi: Spi<'d, Blocking>,
    cs: Output<'d>,
}

impl<'d> Touch<'d> {
    /// A touch controller on its own bus.
    pub fn new(spi: Spi<'d, Blocking>, cs: Output<'d>) -> Self {
        Self { spi, cs }
    }

    fn channel(&mut self, command: u8) -> i32 {
        let mut frame = [command, 0, 0];
        self.cs.set_low();
        let result = self.spi.transfer(&mut frame);
        self.cs.set_high();
        if result.is_err() {
            return 0;
        }
        i32::from(u16::from_be_bytes([frame[1], frame[2]]) >> 3)
    }

    /// The raw position while the panel is pressed.
    pub fn read(&mut self) -> Option<(i32, i32)> {
        let z1 = self.channel(READ_Z1);
        let z2 = self.channel(READ_Z2);
        if z1 + 4_095 - z2 <= PRESS_THRESHOLD {
            return None;
        }
        let (mut raw_x, mut raw_y) = (0, 0);
        for _ in 0..TOUCH_SAMPLES {
            raw_x += self.channel(READ_X);
            raw_y += self.channel(READ_Y);
        }
        Some((raw_x / TOUCH_SAMPLES, raw_y / TOUCH_SAMPLES))
    }
}

/// The WS2812 on GPIO 48.
pub struct StatusLight<'d> {
    channel: Option<Channel<'d, Blocking, Tx>>,
}

impl<'d> StatusLight<'d> {
    /// A light on an RMT channel.
    pub fn new(channel: Channel<'d, Blocking, Tx>) -> Self {
        Self {
            channel: Some(channel),
        }
    }

    /// Sets the colour. The chip expects green, then red, then blue.
    pub fn set(&mut self, light: Light) {
        let (red, green, blue) = light.rgb();
        let bits = u32::from(green) << 16 | u32::from(red) << 8 | u32::from(blue);
        let mut pulses = [PulseCode::end_marker(); LIGHT_BITS + 1];
        for (position, pulse) in pulses[..LIGHT_BITS].iter_mut().enumerate() {
            let high = bits & (1 << (LIGHT_BITS - 1 - position)) != 0;
            let (on, off) = if high {
                (T1_HIGH, T1_LOW)
            } else {
                (T0_HIGH, T0_LOW)
            };
            *pulse = PulseCode::new(Level::High, on, Level::Low, off);
        }
        let Some(channel) = self.channel.take() else {
            return;
        };
        self.channel = match channel.transmit(&pulses) {
            Ok(transaction) => match transaction.wait() {
                Ok(channel) | Err((_, channel)) => Some(channel),
            },
            Err((_, channel)) => Some(channel),
        };
    }
}

/// Notices whether a laptop is on the USB port.
#[derive(Default)]
pub struct UsbWatch {
    frame: u32,
    checked_ms: u64,
    connected: bool,
}

impl UsbWatch {
    /// Whether start-of-frame packets are arriving.
    pub fn connected(&mut self) -> bool {
        let now = now_ms();
        if now.saturating_sub(self.checked_ms) >= USB_CHECK_MS {
            self.checked_ms = now;
            let frame = u32::from(
                USB_DEVICE::regs()
                    .fram_num()
                    .read()
                    .sof_frame_index()
                    .bits(),
            );
            self.connected = frame != self.frame;
            self.frame = frame;
        }
        self.connected
    }
}

/// The parts `qday-ui` drives directly.
pub struct Board<'d, DI, M, RST>
where
    DI: Interface<Word = u8>,
    M: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
{
    /// The screen.
    pub display: Display<DI, M, RST>,
    /// The status light.
    pub light: StatusLight<'d>,
    /// The way out to the laptop.
    pub usb: UsbSerialJtagTx<'d, Blocking>,
}

impl<DI, M, RST> Host for Board<'_, DI, M, RST>
where
    DI: Interface<Word = u8>,
    M: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
{
    fn now_ms(&mut self) -> u64 {
        now_ms()
    }

    fn present(&mut self, fb: &mut Framebuffer) {
        let display = &mut self.display;
        fb.send_changes(|first, rows, pixels| {
            let (top, bottom) = (first as u16, (first + rows - 1) as u16);
            let colors = pixels.iter().map(|&pixel| Rgb565::from(RawU16::new(pixel)));
            let _ = display.set_pixels(0, top, (WIDTH - 1) as u16, bottom, colors);
        });
    }

    fn set_light(&mut self, light: Light) {
        self.light.set(light);
    }

    /// Writes one byte at a time, waiting whenever the 64-byte FIFO is full. The driver's own
    /// `write` assumes the FIFO is empty, so it drops bytes when log output is still queued, and
    /// it has no timeout, so an absent laptop would hang the device.
    fn send(&mut self, line: &str) {
        for &byte in line.as_bytes() {
            let started = now_ms();
            loop {
                match self.usb.write_byte_nb(byte) {
                    Ok(()) => break,
                    Err(nb::Error::WouldBlock) if now_ms() - started < USB_STALL_MS => {}
                    Err(_) => return,
                }
            }
        }
        let started = now_ms();
        while let Err(nb::Error::WouldBlock) = self.usb.flush_tx_nb() {
            if now_ms() - started >= USB_STALL_MS {
                return;
            }
        }
    }

    fn restart(&mut self) {
        esp_hal::system::software_reset();
    }
}
