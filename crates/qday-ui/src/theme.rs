//! The five themes. A theme picks a style
//! family, which owns the layouts, fonts and colours (`ui::standard`,
//! `ui::brutal`, `ui::terminal`); themes in a family differ only in colour.

use core::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Theme {
    StandardLight = 0,
    StandardDark = 1,
    Brutal = 2,
    RetroGreen = 3,
    RetroAmber = 4,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Standard,
    Brutal,
    Terminal,
}

impl Theme {
    /// Picker order.
    pub const ALL: [Theme; 5] = [
        Theme::StandardLight,
        Theme::StandardDark,
        Theme::Brutal,
        Theme::RetroGreen,
        Theme::RetroAmber,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Theme::StandardLight => "Standard Light",
            Theme::StandardDark => "Standard Dark",
            Theme::Brutal => "Neo-brutal",
            Theme::RetroGreen => "Retro Green",
            Theme::RetroAmber => "Retro Amber",
        }
    }

    pub fn family(self) -> Family {
        match self {
            Theme::StandardLight | Theme::StandardDark => Family::Standard,
            Theme::Brutal => Family::Brutal,
            Theme::RetroGreen | Theme::RetroAmber => Family::Terminal,
        }
    }

    pub fn next(self) -> Theme {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    /// The theme stored as `byte`, or `None` (for example erased flash).
    pub fn from_u8(byte: u8) -> Option<Theme> {
        Self::ALL.get(byte as usize).copied()
    }
}

static THEME: AtomicU8 = AtomicU8::new(Theme::StandardLight as u8);

pub fn set(theme: Theme) {
    THEME.store(theme as u8, Ordering::Relaxed);
}

pub fn current() -> Theme {
    Theme::from_u8(THEME.load(Ordering::Relaxed)).unwrap_or(Theme::StandardLight)
}
