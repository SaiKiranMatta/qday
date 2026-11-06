//! Drawing into a 320×240 framebuffer with alpha blending and anti-aliased
//! shapes (signed-distance coverage). Pixels are stored as RGB565 in the
//! display's byte order, so the frame (150 KB) fits in internal RAM and goes
//! to the display as-is; blending works at 8 bits per channel per operation.

use crate::fonts::Font;

pub const WIDTH: i32 = 320;
pub const HEIGHT: i32 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn hex(rgb: u32) -> Color {
        Color {
            r: (rgb >> 16) as u8,
            g: (rgb >> 8) as u8,
            b: rgb as u8,
        }
    }

    /// Nearest RGB565 color (rounded, not truncated).
    pub const fn to_565(self) -> u16 {
        let r = (self.r as u16 * 31 + 127) / 255;
        let g = (self.g as u16 * 63 + 127) / 255;
        let b = (self.b as u16 * 31 + 127) / 255;
        (r << 11) | (g << 5) | b
    }

    pub const fn from_565(v: u16) -> Color {
        let (r, g, b) = ((v >> 11) & 0x1F, (v >> 5) & 0x3F, v & 0x1F);
        Color {
            r: (r << 3 | r >> 2) as u8,
            g: (g << 2 | g >> 4) as u8,
            b: (b << 3 | b >> 2) as u8,
        }
    }

    /// `self` over `under` with coverage `alpha` (0 = under, 255 = self).
    pub fn over(self, under: Color, alpha: u8) -> Color {
        let a = alpha as u16;
        let mix =
            |top: u8, bottom: u8| ((top as u16 * a + bottom as u16 * (255 - a) + 127) / 255) as u8;
        Color {
            r: mix(self.r, under.r),
            g: mix(self.g, under.g),
            b: mix(self.b, under.b),
        }
    }

    /// Straight interpolation from `self` to `other` (`t` 0..=1).
    pub fn lerp(self, other: Color, t: f32) -> Color {
        other.over(self, (t.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
}

/// How to draw text: font, color, opacity and extra letter spacing.
#[derive(Clone, Copy)]
pub struct TextStyle {
    pub font: &'static Font,
    pub color: Color,
    pub opacity: u8,
    /// Extra space between letters, in 1/16 px.
    pub tracking: i32,
    /// Edge contrast ×10: coverage is raised to `gamma / 10`. Below 10
    /// thickens anti-aliased edges, above 10 thins them; 10 is neutral.
    pub gamma: u8,
}

impl TextStyle {
    pub const fn new(font: &'static Font, color: Color) -> Self {
        TextStyle {
            font,
            color,
            opacity: 255,
            tracking: 0,
            gamma: 10,
        }
    }

    pub const fn tracking(self, tracking: i32) -> Self {
        TextStyle { tracking, ..self }
    }

    pub const fn gamma(self, gamma: u8) -> Self {
        TextStyle { gamma, ..self }
    }

    pub const fn opacity(self, opacity: u8) -> Self {
        TextStyle { opacity, ..self }
    }

    pub fn width(&self, text: &str) -> i32 {
        self.font.width(text, self.tracking)
    }
}

/// A full-screen framebuffer of 0x00RRGGBB pixels, row-major.
/// The frame, one RGB565 pixel per `u16`, stored high byte first in memory
/// (what the display takes over SPI), so a frame goes out with no
/// conversion. Half the size of a 24-bit frame: it fits in internal RAM.
pub struct Canvas<'a> {
    px: &'a mut [u16],
}

/// A color as stored in the frame.
#[inline]
fn store(c: Color) -> u16 {
    c.to_565().swap_bytes()
}

/// A stored pixel back as a color.
#[inline]
fn load(v: u16) -> Color {
    Color::from_565(v.swap_bytes())
}

/// 4×4 Bayer matrix, values 0..16.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

impl Color {
    /// RGB565 with an ordered-dither offset for pixel (x, y), so the average
    /// over an area keeps the full 8-bit color.
    pub fn to_565_dithered(self, x: i32, y: i32) -> u16 {
        let t = BAYER[(y & 3) as usize][(x & 3) as usize] as u16;
        let r = ((self.r as u16 + t / 2) >> 3).min(31);
        let g = ((self.g as u16 + t / 4) >> 2).min(63);
        let b = ((self.b as u16 + t / 2) >> 3).min(31);
        (r << 11) | (g << 5) | b
    }
}

impl<'a> Canvas<'a> {
    pub fn new(px: &'a mut [u16]) -> Self {
        assert_eq!(px.len(), (WIDTH * HEIGHT) as usize);
        Canvas { px }
    }

    pub fn get(&self, x: i32, y: i32) -> Color {
        load(self.px[(y * WIDTH + x) as usize])
    }

    /// Rows `y0..y1` as RGB565 values into `out` (`(y1 - y0) * WIDTH` pixels).
    pub fn rows_565(&self, y0: i32, y1: i32, out: &mut [u16]) {
        let from = &self.px[(y0 * WIDTH) as usize..(y1 * WIDTH) as usize];
        for (o, &p) in out.iter_mut().zip(from) {
            *o = p.swap_bytes();
        }
    }

    /// The frame exactly as the display takes it over SPI: RGB565, high
    /// byte first (`2 * WIDTH * HEIGHT` bytes).
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: u16 has no padding or invalid bit patterns, and the byte
        // slice covers exactly the same memory with a weaker alignment.
        unsafe { core::slice::from_raw_parts(self.px.as_ptr().cast::<u8>(), self.px.len() * 2) }
    }

    /// Blends `color` at one pixel with coverage `alpha`; off-screen is ignored.
    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
        if alpha == 0 || x < 0 || y < 0 || x >= WIDTH || y >= HEIGHT {
            return;
        }
        let i = (y * WIDTH + x) as usize;
        self.px[i] = if alpha == 255 {
            store(color)
        } else {
            store(color.over(load(self.px[i]), alpha))
        };
    }

    pub fn clear(&mut self, color: Color) {
        self.px.fill(store(color));
    }

    /// Vertical gradient filling `rect` from `top` to `bottom`.
    pub fn gradient(&mut self, rect: Rect, top: Color, bottom: Color) {
        let (x0, x1, y0, y1) = clip(rect, 0.0);
        let span = (rect.h - 1.0).max(1.0);
        for y in y0..y1 {
            let c = store(top.lerp(bottom, (y as f32 - rect.y) / span));
            let row = (y * WIDTH) as usize;
            self.px[row + x0 as usize..row + x1 as usize].fill(c);
        }
    }

    /// Anti-aliased rounded rectangle, `opacity` 0..=255. The interior is
    /// filled directly; only the edge pixels get the distance maths.
    pub fn round_rect(&mut self, rect: Rect, radius: f32, color: Color, opacity: u8) {
        let sd = |px, py| sd_round_rect(px, py, rect, radius);
        // Fully covered pixels are at distance <= -0.5 from the edge: inside the
        // shape shrunk by that much (and by CORE_MARGIN, clear of rounding).
        let core = round_rect_core(rect, radius, CORE_MARGIN);
        self.shade_core(rect, 1.0, color, opacity, sd, core, true);
    }

    /// `round_rect` without the interior shortcut: the reference it must match.
    #[doc(hidden)]
    pub fn round_rect_exact(&mut self, rect: Rect, radius: f32, color: Color, opacity: u8) {
        self.shade(rect, 1.0, color, opacity, |px, py| {
            sd_round_rect(px, py, rect, radius)
        });
    }

    /// Anti-aliased outline of a rounded rectangle, `width` px wide.
    pub fn round_rect_stroke(
        &mut self,
        rect: Rect,
        radius: f32,
        width: f32,
        color: Color,
        opacity: u8,
    ) {
        let half = width / 2.0;
        let sd = |px, py| libm::fabsf(sd_round_rect(px, py, rect, radius) + half) - half;
        // Deeper inside than the outline's inner edge (by 0.5 + CORE_MARGIN),
        // nothing is drawn: skip those pixels without the distance maths.
        let hollow = round_rect_core(rect, radius, width + 0.5 + CORE_MARGIN);
        self.shade_core(rect, half + 1.0, color, opacity, sd, hollow, false);
    }

    /// `round_rect_stroke` without the hollow shortcut: the reference it must match.
    #[doc(hidden)]
    pub fn round_rect_stroke_exact(
        &mut self,
        rect: Rect,
        radius: f32,
        width: f32,
        color: Color,
        opacity: u8,
    ) {
        let half = width / 2.0;
        self.shade(rect, half + 1.0, color, opacity, |px, py| {
            libm::fabsf(sd_round_rect(px, py, rect, radius) + half) - half
        });
    }

    /// Anti-aliased filled circle (interior filled directly, like `round_rect`).
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, opacity: u8) {
        let bounds = Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r);
        let inner = r - CORE_MARGIN;
        let core = |py: f32| {
            let dy = libm::fabsf(py - cy);
            if inner <= 0.0 || dy > inner {
                return None;
            }
            let half = libm::sqrtf(inner * inner - dy * dy);
            Some((cx - half, cx + half))
        };
        self.shade_core(
            bounds,
            1.0,
            color,
            opacity,
            |px, py| hypot(px - cx, py - cy) - r,
            core,
            true,
        );
    }

    /// `circle` without the interior shortcut: the reference it must match.
    #[doc(hidden)]
    pub fn circle_exact(&mut self, cx: f32, cy: f32, r: f32, color: Color, opacity: u8) {
        let bounds = Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r);
        self.shade(bounds, 1.0, color, opacity, |px, py| {
            hypot(px - cx, py - cy) - r
        });
    }

    /// Blends black at `alpha` over every `step`-th row from `start` (CRT
    /// scanlines); the same result as `blend` per pixel, without the checks.
    pub fn darken_rows(&mut self, start: i32, step: usize, alpha: u8) {
        // Each RGB565 channel maps on its own, so three small tables give the
        // same result as blending black per pixel.
        let black = Color::hex(0);
        let (mut r5, mut g6, mut b5) = ([0u16; 32], [0u16; 64], [0u16; 32]);
        // Expanded exactly as `load` does (Color::from_565).
        let dark = |v: u16| black.over(Color::from_565(v), alpha).to_565();
        for v in 0..64u16 {
            if v < 32 {
                r5[v as usize] = dark(v << 11) & 0xF800;
                b5[v as usize] = dark(v) & 0x001F;
            }
            g6[v as usize] = dark(v << 5) & 0x07E0;
        }
        for y in (start.max(0)..HEIGHT).step_by(step.max(1)) {
            let row = (y * WIDTH) as usize;
            for p in &mut self.px[row..row + WIDTH as usize] {
                let v = p.swap_bytes();
                let out =
                    r5[(v >> 11) as usize] | g6[(v >> 5 & 0x3F) as usize] | b5[(v & 0x1F) as usize];
                *p = out.swap_bytes();
            }
        }
    }

    /// Anti-aliased ring (circle outline) of the given width; the hole in the
    /// middle is skipped without the distance maths.
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, color: Color, opacity: u8) {
        let bounds = Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r);
        let half = width / 2.0;
        let hole = r - half - 0.5 - CORE_MARGIN;
        let core = |py: f32| {
            let dy = libm::fabsf(py - cy);
            if hole <= 0.0 || dy > hole {
                return None;
            }
            let w = libm::sqrtf(hole * hole - dy * dy);
            Some((cx - w, cx + w))
        };
        let sd = |px, py| libm::fabsf(hypot(px - cx, py - cy) - r) - half;
        self.shade_core(bounds, half + 1.0, color, opacity, sd, core, false);
    }

    /// `ring` without the hole shortcut: the reference it must match.
    #[doc(hidden)]
    pub fn ring_exact(&mut self, cx: f32, cy: f32, r: f32, width: f32, color: Color, opacity: u8) {
        let bounds = Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r);
        let half = width / 2.0;
        self.shade(bounds, half + 1.0, color, opacity, |px, py| {
            libm::fabsf(hypot(px - cx, py - cy) - r) - half
        });
    }

    /// Anti-aliased line with round caps.
    #[allow(clippy::too_many_arguments)]
    pub fn line(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
        color: Color,
        opacity: u8,
    ) {
        let half = width / 2.0;
        let bounds = Rect::new(
            x0.min(x1) - half,
            y0.min(y1) - half,
            libm::fabsf(x1 - x0) + width,
            libm::fabsf(y1 - y0) + width,
        );
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        self.shade(bounds, 1.0, color, opacity, |px, py| {
            let t = (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0);
            hypot(px - (x0 + t * dx), py - (y0 + t * dy)) - half
        });
    }

    /// Anti-aliased arc with round caps: `start` and `sweep` are fractions of a
    /// turn, clockwise from 12 o'clock.
    #[allow(clippy::too_many_arguments)]
    pub fn arc(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        width: f32,
        start: f32,
        sweep: f32,
        color: Color,
        opacity: u8,
    ) {
        if sweep <= 0.0 {
            return;
        }
        let sweep = sweep.min(1.0);
        let half = width / 2.0;
        let tau = core::f32::consts::TAU;
        let point = |turn: f32| {
            let a = turn * tau;
            (cx + r * libm::sinf(a), cy - r * libm::cosf(a))
        };
        let (sx, sy) = point(start);
        let (ex, ey) = point(start + sweep);
        let bounds = Rect::new(
            cx - r - half,
            cy - r - half,
            2.0 * (r + half),
            2.0 * (r + half),
        );
        self.shade(bounds, 1.0, color, opacity, |px, py| {
            let (dx, dy) = (px - cx, py - cy);
            // Clockwise turn from 12 o'clock, in 0..1.
            let mut turn = libm::atan2f(dx, -dy) / tau;
            if turn < 0.0 {
                turn += 1.0;
            }
            let mut rel = turn - start;
            rel -= libm::floorf(rel);
            if rel <= sweep {
                libm::fabsf(hypot(dx, dy) - r) - half
            } else {
                // Outside the sweep: only the round caps reach here.
                (hypot(px - sx, py - sy).min(hypot(px - ex, py - ey))) - half
            }
        });
    }

    /// A faint light catching the top edge of a rounded card.
    pub fn gradient_clip_top_highlight(&mut self, r: Rect, radius: f32) {
        let top = Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, (r.h / 2.0).min(22.0));
        let (x0, x1, y0, y1) = clip(top, 0.0);
        for y in y0..y1 {
            let fade = 1.0 - (y as f32 - top.y) / top.h;
            let alpha = (fade * fade * 18.0) as u8;
            for x in x0..x1 {
                let d = sd_round_rect(x as f32 + 0.5, y as f32 + 0.5, r, radius);
                if d < -1.0 {
                    self.blend(x, y, Color::hex(0xFFFFFF), alpha);
                }
            }
        }
    }

    /// Soft radial glow: `opacity` at the center fading smoothly to 0 at `radius`.
    pub fn glow(&mut self, cx: f32, cy: f32, radius: f32, color: Color, opacity: u8) {
        let bounds = Rect::new(cx - radius, cy - radius, 2.0 * radius, 2.0 * radius);
        let (x0, x1, y0, y1) = clip(bounds, 0.0);
        for y in y0..y1 {
            for x in x0..x1 {
                let d = hypot(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy) / radius;
                if d < 1.0 {
                    // Smooth falloff: (1 - d²)², no visible edge.
                    let f = 1.0 - d * d;
                    self.blend(x, y, color, (f * f * opacity as f32) as u8);
                }
            }
        }
    }

    /// Draws `text` with its baseline at `y`, starting at `x`; returns the x
    /// just past it.
    pub fn text(&mut self, x: i32, y: i32, text: &str, style: &TextStyle) -> i32 {
        // Coverage (0..15) to alpha, through the edge-contrast curve.
        let mut alpha = [0u8; 16];
        for (a4, out) in alpha.iter_mut().enumerate() {
            let coverage = libm::powf(a4 as f32 / 15.0, style.gamma as f32 / 10.0);
            *out = (coverage * style.opacity as f32 + 0.5) as u8;
        }
        let font = style.font;
        let mut pen = x * 16;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                pen += font.kern(p, c) + style.tracking;
            }
            prev = Some(c);
            let Some(g) = font.glyph(c) else { continue };
            let gx = (pen + 8) / 16 + g.left as i32;
            let gy = y - g.top as i32;
            let row_bytes = (g.w as usize).div_ceil(2);
            for row in 0..g.h as usize {
                for col in 0..g.w as usize {
                    let byte = font.pixels[g.offset as usize + row * row_bytes + col / 2];
                    let a4 = if col % 2 == 0 { byte >> 4 } else { byte & 0x0F };
                    if a4 != 0 {
                        self.blend(
                            gx + col as i32,
                            gy + row as i32,
                            style.color,
                            alpha[a4 as usize],
                        );
                    }
                }
            }
            pen += g.advance as i32;
        }
        (pen + 8) / 16
    }

    /// Like `text`, centered on `cx`.
    pub fn text_centered(&mut self, cx: i32, y: i32, text: &str, style: &TextStyle) {
        let x = cx - style.font.width(text, style.tracking) / 2;
        self.text(x, y, text, style);
    }

    /// Like `text`, ending at `right`.
    pub fn text_right(&mut self, right: i32, y: i32, text: &str, style: &TextStyle) {
        let x = right - style.font.width(text, style.tracking);
        self.text(x, y, text, style);
    }

    /// Fills pixels by coverage from a signed distance (negative inside),
    /// visiting `bounds` grown by `margin`.
    /// `shade`, but pixels whose centre lies inside `core(row centre)` skip
    /// `sd`: filled when `fill` (an x range certainly fully covered), left
    /// alone otherwise (an x range certainly not covered).
    #[allow(clippy::too_many_arguments)]
    fn shade_core(
        &mut self,
        bounds: Rect,
        margin: f32,
        color: Color,
        opacity: u8,
        sd: impl Fn(f32, f32) -> f32,
        core: impl Fn(f32) -> Option<(f32, f32)>,
        fill: bool,
    ) {
        let (x0, x1, y0, y1) = clip(bounds, margin);
        let packed = store(color);
        for y in y0..y1 {
            let py = y as f32 + 0.5;
            // Pixel x is in the core when x + 0.5 lies in [lo, hi].
            let (c0, c1) = match core(py) {
                Some((lo, hi)) => (
                    (libm::ceilf(lo - 0.5) as i32).clamp(x0, x1),
                    (libm::floorf(hi - 0.5) as i32 + 1).clamp(x0, x1),
                ),
                None => (x1, x1),
            };
            let (c0, c1) = if c0 < c1 { (c0, c1) } else { (x1, x1) };
            for x in (x0..c0).chain(c1..x1) {
                let d = sd(x as f32 + 0.5, py);
                if d >= 0.5 {
                    continue;
                }
                let coverage = if d <= -0.5 { 1.0 } else { 0.5 - d };
                self.blend(x, y, color, (coverage * opacity as f32 + 0.5) as u8);
            }
            if fill && c0 < c1 {
                let row = &mut self.px[(y * WIDTH + c0) as usize..(y * WIDTH + c1) as usize];
                if opacity == 255 {
                    row.fill(packed);
                } else if opacity > 0 {
                    for p in row {
                        *p = store(color.over(load(*p), opacity));
                    }
                }
            }
        }
    }

    fn shade(
        &mut self,
        bounds: Rect,
        margin: f32,
        color: Color,
        opacity: u8,
        sd: impl Fn(f32, f32) -> f32,
    ) {
        let (x0, x1, y0, y1) = clip(bounds, margin);
        for y in y0..y1 {
            for x in x0..x1 {
                let d = sd(x as f32 + 0.5, y as f32 + 0.5);
                if d >= 0.5 {
                    continue;
                }
                let coverage = if d <= -0.5 { 1.0 } else { 0.5 - d };
                self.blend(x, y, color, (coverage * opacity as f32 + 0.5) as u8);
            }
        }
    }
}

/// The x range of row centre `py` inside `rect` (rounded by `radius`) shrunk
/// by `inset` on every side, if the row crosses it.
fn round_rect_core(rect: Rect, radius: f32, inset: f32) -> impl Fn(f32) -> Option<(f32, f32)> {
    let radius = radius.min(rect.w / 2.0).min(rect.h / 2.0).max(0.0);
    let (cy, hh) = (rect.y + rect.h / 2.0, rect.h / 2.0 - inset);
    let inner = (radius - inset).max(0.0);
    move |py: f32| {
        let dy = libm::fabsf(py - cy) - (hh - inner);
        if hh <= 0.0 || rect.w / 2.0 - inset <= 0.0 || dy > inner {
            return None;
        }
        let x_in = if dy > 0.0 {
            inner - libm::sqrtf(inner * inner - dy * dy)
        } else {
            0.0
        };
        Some((rect.x + inset + x_in, rect.x + rect.w - inset - x_in))
    }
}

/// How far inside the ideal "fully covered" boundary the interior shortcut
/// stays, so float rounding can never move a pixel between the two paths.
const CORE_MARGIN: f32 = 0.75;

fn hypot(x: f32, y: f32) -> f32 {
    libm::sqrtf(x * x + y * y)
}

/// Signed distance to a rounded rectangle (negative inside).
fn sd_round_rect(px: f32, py: f32, r: Rect, radius: f32) -> f32 {
    let radius = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    let (hw, hh) = (r.w / 2.0, r.h / 2.0);
    let qx = libm::fabsf(px - (r.x + hw)) - (hw - radius);
    let qy = libm::fabsf(py - (r.y + hh)) - (hh - radius);
    let outside = hypot(qx.max(0.0), qy.max(0.0));
    let inside = qx.max(qy).min(0.0);
    outside + inside - radius
}

/// Pixel bounds of `r` grown by `margin`, clipped to the screen: x0..x1, y0..y1.
fn clip(r: Rect, margin: f32) -> (i32, i32, i32, i32) {
    let x0 = libm::floorf(r.x - margin).max(0.0) as i32;
    let y0 = libm::floorf(r.y - margin).max(0.0) as i32;
    let x1 = (libm::ceilf(r.x + r.w + margin) as i32).min(WIDTH);
    let y1 = (libm::ceilf(r.y + r.h + margin) as i32).min(HEIGHT);
    (x0, x1.max(x0), y0, y1.max(y0))
}
