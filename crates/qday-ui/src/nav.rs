//! What the four soft keys under the screen do, per screen
//!. Screens draw these labels and the
//! firmware acts on the same definitions, so a label can never say one thing
//! while the key does another.

use crate::widgets::Tab;

/// Something a key press does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Switch to a tab (a no-op on the current one).
    Goto(Tab),
    /// Open the current tab: scroll Activity, pick a setting on Device.
    Open,
    /// Leave an opened tab, or go back a page.
    Back,
    ScrollUp,
    ScrollDown,
    PrevSetting,
    NextSetting,
    /// Act on the selected setting.
    Change,
    Reject,
    Approve,
    Cancel,
    Delete,
    Next,
    Done,
    /// Choose answer `n` (0..4) in the backup check.
    Pick(u8),
}

/// How a key's control is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// One of the four tabs; `opens` when pressing it again opens the tab.
    Tab {
        selected: bool,
        opens: bool,
    },
    /// Back to where you came from.
    Back,
    /// An arrow only.
    Up,
    Down,
    /// Forward / do it: the primary control.
    Primary,
    /// A neutral control.
    Plain,
    /// Signing decisions.
    Approve,
    Reject,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SoftKey {
    pub label: &'static str,
    pub kind: Kind,
    pub action: Action,
}

/// K1..K4, left to right; `None` for a key that does nothing here.
pub type Keys = [Option<SoftKey>; 4];

/// The setting picked on the opened Device tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setting {
    Theme,
    Passcode,
    Backup,
    Signing,
}

impl Setting {
    pub const ALL: [Setting; 4] = [
        Setting::Theme,
        Setting::Passcode,
        Setting::Backup,
        Setting::Signing,
    ];

    pub fn next(self) -> Setting {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Setting {
        Self::ALL[(self as usize + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// What acting on it does.
    pub fn verb(self) -> &'static str {
        match self {
            Setting::Theme | Setting::Passcode | Setting::Signing => "Change",
            Setting::Backup => "Open",
        }
    }
}

impl Tab {
    pub const ORDER: [Tab; 4] = [Tab::Home, Tab::Keys, Tab::Activity, Tab::Device];

    pub fn name(self) -> &'static str {
        match self {
            Tab::Home => "Wallet",
            Tab::Keys => "Keys",
            Tab::Activity => "Activity",
            Tab::Device => "Device",
        }
    }

    /// Whether pressing the tab's key again opens it.
    pub fn opens(self) -> bool {
        matches!(self, Tab::Activity | Tab::Device)
    }
}

/// A window of `visible` rows over `len` rows, starting at `offset`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Scroll {
    pub offset: usize,
    pub len: usize,
    pub visible: usize,
}

impl Scroll {
    pub fn can_up(&self) -> bool {
        self.offset > 0
    }

    pub fn can_down(&self) -> bool {
        self.offset + self.visible < self.len
    }

    pub fn up(self) -> Scroll {
        Scroll {
            offset: self.offset.saturating_sub(1),
            ..self
        }
    }

    pub fn down(self) -> Scroll {
        let last = self.len.saturating_sub(self.visible);
        Scroll {
            offset: (self.offset + 1).min(last),
            ..self
        }
    }
}

const fn key(label: &'static str, kind: Kind, action: Action) -> Option<SoftKey> {
    Some(SoftKey {
        label,
        kind,
        action,
    })
}

const BACK: Option<SoftKey> = key("Back", Kind::Back, Action::Back);

/// Keys on the four tabs: browsing (`opened` false) or inside an opened tab.
pub fn tab_keys(tab: Tab, opened: bool, setting: Setting) -> Keys {
    match (tab, opened) {
        (Tab::Activity, true) => [
            BACK,
            key("Up", Kind::Up, Action::ScrollUp),
            key("Down", Kind::Down, Action::ScrollDown),
            BACK,
        ],
        (Tab::Device, true) => [
            BACK,
            key("Up", Kind::Up, Action::PrevSetting),
            key("Down", Kind::Down, Action::NextSetting),
            key(setting.verb(), Kind::Primary, Action::Change),
        ],
        _ => Tab::ORDER.map(|t| {
            let selected = t == tab;
            let opens = selected && t.opens();
            let action = if opens { Action::Open } else { Action::Goto(t) };
            key(t.name(), Kind::Tab { selected, opens }, action)
        }),
    }
}

/// Keys while reviewing a transaction (or a restore). K3 does nothing, so a
/// slip between the middle keys can't approve.
pub fn review_keys() -> Keys {
    let reject = key("Reject", Kind::Reject, Action::Reject);
    [
        reject,
        reject,
        None,
        key("Approve", Kind::Approve, Action::Approve),
    ]
}

pub fn passcode_keys() -> Keys {
    let cancel = key("Cancel", Kind::Reject, Action::Cancel);
    let delete = key("Delete", Kind::Plain, Action::Delete);
    [cancel, cancel, delete, delete]
}

/// Seed backup, two pages of 12 words.
pub fn seed_keys(page: usize) -> Keys {
    let last = if page == 0 {
        key("Next", Kind::Primary, Action::Next)
    } else {
        key("Done", Kind::Primary, Action::Done)
    };
    [BACK, BACK, last, last]
}

/// The backup check: one candidate word per key.
pub fn check_keys(options: &[&'static str; 4]) -> Keys {
    core::array::from_fn(|i| key(options[i], Kind::Plain, Action::Pick(i as u8)))
}

/// Adjacent keys `first..=last` that share one control.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Group {
    pub first: usize,
    pub last: usize,
    pub key: SoftKey,
}

/// The controls to draw: runs of adjacent identical keys merge into one.
pub fn groups(keys: &Keys) -> [Option<Group>; 4] {
    let mut out: [Option<Group>; 4] = [None; 4];
    let mut n = 0;
    for (i, k) in keys.iter().enumerate() {
        let Some(k) = *k else { continue };
        match out[..n].last_mut() {
            Some(Some(g)) if g.last + 1 == i && g.key == k => g.last = i,
            _ => {
                out[n] = Some(Group {
                    first: i,
                    last: i,
                    key: k,
                });
                n += 1;
            }
        }
    }
    out
}
