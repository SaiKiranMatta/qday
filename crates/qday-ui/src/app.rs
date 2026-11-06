//! The device's behaviour: what each press, touch and USB line does.

use alloc::format;
use alloc::string::{String, ToString};

use qday_wallet::activity::{Activity, Entry, Summary};
use qday_wallet::backup::{BackupCheck, Progress, seed_to_words};
use qday_wallet::counter::OneWayCounter;
use qday_wallet::flash::Flash;
use qday_wallet::input::{Approval, Debouncer, Decision, KeyState, Role};
use qday_wallet::passcode::{Attempt, CODE_DIGITS, Code, MAX_FAILURES, Proof, shuffled_digits};
use qday_wallet::protocol::{Line, NextKey, Request, Response, parse_request, response_line};
use qday_wallet::rng::Rng;
use qday_wallet::settings::{CROSSES, Calibration, Settings, ThemeChoice};
use qday_wallet::wallet::{RESTORE_SKIP, Review, Wallet, WalletError};
use qday_xmss::{Hash, LEAVES};

use crate::fb::Framebuffer;
use crate::format::key_number;
use crate::keys::{Action, KeyBar, Tab};
use crate::screens::{ACTIVITY_ROWS, DEVICE_ROWS, Home, Screen, keypad_hit};
use crate::theme::{Theme, theme_for};

/// The status light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    /// Off.
    Off,
    /// Waiting for a decision.
    Amber,
    /// Signed.
    Green,
    /// Refused.
    Red,
}

impl Light {
    /// The colour as (red, green, blue), kept dim.
    #[must_use]
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Off => (0, 0, 0),
            Self::Amber => (40, 18, 0),
            Self::Green => (0, 36, 4),
            Self::Red => (44, 0, 0),
        }
    }
}

/// What the app needs from the hardware.
pub trait Host {
    /// Milliseconds since start.
    fn now_ms(&mut self) -> u64;
    /// Sends the framebuffer's changed rows to the screen.
    fn present(&mut self, fb: &mut Framebuffer);
    /// Sets the status light.
    fn set_light(&mut self, light: Light);
    /// Writes a line to the laptop.
    fn send(&mut self, line: &str);
    /// Restarts the device. On hardware this doesn't return.
    fn restart(&mut self);
}

const RESULT_MS: u64 = 2500;
const PASSCODE_IDLE_MS: u64 = 60_000;
const BACKUP_IDLE_MS: u64 = 120_000;
/// Leaves between screen updates while a tree builds.
const PROGRESS_STEP: u32 = 16;

/// A restore waiting for approval.
#[derive(Clone, Copy)]
struct RestoreJob {
    seed: Hash,
    tree: u32,
    chain_next_index: u32,
}

/// Why a passcode is being entered.
enum Purpose {
    Create,
    CreateConfirm(Code),
    Sign(Review),
    ChangeOld,
    ChangeNew(Proof),
    ChangeConfirm(Proof, Code),
    Backup,
    Restore(RestoreJob),
}

impl Purpose {
    fn title(&self) -> &'static str {
        match self {
            Self::Create | Self::ChangeNew(_) => "New passcode",
            Self::CreateConfirm(_) | Self::ChangeConfirm(..) => "Repeat passcode",
            Self::Sign(_) | Self::ChangeOld | Self::Backup | Self::Restore(_) => "Enter passcode",
        }
    }

    fn hint(&self) -> &'static str {
        match self {
            Self::Create | Self::ChangeNew(_) => "Choose 6 digits",
            Self::CreateConfirm(_) | Self::ChangeConfirm(..) => "Enter it again",
            Self::Sign(_) => "To sign this",
            Self::ChangeOld => "To change it",
            Self::Backup => "To show the words",
            Self::Restore(_) => "To restore",
        }
    }
}

/// What the device is doing.
#[allow(clippy::large_enum_variant, reason = "there is only ever one of these")]
enum Mode {
    Calibrate {
        step: usize,
        raw: [(i32, i32); 3],
    },
    Home(Tab),
    ActivityList {
        top: usize,
    },
    DeviceMenu {
        selected: usize,
    },
    Review {
        review: Review,
        approval: Approval,
    },
    RestoreConfirm {
        job: RestoreJob,
        first_index: u32,
        approval: Approval,
    },
    Passcode {
        purpose: Purpose,
        entered: Code,
        count: usize,
        digits: [u8; 10],
        touched_ms: u64,
        hint: Option<String>,
    },
    Result {
        success: bool,
        title: &'static str,
        detail: String,
        until_ms: u64,
    },
    SeedWords {
        seed: Hash,
        words: [&'static str; 24],
        page: usize,
        touched_ms: u64,
    },
    BackupCheck {
        seed: Hash,
        words: [&'static str; 24],
        check: BackupCheck,
        touched_ms: u64,
    },
    Locked {
        tries_left: u32,
    },
    Notice {
        title: &'static str,
        first: &'static str,
        second: &'static str,
    },
}

fn roles(bar: &KeyBar) -> [Role; 4] {
    bar.keys.map(|key| match key.action {
        Action::Approve => Role::Approve,
        Action::Reject => Role::Reject,
        _ => Role::Ignore,
    })
}

fn seconds_left(approval: &Approval, now_ms: u64) -> u32 {
    u32::try_from(approval.remaining_ms(now_ms).div_ceil(1000)).unwrap_or(0)
}

/// The device.
pub struct App<F, C, R> {
    wallet: Wallet<F, C>,
    settings: Settings<F>,
    rng: R,
    fb: Framebuffer,
    activity: Activity,
    home: Home,
    choice: ThemeChoice,
    calibration: Calibration,
    usb: bool,
    mode: Mode,
    debouncer: Debouncer,
    keys: KeyState,
    touching: bool,
    dirty: bool,
    drawn_tick: u64,
}

impl<F: Flash, C: OneWayCounter, R: Rng> App<F, C, R> {
    /// Starts the device. `calibrate` is true when K1 is held at start.
    pub fn start(
        wallet: Wallet<F, C>,
        mut settings: Settings<F>,
        rng: R,
        calibrate: bool,
        host: &mut impl Host,
    ) -> Self {
        let choice = settings.theme().unwrap_or_default();
        let saved = settings.calibration().ok().flatten();
        let mut app = Self {
            wallet,
            settings,
            rng,
            fb: Framebuffer::new(),
            activity: Activity::default(),
            home: Home {
                key: [0; 32],
                usb: false,
                next_index: 0,
                tree: 0,
                counter: 0,
                version: env!("CARGO_PKG_VERSION"),
                latest: None,
                theme: choice,
                next_ready: false,
            },
            choice,
            calibration: saved.unwrap_or_else(Calibration::factory),
            usb: false,
            mode: Mode::Home(Tab::Wallet),
            debouncer: Debouncer::default(),
            keys: [false; 4],
            touching: false,
            dirty: true,
            drawn_tick: 0,
        };
        if calibrate || saved.is_none() {
            app.mode = Mode::Calibrate {
                step: 0,
                raw: [(0, 0); 3],
            };
        } else {
            app.after_calibration(host);
        }
        app.redraw(host);
        app
    }

    /// Continues the first boot, or goes to the wallet.
    fn after_calibration(&mut self, host: &mut impl Host) {
        if !self.wallet.has_key() {
            let created = {
                let Self {
                    wallet,
                    rng,
                    fb,
                    choice,
                    ..
                } = self;
                let theme = theme_for(*choice);
                wallet.create(rng, |done| {
                    show_progress(fb, theme, &mut *host, "Building keys", done);
                })
            };
            self.mode = Mode::Home(Tab::Wallet);
            if created.is_ok() {
                host.restart();
            } else {
                self.show_notice("Storage failed", "The keys could not be saved.", "");
            }
            return;
        }
        if self.wallet.passcode().exists() == Ok(false) {
            let now = host.now_ms();
            self.ask_passcode(Purpose::Create, None, now);
            return;
        }
        self.go_home(Tab::Wallet);
    }

    fn show_notice(&mut self, title: &'static str, first: &'static str, second: &'static str) {
        self.mode = Mode::Notice {
            title,
            first,
            second,
        };
        self.dirty = true;
    }

    /// Goes to a home tab with fresh figures.
    fn go_home(&mut self, tab: Tab) {
        self.refresh_home();
        self.mode = Mode::Home(tab);
        self.dirty = true;
    }

    fn refresh_home(&mut self) {
        if let Some(active) = self.wallet.active() {
            self.home.key = active.key.root;
            self.home.tree = active.tree;
            self.home.next_index = self.wallet.next_index().unwrap_or(0);
            self.home.counter = self.home.next_index + active.counter_base
                - active.first_index.min(self.home.next_index);
        }
        self.home.usb = self.usb;
        self.home.latest = self.activity.get(0);
        self.home.theme = self.choice;
        self.home.next_ready = self.wallet.next_key_ready().is_some();
    }

    fn ask_passcode(&mut self, purpose: Purpose, hint: Option<String>, now_ms: u64) {
        let digits = shuffled_digits(&mut self.rng);
        self.mode = Mode::Passcode {
            purpose,
            entered: [0; 6],
            count: 0,
            digits,
            touched_ms: now_ms,
            hint,
        };
        self.dirty = true;
    }

    /// Starts entry unless it's locked, in which case the action is refused.
    fn ask_or_lock(&mut self, host: &mut impl Host, purpose: Purpose, now_ms: u64) {
        if self.wallet.passcode().locked_for(now_ms) > 0 {
            self.abandon(host, purpose);
            self.show_locked();
        } else {
            self.ask_passcode(purpose, None, now_ms);
        }
    }

    fn show_locked(&mut self) {
        let failures = self.wallet.passcode().failures().unwrap_or(0);
        self.mode = Mode::Locked {
            tries_left: MAX_FAILURES.saturating_sub(failures),
        };
        self.dirty = true;
    }

    fn respond(host: &mut impl Host, response: &Response) {
        host.send(&response_line(response));
    }

    fn next_pair(&self) -> Option<NextKey> {
        self.wallet
            .next_key_ready()
            .map(|next| (next.key.root, next.key.pub_seed))
    }

    fn show_result(
        &mut self,
        host: &mut impl Host,
        success: bool,
        title: &'static str,
        detail: String,
    ) {
        host.set_light(if success { Light::Green } else { Light::Red });
        let until_ms = host.now_ms() + RESULT_MS;
        self.mode = Mode::Result {
            success,
            title,
            detail,
            until_ms,
        };
        self.dirty = true;
    }

    /// Tells the laptop a signature was refused, and records it.
    fn refuse(&mut self, host: &mut impl Host, review: &Review) {
        self.activity.push(Entry {
            signed: false,
            index: review.tx.index,
            summary: Summary::of(&review.tx.action),
        });
        Self::respond(host, &Response::Err(WalletError::Rejected));
        self.show_result(host, false, "Refused", "Nothing was signed".to_string());
    }

    /// Ends a passcode entry without doing what it was for.
    fn abandon(&mut self, host: &mut impl Host, purpose: Purpose) {
        match purpose {
            Purpose::Sign(review) => self.refuse(host, &review),
            Purpose::Restore(_) => {
                Self::respond(host, &Response::Err(WalletError::Rejected));
                self.show_result(
                    host,
                    false,
                    "Refused",
                    "The keys were not changed".to_string(),
                );
            }
            Purpose::Create | Purpose::CreateConfirm(_) => {
                let now = host.now_ms();
                self.ask_passcode(Purpose::Create, None, now);
            }
            Purpose::ChangeOld
            | Purpose::ChangeNew(_)
            | Purpose::ChangeConfirm(..)
            | Purpose::Backup => {
                self.mode = Mode::DeviceMenu { selected: 0 };
                self.dirty = true;
            }
        }
    }

    fn code_entered(&mut self, host: &mut impl Host, purpose: Purpose, code: Code, now_ms: u64) {
        match purpose {
            Purpose::Create => self.ask_passcode(Purpose::CreateConfirm(code), None, now_ms),
            Purpose::CreateConfirm(first) if first == code => {
                let created = self.wallet.passcode().create(&code, &mut self.rng);
                if created.is_ok() {
                    self.go_home(Tab::Wallet);
                } else {
                    self.show_notice("Storage failed", "The passcode was not saved.", "");
                }
            }
            Purpose::CreateConfirm(_) => {
                let hint = Some("Those did not match".to_string());
                self.ask_passcode(Purpose::Create, hint, now_ms);
            }
            Purpose::ChangeNew(proof) => {
                self.ask_passcode(Purpose::ChangeConfirm(proof, code), None, now_ms);
            }
            Purpose::ChangeConfirm(proof, first) if first == code => {
                let changed = self.wallet.passcode().change(proof, &code, &mut self.rng);
                let (success, title) = if changed.is_ok() {
                    (true, "Passcode changed")
                } else {
                    (false, "Storage failed")
                };
                self.show_result(host, success, title, String::new());
            }
            Purpose::ChangeConfirm(proof, _) => {
                let hint = Some("Those did not match".to_string());
                self.ask_passcode(Purpose::ChangeNew(proof), hint, now_ms);
            }
            purpose => self.check_code(host, purpose, code, now_ms),
        }
    }

    fn check_code(&mut self, host: &mut impl Host, purpose: Purpose, code: Code, now_ms: u64) {
        match self.wallet.enter_passcode(&code, now_ms) {
            Ok(Attempt::Correct(proof)) => self.finish(host, purpose, proof),
            Ok(Attempt::Wrong {
                tries_left,
                wait_ms: 0,
                ..
            }) => {
                let hint = format!("Wrong: {tries_left} tries left");
                self.ask_passcode(purpose, Some(hint), now_ms);
            }
            Ok(Attempt::Wrong { .. } | Attempt::Locked { .. }) => {
                self.abandon(host, purpose);
                self.show_locked();
            }
            Ok(Attempt::Exhausted) => {
                self.abandon(host, purpose);
                self.show_notice("Wallet erased", "Too many wrong passcodes.", "");
                host.restart();
            }
            Err(error) => {
                if let Purpose::Sign(_) | Purpose::Restore(_) = purpose {
                    Self::respond(host, &Response::Err(error));
                }
                self.show_result(host, false, "Storage failed", String::new());
            }
        }
    }

    /// Does what a correct passcode was entered for.
    fn finish(&mut self, host: &mut impl Host, purpose: Purpose, proof: Proof) {
        match purpose {
            Purpose::Sign(review) => self.sign(host, &review, proof),
            Purpose::ChangeOld => {
                let now = host.now_ms();
                self.ask_passcode(Purpose::ChangeNew(proof), None, now);
            }
            Purpose::Backup => match self.wallet.seed(proof) {
                Ok(seed) => {
                    self.mode = Mode::SeedWords {
                        seed,
                        words: seed_to_words(&seed),
                        page: 0,
                        touched_ms: host.now_ms(),
                    };
                    self.dirty = true;
                }
                Err(_) => self.show_result(host, false, "Storage failed", String::new()),
            },
            Purpose::Restore(job) => self.restore(host, job, proof),
            Purpose::Create
            | Purpose::CreateConfirm(_)
            | Purpose::ChangeNew(_)
            | Purpose::ChangeConfirm(..) => {}
        }
    }

    fn sign(&mut self, host: &mut impl Host, review: &Review, proof: Proof) {
        match self.wallet.sign(review, proof) {
            Ok(signature) => {
                let response = Response::Sig {
                    signature: signature.to_vec(),
                    next: self.next_pair(),
                };
                Self::respond(host, &response);
                self.activity.push(Entry {
                    signed: true,
                    index: review.tx.index,
                    summary: Summary::of(&review.tx.action),
                });
                let (grey, plain) = key_number(review.tx.index);
                let detail = format!("Key {grey}{plain} used");
                self.show_result(host, true, "Signed", detail);
            }
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                self.show_result(host, false, "Refused", "Nothing was signed".to_string());
            }
        }
    }

    fn restore(&mut self, host: &mut impl Host, job: RestoreJob, proof: Proof) {
        let result = {
            let Self {
                wallet, fb, choice, ..
            } = self;
            let theme = theme_for(*choice);
            wallet.restore(&job.seed, job.tree, job.chain_next_index, proof, |done| {
                show_progress(fb, theme, &mut *host, "Restoring keys", done);
            })
        };
        match result {
            Ok(()) => {
                Self::respond(host, &Response::Ok);
                self.go_home(Tab::Wallet);
                host.restart();
            }
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                self.show_result(
                    host,
                    false,
                    "Refused",
                    "The keys were not changed".to_string(),
                );
            }
        }
    }

    fn prepare(&mut self, host: &mut impl Host) -> Result<(), WalletError> {
        let Self {
            wallet, fb, choice, ..
        } = self;
        let theme = theme_for(*choice);
        wallet.prepare(|done| show_progress(fb, theme, &mut *host, "Building next keys", done))?;
        Ok(())
    }

    /// Leaves a result, building the next tree first if it's time.
    fn leave_result(&mut self, host: &mut impl Host) {
        host.set_light(Light::Off);
        if self.wallet.should_prepare() == Ok(true) {
            let _ = self.prepare(host);
        }
        self.go_home(Tab::Wallet);
    }

    /// Takes the raw keys and touch, as often as possible.
    pub fn poll(&mut self, host: &mut impl Host, raw: KeyState, touch: Option<(i32, i32)>) {
        let now_ms = host.now_ms();
        let settled = self.debouncer.update(now_ms, raw);
        let previous = self.keys;
        self.keys = settled;
        let pressed = (0..4).find(|&key| settled[key] && !previous[key]);
        let touched = touch.filter(|_| !self.touching);
        self.touching = touch.is_some();

        self.expire(host, now_ms);
        // A press on a signing screen belongs to the approval logic, and must not also act on
        // whatever screen that decision leads to.
        let deciding = matches!(self.mode, Mode::Review { .. } | Mode::RestoreConfirm { .. });
        self.decide(host, now_ms);
        if let Some(raw_touch) = touched {
            self.on_touch(host, raw_touch, now_ms);
        }
        if let (Some(key), false) = (pressed, deciding) {
            let action = self.bar(now_ms).action(key);
            self.act(host, action, now_ms);
        }
        self.redraw(host);
    }

    /// Runs the timers.
    fn expire(&mut self, host: &mut impl Host, now_ms: u64) {
        match &mut self.mode {
            Mode::Result { until_ms, .. } if now_ms >= *until_ms => self.leave_result(host),
            Mode::Passcode {
                touched_ms, count, ..
            } if now_ms.saturating_sub(*touched_ms) >= PASSCODE_IDLE_MS => {
                *touched_ms = now_ms;
                *count = 0;
                self.dirty = true;
                self.cancel_passcode(host);
            }
            Mode::SeedWords { touched_ms, .. } | Mode::BackupCheck { touched_ms, .. }
                if now_ms.saturating_sub(*touched_ms) >= BACKUP_IDLE_MS =>
            {
                self.mode = Mode::DeviceMenu {
                    selected: DEVICE_ROWS - 1,
                };
                self.dirty = true;
            }
            Mode::Locked { .. } if self.wallet.passcode().locked_for(now_ms) == 0 => {
                host.set_light(Light::Off);
                self.go_home(Tab::Wallet);
            }
            _ => {}
        }
    }

    fn cancel_passcode(&mut self, host: &mut impl Host) {
        let mode = core::mem::replace(&mut self.mode, Mode::Home(Tab::Wallet));
        if let Mode::Passcode { purpose, .. } = mode {
            self.abandon(host, purpose);
        }
    }

    /// Asks the approval logic whether the screen on show has been decided.
    fn decide(&mut self, host: &mut impl Host, now_ms: u64) {
        let keys = self.keys;
        let key_roles = roles(&self.bar(now_ms));
        let decision = match &mut self.mode {
            Mode::Review { approval, .. } | Mode::RestoreConfirm { approval, .. } => {
                approval.update(now_ms, keys, key_roles)
            }
            _ => return,
        };
        if decision == Decision::Waiting {
            return;
        }
        let approved = decision == Decision::Approved;
        match core::mem::replace(&mut self.mode, Mode::Home(Tab::Wallet)) {
            Mode::Review { review, .. } if approved => {
                self.ask_or_lock(host, Purpose::Sign(review), now_ms);
            }
            Mode::Review { review, .. } => self.refuse(host, &review),
            Mode::RestoreConfirm { job, .. } if approved => {
                self.ask_or_lock(host, Purpose::Restore(job), now_ms);
            }
            Mode::RestoreConfirm { job, .. } => self.abandon(host, Purpose::Restore(job)),
            _ => {}
        }
    }

    fn on_touch(&mut self, host: &mut impl Host, raw: (i32, i32), now_ms: u64) {
        match &mut self.mode {
            Mode::Calibrate { step, raw: points } => {
                points[*step] = raw;
                *step += 1;
                self.dirty = true;
                if *step == CROSSES.len() {
                    let points = *points;
                    self.finish_calibration(host, points);
                }
            }
            Mode::Passcode { .. } => {
                let (x, y) = self.calibration.to_screen(raw.0, raw.1);
                if let Some(slot) = keypad_hit(x, y) {
                    self.press_digit(host, slot, now_ms);
                }
            }
            _ => {}
        }
    }

    fn finish_calibration(&mut self, host: &mut impl Host, points: [(i32, i32); 3]) {
        let Some(calibration) = Calibration::from_points(points) else {
            self.mode = Mode::Calibrate {
                step: 0,
                raw: [(0, 0); 3],
            };
            return;
        };
        self.calibration = calibration;
        let _ = self.settings.set_calibration(&calibration);
        self.after_calibration(host);
    }

    fn press_digit(&mut self, host: &mut impl Host, slot: usize, now_ms: u64) {
        let Mode::Passcode {
            entered,
            count,
            digits,
            touched_ms,
            ..
        } = &mut self.mode
        else {
            return;
        };
        entered[*count] = digits[slot];
        *count += 1;
        *touched_ms = now_ms;
        self.dirty = true;
        if *count < CODE_DIGITS {
            return;
        }
        let code = *entered;
        let mode = core::mem::replace(&mut self.mode, Mode::Home(Tab::Wallet));
        if let Mode::Passcode { purpose, .. } = mode {
            self.code_entered(host, purpose, code, now_ms);
        }
    }

    /// Does what the key table says a key does.
    fn act(&mut self, host: &mut impl Host, action: Action, now_ms: u64) {
        self.dirty = true;
        match action {
            Action::Tab(tab) => self.choose_tab(tab),
            Action::Back => self.go_back(),
            Action::Up | Action::Down => self.scroll(action == Action::Down),
            Action::Select => self.select(host, now_ms),
            Action::Cancel => self.cancel_passcode(host),
            Action::Delete => {
                if let Mode::Passcode {
                    count, touched_ms, ..
                } = &mut self.mode
                {
                    *count = count.saturating_sub(1);
                    *touched_ms = now_ms;
                }
            }
            Action::Next => {
                if let Mode::SeedWords {
                    page, touched_ms, ..
                } = &mut self.mode
                {
                    *page = 1;
                    *touched_ms = now_ms;
                }
            }
            Action::Done => self.start_check(now_ms),
            Action::Pick(choice) => self.pick(host, usize::from(choice), now_ms),
            Action::Dismiss => self.dismiss(host),
            Action::Nothing | Action::Reject | Action::Approve => {}
        }
    }

    fn choose_tab(&mut self, tab: Tab) {
        match (&self.mode, tab) {
            (Mode::Home(Tab::Activity), Tab::Activity) => {
                self.mode = Mode::ActivityList { top: 0 };
            }
            (Mode::Home(Tab::Device), Tab::Device) => self.mode = Mode::DeviceMenu { selected: 0 },
            _ => self.go_home(tab),
        }
    }

    fn go_back(&mut self) {
        match &mut self.mode {
            Mode::ActivityList { .. } => self.go_home(Tab::Activity),
            Mode::DeviceMenu { .. } => self.go_home(Tab::Device),
            Mode::SeedWords { page: page @ 1, .. } => *page = 0,
            Mode::SeedWords { .. } => {
                self.mode = Mode::DeviceMenu {
                    selected: DEVICE_ROWS - 1,
                };
            }
            _ => {}
        }
    }

    fn scroll(&mut self, down: bool) {
        let entries = self.activity.len();
        match &mut self.mode {
            Mode::ActivityList { top } if down => {
                if *top + ACTIVITY_ROWS < entries {
                    *top += 1;
                }
            }
            Mode::ActivityList { top } => *top = top.saturating_sub(1),
            Mode::DeviceMenu { selected } if down => {
                *selected = (*selected + 1).min(DEVICE_ROWS - 1);
            }
            Mode::DeviceMenu { selected } => *selected = selected.saturating_sub(1),
            _ => {}
        }
    }

    fn select(&mut self, host: &mut impl Host, now_ms: u64) {
        let Mode::DeviceMenu { selected } = self.mode else {
            return;
        };
        match selected {
            0 => {
                self.choice = match self.choice {
                    ThemeChoice::Light => ThemeChoice::Dark,
                    ThemeChoice::Dark => ThemeChoice::Light,
                };
                self.home.theme = self.choice;
                let _ = self.settings.set_theme(self.choice);
            }
            1 => self.ask_or_lock(host, Purpose::ChangeOld, now_ms),
            _ => self.ask_or_lock(host, Purpose::Backup, now_ms),
        }
    }

    fn start_check(&mut self, now_ms: u64) {
        if let Mode::SeedWords { seed, words, .. } = self.mode {
            let check = BackupCheck::new(&seed, &mut self.rng);
            self.mode = Mode::BackupCheck {
                seed,
                words,
                check,
                touched_ms: now_ms,
            };
        }
    }

    fn pick(&mut self, host: &mut impl Host, choice: usize, now_ms: u64) {
        let Mode::BackupCheck {
            seed,
            words,
            check,
            touched_ms,
        } = &mut self.mode
        else {
            return;
        };
        *touched_ms = now_ms;
        match check.pick(choice) {
            Progress::Next => {}
            Progress::Passed => {
                let detail = "Keep the words safe".to_string();
                self.show_result(host, true, "Backup checked", detail);
            }
            Progress::Wrong => {
                self.mode = Mode::SeedWords {
                    seed: *seed,
                    words: *words,
                    page: 0,
                    touched_ms: now_ms,
                };
            }
        }
    }

    fn dismiss(&mut self, host: &mut impl Host) {
        match self.mode {
            Mode::Result { .. } => self.leave_result(host),
            Mode::Notice { .. } | Mode::Locked { .. } => {
                host.set_light(Light::Off);
                self.go_home(Tab::Wallet);
            }
            _ => {}
        }
    }

    /// Whether a new request may take over the screen.
    fn idle(&self) -> bool {
        matches!(
            self.mode,
            Mode::Home(_)
                | Mode::ActivityList { .. }
                | Mode::DeviceMenu { .. }
                | Mode::Result { .. }
                | Mode::Notice { .. }
                | Mode::Locked { .. }
        )
    }

    /// Takes a line from the laptop.
    pub fn on_line(&mut self, host: &mut impl Host, line: &Line<'_>) {
        let Line::Text(text) = *line else {
            Self::respond(host, &Response::Err(WalletError::BadRequest));
            return;
        };
        match parse_request(text) {
            None => {}
            Some(Err(error)) => Self::respond(host, &Response::Err(error)),
            Some(Ok(Request::PubKey)) => self.report_keys(host),
            Some(Ok(_)) if !self.idle() => {
                Self::respond(host, &Response::Err(WalletError::BadRequest));
            }
            Some(Ok(request)) => self.handle(host, request),
        }
        self.redraw(host);
    }

    fn report_keys(&mut self, host: &mut impl Host) {
        let response = match (self.wallet.active(), self.wallet.next_index()) {
            (Some(active), Ok(next_index)) => Response::PubKey {
                root: active.key.root,
                pub_seed: active.key.pub_seed,
                next_index,
                tree: active.tree,
                next: self.next_pair(),
            },
            (_, Err(error)) => Response::Err(error),
            (None, Ok(_)) => Response::Err(WalletError::StorageFailure),
        };
        Self::respond(host, &response);
    }

    fn handle(&mut self, host: &mut impl Host, request: Request) {
        let now_ms = host.now_ms();
        match request {
            Request::PubKey => {}
            Request::SignFrame(raw) => match self.wallet.review(&raw) {
                Ok(review) => {
                    host.set_light(Light::Amber);
                    let approval = Approval::new(now_ms, self.keys);
                    self.mode = Mode::Review { review, approval };
                    self.dirty = true;
                }
                Err(error) => {
                    Self::respond(host, &Response::Err(error));
                    if error == WalletError::RotationNeeded {
                        let second = "Rotate to the next tree to keep signing.";
                        self.show_notice("Rotation needed", "Only reserved keys are left.", second);
                    }
                }
            },
            Request::Prepare => {
                let response = self
                    .prepare(host)
                    .map_or_else(Response::Err, |()| Response::Ok);
                Self::respond(host, &response);
                self.go_home(Tab::Wallet);
            }
            Request::Activate(tree) => match self.wallet.activate(tree) {
                Ok(()) => {
                    Self::respond(host, &Response::Ok);
                    host.restart();
                }
                Err(error) => Self::respond(host, &Response::Err(error)),
            },
            Request::Restore {
                seed,
                tree,
                chain_next_index,
            } => {
                let job = RestoreJob {
                    seed,
                    tree,
                    chain_next_index,
                };
                let Some(first_index) = chain_next_index.checked_add(RESTORE_SKIP) else {
                    Self::respond(host, &Response::Err(WalletError::BadRequest));
                    return;
                };
                host.set_light(Light::Amber);
                let approval = Approval::new(now_ms, self.keys);
                self.mode = Mode::RestoreConfirm {
                    job,
                    first_index,
                    approval,
                };
                self.dirty = true;
            }
        }
    }

    /// Sets whether a laptop is connected.
    pub fn set_usb(&mut self, connected: bool) {
        if self.usb != connected {
            self.usb = connected;
            self.home.usb = connected;
            self.dirty = true;
        }
    }

    fn bar(&self, now_ms: u64) -> KeyBar {
        self.screen(now_ms).keys()
    }

    /// The screen for the current mode.
    fn screen(&self, now_ms: u64) -> Screen<'_> {
        let lock_ms = self.wallet_lock(now_ms);
        screen_for(&self.mode, &self.home, &self.activity, lock_ms, now_ms)
    }

    fn wallet_lock(&self, now_ms: u64) -> u64 {
        self.wallet.locked_for(now_ms)
    }

    /// A number that changes when a countdown on screen needs redrawing.
    fn tick(&self, now_ms: u64) -> u64 {
        match &self.mode {
            Mode::Review { approval, .. } | Mode::RestoreConfirm { approval, .. } => {
                u64::from(seconds_left(approval, now_ms))
            }
            Mode::Locked { .. } => self.wallet_lock(now_ms).div_ceil(1000),
            _ => 0,
        }
    }

    fn redraw(&mut self, host: &mut impl Host) {
        let now_ms = host.now_ms();
        let tick = self.tick(now_ms);
        if !self.dirty && tick == self.drawn_tick {
            return;
        }
        self.dirty = false;
        self.drawn_tick = tick;
        let lock_ms = self.wallet_lock(now_ms);
        let screen = screen_for(&self.mode, &self.home, &self.activity, lock_ms, now_ms);
        screen.draw(&mut self.fb, theme_for(self.choice));
        host.present(&mut self.fb);
    }

    /// A short name for the current screen.
    #[must_use]
    pub fn screen_name(&self) -> &'static str {
        match &self.mode {
            Mode::Calibrate { .. } => "calibrate",
            Mode::Home(Tab::Wallet) => "wallet",
            Mode::Home(Tab::Keys) => "keys",
            Mode::Home(Tab::Activity) => "activity",
            Mode::Home(Tab::Device) => "device",
            Mode::ActivityList { .. } => "activity-opened",
            Mode::DeviceMenu { .. } => "device-opened",
            Mode::Review { .. } => "review",
            Mode::RestoreConfirm { .. } => "restore",
            Mode::Passcode { .. } => "passcode",
            Mode::Result { .. } => "result",
            Mode::SeedWords { .. } => "seed-words",
            Mode::BackupCheck { .. } => "backup-check",
            Mode::Locked { .. } => "locked",
            Mode::Notice { .. } => "notice",
        }
    }

    /// The shuffled keypad, while one is showing.
    #[must_use]
    pub fn keypad(&self) -> Option<[u8; 10]> {
        match &self.mode {
            Mode::Passcode { digits, .. } => Some(*digits),
            _ => None,
        }
    }

    /// The seed words on screen, while a backup is showing.
    #[must_use]
    pub fn backup_words(&self) -> Option<[&'static str; 24]> {
        match &self.mode {
            Mode::SeedWords { words, .. } => Some(*words),
            _ => None,
        }
    }

    /// The choice that answers the backup question on screen.
    #[must_use]
    pub fn backup_answer(&self) -> Option<usize> {
        match &self.mode {
            Mode::BackupCheck { check, .. } => Some(check.question().answer),
            _ => None,
        }
    }

    /// The framebuffer.
    #[must_use]
    pub fn frame(&self) -> &Framebuffer {
        &self.fb
    }

    /// The wallet.
    pub fn wallet(&mut self) -> &mut Wallet<F, C> {
        &mut self.wallet
    }
}

/// The screen for a mode. A free function, so drawing can borrow the framebuffer alongside it.
fn screen_for<'a>(
    mode: &'a Mode,
    home: &'a Home,
    activity: &'a Activity,
    lock_ms: u64,
    now_ms: u64,
) -> Screen<'a> {
    match mode {
        Mode::Calibrate { step, .. } => Screen::Calibrate {
            step: (*step).min(2),
        },
        Mode::Home(tab) => Screen::Tab(*tab, home, activity),
        Mode::ActivityList { top } => Screen::ActivityList(activity, *top),
        Mode::DeviceMenu { selected } => Screen::DeviceMenu(home, *selected),
        Mode::Review { review, approval } => {
            Screen::Review(&review.tx, seconds_left(approval, now_ms))
        }
        Mode::RestoreConfirm {
            job,
            first_index,
            approval,
        } => Screen::RestoreConfirm {
            tree: job.tree,
            first_index: *first_index,
            seconds: seconds_left(approval, now_ms),
        },
        Mode::Passcode {
            purpose,
            count,
            digits,
            hint,
            ..
        } => Screen::Passcode {
            title: purpose.title(),
            hint: hint.as_deref().unwrap_or_else(|| purpose.hint()),
            entered: *count,
            digits,
        },
        Mode::Result {
            success,
            title,
            detail,
            ..
        } => Screen::Result {
            success: *success,
            title,
            detail,
        },
        Mode::SeedWords { words, page, .. } => Screen::SeedWords { words, page: *page },
        Mode::BackupCheck { check, .. } => {
            let question = check.question();
            Screen::BackupCheck {
                number: check.number(),
                position: question.position,
                choices: question.choices,
            }
        }
        Mode::Locked { tries_left } => Screen::Locked {
            wait_ms: lock_ms,
            tries_left: *tries_left,
        },
        Mode::Notice {
            title,
            first,
            second,
        } => Screen::Notice {
            title,
            first,
            second,
        },
    }
}

/// Draws a progress screen straight away, because tree building blocks the main loop.
fn show_progress(
    fb: &mut Framebuffer,
    theme: &Theme,
    host: &mut impl Host,
    title: &str,
    done: u32,
) {
    if done % PROGRESS_STEP == 0 {
        Screen::Progress {
            title,
            done,
            total: LEAVES,
        }
        .draw(fb, theme);
        host.present(fb);
    }
}
