//! Anti-aliased Inter glyphs, pre-rendered by `qday-fontgen`.

mod generated;

/// Bricolage Grotesque, for the Neo-brutal theme.
pub mod bricolage;

pub use generated::{BODY, BODY_STRONG, CAPTION, DISPLAY, HEADLINE, HERO, TITLE};

/// One glyph: a 4-bit alpha bitmap placed relative to the pen position.
pub struct Glyph {
    /// Pixels from the pen to the bitmap's left edge.
    pub left: i8,
    /// Pixels from the baseline up to the bitmap's top edge.
    pub top: i8,
    pub w: u8,
    pub h: u8,
    /// Pen advance in 1/16 px.
    pub advance: u16,
    /// Start in `Font::pixels`.
    pub offset: u32,
}

pub struct Font {
    pub size: u8,
    /// Pixels above the baseline (positive) and below it (negative).
    pub ascent: i16,
    pub descent: i16,
    pub first: u8,
    pub glyphs: &'static [Glyph],
    pub pixels: &'static [u8],
    /// (left, right, adjustment in 1/16 px), sorted.
    pub kerning: &'static [(u8, u8, i8)],
}

impl Font {
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        let i = (c as u32).checked_sub(self.first as u32)? as usize;
        self.glyphs.get(i)
    }

    /// Extra spacing between `a` and `b` in 1/16 px.
    pub fn kern(&self, a: char, b: char) -> i32 {
        let (Ok(a), Ok(b)) = (u8::try_from(a), u8::try_from(b)) else {
            return 0;
        };
        self.kerning
            .binary_search_by(|&(l, r, _)| (l, r).cmp(&(a, b)))
            .map_or(0, |i| self.kerning[i].2 as i32)
    }

    /// Width of `text` in pixels, with `tracking` extra 1/16 px per gap.
    pub fn width(&self, text: &str, tracking: i32) -> i32 {
        let mut pen = 0i32;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                pen += self.kern(p, c) + tracking;
            }
            pen += self.glyph(c).map_or(0, |g| g.advance as i32);
            prev = Some(c);
        }
        (pen + 8) / 16
    }
}
