//! What K1 to K4 do. This one table drives both the drawing and the acting.

/// The four tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// Balance of keys and status.
    Wallet,
    /// The key map.
    Keys,
    /// Recent signatures.
    Activity,
    /// Device details and settings.
    Device,
}

/// What pressing a key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing.
    Nothing,
    /// Goes to a tab, or opens it if it's the tab on screen.
    Tab(Tab),
    /// Leaves an opened tab.
    Back,
    /// Moves up a list.
    Up,
    /// Moves down a list.
    Down,
    /// Changes or opens the selected setting.
    Select,
    /// Refuses to sign.
    Reject,
    /// Signs.
    Approve,
    /// Leaves passcode entry.
    Cancel,
    /// Removes the last digit.
    Delete,
    /// Goes to the next page.
    Next,
    /// Finishes a page.
    Done,
    /// Picks choice 0 to 3.
    Pick(u8),
    /// Closes the screen.
    Dismiss,
}

/// What a key's label says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    /// No label.
    Blank,
    /// Fixed text.
    Text(&'static str),
    /// Seconds left, which no key acts on.
    Seconds(u32),
}

/// One key's meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoftKey {
    /// What it says.
    pub label: Label,
    /// What it does.
    pub action: Action,
}

impl SoftKey {
    /// A key with a text label.
    #[must_use]
    pub const fn new(label: &'static str, action: Action) -> Self {
        Self {
            label: Label::Text(label),
            action,
        }
    }

    /// A key that does nothing and says nothing.
    pub const BLANK: Self = Self {
        label: Label::Blank,
        action: Action::Nothing,
    };
}

/// How the keys are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Plain text along the bottom.
    Tabs(Tab),
    /// Pill controls.
    Pills,
    /// Nothing, because no key does anything.
    Hidden,
}

/// What all four keys do on one screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyBar {
    /// K1 to K4.
    pub keys: [SoftKey; 4],
    /// How they look.
    pub style: Style,
}

/// A control on screen, which may span several keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    /// The first key it covers, from 0.
    pub first: usize,
    /// How many keys it covers.
    pub span: usize,
    /// What it says and does.
    pub key: SoftKey,
}

impl KeyBar {
    /// What pressing key `index` does.
    #[must_use]
    pub fn action(&self, index: usize) -> Action {
        self.keys
            .get(index)
            .map_or(Action::Nothing, |key| key.action)
    }

    /// The controls to draw, with adjacent keys of the same action as one control.
    #[must_use]
    pub fn controls(&self) -> ([Option<Control>; 4], usize) {
        let mut controls = [None; 4];
        let mut count = 0;
        let mut first = 0;
        while first < self.keys.len() {
            let key = self.keys[first];
            let same = |other: &SoftKey| other.action == key.action && other.label == key.label;
            let span = self.keys[first..]
                .iter()
                .take_while(|other| same(other))
                .count();
            controls[count] = Some(Control { first, span, key });
            count += 1;
            first += span;
        }
        (controls, count)
    }
}

const BACK: SoftKey = SoftKey::new("Back", Action::Back);
const UP: SoftKey = SoftKey::new("Up", Action::Up);
const DOWN: SoftKey = SoftKey::new("Down", Action::Down);
const REJECT: SoftKey = SoftKey::new("Reject", Action::Reject);
const CANCEL: SoftKey = SoftKey::new("Cancel", Action::Cancel);
const DELETE: SoftKey = SoftKey::new("Delete", Action::Delete);

const fn pills(keys: [SoftKey; 4]) -> KeyBar {
    KeyBar {
        keys,
        style: Style::Pills,
    }
}

/// The tab bar, with `current` marked.
#[must_use]
pub fn tabs(current: Tab) -> KeyBar {
    KeyBar {
        keys: [
            SoftKey::new("Wallet", Action::Tab(Tab::Wallet)),
            SoftKey::new("Keys", Action::Tab(Tab::Keys)),
            SoftKey::new("Activity", Action::Tab(Tab::Activity)),
            SoftKey::new("Device", Action::Tab(Tab::Device)),
        ],
        style: Style::Tabs(current),
    }
}

/// An opened Activity tab.
#[must_use]
pub fn activity_opened() -> KeyBar {
    pills([BACK, UP, DOWN, BACK])
}

/// An opened Device tab. `select` is "Change" or "Open".
#[must_use]
pub fn device_opened(select: &'static str) -> KeyBar {
    pills([BACK, UP, DOWN, SoftKey::new(select, Action::Select)])
}

/// A signing or restore screen with `seconds` left. Only K4 approves, and K3 acts on nothing, so a
/// slip can't approve.
#[must_use]
pub fn review(seconds: u32) -> KeyBar {
    let countdown = SoftKey {
        label: Label::Seconds(seconds),
        action: Action::Nothing,
    };
    pills([
        REJECT,
        REJECT,
        countdown,
        SoftKey::new("Approve", Action::Approve),
    ])
}

/// Passcode entry.
#[must_use]
pub fn passcode() -> KeyBar {
    pills([CANCEL, CANCEL, DELETE, DELETE])
}

/// A page of seed words.
#[must_use]
pub fn seed_page(last: bool) -> KeyBar {
    let forward = if last {
        SoftKey::new("Done", Action::Done)
    } else {
        SoftKey::new("Next", Action::Next)
    };
    pills([BACK, BACK, forward, forward])
}

/// The backup check, with each key offering a word.
#[must_use]
pub fn backup_check(words: [&'static str; 4]) -> KeyBar {
    let [first, second, third, fourth] = words;
    pills([
        SoftKey::new(first, Action::Pick(0)),
        SoftKey::new(second, Action::Pick(1)),
        SoftKey::new(third, Action::Pick(2)),
        SoftKey::new(fourth, Action::Pick(3)),
    ])
}

/// A screen any key closes.
#[must_use]
pub fn dismiss(label: &'static str) -> KeyBar {
    pills([SoftKey::new(label, Action::Dismiss); 4])
}

/// A screen where keys do nothing.
#[must_use]
pub fn none() -> KeyBar {
    KeyBar {
        keys: [SoftKey::BLANK; 4],
        style: Style::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Action, Label, Style, Tab, activity_opened, backup_check, device_opened, dismiss, none,
        passcode, review, seed_page, tabs,
    };

    fn labels(bar: &super::KeyBar) -> [Label; 4] {
        bar.keys.map(|key| key.label)
    }

    fn actions(bar: &super::KeyBar) -> [Action; 4] {
        bar.keys.map(|key| key.action)
    }

    fn text(words: [&'static str; 4]) -> [Label; 4] {
        words.map(Label::Text)
    }

    #[test]
    fn tabs_match_the_table() {
        let bar = tabs(Tab::Keys);
        assert_eq!(labels(&bar), text(["Wallet", "Keys", "Activity", "Device"]));
        assert_eq!(
            actions(&bar),
            [
                Action::Tab(Tab::Wallet),
                Action::Tab(Tab::Keys),
                Action::Tab(Tab::Activity),
                Action::Tab(Tab::Device)
            ]
        );
        assert_eq!(bar.style, Style::Tabs(Tab::Keys));
    }

    #[test]
    fn activity_opened_matches_the_table() {
        let bar = activity_opened();
        assert_eq!(labels(&bar), text(["Back", "Up", "Down", "Back"]));
        assert_eq!(
            actions(&bar),
            [Action::Back, Action::Up, Action::Down, Action::Back]
        );
    }

    #[test]
    fn device_opened_matches_the_table() {
        for word in ["Change", "Open"] {
            let bar = device_opened(word);
            assert_eq!(labels(&bar), text(["Back", "Up", "Down", word]));
            assert_eq!(
                actions(&bar),
                [Action::Back, Action::Up, Action::Down, Action::Select]
            );
        }
    }

    #[test]
    fn review_matches_the_table() {
        let bar = review(25);
        assert_eq!(
            labels(&bar),
            [
                Label::Text("Reject"),
                Label::Text("Reject"),
                Label::Seconds(25),
                Label::Text("Approve")
            ]
        );
        assert_eq!(
            actions(&bar),
            [
                Action::Reject,
                Action::Reject,
                Action::Nothing,
                Action::Approve
            ]
        );
    }

    #[test]
    fn only_k4_approves_anywhere() {
        for bar in [
            review(1),
            passcode(),
            seed_page(false),
            activity_opened(),
            none(),
        ] {
            for slot in 0..3 {
                assert_ne!(bar.action(slot), Action::Approve, "K{}", slot + 1);
            }
        }
    }

    #[test]
    fn passcode_matches_the_table() {
        let bar = passcode();
        assert_eq!(labels(&bar), text(["Cancel", "Cancel", "Delete", "Delete"]));
        assert_eq!(
            actions(&bar),
            [
                Action::Cancel,
                Action::Cancel,
                Action::Delete,
                Action::Delete
            ]
        );
    }

    #[test]
    fn seed_pages_match_the_table() {
        let first = seed_page(false);
        assert_eq!(labels(&first), text(["Back", "Back", "Next", "Next"]));
        assert_eq!(
            actions(&first),
            [Action::Back, Action::Back, Action::Next, Action::Next]
        );
        let last = seed_page(true);
        assert_eq!(labels(&last), text(["Back", "Back", "Done", "Done"]));
        assert_eq!(
            actions(&last),
            [Action::Back, Action::Back, Action::Done, Action::Done]
        );
    }

    #[test]
    fn backup_check_offers_a_word_on_each_key() {
        let bar = backup_check(["able", "baby", "cable", "dance"]);
        assert_eq!(labels(&bar), text(["able", "baby", "cable", "dance"]));
        assert_eq!(
            actions(&bar),
            [
                Action::Pick(0),
                Action::Pick(1),
                Action::Pick(2),
                Action::Pick(3)
            ]
        );
    }

    #[test]
    fn adjacent_keys_with_one_action_draw_as_one_control() {
        let (controls, count) = review(9).controls();
        assert_eq!(count, 3);
        let reject = controls[0].expect("there is a first control");
        assert_eq!((reject.first, reject.span), (0, 2));
        let seconds = controls[1].expect("there is a second control");
        assert_eq!((seconds.first, seconds.span), (2, 1));
        let approve = controls[2].expect("there is a third control");
        assert_eq!(
            (approve.first, approve.span, approve.key.action),
            (3, 1, Action::Approve)
        );
    }

    #[test]
    fn the_same_action_apart_draws_as_two_controls() {
        let (controls, count) = activity_opened().controls();
        assert_eq!(count, 4);
        assert_eq!(controls[0].map(|control| control.span), Some(1));
        assert_eq!(
            controls[3].map(|control| control.key.action),
            Some(Action::Back)
        );
    }

    #[test]
    fn a_dismiss_bar_is_one_control_on_all_keys() {
        let bar = dismiss("OK");
        let (controls, count) = bar.controls();
        assert_eq!(count, 1);
        assert_eq!(controls[0].map(|control| control.span), Some(4));
        for slot in 0..4 {
            assert_eq!(bar.action(slot), Action::Dismiss);
        }
    }

    #[test]
    fn every_control_acts_the_way_its_keys_do() {
        for bar in [
            tabs(Tab::Wallet),
            review(3),
            passcode(),
            seed_page(true),
            dismiss("OK"),
        ] {
            let (controls, count) = bar.controls();
            for control in controls.iter().take(count).flatten() {
                for key in control.first..control.first + control.span {
                    assert_eq!(bar.action(key), control.key.action);
                }
            }
        }
    }

    #[test]
    fn a_key_past_the_end_does_nothing() {
        assert_eq!(review(1).action(4), Action::Nothing);
    }
}
