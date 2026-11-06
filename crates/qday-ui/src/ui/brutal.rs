//! Neo-brutal: pastel tiles on cream,
//! black outlines and hard shadows, Bricolage Grotesque, and the mascot.

use crate::canvas::{Canvas, Color, Rect, TextStyle};
use crate::fonts::Font;
use crate::fonts::bricolage::{BIG, BODY, CAP, HERO, TITLE};
use crate::nav::{self, Keys, Kind, Setting};
use crate::screens::*;
use crate::text;
use crate::theme;
use crate::widgets::*;

const BG: Color = Color::hex(0xF3EDE0);
const INK: Color = Color::hex(0x15131A);
const MUTED: Color = Color::hex(0x6E6779);
const CARD: Color = Color::hex(0xFFFFFF);
const PURPLE: Color = Color::hex(0x6B4EE6);
const MINT: Color = Color::hex(0x6EE7B7);
const SKY: Color = Color::hex(0x8FD3F7);
const LIME: Color = Color::hex(0xC8F04A);
const PINK: Color = Color::hex(0xF6A9D8);
const RED: Color = Color::hex(0xFF5D6C);
const GREEN: Color = Color::hex(0x1FC46A);
const ORANGE: Color = Color::hex(0xFFB547);
const WHITE: Color = Color::hex(0xFFFFFF);

// Grid: shapes span x 10..307 (the 3 px shadow makes the visible margin 10);
// content in y 42..184; pills at y 199..227.
const LEFT: f32 = 10.0;
const RIGHT: f32 = 307.0;
const TOP: f32 = 42.0;
const BOTTOM: f32 = 184.0;
const GAP: f32 = 10.0;
const PILL_Y: f32 = 199.0;
const PILL_H: f32 = 28.0;
const SHADOW: f32 = 3.0;

fn ts(font: &'static Font, color: Color) -> TextStyle {
    TextStyle::new(font, color)
}

/// A tile: hard shadow, fill, 2 px outline. `pushed` sinks it onto its shadow.
fn tile(c: &mut Canvas, r: Rect, fill: Color, radius: f32, pushed: bool) {
    let r = if pushed {
        Rect::new(r.x + SHADOW, r.y + SHADOW, r.w, r.h)
    } else {
        r
    };
    if !pushed {
        c.round_rect(
            Rect::new(r.x + SHADOW, r.y + SHADOW, r.w, r.h),
            radius,
            INK,
            255,
        );
    }
    c.round_rect(r, radius, fill, 255);
    c.round_rect_stroke(r, radius, 2.0, INK, 255);
}

fn header(c: &mut Canvas, title: &str, chip: &str) {
    c.text(LEFT as i32 + 4, 25, title, &ts(&TITLE, INK));
    if chip.is_empty() {
        return;
    }
    let style = ts(&CAP, INK);
    let w = style.width(chip) as f32 + 18.0;
    let r = Rect::new(RIGHT - w, 8.0, w, 22.0);
    tile(c, r, CARD, 11.0, false);
    c.text_centered((r.x + w / 2.0) as i32, 23, chip, &style);
}

/// Pill rectangle for keys `first..=last`.
fn pill_rect(first: usize, last: usize) -> Rect {
    let w = (RIGHT - LEFT - 3.0 * 8.0) / 4.0;
    let x = LEFT + first as f32 * (w + 8.0);
    Rect::new(
        x,
        PILL_Y,
        (last - first + 1) as f32 * w + (last - first) as f32 * 8.0,
        PILL_H,
    )
}

fn controls(c: &mut Canvas, keys: &Keys, pressed: Option<usize>) {
    // One label size for the whole row: the big one only if every label fits.
    let fits = nav::groups(keys).into_iter().flatten().all(|g| {
        let r = pill_rect(g.first, g.last);
        let extra = if matches!(g.key.kind, Kind::Tab { opens: true, .. }) {
            9
        } else {
            0
        };
        ts(&TITLE, INK).width(&lower(g.key.label)) + extra <= r.w as i32 - 16
    });
    let font: &'static Font = if fits { &TITLE } else { &BODY };
    for g in nav::groups(keys).into_iter().flatten() {
        let held = pressed.is_some_and(|i| (g.first..=g.last).contains(&i));
        let r = pill_rect(g.first, g.last);
        let k = g.key;
        let (fill, ink) = match k.kind {
            Kind::Tab { selected: true, .. } => (PURPLE, WHITE),
            Kind::Approve => (GREEN, WHITE),
            Kind::Reject => (RED, WHITE),
            Kind::Primary => (PURPLE, WHITE),
            _ => (CARD, INK),
        };
        tile(c, r, fill, PILL_H / 2.0, held);
        let off = if held { SHADOW } else { 0.0 };
        let (cx, cy) = (r.x + r.w / 2.0 + off, r.y + r.h / 2.0 + off);
        match k.kind {
            Kind::Up | Kind::Down => arrow(c, cx, cy, 12.0, ink, k.kind == Kind::Down),
            _ => {
                let label = lower(k.label);
                let style = ts(font, ink);
                let opens = matches!(k.kind, Kind::Tab { opens: true, .. });
                let w = style.width(label.as_str()) + if opens { 9 } else { 0 };
                let x = c.text(cx as i32 - w / 2, cy as i32 + 6, label.as_str(), &style);
                if opens {
                    chevron(c, x as f32 + 5.0, cy, 3.5, ink, false);
                }
            }
        }
    }
}

fn countdown(c: &mut Canvas, seconds_left: u32) {
    let r = pill_rect(2, 2);
    c.text_centered(
        (r.x + r.w / 2.0) as i32,
        (r.y + r.h / 2.0 + 6.0) as i32,
        text!(6, "{seconds_left}s").as_str(),
        &ts(&TITLE, INK),
    );
}

#[derive(Clone, Copy, PartialEq)]
enum Mood {
    Happy,
    Curious,
    Shy,
    Worried,
}

/// The mascot: a round purple "Q" with eyes.
fn mascot(c: &mut Canvas, cx: f32, cy: f32, r: f32, mood: Mood) {
    c.line(
        cx + r * 0.55,
        cy + r * 0.55,
        cx + r * 1.05,
        cy + r * 1.0,
        r * 0.28,
        INK,
        255,
    );
    c.circle(cx + SHADOW, cy + SHADOW, r, INK, 255);
    c.line(
        cx + r * 0.55,
        cy + r * 0.55,
        cx + r * 1.0,
        cy + r * 0.95,
        r * 0.2,
        PURPLE,
        255,
    );
    c.circle(cx, cy, r, PURPLE, 255);
    c.ring(cx, cy, r, 2.0, INK, 255);
    let (ex, ey, er) = (r * 0.34, cy - r * 0.15, r * 0.24);
    let look = match mood {
        Mood::Curious => (r * 0.07, -r * 0.03),
        Mood::Worried => (0.0, r * 0.05),
        _ => (0.0, 0.0),
    };
    for side in [-1.0, 1.0] {
        let x = cx + side * ex;
        c.circle(x, ey, er, WHITE, 255);
        if mood == Mood::Shy {
            c.arc(x, ey + er * 0.4, er * 0.7, r * 0.06, 0.8, 0.4, INK, 255);
        } else {
            c.circle(x + look.0, ey + look.1, er * 0.5, INK, 255);
            c.circle(
                x + look.0 + er * 0.18,
                ey + look.1 - er * 0.2,
                er * 0.15,
                WHITE,
                255,
            );
        }
    }
    let my = cy + r * 0.38;
    match mood {
        Mood::Happy => {
            c.arc(cx, my - r * 0.12, r * 0.28, r * 0.08, 0.3, 0.4, INK, 255);
            c.circle(cx - r * 0.62, cy + r * 0.2, r * 0.12, PINK, 220);
            c.circle(cx + r * 0.62, cy + r * 0.2, r * 0.12, PINK, 220);
        }
        Mood::Curious => c.circle(cx + r * 0.05, my, r * 0.08, INK, 255),
        Mood::Shy => c.line(cx - r * 0.12, my, cx + r * 0.12, my, r * 0.07, INK, 255),
        Mood::Worried => {
            c.arc(cx, my + r * 0.2, r * 0.22, r * 0.07, 0.85, 0.3, INK, 255);
            for side in [-1.0, 1.0] {
                let x = cx + side * ex;
                c.line(
                    x - side * er * 0.9,
                    ey - er * 1.6,
                    x + side * er * 0.6,
                    ey - er * 1.25,
                    r * 0.06,
                    INK,
                    255,
                );
            }
        }
    }
}

fn tick(c: &mut Canvas, cx: f32, cy: f32, r: f32, ok: bool) {
    c.circle(cx + 2.0, cy + 2.0, r, INK, 255);
    c.circle(cx, cy, r, if ok { MINT } else { RED }, 255);
    c.ring(cx, cy, r, 1.6, INK, 255);
    if ok {
        icon_check(c, cx, cy, r, INK);
    } else {
        icon_cross(c, cx, cy, r, WHITE);
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
    c.clear(BG);
    header(c, "qday", text!(16, "tree {}", s.tree).as_str());
    let big = Rect::new(LEFT, TOP, 146.0, BOTTOM - TOP);
    tile(c, big, MINT, 18.0, false);
    c.text(24, 66, "account key", &ts(&BODY, INK));
    let (head, tail) = hex_parts(root);
    c.text(22, 102, head.as_str(), &ts(&BIG, INK));
    c.text(
        22,
        134,
        text!(12, "...{}", tail.as_str()).as_str(),
        &ts(&BIG, INK),
    );
    let status = if s.connected {
        "ready to sign"
    } else {
        "plug in usb"
    };
    let chip = Rect::new(22.0, 152.0, ts(&CAP, INK).width(status) as f32 + 20.0, 20.0);
    c.round_rect(chip, 10.0, WHITE, if s.connected { 200 } else { 120 });
    c.text(32, 166, status, &ts(&CAP, INK));

    let x = LEFT + 146.0 + GAP;
    let w = RIGHT - x;
    let h = (BOTTOM - TOP - GAP) / 2.0;
    tile(c, Rect::new(x, TOP, w, h), SKY, 18.0, false);
    c.text(
        x as i32 + 14,
        TOP as i32 + 22,
        "signatures",
        &ts(&BODY, INK),
    );
    let end = c.text(
        x as i32 + 13,
        TOP as i32 + 54,
        text!(8, "{}", s.next_index).as_str(),
        &ts(&BIG, INK),
    );
    c.text(end + 4, TOP as i32 + 54, "/ 1,024", &ts(&CAP, INK));
    let y2 = TOP + h + GAP;
    let half = (w - GAP) / 2.0;
    tile(c, Rect::new(x, y2, half, h), LIME, 18.0, false);
    c.text(x as i32 + 12, y2 as i32 + 22, "tree", &ts(&BODY, INK));
    c.text(
        x as i32 + 12,
        y2 as i32 + 54,
        text!(8, "{}", s.tree).as_str(),
        &ts(&BIG, INK),
    );
    if pending {
        c.text(x as i32 + 34, y2 as i32 + 54, "+1", &ts(&CAP, INK));
    }
    let pink = Rect::new(x + half + GAP, y2, w - half - GAP, h);
    tile(c, pink, PINK, 18.0, false);
    let mood = match latest {
        Some(a) if !a.approved => Mood::Worried,
        _ => Mood::Happy,
    };
    mascot(
        c,
        pink.x + pink.w / 2.0 - 2.0,
        y2 + h / 2.0 - 2.0,
        19.0,
        mood,
    );
    controls(c, &nav::tab_keys(Tab::Home, false, Setting::Theme), pressed);
}

pub fn keys(c: &mut Canvas, s: &Status, pending: bool, pressed: Option<usize>) {
    c.clear(BG);
    header(c, "keys", text!(16, "tree {}", s.tree).as_str());
    let hero = Rect::new(LEFT, TOP, RIGHT - LEFT, 54.0);
    tile(c, hero, SKY, 18.0, false);
    c.text(24, 62, "next key", &ts(&BODY, INK));
    let (zeros, num) = key_parts(s.next_index);
    let end = c.text(22, 88, zeros.as_str(), &ts(&BIG, MUTED));
    c.text(end, 88, num.as_str(), &ts(&BIG, INK));
    c.text_right(
        293,
        62,
        text!(24, "{} of 1,024 used", s.next_index).as_str(),
        &ts(&CAP, INK),
    );
    let note = if pending {
        "next tree ready"
    } else {
        "rotates at 900"
    };
    c.text_right(293, 86, note, &ts(&BODY, INK));

    let map = Rect::new(
        LEFT,
        TOP + 54.0 + GAP,
        RIGHT - LEFT,
        BOTTOM - (TOP + 54.0 + GAP),
    );
    tile(c, map, CARD, 18.0, false);
    let pitch = 4.25;
    let x0 = map.x + (map.w - 64.0 * pitch) / 2.0 + pitch / 2.0;
    let y0 = map.y + (map.h - 16.0 * pitch) / 2.0 + pitch / 2.0;
    key_map(
        c,
        x0,
        y0,
        pitch,
        64,
        s.next_index,
        RESERVED_FROM,
        [INK, PURPLE, RED, Color::hex(0xD9D4CC)],
    );
    controls(c, &nav::tab_keys(Tab::Keys, false, Setting::Theme), pressed);
}

pub fn activity(
    c: &mut Canvas,
    _s: &Status,
    items: &[Activity],
    offset: usize,
    opened: bool,
    pressed: Option<usize>,
) {
    c.clear(BG);
    let chip = text!(16, "{} items", items.len());
    header(c, "activity", chip.as_str());
    let list = Rect::new(LEFT, TOP, RIGHT - LEFT, BOTTOM - TOP);
    tile(c, list, CARD, 18.0, false);
    if items.is_empty() {
        mascot(c, 158.0, 92.0, 22.0, Mood::Curious);
        c.text_centered(158, 146, "nothing signed yet", &ts(&TITLE, INK));
        c.text_centered(158, 164, "approved signatures land here", &ts(&BODY, MUTED));
    }
    for (i, item) in items.iter().skip(offset).take(ACTIVITY_ROWS).enumerate() {
        let y = TOP + 6.0 + i as f32 * 34.0;
        tick(c, 32.0, y + 16.0, 10.0, item.approved);
        c.text(52, y as i32 + 14, item.title, &ts(&BODY, INK));
        c.text(52, y as i32 + 28, item.detail, &ts(&CAP, MUTED));
        if let Some(index) = item.index {
            c.text_right(
                292,
                y as i32 + 14,
                text!(8, "#{index:04}").as_str(),
                &ts(&BODY, INK),
            );
        }
        if i < ACTIVITY_ROWS - 1 && offset + i + 1 < items.len() {
            c.line(
                52.0,
                y + 34.0,
                292.0,
                y + 34.0,
                1.0,
                Color::hex(0xE5DFD3),
                255,
            );
        }
    }
    if opened && items.len() > ACTIVITY_ROWS {
        let track = Rect::new(298.0, TOP + 12.0, 3.0, BOTTOM - TOP - 24.0);
        c.round_rect(track, 1.5, Color::hex(0xE5DFD3), 255);
        let h = track.h * ACTIVITY_ROWS as f32 / items.len() as f32;
        let y = track.y + (track.h - h) * offset as f32 / (items.len() - ACTIVITY_ROWS) as f32;
        c.round_rect(Rect::new(track.x, y, track.w, h), 1.5, INK, 255);
    }
    controls(
        c,
        &nav::tab_keys(Tab::Activity, opened, Setting::Theme),
        pressed,
    );
}

pub fn device(
    c: &mut Canvas,
    _s: &Status,
    info: &DeviceInfo,
    setting: Setting,
    opened: bool,
    pressed: Option<usize>,
) {
    c.clear(BG);
    header(c, "device", info.firmware);
    let facts = Rect::new(LEFT, TOP, RIGHT - LEFT, 66.0);
    tile(c, facts, CARD, 18.0, false);
    let root = short_hex(info.root);
    let counter = text!(12, "{}", info.counter);
    for (i, (k, v)) in [
        ("account key", root.as_str()),
        ("one-way counter", counter.as_str()),
    ]
    .iter()
    .enumerate()
    {
        let y = TOP as i32 + 26 + 24 * i as i32;
        c.text(24, y, k, &ts(&BODY, MUTED));
        c.text_right(293, y, v, &ts(&BODY, INK));
    }
    let y = TOP + 66.0 + GAP;
    let w = (RIGHT - LEFT - 2.0 * GAP) / 3.0;
    let theme_name = lower(theme::current().name());
    let settings = [
        (Setting::Theme, "theme", theme_name.as_str(), LIME),
        (Setting::Passcode, "passcode", "6 digits", SKY),
        (Setting::Backup, "back up seed", "24 words", PINK),
    ];
    for (i, (which, k, v, col)) in settings.iter().enumerate() {
        let r = Rect::new(LEFT + i as f32 * (w + GAP), y, w, BOTTOM - y);
        let selected = opened && *which == setting;
        tile(c, r, if selected { PURPLE } else { *col }, 16.0, false);
        let ink = if selected { WHITE } else { INK };
        c.text(r.x as i32 + 12, r.y as i32 + 24, k, &ts(&CAP, ink));
        c.text(r.x as i32 + 12, r.y as i32 + 48, v, &ts(&BODY, ink));
    }
    controls(c, &nav::tab_keys(Tab::Device, opened, setting), pressed);
}

// --- signing ---------------------------------------------------------------

fn address_lines(c: &mut Canvas, x: i32, y: i32, addr: &str) {
    match address_groups(addr) {
        Some(g) => {
            let style = ts(&BODY, INK).tracking(4);
            c.text(
                x,
                y,
                text!(32, "0x{} {} {} {} {}", g[0], g[1], g[2], g[3], g[4]).as_str(),
                &style,
            );
            c.text(
                x + style.width("0x"),
                y + 17,
                text!(32, "{} {} {} {} {}", g[5], g[6], g[7], g[8], g[9]).as_str(),
                &style,
            );
        }
        None => {
            c.text(x, y, addr, &ts(&BODY, INK));
        }
    }
}

pub fn review(c: &mut Canvas, info: &ReviewInfo, pressed: Option<usize>) {
    c.clear(BG);
    let chip = if info.chain_id != 0 {
        text!(20, "chain {}", info.chain_id)
    } else {
        text!(20, "")
    };
    header(
        c,
        if info.review.is_risky() {
            "check carefully"
        } else {
            "sign request"
        },
        chip.as_str(),
    );
    let risky = info.review.is_risky();
    let hero = Rect::new(LEFT, TOP, RIGHT - LEFT, 70.0);
    tile(c, hero, if risky { ORANGE } else { PURPLE }, 18.0, false);
    let ink = if risky { INK } else { WHITE };
    let (label, big, unit): (&str, text::Text<40>, &str) = match &info.review {
        Review::Send { amount, unit, .. } => ("send", text!(40, "{amount}"), unit),
        Review::Token { amount, .. } => ("token transfer (raw units)", text!(40, "{amount}"), ""),
        Review::Call { function, .. } | Review::UnknownCall { function, .. } => {
            ("contract call", text!(40, "{function}"), "")
        }
        Review::Batch { calls } => ("batch", text!(40, "{calls} calls"), ""),
        Review::Unknown { .. } => ("can't decode this", text!(40, "unknown"), ""),
        Review::Blind { .. } => ("not checked by the device", text!(40, "blind sign"), ""),
    };
    let key = if info.rotation {
        text!(40, "fee {} - key #{:04} + next tree", info.fee, info.index)
    } else {
        text!(40, "fee {} - key #{:04}", info.fee, info.index)
    };
    // The amount a call sends sits on the card's top line, unless it would run into the fee
    // there. Then it moves up here.
    let sends = match &info.review {
        Review::Call { value, .. } | Review::UnknownCall { value, .. } => Some(*value),
        _ => None,
    };
    let sends_moved = sends.filter(|value| {
        let line = text!(40, "contract - sends {value}");
        24 + ts(&CAP, MUTED).width(line.as_str()) + 8 > 293 - ts(&CAP, MUTED).width(key.as_str())
    });
    match sends_moved {
        Some(value) => {
            c.text(
                24,
                62,
                text!(40, "call, sends {value}").as_str(),
                &ts(&BODY, ink),
            );
        }
        None => {
            c.text(24, 62, label, &ts(&BODY, ink));
        }
    }
    // Everything must clear the mascot, so a long amount steps down a size.
    let clear = |font: &'static Font| {
        ts(font, ink).width(big.as_str()) + 8 + ts(&TITLE, ink).width(unit) <= 216
    };
    let style = if ts(&HERO, ink).width(big.as_str()) < 200 {
        ts(&HERO, ink)
    } else if clear(&BIG) {
        ts(&BIG, ink)
    } else if clear(&TITLE) {
        ts(&TITLE, ink)
    } else {
        ts(&BODY, ink)
    };
    let end = c.text(22, 100, big.as_str(), &style);
    c.text(end + 8, 100, unit, &ts(&TITLE, ink));
    mascot(
        c,
        266.0,
        75.0,
        22.0,
        if risky { Mood::Worried } else { Mood::Curious },
    );

    let top = TOP + 70.0 + GAP;
    let card = Rect::new(LEFT, top, RIGHT - LEFT, BOTTOM - top);
    tile(c, card, CARD, 18.0, false);
    let y = top as i32;
    match &info.review {
        Review::Send { to, .. } | Review::Token { to, .. } => {
            c.text(24, y + 18, "to", &ts(&CAP, MUTED));
            address_lines(c, 24, y + 35, to);
        }
        Review::Call {
            contract, value, ..
        }
        | Review::UnknownCall {
            contract, value, ..
        } => {
            let line = if sends_moved.is_some() {
                text!(40, "contract")
            } else {
                text!(40, "contract - sends {value}")
            };
            c.text(24, y + 18, line.as_str(), &ts(&CAP, MUTED));
            address_lines(c, 24, y + 35, contract);
        }
        Review::Batch { .. } => {
            c.text(
                24,
                y + 26,
                "the device can't show each call.",
                &ts(&BODY, INK),
            );
            c.text(
                24,
                y + 43,
                "only approve if you expect this.",
                &ts(&BODY, INK),
            );
        }
        Review::Unknown { selector } => {
            c.text(
                24,
                y + 26,
                text!(32, "function {selector}").as_str(),
                &ts(&BODY, INK),
            );
            c.text(
                24,
                y + 43,
                "only approve if you expect this.",
                &ts(&BODY, INK),
            );
        }
        Review::Blind { description, hash } => {
            c.text(24, y + 18, "the laptop says", &ts(&CAP, MUTED));
            c.text(24, y + 35, description, &ts(&BODY, INK));
            c.text(24, y + 52, hash, &ts(&CAP, MUTED));
        }
    }
    c.text_right(293, y + 18, key.as_str(), &ts(&CAP, MUTED));
    controls(c, &nav::review_keys(), pressed);
    countdown(c, info.seconds_left);
}

pub fn passcode(c: &mut Canvas, pc: &Passcode, pressed: Option<usize>) {
    c.clear(BG);
    let cx = 72.0;
    mascot(
        c,
        cx,
        54.0,
        28.0,
        if pc.error.is_some() {
            Mood::Worried
        } else {
            Mood::Shy
        },
    );
    let title = lower(pc.title);
    let style = if ts(&TITLE, INK).width(&title) <= 124 {
        ts(&TITLE, INK)
    } else {
        ts(&BODY, INK)
    };
    c.text_centered(cx as i32, 114, &title, &style);
    c.text_centered(cx as i32, 132, pc.hint, &ts(&BODY, MUTED));
    for i in 0..6 {
        let x = cx + (i as f32 - 2.5) * 17.0;
        if pc.error.is_some() {
            c.circle(x, 154.0, 6.0, RED, 255);
        } else if i < pc.entered {
            c.circle(x, 154.0, 6.0, PURPLE, 255);
        } else {
            c.ring(x, 154.0, 5.5, 2.0, MUTED, 255);
        }
    }
    if let Some(e) = pc.error {
        let style = ts(&CAP, RED);
        let e = lower(e);
        let (a, b) = split_to_fit(&e, 124, |t| style.width(t));
        c.text_centered(cx as i32, 174, a, &style);
        c.text_centered(cx as i32, 188, b, &style);
    }
    for i in 0..12 {
        let Some(d) = digit_in_slot(pc.digits, i) else {
            continue;
        };
        let r = key_rect(i);
        let lit = pc.pressed == Some(i);
        tile(
            c,
            Rect::new(r.x, r.y, r.w - SHADOW, r.h - SHADOW),
            if lit { PURPLE } else { CARD },
            12.0,
            lit,
        );
        let off = if lit { SHADOW } else { 0.0 };
        let col = if lit { WHITE } else { INK };
        c.text_centered(
            (r.x + (r.w - SHADOW) / 2.0 + off) as i32,
            (r.y + r.h / 2.0 + 7.0 + off) as i32,
            text!(2, "{d}").as_str(),
            &ts(&BIG, col),
        );
    }
    controls(c, &nav::passcode_keys(), pressed);
}

pub fn progress(c: &mut Canvas, title: &str, lines: &[&str], done: u32, total: u32) {
    c.clear(BG);
    header(c, &lower(title), "");
    let hero = Rect::new(LEFT, TOP, RIGHT - LEFT, 76.0);
    tile(c, hero, LIME, 18.0, false);
    let pct = (done as u64 * 100 / total.max(1) as u64) as u32;
    let end = c.text(22, 100, text!(4, "{pct}").as_str(), &ts(&HERO, INK));
    c.text(end + 2, 100, "%", &ts(&TITLE, INK));
    c.text_right(
        293,
        64,
        text!(24, "key {done} of {total}").as_str(),
        &ts(&BODY, INK),
    );
    let bar = Rect::new(150.0, 80.0, 143.0, 18.0);
    c.round_rect(bar, 9.0, CARD, 255);
    let f = done as f32 / total.max(1) as f32;
    if f > 0.0 {
        c.round_rect(
            Rect::new(bar.x, bar.y, (bar.w * f).max(18.0), bar.h),
            9.0,
            PURPLE,
            255,
        );
    }
    c.round_rect_stroke(bar, 9.0, 2.0, INK, 255);
    let card = Rect::new(LEFT, TOP + 76.0 + GAP, RIGHT - LEFT, 66.0);
    tile(c, card, CARD, 18.0, false);
    for (i, l) in lines.iter().take(3).enumerate() {
        c.text(
            24,
            card.y as i32 + 22 + 17 * i as i32,
            &lower(l),
            &ts(&BODY, INK),
        );
    }
}

pub fn result(c: &mut Canvas, ok: bool, title: &str, detail: &str) {
    c.clear(BG);
    let panel = Rect::new(LEFT, LEFT, RIGHT - LEFT, 217.0);
    tile(c, panel, if ok { MINT } else { PINK }, 24.0, false);
    mascot(
        c,
        158.0,
        78.0,
        38.0,
        if ok { Mood::Happy } else { Mood::Worried },
    );
    let title = text!(32, "{}{}", lower(title).as_str(), if ok { "!" } else { "" });
    c.text_centered(158, 166, title.as_str(), &ts(&HERO, INK));
    c.text_centered(158, 196, &lower(detail), &ts(&BODY, INK));
}

pub fn seed_words(c: &mut Canvas, words: &[&str; 24], page: usize, pressed: Option<usize>) {
    c.clear(BG);
    header(c, "write these down", text!(8, "{} / 2", page + 1).as_str());
    let w = (RIGHT - LEFT - 2.0 * GAP) / 3.0;
    for slot in 0..12 {
        let n = page * 12 + slot;
        let (col, row) = ((slot % 3) as f32, (slot / 3) as f32);
        let r = Rect::new(LEFT + col * (w + GAP), TOP + row * 37.0, w, 31.0);
        tile(
            c,
            r,
            if n % 2 == 0 {
                CARD
            } else {
                Color::hex(0xFFF8E8)
            },
            10.0,
            false,
        );
        c.text_right(
            r.x as i32 + 22,
            r.y as i32 + 20,
            text!(4, "{}", n + 1).as_str(),
            &ts(&CAP, MUTED),
        );
        c.text(r.x as i32 + 28, r.y as i32 + 20, words[n], &ts(&BODY, INK));
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
    c.clear(BG);
    header(
        c,
        "check your backup",
        text!(8, "{} / 3", step + 1).as_str(),
    );
    let hero = Rect::new(LEFT, TOP, RIGHT - LEFT, BOTTOM - TOP);
    tile(c, hero, if wrong { PINK } else { SKY }, 18.0, false);
    c.text(24, 70, "which is word", &ts(&BODY, INK));
    c.text(
        22,
        120,
        text!(4, "#{}", position + 1).as_str(),
        &ts(&HERO, INK),
    );
    let line = if wrong {
        "not that one. check your list."
    } else {
        "press the key under it"
    };
    c.text(24, 160, line, &ts(&BODY, INK));
    mascot(
        c,
        252.0,
        100.0,
        30.0,
        if wrong { Mood::Worried } else { Mood::Curious },
    );
    controls(c, &nav::check_keys(options), pressed);
}

pub fn locked(c: &mut Canvas, wait: &str, attempts_left: u32) {
    c.clear(BG);
    let panel = Rect::new(LEFT, LEFT, RIGHT - LEFT, 217.0);
    tile(c, panel, RED, 24.0, false);
    icon_lock(c, 158.0, 56.0, 40.0, INK, RED);
    c.text_centered(158, 112, "too many wrong codes", &ts(&TITLE, INK));
    c.text_centered(158, 160, wait, &ts(&HERO, INK));
    c.text_centered(
        158,
        194,
        text!(48, "{attempts_left} more wrong codes erase the keys").as_str(),
        &ts(&BODY, INK),
    );
}

pub fn restore_confirm(
    c: &mut Canvas,
    tree: u32,
    first_index: u32,
    seconds_left: u32,
    _timeout: u32,
    pressed: Option<usize>,
) {
    c.clear(BG);
    header(c, "check carefully", "");
    let hero = Rect::new(LEFT, TOP, RIGHT - LEFT, 70.0);
    tile(c, hero, ORANGE, 18.0, false);
    c.text(24, 62, "replaces the keys on this device", &ts(&BODY, INK));
    c.text(22, 100, "restore keys?", &ts(&BIG, INK));
    let top = TOP + 70.0 + GAP;
    tile(
        c,
        Rect::new(LEFT, top, RIGHT - LEFT, BOTTOM - top),
        CARD,
        18.0,
        false,
    );
    let y = top as i32;
    c.text(24, y + 24, "key tree", &ts(&BODY, MUTED));
    c.text_right(293, y + 24, text!(8, "{tree}").as_str(), &ts(&BODY, INK));
    c.text(24, y + 46, "first key", &ts(&BODY, MUTED));
    c.text_right(
        293,
        y + 46,
        text!(8, "#{first_index:04}").as_str(),
        &ts(&BODY, INK),
    );
    controls(c, &nav::review_keys(), pressed);
    countdown(c, seconds_left);
}

pub fn notice(c: &mut Canvas, title: &str, detail: &str) {
    c.clear(BG);
    let r = Rect::new(30.0, 70.0, 257.0, 90.0);
    tile(c, r, CARD, 18.0, false);
    c.text_centered(158, 108, &lower(title), &ts(&TITLE, INK));
    c.text_centered(158, 134, detail, &ts(&BODY, MUTED));
}
