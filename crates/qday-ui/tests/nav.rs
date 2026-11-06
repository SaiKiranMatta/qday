use qday_ui::nav::*;
use qday_ui::theme::{Family, Theme};
use qday_ui::widgets::Tab;

fn action(keys: &Keys, i: usize) -> Option<Action> {
    keys[i].map(|k| k.action)
}

fn browse(tab: Tab) -> Keys {
    tab_keys(tab, false, Setting::Theme)
}

#[test]
fn the_four_keys_jump_straight_to_the_four_tabs() {
    let keys = browse(Tab::Keys);
    let labels: Vec<&str> = keys.iter().map(|k| k.unwrap().label).collect();
    assert_eq!(labels, ["Wallet", "Keys", "Activity", "Device"]);
    assert_eq!(action(&keys, 0), Some(Action::Goto(Tab::Home)));
    assert_eq!(action(&keys, 2), Some(Action::Goto(Tab::Activity)));
    assert_eq!(action(&keys, 3), Some(Action::Goto(Tab::Device)));
    assert_eq!(
        keys[1].unwrap().kind,
        Kind::Tab {
            selected: true,
            opens: false
        }
    );
    assert_eq!(
        keys[0].unwrap().kind,
        Kind::Tab {
            selected: false,
            opens: false
        }
    );
}

#[test]
fn pressing_the_current_tab_opens_it_when_it_has_more() {
    let keys = browse(Tab::Activity);
    assert_eq!(action(&keys, 2), Some(Action::Open));
    assert_eq!(
        keys[2].unwrap().kind,
        Kind::Tab {
            selected: true,
            opens: true
        }
    );
    assert_eq!(action(&browse(Tab::Device), 3), Some(Action::Open));
    // Wallet and Keys have nothing more: their key stays where it is.
    assert_eq!(action(&browse(Tab::Home), 0), Some(Action::Goto(Tab::Home)));
    assert!(!Tab::Home.opens() && !Tab::Keys.opens());
    assert!(Tab::Activity.opens() && Tab::Device.opens());
}

#[test]
fn opened_activity_scrolls_and_backs_out_from_either_end() {
    let keys = tab_keys(Tab::Activity, true, Setting::Theme);
    assert_eq!(
        keys.map(|k| k.map(|k| k.action)),
        [
            Some(Action::Back),
            Some(Action::ScrollUp),
            Some(Action::ScrollDown),
            Some(Action::Back)
        ]
    );
    assert_eq!(keys[1].unwrap().kind, Kind::Up);
    assert_eq!(keys[2].unwrap().kind, Kind::Down);
}

#[test]
fn opened_device_moves_between_settings_and_acts_on_one() {
    let keys = tab_keys(Tab::Device, true, Setting::Theme);
    assert_eq!(action(&keys, 0), Some(Action::Back));
    assert_eq!(action(&keys, 1), Some(Action::PrevSetting));
    assert_eq!(action(&keys, 2), Some(Action::NextSetting));
    assert_eq!(action(&keys, 3), Some(Action::Change));
    assert_eq!(keys[3].unwrap().label, "Change");
    assert_eq!(
        tab_keys(Tab::Device, true, Setting::Backup)[3]
            .unwrap()
            .label,
        "Open"
    );
    assert_eq!(
        tab_keys(Tab::Device, true, Setting::Passcode)[3]
            .unwrap()
            .label,
        "Change"
    );
    assert_eq!(
        Setting::ALL,
        [Setting::Theme, Setting::Passcode, Setting::Backup]
    );
    assert_eq!(Setting::Theme.next(), Setting::Passcode);
    assert_eq!(Setting::Backup.next(), Setting::Theme, "wraps");
    assert_eq!(Setting::Theme.prev(), Setting::Backup, "wraps");
}

#[test]
fn opening_a_tab_without_more_just_browses() {
    assert_eq!(tab_keys(Tab::Home, true, Setting::Theme), browse(Tab::Home));
}

#[test]
fn scrolling_stops_at_both_ends() {
    let top = Scroll {
        offset: 0,
        len: 7,
        visible: 4,
    };
    assert_eq!(top.up().offset, 0);
    let bottom = top.down().down().down().down();
    assert_eq!(bottom.offset, 3, "stops at the last full page");
    assert!(top.can_down() && !top.can_up());
    assert!(bottom.can_up() && !bottom.can_down());
}

/// Approve is K4 alone; K3 never acts, so a slip between the middle keys
/// can't approve. More ways to reject is the safe direction.
#[test]
fn only_k4_approves_and_both_left_keys_reject() {
    let keys = review_keys();
    assert_eq!(
        keys.map(|k| k.map(|k| k.action)),
        [
            Some(Action::Reject),
            Some(Action::Reject),
            None,
            Some(Action::Approve)
        ]
    );
    assert_eq!(keys[3].unwrap().kind, Kind::Approve);
    assert_eq!(keys[0].unwrap().kind, Kind::Reject);
}

#[test]
fn passcode_and_seed_keys_pair_up() {
    let keys = passcode_keys();
    assert_eq!(
        keys.map(|k| k.map(|k| k.action)),
        [
            Some(Action::Cancel),
            Some(Action::Cancel),
            Some(Action::Delete),
            Some(Action::Delete)
        ]
    );
    assert_eq!(action(&seed_keys(0), 1), Some(Action::Back));
    assert_eq!(action(&seed_keys(0), 2), Some(Action::Next));
    assert_eq!(action(&seed_keys(1), 3), Some(Action::Done));
}

#[test]
fn adjacent_keys_with_the_same_action_draw_as_one_control() {
    let spans: Vec<(usize, usize, Action)> = groups(&review_keys())
        .into_iter()
        .flatten()
        .map(|g| (g.first, g.last, g.key.action))
        .collect();
    assert_eq!(spans, [(0, 1, Action::Reject), (3, 3, Action::Approve)]);
    assert_eq!(groups(&browse(Tab::Home)).into_iter().flatten().count(), 4);
    let opened: Vec<(usize, usize)> = groups(&tab_keys(Tab::Activity, true, Setting::Theme))
        .into_iter()
        .flatten()
        .map(|g| (g.first, g.last))
        .collect();
    assert_eq!(
        opened,
        [(0, 0), (1, 1), (2, 2), (3, 3)],
        "Back at both ends is not adjacent"
    );
}

#[test]
fn themes_cycle_through_all_five_and_survive_a_byte() {
    let mut t = Theme::StandardLight;
    let mut seen = Vec::new();
    for _ in 0..5 {
        seen.push(t);
        t = t.next();
    }
    assert_eq!(t, Theme::StandardLight, "wraps around");
    assert_eq!(seen, Theme::ALL);
    for t in Theme::ALL {
        assert_eq!(Theme::from_u8(t as u8), Some(t));
    }
    assert_eq!(Theme::from_u8(0xFF), None, "erased flash reads as no theme");
    assert_eq!(Theme::StandardDark.family(), Family::Standard);
    assert_eq!(Theme::Brutal.family(), Family::Brutal);
    assert_eq!(Theme::RetroAmber.family(), Family::Terminal);
    assert_eq!(Theme::RetroGreen.name(), "Retro Green");
}

#[test]
fn the_backup_check_puts_one_word_on_each_key() {
    let keys = check_keys(&["apple", "zoo", "legal", "art"]);
    for (i, word) in ["apple", "zoo", "legal", "art"].iter().enumerate() {
        assert_eq!(keys[i].unwrap().label, *word);
        assert_eq!(action(&keys, i), Some(Action::Pick(i as u8)));
    }
    assert_eq!(
        groups(&keys).into_iter().flatten().count(),
        4,
        "never merged"
    );
}
