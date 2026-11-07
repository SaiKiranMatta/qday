//! The device's behaviour: what each press, touch and USB line does.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use qday_wallet::backup::{BackupCheck, Progress, seed_to_words};
use qday_wallet::counter::OneWayCounter;
use qday_wallet::flash::Flash;
use qday_wallet::frame::{Action as TxAction, Address, CHAIN_ID, FrameTx, MlFrameTx};
use qday_wallet::input::{APPROVAL_TIMEOUT_MS, Approval, Debouncer, Decision, KeyState, Role};
use qday_wallet::lockdown::Fuses;
use qday_wallet::passcode::{Attempt, CODE_DIGITS, Code, MAX_FAILURES, Proof, shuffled_digits};
use qday_wallet::protocol::{Line, NextKey, Request, Response, parse_request, response_line};
use qday_wallet::rng::Rng;
use qday_wallet::settings::{CROSSES, Calibration, Scheme, Settings};
use qday_wallet::stealth::StealthSpend;
use qday_wallet::tokens;
use qday_wallet::wallet::{
    MAIN_ACCOUNT, MlReview, Pending, RESTORE_SKIP, Review, Wallet, WalletError,
};
use qday_xmss::{Hash, LEAVES, keccak256};

use crate::canvas::Canvas;
use crate::fb::Framebuffer;
use crate::format::{checksum_address, eth, short_address, token_amount, wait_text};
use crate::nav::{self, Action, Keys, Scroll, Setting};
use crate::screens::{
    self, ACTIVITY_ROWS, Activity, Coin, DeviceInfo, Passcode, ReviewInfo, ShareInfo, Status,
    other_scheme, scheme_label,
};
use crate::theme::{self, Theme};
use crate::widgets::{Tab, digit_in_slot, key_rect};

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
    /// Waits, so a message stays on screen.
    fn pause(&mut self, ms: u32);
    /// Restarts the device. On hardware this doesn't return.
    fn restart(&mut self);
    /// Reads the chip's security fuses. Hardware with none to read reports a new chip.
    fn fuses(&mut self) -> Fuses {
        Fuses::default()
    }
}

const RESULT_MS: u64 = 2500;
const PASSCODE_IDLE_MS: u64 = 60_000;
const BACKUP_IDLE_MS: u64 = 120_000;
/// How long a message stays up before a restart.
const RESTART_PAUSE_MS: u32 = 1500;
/// Leaves between screen updates while a tree builds.
const PROGRESS_STEP: u32 = 16;
/// Entries kept for the Activity tab.
const ACTIVITY_KEPT: usize = 20;
/// The longest values the review screen shows whole in every theme, with their unit. Anything
/// longer is refused, because a screen must never show part of what's being signed.
const LONGEST_AMOUNT: usize = 24;
const LONGEST_CALL_VALUE: usize = 16;
const LONGEST_FEE: usize = 12;
const FIRMWARE: &str = concat!("qday ", env!("CARGO_PKG_VERSION"));

/// A restore waiting for approval.
#[derive(Clone, Copy)]
struct RestoreJob {
    seed: Hash,
    tree: u32,
    chain_next_index: u32,
}

/// One line of the Activity tab.
struct LogEntry {
    approved: bool,
    title: String,
    detail: String,
    index: Option<u32>,
    coin: Coin,
}

/// What a transaction moves, for the activity row's logo.
fn coin_of(tx: &FrameTx) -> Coin {
    match &tx.action {
        TxAction::Transfer { .. } | TxAction::StealthPay { .. } => Coin::Eth,
        TxAction::TokenTransfer { token, .. } => {
            tokens::known(token).map_or(Coin::UnknownToken, |t| Coin::Token(t.symbol))
        }
        _ => Coin::None,
    }
}

impl LogEntry {
    fn item(&self) -> Activity<'_> {
        Activity {
            approved: self.approved,
            title: &self.title,
            detail: &self.detail,
            index: self.index,
            coin: self.coin,
        }
    }
}

/// What the review screen shows, all decoded on the device.
pub struct Shown {
    kind: ShownKind,
    a: String,
    b: String,
    c: String,
    fee: String,
    /// What `a` is counted in, for sends: ETH or a known token's symbol.
    unit: &'static str,
    rotation: bool,
    /// Short lines for the Activity tab.
    summary: String,
    detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShownKind {
    Send,
    /// A token the device doesn't know: raw units and the contract.
    Token,
    Call,
    UnknownCall,
}

fn selector_text(data: &[u8]) -> String {
    match data {
        [a, b, c, d, ..] => format!("0x{a:02x}{b:02x}{c:02x}{d:02x}"),
        _ => "Plain call".to_string(),
    }
}

impl Shown {
    /// Describes a two-frame transfer. `None` if it can't be shown whole, so it mustn't be signed.
    #[must_use]
    pub fn from_ml_tx(tx: &MlFrameTx) -> Option<Self> {
        let fee = eth(tx.max_fee_wei());
        let amount = eth(tx.value);
        if fee.len() > LONGEST_FEE || amount.len() > LONGEST_AMOUNT {
            return None;
        }
        Some(Self {
            kind: ShownKind::Send,
            summary: if tx.note.is_some() {
                format!("Paid stealth note {amount}")
            } else {
                format!("Sent {amount}")
            },
            detail: format!("to {}", short_address(&tx.to)),
            a: amount.trim_end_matches(" ETH").to_string(),
            b: checksum_address(&tx.to),
            c: String::new(),
            fee,
            rotation: false,
            unit: "ETH",
        })
    }

    /// Describes a spend of a stealth note. `None` if it can't be shown whole.
    #[must_use]
    pub fn from_stealth(spend: &StealthSpend) -> Option<Self> {
        let fee = eth(spend.fee);
        if fee.len() > LONGEST_FEE {
            return None;
        }
        Some(Self {
            kind: ShownKind::Send,
            summary: "Spent stealth note".to_string(),
            detail: format!("to {}", short_address(&spend.to)),
            a: "Whole note".to_string(),
            b: checksum_address(&spend.to),
            c: String::new(),
            fee,
            rotation: false,
            unit: "",
        })
    }

    /// Describes a transaction. `None` if it can't be shown whole, so it mustn't be signed.
    #[must_use]
    pub fn from_tx(tx: &FrameTx) -> Option<Self> {
        let fee = eth(tx.max_fee_wei());
        if fee.len() > LONGEST_FEE {
            return None;
        }
        match &tx.action {
            TxAction::Transfer { to, value } => {
                let amount = eth(*value);
                if amount.len() > LONGEST_AMOUNT {
                    return None;
                }
                Some(Self {
                    kind: ShownKind::Send,
                    summary: format!("Sent {amount}"),
                    detail: format!("to {}", short_address(to)),
                    a: amount.trim_end_matches(" ETH").to_string(),
                    b: checksum_address(to),
                    c: String::new(),
                    fee,
                    unit: "ETH",
                    rotation: false,
                })
            }
            TxAction::StealthPay { pool, value, .. } => {
                let amount = eth(*value);
                if amount.len() > LONGEST_AMOUNT {
                    return None;
                }
                Some(Self {
                    kind: ShownKind::Send,
                    summary: format!("Paid stealth note {amount}"),
                    detail: format!("to {}", short_address(pool)),
                    a: amount.trim_end_matches(" ETH").to_string(),
                    b: checksum_address(pool),
                    c: String::new(),
                    fee,
                    unit: "ETH",
                    rotation: false,
                })
            }
            TxAction::TokenTransfer { token, to, amount } => {
                // A token on the device's own list reads like a send; any other shows its raw units.
                if let Some(known) = tokens::known(token) {
                    let shown = token_amount(*amount, known.decimals, known.symbol);
                    if shown.len() > LONGEST_AMOUNT {
                        return None;
                    }
                    return Some(Self {
                        kind: ShownKind::Send,
                        summary: format!("Sent {shown}"),
                        detail: format!("to {}", short_address(to)),
                        a: shown.trim_end_matches(known.symbol).trim_end().to_string(),
                        b: checksum_address(to),
                        c: String::new(),
                        fee,
                        unit: known.symbol,
                        rotation: false,
                    });
                }
                let raw = amount.to_string();
                if raw.len() > LONGEST_AMOUNT {
                    return None;
                }
                Some(Self {
                    kind: ShownKind::Token,
                    summary: "Token transfer".to_string(),
                    detail: format!("to {}", short_address(to)),
                    a: raw,
                    b: checksum_address(to),
                    c: checksum_address(token),
                    fee,
                    unit: "",
                    rotation: false,
                })
            }
            TxAction::Create { factory, .. } => Some(Self {
                kind: ShownKind::Call,
                a: checksum_address(factory),
                b: eth(0),
                c: "Create account".to_string(),
                fee,
                unit: "ETH",
                rotation: false,
                summary: "Created account".to_string(),
                detail: format!("at {}", short_address(&tx.sender)),
            }),
            TxAction::Rotate { .. } => Some(Self {
                kind: ShownKind::Call,
                a: checksum_address(&tx.sender),
                b: eth(0),
                c: "Rotate keys".to_string(),
                fee,
                unit: "ETH",
                rotation: true,
                summary: "Rotated keys".to_string(),
                detail: "to the next tree".to_string(),
            }),
            TxAction::Other {
                target,
                value,
                data,
                ..
            } => {
                let amount = eth(*value);
                if amount.len() > LONGEST_CALL_VALUE {
                    return None;
                }
                let target: Address = target.unwrap_or(tx.sender);
                let function = selector_text(data);
                Some(Self {
                    kind: ShownKind::UnknownCall,
                    summary: "Unknown call".to_string(),
                    detail: format!("{function} at {}", short_address(&target)),
                    a: checksum_address(&target),
                    b: amount,
                    c: function,
                    fee,
                    unit: "ETH",
                    rotation: false,
                })
            }
        }
    }

    /// The screen's view of this.
    #[must_use]
    pub fn review(&self) -> screens::Review<'_> {
        match self.kind {
            ShownKind::Send => screens::Review::Send {
                amount: &self.a,
                unit: self.unit,
                to: &self.b,
            },
            ShownKind::Token => screens::Review::Token {
                amount: &self.a,
                to: &self.b,
                token: &self.c,
            },
            ShownKind::Call => screens::Review::Call {
                contract: &self.a,
                value: &self.b,
                function: &self.c,
            },
            ShownKind::UnknownCall => screens::Review::UnknownCall {
                contract: &self.a,
                value: &self.b,
                function: &self.c,
            },
        }
    }
}

/// Why a passcode is being entered.
#[allow(clippy::large_enum_variant, reason = "there is only ever one of these")]
enum Purpose {
    Create,
    CreateConfirm(Code),
    Sign(Pending, Shown),
    Scheme(Scheme),
    ViewKey,
    ChangeOld,
    ChangeNew(Proof),
    ChangeConfirm(Proof, Code),
    Backup,
    Restore(RestoreJob),
}

impl Purpose {
    fn title(&self) -> &'static str {
        match self {
            Self::Create => "Create a passcode",
            Self::ChangeNew(_) => "New passcode",
            Self::CreateConfirm(_) | Self::ChangeConfirm(..) => "Enter it again",
            Self::Sign(..)
            | Self::Scheme(_)
            | Self::ViewKey
            | Self::ChangeOld
            | Self::Backup
            | Self::Restore(_) => "Enter passcode",
        }
    }

    fn hint(&self) -> String {
        match self {
            Self::Create | Self::ChangeNew(_) => "6 digits".to_string(),
            Self::CreateConfirm(_) | Self::ChangeConfirm(..) => "to confirm".to_string(),
            Self::Sign(Pending::Xmss(review), _) => format!("for key #{:04}", review.tx().index),
            Self::Sign(_, _) => "to sign".to_string(),
            Self::ViewKey => "to share the view key".to_string(),
            Self::Scheme(_) => "to switch signing".to_string(),
            Self::ChangeOld => "to change it".to_string(),
            Self::Backup => "to show the words".to_string(),
            Self::Restore(_) => "to restore".to_string(),
        }
    }
}

/// What's about to be handed to the laptop.
#[derive(Clone, Copy)]
enum Sharing {
    Address,
    ViewKey,
}

/// What the device is doing.
#[allow(clippy::large_enum_variant, reason = "there is only ever one of these")]
enum Mode {
    Calibrate {
        step: usize,
        raw: [(i32, i32); 3],
        retry: bool,
    },
    Home(Tab),
    ActivityList {
        top: usize,
    },
    DeviceMenu {
        setting: Setting,
    },
    Review {
        review: Pending,
        shown: Shown,
        approval: Approval,
    },
    SchemeConfirm {
        target: Scheme,
        approval: Approval,
    },
    ShareConfirm {
        what: Sharing,
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
        error: Option<String>,
    },
    Result {
        success: bool,
        title: String,
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
        detail: &'static str,
    },
}

/// What the tabs show, refreshed when something changes rather than on every frame.
struct View {
    status: Status,
    root: Hash,
    counter: u32,
    next_ready: bool,
    log: Vec<LogEntry>,
}

fn roles(keys: &Keys) -> [Role; 4] {
    keys.map(|key| match key.map(|key| key.action) {
        Some(Action::Approve) => Role::Approve,
        Some(Action::Reject) => Role::Reject,
        _ => Role::Ignore,
    })
}

fn seconds_left(approval: &Approval, now_ms: u64) -> u32 {
    u32::try_from(approval.remaining_ms(now_ms).div_ceil(1000)).unwrap_or(0)
}

const TIMEOUT_SECONDS: u32 = (APPROVAL_TIMEOUT_MS / 1000) as u32;

/// What K1 to K4 do in a mode. The screens draw from the same `nav` tables.
fn keys_for(mode: &Mode) -> Keys {
    const NONE: Keys = [None; 4];
    match mode {
        Mode::Home(tab) => nav::tab_keys(*tab, false, Setting::Theme),
        Mode::ActivityList { .. } => nav::tab_keys(Tab::Activity, true, Setting::Theme),
        Mode::DeviceMenu { setting } => nav::tab_keys(Tab::Device, true, *setting),
        Mode::Review { .. }
        | Mode::SchemeConfirm { .. }
        | Mode::ShareConfirm { .. }
        | Mode::RestoreConfirm { .. } => nav::review_keys(),
        Mode::Passcode { .. } => nav::passcode_keys(),
        Mode::SeedWords { page, .. } => nav::seed_keys(*page),
        Mode::BackupCheck { check, .. } => nav::check_keys(&check.question().choices),
        Mode::Calibrate { .. }
        | Mode::Result { .. }
        | Mode::Locked { .. }
        | Mode::Notice { .. } => NONE,
    }
}

/// Draws a mode. A free function, so drawing can borrow the framebuffer alongside it.
fn draw(
    c: &mut Canvas,
    mode: &Mode,
    view: &View,
    lock_ms: u64,
    now_ms: u64,
    flash: Option<usize>,
    slot: Option<usize>,
) {
    let status = view.status;
    let device = DeviceInfo {
        firmware: FIRMWARE,
        root: &view.root,
        counter: view.counter,
    };
    let items = || view.log.iter().map(LogEntry::item).collect::<Vec<_>>();
    match mode {
        Mode::Calibrate { step, retry, .. } => {
            screens::calibrate(c, (*step).min(2), *retry, "");
        }
        Mode::Home(Tab::Home) => {
            let latest = view.log.first().map(LogEntry::item);
            screens::home(c, &status, &view.root, latest.as_ref(), flash);
        }
        Mode::Home(Tab::Keys) if status.scheme == Scheme::MlDsa44 => {
            screens::keys_ml(c, &status, &view.root, flash);
        }
        Mode::Home(Tab::Keys) => screens::keys(c, &status, view.next_ready, flash),
        Mode::Home(Tab::Activity) => screens::activity(c, &status, &items(), 0, false, flash),
        Mode::Home(Tab::Device) => {
            screens::device(c, &status, &device, Setting::Theme, false, flash);
        }
        Mode::ActivityList { top } => screens::activity(c, &status, &items(), *top, true, flash),
        Mode::DeviceMenu { setting } => screens::device(c, &status, &device, *setting, true, flash),
        Mode::Review {
            review,
            shown,
            approval,
        } => {
            let (scheme, index) = match review {
                Pending::Xmss(review) => (Scheme::Xmss, review.tx().index),
                Pending::MlDsa(_) => (Scheme::MlDsa44, 0),
                Pending::Stealth(_) => (Scheme::Xmss, 0),
            };
            let info = ReviewInfo {
                review: shown.review(),
                fee: &shown.fee,
                chain_id: CHAIN_ID,
                scheme,
                signature: match review {
                    Pending::Stealth(_) => Some("One-time ML-DSA"),
                    Pending::Xmss(_) | Pending::MlDsa(_) => None,
                },
                index,
                rotation: shown.rotation,
                seconds_left: seconds_left(approval, now_ms),
                timeout: TIMEOUT_SECONDS,
            };
            screens::review(c, &info, flash);
        }
        Mode::SchemeConfirm { target, approval } => {
            let seconds = seconds_left(approval, now_ms);
            screens::scheme_confirm(c, *target, seconds, TIMEOUT_SECONDS, flash);
        }
        Mode::ShareConfirm { what, approval } => {
            let (title, detail, gives, passcode) = match what {
                Sharing::Address => (
                    "Stealth address",
                    "Lets anyone pay you privately",
                    "your public address",
                    false,
                ),
                Sharing::ViewKey => (
                    "View key",
                    "Lets the laptop find your payments",
                    "your view key",
                    true,
                ),
            };
            let info = ShareInfo {
                title,
                detail,
                gives,
                passcode,
                seconds_left: seconds_left(approval, now_ms),
                timeout: TIMEOUT_SECONDS,
            };
            screens::share_confirm(c, &info, flash);
        }
        Mode::RestoreConfirm {
            job,
            first_index,
            approval,
        } => {
            let seconds = seconds_left(approval, now_ms);
            screens::restore_confirm(c, job.tree, *first_index, seconds, TIMEOUT_SECONDS, flash);
        }
        Mode::Passcode {
            purpose,
            count,
            digits,
            error,
            ..
        } => {
            let hint = purpose.hint();
            let pc = Passcode {
                title: purpose.title(),
                hint: &hint,
                entered: *count,
                error: error.as_deref(),
                digits,
                pressed: slot,
            };
            screens::passcode(c, &pc, flash);
        }
        Mode::Result {
            success,
            title,
            detail,
            ..
        } => screens::result(c, *success, title, detail),
        Mode::SeedWords { words, page, .. } => screens::seed_words(c, words, *page, flash),
        Mode::BackupCheck { check, .. } => {
            let question = check.question();
            let number = check.number();
            screens::seed_check(
                c,
                number,
                question.position,
                &question.choices,
                false,
                flash,
            );
        }
        Mode::Locked { tries_left } => screens::locked(c, &wait_text(lock_ms), *tries_left),
        Mode::Notice { title, detail } => screens::notice(c, title, detail),
    }
}

/// The keypad slot under a pixel.
fn slot_at(digits: &[u8; 10], x: i32, y: i32) -> Option<usize> {
    #[allow(clippy::cast_precision_loss, reason = "screen positions are small")]
    let (x, y) = (x as f32, y as f32);
    (0..12).find(|&slot| {
        let rect = key_rect(slot);
        let inside = x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h;
        inside && digit_in_slot(digits, slot).is_some()
    })
}

/// The device.
pub struct App<F, C, R> {
    wallet: Wallet<F, C>,
    settings: Settings<F>,
    rng: R,
    fb: Framebuffer,
    view: View,
    calibration: Calibration,
    mode: Mode,
    debouncer: Debouncer,
    keys: KeyState,
    touching: bool,
    touched_slot: Option<usize>,
    dirty: bool,
    drawn_tick: u64,
}

impl<F: Flash, C: OneWayCounter, R: Rng> App<F, C, R> {
    /// Starts the device. `calibrate` is true when K1 is held at start.
    pub fn start(
        wallet: Wallet<F, C>,
        mut settings: Settings<F>,
        fb: Framebuffer,
        rng: R,
        calibrate: bool,
        host: &mut impl Host,
    ) -> Self {
        let saved = settings.theme().ok().and_then(Theme::from_u8);
        theme::set(saved.unwrap_or(Theme::StandardLight));
        let calibration = settings.calibration().ok().flatten();
        let mut app = Self {
            wallet,
            settings,
            rng,
            fb,
            view: View {
                status: Status {
                    tree: 0,
                    next_index: 0,
                    connected: false,
                    scheme: Scheme::Xmss,
                },
                root: [0; 32],
                counter: 0,
                next_ready: false,
                log: Vec::new(),
            },
            calibration: calibration.unwrap_or_else(Calibration::factory),
            mode: Mode::Home(Tab::Home),
            debouncer: Debouncer::default(),
            keys: [false; 4],
            touching: false,
            touched_slot: None,
            dirty: true,
            drawn_tick: 0,
        };
        if calibrate || calibration.is_none() {
            app.mode = Mode::Calibrate {
                step: 0,
                raw: [(0, 0); 3],
                retry: false,
            };
        } else {
            app.after_calibration(host);
        }
        app.redraw(host);
        app
    }

    /// Continues the first boot, or goes to the wallet.
    fn after_calibration(&mut self, host: &mut impl Host) {
        // On a sealed device the passcode comes first, so keys with no passcode can't be normal.
        if self.wallet.is_sealed() && self.wallet.passcode().exists() == Ok(false) {
            let now = host.now_ms();
            self.ask_passcode(Purpose::Create, None, now);
            return;
        }
        if !self.wallet.has_key() {
            let lines = [
                "Building your 1,024 keys.",
                "Once per key tree,",
                "about 3 minutes.",
            ];
            let created = {
                let Self {
                    wallet, rng, fb, ..
                } = self;
                wallet.create(rng, |done| {
                    show_progress(fb, &mut *host, "Setting up", &lines, done);
                })
            };
            self.mode = Mode::Home(Tab::Home);
            if created.is_ok() {
                host.restart();
            } else {
                self.show_notice("Device error", "The keys could not be saved.");
            }
            return;
        }
        if self.wallet.passcode().exists() == Ok(false) {
            let now = host.now_ms();
            self.ask_passcode(Purpose::Create, None, now);
            return;
        }
        self.go_home(Tab::Home);
    }

    fn show_notice(&mut self, title: &'static str, detail: &'static str) {
        self.mode = Mode::Notice { title, detail };
        self.dirty = true;
    }

    /// Goes to a home tab with fresh figures.
    fn go_home(&mut self, tab: Tab) {
        self.refresh_view();
        self.mode = Mode::Home(tab);
        self.dirty = true;
    }

    fn refresh_view(&mut self) {
        if let Some(active) = self.wallet.active() {
            let next_index = self.wallet.next_index().unwrap_or(0);
            self.view.root = active.key.root;
            self.view.status.tree = active.tree;
            self.view.status.next_index = next_index;
            self.view.counter = active.counter_base + next_index.saturating_sub(active.first_index);
        }
        self.view.status.scheme = self.wallet.scheme();
        if self.wallet.scheme() == Scheme::MlDsa44 {
            // A key lost to a power cut shows as zeros until the next signature rebuilds it.
            self.view.root = self
                .wallet
                .ml_public_key(MAIN_ACCOUNT)
                .map_or([0; 32], |key| keccak256(&[&key]));
        }
        self.view.next_ready = self.wallet.next_key_ready().is_some();
    }

    fn ask_passcode(&mut self, purpose: Purpose, error: Option<String>, now_ms: u64) {
        let digits = shuffled_digits(&mut self.rng);
        self.mode = Mode::Passcode {
            purpose,
            entered: [0; 6],
            count: 0,
            digits,
            touched_ms: now_ms,
            error,
        };
        self.dirty = true;
    }

    /// Starts entry unless it's locked, in which case the action is refused.
    fn ask_or_lock(&mut self, host: &mut impl Host, purpose: Purpose, now_ms: u64) {
        if self.wallet.locked_for(now_ms) > 0 {
            self.abandon(host, purpose, false);
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

    fn remember(
        &mut self,
        approved: bool,
        title: String,
        detail: String,
        index: Option<u32>,
        coin: Coin,
    ) {
        self.view.log.insert(
            0,
            LogEntry {
                approved,
                title,
                detail,
                index,
                coin,
            },
        );
        self.view.log.truncate(ACTIVITY_KEPT);
    }

    fn show_result(&mut self, host: &mut impl Host, success: bool, title: &str, detail: String) {
        host.set_light(if success { Light::Green } else { Light::Red });
        let until_ms = host.now_ms() + RESULT_MS;
        self.mode = Mode::Result {
            success,
            title: title.to_string(),
            detail,
            until_ms,
        };
        self.dirty = true;
    }

    /// Tells the laptop a signature was refused, and records it.
    fn refuse(&mut self, host: &mut impl Host, pending: &Pending, timed_out: bool) {
        let (title, why) = if timed_out {
            ("Timed out", "No decision in 30 s")
        } else {
            ("Rejected", "You rejected it")
        };
        Self::respond(host, &Response::Err(WalletError::Rejected));
        let detail = match pending {
            Pending::Xmss(review) => {
                format!(
                    "No signature. Key #{:04} is still unused.",
                    review.tx().index
                )
            }
            Pending::MlDsa(_) | Pending::Stealth(_) => "No signature was made.".to_string(),
        };
        self.show_result(host, false, title, detail);
        let coin = match pending {
            Pending::Xmss(review) => coin_of(review.tx()),
            Pending::MlDsa(_) | Pending::Stealth(_) => Coin::Eth,
        };
        self.remember(false, title.to_string(), why.to_string(), None, coin);
    }

    /// Ends a passcode entry without doing what it was for.
    fn abandon(&mut self, host: &mut impl Host, purpose: Purpose, timed_out: bool) {
        match purpose {
            Purpose::Sign(pending, _) => self.refuse(host, &pending, timed_out),
            Purpose::Scheme(_) => self.leave_switch(host),
            Purpose::ViewKey => {
                Self::respond(host, &Response::Err(WalletError::Rejected));
                let detail = "The view key stayed on the device.".to_string();
                self.show_result(host, false, "Not shared", detail);
            }
            Purpose::Restore(_) => {
                Self::respond(host, &Response::Err(WalletError::Rejected));
                let detail = "The device's keys are unchanged.".to_string();
                self.show_result(host, false, "Restore cancelled", detail);
            }
            Purpose::Create | Purpose::CreateConfirm(_) => {
                let now = host.now_ms();
                self.ask_passcode(Purpose::Create, Some("Start again".to_string()), now);
            }
            Purpose::ChangeOld
            | Purpose::ChangeNew(_)
            | Purpose::ChangeConfirm(..)
            | Purpose::Backup => {
                self.mode = Mode::DeviceMenu {
                    setting: Setting::Theme,
                };
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
                    self.after_calibration(host);
                } else {
                    self.show_notice("Device error", "Saving the passcode failed.");
                }
            }
            Purpose::CreateConfirm(_) => {
                let error = Some("Codes didn't match".to_string());
                self.ask_passcode(Purpose::Create, error, now_ms);
            }
            Purpose::ChangeNew(proof) => {
                self.ask_passcode(Purpose::ChangeConfirm(proof, code), None, now_ms);
            }
            Purpose::ChangeConfirm(proof, first) if first == code => {
                let changed = self.wallet.passcode().change(proof, &code, &mut self.rng);
                if changed.is_ok() {
                    let detail = "Asked for every signature".to_string();
                    self.show_result(host, true, "Passcode changed", detail);
                } else {
                    self.show_result(host, false, "Device error", "Nothing changed".to_string());
                }
            }
            Purpose::ChangeConfirm(proof, _) => {
                let error = Some("Codes didn't match".to_string());
                self.ask_passcode(Purpose::ChangeNew(proof), error, now_ms);
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
                let word = if tries_left == 1 { "try" } else { "tries" };
                let error = format!("Wrong code, {tries_left} {word} left");
                self.ask_passcode(purpose, Some(error), now_ms);
            }
            Ok(Attempt::Wrong { .. } | Attempt::Locked { .. }) => {
                self.abandon(host, purpose, false);
                self.show_locked();
            }
            Ok(Attempt::Exhausted) => {
                self.abandon(host, purpose, false);
                self.show_result(
                    host,
                    false,
                    "Keys erased",
                    "Too many wrong passcodes".to_string(),
                );
                self.redraw(host);
                host.pause(RESTART_PAUSE_MS);
                host.restart();
            }
            Err(error) => {
                if let Purpose::Sign(..) | Purpose::Restore(_) | Purpose::ViewKey = purpose {
                    Self::respond(host, &Response::Err(error));
                }
                self.show_result(host, false, "Device error", "Storage failed".to_string());
            }
        }
    }

    /// Does what a correct passcode was entered for.
    fn finish(&mut self, host: &mut impl Host, purpose: Purpose, proof: Proof) {
        match purpose {
            Purpose::Sign(Pending::Xmss(review), shown) => {
                self.sign(host, &review, &shown, proof);
            }
            Purpose::Sign(Pending::MlDsa(review), shown) => {
                self.ml_sign(host, &review, &shown, proof);
            }
            Purpose::Sign(Pending::Stealth(spend), shown) => {
                self.spend_stealth(host, &spend, &shown, proof);
            }
            Purpose::ViewKey => self.hand_over_view_key(host, proof),
            Purpose::Scheme(target) => self.switch_scheme(host, target, proof),
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
                Err(_) => {
                    self.show_result(host, false, "Device error", "Storage failed".to_string());
                }
            },
            Purpose::Restore(job) => self.restore(host, job, proof),
            Purpose::Create
            | Purpose::CreateConfirm(_)
            | Purpose::ChangeNew(_)
            | Purpose::ChangeConfirm(..) => {}
        }
    }

    fn sign(&mut self, host: &mut impl Host, review: &Review, shown: &Shown, proof: Proof) {
        let index = review.tx().index;
        match self.wallet.sign(review, proof) {
            Ok(signature) => {
                let response = Response::Sig {
                    signature: signature.to_vec(),
                    next: self.next_pair(),
                };
                Self::respond(host, &response);
                self.remember(
                    true,
                    shown.summary.clone(),
                    shown.detail.clone(),
                    Some(index),
                    coin_of(review.tx()),
                );
                self.show_result(host, true, "Signed", format!("Key #{index:04} used"));
            }
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                let detail = format!("No signature. Key #{index:04} is still unused.");
                self.show_result(host, false, "Not signed", detail);
            }
        }
    }

    fn ml_sign(&mut self, host: &mut impl Host, review: &MlReview, shown: &Shown, proof: Proof) {
        // Signing can take about 0.3 s, so say so before the wait.
        self.show_notice("Signing...", scheme_label(Scheme::MlDsa44));
        self.redraw(host);
        match self
            .wallet
            .ml_sign(review, MAIN_ACCOUNT, proof, &mut self.rng)
        {
            Ok(signature) => {
                let response = Response::Sig {
                    signature: signature.to_vec(),
                    next: None,
                };
                Self::respond(host, &response);
                self.remember(
                    true,
                    shown.summary.clone(),
                    shown.detail.clone(),
                    None,
                    Coin::Eth,
                );
                let detail = format!("{} signature sent", scheme_label(Scheme::MlDsa44));
                self.show_result(host, true, "Signed", detail);
            }
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                let detail = "No signature was made.".to_string();
                self.show_result(host, false, "Not signed", detail);
            }
        }
    }

    fn spend_stealth(
        &mut self,
        host: &mut impl Host,
        spend: &StealthSpend,
        shown: &Shown,
        proof: Proof,
    ) {
        self.show_notice("Signing...", "One-time ML-DSA");
        self.redraw(host);
        match self
            .wallet
            .sign_stealth(spend, MAIN_ACCOUNT, proof, &mut self.rng)
        {
            Ok(signature) => {
                let response = Response::Sig {
                    signature: signature.to_vec(),
                    next: None,
                };
                Self::respond(host, &response);
                self.remember(
                    true,
                    shown.summary.clone(),
                    shown.detail.clone(),
                    None,
                    Coin::Eth,
                );
                self.show_result(host, true, "Signed", "Stealth note spent".to_string());
            }
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                let detail = "No signature was made.".to_string();
                self.show_result(host, false, "Not signed", detail);
            }
        }
    }

    fn hand_over_view_key(&mut self, host: &mut impl Host, proof: Proof) {
        match self.wallet.view_seed(MAIN_ACCOUNT, proof) {
            Ok(view) => {
                Self::respond(host, &Response::ViewKey(view));
                let detail = "It can find payments, not spend them.".to_string();
                self.show_result(host, true, "View key shared", detail);
            }
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                let detail = "The view key stayed on the device.".to_string();
                self.show_result(host, false, "Not shared", detail);
            }
        }
    }

    /// Hands over what doesn't need a passcode.
    fn hand_over(&mut self, host: &mut impl Host, what: Sharing, now_ms: u64) {
        let response = match what {
            Sharing::Address => self
                .wallet
                .meta_address(MAIN_ACCOUNT)
                .map_or_else(Response::Err, |meta| {
                    Response::StealthAddress(meta.to_vec())
                }),
            Sharing::ViewKey => {
                self.ask_or_lock(host, Purpose::ViewKey, now_ms);
                return;
            }
        };
        Self::respond(host, &response);
        host.set_light(Light::Off);
        self.go_home(Tab::Home);
    }

    fn switch_scheme(&mut self, host: &mut impl Host, target: Scheme, proof: Proof) {
        match self.wallet.set_scheme(target, proof) {
            Ok(()) => {
                let detail = format!("Now signing with {}", scheme_label(target));
                self.show_result(host, true, "Switched", detail);
            }
            Err(_) => {
                let detail = "Signing has not changed.".to_string();
                self.show_result(host, false, "Not switched", detail);
            }
        }
    }

    /// Goes back to the menu without switching.
    fn leave_switch(&mut self, host: &mut impl Host) {
        host.set_light(Light::Off);
        self.mode = Mode::DeviceMenu {
            setting: Setting::Signing,
        };
        self.dirty = true;
    }

    fn restore(&mut self, host: &mut impl Host, job: RestoreJob, proof: Proof) {
        let lines = ["Rebuilding keys from", "the seed. About 3 minutes."];
        let result = {
            let Self { wallet, fb, .. } = self;
            wallet.restore(&job.seed, job.tree, job.chain_next_index, proof, |done| {
                show_progress(fb, &mut *host, "Restoring", &lines, done);
            })
        };
        match result {
            Ok(()) => {
                Self::respond(host, &Response::Ok);
                self.show_result(host, true, "Keys restored", "Restarting...".to_string());
                self.redraw(host);
                host.pause(RESTART_PAUSE_MS);
                host.restart();
            }
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                let detail = "The device's keys are unchanged.".to_string();
                self.show_result(host, false, "Not restored", detail);
            }
        }
    }

    fn prepare(&mut self, host: &mut impl Host) -> Result<(), WalletError> {
        let lines = [
            "Preparing 1,024 new keys.",
            "Signatures will move the",
            "account to them.",
        ];
        let Self { wallet, fb, .. } = self;
        wallet.prepare(|done| show_progress(fb, &mut *host, "Next key tree", &lines, done))?;
        Ok(())
    }

    /// Leaves a result, building the next tree first if it's time.
    fn leave_result(&mut self, host: &mut impl Host) {
        host.set_light(Light::Off);
        if self.wallet.should_prepare() == Ok(true) {
            let _ = self.prepare(host);
        }
        self.go_home(Tab::Home);
    }

    /// Takes the raw keys and touch, as often as possible.
    pub fn poll(&mut self, host: &mut impl Host, raw: KeyState, touch: Option<(i32, i32)>) {
        let now_ms = host.now_ms();
        let settled = self.debouncer.update(now_ms, raw);
        let previous = self.keys;
        self.keys = settled;
        if settled != previous {
            self.dirty = true;
        }
        let pressed = (0..4).find(|&key| settled[key] && !previous[key]);
        let touched = touch.filter(|_| !self.touching);
        self.touching = touch.is_some();
        if touch.is_none() && self.touched_slot.take().is_some() {
            self.dirty = true;
        }

        self.expire(host, now_ms);
        // A press on a signing screen belongs to the approval logic, and must not also act on
        // whatever screen that decision leads to.
        let deciding = matches!(
            self.mode,
            Mode::Review { .. }
                | Mode::SchemeConfirm { .. }
                | Mode::ShareConfirm { .. }
                | Mode::RestoreConfirm { .. }
        );
        self.decide(host, now_ms);
        if let Some(raw_touch) = touched {
            self.on_touch(host, raw_touch, now_ms);
        }
        if let (Some(key), false) = (pressed, deciding) {
            self.press(host, key, now_ms);
        }
        self.redraw(host);
    }

    fn press(&mut self, host: &mut impl Host, key: usize, now_ms: u64) {
        if matches!(
            self.mode,
            Mode::Result { .. } | Mode::Notice { .. } | Mode::Locked { .. }
        ) {
            self.dismiss(host);
        } else if let Some(soft) = keys_for(&self.mode)[key] {
            self.act(host, soft.action, now_ms);
        }
    }

    /// Runs the timers.
    fn expire(&mut self, host: &mut impl Host, now_ms: u64) {
        match &mut self.mode {
            Mode::Result { until_ms, .. } if now_ms >= *until_ms => self.leave_result(host),
            Mode::Passcode {
                touched_ms,
                purpose,
                count,
                ..
            } if !matches!(purpose, Purpose::Create | Purpose::CreateConfirm(_))
                && now_ms.saturating_sub(*touched_ms) >= PASSCODE_IDLE_MS =>
            {
                *count = 0;
                self.dirty = true;
                self.cancel_passcode(host);
            }
            Mode::SeedWords { touched_ms, .. } | Mode::BackupCheck { touched_ms, .. }
                if now_ms.saturating_sub(*touched_ms) >= BACKUP_IDLE_MS =>
            {
                self.mode = Mode::DeviceMenu {
                    setting: Setting::Backup,
                };
                self.dirty = true;
            }
            Mode::Locked { .. } if self.wallet.locked_for(now_ms) == 0 => {
                host.set_light(Light::Off);
                self.go_home(Tab::Home);
            }
            _ => {}
        }
    }

    fn cancel_passcode(&mut self, host: &mut impl Host) {
        let mode = core::mem::replace(&mut self.mode, Mode::Home(Tab::Home));
        if let Mode::Passcode { purpose, .. } = mode {
            self.abandon(host, purpose, false);
        }
    }

    /// Asks the approval logic whether the screen on show has been decided.
    fn decide(&mut self, host: &mut impl Host, now_ms: u64) {
        let keys = self.keys;
        let key_roles = roles(&keys_for(&self.mode));
        let (decision, timed_out) = match &mut self.mode {
            Mode::Review { approval, .. }
            | Mode::SchemeConfirm { approval, .. }
            | Mode::ShareConfirm { approval, .. }
            | Mode::RestoreConfirm { approval, .. } => (
                approval.update(now_ms, keys, key_roles),
                approval.remaining_ms(now_ms) == 0,
            ),
            _ => return,
        };
        if decision == Decision::Waiting {
            return;
        }
        let approved = decision == Decision::Approved;
        match core::mem::replace(&mut self.mode, Mode::Home(Tab::Home)) {
            Mode::Review { review, shown, .. } if approved => {
                self.ask_or_lock(host, Purpose::Sign(review, shown), now_ms);
            }
            Mode::Review { review, .. } => self.refuse(host, &review, timed_out),
            Mode::SchemeConfirm { target, .. } if approved => {
                self.ask_or_lock(host, Purpose::Scheme(target), now_ms);
            }
            Mode::SchemeConfirm { .. } => self.leave_switch(host),
            Mode::ShareConfirm { what, .. } if approved => self.hand_over(host, what, now_ms),
            Mode::ShareConfirm { .. } => {
                Self::respond(host, &Response::Err(WalletError::Rejected));
                host.set_light(Light::Off);
                self.go_home(Tab::Home);
            }
            Mode::RestoreConfirm { job, .. } if approved => {
                self.ask_or_lock(host, Purpose::Restore(job), now_ms);
            }
            Mode::RestoreConfirm { job, .. } => {
                self.abandon(host, Purpose::Restore(job), timed_out);
            }
            _ => {}
        }
    }

    fn on_touch(&mut self, host: &mut impl Host, raw: (i32, i32), now_ms: u64) {
        match &mut self.mode {
            Mode::Calibrate {
                step, raw: points, ..
            } => {
                points[*step] = raw;
                *step += 1;
                self.dirty = true;
                if *step == CROSSES.len() {
                    let points = *points;
                    self.finish_calibration(host, points);
                }
            }
            Mode::Passcode { digits, .. } => {
                let digits = *digits;
                let (x, y) = self.calibration.to_screen(raw.0, raw.1);
                if let Some(slot) = slot_at(&digits, x, y) {
                    self.touched_slot = Some(slot);
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
                retry: true,
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
            error,
            ..
        } = &mut self.mode
        else {
            return;
        };
        let Some(digit) = digit_in_slot(digits, slot) else {
            return;
        };
        entered[*count] = digit;
        *count += 1;
        *touched_ms = now_ms;
        *error = None;
        self.dirty = true;
        if *count < CODE_DIGITS {
            return;
        }
        let code = *entered;
        let mode = core::mem::replace(&mut self.mode, Mode::Home(Tab::Home));
        if let Mode::Passcode { purpose, .. } = mode {
            self.code_entered(host, purpose, code, now_ms);
        }
    }

    /// Does what the key table says a key does.
    fn act(&mut self, host: &mut impl Host, action: Action, now_ms: u64) {
        self.dirty = true;
        match action {
            Action::Goto(tab) => self.go_home(tab),
            Action::Open => self.open_tab(),
            Action::Back => self.go_back(),
            Action::ScrollUp | Action::ScrollDown => self.scroll(action == Action::ScrollDown),
            Action::PrevSetting | Action::NextSetting => {
                self.move_setting(action == Action::NextSetting)
            }
            Action::Change => self.change(host, now_ms),
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
            Action::Reject | Action::Approve => {}
        }
    }

    fn open_tab(&mut self) {
        match self.mode {
            Mode::Home(Tab::Activity) => self.mode = Mode::ActivityList { top: 0 },
            Mode::Home(Tab::Device) => {
                self.mode = Mode::DeviceMenu {
                    setting: Setting::Theme,
                }
            }
            _ => {}
        }
    }

    fn go_back(&mut self) {
        match &mut self.mode {
            Mode::ActivityList { .. } => self.go_home(Tab::Activity),
            Mode::DeviceMenu { .. } => self.go_home(Tab::Device),
            Mode::SeedWords { page: page @ 1, .. } => *page = 0,
            Mode::SeedWords { .. } => {
                self.mode = Mode::DeviceMenu {
                    setting: Setting::Backup,
                }
            }
            _ => {}
        }
    }

    fn scroll(&mut self, down: bool) {
        let len = self.view.log.len();
        if let Mode::ActivityList { top } = &mut self.mode {
            let scroll = Scroll {
                offset: *top,
                len,
                visible: ACTIVITY_ROWS,
            };
            *top = if down { scroll.down() } else { scroll.up() }.offset;
        }
    }

    fn move_setting(&mut self, forward: bool) {
        if let Mode::DeviceMenu { setting } = &mut self.mode {
            *setting = if forward {
                setting.next()
            } else {
                setting.prev()
            };
        }
    }

    fn change(&mut self, host: &mut impl Host, now_ms: u64) {
        let Mode::DeviceMenu { setting } = self.mode else {
            return;
        };
        match setting {
            Setting::Theme => {
                let next = theme::current().next();
                theme::set(next);
                let _ = self.settings.set_theme(next as u8);
            }
            Setting::Passcode => self.ask_or_lock(host, Purpose::ChangeOld, now_ms),
            Setting::Backup => self.ask_or_lock(host, Purpose::Backup, now_ms),
            Setting::Signing => {
                host.set_light(Light::Amber);
                self.mode = Mode::SchemeConfirm {
                    target: other_scheme(self.wallet.scheme()),
                    approval: Approval::new(now_ms, self.keys),
                };
            }
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
                self.go_home(Tab::Home);
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
            Some(Ok(Request::Scheme)) => {
                Self::respond(host, &Response::Scheme(self.wallet.scheme()));
            }
            Some(Ok(Request::Lockdown)) => {
                let response = Response::Lockdown {
                    fuses: host.fuses(),
                    sealed: self.wallet.is_sealed(),
                };
                Self::respond(host, &response);
            }
            Some(Ok(Request::MlPubKey)) => {
                let response = self
                    .wallet
                    .ml_public_key(MAIN_ACCOUNT)
                    .map_or_else(Response::Err, |key| Response::MlPubKey(key.to_vec()));
                Self::respond(host, &response);
            }
            Some(Ok(_)) if !self.idle() => {
                Self::respond(host, &Response::Err(WalletError::BadRequest));
            }
            Some(Ok(request)) => self.handle(host, request),
        }
        self.redraw(host);
    }

    fn report_keys(&mut self, host: &mut impl Host) {
        if let Err(error) = self.wallet.require_xmss() {
            Self::respond(host, &Response::Err(error));
            return;
        }
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
            Request::PubKey | Request::Scheme | Request::MlPubKey | Request::Lockdown => {}
            Request::StealthAddress => self.ask_to_share(host, Sharing::Address, now_ms),
            Request::ViewKey => self.ask_to_share(host, Sharing::ViewKey, now_ms),
            Request::SpendStealth(spend) => {
                if spend.to == [0; 20] || spend.pool == [0; 20] {
                    Self::respond(host, &Response::Err(WalletError::BadRequest));
                } else {
                    self.show_review(host, Pending::Stealth(spend), now_ms);
                }
            }
            Request::SignFrame(raw) => self.start_review(host, &raw, now_ms),
            Request::Prepare => {
                let response = self
                    .prepare(host)
                    .map_or_else(Response::Err, |()| Response::Ok);
                Self::respond(host, &response);
                self.go_home(Tab::Home);
            }
            Request::Activate(tree) => match self.wallet.activate(tree) {
                Ok(()) => {
                    Self::respond(host, &Response::Ok);
                    let title = format!("Now on tree {tree}");
                    let detail = "Restarting onto the new keys...".to_string();
                    self.show_result(host, true, &title, detail);
                    self.redraw(host);
                    host.pause(RESTART_PAUSE_MS);
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

    fn ask_to_share(&mut self, host: &mut impl Host, what: Sharing, now_ms: u64) {
        host.set_light(Light::Amber);
        self.mode = Mode::ShareConfirm {
            what,
            approval: Approval::new(now_ms, self.keys),
        };
        self.dirty = true;
    }

    fn show_review(&mut self, host: &mut impl Host, review: Pending, now_ms: u64) {
        let shown = match &review {
            Pending::Xmss(review) => Shown::from_tx(review.tx()),
            Pending::MlDsa(review) => Shown::from_ml_tx(review.tx()),
            Pending::Stealth(spend) => Shown::from_stealth(spend),
        };
        let Some(shown) = shown else {
            Self::respond(host, &Response::Err(WalletError::BadRequest));
            self.show_notice("Not shown", "That amount is too long to show whole.");
            return;
        };
        host.set_light(Light::Amber);
        let approval = Approval::new(now_ms, self.keys);
        self.mode = Mode::Review {
            review,
            shown,
            approval,
        };
        self.dirty = true;
    }

    fn start_review(&mut self, host: &mut impl Host, raw: &[u8], now_ms: u64) {
        match self.wallet.check_transaction(raw) {
            Ok(review) => self.show_review(host, review, now_ms),
            Err(error) => {
                Self::respond(host, &Response::Err(error));
                if error == WalletError::RotationNeeded {
                    self.show_notice(
                        "Rotation needed",
                        "Rotate to the next tree to keep signing.",
                    );
                }
            }
        }
    }

    /// Sets whether a laptop is connected.
    pub fn set_usb(&mut self, connected: bool) {
        if self.view.status.connected != connected {
            self.view.status.connected = connected;
            self.dirty = true;
        }
    }

    /// A number that changes when a countdown on screen needs redrawing.
    fn tick(&self, now_ms: u64) -> u64 {
        match &self.mode {
            Mode::Review { approval, .. } | Mode::RestoreConfirm { approval, .. } => {
                u64::from(seconds_left(approval, now_ms))
            }
            Mode::Locked { .. } => self.wallet.locked_for(now_ms).div_ceil(1000),
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
        let lock_ms = self.wallet.locked_for(now_ms);
        let flash = (0..4).find(|&key| self.keys[key]);
        let mut canvas = self.fb.canvas();
        draw(
            &mut canvas,
            &self.mode,
            &self.view,
            lock_ms,
            now_ms,
            flash,
            self.touched_slot,
        );
        host.present(&mut self.fb);
    }

    /// The key on the Wallet tab: the XMSS root, or in ML-DSA mode `keccak256(pk)`.
    #[must_use]
    pub fn shown_key(&self) -> &Hash {
        &self.view.root
    }

    /// The Activity tab's entries, newest first.
    #[must_use]
    pub fn activity(&self) -> Vec<Activity<'_>> {
        self.view.log.iter().map(LogEntry::item).collect()
    }

    /// A short name for the current screen.
    #[must_use]
    pub fn screen_name(&self) -> &'static str {
        match &self.mode {
            Mode::Calibrate { .. } => "calibrate",
            Mode::Home(Tab::Home) => "wallet",
            Mode::Home(Tab::Keys) => "keys",
            Mode::Home(Tab::Activity) => "activity",
            Mode::Home(Tab::Device) => "device",
            Mode::ActivityList { .. } => "activity-opened",
            Mode::DeviceMenu { .. } => "device-opened",
            Mode::Review { .. } => "review",
            Mode::SchemeConfirm { .. } => "scheme",
            Mode::ShareConfirm { what, .. } => match what {
                Sharing::Address => "address",
                Sharing::ViewKey => "viewkey",
            },
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

/// Draws a progress screen straight away, because tree building blocks the main loop.
fn show_progress(
    fb: &mut Framebuffer,
    host: &mut impl Host,
    title: &str,
    lines: &[&str],
    done: u32,
) {
    if done % PROGRESS_STEP == 0 {
        let mut canvas = fb.canvas();
        screens::progress(&mut canvas, title, lines, done, LEAVES);
        host.present(fb);
    }
}
