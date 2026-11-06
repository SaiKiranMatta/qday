//! The device has 256 KB of heap, and running out would crash it after a long tree build. This
//! measures the real peak through the heaviest flows.

// A counting allocator can't be written without this. It's the only place in the project.
#![allow(unsafe_code)]

#[path = "../../qday-wallet/tests/common/mod.rs"]
mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{SimFlash, TestRng};
use qday_ui::app::{App, Host, Light};
use qday_ui::fb::Framebuffer;
use qday_wallet::counter::FlashCounter;
use qday_wallet::flash::layout;
use qday_wallet::frame::FrameTx;
use qday_wallet::keystore::KeyStore;
use qday_wallet::passcode::Passcode;
use qday_wallet::protocol::{Line, LineBuffer, Request, request_line};
use qday_wallet::settings::{Calibration, Settings};
use qday_wallet::wallet::Wallet;

/// What the firmware gives the heap, less room for fragmentation.
const LIMIT: usize = 240 * 1024;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: every call is passed straight to the system allocator.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = CURRENT.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
        PEAK.fetch_max(now, Ordering::SeqCst);
        // SAFETY: the caller's obligations are the same.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::SeqCst);
        // SAFETY: the caller's obligations are the same.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct NullHost(u64);

impl Host for NullHost {
    fn now_ms(&mut self) -> u64 {
        self.0
    }

    fn present(&mut self, fb: &mut Framebuffer) {
        fb.send_changes(|_, _, _| {});
    }

    fn set_light(&mut self, _light: Light) {}

    fn send(&mut self, _line: &str) {}

    fn restart(&mut self) {}
}

fn start(flash: &SimFlash, host: &mut NullHost) -> App<SimFlash, FlashCounter<SimFlash>, TestRng> {
    let wallet = Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        FlashCounter::new(flash.clone(), layout::COUNTER),
        0,
    )
    .expect("the flash is healthy");
    App::start(
        wallet,
        Settings::new(flash.clone()),
        TestRng(3),
        false,
        host,
    )
}

/// One test, because the counters are shared across the whole binary.
#[test]
fn the_heaviest_flows_fit_in_the_heap() {
    let flash = SimFlash::new();
    Settings::new(flash.clone())
        .set_calibration(&Calibration::factory())
        .expect("the flash is healthy");

    // The simulated flash holds a 256 KB buffer of its own, which the device doesn't need.
    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);

    let mut host = NullHost(0);
    let mut app = start(&flash, &mut host);
    assert!(app.wallet().has_key(), "first boot builds the keys");
    let first_boot = PEAK.load(Ordering::SeqCst) - baseline;

    let mut line_buffer = LineBuffer::default();
    let prepare = request_line(&Request::Prepare);
    for byte in prepare.bytes() {
        if let Some(line) = line_buffer.push(byte) {
            app.on_line(&mut host, &line);
        }
    }
    assert!(app.wallet().next_key_ready().is_some());

    let raw = FrameTx::transfer([0x11; 20], 0, 0, [0x22; 20], 1).encode(&[]);
    let sign = request_line(&Request::SignFrame(raw));
    app.on_line(&mut host, &Line::Text(sign.trim_end()));

    let peak = PEAK.load(Ordering::SeqCst) - baseline;
    println!("first boot {first_boot} bytes, all flows {peak} bytes");
    assert!(peak < LIMIT, "peak heap was {peak} bytes");
}
