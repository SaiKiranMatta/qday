//! Every screen of the device, as a pure function of what it shows. Each one
//! is drawn by the active theme's style family (`crate::ui`); the content and
//! the soft keys are the same in every theme. The firmware calls these into
//! its framebuffer; `examples/preview.rs` renders them to PNG on the laptop.

use crate::canvas::Canvas;
use crate::nav::Setting;
use crate::text::Text;
use crate::theme::{self, Family};
use crate::ui;
pub use crate::widgets::Status;

pub const LEAVES: u32 = 1024;
pub const ROTATE_AT: u32 = 900;
pub const RESERVED_FROM: u32 = 1008;

/// Rows of the activity list that fit on screen.
pub const ACTIVITY_ROWS: usize = 4;

/// `0x9e83...019c`: the short form of a 32-byte value.
pub fn short_hex(bytes: &[u8; 32]) -> Text<20> {
    let (head, tail) = hex_parts(bytes);
    crate::text!(20, "{}...{}", head.as_str(), tail.as_str())
}

/// The parts of `short_hex` people compare (`0x9e83`, `019c`); screens grey
/// out the `...` between them.
pub fn hex_parts(bytes: &[u8; 32]) -> (Text<8>, Text<8>) {
    (
        crate::text!(8, "0x{:02x}{:02x}", bytes[0], bytes[1]),
        crate::text!(8, "{:02x}{:02x}", bytes[30], bytes[31]),
    )
}

/// `#0067` split into the greyed leading zeros (`#00`) and the number (`67`).
pub fn key_parts(index: u32) -> (Text<8>, Text<8>) {
    let digits = crate::text!(8, "{index:04}");
    let s = digits.as_str();
    let zeros = s.len() - s.trim_start_matches('0').len();
    let zeros = zeros.min(s.len() - 1); // always show one digit
    (
        crate::text!(8, "#{}", &s[..zeros]),
        crate::text!(8, "{}", &s[zeros..]),
    )
}

/// A 42-character address as `0x` plus ten groups of four, or `None` for
/// anything else.
pub fn address_groups(addr: &str) -> Option<[&str; 10]> {
    if addr.len() != 42 || !addr.is_ascii() {
        return None;
    }
    let mut g = [""; 10];
    for (n, slot) in g.iter_mut().enumerate() {
        *slot = &addr[2 + 4 * n..6 + 4 * n];
    }
    Some(g)
}

/// One line in the activity list.
pub struct Activity<'a> {
    pub approved: bool,
    pub title: &'a str,
    pub detail: &'a str,
    pub index: Option<u32>,
}

pub struct DeviceInfo<'a> {
    pub firmware: &'a str,
    pub root: &'a [u8; 32],
    pub counter: u32,
}

/// What the review screen shows, all decoded on the device.
pub enum Review<'a> {
    Send {
        amount: &'a str,
        unit: &'a str,
        to: &'a str,
    },
    Token {
        amount: &'a str,
        to: &'a str,
        token: &'a str,
    },
    Call {
        contract: &'a str,
        value: &'a str,
        function: &'a str,
    },
    Batch {
        calls: usize,
    },
    Unknown {
        selector: &'a str,
    },
    /// A call the device doesn't recognise: the Call layout, with a warning.
    UnknownCall {
        contract: &'a str,
        value: &'a str,
        function: &'a str,
    },
    /// A bare hash: the device can't check what it is.
    Blind {
        description: &'a str,
        hash: &'a str,
    },
}

impl Review<'_> {
    /// The device can't check this: warn in the heading.
    pub fn is_risky(&self) -> bool {
        matches!(
            self,
            Review::Blind { .. }
                | Review::Unknown { .. }
                | Review::UnknownCall { .. }
                | Review::Batch { .. }
        )
    }
}

pub struct ReviewInfo<'a> {
    pub review: Review<'a>,
    pub fee: &'a str,
    pub chain_id: u64,
    pub index: u32,
    pub rotation: bool,
    /// Seconds left to decide, and the full timeout.
    pub seconds_left: u32,
    pub timeout: u32,
}

pub struct Passcode<'a> {
    pub title: &'a str,
    pub hint: &'a str,
    pub entered: usize,
    pub error: Option<&'a str>,
    /// `digits[i]` is the digit in the i-th digit slot (shuffled each time).
    pub digits: &'a [u8; 10],
    /// The keypad slot being touched.
    pub pressed: Option<usize>,
}

/// Calls the active family's version of a screen.
macro_rules! dispatch {
    ($screen:ident($($arg:expr),*)) => {
        match theme::current().family() {
            Family::Standard => ui::standard::$screen($($arg),*),
            Family::Brutal => ui::brutal::$screen($($arg),*),
            Family::Terminal => ui::terminal::$screen($($arg),*),
        }
    };
}

// --- tabs ------------------------------------------------------------------

/// Wallet: the account key, whether it can sign, quiet key usage, the latest
/// activity. `pressed` is the soft key being held (drawn pressed).
pub fn home(
    c: &mut Canvas,
    s: &Status,
    root: &[u8; 32],
    pending: bool,
    latest: Option<&Activity>,
    pressed: Option<usize>,
) {
    dispatch!(home(c, s, root, pending, latest, pressed))
}

/// The key tree: next key, usage and the map of all 1,024 keys.
pub fn keys(c: &mut Canvas, s: &Status, pending: bool, pressed: Option<usize>) {
    dispatch!(keys(c, s, pending, pressed))
}

/// Latest signatures; `opened` when the soft keys scroll it.
pub fn activity(
    c: &mut Canvas,
    s: &Status,
    items: &[Activity],
    offset: usize,
    opened: bool,
    pressed: Option<usize>,
) {
    dispatch!(activity(c, s, items, offset, opened, pressed))
}

/// Device facts and settings; `opened` when the soft keys pick a setting.
pub fn device(
    c: &mut Canvas,
    s: &Status,
    info: &DeviceInfo,
    setting: Setting,
    opened: bool,
    pressed: Option<usize>,
) {
    dispatch!(device(c, s, info, setting, opened, pressed))
}

// --- signing ---------------------------------------------------------------

pub fn review(c: &mut Canvas, info: &ReviewInfo, pressed: Option<usize>) {
    dispatch!(review(c, info, pressed))
}

pub fn passcode(c: &mut Canvas, pc: &Passcode, pressed: Option<usize>) {
    dispatch!(passcode(c, pc, pressed))
}

/// Progress for long jobs (building a key tree).
pub fn progress(c: &mut Canvas, title: &str, lines: &[&str], done: u32, total: u32) {
    dispatch!(progress(c, title, lines, done, total))
}

/// Signed / rejected / failed, with one line of detail.
pub fn result(c: &mut Canvas, ok: bool, title: &str, detail: &str) {
    dispatch!(result(c, ok, title, detail))
}

/// 12 of the 24 backup words (`page` 0 or 1).
pub fn seed_words(c: &mut Canvas, words: &[&str; 24], page: usize, pressed: Option<usize>) {
    dispatch!(seed_words(c, words, page, pressed))
}

/// The backup check, question `step` (0..3): which word is number
/// `position + 1`? The four `options` sit on K1-K4. `wrong` after a miss.
pub fn seed_check(
    c: &mut Canvas,
    step: usize,
    position: usize,
    options: &[&'static str; 4],
    wrong: bool,
    pressed: Option<usize>,
) {
    dispatch!(seed_check(c, step, position, options, wrong, pressed))
}

/// Too many wrong passcodes.
pub fn locked(c: &mut Canvas, wait: &str, attempts_left: u32) {
    dispatch!(locked(c, wait, attempts_left))
}

/// Asks before replacing the keys with ones restored from a seed.
pub fn restore_confirm(
    c: &mut Canvas,
    tree: u32,
    first_index: u32,
    seconds_left: u32,
    timeout: u32,
    pressed: Option<usize>,
) {
    dispatch!(restore_confirm(
        c,
        tree,
        first_index,
        seconds_left,
        timeout,
        pressed
    ))
}

/// A short message with no choices, e.g. "Tree switched".
pub fn notice(c: &mut Canvas, title: &str, detail: &str) {
    dispatch!(notice(c, title, detail))
}

/// Touch calibration, step `step` (0..3): a cross at `settings::CROSSES[step]`.
/// One plain look in every theme; it's a one-time setup step.
pub fn calibrate(c: &mut Canvas, step: usize, retry: bool, status: &str) {
    use crate::canvas::{Color, TextStyle};
    use crate::fonts::{BODY, TITLE};
    let (bg, fg, muted) = (
        Color::hex(0x212121),
        Color::hex(0xF7F7F7),
        Color::hex(0x8C8C8C),
    );
    c.clear(bg);
    let (x, y) = qday_wallet::settings::CROSSES[step.min(2)];
    let (x, y) = (x as f32, y as f32);
    c.line(x - 12.0, y, x + 12.0, y, 1.5, fg, 255);
    c.line(x, y - 12.0, x, y + 12.0, 1.5, fg, 255);
    c.ring(x, y, 5.0, 1.5, fg, 255);
    c.text_centered(160, 112, "Touch the cross", &TextStyle::new(&TITLE, fg));
    let line = if retry {
        "Those taps were too close. Again, firmly."
    } else {
        "Press firmly, then let go."
    };
    c.text_centered(160, 134, line, &TextStyle::new(&BODY, muted));
    c.text_centered(
        160,
        154,
        crate::text!(16, "{} of 3", step + 1).as_str(),
        &TextStyle::new(&BODY, muted),
    );
    // What the touch chip reports right now, for checking the wiring.
    c.text_centered(
        160,
        190,
        status,
        &TextStyle::new(&crate::fonts::CAPTION, muted),
    );
}
