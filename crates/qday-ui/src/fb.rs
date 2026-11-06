//! A framebuffer that remembers which rows changed, so only those are sent to the screen.

use alloc::vec;
use alloc::vec::Vec;

use crate::canvas::{Canvas, HEIGHT, WIDTH};

const ROW_PIXELS: usize = 320;
const ROWS: usize = 240;
/// The pixels in one screen.
pub const PIXELS: usize = ROW_PIXELS * ROWS;

fn row_hash(pixels: &[u16]) -> u32 {
    pixels.iter().fold(0x811C_9DC5, |hash, &pixel| {
        (hash ^ u32::from(pixel)).wrapping_mul(0x0100_0193)
    })
}

/// Where the pixels live: the heap, or memory the firmware set aside.
enum Pixels {
    Heap(Vec<u16>),
    Placed(&'static mut [u16]),
}

impl Pixels {
    fn get(&self) -> &[u16] {
        match self {
            Self::Heap(pixels) => pixels,
            Self::Placed(pixels) => pixels,
        }
    }

    fn get_mut(&mut self) -> &mut [u16] {
        match self {
            Self::Heap(pixels) => pixels,
            Self::Placed(pixels) => pixels,
        }
    }
}

/// The screen's contents, in the byte order the canvas stores: high byte first.
pub struct Framebuffer {
    pixels: Pixels,
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
        debug_assert_eq!((WIDTH * HEIGHT) as usize, PIXELS);
        Self {
            pixels: Pixels::Heap(vec![0; PIXELS]),
            sent: vec![None; ROWS],
        }
    }

    /// A black framebuffer in memory the caller set aside, with every row due to be sent.
    ///
    /// # Panics
    ///
    /// Panics unless `pixels` holds exactly one screen.
    #[must_use]
    pub fn placed(pixels: &'static mut [u16]) -> Self {
        assert_eq!(pixels.len(), PIXELS, "a framebuffer holds one screen");
        pixels.fill(0);
        Self {
            pixels: Pixels::Placed(pixels),
            sent: vec![None; ROWS],
        }
    }

    /// A canvas that draws into this framebuffer.
    pub fn canvas(&mut self) -> Canvas<'_> {
        Canvas::new(self.pixels.get_mut())
    }

    /// The stored pixel at a position.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> u16 {
        self.pixels.get()[y * ROW_PIXELS + x]
    }

    /// Calls `send` for each run of changed rows, as (first row, rows, stored pixels), then marks
    /// them as sent.
    pub fn send_changes(&mut self, mut send: impl FnMut(i32, i32, &[u16])) {
        let mut run: Option<usize> = None;
        let screen = self.pixels.get();
        for row in 0..=ROWS {
            let changed = row < ROWS && {
                let hash = row_hash(&screen[row * ROW_PIXELS..][..ROW_PIXELS]);
                let changed = self.sent[row] != Some(hash);
                self.sent[row] = Some(hash);
                changed
            };
            match (changed, run) {
                (true, None) => run = Some(row),
                (false, Some(first)) => {
                    let pixels = &screen[first * ROW_PIXELS..row * ROW_PIXELS];
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

    use alloc::vec;

    use super::{Framebuffer, PIXELS};
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

    fn placed() -> Framebuffer {
        Framebuffer::placed(vec![0xAAAA; PIXELS].leak())
    }

    #[test]
    fn a_placed_framebuffer_starts_black() {
        let fb = placed();
        assert_eq!(fb.pixel(0, 0), 0);
        assert_eq!(fb.pixel(319, 239), 0);
    }

    #[test]
    fn a_placed_framebuffer_draws_like_any_other() {
        let mut fb = placed();
        let mut heap = Framebuffer::new();
        for frame in [&mut fb, &mut heap] {
            frame.canvas().clear(RED);
            band(frame, 10.0, 2.0);
        }
        for (x, y) in [(0, 0), (5, 10), (319, 239)] {
            assert_eq!(fb.pixel(x, y), heap.pixel(x, y));
        }
        assert_eq!(changes(&mut fb), [(0, HEIGHT)]);
    }

    #[test]
    #[should_panic(expected = "a framebuffer holds one screen")]
    fn a_placed_framebuffer_must_hold_one_screen() {
        let _ = Framebuffer::placed(vec![0; PIXELS - 1].leak());
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
