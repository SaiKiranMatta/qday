//! Inter, as 4-bit anti-aliased glyph atlases.

mod data;

pub use data::{BODY, HERO, LABEL, NUMBER, SMALL, TITLE};

/// One character in an atlas.
pub struct Glyph {
    /// Where its rows start in the bitmap.
    pub offset: usize,
    /// Width in pixels.
    pub width: u8,
    /// Height in pixels.
    pub height: u8,
    /// Pixels from the pen to the left edge.
    pub left: i8,
    /// Pixels from the baseline to the top edge, negative upwards.
    pub top: i8,
    /// Pixels to move the pen.
    pub advance: u8,
}

/// A font at one size and weight, covering ASCII.
pub struct Font {
    /// Pixels above the baseline.
    pub ascent: i32,
    /// Pixels below the baseline.
    pub descent: i32,
    /// Glyphs from space to tilde.
    pub glyphs: &'static [Glyph],
    /// Rows of two pixels per byte, each row padded to a whole byte.
    pub bitmap: &'static [u8],
}

impl Font {
    /// The glyph for a character. Anything outside ASCII becomes a question mark.
    #[must_use]
    pub fn glyph(&self, character: char) -> &Glyph {
        let code = character as usize;
        let index = if (32..=126).contains(&code) {
            code - 32
        } else {
            31
        };
        &self.glyphs[index]
    }

    /// The width of `text` in pixels.
    #[must_use]
    pub fn width(&self, text: &str) -> i32 {
        text.chars()
            .map(|character| i32::from(self.glyph(character).advance))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::{BODY, HERO, LABEL, NUMBER, SMALL, TITLE};

    #[test]
    fn every_font_covers_ascii() {
        for font in [&SMALL, &BODY, &LABEL, &TITLE, &NUMBER, &HERO] {
            assert_eq!(font.glyphs.len(), 95);
        }
    }

    #[test]
    fn each_glyph_stays_inside_its_atlas() {
        for font in [&SMALL, &BODY, &LABEL, &TITLE, &NUMBER, &HERO] {
            for glyph in font.glyphs {
                let row = usize::from(glyph.width).div_ceil(2);
                assert!(glyph.offset + row * usize::from(glyph.height) <= font.bitmap.len());
            }
        }
    }

    #[test]
    fn width_adds_up_the_advances() {
        assert!(BODY.width("ab") > BODY.width("a"));
        assert_eq!(BODY.width(""), 0);
    }

    #[test]
    fn unknown_characters_become_a_question_mark() {
        let unknown = core::ptr::from_ref(BODY.glyph('\u{e9}'));
        assert_eq!(unknown, core::ptr::from_ref(BODY.glyph('?')));
    }
}
