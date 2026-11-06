//! Pieces every style family shares: what the status shows, the tabs, the
//! passcode keypad geometry (one for all themes, so touch hit-testing is
//! shared), simple icons, and drawing the bitmap terminal fonts.

use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};

use qday_wallet::settings::Scheme;

use crate::canvas::{Canvas, Color, HEIGHT, Rect, WIDTH};

/// What the tabs show about the device.
#[derive(Clone, Copy)]
pub struct Status {
    pub tree: u32,
    pub next_index: u32,
    /// A request has arrived over USB since boot.
    pub connected: bool,
    pub scheme: Scheme,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Home,
    Keys,
    Activity,
    Device,
}

// --- layout ------------------------------

/// Bottom controls: pills sit at y 194..228 (Neo-brutal: 199..227 plus shadow).
pub const CONTROLS_Y: f32 = 194.0;
pub const CONTROLS_H: f32 = 34.0;

/// Left and right edge of soft key `i`'s column, with a 12 px screen margin
/// and 8 px between neighbouring controls.
pub fn key_column(first: usize, last: usize) -> (f32, f32) {
    let left = if first == 0 {
        12.0
    } else {
        first as f32 * 80.0 + 4.0
    };
    let right = if last == 3 {
        308.0
    } else {
        (last + 1) as f32 * 80.0 - 4.0
    };
    (left, right)
}

// --- passcode keypad ----------------------------------------------------------

pub const KEY_W: f32 = 50.0;
pub const KEY_H: f32 = 40.0;
const KEY_GAP_X: f32 = 6.0;
const KEY_GAP_Y: f32 = 4.0;
const PAD_X: f32 = 146.0;
const PAD_Y: f32 = 8.0;

/// Keypad slot `i` (3 columns by 4 rows, left to right, top to bottom).
pub fn key_rect(i: usize) -> Rect {
    let (col, row) = ((i % 3) as f32, (i / 3) as f32);
    Rect::new(
        PAD_X + col * (KEY_W + KEY_GAP_X),
        PAD_Y + row * (KEY_H + KEY_GAP_Y),
        KEY_W,
        KEY_H,
    )
}

/// The digit in slot `i`: `digits[j]` fills the j-th digit slot (the bottom
/// row has its digit in the middle); the bottom corners are empty.
pub fn digit_in_slot(digits: &[u8; 10], i: usize) -> Option<u8> {
    match i {
        0..=8 => Some(digits[i]),
        10 => Some(digits[9]),
        _ => None,
    }
}

/// Which digit a touch at (x, y) lands on; the gaps between keys count as
/// nothing, so a press near an edge can't hit the neighbour by accident.
pub fn digit_at(digits: &[u8; 10], x: i32, y: i32) -> Option<u8> {
    (0..12).find_map(|i| {
        let r = key_rect(i);
        let inside = (x as f32) >= r.x
            && (x as f32) < r.x + r.w
            && (y as f32) >= r.y
            && (y as f32) < r.y + r.h;
        if inside {
            digit_in_slot(digits, i)
        } else {
            None
        }
    })
}

// --- icons ---------------------------------------------------------------------

pub fn icon_check(c: &mut Canvas, cx: f32, cy: f32, s: f32, color: Color) {
    let w = s * 0.14;
    c.line(
        cx - 0.45 * s,
        cy + 0.02 * s,
        cx - 0.12 * s,
        cy + 0.33 * s,
        w,
        color,
        255,
    );
    c.line(
        cx - 0.12 * s,
        cy + 0.33 * s,
        cx + 0.48 * s,
        cy - 0.3 * s,
        w,
        color,
        255,
    );
}

pub fn icon_cross(c: &mut Canvas, cx: f32, cy: f32, s: f32, color: Color) {
    let d = 0.32 * s;
    c.line(cx - d, cy - d, cx + d, cy + d, s * 0.13, color, 255);
    c.line(cx - d, cy + d, cx + d, cy - d, s * 0.13, color, 255);
}

/// A padlock; `hole` is the colour of the keyhole (the background).
pub fn icon_lock(c: &mut Canvas, cx: f32, cy: f32, s: f32, color: Color, hole: Color) {
    let w = 0.09 * s;
    c.arc(cx, cy - 0.12 * s, 0.26 * s, w, 0.75, 0.5, color, 255);
    c.line(
        cx - 0.26 * s,
        cy - 0.12 * s,
        cx - 0.26 * s,
        cy + 0.05 * s,
        w,
        color,
        255,
    );
    c.line(
        cx + 0.26 * s,
        cy - 0.12 * s,
        cx + 0.26 * s,
        cy + 0.05 * s,
        w,
        color,
        255,
    );
    c.round_rect(
        Rect::new(cx - 0.4 * s, cy + 0.02 * s, 0.8 * s, 0.56 * s),
        0.12 * s,
        color,
        255,
    );
    c.circle(cx, cy + 0.27 * s, 0.07 * s, hole, 255);
}

/// `>`-style chevron pointing right (or left when `left`).
pub fn chevron(c: &mut Canvas, x: f32, y: f32, s: f32, color: Color, left: bool) {
    let d = if left { -0.5 * s } else { 0.5 * s };
    c.line(x - d, y - s, x + d, y, 0.4 * s, color, 255);
    c.line(x + d, y, x - d, y + s, 0.4 * s, color, 255);
}

/// An arrow pointing up (or down when `down`).
pub fn arrow(c: &mut Canvas, cx: f32, cy: f32, s: f32, color: Color, down: bool) {
    let k = if down { -1.0 } else { 1.0 };
    let w = 0.18 * s;
    c.line(cx, cy + k * 0.5 * s, cx, cy - k * 0.5 * s, w, color, 255);
    c.line(
        cx - 0.4 * s,
        cy - k * 0.1 * s,
        cx,
        cy - k * 0.5 * s,
        w,
        color,
        255,
    );
    c.line(
        cx + 0.4 * s,
        cy - k * 0.1 * s,
        cx,
        cy - k * 0.5 * s,
        w,
        color,
        255,
    );
}

/// The Ether coin: a circle with the diamond.
pub fn eth_coin(c: &mut Canvas, cx: f32, cy: f32, r: f32, coin: Color) {
    let white = Color::hex(0xFFFFFF);
    c.circle(cx, cy, r, coin, 255);
    let (w, h) = (r * 0.38, r * 0.62);
    let pts = [(cx, cy - h), (cx + w, cy), (cx, cy + h), (cx - w, cy)];
    for i in 0..4 {
        let (a, b) = (pts[i], pts[(i + 1) % 4]);
        c.line(a.0, a.1, b.0, b.1, r * 0.11, white, 255);
    }
    c.line(cx - w, cy, cx + w, cy, r * 0.09, white, 200);
}

pub fn key_glyph(c: &mut Canvas, cx: f32, cy: f32, color: Color) {
    c.ring(cx - 2.5, cy, 2.6, 1.4, color, 255);
    c.line(cx, cy, cx + 5.0, cy, 1.4, color, 255);
    c.line(cx + 3.6, cy, cx + 3.6, cy + 2.4, 1.4, color, 255);
}

pub fn tree_glyph(c: &mut Canvas, cx: f32, cy: f32, color: Color) {
    c.line(cx, cy - 3.0, cx - 3.5, cy + 3.0, 1.2, color, 255);
    c.line(cx, cy - 3.0, cx + 3.5, cy + 3.0, 1.2, color, 255);
    for (x, y) in [(cx, cy - 3.0), (cx - 3.5, cy + 3.0), (cx + 3.5, cy + 3.0)] {
        c.circle(x, y, 1.8, color, 255);
    }
}

/// Every one-time key as a dot, `cols` per row: used, next, reserved for
/// rotation, and unused.
#[allow(clippy::too_many_arguments)]
pub fn key_map(
    c: &mut Canvas,
    x0: f32,
    y0: f32,
    pitch: f32,
    cols: u32,
    next: u32,
    reserved_from: u32,
    [used, current, reserved, unused]: [Color; 4],
) {
    for i in 0..1024u32 {
        let (x, y) = (
            x0 + (i % cols) as f32 * pitch,
            y0 + (i / cols) as f32 * pitch,
        );
        if i < next {
            c.circle(x, y, pitch * 0.38, used, 255);
        } else if i == next {
            c.circle(x, y, pitch * 0.5, current, 255);
        } else if i >= reserved_from {
            c.circle(x, y, pitch * 0.33, reserved, 255);
        } else {
            c.circle(x, y, pitch * 0.33, unused, 255);
        }
    }
}

/// Splits `text` into at most two lines no wider than `max` (by `width`),
/// breaking at the last space that fits; the second line may still overflow.
pub fn split_to_fit(text: &str, max: i32, width: impl Fn(&str) -> i32) -> (&str, &str) {
    if width(text) <= max {
        return (text, "");
    }
    let mut cut = None;
    for (i, ch) in text.char_indices() {
        if ch == ' ' && width(&text[..i]) <= max {
            cut = Some(i);
        }
    }
    match cut {
        Some(i) => (&text[..i], &text[i + 1..]),
        None => (text, ""),
    }
}

/// ASCII lowercase copy (labels are ASCII).
pub fn lower(s: &str) -> crate::text::Text<48> {
    let mut t = crate::text::Text::<48>::new();
    for ch in s.chars() {
        let _ = core::fmt::Write::write_char(&mut t, ch.to_ascii_lowercase());
    }
    t
}

/// ASCII uppercase copy.
pub fn upper(s: &str) -> crate::text::Text<48> {
    let mut t = crate::text::Text::<48>::new();
    for ch in s.chars() {
        let _ = core::fmt::Write::write_char(&mut t, ch.to_ascii_uppercase());
    }
    t
}

// --- bitmap fonts (terminal family) ---------------------------------------

/// embedded-graphics drawing onto the canvas.
struct Eg<'a, 'b> {
    c: &'a mut Canvas<'b>,
    alpha: u8,
}

impl OriginDimensions for Eg<'_, '_> {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for Eg<'_, '_> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb888>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(pt, col) in pixels {
            self.c.blend(
                pt.x,
                pt.y,
                Color {
                    r: col.r(),
                    g: col.g(),
                    b: col.b(),
                },
                self.alpha,
            );
        }
        Ok(())
    }
}

/// Bitmap text with its cell's top-left at (x, y); `bloom` adds a faint
/// phosphor glow. Returns the x just past it.
///
/// Each glyph is captured once and reused for the four bloom passes and the
/// letter itself, drawn in the same order as `mono_exact` (every bloom pass
/// over the whole string, then the letters), so the pixels are identical.
pub fn mono(
    c: &mut Canvas,
    x: i32,
    y: i32,
    s: &str,
    font: &MonoFont<'_>,
    color: Color,
    bloom: bool,
) -> i32 {
    const MAX_CHARS: usize = 48;
    let (cw, ch) = (
        font.character_size.width as i32,
        font.character_size.height as i32,
    );
    let advance = cw + font.character_spacing as i32;
    let mut glyphs = [GlyphMask::default(); MAX_CHARS];
    let mut n = 0;
    for (i, chr) in s.chars().take(MAX_CHARS).enumerate() {
        let mut buf = [0u8; 4];
        let style = MonoTextStyle::new(font, Rgb888::WHITE);
        Text::with_baseline(
            chr.encode_utf8(&mut buf),
            Point::zero(),
            style,
            Baseline::Top,
        )
        .draw(&mut glyphs[i])
        .ok();
        n = i + 1;
    }
    let lit = |c: &mut Canvas, dx: i32, dy: i32, alpha: u8| {
        for (i, g) in glyphs[..n].iter().enumerate() {
            let gx = x + i as i32 * advance + dx;
            for (row, bits) in g.rows.iter().take(ch as usize).enumerate() {
                for col in 0..cw {
                    if bits & (1 << col) != 0 {
                        c.blend(gx + col, y + dy + row as i32, color, alpha);
                    }
                }
            }
        }
    };
    if bloom {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            lit(c, dx, dy, 34);
        }
    }
    lit(c, 0, 0, 255);
    x + s.len() as i32 * font.character_size.width as i32
}

/// One glyph's lit pixels: bit `col` of `rows[row]`. Up to 16 x 24, enough
/// for the terminal fonts (10 x 20 at most); 2.3 KB for a full line.
#[derive(Clone, Copy, Default)]
struct GlyphMask {
    rows: [u16; 24],
}

impl OriginDimensions for GlyphMask {
    fn size(&self) -> Size {
        Size::new(16, 24)
    }
}

impl DrawTarget for GlyphMask {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb888>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(pt, _) in pixels {
            if (0..16).contains(&pt.x) && (0..24).contains(&pt.y) {
                self.rows[pt.y as usize] |= 1 << pt.x;
            }
        }
        Ok(())
    }
}

/// `mono` drawn through embedded-graphics: the reference it must match.
#[doc(hidden)]
pub fn mono_exact(
    c: &mut Canvas,
    x: i32,
    y: i32,
    s: &str,
    font: &MonoFont<'_>,
    color: Color,
    bloom: bool,
) -> i32 {
    let style = MonoTextStyle::new(font, Rgb888::new(color.r, color.g, color.b));
    if bloom {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let mut eg = Eg { c, alpha: 34 };
            Text::with_baseline(s, Point::new(x + dx, y + dy), style, Baseline::Top)
                .draw(&mut eg)
                .ok();
        }
    }
    let mut eg = Eg { c, alpha: 255 };
    Text::with_baseline(s, Point::new(x, y), style, Baseline::Top)
        .draw(&mut eg)
        .ok();
    x + s.len() as i32 * font.character_size.width as i32
}
