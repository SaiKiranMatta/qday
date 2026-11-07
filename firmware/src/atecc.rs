//! The ATECC608A on GPIO 5 (SDA) and GPIO 4 (SCL), built only with the `atecc-probe` or
//! `atecc-counter` feature. The probe only reads. Nothing here changes the chip's configuration or
//! locks it. `qday device-log` prints the report.

use core::fmt::Write as _;

use esp_hal::{
    Blocking,
    delay::Delay,
    i2c::master::{Config, I2c},
    peripherals::{GPIO4, GPIO5, I2C0},
    time::Rate,
    usb::usb_serial_jtag::UsbSerialJtagTx,
};
use qday_ui::text::Text;
use qday_wallet::atecc::{ADDRESS, Atecc, AteccBus, BusError};

use crate::hardware::now_ms;

/// The wake pulse is a write to address 0, which only holds SDA low for long enough at this speed.
const I2C_KHZ: u32 = 100;
const REPORT_MS: u64 = 3000;
/// How long a report may wait for the laptop.
const WRITE_STALL_MS: u64 = 20;

/// The I2C wires to the chip.
pub struct Wires {
    i2c: I2c<'static, Blocking>,
    delay: Delay,
}

impl Wires {
    pub fn new(i2c: I2C0<'static>, sda: GPIO5<'static>, scl: GPIO4<'static>) -> Self {
        let config = Config::default().with_frequency(Rate::from_khz(I2C_KHZ));
        let i2c = I2c::new(i2c, config)
            .expect("the I2C config is valid")
            .with_sda(sda)
            .with_scl(scl);
        Self {
            i2c,
            delay: Delay::new(),
        }
    }
}

impl AteccBus for Wires {
    fn wake_pulse(&mut self) {
        // Nothing lives at address 0, so this is always refused. The low pulse is all it's for.
        let _ = self.i2c.write(0u8, &[0]);
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), BusError> {
        self.i2c.write(ADDRESS, bytes).map_err(|_| BusError)
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<(), BusError> {
        self.i2c.read(ADDRESS, buffer).map_err(|_| BusError)
    }

    fn wait_us(&mut self, us: u32) {
        self.delay.delay_micros(us);
    }
}

/// Repeats what a read-only look at the chip found, so a laptop can connect at any time.
pub struct Report {
    line: Text<96>,
    reported_ms: u64,
}

impl Report {
    pub fn run(chip: &mut Atecc<Wires>) -> Self {
        let mut line = Text::new();
        let _ = match chip.probe() {
            Ok(probe) => {
                let [a, b, c, d] = probe.revision;
                write!(
                    line,
                    "I qday-atecc revision {a:02x}{b:02x}{c:02x}{d:02x} config-locked {} \
                     data-locked {} counter {:?}\n",
                    u8::from(probe.locks.config),
                    u8::from(probe.locks.data),
                    probe.counter,
                )
            }
            Err(error) => write!(line, "I qday-atecc no answer {error:?}\n"),
        };
        Self {
            line,
            reported_ms: 0,
        }
    }

    pub fn tick(&mut self, usb: &mut UsbSerialJtagTx<'_, Blocking>) {
        let now = now_ms();
        if now.saturating_sub(self.reported_ms) < REPORT_MS {
            return;
        }
        self.reported_ms = now;
        for &byte in self.line.as_str().as_bytes() {
            let started = now_ms();
            while usb.write_byte_nb(byte).is_err() {
                if now_ms().saturating_sub(started) > WRITE_STALL_MS {
                    return;
                }
            }
        }
        let _ = usb.flush_tx_nb();
    }
}
