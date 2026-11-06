//! Running out of heap would crash the device after a long tree build. This measures the real
//! peak through the heaviest flows of both schemes. The framebuffer isn't in it, because on the
//! device it lives in PSRAM, so the firmware's heap only has to cover what's counted here.

// A counting allocator can't be written without this. It's the only place in the project.
#![allow(unsafe_code)]

#[path = "../../qday-wallet/tests/common/mod.rs"]
mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{SimFlash, TestRng};
use qday_ui::app::{App, Host, Light};
use qday_ui::fb::{Framebuffer, PIXELS};
use qday_wallet::counter::FlashCounter;
use qday_wallet::flash::layout;
use qday_wallet::frame::{FrameTx, MlFrameTx};
use qday_wallet::invoice::SpendLog;
use qday_wallet::keystore::KeyStore;
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::{Attempt, Passcode};
use qday_wallet::protocol::{Line, LineBuffer, Request, request_line};
use qday_wallet::settings::{Calibration, Scheme, Settings};
use qday_wallet::wallet::{MAIN_ACCOUNT, Pending, Wallet};

/// The firmware's main heap region. The reclaimed region on top of it is the room for
/// fragmentation, and the peak has to fit in this one alone.
const LIMIT: usize = 96 * 1024;

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

    fn pause(&mut self, _ms: u32) {}

    fn restart(&mut self) {}
}

const CODE: [u8; 6] = [1, 2, 3, 4, 5, 6];

fn start(
    flash: &SimFlash,
    host: &mut NullHost,
    screen: &'static mut [u16],
) -> App<SimFlash, FlashCounter<SimFlash>, TestRng> {
    let fb = Framebuffer::placed(screen);
    let wallet = Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        FlashCounter::new(flash.clone(), layout::COUNTER),
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        SpendLog::new(flash.clone()),
        0,
    )
    .expect("the flash is healthy");
    App::start(
        wallet,
        Settings::new(flash.clone()),
        fb,
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

    // The simulated flash holds a 256 KB buffer of its own, which the device doesn't need, and the
    // screens stand in for PSRAM.
    let [first_screen, second_screen, third_screen] = [(); 3].map(|()| vec![0_u16; PIXELS].leak());
    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);

    let mut host = NullHost(0);
    let mut app = start(&flash, &mut host, first_screen);
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

    let xmss = PEAK.load(Ordering::SeqCst) - baseline;

    // The same device after a restart, switched to ML-DSA and signing.
    drop(app);
    let mut app = start(&flash, &mut host, second_screen);
    let wallet = app.wallet();
    wallet
        .passcode()
        .create(&CODE, &mut TestRng(4))
        .expect("the flash is healthy");
    let Ok(Attempt::Correct(proof)) = wallet.enter_passcode(&CODE, 0) else {
        panic!("the code is right");
    };
    wallet
        .set_scheme(Scheme::MlDsa44, proof)
        .expect("the switch works");
    let raw = MlFrameTx::transfer([0x11; 20], 0, [0x22; 20], 1).encode(&[]);
    let Ok(Pending::MlDsa(review)) = wallet.check_transaction(&raw) else {
        panic!("this is an ML-DSA transfer");
    };
    let Ok(Attempt::Correct(proof)) = wallet.enter_passcode(&CODE, 0) else {
        panic!("the code is right");
    };
    wallet
        .ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(5))
        .expect("it signs");
    drop(app);
    let mut app = start(&flash, &mut host, third_screen);
    let sign = request_line(&Request::SignFrame(raw));
    app.on_line(&mut host, &Line::Text(sign.trim_end()));
    assert_eq!(app.screen_name(), "review");

    let peak = PEAK.load(Ordering::SeqCst) - baseline;
    println!("first boot {first_boot} bytes, XMSS flows {xmss} bytes, all flows {peak} bytes");
    assert!(peak < LIMIT, "peak heap was {peak} bytes");
}
