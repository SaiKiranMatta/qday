//! Retro Green / Amber: a monochrome CRT
//! text terminal on a 16 px grid, an inverse title bar, a DOS-style
//! function-key bar, dotted leaders and scanlines.

use embedded_graphics::mono_font::MonoFont;
use embedded_graphics::mono_font::ascii::{FONT_8X13, FONT_9X18_BOLD, FONT_10X20};

use qday_wallet::settings::Scheme;

use crate::canvas::{Canvas, Color, Rect};
use crate::nav::{self, Keys, Kind, Setting};
use crate::screens::*;
use crate::text;
use crate::text::Text;
use crate::theme::{self, Theme};
use crate::widgets::*;

#[derive(Clone, Copy)]
struct Crt {
    bg: Color,
    fg: Color,
    dim: Color,
    bright: Color,
    alert: Color,
}

const GREEN: Crt = Crt {
    bg: Color::hex(0x020A03),
    fg: Color::hex(0x33FF66),
    dim: Color::hex(0x1A8A3C),
    bright: Color::hex(0xC4FFD4),
    alert: Color::hex(0xFF5E5E),
};

const AMBER: Crt = Crt {
    bg: Color::hex(0x0B0600),
    fg: Color::hex(0xFFB000),
    dim: Color::hex(0x9A6A00),
    bright: Color::hex(0xFFE6A8),
    alert: Color::hex(0xFF5040),
};

fn k() -> Crt {
    if theme::current() == Theme::RetroAmber {
        AMBER
    } else {
        GREEN
    }
}

const F: &MonoFont = &FONT_8X13;
/// Characters per line at 8 px, inside an 8 px margin.
const COLS: usize = 38;
const FKEY_Y: i32 = 222;

fn tt(c: &mut Canvas, x: i32, y: i32, s: &str, col: Color) -> i32 {
    mono(c, x, y, s, F, col, true)
}

fn big(c: &mut Canvas, x: i32, y: i32, s: &str, col: Color) -> i32 {
    mono(c, x, y, s, &FONT_10X20, col, true)
}

/// Inverse video: a block of `fg` with the text in `bg`.
fn inverse(c: &mut Canvas, x: i32, y: i32, s: &str, font: &MonoFont, crt: Crt) -> i32 {
    let w = s.len() as f32 * font.character_size.width as f32;
    c.round_rect(
        Rect::new(x as f32, y as f32, w, font.character_size.height as f32),
        0.0,
        crt.fg,
        255,
    );
    mono(c, x, y, s, font, crt.bg, false);
    x + w as i32
}

/// `LABEL ........ VALUE`, `width` characters.
fn dots(label: &str, value: &str, width: usize) -> Text<64> {
    let fill = width.saturating_sub(label.len() + value.len() + 2);
    let mut t = Text::<64>::new();
    let _ = core::fmt::Write::write_fmt(&mut t, format_args!("{label} "));
    for _ in 0..fill {
        let _ = core::fmt::Write::write_char(&mut t, '.');
    }
    let _ = core::fmt::Write::write_fmt(&mut t, format_args!(" {value}"));
    t
}

/// Title bar: inverse, full width, `right` flush right.
fn title_bar(c: &mut Canvas, left: &str, right: &str) {
    let crt = k();
    let line = text!(
        48,
        " {:<w$}{} ",
        upper(left).as_str(),
        upper(right).as_str(),
        w = 38 - right.len()
    );
    inverse(c, 0, 0, line.as_str(), F, crt);
}

/// DOS-style function-key bar: key numbers, then inverse labels; merged keys
/// show both numbers (`1-2`).
fn fkeys(c: &mut Canvas, keys: &Keys, pressed: Option<usize>) {
    let crt = k();
    let groups = nav::groups(keys);
    for i in 0..4 {
        let covered = groups
            .iter()
            .flatten()
            .any(|g| (g.first..=g.last).contains(&i));
        if !covered {
            let x = i as i32 * 80;
            tt(c, x + 2, FKEY_Y, text!(2, "{}", i + 1).as_str(), crt.dim);
            c.round_rect(
                Rect::new(x as f32 + 12.0, FKEY_Y as f32, 64.0, 13.0),
                0.0,
                crt.dim,
                60,
            );
        }
    }
    for g in groups.into_iter().flatten() {
        let x = g.first as i32 * 80;
        let num = if g.first == g.last {
            text!(4, "{}", g.first + 1)
        } else {
            text!(4, "{}-{}", g.first + 1, g.last + 1)
        };
        let lx = tt(c, x + 2, FKEY_Y, num.as_str(), crt.fg) + 2;
        let width = ((g.last + 1) as i32 * 80 - 4 - lx) / 8;
        let label = match g.key.kind {
            Kind::Up => text!(48, "UP"),
            Kind::Down => text!(48, "DOWN"),
            Kind::Tab { opens: true, .. } => text!(48, "{}>", upper(g.key.label).as_str()),
            _ => upper(g.key.label),
        };
        let padded = text!(48, "{:<w$}", label.as_str(), w = width.max(0) as usize);
        let held = pressed.is_some_and(|i| (g.first..=g.last).contains(&i));
        let selected = matches!(g.key.kind, Kind::Tab { selected: true, .. });
        let is_tab = matches!(g.key.kind, Kind::Tab { .. });
        if held {
            c.round_rect(
                Rect::new(
                    lx as f32,
                    FKEY_Y as f32,
                    padded.as_str().len() as f32 * 8.0,
                    13.0,
                ),
                0.0,
                crt.bright,
                255,
            );
            mono(c, lx, FKEY_Y, padded.as_str(), F, crt.bg, false);
        } else if is_tab && !selected {
            tt(c, lx, FKEY_Y, label.as_str(), crt.fg);
        } else {
            inverse(c, lx, FKEY_Y, padded.as_str(), F, crt);
        }
    }
}

/// CRT finish: scanlines. (No darkened rim: on this panel it only dimmed the
/// title bar and key labels in the corners.)
fn crt_effects(c: &mut Canvas) {
    c.darken_rows(1, 2, 70);
}

fn prompt(c: &mut Canvas, y: i32, text: &str) {
    let crt = k();
    let end = tt(c, 8, y, text, crt.bright);
    c.round_rect(
        Rect::new(end as f32 + 4.0, y as f32, 8.0, 13.0),
        0.0,
        crt.fg,
        255,
    );
}

// --- tabs ------------------------------------------------------------------

fn status_text(s: &Status) -> Text<24> {
    let usb = if s.connected { "USB" } else { "---" };
    match s.scheme {
        Scheme::Xmss => text!(24, "TREE {}  #{:04} {usb}", s.tree, s.next_index),
        Scheme::MlDsa44 => text!(24, "{}  {usb}", scheme_label(s.scheme)),
    }
}

pub fn home(
    c: &mut Canvas,
    s: &Status,
    root: &[u8; 32],
    latest: Option<&Activity>,
    pressed: Option<usize>,
) {
    let crt = k();
    c.clear(crt.bg);
    // Key usage and the tree are the Keys screen's; home only says whether it's connected.
    title_bar(c, "QDAY/OS 0.7", if s.connected { "USB" } else { "---" });
    let subtitle = if s.scheme == Scheme::MlDsa44 {
        "QUANTUM-SAFE WALLET (ML-DSA-44)"
    } else {
        "QUANTUM-SAFE WALLET (XMSS/KECCAK)"
    };
    tt(c, 8, 22, subtitle, crt.dim);
    let (head, tail) = hex_parts(root);
    let account = upper(text!(20, "{}..{}", head.as_str(), tail.as_str()).as_str());
    let status = if s.connected { "READY" } else { "NO USB" };
    tt(
        c,
        8,
        44,
        dots("STATUS", status, COLS).as_str(),
        if s.connected { crt.fg } else { crt.alert },
    );
    tt(
        c,
        8,
        60,
        dots("ACCOUNT KEY", account.as_str(), COLS).as_str(),
        crt.fg,
    );
    tt(c, 8, 88, "LAST", crt.dim);
    match latest {
        Some(a) => {
            let mark = if a.approved { "OK " } else { "NO " };
            let line = text!(48, "{mark}{}", upper(a.title).as_str());
            tt(
                c,
                8,
                104,
                line.as_str(),
                if a.approved { crt.fg } else { crt.alert },
            );
            let detail = text!(48, "   {}", upper(a.detail).as_str());
            tt(c, 8, 120, detail.as_str(), crt.dim);
        }
        None => {
            tt(c, 8, 104, "(NOTHING SIGNED YET)", crt.dim);
        }
    }
    prompt(
        c,
        164,
        if s.connected {
            "QDAY> READY"
        } else {
            "QDAY> WAITING FOR HOST"
        },
    );
    fkeys(c, &nav::tab_keys(Tab::Home, false, Setting::Theme), pressed);
    crt_effects(c);
}

pub fn keys(c: &mut Canvas, s: &Status, pending: bool, pressed: Option<usize>) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "KEY TREE", status_text(s).as_str());
    let next = text!(8, "#{:04}", s.next_index);
    let used = text!(16, "{}/1024", s.next_index);
    tt(
        c,
        8,
        22,
        dots("NEXT KEY", next.as_str(), COLS).as_str(),
        crt.bright,
    );
    tt(c, 8, 38, dots("USED", used.as_str(), COLS).as_str(), crt.fg);
    tt(c, 8, 54, dots("ROTATE AT", "#0900", COLS).as_str(), crt.fg);
    tt(
        c,
        8,
        70,
        dots("ROTATION ONLY", "#1008-#1023", COLS).as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        86,
        dots("NEXT TREE", if pending { "READY" } else { "NOT YET" }, COLS).as_str(),
        crt.fg,
    );
    // One character per 32 keys: # used, > next, : rotation only, . unused.
    tt(c, 8, 110, "KEY MAP (1 CHAR = 32 KEYS)", crt.dim);
    let mut map = Text::<40>::new();
    let _ = core::fmt::Write::write_char(&mut map, '[');
    for cell in 0..32u32 {
        let (lo, hi) = (cell * 32, cell * 32 + 32);
        let ch = if s.next_index >= hi {
            '#'
        } else if s.next_index >= lo {
            '>'
        } else if lo >= RESERVED_FROM {
            ':'
        } else {
            '.'
        };
        let _ = core::fmt::Write::write_char(&mut map, ch);
    }
    let _ = core::fmt::Write::write_char(&mut map, ']');
    tt(c, 8, 126, map.as_str(), crt.fg);
    let pct = s.next_index * 100 / LEAVES;
    tt(c, 8, 142, text!(24, "{pct}% USED").as_str(), crt.dim);
    fkeys(c, &nav::tab_keys(Tab::Keys, false, Setting::Theme), pressed);
    crt_effects(c);
}

pub fn activity(
    c: &mut Canvas,
    s: &Status,
    items: &[Activity],
    offset: usize,
    opened: bool,
    pressed: Option<usize>,
) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "ACTIVITY LOG", status_text(s).as_str());
    if items.is_empty() {
        tt(c, 8, 30, "NO ENTRIES.", crt.fg);
        tt(c, 8, 46, "SIGNATURES YOU APPROVE APPEAR HERE.", crt.dim);
    }
    for (i, item) in items.iter().skip(offset).take(ACTIVITY_ROWS).enumerate() {
        let y = 24 + i as i32 * 44;
        let mark = if item.approved { "OK" } else { "NO" };
        let key = item.index.map(|n| text!(8, "#{n:04}")).unwrap_or_default();
        let line = dots(
            text!(48, "{mark} {}", upper(item.title).as_str()).as_str(),
            key.as_str(),
            COLS,
        );
        tt(
            c,
            8,
            y,
            line.as_str(),
            if item.approved { crt.fg } else { crt.alert },
        );
        tt(c, 32, y + 16, upper(item.detail).as_str(), crt.dim);
    }
    if items.len() > ACTIVITY_ROWS {
        let more = text!(
            24,
            "{}-{}/{}",
            offset + 1,
            (offset + ACTIVITY_ROWS).min(items.len()),
            items.len()
        );
        tt(
            c,
            312 - more.as_str().len() as i32 * 8,
            200,
            more.as_str(),
            if opened { crt.bright } else { crt.dim },
        );
    }
    fkeys(
        c,
        &nav::tab_keys(Tab::Activity, opened, Setting::Theme),
        pressed,
    );
    crt_effects(c);
}

pub fn device(
    c: &mut Canvas,
    s: &Status,
    info: &DeviceInfo,
    setting: Setting,
    opened: bool,
    pressed: Option<usize>,
) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "DEVICE", status_text(s).as_str());
    let (head, tail) = hex_parts(info.root);
    let account = upper(text!(20, "{}..{}", head.as_str(), tail.as_str()).as_str());
    let counter = text!(12, "{}", info.counter);
    tt(
        c,
        8,
        22,
        dots("ACCOUNT KEY", account.as_str(), COLS).as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        38,
        dots("ONE-WAY COUNTER", counter.as_str(), COLS).as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        54,
        dots("FIRMWARE", upper(info.firmware).as_str(), COLS).as_str(),
        crt.fg,
    );
    tt(c, 8, 86, "SETTINGS", crt.dim);
    let name = upper(theme::current().name());
    let rows = [
        (Setting::Theme, dots("THEME", name.as_str(), COLS - 2)),
        (Setting::Passcode, dots("PASSCODE", "6 DIGITS", COLS - 2)),
        (Setting::Backup, dots("BACK UP SEED", "24 WORDS", COLS - 2)),
        (
            Setting::Signing,
            dots("SIGNING", scheme_label(s.scheme), COLS - 2),
        ),
    ];
    for (i, (which, line)) in rows.iter().enumerate() {
        let y = 102 + i as i32 * 16;
        if opened && *which == setting {
            tt(c, 8, y, ">", crt.bright);
            inverse(c, 24, y, line.as_str(), F, crt);
        } else {
            tt(c, 24, y, line.as_str(), crt.fg);
        }
    }
    fkeys(c, &nav::tab_keys(Tab::Device, opened, setting), pressed);
    crt_effects(c);
}

// --- signing ---------------------------------------------------------------

fn address(c: &mut Canvas, y: i32, addr: &str, crt: Crt) {
    match address_groups(addr) {
        Some(g) => {
            let a = upper(text!(32, "0x{} {} {} {} {}", g[0], g[1], g[2], g[3], g[4]).as_str());
            let b = upper(text!(32, "{} {} {} {} {}", g[5], g[6], g[7], g[8], g[9]).as_str());
            tt(c, 8, y, a.as_str(), crt.fg);
            tt(c, 24, y + 16, b.as_str(), crt.fg);
        }
        None => {
            tt(c, 8, y, upper(addr).as_str(), crt.fg);
        }
    }
}

pub fn review(c: &mut Canvas, info: &ReviewInfo, pressed: Option<usize>) {
    let crt = k();
    c.clear(crt.bg);
    let chain = if info.chain_id != 0 {
        text!(20, "CHAIN {}", info.chain_id)
    } else {
        text!(20, "")
    };
    title_bar(
        c,
        if info.review.is_risky() {
            "!!! CHECK CAREFULLY !!!"
        } else {
            "*** SIGN REQUEST ***"
        },
        chain.as_str(),
    );
    match &info.review {
        Review::Send { amount, unit, to } => {
            tt(c, 8, 22, "SEND", crt.dim);
            big(
                c,
                8,
                36,
                upper(text!(40, "{amount} {unit}").as_str()).as_str(),
                crt.bright,
            );
            tt(c, 8, 64, "TO", crt.dim);
            address(c, 80, to, crt);
        }
        Review::Token { amount, to, token } => {
            let short = upper(
                text!(
                    16,
                    "{}..{}",
                    &token[..token.len().min(6)],
                    &token[token.len().saturating_sub(4)..]
                )
                .as_str(),
            );
            tt(
                c,
                8,
                22,
                text!(40, "TOKEN {}, RAW UNITS", short.as_str()).as_str(),
                crt.dim,
            );
            big(c, 8, 36, amount, crt.bright);
            tt(c, 8, 64, "TO", crt.dim);
            address(c, 80, to, crt);
        }
        Review::Call {
            contract,
            value,
            function,
        }
        | Review::UnknownCall {
            contract,
            value,
            function,
        } => {
            tt(
                c,
                8,
                22,
                text!(40, "CONTRACT CALL, SENDS {}", upper(value).as_str()).as_str(),
                crt.dim,
            );
            big(c, 8, 36, upper(function).as_str(), crt.bright);
            tt(c, 8, 64, "CONTRACT", crt.dim);
            address(c, 80, contract, crt);
        }
        Review::Batch { calls } => {
            big(
                c,
                8,
                28,
                text!(24, "BATCH OF {calls} CALLS").as_str(),
                crt.alert,
            );
            tt(c, 8, 64, "THE DEVICE CAN'T SHOW EACH CALL.", crt.fg);
        }
        Review::Unknown { selector } => {
            big(c, 8, 28, "UNKNOWN ACTION", crt.alert);
            tt(
                c,
                8,
                64,
                upper(text!(32, "FUNCTION {selector}").as_str()).as_str(),
                crt.fg,
            );
        }
        Review::Blind { description, hash } => {
            big(c, 8, 28, "BLIND SIGNATURE", crt.alert);
            tt(c, 8, 64, "LAPTOP SAYS (NOT CHECKED):", crt.dim);
            tt(c, 8, 80, upper(description).as_str(), crt.fg);
            tt(c, 8, 96, upper(hash).as_str(), crt.dim);
        }
    }
    let key = if info.rotation {
        text!(24, "#{:04}+NEXT TREE", info.index)
    } else {
        text!(24, "#{:04}", info.index)
    };
    tt(
        c,
        8,
        120,
        dots("MAX FEE", upper(info.fee).as_str(), COLS).as_str(),
        crt.fg,
    );
    let key_row = match info.scheme {
        _ if info.signature.is_some() => dots(
            "SIGNATURE",
            upper(info.signature.unwrap_or("")).as_str(),
            COLS,
        ),
        Scheme::Xmss => dots("ONE-TIME KEY", key.as_str(), COLS),
        Scheme::MlDsa44 => dots("SIGNATURE", scheme_label(info.scheme), COLS),
    };
    tt(c, 8, 136, key_row.as_str(), crt.fg);
    let left = (info.seconds_left * 20 / info.timeout.max(1)) as usize;
    let mut bar = Text::<48>::new();
    let _ = core::fmt::Write::write_str(&mut bar, "TIMEOUT [");
    for i in 0..20 {
        let _ = core::fmt::Write::write_char(&mut bar, if i < left { '=' } else { '-' });
    }
    let _ = core::fmt::Write::write_fmt(&mut bar, format_args!("] {}S", info.seconds_left));
    tt(c, 8, 160, bar.as_str(), crt.dim);
    tt(c, 8, 176, "APPROVE ONLY IF THIS MATCHES", crt.alert);
    fkeys(c, &nav::review_keys(), pressed);
    crt_effects(c);
}

pub fn passcode(c: &mut Canvas, pc: &Passcode, pressed: Option<usize>) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, &upper(pc.title), "");
    tt(c, 8, 26, upper(pc.hint).as_str(), crt.dim);
    tt(c, 8, 50, "CODE:", crt.dim);
    let mut code = Text::<16>::new();
    for i in 0..6 {
        let _ = core::fmt::Write::write_str(&mut code, if i < pc.entered { "* " } else { "_ " });
    }
    mono(
        c,
        8,
        66,
        code.as_str(),
        &FONT_9X18_BOLD,
        if pc.error.is_some() {
            crt.alert
        } else {
            crt.bright
        },
        true,
    );
    match pc.error {
        Some(e) => {
            let e = upper(e);
            let (a, b) = split_to_fit(&e, 128, |t| t.len() as i32 * 8);
            tt(c, 8, 100, a, crt.alert);
            tt(c, 8, 116, b, crt.alert);
        }
        None => {
            tt(c, 8, 100, "DIGITS MOVE", crt.dim);
            tt(c, 8, 116, "EVERY TIME", crt.dim);
        }
    }
    for i in 0..12 {
        let Some(d) = digit_in_slot(pc.digits, i) else {
            continue;
        };
        let r = key_rect(i);
        let label = text!(2, "{d}");
        let (tx, ty) = (r.x as i32 + 20, r.y as i32 + 10);
        if pc.pressed == Some(i) {
            c.round_rect(r, 0.0, crt.fg, 255);
            mono(c, tx, ty, label.as_str(), &FONT_10X20, crt.bg, false);
        } else {
            c.round_rect_stroke(r, 0.0, 1.0, crt.dim, 255);
            big(c, tx, ty, label.as_str(), crt.fg);
        }
    }
    fkeys(c, &nav::passcode_keys(), pressed);
    crt_effects(c);
}

pub fn progress(c: &mut Canvas, title: &str, lines: &[&str], done: u32, total: u32) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, title, "");
    for (i, l) in lines.iter().take(3).enumerate() {
        tt(
            c,
            8,
            22 + 16 * i as i32,
            upper(l).as_str(),
            if i == 0 { crt.fg } else { crt.dim },
        );
    }
    let pct = (done as u64 * 100 / total.max(1) as u64) as u32;
    big(c, 8, 80, text!(6, "{pct}%").as_str(), crt.bright);
    let filled = (done as u64 * 36 / total.max(1) as u64) as usize;
    let mut bar = Text::<40>::new();
    let _ = core::fmt::Write::write_char(&mut bar, '[');
    for i in 0..36 {
        let _ = core::fmt::Write::write_char(&mut bar, if i < filled { '#' } else { '.' });
    }
    let _ = core::fmt::Write::write_char(&mut bar, ']');
    tt(c, 8, 110, bar.as_str(), crt.fg);
    tt(
        c,
        8,
        136,
        dots("LEAVES", text!(16, "{done}/{total}").as_str(), COLS).as_str(),
        crt.fg,
    );
    crt_effects(c);
}

pub fn result(c: &mut Canvas, ok: bool, title: &str, detail: &str) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "QDAY/OS 0.7", "");
    let banner = text!(40, "*** {} ***", upper(title).as_str());
    let x = (320 - banner.as_str().len() as i32 * 10) / 2;
    big(
        c,
        x,
        70,
        banner.as_str(),
        if ok { crt.bright } else { crt.alert },
    );
    let d = upper(detail);
    let (a, b) = split_to_fit(&d, 304, |t| t.len() as i32 * 8);
    tt(c, (320 - a.len() as i32 * 8) / 2, 112, a, crt.dim);
    tt(c, (320 - b.len() as i32 * 8) / 2, 128, b, crt.dim);
    prompt(c, 160, "QDAY>");
    crt_effects(c);
}

pub fn seed_words(c: &mut Canvas, words: &[&str; 24], page: usize, pressed: Option<usize>) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "WRITE THESE DOWN", text!(8, "{}/2", page + 1).as_str());
    tt(c, 8, 22, "ANYONE WITH THESE WORDS HAS THE KEYS", crt.alert);
    for slot in 0..12 {
        let n = page * 12 + slot;
        let (col, row) = ((slot % 3) as i32, (slot / 3) as i32);
        let line = text!(16, "{:02} {}", n + 1, upper(words[n]).as_str());
        tt(c, 8 + col * 104, 50 + row * 24, line.as_str(), crt.fg);
    }
    fkeys(c, &nav::seed_keys(page), pressed);
    crt_effects(c);
}

pub fn seed_check(
    c: &mut Canvas,
    step: usize,
    position: usize,
    options: &[&'static str; 4],
    wrong: bool,
    pressed: Option<usize>,
) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "BACKUP CHECK", text!(8, "{}/3", step + 1).as_str());
    tt(c, 8, 30, "FROM YOUR WRITTEN LIST:", crt.dim);
    big(
        c,
        8,
        54,
        text!(24, "WHICH IS WORD #{:02}?", position + 1).as_str(),
        crt.bright,
    );
    if wrong {
        tt(c, 8, 96, "NOT THAT ONE. CHECK YOUR LIST.", crt.alert);
    } else {
        tt(c, 8, 96, "PRESS THE KEY UNDER THE WORD.", crt.dim);
    }
    fkeys(c, &nav::check_keys(options), pressed);
    crt_effects(c);
}

pub fn locked(c: &mut Canvas, wait: &str, attempts_left: u32) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "!!! LOCKED !!!", "");
    tt(c, 8, 30, "TOO MANY WRONG PASSCODES.", crt.alert);
    big(c, 8, 60, upper(wait).as_str(), crt.bright);
    tt(
        c,
        8,
        100,
        text!(48, "{attempts_left} MORE WRONG CODES ERASE THE KEYS.").as_str(),
        crt.fg,
    );
    crt_effects(c);
}

pub fn restore_confirm(
    c: &mut Canvas,
    tree: u32,
    first_index: u32,
    seconds_left: u32,
    _timeout: u32,
    pressed: Option<usize>,
) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "!!! CHECK CAREFULLY !!!", "");
    big(c, 8, 28, "RESTORE KEYS?", crt.bright);
    tt(c, 8, 64, "REPLACES THE KEYS ON THIS DEVICE WITH", crt.fg);
    tt(c, 8, 80, "ONES FROM THE SEED YOUR LAPTOP SENT.", crt.fg);
    tt(
        c,
        8,
        104,
        dots("KEY TREE", text!(8, "{tree}").as_str(), COLS).as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        120,
        dots("FIRST KEY", text!(8, "#{first_index:04}").as_str(), COLS).as_str(),
        crt.fg,
    );
    tt(c, 8, 136, "USED KEYS ARE SKIPPED, PLUS 64 MORE.", crt.dim);
    tt(
        c,
        8,
        176,
        text!(32, "TIMEOUT {seconds_left}S").as_str(),
        crt.dim,
    );
    fkeys(c, &nav::review_keys(), pressed);
    crt_effects(c);
}

pub fn notice(c: &mut Canvas, title: &str, detail: &str) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "QDAY/OS 0.7", "");
    big(c, 8, 70, upper(title).as_str(), crt.bright);
    tt(c, 8, 104, upper(detail).as_str(), crt.fg);
    crt_effects(c);
}

pub fn keys_ml(c: &mut Canvas, s: &Status, fingerprint: &[u8; 32], pressed: Option<usize>) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "SIGNING KEY", status_text(s).as_str());
    let (head, tail) = hex_parts(fingerprint);
    let key = upper(text!(20, "{}..{}", head.as_str(), tail.as_str()).as_str());
    tt(
        c,
        8,
        22,
        dots("FINGERPRINT", key.as_str(), COLS).as_str(),
        crt.bright,
    );
    tt(
        c,
        8,
        38,
        dots("SCHEME", scheme_label(s.scheme), COLS).as_str(),
        crt.fg,
    );
    tt(c, 8, 54, dots("KEY LIMIT", "NONE", COLS).as_str(), crt.fg);
    tt(c, 8, 86, "ONE KEY SIGNS EVERY TRANSFER.", crt.dim);
    fkeys(c, &nav::tab_keys(Tab::Keys, false, Setting::Theme), pressed);
    crt_effects(c);
}

pub fn scheme_confirm(
    c: &mut Canvas,
    target: Scheme,
    seconds_left: u32,
    _timeout: u32,
    pressed: Option<usize>,
) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "!!! CHECK CAREFULLY !!!", "");
    big(
        c,
        8,
        28,
        text!(32, "SWITCH TO {}?", scheme_label(target)).as_str(),
        crt.bright,
    );
    tt(c, 8, 64, "YOUR ACCOUNT CHANGES.", crt.fg);
    tt(c, 8, 80, "FUNDS DON'T MOVE.", crt.fg);
    tt(
        c,
        8,
        104,
        dots("SIGNING NOW", scheme_label(other_scheme(target)), COLS).as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        120,
        dots("SIGNING NEXT", scheme_label(target), COLS).as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        176,
        text!(32, "TIMEOUT {seconds_left}S").as_str(),
        crt.dim,
    );
    fkeys(c, &nav::review_keys(), pressed);
    crt_effects(c);
}

pub fn share_confirm(c: &mut Canvas, info: &ShareInfo, pressed: Option<usize>) {
    let crt = k();
    c.clear(crt.bg);
    title_bar(c, "*** SHARE WITH LAPTOP ***", "");
    big(c, 8, 28, upper(info.title).as_str(), crt.bright);
    tt(c, 8, 64, upper(info.detail).as_str(), crt.fg);
    tt(c, 8, 80, "FUNDS DON'T MOVE.", crt.fg);
    tt(
        c,
        8,
        104,
        dots("GIVES", upper(info.gives).as_str(), COLS).as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        120,
        dots(
            "PASSCODE",
            if info.passcode {
                "NEEDED"
            } else {
                "NOT NEEDED"
            },
            COLS,
        )
        .as_str(),
        crt.fg,
    );
    tt(
        c,
        8,
        176,
        text!(32, "TIMEOUT {}S", info.seconds_left).as_str(),
        crt.dim,
    );
    fkeys(c, &nav::review_keys(), pressed);
    crt_effects(c);
}
