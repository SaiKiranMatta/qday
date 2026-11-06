//! A framebuffer that remembers which rows changed, so only those are sent to the screen.

use alloc::vec;
use alloc::vec::Vec;

use crate::canvas::{Canvas, HEIGHT, WIDTH};

const ROW_PIXELS: usize = 320;
const ROWS: usize = 240;

fn row_hash(pixels: &[u16]) -> u32 {
    pixels.iter().fold(0x811C_9DC5, |hash, &pixel| {
        (hash ^ u32::from(pixel)).wrapping_mul(0x0100_0193)
    })
}

/// The screen's contents, in the byte order the canvas stores: high byte first.
pub struct Framebuffer {
    pixels: Vec<u16>,
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
        debug_assert_eq!((WIDTH * HEIGHT) as usize, ROW_PIXELS * ROWS);
        Self {
            pixels: vec![0; ROW_PIXELS * ROWS],
            sent: vec![None; ROWS],
        }
    }

    /// A canvas that draws into this framebuffer.
    pub fn canvas(&mut self) -> Canvas<'_> {
        Canvas::new(&mut self.pixels)
    }

    /// The stored pixel at a position.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> u16 {
        self.pixels[y * ROW_PIXELS + x]
    }

    /// Rows that differ from what was last sent to the screen, before they are sent.
    #[must_use]
    pub fn unsent_rows(&self) -> Vec<bool> {
        (0..ROWS)
            .map(|row| {
                self.sent[row] != Some(row_hash(&self.pixels[row * ROW_PIXELS..][..ROW_PIXELS]))
            })
            .collect()
    }

    /// Calls `send` for each run of changed rows, as (first row, rows, stored pixels), then marks
    /// them as sent.
    pub fn send_changes(&mut self, mut send: impl FnMut(i32, i32, &[u16])) {
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

    use super::Framebuffer;
    use crate::canvas::{Color, HEIGHT, Rect, WIDTH};

    const RED: Color = Color::hex(0xFF0000);
    const WHITE: Color = Color::hex(0xFFFFFF);

    fn changes(fb: &mut Framebuffer) -> Vec<(i32, i32)> {
        let mut runs = Vec::new();
        fb.send_changes(|first, rows, pixels| {
            assert_eq!(i32::try_from(pixels.len()), Ok(rows * WIDTH));
            runs.push((first, rows));
        });
        runs
    }

    fn band(fb: &mut Framebuffer, y: f32, height: f32) {
        fb.canvas()
            .round_rect(Rect::new(0.0, y, 40.0, height), 0.0, WHITE, 255);
    }

    #[test]
    fn the_first_frame_sends_everything_in_one_run() {
        let mut fb = Framebuffer::new();
        fb.canvas().clear(RED);
        assert_eq!(changes(&mut fb), [(0, HEIGHT)]);
    }

    #[test]
    fn nothing_changed_sends_nothing() {
        let mut fb = Framebuffer::new();
        fb.canvas().clear(RED);
        changes(&mut fb);
        fb.canvas().clear(RED);
        assert_eq!(changes(&mut fb), []);
    }

    #[test]
    fn only_changed_rows_are_sent() {
        let mut fb = Framebuffer::new();
        fb.canvas().clear(RED);
        changes(&mut fb);
        band(&mut fb, 10.0, 2.0);
        band(&mut fb, 100.0, 1.0);
        assert_eq!(changes(&mut fb), [(10, 2), (100, 1)]);
    }

    #[test]
    fn redrawing_the_same_frame_sends_nothing() {
        let mut fb = Framebuffer::new();
        fb.canvas().clear(RED);
        band(&mut fb, 10.0, 2.0);
        changes(&mut fb);
        fb.canvas().clear(RED);
        band(&mut fb, 10.0, 2.0);
        assert_eq!(changes(&mut fb), []);
    }
}
