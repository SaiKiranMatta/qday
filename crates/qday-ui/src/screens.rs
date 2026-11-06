//! Every screen, and how it draws.

use alloc::format;
use alloc::string::{String, ToString};

use qday_wallet::activity::{Activity, Entry, Summary};
use qday_wallet::frame::{Action as TxAction, Address, CHAIN_ID, FrameTx};
use qday_wallet::hex;
use qday_wallet::settings::{CROSSES, ThemeChoice};
use qday_wallet::tokens;
use qday_wallet::wallet::{PREPARE_FROM, RESERVED_FROM};
use qday_xmss::{Hash, LEAVES};

use crate::fb::{Framebuffer, WIDTH};
use crate::fonts::{BODY, Font, HERO, LABEL, NUMBER, SMALL, TITLE};
use crate::format::{
    address_lines, base_units, duration, eth, grouped, key_number, key_parts, token,
};
use crate::keys::{self, Action, KeyBar, Label, SoftKey, Style, Tab};
use crate::theme::{Color, Theme};

/// Pixels between the screen edge and content.
pub const MARGIN: i32 = 12;
/// The last row content may use on a tab.
pub const TAB_CONTENT_END: i32 = 207;
/// The last row content may use above pills.
pub const PILL_CONTENT_END: i32 = 182;
/// The first row of pills.
pub const PILLS_TOP: i32 = 194;
/// Passcode keypad position and sizes.
pub const PAD_LEFT: i32 = 146;
/// The keypad's top row.
pub const PAD_TOP: i32 = 8;
/// A keypad key's width.
pub const PAD_KEY_WIDTH: i32 = 50;
/// A keypad key's height.
pub const PAD_KEY_HEIGHT: i32 = 40;
/// The space between keypad columns.
pub const PAD_GAP_X: i32 = 6;
/// The space between keypad rows.
pub const PAD_GAP_Y: i32 = 4;

/// What the home tabs show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Home {
    /// The account's root.
    pub key: Hash,
    /// Whether a laptop is connected.
    pub usb: bool,
    /// The next one-time key.
    pub next_index: u32,
    /// The active tree.
    pub tree: u32,
    /// The counter's value.
    pub counter: u32,
    /// Firmware version.
    pub version: &'static str,
    /// The newest activity.
    pub latest: Option<Entry>,
    /// The saved theme.
    pub theme: ThemeChoice,
    /// Whether the next tree is ready.
    pub next_ready: bool,
}

/// Rows on the opened Device tab.
pub const DEVICE_ROWS: usize = 3;
/// Activity rows on screen.
pub const ACTIVITY_ROWS: usize = 4;

/// One screen.
#[derive(Clone, Copy)]
pub enum Screen<'a> {
    /// A home tab.
    Tab(Tab, &'a Home, &'a Activity),
    /// The opened Activity tab, from entry `top`.
    ActivityList(&'a Activity, usize),
    /// The opened Device tab.
    DeviceMenu(&'a Home, usize),
    /// A transaction to approve.
    Review(&'a FrameTx, u32),
    /// A restore to approve.
    RestoreConfirm {
        /// The tree being restored.
        tree: u32,
        /// The first key it will use.
        first_index: u32,
        /// Seconds left.
        seconds: u32,
    },
    /// Passcode entry.
    Passcode {
        /// The heading.
        title: &'a str,
        /// The line under it.
        hint: &'a str,
        /// Digits entered so far.
        entered: usize,
        /// The shuffled keypad.
        digits: &'a [u8; 10],
    },
    /// Work in progress.
    Progress {
        /// What's happening.
        title: &'a str,
        /// How far it has got.
        done: u32,
        /// How far it has to go.
        total: u32,
    },
    /// How something ended.
    Result {
        /// Whether it worked.
        success: bool,
        /// The heading.
        title: &'a str,
        /// The line under it.
        detail: &'a str,
    },
    /// Twelve of the 24 words.
    SeedWords {
        /// All 24 words.
        words: &'a [&'static str; 24],
        /// 0 or 1.
        page: usize,
    },
    /// A question about the backup.
    BackupCheck {
        /// Which question, from 0.
        number: usize,
        /// Which word is being asked for, from 0.
        position: usize,
        /// The four words offered.
        choices: [&'static str; 4],
    },
    /// Passcode entry is locked.
    Locked {
        /// How long is left.
        wait_ms: u64,
        /// Tries before the keys are erased.
        tries_left: u32,
    },
    /// Something the user needs to read.
    Notice {
        /// The heading.
        title: &'a str,
        /// The first line.
        first: &'a str,
        /// The second line.
        second: &'a str,
    },
    /// One touch calibration cross.
    Calibrate {
        /// Which cross, from 0.
        step: usize,
    },
}

/// The right edge of content.
const RIGHT: i32 = WIDTH - MARGIN;
/// The width of content.
const CONTENT: i32 = RIGHT - MARGIN;
const GAP: i32 = 8;
const KEY_WIDTH: i32 = 68;
const PILL_HEIGHT: i32 = 34;
const CARD_RADIUS: i32 = 10;
const TAB_BASELINE: i32 = 227;
/// Where values start in a labelled row.
const VALUE_X: i32 = 72;
const ROW_STEP: i32 = 17;

fn key_left(key: usize) -> i32 {
    MARGIN + i32::try_from(key).unwrap_or(0) * (KEY_WIDTH + GAP)
}

fn centered(fb: &mut Framebuffer, font: &Font, center: i32, y: i32, text: &str, color: Color) {
    fb.text(font, center - font.width(text) / 2, y, text, color);
}

fn right_aligned(fb: &mut Framebuffer, font: &Font, right: i32, y: i32, text: &str, color: Color) {
    fb.text(font, right - font.width(text), y, text, color);
}

fn card(fb: &mut Framebuffer, theme: &Theme, x: i32, y: i32, width: i32, height: i32) {
    fb.rounded(x, y, width, height, CARD_RADIUS, theme.card);
}

fn draw_tabs(fb: &mut Framebuffer, theme: &Theme, bar: &KeyBar, current: Tab) {
    for (slot, key) in bar.keys.iter().enumerate() {
        let Label::Text(text) = key.label else {
            continue;
        };
        let color = if key.action == Action::Tab(current) {
            theme.text
        } else {
            theme.muted
        };
        let center = key_left(slot) + KEY_WIDTH / 2;
        centered(fb, &SMALL, center, TAB_BASELINE, text, color);
    }
}

fn pill_text(key: &SoftKey) -> Option<String> {
    match key.label {
        Label::Blank => None,
        Label::Text(text) => Some(text.to_string()),
        Label::Seconds(seconds) => Some(format!("{seconds} s")),
    }
}

fn draw_pills(fb: &mut Framebuffer, theme: &Theme, bar: &KeyBar) {
    let (controls, count) = bar.controls();
    for control in controls.iter().take(count).flatten() {
        let Some(text) = pill_text(&control.key) else {
            continue;
        };
        let x = key_left(control.first);
        let span = i32::try_from(control.span).unwrap_or(1);
        let width = KEY_WIDTH * span + GAP * (span - 1);
        let countdown = matches!(control.key.label, Label::Seconds(_));
        let (fill, ink) = match control.key.action {
            Action::Approve => (Some(theme.primary), theme.on_primary),
            _ if countdown => (None, theme.muted),
            _ => (Some(theme.fill), theme.text),
        };
        if let Some(fill) = fill {
            fb.rounded(x, PILLS_TOP, width, PILL_HEIGHT, PILL_HEIGHT / 2, fill);
        }
        let font: &Font = if LABEL.width(&text) <= width - 8 {
            &LABEL
        } else {
            &SMALL
        };
        centered(fb, font, x + width / 2, PILLS_TOP + 22, &text, ink);
    }
}

fn draw_key_number(fb: &mut Framebuffer, theme: &Theme, font: &Font, x: i32, y: i32, index: u32) {
    let (grey, plain) = key_number(index);
    let pen = fb.text(font, x, y, &grey, theme.muted);
    fb.text(font, pen, y, &plain, theme.text);
}

fn draw_account_key(fb: &mut Framebuffer, theme: &Theme, x: i32, y: i32, key: &Hash) {
    let (start, middle, end) = key_parts(key);
    let pen = fb.text(&LABEL, x, y, &start, theme.text);
    let pen = fb.text(&LABEL, pen, y, &middle, theme.muted);
    let pen = fb.text(&LABEL, pen, y, "...", theme.muted);
    fb.text(&LABEL, pen, y, &end, theme.text);
}

fn summary_text(summary: &Summary) -> String {
    match summary {
        Summary::Transfer { value, .. } => eth(*value),
        Summary::TokenTransfer {
            token: contract,
            amount,
            ..
        } => tokens::known(contract).map_or_else(
            || "Token transfer".to_string(),
            |known| token(*amount, known.decimals, known.symbol),
        ),
        Summary::Rotation => "Rotation".to_string(),
        Summary::Creation => "Account created".to_string(),
        Summary::Other => "Unknown call".to_string(),
    }
}

fn entry_row(fb: &mut Framebuffer, theme: &Theme, y: i32, entry: &Entry) {
    card(fb, theme, MARGIN, y, CONTENT, 36);
    let (word, color) = if entry.signed {
        ("Signed", theme.text)
    } else {
        ("Rejected", theme.muted)
    };
    let pen = fb.text(&LABEL, MARGIN + 12, y + 23, word, color);
    let (grey, plain) = key_number(entry.index);
    let pen = fb.text(&SMALL, pen + 8, y + 23, &grey, theme.muted);
    fb.text(&SMALL, pen, y + 23, &plain, theme.muted);
    let text = summary_text(&entry.summary);
    right_aligned(fb, &BODY, RIGHT - 12, y + 23, &text, theme.text);
}

fn draw_activity(fb: &mut Framebuffer, theme: &Theme, activity: &Activity, top: usize) {
    if activity.is_empty() {
        centered(fb, &BODY, WIDTH / 2, 100, "Nothing signed yet", theme.muted);
        return;
    }
    for row in 0..ACTIVITY_ROWS {
        if let Some(entry) = activity.get(top + row) {
            let offset = i32::try_from(row).unwrap_or(0) * (36 + GAP);
            entry_row(fb, theme, MARGIN + offset, &entry);
        }
    }
}

fn draw_wallet(fb: &mut Framebuffer, theme: &Theme, home: &Home) {
    card(fb, theme, MARGIN, 12, CONTENT, 80);
    fb.text(&SMALL, MARGIN + 12, 32, "Account key", theme.muted);
    draw_account_key(fb, theme, MARGIN + 12, 53, &home.key);
    let (status, dot) = if home.usb {
        ("Ready to sign", theme.success)
    } else {
        ("Plug in USB", theme.muted)
    };
    fb.circle(MARGIN + 16, 73, 4, dot);
    fb.text(&BODY, MARGIN + 26, 78, status, theme.text);

    let half = (CONTENT - GAP) / 2;
    card(fb, theme, MARGIN, 100, half, 56);
    fb.text(&SMALL, MARGIN + 12, 118, "Signatures used", theme.muted);
    let used = format!("{} / {}", grouped(home.next_index), grouped(LEAVES));
    fb.text(&NUMBER, MARGIN + 12, 144, &used, theme.text);
    let second = MARGIN + half + GAP;
    card(fb, theme, second, 100, half, 56);
    fb.text(&SMALL, second + 12, 118, "Tree", theme.muted);
    fb.text(
        &NUMBER,
        second + 12,
        144,
        &home.tree.to_string(),
        theme.text,
    );

    card(fb, theme, MARGIN, 164, CONTENT, 40);
    let latest = home.latest.map_or_else(
        || "No activity yet".to_string(),
        |entry| {
            let word = if entry.signed { "Signed" } else { "Rejected" };
            format!("{word} {}", summary_text(&entry.summary))
        },
    );
    fb.text(&SMALL, MARGIN + 12, 188, "Latest", theme.muted);
    right_aligned(fb, &BODY, RIGHT - 12, 188, &latest, theme.text);
}

/// The left edge and top of the key map.
const MAP_LEFT: i32 = 148;
const MAP_TOP: i32 = 24;
const MAP_PITCH: i32 = 5;
const MAP_COLUMNS: u32 = 32;

fn legend(fb: &mut Framebuffer, theme: &Theme, y: i32, color: Color, text: &str) {
    fb.rect(MARGIN, y - 7, 5, 5, color);
    fb.text(&SMALL, MARGIN + 11, y, text, theme.muted);
}

fn draw_keys(fb: &mut Framebuffer, theme: &Theme, home: &Home) {
    fb.text(&SMALL, MARGIN, 28, "Next key", theme.muted);
    draw_key_number(fb, theme, &NUMBER, MARGIN, 52, home.next_index);
    fb.text(&SMALL, MARGIN, 76, "Tree", theme.muted);
    fb.text(&LABEL, MARGIN + 34, 76, &home.tree.to_string(), theme.text);

    legend(fb, theme, 112, theme.muted, "Used");
    legend(fb, theme, 128, theme.primary, "Next");
    legend(fb, theme, 144, theme.dot, "Unused");
    legend(fb, theme, 160, theme.warning, "Key 900");
    legend(fb, theme, 176, theme.danger, "Reserved");

    for index in 0..LEAVES {
        let x = MAP_LEFT + i32::try_from(index % MAP_COLUMNS).unwrap_or(0) * MAP_PITCH;
        let y = MAP_TOP + i32::try_from(index / MAP_COLUMNS).unwrap_or(0) * MAP_PITCH;
        if index == home.next_index {
            fb.rect(x - 1, y - 1, 5, 5, theme.primary);
            continue;
        }
        let color = if index < home.next_index {
            theme.muted
        } else if index >= RESERVED_FROM {
            theme.danger
        } else if index == PREPARE_FROM {
            theme.warning
        } else {
            theme.dot
        };
        fb.rect(x, y, 3, 3, color);
    }
}

fn labelled(fb: &mut Framebuffer, theme: &Theme, y: i32, label: &str, value: &str) {
    fb.text(&SMALL, MARGIN, y, label, theme.muted);
    fb.text(&SMALL, VALUE_X, y, value, theme.text);
}

fn draw_device(fb: &mut Framebuffer, theme: &Theme, home: &Home) {
    card(fb, theme, MARGIN, 12, CONTENT, 192);
    fb.text(&SMALL, MARGIN + 12, 34, "Account key", theme.muted);
    draw_account_key(fb, theme, MARGIN + 12, 54, &home.key);
    let rows = [
        ("Counter", grouped(home.counter)),
        ("Firmware", home.version.to_string()),
        (
            "Next tree",
            (if home.next_ready {
                "Ready"
            } else {
                "Not built"
            })
            .to_string(),
        ),
    ];
    for ((label, value), y) in rows.iter().zip([92, 118, 144]) {
        fb.text(&BODY, MARGIN + 12, y, label, theme.muted);
        right_aligned(fb, &BODY, RIGHT - 12, y, value, theme.text);
    }
    fb.text(
        &SMALL,
        MARGIN + 12,
        188,
        "Press Device again for settings",
        theme.muted,
    );
}

fn draw_device_menu(fb: &mut Framebuffer, theme: &Theme, home: &Home, selected: usize) {
    let theme_name = match home.theme {
        ThemeChoice::Light => "Standard Light",
        ThemeChoice::Dark => "Standard Dark",
    };
    let rows = [
        ("Theme", theme_name),
        ("Passcode", "6 digits"),
        ("Back up seed", "24 words"),
    ];
    for (row, ((label, value), y)) in rows.iter().zip([12, 70, 128]).enumerate() {
        let fill = if row == selected {
            theme.fill
        } else {
            theme.card
        };
        fb.rounded(MARGIN, y, CONTENT, 50, CARD_RADIUS, fill);
        fb.text(&LABEL, MARGIN + 12, y + 30, label, theme.text);
        right_aligned(fb, &BODY, RIGHT - 12, y + 30, value, theme.muted);
    }
}

/// Draws an amount without ever clipping it: smaller fonts first, then two lines.
fn draw_amount(fb: &mut Framebuffer, theme: &Theme, text: &str) {
    for font in [&HERO, &TITLE, &LABEL, &SMALL] {
        if font.width(text) <= CONTENT {
            fb.text(font, MARGIN, 58, text, theme.text);
            return;
        }
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    fb.text(&SMALL, MARGIN, 45, whole, theme.text);
    fb.text(&SMALL, MARGIN, 60, &format!(".{fraction}"), theme.text);
}

fn draw_address(fb: &mut Framebuffer, theme: &Theme, y: i32, label: &str, address: &Address) {
    let (first, second) = address_lines(address);
    labelled(fb, theme, y, label, &first);
    fb.text(&SMALL, VALUE_X, y + 15, &second, theme.text);
}

/// The top of a token transfer's review. Returns where the next row goes.
fn draw_token_transfer(
    fb: &mut Framebuffer,
    theme: &Theme,
    mut y: i32,
    contract: &Address,
    to: &Address,
    amount: u128,
) -> i32 {
    if let Some(known) = tokens::known(contract) {
        fb.text(&SMALL, MARGIN, 24, "Send", theme.muted);
        draw_amount(fb, theme, &token(amount, known.decimals, known.symbol));
    } else {
        fb.text(
            &LABEL,
            MARGIN,
            24,
            "Unknown token: check carefully",
            theme.warning,
        );
        draw_amount(fb, theme, &base_units(amount));
        draw_address(fb, theme, y, "Token", contract);
        y += 15 + ROW_STEP;
    }
    draw_address(fb, theme, y, "To", to);
    y + 15 + ROW_STEP
}

fn draw_review(fb: &mut Framebuffer, theme: &Theme, tx: &FrameTx) {
    let mut y = 82;
    match &tx.action {
        TxAction::Transfer { to, value } => {
            fb.text(&SMALL, MARGIN, 24, "Send", theme.muted);
            draw_amount(fb, theme, &eth(*value));
            draw_address(fb, theme, y, "To", to);
            y += 15 + ROW_STEP;
        }
        TxAction::TokenTransfer { token, to, amount } => {
            y = draw_token_transfer(fb, theme, y, token, to, *amount);
        }
        TxAction::Rotate { next_root, .. } => {
            fb.text(&SMALL, MARGIN, 24, "Rotate to a new tree", theme.muted);
            draw_amount(fb, theme, "New key");
            let (start, middle, end) = key_parts(next_root);
            labelled(
                fb,
                theme,
                y,
                "Next key",
                &format!("{start}{middle}...{end}"),
            );
            y += ROW_STEP;
        }
        TxAction::Create { factory, .. } => {
            fb.text(&SMALL, MARGIN, 24, "Set up this wallet", theme.muted);
            draw_amount(fb, theme, "Create account");
            draw_address(fb, theme, y, "Account", &tx.sender);
            y += 15 + ROW_STEP;
            draw_address(fb, theme, y, "Factory", factory);
            y += 15 + ROW_STEP;
        }
        TxAction::Other {
            target,
            value,
            data,
            ..
        } => {
            fb.text(
                &LABEL,
                MARGIN,
                24,
                "Unknown call: check carefully",
                theme.warning,
            );
            draw_amount(fb, theme, &eth(*value));
            if let Some(target) = target {
                draw_address(fb, theme, y, "Target", target);
                y += 15 + ROW_STEP;
            } else {
                labelled(fb, theme, y, "Target", "This account");
                y += ROW_STEP;
            }
            let selector = data
                .get(..4)
                .map_or_else(|| "None".to_string(), hex::encode);
            labelled(fb, theme, y, "Selector", &selector);
            y += ROW_STEP;
        }
    }
    labelled(
        fb,
        theme,
        y,
        "Fee",
        &format!("Up to {}", eth(tx.max_fee_wei())),
    );
    y += ROW_STEP;
    // Warning screens need the room, so they leave out the network.
    let warns = match &tx.action {
        TxAction::Other { .. } => true,
        TxAction::TokenTransfer { token, .. } => tokens::known(token).is_none(),
        _ => false,
    };
    if !warns {
        labelled(
            fb,
            theme,
            y,
            "Network",
            &format!("QDay devnet, chain {CHAIN_ID}"),
        );
        y += ROW_STEP;
    }
    fb.text(&SMALL, MARGIN, y, "Key", theme.muted);
    draw_key_number(fb, theme, &SMALL, VALUE_X, y, tx.index);
    fb.text(
        &SMALL,
        MARGIN,
        176,
        "Approve only if this matches",
        theme.muted,
    );
}

fn draw_restore(fb: &mut Framebuffer, theme: &Theme, tree: u32, first_index: u32) {
    fb.text(&SMALL, MARGIN, 24, "Restore", theme.muted);
    fb.text(&TITLE, MARGIN, 52, "Replace the keys?", theme.text);
    labelled(fb, theme, 82, "Tree", &tree.to_string());
    fb.text(&SMALL, MARGIN, 99, "First key", theme.muted);
    draw_key_number(fb, theme, &SMALL, VALUE_X, 99, first_index);
    fb.text(
        &SMALL,
        MARGIN,
        130,
        "The keys on this device will be erased.",
        theme.muted,
    );
    fb.text(
        &SMALL,
        MARGIN,
        176,
        "Approve only if you asked for this",
        theme.muted,
    );
}

/// The keypad slots that hold digits, row by row. The bottom row has one key, in the middle.
const PAD_SLOTS: [Option<usize>; 12] = [
    Some(0),
    Some(1),
    Some(2),
    Some(3),
    Some(4),
    Some(5),
    Some(6),
    Some(7),
    Some(8),
    None,
    Some(9),
    None,
];
const PAD_COLUMNS: i32 = 3;

fn draw_passcode(
    fb: &mut Framebuffer,
    theme: &Theme,
    title: &str,
    hint: &str,
    entered: usize,
    digits: &[u8; 10],
) {
    fb.text(&LABEL, MARGIN, 34, title, theme.text);
    fb.text(&SMALL, MARGIN, 54, hint, theme.muted);
    for dot in 0_usize..6 {
        let color = if dot < entered {
            theme.primary
        } else {
            theme.dot
        };
        fb.circle(
            MARGIN + 7 + i32::try_from(dot).unwrap_or(0) * 20,
            104,
            6,
            color,
        );
    }
    for (slot, digit) in (0_i32..).zip(PAD_SLOTS) {
        let Some(digit) = digit else {
            continue;
        };
        let x = PAD_LEFT + (slot % PAD_COLUMNS) * (PAD_KEY_WIDTH + PAD_GAP_X);
        let y = PAD_TOP + (slot / PAD_COLUMNS) * (PAD_KEY_HEIGHT + PAD_GAP_Y);
        fb.rounded(x, y, PAD_KEY_WIDTH, PAD_KEY_HEIGHT, CARD_RADIUS, theme.fill);
        let text = digits[digit].to_string();
        centered(
            fb,
            &NUMBER,
            x + PAD_KEY_WIDTH / 2,
            y + 27,
            &text,
            theme.text,
        );
    }
}

fn draw_progress(fb: &mut Framebuffer, theme: &Theme, title: &str, done: u32, total: u32) {
    let center = WIDTH / 2;
    centered(fb, &TITLE, center, 88, title, theme.text);
    let track = CONTENT - 40;
    let left = MARGIN + 20;
    fb.rounded(left, 104, track, 8, 4, theme.fill);
    let total_or_one = i64::from(total.max(1));
    let filled = i64::from(track) * i64::from(done.min(total)) / total_or_one;
    let filled = i32::try_from(filled).unwrap_or(track);
    if filled >= 8 {
        fb.rounded(left, 104, filled, 8, 4, theme.primary);
    }
    let percent = i64::from(done.min(total)) * 100 / total_or_one;
    centered(fb, &BODY, center, 136, &format!("{percent}%"), theme.muted);
    centered(fb, &SMALL, center, 160, "Keep the power on", theme.muted);
}

/// Draws a thick line as a row of soft dots.
fn stroke(fb: &mut Framebuffer, from: (i32, i32), to: (i32, i32), color: Color) {
    const STEPS: i32 = 16;
    for step in 0..=STEPS {
        let x = from.0 + (to.0 - from.0) * step / STEPS;
        let y = from.1 + (to.1 - from.1) * step / STEPS;
        fb.circle(x, y, 2, color);
    }
}

fn draw_result(fb: &mut Framebuffer, theme: &Theme, success: bool, title: &str, detail: &str) {
    let center = WIDTH / 2;
    let color = if success { theme.success } else { theme.danger };
    fb.circle(center, 66, 28, color);
    if success {
        stroke(fb, (center - 12, 67), (center - 4, 76), theme.bg);
        stroke(fb, (center - 4, 76), (center + 13, 57), theme.bg);
    } else {
        stroke(fb, (center - 10, 56), (center + 10, 76), theme.bg);
        stroke(fb, (center + 10, 56), (center - 10, 76), theme.bg);
    }
    centered(fb, &TITLE, center, 126, title, theme.text);
    centered(fb, &BODY, center, 150, detail, theme.muted);
}

fn draw_seed_words(fb: &mut Framebuffer, theme: &Theme, words: &[&str; 24], page: usize) {
    let first = page * 12;
    let heading = format!("Words {} to {} of 24", first + 1, first + 12);
    fb.text(&SMALL, MARGIN, 24, &heading, theme.muted);
    right_aligned(fb, &SMALL, RIGHT, 24, "Write these down", theme.muted);
    for (slot, word) in (0_i32..).zip(&words[first..first + 12]) {
        let x = MARGIN + (slot / 6) * (CONTENT / 2 + 4);
        let y = 52 + (slot % 6) * 23;
        let number = first + usize::try_from(slot).unwrap_or(0) + 1;
        right_aligned(fb, &SMALL, x + 18, y, &number.to_string(), theme.muted);
        fb.text(&LABEL, x + 26, y, word, theme.text);
    }
}

fn draw_backup_check(fb: &mut Framebuffer, theme: &Theme, number: usize, position: usize) {
    let center = WIDTH / 2;
    let heading = format!("Question {} of 3", number + 1);
    centered(fb, &SMALL, center, 60, &heading, theme.muted);
    centered(
        fb,
        &TITLE,
        center,
        100,
        &format!("Which is word {}?", position + 1),
        theme.text,
    );
    centered(
        fb,
        &SMALL,
        center,
        130,
        "Press the key under it",
        theme.muted,
    );
}

fn draw_locked(fb: &mut Framebuffer, theme: &Theme, wait_ms: u64, tries_left: u32) {
    let center = WIDTH / 2;
    centered(fb, &TITLE, center, 70, "Locked", theme.text);
    centered(fb, &NUMBER, center, 110, &duration(wait_ms), theme.text);
    let tries = format!("{tries_left} tries left before the keys are erased");
    centered(fb, &SMALL, center, 142, &tries, theme.muted);
}

fn draw_notice(fb: &mut Framebuffer, theme: &Theme, title: &str, first: &str, second: &str) {
    let center = WIDTH / 2;
    centered(fb, &TITLE, center, 80, title, theme.text);
    centered(fb, &BODY, center, 112, first, theme.muted);
    centered(fb, &BODY, center, 132, second, theme.muted);
}

fn draw_calibrate(fb: &mut Framebuffer, theme: &Theme, step: usize) {
    let (x, y) = CROSSES[step];
    fb.rect(x - 7, y, 15, 1, theme.text);
    fb.rect(x, y - 7, 1, 15, theme.text);
    let center = WIDTH / 2;
    centered(fb, &TITLE, center, 112, "Touch the cross", theme.text);
    let count = format!("{} of 3", step + 1);
    centered(fb, &SMALL, center, 136, &count, theme.muted);
}

impl Screen<'_> {
    /// What the keys do on this screen, for both drawing and acting.
    #[must_use]
    pub fn keys(&self) -> KeyBar {
        match *self {
            Self::Tab(tab, ..) => keys::tabs(tab),
            Self::ActivityList(..) => keys::activity_opened(),
            Self::DeviceMenu(_, selected) => keys::device_opened(if selected + 1 == DEVICE_ROWS {
                "Open"
            } else {
                "Change"
            }),
            Self::Review(_, seconds) | Self::RestoreConfirm { seconds, .. } => {
                keys::review(seconds)
            }
            Self::Passcode { .. } => keys::passcode(),
            Self::Result { .. } | Self::Notice { .. } => keys::dismiss("OK"),
            Self::SeedWords { page, .. } => keys::seed_page(page == 1),
            Self::BackupCheck { choices, .. } => keys::backup_check(choices),
            Self::Locked { .. } => keys::dismiss("Back"),
            Self::Progress { .. } | Self::Calibrate { .. } => keys::none(),
        }
    }

    /// Draws the whole screen.
    pub fn draw(&self, fb: &mut Framebuffer, theme: &Theme) {
        fb.clear(theme.bg);
        match *self {
            Self::Tab(Tab::Wallet, home, _) => draw_wallet(fb, theme, home),
            Self::Tab(Tab::Keys, home, _) => draw_keys(fb, theme, home),
            Self::Tab(Tab::Activity, _, activity) => draw_activity(fb, theme, activity, 0),
            Self::Tab(Tab::Device, home, _) => draw_device(fb, theme, home),
            Self::ActivityList(activity, top) => draw_activity(fb, theme, activity, top),
            Self::DeviceMenu(home, selected) => draw_device_menu(fb, theme, home, selected),
            Self::Review(tx, _) => draw_review(fb, theme, tx),
            Self::RestoreConfirm {
                tree, first_index, ..
            } => {
                draw_restore(fb, theme, tree, first_index);
            }
            Self::Passcode {
                title,
                hint,
                entered,
                digits,
            } => {
                draw_passcode(fb, theme, title, hint, entered, digits);
            }
            Self::Progress { title, done, total } => draw_progress(fb, theme, title, done, total),
            Self::Result {
                success,
                title,
                detail,
            } => {
                draw_result(fb, theme, success, title, detail);
            }
            Self::SeedWords { words, page } => draw_seed_words(fb, theme, words, page),
            Self::BackupCheck {
                number, position, ..
            } => {
                draw_backup_check(fb, theme, number, position);
            }
            Self::Locked {
                wait_ms,
                tries_left,
            } => draw_locked(fb, theme, wait_ms, tries_left),
            Self::Notice {
                title,
                first,
                second,
            } => draw_notice(fb, theme, title, first, second),
            Self::Calibrate { step } => draw_calibrate(fb, theme, step),
        }
        let bar = self.keys();
        if let Style::Tabs(current) = bar.style {
            draw_tabs(fb, theme, &bar, current);
        } else if bar.style == Style::Pills {
            draw_pills(fb, theme, &bar);
        }
    }
}

/// The keypad digit under a touch, as an index into the shuffled digits. Gaps count as nothing.
#[must_use]
pub fn keypad_hit(x: i32, y: i32) -> Option<usize> {
    let step_x = PAD_KEY_WIDTH + PAD_GAP_X;
    let step_y = PAD_KEY_HEIGHT + PAD_GAP_Y;
    let (dx, dy) = (x - PAD_LEFT, y - PAD_TOP);
    if dx < 0 || dy < 0 || dx % step_x >= PAD_KEY_WIDTH || dy % step_y >= PAD_KEY_HEIGHT {
        return None;
    }
    let (column, row) = (dx / step_x, dy / step_y);
    if column >= PAD_COLUMNS {
        return None;
    }
    let slot = usize::try_from(row * PAD_COLUMNS + column).ok()?;
    PAD_SLOTS.get(slot).copied().flatten()
}
