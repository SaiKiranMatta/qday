//! The whole device, with a simulated flash and hardware.

#[path = "../../qday-wallet/tests/common/mod.rs"]
mod common;

use std::sync::OnceLock;

use common::{SimFlash, TestRng};
use qday_ui::app::{App, Host, Light, Shown};
use qday_ui::fb::Framebuffer;
use qday_ui::screens::Review;
use qday_ui::widgets::{digit_in_slot, key_rect};
use qday_wallet::counter::{FlashCounter, OneWayCounter};
use qday_wallet::flash::layout;
use qday_wallet::frame::{Action, FrameTx};
use qday_wallet::hex;
use qday_wallet::keystore::KeyStore;
use qday_wallet::passcode::{Code, Passcode};
use qday_wallet::protocol::{Line, Request, Response, parse_response, request_line};
use qday_wallet::settings::{CROSSES, Calibration, Settings};
use qday_wallet::wallet::{Wallet, WalletError};
use qday_xmss::{PublicKey, verify};

const CODE: Code = [1, 2, 3, 4, 5, 6];
const OTHER: Code = [6, 5, 4, 3, 2, 1];
const WRONG: Code = [9, 9, 9, 9, 9, 9];
const SENDER: [u8; 20] = [0x11; 20];
const TO: [u8; 20] = [0x22; 20];
const NONE: [bool; 4] = [false; 4];
/// Raw touch readings that land on the three crosses with the factory calibration.
const RAW_CROSSES: [(i32, i32); 3] = [(523, 567), (523, 3673), (3523, 567)];

#[derive(Default)]
struct FakeHost {
    now: u64,
    light: Option<Light>,
    sent: Vec<String>,
    restarts: u32,
    frames: u32,
}

impl Host for FakeHost {
    fn now_ms(&mut self) -> u64 {
        self.now
    }

    fn present(&mut self, fb: &mut Framebuffer) {
        self.frames += 1;
        fb.send_changes(|_, _, _| {});
    }

    fn set_light(&mut self, light: Light) {
        self.light = Some(light);
    }

    fn send(&mut self, line: &str) {
        self.sent.push(line.to_string());
    }

    fn pause(&mut self, ms: u32) {
        self.now += u64::from(ms);
    }

    fn restart(&mut self) {
        self.restarts += 1;
    }
}

type TestApp = App<SimFlash, FlashCounter<SimFlash>, TestRng>;

struct Rig {
    flash: SimFlash,
    host: FakeHost,
    app: TestApp,
}

fn boot(flash: &SimFlash, seed: u64, hold_k1: bool) -> (FakeHost, TestApp) {
    let mut host = FakeHost::default();
    let wallet = Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        FlashCounter::new(flash.clone(), layout::COUNTER),
        0,
    )
    .expect("the flash is healthy");
    let settings = Settings::new(flash.clone());
    let app = App::start(wallet, settings, TestRng(seed), hold_k1, &mut host);
    (host, app)
}

/// A provisioned image with a key, a calibration and the passcode, built once.
fn image() -> &'static Vec<u8> {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let flash = SimFlash::new();
        let mut wallet = Wallet::open(
            KeyStore::new(flash.clone()),
            Passcode::new(flash.clone(), layout::PASSCODE),
            FlashCounter::new(flash.clone(), layout::COUNTER),
            0,
        )
        .expect("the flash is healthy");
        wallet
            .create(&mut TestRng(7), |_| {})
            .expect("the flash is healthy");
        wallet
            .passcode()
            .create(&CODE, &mut TestRng(8))
            .expect("the flash is healthy");
        Settings::new(flash.clone())
            .set_calibration(&Calibration::factory())
            .expect("the flash is healthy");
        flash.image()
    })
}

fn rig() -> Rig {
    let flash = SimFlash::new();
    flash.poke(0, image());
    let (host, app) = boot(&flash, 11, false);
    Rig { flash, host, app }
}

/// Inverts the factory calibration, to find the raw reading for a pixel.
fn raw_for(x: i32, y: i32) -> (i32, i32) {
    let raw_y = 567 + (x - 20) * (3673 - 567) / 280;
    let raw_x = 523 + (y - 20) * (3523 - 523) / 200;
    (raw_x, raw_y)
}

impl Rig {
    /// Lets `ms` pass with nothing held.
    fn wait(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(20).max(1) {
            self.host.now += 20;
            self.app.poll(&mut self.host, NONE, None);
        }
    }

    /// Holds a key long enough to count, then releases it.
    fn press(&mut self, key: usize) {
        let mut keys = NONE;
        keys[key] = true;
        for _ in 0..3 {
            self.host.now += 20;
            self.app.poll(&mut self.host, keys, None);
        }
        self.wait(60);
    }

    fn touch(&mut self, raw: (i32, i32)) {
        self.host.now += 20;
        self.app.poll(&mut self.host, NONE, Some(raw));
        self.host.now += 20;
        self.app.poll(&mut self.host, NONE, None);
    }

    fn tap_digit(&mut self, digit: u8) {
        let pad = self.app.keypad().expect("a keypad is showing");
        let slot = (0..12)
            .find(|&slot| digit_in_slot(&pad, slot) == Some(digit))
            .expect("every digit is there");
        let rect = key_rect(slot);
        self.touch(raw_for((rect.x + 25.0) as i32, (rect.y + 20.0) as i32));
    }

    fn enter(&mut self, code: Code) {
        for digit in code {
            self.tap_digit(digit);
        }
    }

    fn request(&mut self, request: &Request) {
        let line = request_line(request);
        self.app
            .on_line(&mut self.host, &Line::Text(line.trim_end()));
    }

    fn last_response(&self) -> Response {
        let line = self.host.sent.last().expect("something was sent");
        parse_response(line.trim_end()).expect("the line is valid")
    }

    fn sign_request(&mut self, index: u32) {
        let raw = FrameTx::transfer(SENDER, 1, index, TO, 1000).encode(&[]);
        self.request(&Request::SignFrame(raw));
    }

    fn counter(&self) -> u32 {
        FlashCounter::new(self.flash.clone(), layout::COUNTER)
            .value()
            .expect("it reads")
    }

    fn screen(&self) -> &'static str {
        self.app.screen_name()
    }

    /// Waits out the guard, approves, and enters the code.
    fn approve(&mut self, code: Code) {
        self.wait(520);
        self.press(3);
        assert_eq!(self.screen(), "passcode");
        self.enter(code);
    }
}

// First boot

#[test]
fn a_new_device_calibrates_builds_keys_and_restarts() {
    let flash = SimFlash::new();
    let (host, app) = boot(&flash, 1, false);
    let mut rig = Rig { flash, host, app };
    assert_eq!(rig.screen(), "calibrate");
    for raw in RAW_CROSSES {
        rig.touch(raw);
    }
    assert_eq!(
        rig.host.restarts, 1,
        "keys are built, then the device restarts"
    );
    assert!(rig.app.wallet().has_key());
    let saved = Settings::new(rig.flash.clone())
        .calibration()
        .expect("it reads");
    assert!(saved.is_some());
}

#[test]
fn after_the_restart_it_asks_for_a_new_passcode_twice() {
    let flash = SimFlash::new();
    let (host, app) = boot(&flash, 1, false);
    let mut first = Rig { flash, host, app };
    for raw in RAW_CROSSES {
        first.touch(raw);
    }
    let (host, app) = boot(&first.flash, 2, false);
    let mut rig = Rig {
        flash: first.flash,
        host,
        app,
    };
    assert_eq!(rig.screen(), "passcode");
    rig.enter(CODE);
    assert_eq!(rig.screen(), "passcode");
    rig.enter(CODE);
    rig.wait(40);
    assert_eq!(rig.screen(), "wallet");
    assert_eq!(rig.app.wallet().passcode().exists(), Ok(true));
}

#[test]
fn a_passcode_that_does_not_match_starts_again() {
    let flash = SimFlash::new();
    flash.poke(0, image());
    Passcode::new(flash.clone(), layout::PASSCODE)
        .wipe()
        .expect("the flash is healthy");
    let (host, app) = boot(&flash, 2, false);
    let mut rig = Rig { flash, host, app };
    rig.enter(CODE);
    rig.enter(OTHER);
    assert_eq!(rig.screen(), "passcode");
    assert_eq!(rig.app.wallet().passcode().exists(), Ok(false));
    rig.wait(2600);
    rig.enter(OTHER);
    rig.enter(OTHER);
    assert_eq!(rig.app.wallet().passcode().exists(), Ok(true));
}

#[test]
fn a_provisioned_device_goes_straight_to_the_wallet() {
    assert_eq!(rig().screen(), "wallet");
}

#[test]
fn holding_k1_at_start_runs_calibration() {
    let flash = SimFlash::new();
    flash.poke(0, image());
    let (host, app) = boot(&flash, 2, true);
    let mut rig = Rig { flash, host, app };
    assert_eq!(rig.screen(), "calibrate");
    for raw in RAW_CROSSES {
        rig.touch(raw);
    }
    assert_eq!(rig.screen(), "wallet");
}

#[test]
fn the_crosses_match_the_calibration_points() {
    assert_eq!(CROSSES, [(30, 30), (290, 30), (30, 210)]);
    assert_eq!(raw_for(20, 20), RAW_CROSSES[0]);
}

// Signing

#[test]
fn an_approved_transfer_gives_a_valid_signature() {
    let mut rig = rig();
    let active = rig.app.wallet().active().expect("a key is active");
    let key: PublicKey = active.key;
    rig.sign_request(0);
    assert_eq!(rig.screen(), "review");
    assert_eq!(rig.host.light, Some(Light::Amber));

    rig.approve(CODE);
    let Response::Sig { signature, next } = rig.last_response() else {
        panic!("expected a signature, got {:?}", rig.host.sent);
    };
    assert_eq!(next, None);
    let message = FrameTx::transfer(SENDER, 1, 0, TO, 1000).sig_hash();
    assert!(verify(&key, &message, &signature));
    assert_eq!(rig.counter(), 1);
    assert_eq!(rig.host.light, Some(Light::Green));
    assert_eq!(rig.screen(), "result");
}

#[test]
fn k1_and_k2_reject_and_the_key_stays_unused() {
    for key in [0, 1] {
        let mut rig = rig();
        rig.sign_request(0);
        rig.wait(520);
        rig.press(key);
        assert_eq!(rig.last_response(), Response::Err(WalletError::Rejected));
        assert_eq!(rig.counter(), 0);
        assert_eq!(rig.host.light, Some(Light::Red));
    }
}

#[test]
fn k3_does_nothing_on_a_signing_screen() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(520);
    rig.press(2);
    assert_eq!(rig.screen(), "review");
    assert!(rig.host.sent.is_empty());
}

#[test]
fn a_press_in_the_first_500_ms_is_ignored() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.press(3);
    assert_eq!(rig.screen(), "review");
}

#[test]
fn a_key_held_when_the_review_appears_must_be_released() {
    let mut rig = rig();
    let held = [false, false, false, true];
    for _ in 0..3 {
        rig.host.now += 20;
        rig.app.poll(&mut rig.host, held, None);
    }
    rig.sign_request(0);
    for _ in 0..50 {
        rig.host.now += 20;
        rig.app.poll(&mut rig.host, held, None);
    }
    assert_eq!(rig.screen(), "review");
}

#[test]
fn no_decision_in_30_seconds_rejects() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(29_900);
    assert_eq!(rig.screen(), "review");
    rig.wait(200);
    assert_eq!(rig.last_response(), Response::Err(WalletError::Rejected));
    assert_eq!(rig.counter(), 0);
}

#[test]
fn cancelling_the_passcode_rejects() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(520);
    rig.press(3);
    rig.press(0);
    assert_eq!(rig.last_response(), Response::Err(WalletError::Rejected));
    assert_eq!(rig.counter(), 0);
}

#[test]
fn passcode_entry_gives_up_after_60_seconds_idle() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(520);
    rig.press(3);
    rig.wait(59_800);
    assert_eq!(rig.screen(), "passcode");
    rig.wait(400);
    assert_eq!(rig.last_response(), Response::Err(WalletError::Rejected));
}

#[test]
fn touching_a_gap_enters_nothing() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(520);
    rig.press(3);
    let first = key_rect(0);
    rig.touch(raw_for((first.x + first.w + 2.0) as i32, 20));
    rig.enter(CODE);
    assert!(matches!(rig.last_response(), Response::Sig { .. }));
}

#[test]
fn delete_removes_the_last_digit() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(520);
    rig.press(3);
    rig.tap_digit(9);
    rig.press(2);
    rig.enter(CODE);
    assert!(matches!(rig.last_response(), Response::Sig { .. }));
}

#[test]
fn a_wrong_passcode_can_be_tried_again() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.approve(WRONG);
    assert_eq!(rig.screen(), "passcode");
    assert!(rig.host.sent.is_empty());
    rig.enter(CODE);
    assert!(matches!(rig.last_response(), Response::Sig { .. }));
}

#[test]
fn the_keypad_is_shuffled_for_every_entry() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(520);
    rig.press(3);
    let first = rig.app.keypad().expect("a keypad is showing");
    rig.enter(WRONG);
    let second = rig.app.keypad().expect("a keypad is showing");
    assert_ne!(first, second);
}

#[test]
fn three_wrong_codes_lock_entry_and_reject() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.approve(WRONG);
    rig.enter(WRONG);
    rig.enter(WRONG);
    assert_eq!(rig.screen(), "locked");
    assert_eq!(rig.last_response(), Response::Err(WalletError::Rejected));
    assert_eq!(rig.counter(), 0);
}

#[test]
fn signing_while_locked_goes_to_the_locked_screen() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.approve(WRONG);
    rig.enter(WRONG);
    rig.enter(WRONG);
    rig.press(0);
    rig.sign_request(0);
    rig.wait(520);
    rig.press(3);
    assert_eq!(rig.screen(), "locked");
    assert_eq!(rig.last_response(), Response::Err(WalletError::Rejected));
}

#[test]
fn the_tenth_failure_erases_and_restarts() {
    let mut rig = rig();
    for round in 0..10 {
        rig.sign_request(0);
        rig.approve(WRONG);
        if round < 9 && rig.screen() == "passcode" {
            rig.press(0);
        }
        rig.wait(3_700_000);
    }
    assert!(!rig.app.wallet().has_key());
    assert_eq!(rig.host.restarts, 1);
}

#[test]
fn the_result_clears_after_2_5_seconds() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.approve(CODE);
    rig.wait(2300);
    assert_eq!(rig.screen(), "result");
    rig.wait(300);
    assert_eq!(rig.screen(), "wallet");
    assert_eq!(rig.host.light, Some(Light::Off));
}

#[test]
fn any_key_dismisses_a_result() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.approve(CODE);
    rig.press(1);
    assert_eq!(rig.screen(), "wallet");
}

// Bad requests

#[test]
fn a_bad_transaction_gets_its_error_and_nothing_changes() {
    let mut rig = rig();
    rig.request(&Request::SignFrame(vec![1, 2, 3]));
    assert_eq!(rig.last_response(), Response::Err(WalletError::BadRequest));
    assert_eq!(rig.screen(), "wallet");
}

#[test]
fn a_transaction_for_the_wrong_key_is_refused_without_a_screen() {
    let mut rig = rig();
    rig.sign_request(5);
    assert_eq!(rig.last_response(), Response::Err(WalletError::BadRequest));
    assert_eq!(rig.screen(), "wallet");
}

#[test]
fn a_second_request_during_a_review_is_refused_and_the_review_stands() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.sign_request(0);
    assert_eq!(rig.last_response(), Response::Err(WalletError::BadRequest));
    assert_eq!(rig.screen(), "review");
    rig.host.sent.clear();
    rig.approve(CODE);
    assert!(matches!(rig.last_response(), Response::Sig { .. }));
}

#[test]
fn log_lines_and_overlong_lines_are_handled() {
    let mut rig = rig();
    rig.app
        .on_line(&mut rig.host, &Line::Text("I (20) boot: something"));
    assert!(rig.host.sent.is_empty());
    rig.app.on_line(&mut rig.host, &Line::Bad);
    assert_eq!(rig.last_response(), Response::Err(WalletError::BadRequest));
}

// Other requests

#[test]
fn pubkey_reports_the_keys() {
    let mut rig = rig();
    rig.request(&Request::PubKey);
    let active = rig.app.wallet().active().expect("a key is active");
    assert_eq!(
        rig.last_response(),
        Response::PubKey {
            root: active.key.root,
            pub_seed: active.key.pub_seed,
            next_index: 0,
            tree: 0,
            next: None,
        }
    );
}

#[test]
fn prepare_then_pubkey_reports_the_next_key() {
    let mut rig = rig();
    rig.request(&Request::Prepare);
    assert_eq!(rig.last_response(), Response::Ok);
    rig.request(&Request::PubKey);
    let Response::PubKey { next: Some(_), .. } = rig.last_response() else {
        panic!("the next key must be reported");
    };
}

#[test]
fn a_rotation_signs_and_activate_restarts() {
    let mut rig = rig();
    rig.request(&Request::Prepare);
    let next = rig
        .app
        .wallet()
        .next_key_ready()
        .expect("the tree is ready")
        .key;
    let raw = FrameTx::rotation(SENDER, 1, 0, next.root, next.pub_seed).encode(&[]);
    rig.request(&Request::SignFrame(raw));
    rig.approve(CODE);
    let Response::Sig {
        next: Some(reported),
        ..
    } = rig.last_response()
    else {
        panic!("a signature with the next key is expected");
    };
    assert_eq!(reported, (next.root, next.pub_seed));
    rig.wait(2600);

    rig.request(&Request::Activate(1));
    assert_eq!(rig.last_response(), Response::Ok);
    assert_eq!(rig.host.restarts, 1);
}

#[test]
fn activate_with_nothing_prepared_is_refused() {
    let mut rig = rig();
    rig.request(&Request::Activate(1));
    assert_eq!(
        rig.last_response(),
        Response::Err(WalletError::NothingPending)
    );
    assert_eq!(rig.host.restarts, 0);
}

fn restore_request() -> Request {
    Request::Restore {
        seed: [0x77; 32],
        tree: 1,
        chain_next_index: 100,
    }
}

#[test]
fn an_approved_restore_replaces_the_keys_and_restarts() {
    let mut rig = rig();
    rig.request(&restore_request());
    assert_eq!(rig.screen(), "restore");
    rig.approve(CODE);
    assert_eq!(rig.last_response(), Response::Ok);
    assert_eq!(rig.host.restarts, 1);
    assert_eq!(rig.app.wallet().next_index(), Ok(164));
}

#[test]
fn a_rejected_restore_changes_nothing() {
    let mut rig = rig();
    let before = rig.app.wallet().active();
    rig.request(&restore_request());
    rig.wait(520);
    rig.press(0);
    assert_eq!(rig.last_response(), Response::Err(WalletError::Rejected));
    assert_eq!(rig.app.wallet().active(), before);
}

#[test]
fn the_next_tree_builds_itself_from_key_900() {
    let mut rig = rig();
    let mut counter = FlashCounter::new(rig.flash.clone(), layout::COUNTER);
    for _ in 0..899 {
        counter.advance().expect("the flash is healthy");
    }
    rig.sign_request(899);
    rig.approve(CODE);
    rig.wait(2600);
    assert!(rig.app.wallet().next_key_ready().is_some());
}

#[test]
fn an_amount_too_long_to_show_is_refused() {
    let mut rig = rig();
    let raw = FrameTx::transfer(SENDER, 1, 0, TO, 12_345_678_901_234_567_890_123_456).encode(&[]);
    rig.request(&Request::SignFrame(raw));
    assert_eq!(rig.last_response(), Response::Err(WalletError::BadRequest));
    assert_eq!(rig.counter(), 0);
    assert_eq!(rig.screen(), "notice");
}

#[test]
fn the_longest_transfer_amount_is_shown() {
    let tx = FrameTx::transfer(SENDER, 1, 0, TO, 8_888_888_888_888_888_888);
    assert!(Shown::from_tx(&tx).is_some());
    let tx = FrameTx::transfer(SENDER, 1, 0, TO, 18_888_888_888_888_888_888);
    assert!(Shown::from_tx(&tx).is_none());
}

#[test]
fn an_unknown_call_with_a_long_value_is_refused() {
    let call = |value| {
        let mut tx = FrameTx::transfer(SENDER, 1, 0, TO, 0);
        tx.action = Action::Other {
            target: Some(TO),
            value,
            data: vec![1, 2, 3, 4],
            execution: 50_000,
            state: 200_000,
        };
        tx
    };
    assert!(Shown::from_tx(&call(123_456_789_000_000_000)).is_some());
    assert!(Shown::from_tx(&call(1_234_567_891_000_000_000)).is_some());
    assert!(Shown::from_tx(&call(1_234_567_890_120_000_000)).is_none());
}

#[test]
fn a_fee_too_long_to_show_is_refused() {
    let mut tx = FrameTx::transfer(SENDER, 1, 0, TO, 1);
    assert!(Shown::from_tx(&tx).is_some());
    tx.max_fee = 2_000_000_001;
    assert!(Shown::from_tx(&tx).is_none());
}

#[test]
fn a_transfer_shows_the_exact_amount_and_checksummed_address() {
    let tx = FrameTx::transfer(SENDER, 1, 0, TO, 1_234_567_890_123_456_789);
    let shown = Shown::from_tx(&tx).expect("this fits");
    let Review::Send { amount, unit, to } = shown.review() else {
        panic!("a transfer shows as Send");
    };
    assert_eq!((amount, unit), ("1.234567890123456789", "ETH"));
    assert_eq!(to.len(), 42);
    assert_eq!(to.to_lowercase(), format!("0x{}", "22".repeat(20)));
}

#[test]
fn a_rotation_shows_as_a_call_and_is_not_marked_risky() {
    let tx = FrameTx::rotation(SENDER, 1, 0, [1; 32], [2; 32]);
    let shown = Shown::from_tx(&tx).expect("this fits");
    assert!(matches!(shown.review(), Review::Call { .. }));
    assert!(!shown.review().is_risky());
}

#[test]
fn an_unknown_call_warns_with_target_value_and_selector() {
    let mut tx = FrameTx::transfer(SENDER, 1, 0, TO, 0);
    tx.action = Action::Other {
        target: Some(TO),
        value: 5_000_000_000_000_000,
        data: vec![0xa9, 0x05, 0x9c, 0xbb, 1],
        execution: 50_000,
        state: 200_000,
    };
    let shown = Shown::from_tx(&tx).expect("this fits");
    let review = shown.review();
    assert!(review.is_risky());
    let Review::UnknownCall {
        contract,
        value,
        function,
    } = review
    else {
        panic!("an unknown call shows its details");
    };
    assert_eq!(contract.to_lowercase(), format!("0x{}", "22".repeat(20)));
    assert_eq!(value, "0.005 ETH");
    assert_eq!(function, "0xa9059cbb");
}

// Tabs

#[test]
fn keys_switch_tabs() {
    let mut rig = rig();
    for (key, name) in [(1, "keys"), (2, "activity"), (3, "device"), (0, "wallet")] {
        rig.press(key);
        assert_eq!(rig.screen(), name);
    }
}

#[test]
fn pressing_the_current_tab_opens_activity_and_device() {
    let mut rig = rig();
    rig.press(2);
    rig.press(2);
    assert_eq!(rig.screen(), "activity-opened");
    rig.press(0);
    assert_eq!(rig.screen(), "activity");
    rig.press(3);
    rig.press(3);
    assert_eq!(rig.screen(), "device-opened");
    rig.press(0);
    assert_eq!(rig.screen(), "device");
}

#[test]
fn pressing_wallet_or_keys_again_opens_nothing() {
    let mut rig = rig();
    rig.press(0);
    assert_eq!(rig.screen(), "wallet");
    rig.press(1);
    rig.press(1);
    assert_eq!(rig.screen(), "keys");
}

fn open_device(rig: &mut Rig) {
    rig.press(3);
    rig.press(3);
}

#[test]
fn the_theme_changes_and_is_saved() {
    let mut rig = rig();
    open_device(&mut rig);
    rig.press(3);
    let saved = Settings::new(rig.flash.clone()).theme();
    assert_eq!(saved, Ok(1));
    assert_eq!(rig.screen(), "device-opened");
}

#[test]
fn the_passcode_can_be_changed() {
    let mut rig = rig();
    open_device(&mut rig);
    rig.press(2);
    rig.press(3);
    assert_eq!(rig.screen(), "passcode");
    rig.enter(CODE);
    rig.enter(OTHER);
    rig.enter(OTHER);
    rig.wait(2600);

    rig.sign_request(0);
    rig.approve(OTHER);
    assert!(matches!(rig.last_response(), Response::Sig { .. }));
}

fn open_backup(rig: &mut Rig) {
    open_device(rig);
    rig.press(2);
    rig.press(2);
    rig.press(3);
    rig.enter(CODE);
    assert_eq!(rig.screen(), "seed-words");
}

#[test]
fn backing_up_needs_the_passcode() {
    let mut rig = rig();
    open_device(&mut rig);
    rig.press(2);
    rig.press(2);
    rig.press(3);
    assert_eq!(rig.screen(), "passcode");
    assert_eq!(rig.app.backup_words(), None);
}

#[test]
fn the_backup_shows_the_real_words() {
    let mut rig = rig();
    open_backup(&mut rig);
    let words = rig.app.backup_words().expect("the words are showing");
    let phrase = words.join(" ");
    let seed = qday_wallet::backup::words_to_seed(&phrase).expect("the words are valid");
    assert!(rig.flash.contains(&seed));
}

#[test]
fn right_answers_finish_the_backup() {
    let mut rig = rig();
    open_backup(&mut rig);
    rig.press(3);
    rig.press(3);
    assert_eq!(rig.screen(), "backup-check");
    for _ in 0..3 {
        let answer = rig.app.backup_answer().expect("a question is showing");
        rig.press(answer);
    }
    assert_eq!(rig.screen(), "result");
}

#[test]
fn a_wrong_answer_returns_to_the_words() {
    let mut rig = rig();
    open_backup(&mut rig);
    rig.press(3);
    rig.press(3);
    let answer = rig.app.backup_answer().expect("a question is showing");
    rig.press((answer + 1) % 4);
    assert_eq!(rig.screen(), "seed-words");
}

#[test]
fn the_words_leave_the_screen_after_2_minutes_idle() {
    let mut rig = rig();
    open_backup(&mut rig);
    rig.wait(119_800);
    assert_eq!(rig.screen(), "seed-words");
    rig.wait(400);
    assert_ne!(rig.screen(), "seed-words");
    assert_eq!(rig.app.backup_words(), None);
}

#[test]
fn a_request_during_a_backup_is_refused() {
    let mut rig = rig();
    open_backup(&mut rig);
    rig.sign_request(0);
    assert_eq!(rig.last_response(), Response::Err(WalletError::BadRequest));
    assert_eq!(rig.screen(), "seed-words");
}

#[test]
fn the_activity_log_records_signatures_and_rejections() {
    let mut rig = rig();
    rig.sign_request(0);
    rig.wait(520);
    rig.press(0);
    rig.wait(2600);
    rig.sign_request(0);
    rig.approve(CODE);
    rig.wait(2600);
    rig.press(2);
    assert_eq!(rig.screen(), "activity");
    assert_ne!(rig.app.frame().pixel(40, 40), 0);
}

#[test]
fn nothing_redraws_when_nothing_changed() {
    let mut rig = rig();
    rig.wait(100);
    let before = rig.host.frames;
    rig.wait(2000);
    assert_eq!(rig.host.frames, before);
}

#[test]
fn hex_is_used_on_the_wire() {
    let mut rig = rig();
    rig.request(&Request::PubKey);
    assert!(rig.host.sent[0].contains(&hex::encode(
        &rig.app.wallet().active().expect("a key").key.root
    )));
}
