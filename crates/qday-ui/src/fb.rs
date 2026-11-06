//! A framebuffer that remembers which rows changed, so only those are sent to the screen.

use alloc::vec;
use alloc::vec::Vec;

use crate::fonts::Font;
use crate::theme::Color;

/// Screen width.
pub const WIDTH: i32 = 320;
/// Screen height.
pub const HEIGHT: i32 = 240;

const ROW_PIXELS: usize = 320;
const ROWS: usize = 240;
/// Sub-pixels per side when working out how much of a pixel a curve covers.
const SAMPLES: i32 = 4;
const SAMPLE_COUNT: u32 = 16;

/// Mixes `top` over `under`, with `alpha` from 0 to 15.
fn blend(under: Color, top: Color, alpha: u32) -> Color {
    let mix = |shift: u32, mask: u32| {
        let low = (u32::from(under) >> shift) & mask;
        let high = (u32::from(top) >> shift) & mask;
        ((low * (15 - alpha) + high * alpha + 7) / 15) << shift
    };
    u16::try_from(mix(11, 0x1F) | mix(5, 0x3F) | mix(0, 0x1F)).unwrap_or(top)
}

/// How much of the pixel at (`x`, `y`) lies in a circle around the origin, from 0 to 15.
/// Positions are in half pixels, so a circle can sit on a pixel's centre or a pixel's corner.
fn coverage(x_halves: i32, y_halves: i32, radius_halves: i32) -> u32 {
    let limit = (radius_halves * SAMPLES).pow(2);
    let mut inside = 0;
    for sub_x in 0..SAMPLES {
        for sub_y in 0..SAMPLES {
            let px = x_halves * SAMPLES + 2 * sub_x + 1 - SAMPLES;
            let py = y_halves * SAMPLES + 2 * sub_y + 1 - SAMPLES;
            if px * px + py * py <= limit {
                inside += 1;
            }
        }
    }
    inside * 15 / SAMPLE_COUNT
}

fn row_hash(pixels: &[Color]) -> u32 {
    pixels.iter().fold(0x811C_9DC5, |hash, &pixel| {
        (hash ^ u32::from(pixel)).wrapping_mul(0x0100_0193)
    })
}

/// The screen's contents.
pub struct Framebuffer {
    pixels: Vec<Color>,
    sent: Vec<Option<u32>>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    /// A black framebuffer, with every row due to be sent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: vec![0; ROW_PIXELS * ROWS],
            sent: vec![None; ROWS],
        }
    }

    fn index(x: i32, y: i32) -> Option<usize> {
        let column = usize::try_from(x)
            .ok()
            .filter(|&column| column < ROW_PIXELS)?;
        let row = usize::try_from(y).ok().filter(|&row| row < ROWS)?;
        Some(row * ROW_PIXELS + column)
    }

    /// The pixel at a position, or 0 off the screen.
    #[must_use]
    pub fn pixel(&self, x: i32, y: i32) -> Color {
        Self::index(x, y).map_or(0, |index| self.pixels[index])
    }

    fn put(&mut self, x: i32, y: i32, color: Color) {
        if let Some(index) = Self::index(x, y) {
            self.pixels[index] = color;
        }
    }

    fn mix(&mut self, x: i32, y: i32, color: Color, alpha: u32) {
        if let Some(index) = Self::index(x, y) {
            self.pixels[index] = blend(self.pixels[index], color, alpha);
        }
    }

    /// Fills the whole screen.
    pub fn clear(&mut self, color: Color) {
        self.pixels.fill(color);
    }

    /// Fills a rectangle, clipped to the screen.
    pub fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        for row in y.max(0)..(y + height).min(HEIGHT) {
            for column in x.max(0)..(x + width).min(WIDTH) {
                self.put(column, row, color);
            }
        }
    }

    /// Fills a rounded rectangle with soft corners.
    pub fn rounded(&mut self, x: i32, y: i32, width: i32, height: i32, radius: i32, color: Color) {
        self.rect(x + radius, y, width - 2 * radius, height, color);
        self.rect(x, y + radius, radius, height - 2 * radius, color);
        self.rect(
            x + width - radius,
            y + radius,
            radius,
            height - 2 * radius,
            color,
        );
        for row in 0..radius {
            for column in 0..radius {
                // The corner's centre sits on a pixel corner, so pixel centres are odd half pixels.
                let alpha = coverage(
                    2 * (radius - column) - 1,
                    2 * (radius - row) - 1,
                    2 * radius,
                );
                self.mix(x + column, y + row, color, alpha);
                self.mix(x + width - 1 - column, y + row, color, alpha);
                self.mix(x + column, y + height - 1 - row, color, alpha);
                self.mix(x + width - 1 - column, y + height - 1 - row, color, alpha);
            }
        }
    }

    /// Fills a circle with a soft edge.
    pub fn circle(&mut self, center_x: i32, center_y: i32, radius: i32, color: Color) {
        for row in -radius..=radius {
            for column in -radius..=radius {
                let alpha = coverage(2 * column, 2 * row, 2 * radius);
                self.mix(center_x + column, center_y + row, color, alpha);
            }
        }
    }

    /// Draws text with its pen starting at `x` on baseline `y`, returning where the pen ends.
    pub fn text(&mut self, font: &Font, x: i32, y: i32, text: &str, color: Color) -> i32 {
        let mut pen = x;
        for character in text.chars() {
            let glyph = font.glyph(character);
            let stride = usize::from(glyph.width).div_ceil(2);
            for row in 0..usize::from(glyph.height) {
                let line = &font.bitmap[glyph.offset + row * stride..][..stride];
                for column in 0..usize::from(glyph.width) {
                    let byte = line[column / 2];
                    let alpha = if column % 2 == 0 {
                        byte >> 4
                    } else {
                        byte & 15
                    };
                    if alpha > 0 {
                        let left = pen + i32::from(glyph.left);
                        let top = y + i32::from(glyph.top);
                        let px = left + i32::try_from(column).unwrap_or(0);
                        let py = top + i32::try_from(row).unwrap_or(0);
                        self.mix(px, py, color, u32::from(alpha));
                    }
                }
            }
            pen += i32::from(glyph.advance);
        }
        pen
    }

    /// Calls `send` for each run of changed rows, as (first row, rows, pixels), then marks them
    /// as sent.
    pub fn send_changes(&mut self, mut send: impl FnMut(i32, i32, &[Color])) {
        let mut run: Option<usize> = None;
        for row in 0..=ROWS {
            let changed = row < ROWS && {
                let hash = row_hash(&self.pixels[row * ROW_PIXELS..][..ROW_PIXELS]);
                let changed = self.sent[row] != Some(hash);
                self.sent[row] = Some(hash);
                changed
            };
            match (changed, run) {
                (true, None) => run = Some(row),
                (false, Some(first)) => {
                    let pixels = &self.pixels[first * ROW_PIXELS..row * ROW_PIXELS];
                    let top = i32::try_from(first).unwrap_or(0);
                    let rows = i32::try_from(row - first).unwrap_or(0);
                    send(top, rows, pixels);
                    run = None;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{Framebuffer, HEIGHT, WIDTH};
    use crate::fonts::{BODY, HERO};

    const RED: u16 = 0xF800;
    const WHITE: u16 = 0xFFFF;

    fn changes(fb: &mut Framebuffer) -> Vec<(i32, i32)> {
        let mut runs = Vec::new();
        fb.send_changes(|first, rows, pixels| {
            assert_eq!(i32::try_from(pixels.len()), Ok(rows * WIDTH));
            runs.push((first, rows));
        });
        runs
    }

    #[test]
    fn a_rect_fills_exactly_its_area() {
        let mut fb = Framebuffer::new();
        fb.rect(10, 20, 3, 2, RED);
        assert_eq!(fb.pixel(10, 20), RED);
        assert_eq!(fb.pixel(12, 21), RED);
        assert_eq!(fb.pixel(13, 20), 0);
        assert_eq!(fb.pixel(10, 22), 0);
        assert_eq!(fb.pixel(9, 20), 0);
    }

    #[test]
    fn drawing_is_clipped_to_the_screen() {
        let mut fb = Framebuffer::new();
        fb.rect(-5, -5, 10, 10, RED);
        fb.rect(WIDTH - 2, HEIGHT - 2, 50, 50, RED);
        fb.circle(0, 0, 8, RED);
        fb.text(&BODY, 310, 5, "clipped text", WHITE);
        fb.text(&BODY, -20, 239, "clipped text", WHITE);
        assert_eq!(fb.pixel(0, 0), RED);
        assert_eq!(fb.pixel(WIDTH - 1, HEIGHT - 1), RED);
        assert_eq!(fb.pixel(-1, 0), 0);
    }

    #[test]
    fn clear_sets_every_pixel() {
        let mut fb = Framebuffer::new();
        fb.clear(RED);
        assert_eq!(fb.pixel(0, 0), RED);
        assert_eq!(fb.pixel(WIDTH - 1, HEIGHT - 1), RED);
    }

    #[test]
    fn a_rounded_rect_cuts_its_corners() {
        let mut fb = Framebuffer::new();
        fb.rounded(10, 10, 40, 30, 8, RED);
        assert_eq!(fb.pixel(10, 10), 0);
        assert_eq!(fb.pixel(30, 10), RED);
        assert_eq!(fb.pixel(10, 25), RED);
        assert_eq!(fb.pixel(49, 39), 0);
    }

    #[test]
    fn a_circle_is_round() {
        let mut fb = Framebuffer::new();
        fb.circle(50, 50, 10, RED);
        assert_eq!(fb.pixel(50, 50), RED);
        assert_eq!(fb.pixel(50, 41), RED);
        assert_eq!(fb.pixel(42, 42), 0);
        assert_eq!(fb.pixel(50, 62), 0);
    }

    #[test]
    fn text_marks_pixels_and_moves_the_pen() {
        let mut fb = Framebuffer::new();
        let end = fb.text(&HERO, 20, 100, "H", WHITE);
        assert_eq!(end, 20 + HERO.width("H"));
        let lit = (20..end).flat_map(|x| (70..100).map(move |y| (x, y)));
        assert!(lit.filter(|&(x, y)| fb.pixel(x, y) == WHITE).count() > 40);
    }

    #[test]
    fn text_blends_into_what_is_under_it() {
        let mut fb = Framebuffer::new();
        fb.clear(0x0000);
        fb.text(&BODY, 20, 40, "S", WHITE);
        let shades = (20..30).flat_map(|x| (28..41).map(move |y| (x, y)));
        let partial = shades
            .map(|(x, y)| fb.pixel(x, y))
            .filter(|&pixel| pixel != 0 && pixel != WHITE)
            .count();
        assert!(partial > 0, "text must be anti-aliased");
    }

    #[test]
    fn the_first_frame_sends_everything_in_one_run() {
        let mut fb = Framebuffer::new();
        fb.clear(RED);
        assert_eq!(changes(&mut fb), [(0, HEIGHT)]);
    }

    #[test]
    fn nothing_changed_sends_nothing() {
        let mut fb = Framebuffer::new();
        fb.clear(RED);
        changes(&mut fb);
        fb.clear(RED);
        assert_eq!(changes(&mut fb), []);
    }

    #[test]
    fn only_changed_rows_are_sent() {
        let mut fb = Framebuffer::new();
        fb.clear(RED);
        changes(&mut fb);
        fb.rect(0, 10, 5, 2, WHITE);
        fb.rect(0, 100, 5, 1, WHITE);
        assert_eq!(changes(&mut fb), [(10, 2), (100, 1)]);
    }

    #[test]
    fn redrawing_the_same_frame_sends_nothing() {
        let mut fb = Framebuffer::new();
        fb.clear(RED);
        fb.rect(0, 10, 5, 2, WHITE);
        changes(&mut fb);
        fb.clear(RED);
        fb.rect(0, 10, 5, 2, WHITE);
        assert_eq!(changes(&mut fb), []);
    }
}
