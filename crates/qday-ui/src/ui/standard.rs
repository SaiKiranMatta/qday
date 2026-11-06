//! Standard Light / Dark: monochrome,
//! light grey with cards a step darker, black pills, green only for success,
//! greyed secondary digits, text tabs and a dot-matrix key map.

use crate::canvas::{Canvas, Color, Rect, TextStyle};
use crate::fonts::{BODY, BODY_STRONG, CAPTION, DISPLAY, Font, HEADLINE, TITLE};
use crate::nav::{self, Keys, Kind, Setting};
use crate::screens::*;
use crate::text;
use crate::theme::{self, Theme};
use crate::widgets::*;

/// Greys round to neutral in RGB565.
struct P {
    bg: Color,
    card: Color,
    fill: Color,
    text: Color,
    muted: Color,
    dot: Color,
    primary: Color,
    on_primary: Color,
    success: Color,
    warning: Color,
    danger: Color,
    eth: Color,
}

const LIGHT: P = P {
    bg: Color::hex(0xF7F7F7),
    card: Color::hex(0xE7E7E7),
    fill: Color::hex(0xD6D6D6),
    text: Color::hex(0x101010),
    muted: Color::hex(0x8C8C8C),
    dot: Color::hex(0xCECECE),
    primary: Color::hex(0x101010),
    on_primary: Color::hex(0xFFFFFF),
    success: Color::hex(0x2FC651),
    warning: Color::hex(0xFF9500),
    danger: Color::hex(0xFF3B30),
    eth: Color::hex(0x627EEA),
};

const DARK: P = P {
    bg: Color::hex(0x212121),
    card: Color::hex(0x313131),
    fill: Color::hex(0x424242),
    text: Color::hex(0xF7F7F7),
    muted: Color::hex(0x8C8C8C),
    dot: Color::hex(0x4A4A4A),
    primary: Color::hex(0xF7F7F7),
    on_primary: Color::hex(0x212121),
    ..LIGHT
};

fn p() -> &'static P {
    if theme::current() == Theme::StandardDark {
        &DARK
    } else {
        &LIGHT
    }
}

fn ts(font: &'static Font, color: Color) -> TextStyle {
    TextStyle::new(font, color)
}

/// Runs of differently styled text on one baseline; returns the end x.
fn runs(c: &mut Canvas, x: i32, y: i32, parts: &[(&str, TextStyle)]) -> i32 {
    parts.iter().fold(x, |x, (s, style)| c.text(x, y, s, style))
}

fn runs_right(c: &mut Canvas, right: i32, y: i32, parts: &[(&str, TextStyle)]) {
    let w: i32 = parts.iter().map(|(s, style)| style.width(s)).sum();
    runs(c, right - w, y, parts);
}

/// The largest of `fonts` that fits `text` in `width`.
fn fit(text: &str, width: i32, fonts: &[&'static Font], color: Color) -> TextStyle {
    for f in fonts {
        let style = ts(f, color);
        if style.width(text) <= width {
            return style;
        }
    }
    ts(fonts[fonts.len() - 1], color)
}

// --- shared pieces ---------------------------------------------------------

fn header(c: &mut Canvas, s: &Status) {
    let p = p();
    c.text(16, 24, "qday", &ts(&TITLE, p.text));
    let (dot, label) = if s.connected {
        (p.success, "USB")
    } else {
        (p.muted, "No USB")
    };
    let style = ts(&CAPTION, p.muted);
    let x = 306 - style.width(label);
    c.circle(x as f32 - 8.0, 19.0, 3.5, dot, 255);
    c.text(x, 23, label, &style);
}

/// The soft-key controls: text tabs, or pills spanning their keys.
fn controls(c: &mut Canvas, keys: &Keys, pressed: Option<usize>) {
    let p = p();
    for g in nav::groups(keys).into_iter().flatten() {
        let held = pressed.is_some_and(|i| (g.first..=g.last).contains(&i));
        let k = g.key;
        if let Kind::Tab { selected, opens } = k.kind {
            let cx = 40.0 + g.first as f32 * 80.0;
            if held {
                let (l, r) = key_column(g.first, g.last);
                c.round_rect(Rect::new(l, 213.0, r - l, 21.0), 10.5, p.card, 255);
            }
            let style = ts(&BODY_STRONG, if selected { p.text } else { p.muted });
            let w = style.width(k.label) + if opens { 9 } else { 0 };
            let x = c.text(cx as i32 - w / 2, 229, k.label, &style);
            if opens {
                chevron(c, x as f32 + 5.0, 224.5, 3.5, p.text, false);
            }
            continue;
        }
        let (l, r) = key_column(g.first, g.last);
        let rect = Rect::new(l, CONTROLS_Y, r - l, CONTROLS_H);
        let (fill, ink) = match k.kind {
            Kind::Approve | Kind::Primary => (p.primary, p.on_primary),
            _ => (p.fill, p.text),
        };
        c.round_rect(rect, CONTROLS_H / 2.0, fill, 255);
        if held {
            c.round_rect(rect, CONTROLS_H / 2.0, p.muted, 90);
        }
        let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        match k.kind {
            Kind::Up | Kind::Down => arrow(c, cx, cy, 12.0, ink, k.kind == Kind::Down),
            Kind::Back => {
                let style = ts(&BODY_STRONG, ink);
                let w = style.width(k.label) + 12;
                chevron(c, cx - w as f32 / 2.0 + 2.0, cy, 3.5, ink, true);
                c.text(cx as i32 - w / 2 + 12, cy as i32 + 5, k.label, &style);
            }
            _ => c.text_centered(cx as i32, cy as i32 + 5, k.label, &ts(&BODY_STRONG, ink)),
        }
    }
}

/// Time left to decide, in K3's column (which does nothing).
fn countdown(c: &mut Canvas, seconds_left: u32, timeout: u32) {
    let p = p();
    let (cx, cy) = (200.0, 211.0);
    c.ring(cx, cy, 13.0, 2.5, p.dot, 255);
    let left = seconds_left as f32 / timeout.max(1) as f32;
    c.arc(cx, cy, 13.0, 2.5, 0.0, left, p.text, 255);
    c.text_centered(
        cx as i32,
        cy as i32 + 4,
        text!(4, "{seconds_left}").as_str(),
        &ts(&CAPTION, p.text),
    );
}

enum Row<'a> {
    Pair(&'a str, &'a str),
    /// A full address on two lines, groups of four, ends emphasised.
    Address(&'a str, &'a str),
}

const ROW_H: f32 = 23.0;
const ADDRESS_H: f32 = 40.0;

/// A card of label / value rows starting at `top`; returns its bottom.
fn card_rows(c: &mut Canvas, top: f32, rows: &[Row]) -> f32 {
    let p = p();
    let h: f32 = rows
        .iter()
        .map(|r| match r {
            Row::Pair(..) => ROW_H,
            Row::Address(_, a) if address_groups(a).is_some() => ADDRESS_H,
            Row::Address(..) => ROW_H,
        })
        .sum::<f32>()
        + 8.0;
    c.round_rect(Rect::new(12.0, top, 296.0, h), 12.0, p.card, 255);
    let mut y = top + 4.0;
    let label = ts(&BODY_STRONG, p.text);
    for row in rows {
        match row {
            Row::Pair(k, v) => {
                let base = (y + ROW_H / 2.0 + 4.5) as i32;
                c.text(26, base, k, &label);
                c.text_right(294, base, v, &ts(&BODY, p.text));
                y += ROW_H;
            }
            Row::Address(k, a) => match address_groups(a) {
                Some(g) => {
                    let base = y as i32 + 16;
                    c.text(26, base, k, &label);
                    let (hi, lo) = (ts(&BODY, p.text), ts(&BODY, p.muted));
                    let sp = (" ", lo);
                    runs_right(
                        c,
                        294,
                        base,
                        &[
                            ("0x", lo),
                            (g[0], hi),
                            sp,
                            (g[1], lo),
                            sp,
                            (g[2], lo),
                            sp,
                            (g[3], lo),
                            sp,
                            (g[4], lo),
                        ],
                    );
                    runs_right(
                        c,
                        294,
                        base + 17,
                        &[
                            (g[5], lo),
                            sp,
                            (g[6], lo),
                            sp,
                            (g[7], lo),
                            sp,
                            (g[8], hi),
                            sp,
                            (g[9], hi),
                        ],
                    );
                    y += ADDRESS_H;
                }
                None => {
                    let base = (y + ROW_H / 2.0 + 4.5) as i32;
                    c.text(26, base, k, &label);
                    c.text_right(294, base, a, &ts(&BODY, p.text));
                    y += ROW_H;
                }
            },
        }
    }
    top + h
}

fn tick_circle(c: &mut Canvas, cx: f32, cy: f32, r: f32, ok: bool) {
    let p = p();
    if ok {
        c.circle(cx, cy, r, p.text, 255);
        icon_check(c, cx, cy, r, p.bg);
    } else {
        c.circle(cx, cy, r, p.fill, 255);
        icon_cross(c, cx, cy, r, p.text);
    }
}

// --- tabs ------------------------------------------------------------------

pub fn home(
    c: &mut Canvas,
    s: &Status,
    root: &[u8; 32],
    pending: bool,
    latest: Option<&Activity>,
    pressed: Option<usize>,
) {
    let p = p();
    c.clear(p.bg);
    header(c, s);
    c.text(16, 48, "Account key", &ts(&CAPTION, p.muted));
    let (head, tail) = hex_parts(root);
    runs(
        c,
        14,
        84,
        &[
            (head.as_str(), ts(&DISPLAY, p.text)),
            ("...", ts(&DISPLAY, p.muted)),
            (tail.as_str(), ts(&DISPLAY, p.text)),
        ],
    );
    if s.connected {
        c.circle(22.0, 99.0, 6.0, p.success, 255);
        icon_check(c, 22.0, 99.0, 5.5, Color::hex(0xFFFFFF));
        c.text(33, 104, "Ready to sign", &ts(&BODY_STRONG, p.success));
    } else {
        c.circle(22.0, 99.0, 4.0, p.muted, 255);
        c.text(33, 104, "Plug in USB to sign", &ts(&BODY_STRONG, p.muted));
    }

    let used = text!(8, "{}", s.next_index);
    let tree = text!(8, "{}", s.tree);
    let tree_note = if pending {
        "  next tree ready"
    } else {
        "  rotates at 900"
    };
    let cards: [(&str, &str, &str); 2] = [
        ("Signatures", used.as_str(), " / 1,024"),
        ("Tree", tree.as_str(), tree_note),
    ];
    for (i, (title, a, b)) in cards.iter().enumerate() {
        let r = Rect::new(12.0 + i as f32 * 152.0, 116.0, 144.0, 50.0);
        c.round_rect(r, 12.0, p.card, 255);
        c.text(
            r.x as i32 + 12,
            r.y as i32 + 18,
            title,
            &ts(&CAPTION, p.text),
        );
        c.circle(r.x + r.w - 16.0, r.y + 14.0, 7.5, p.text, 255);
        if i == 0 {
            key_glyph(c, r.x + r.w - 16.0, r.y + 14.0, p.bg);
        } else {
            tree_glyph(c, r.x + r.w - 16.0, r.y + 14.0, p.bg);
        }
        runs(
            c,
            r.x as i32 + 12,
            r.y as i32 + 41,
            &[(a, ts(&TITLE, p.text)), (b, ts(&CAPTION, p.muted))],
        );
    }

    let row = Rect::new(12.0, 174.0, 296.0, 28.0);
    c.round_rect(row, 12.0, p.card, 255);
    match latest {
        Some(a) => {
            tick_circle(c, 28.0, 188.0, 7.0, a.approved);
            c.text(42, 193, a.title, &ts(&BODY_STRONG, p.text));
            if let Some(i) = a.index {
                c.text_right(
                    286,
                    193,
                    text!(8, "#{i:04}").as_str(),
                    &ts(&CAPTION, p.muted),
                );
            }
        }
        None => {
            c.text(26, 193, "No activity yet", &ts(&BODY_STRONG, p.muted));
        }
    }
    chevron(c, 296.0, 188.0, 3.5, p.text, false);
    controls(c, &nav::tab_keys(Tab::Home, false, Setting::Theme), pressed);
}

pub fn keys(c: &mut Canvas, s: &Status, pending: bool, pressed: Option<usize>) {
    let p = p();
    c.clear(p.bg);
    header(c, s);
    c.text(16, 48, "Next key", &ts(&CAPTION, p.muted));
    let (zeros, num) = key_parts(s.next_index);
    runs(
        c,
        14,
        84,
        &[
            (zeros.as_str(), ts(&DISPLAY, p.muted)),
            (num.as_str(), ts(&DISPLAY, p.text)),
        ],
    );
    c.text_right(
        306,
        64,
        text!(16, "Tree {}", s.tree).as_str(),
        &ts(&BODY_STRONG, p.text),
    );
    c.text_right(
        306,
        82,
        text!(24, "{} of 1,024 used", s.next_index).as_str(),
        &ts(&CAPTION, p.muted),
    );

    c.round_rect(Rect::new(12.0, 96.0, 296.0, 110.0), 12.0, p.card, 255);
    let pitch = 4.25;
    key_map(
        c,
        24.0 + pitch / 2.0,
        108.0 + pitch / 2.0,
        pitch,
        64,
        s.next_index,
        RESERVED_FROM,
        [p.text, p.success, p.warning, p.dot],
    );
    // Legend.
    let mut x = 26;
    for (col, name) in [
        (p.text, "used"),
        (p.success, "next"),
        (p.warning, "rotation only"),
    ] {
        c.circle(x as f32 + 2.0, 192.0, 2.5, col, 255);
        x = c.text(x + 9, 196, name, &ts(&CAPTION, p.muted)) + 12;
    }
    let (note, col) = if pending {
        ("next tree ready", p.success)
    } else {
        ("rotates at 900", p.muted)
    };
    c.text_right(294, 196, note, &ts(&CAPTION, col));
    controls(c, &nav::tab_keys(Tab::Keys, false, Setting::Theme), pressed);
}

pub fn activity(
    c: &mut Canvas,
    s: &Status,
    items: &[Activity],
    offset: usize,
    opened: bool,
    pressed: Option<usize>,
) {
    let p = p();
    c.clear(p.bg);
    header(c, s);
    // Opened, the pills take more room than the text tabs: drop the title row.
    let (card_h, rows_y) = if opened { (148.0, 38.0) } else { (172.0, 62.0) };
    c.round_rect(Rect::new(12.0, 34.0, 296.0, card_h), 12.0, p.card, 255);
    if !opened {
        c.text(26, 54, "Recent activity", &ts(&BODY_STRONG, p.text));
    }
    if items.len() > ACTIVITY_ROWS {
        let shown = text!(
            24,
            "{}-{} of {}",
            offset + 1,
            (offset + ACTIVITY_ROWS).min(items.len()),
            items.len()
        );
        let style = ts(&CAPTION, p.muted);
        if opened {
            c.text_centered(160, 23, shown.as_str(), &style);
        } else {
            c.text_right(294, 54, shown.as_str(), &style);
        }
    }
    if items.is_empty() {
        c.text_centered(160, 118, "No activity yet", &ts(&TITLE, p.text));
        c.text_centered(
            160,
            138,
            "Signatures you approve appear here",
            &ts(&BODY, p.muted),
        );
    }
    for (i, item) in items.iter().skip(offset).take(ACTIVITY_ROWS).enumerate() {
        let y = rows_y + i as f32 * 35.0;
        tick_circle(c, 32.0, y + 17.0, 11.0, item.approved);
        c.text(52, y as i32 + 15, item.title, &ts(&BODY_STRONG, p.text));
        c.text(52, y as i32 + 29, item.detail, &ts(&CAPTION, p.muted));
        if let Some(index) = item.index {
            c.text_right(
                290,
                y as i32 + 15,
                text!(8, "#{index:04}").as_str(),
                &ts(&BODY_STRONG, p.text),
            );
            c.text_right(290, y as i32 + 29, "key", &ts(&CAPTION, p.muted));
        }
    }
    // Scroll position, while opened.
    if opened && items.len() > ACTIVITY_ROWS {
        let track = Rect::new(300.0, 44.0, 3.0, card_h - 20.0);
        c.round_rect(track, 1.5, p.fill, 255);
        let h = track.h * ACTIVITY_ROWS as f32 / items.len() as f32;
        let y = track.y + (track.h - h) * offset as f32 / (items.len() - ACTIVITY_ROWS) as f32;
        c.round_rect(Rect::new(track.x, y, track.w, h), 1.5, p.text, 255);
    }
    controls(
        c,
        &nav::tab_keys(Tab::Activity, opened, Setting::Theme),
        pressed,
    );
}

pub fn device(
    c: &mut Canvas,
    s: &Status,
    info: &DeviceInfo,
    setting: Setting,
    opened: bool,
    pressed: Option<usize>,
) {
    let p = p();
    c.clear(p.bg);
    header(c, s);
    let counter = text!(12, "{}", info.counter);
    let root = short_hex(info.root);
    let facts = [
        ("Account key", root.as_str()),
        ("One-way counter", counter.as_str()),
    ];
    c.round_rect(Rect::new(12.0, 34.0, 296.0, 48.0), 12.0, p.card, 255);
    for (i, (k, v)) in facts.iter().enumerate() {
        let y = 53 + 20 * i as i32;
        c.text(26, y, k, &ts(&BODY, p.muted));
        c.text_right(294, y, v, &ts(&BODY_STRONG, p.text));
    }
    c.text(16, 98, "Settings", &ts(&CAPTION, p.muted));
    c.text_right(306, 98, info.firmware, &ts(&CAPTION, p.muted));
    c.round_rect(Rect::new(12.0, 104.0, 296.0, 78.0), 12.0, p.card, 255);
    let settings = [
        (Setting::Theme, "Theme", theme::current().name()),
        (Setting::Passcode, "Passcode", "6 digits"),
        (Setting::Backup, "Back up seed", "24 words"),
    ];
    for (i, (which, k, v)) in settings.iter().enumerate() {
        let r = Rect::new(16.0, 107.0 + i as f32 * 24.0, 288.0, 24.0);
        if opened && *which == setting {
            c.round_rect(r, 9.0, p.fill, 255);
        }
        let y = r.y as i32 + 17;
        c.text(26, y, k, &ts(&BODY_STRONG, p.text));
        c.text_right(280, y, v, &ts(&BODY, p.muted));
        chevron(c, 292.0, r.y + 12.0, 3.5, p.muted, false);
    }
    controls(c, &nav::tab_keys(Tab::Device, opened, setting), pressed);
}

// --- signing ---------------------------------------------------------------

pub fn review(c: &mut Canvas, info: &ReviewInfo, pressed: Option<usize>) {
    let p = p();
    c.clear(p.bg);
    let risky = info.review.is_risky();
    let (heading, col) = if risky {
        ("Check carefully", p.warning)
    } else {
        ("Review", p.text)
    };
    c.text(16, 22, heading, &ts(&BODY_STRONG, col));
    if info.chain_id != 0 {
        c.text_right(
            306,
            22,
            text!(20, "Chain {}", info.chain_id).as_str(),
            &ts(&BODY, p.muted),
        );
    }

    let key = if info.rotation {
        text!(24, "#{:04} + next tree", info.index)
    } else {
        text!(24, "#{:04}", info.index)
    };
    let fee = Row::Pair("Fee", info.fee);
    let key_row = Row::Pair("Key", key.as_str());
    let hero = |c: &mut Canvas, text: &str, color: Color, width: i32| {
        let style = fit(text, width, &[&DISPLAY, &HEADLINE, &TITLE], color);
        c.text(14, 58, text, &style);
    };
    match &info.review {
        Review::Send { amount, unit, to } => {
            let amount = text!(40, "{amount} {unit}");
            hero(c, amount.as_str(), p.text, 250);
            eth_coin(c, 286.0, 46.0, 16.0, p.eth);
            c.text(16, 77, "Send", &ts(&BODY, p.muted));
            card_rows(c, 88.0, &[Row::Address("To", to), fee, key_row]);
        }
        Review::Token { amount, to, token } => {
            hero(c, amount, p.text, 290);
            let sub = text!(48, "Token {}... (raw units)", &token[..token.len().min(6)]);
            c.text(16, 77, sub.as_str(), &ts(&BODY, p.muted));
            card_rows(c, 88.0, &[Row::Address("To", to), fee, key_row]);
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
            hero(c, function, p.text, 290);
            c.text(
                16,
                77,
                text!(40, "Contract call, sends {value}").as_str(),
                &ts(&BODY, p.muted),
            );
            card_rows(c, 88.0, &[Row::Address("Contract", contract), fee, key_row]);
        }
        Review::Batch { calls } => {
            hero(c, text!(24, "Batch of {calls} calls").as_str(), p.text, 290);
            c.text(
                16,
                77,
                "The device can't show each call",
                &ts(&BODY, p.warning),
            );
            card_rows(c, 88.0, &[fee, key_row]);
        }
        Review::Unknown { selector } => {
            hero(c, "Unknown action", p.warning, 290);
            c.text(
                16,
                77,
                text!(32, "Function {selector}").as_str(),
                &ts(&BODY, p.muted),
            );
            card_rows(c, 88.0, &[fee, key_row]);
        }
        Review::Blind { description, hash } => {
            hero(c, "Blind signature", p.warning, 290);
            c.text(
                16,
                77,
                "The laptop says (not checked):",
                &ts(&BODY, p.muted),
            );
            card_rows(
                c,
                88.0,
                &[
                    Row::Pair("Says", description),
                    Row::Pair("Hash", hash),
                    key_row,
                ],
            );
        }
    }
    controls(c, &nav::review_keys(), pressed);
    countdown(c, info.seconds_left, info.timeout);
}

pub fn passcode(c: &mut Canvas, pc: &Passcode, pressed: Option<usize>) {
    let p = p();
    c.clear(p.bg);
    c.text(16, 24, pc.title, &ts(&BODY_STRONG, p.text));
    let cx = 72.0;
    for i in 0..6 {
        let x = cx + (i as f32 - 2.5) * 18.0;
        let col = match (pc.error.is_some(), i < pc.entered) {
            (true, _) => p.danger,
            (false, true) => p.text,
            (false, false) => p.fill,
        };
        c.circle(x, 100.0, 6.0, col, 255);
    }
    c.text_centered(cx as i32, 132, pc.hint, &ts(&BODY, p.muted));
    match pc.error {
        Some(e) => {
            let style = ts(&BODY_STRONG, p.danger);
            let (a, b) = split_to_fit(e, 124, |t| style.width(t));
            c.text_centered(cx as i32, 160, a, &style);
            c.text_centered(cx as i32, 177, b, &style);
        }
        None => c.text_centered(
            cx as i32,
            160,
            "Digits move each time",
            &ts(&CAPTION, p.muted),
        ),
    }
    for i in 0..12 {
        let Some(d) = digit_in_slot(pc.digits, i) else {
            continue;
        };
        let k = key_rect(i);
        let (x, y) = (k.x + k.w / 2.0, k.y + k.h / 2.0);
        if pc.pressed == Some(i) {
            c.circle(x, y, 19.0, p.card, 255);
        }
        c.text_centered(
            x as i32,
            y as i32 + 7,
            text!(2, "{d}").as_str(),
            &ts(&TITLE, p.text),
        );
    }
    controls(c, &nav::passcode_keys(), pressed);
}

pub fn progress(c: &mut Canvas, title: &str, lines: &[&str], done: u32, total: u32) {
    let p = p();
    c.clear(p.bg);
    c.text(16, 24, title, &ts(&BODY_STRONG, p.text));
    for (i, l) in lines.iter().take(3).enumerate() {
        c.text(16, 46 + 16 * i as i32, l, &ts(&BODY, p.muted));
    }
    let pct = (done as u64 * 100 / total.max(1) as u64) as u32;
    runs_right(
        c,
        306,
        80,
        &[
            (text!(4, "{pct}").as_str(), ts(&DISPLAY, p.text)),
            ("%", ts(&TITLE, p.muted)),
        ],
    );
    c.round_rect(Rect::new(12.0, 96.0, 296.0, 96.0), 12.0, p.card, 255);
    let pitch = 4.25;
    let built = (done as u64 * 1024 / total.max(1) as u64) as u32;
    key_map(
        c,
        24.0 + pitch / 2.0,
        110.0 + pitch / 2.0,
        pitch,
        64,
        built,
        1024,
        [p.text, p.text, p.dot, p.dot],
    );
    c.text(
        16,
        214,
        text!(32, "Key {done} of {total}").as_str(),
        &ts(&CAPTION, p.muted),
    );
}

pub fn result(c: &mut Canvas, ok: bool, title: &str, detail: &str) {
    let p = p();
    c.clear(p.bg);
    if ok {
        c.ring(160.0, 84.0, 32.0, 5.0, p.success, 255);
        icon_check(c, 160.0, 84.0, 30.0, p.success);
    } else {
        c.circle(160.0, 84.0, 32.0, p.fill, 255);
        icon_cross(c, 160.0, 84.0, 30.0, p.text);
    }
    c.text_centered(160, 152, title, &ts(&HEADLINE, p.text));
    c.text_centered(160, 174, detail, &ts(&BODY, p.muted));
}

pub fn seed_words(c: &mut Canvas, words: &[&str; 24], page: usize, pressed: Option<usize>) {
    let p = p();
    c.clear(p.bg);
    c.text(16, 24, "Write these down", &ts(&BODY_STRONG, p.warning));
    c.text_right(
        306,
        24,
        text!(8, "{} / 2", page + 1).as_str(),
        &ts(&BODY, p.muted),
    );
    c.text(
        16,
        42,
        "Anyone with these words controls the account.",
        &ts(&CAPTION, p.muted),
    );
    for slot in 0..12 {
        let n = page * 12 + slot;
        let (col, row) = ((slot % 3) as f32, (slot / 3) as f32);
        let r = Rect::new(12.0 + col * 101.0, 52.0 + row * 32.0, 94.0, 28.0);
        c.round_rect(r, 8.0, p.card, 255);
        c.text_right(
            r.x as i32 + 20,
            r.y as i32 + 18,
            text!(4, "{}", n + 1).as_str(),
            &ts(&CAPTION, p.muted),
        );
        c.text(
            r.x as i32 + 26,
            r.y as i32 + 18,
            words[n],
            &ts(&BODY_STRONG, p.text),
        );
    }
    controls(c, &nav::seed_keys(page), pressed);
}

pub fn seed_check(
    c: &mut Canvas,
    step: usize,
    position: usize,
    options: &[&'static str; 4],
    wrong: bool,
    pressed: Option<usize>,
) {
    let p = p();
    c.clear(p.bg);
    c.text(16, 24, "Check your backup", &ts(&BODY_STRONG, p.text));
    c.text_right(
        306,
        24,
        text!(8, "{} of 3", step + 1).as_str(),
        &ts(&BODY, p.muted),
    );
    c.text(16, 66, "Which is", &ts(&BODY, p.muted));
    runs(
        c,
        14,
        110,
        &[
            ("word ", ts(&DISPLAY, p.muted)),
            (text!(4, "#{}", position + 1).as_str(), ts(&DISPLAY, p.text)),
        ],
    );
    if wrong {
        c.text(
            16,
            140,
            "Not that one. Check your list.",
            &ts(&BODY_STRONG, p.danger),
        );
    } else {
        c.text(
            16,
            140,
            "Press the key under the word.",
            &ts(&BODY, p.muted),
        );
    }
    controls(c, &nav::check_keys(options), pressed);
}

pub fn locked(c: &mut Canvas, wait: &str, attempts_left: u32) {
    let p = p();
    c.clear(p.bg);
    icon_lock(c, 160.0, 60.0, 40.0, p.danger, p.bg);
    c.text_centered(160, 114, "Too many wrong passcodes", &ts(&TITLE, p.text));
    c.text_centered(160, 160, wait, &ts(&DISPLAY, p.text));
    let left = text!(48, "{attempts_left} more wrong codes erase the keys");
    c.text_centered(160, 188, left.as_str(), &ts(&BODY, p.muted));
}

pub fn restore_confirm(
    c: &mut Canvas,
    tree: u32,
    first_index: u32,
    seconds_left: u32,
    timeout: u32,
    pressed: Option<usize>,
) {
    let p = p();
    c.clear(p.bg);
    c.text(16, 22, "Check carefully", &ts(&BODY_STRONG, p.warning));
    c.text(14, 58, "Restore keys?", &ts(&HEADLINE, p.text));
    c.text(
        16,
        77,
        "Replaces the keys on this device",
        &ts(&BODY, p.muted),
    );
    let tree = text!(8, "{tree}");
    let first = text!(8, "#{first_index:04}");
    card_rows(
        c,
        88.0,
        &[
            Row::Pair("Key tree", tree.as_str()),
            Row::Pair("First key", first.as_str()),
            Row::Pair("Skipped", "used keys + 64"),
        ],
    );
    controls(c, &nav::review_keys(), pressed);
    countdown(c, seconds_left, timeout);
}

pub fn notice(c: &mut Canvas, title: &str, detail: &str) {
    let p = p();
    c.clear(p.bg);
    c.text_centered(160, 112, title, &ts(&HEADLINE, p.text));
    c.text_centered(160, 136, detail, &ts(&BODY, p.muted));
}
