//! The two themes. Only these greys are used, because they stay neutral on the TN panel.

#![allow(clippy::unreadable_literal)]

use qday_wallet::settings::ThemeChoice;

/// A colour in RGB565.
pub type Color = u16;

/// Converts 0xRRGGBB to RGB565.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the channels fill exactly 16 bits"
)]
pub const fn rgb(hex: u32) -> Color {
    let red = (hex >> 19) & 0x1F;
    let green = (hex >> 10) & 0x3F;
    let blue = (hex >> 3) & 0x1F;
    ((red << 11) | (green << 5) | blue) as Color
}

/// The colours of one theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Screen background.
    pub bg: Color,
    /// Cards.
    pub card: Color,
    /// Controls and bars.
    pub fill: Color,
    /// Main text.
    pub text: Color,
    /// Quiet text.
    pub muted: Color,
    /// Empty dots.
    pub dot: Color,
    /// Main action.
    pub primary: Color,
    /// Text on the main action.
    pub on_primary: Color,
    /// Success only.
    pub success: Color,
    /// Warnings.
    pub warning: Color,
    /// Danger.
    pub danger: Color,
}

/// Standard Light.
pub const LIGHT: Theme = Theme {
    bg: rgb(0xF7F7F7),
    card: rgb(0xE7E7E7),
    fill: rgb(0xD6D6D6),
    text: rgb(0x101010),
    muted: rgb(0x8C8C8C),
    dot: rgb(0xCECECE),
    primary: rgb(0x101010),
    on_primary: rgb(0xFFFFFF),
    success: rgb(0x2FC651),
    warning: rgb(0xFF9500),
    danger: rgb(0xFF3B30),
};

/// Standard Dark.
pub const DARK: Theme = Theme {
    bg: rgb(0x212121),
    card: rgb(0x313131),
    fill: rgb(0x424242),
    text: rgb(0xF7F7F7),
    muted: rgb(0x8C8C8C),
    dot: rgb(0x4A4A4A),
    primary: rgb(0xF7F7F7),
    on_primary: rgb(0x212121),
    success: rgb(0x2FC651),
    warning: rgb(0xFF9500),
    danger: rgb(0xFF3B30),
};

/// The theme for a saved choice.
#[must_use]
pub fn theme_for(choice: ThemeChoice) -> &'static Theme {
    match choice {
        ThemeChoice::Light => &LIGHT,
        ThemeChoice::Dark => &DARK,
    }
}

#[cfg(test)]
mod tests {
    use super::{DARK, LIGHT, rgb};

    #[test]
    fn colours_pack_into_rgb565() {
        assert_eq!(rgb(0x000000), 0x0000);
        assert_eq!(rgb(0xFFFFFF), 0xFFFF);
        assert_eq!(rgb(0xFF0000), 0xF800);
        assert_eq!(rgb(0x00FF00), 0x07E0);
        assert_eq!(rgb(0x0000FF), 0x001F);
    }

    #[test]
    fn no_background_is_pure_black() {
        assert_ne!(LIGHT.bg, 0);
        assert_ne!(DARK.bg, 0);
    }

    #[test]
    fn success_is_the_same_green_in_both_themes() {
        assert_eq!(LIGHT.success, DARK.success);
        assert_eq!(LIGHT.success, rgb(0x2FC651));
    }
}
