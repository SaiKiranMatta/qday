//! On-board measurements, built only with the `measure` feature: how much stack the firmware has
//! really used, found by painting it, and how fast screens redraw. The results go out as log
//! lines, which the laptop tool skips and `qday device-log` prints.

use core::fmt::Write as _;
use core::ptr::{addr_of, read_volatile, write_volatile};

use esp_hal::{Blocking, time::Instant, usb::usb_serial_jtag::UsbSerialJtagTx};
use qday_ui::app::Host;
use qday_ui::fb::Framebuffer;
use qday_ui::nav::Setting;
use qday_ui::screens::{self, DeviceInfo, Review, ReviewInfo, Status};
use qday_ui::text::Text;
use qday_ui::theme::{self, Theme};
use qday_wallet::settings::Scheme;

use crate::hardware::now_ms;

const PAINT: u32 = 0xA5A5_A5A5;
/// Left unpainted at the far end, where the HAL keeps its stack guard.
const GUARD_BYTES: usize = 256;
/// Left unpainted below the caller, for this function's own frame.
const HEADROOM_BYTES: usize = 2048;
const REPORT_MS: u64 = 3000;
/// How long one report line may wait for the laptop.
const WRITE_STALL_MS: u64 = 20;

const ROUNDS: usize = 3;
const SCREENS: usize = 4;
const SAMPLES: usize = ROUNDS * SCREENS;

const ROOT: [u8; 32] = [0x9E; 32];
const TO: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";

unsafe extern "C" {
    static _stack_start_cpu0: u32;
    static _stack_end_cpu0: u32;
}

/// The addresses at the top of the stack, where it starts, and the bottom, where it would end.
fn stack_bounds() -> (usize, usize) {
    (
        addr_of!(_stack_start_cpu0) as usize,
        addr_of!(_stack_end_cpu0) as usize,
    )
}

/// Fills the unused stack with a pattern. Call this first in `main`.
#[inline(never)]
pub fn paint_stack() {
    let marker = 0_u32;
    let here = addr_of!(marker) as usize;
    let (_, bottom) = stack_bounds();
    let mut word = (bottom + GUARD_BYTES) as *mut u32;
    while (word as usize) < here - HEADROOM_BYTES {
        // SAFETY: this range is inside the stack and below every live frame.
        unsafe {
            write_volatile(word, PAINT);
            word = word.add(1);
        }
    }
}

/// The stack's size, and the most of it used so far.
fn stack_use() -> (usize, usize) {
    let (top, bottom) = stack_bounds();
    let mut word = (bottom + GUARD_BYTES) as *const u32;
    // SAFETY: this reads only the stack, one word at a time, and stops at its top.
    while (word as usize) < top && unsafe { read_volatile(word) } == PAINT {
        word = unsafe { word.add(1) };
    }
    (top - bottom, top - word as usize)
}

/// Middle and worst of a set of timings, in microseconds.
#[derive(Clone, Copy, Default)]
struct Spread {
    middle: u32,
    worst: u32,
}

impl Spread {
    fn of(samples: &mut [u32]) -> Self {
        samples.sort_unstable();
        Self {
            middle: samples[samples.len() / 2],
            worst: samples[samples.len() - 1],
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ThemeTimes {
    draw: Spread,
    send: Spread,
    whole: Spread,
}

fn draw(c: &mut qday_ui::canvas::Canvas, screen: usize) {
    let status = Status {
        tree: 0,
        next_index: 67,
        connected: true,
        scheme: Scheme::Xmss,
    };
    match screen {
        0 => screens::home(c, &status, &ROOT, false, None, None),
        1 => screens::keys(c, &status, false, None),
        2 => {
            let info = DeviceInfo {
                firmware: "qday",
                root: &ROOT,
                counter: 67,
            };
            screens::device(c, &status, &info, Setting::Theme, false, None);
        }
        _ => {
            let info = ReviewInfo {
                review: Review::Send {
                    amount: "0.5",
                    unit: "ETH",
                    to: TO,
                },
                fee: "0.0026 ETH",
                chain_id: 1337,
                scheme: Scheme::Xmss,
                index: 67,
                rotation: false,
                seconds_left: 24,
                timeout: 30,
            };
            screens::review(c, &info, None);
        }
    }
}

fn micros_since(start: Instant) -> u32 {
    start.elapsed().as_micros() as u32
}

/// What's been measured, reported every few seconds.
pub struct Report {
    themes: [ThemeTimes; Theme::ALL.len()],
    /// Sending a frame in which nothing changed: the cost of checking every row.
    unchanged: Spread,
    reported_ms: u64,
}

impl Report {
    /// Times four screens in every theme, on the real display.
    pub fn run(fb: &mut Framebuffer, host: &mut impl Host) -> Self {
        let mut themes = [ThemeTimes::default(); Theme::ALL.len()];
        let mut unchanged = [0; SAMPLES];
        for (times, &candidate) in themes.iter_mut().zip(Theme::ALL.iter()) {
            theme::set(candidate);
            let (mut drawing, mut sending, mut whole) = ([0; SAMPLES], [0; SAMPLES], [0; SAMPLES]);
            for sample in 0..SAMPLES {
                let started = Instant::now();
                draw(&mut fb.canvas(), sample % SCREENS);
                drawing[sample] = micros_since(started);
                let sent = Instant::now();
                host.present(fb);
                sending[sample] = micros_since(sent);
                whole[sample] = micros_since(started);
            }
            *times = ThemeTimes {
                draw: Spread::of(&mut drawing),
                send: Spread::of(&mut sending),
                whole: Spread::of(&mut whole),
            };
        }
        for sample in &mut unchanged {
            let started = Instant::now();
            host.present(fb);
            *sample = micros_since(started);
        }
        Self {
            themes,
            unchanged: Spread::of(&mut unchanged),
            reported_ms: 0,
        }
    }

    /// Writes the results, if it's time. A laptop that isn't reading costs at most a short wait.
    pub fn tick(&mut self, usb: &mut UsbSerialJtagTx<'_, Blocking>) {
        let now = now_ms();
        if now.saturating_sub(self.reported_ms) < REPORT_MS {
            return;
        }
        self.reported_ms = now;
        let (size, used) = stack_use();
        let stats = esp_alloc::HEAP.stats();
        let mut line = Text::<96>::new();
        let _ = write!(
            line,
            "I qday-measure stack {size} used {used} free {} heap size {} peak {}\n",
            size - used,
            stats.size,
            stats.max_usage,
        );
        write_line(usb, line.as_str());
        for (times, &candidate) in self.themes.iter().zip(Theme::ALL.iter()) {
            let mut line = Text::<96>::new();
            let _ = write!(
                line,
                "I qday-measure {} draw {}/{} send {}/{} whole {}/{} us\n",
                candidate as u8,
                times.draw.middle,
                times.draw.worst,
                times.send.middle,
                times.send.worst,
                times.whole.middle,
                times.whole.worst,
            );
            write_line(usb, line.as_str());
        }
        let mut line = Text::<96>::new();
        let _ = write!(
            line,
            "I qday-measure unchanged {}/{} us\n",
            self.unchanged.middle, self.unchanged.worst,
        );
        write_line(usb, line.as_str());
    }
}

fn write_line(usb: &mut UsbSerialJtagTx<'_, Blocking>, line: &str) {
    let started = now_ms();
    for &byte in line.as_bytes() {
        while usb.write_byte_nb(byte).is_err() {
            if now_ms() - started >= WRITE_STALL_MS {
                return;
            }
        }
    }
    let _ = usb.flush_tx_nb();
}
