//! Layout rules, checked on every screen in both themes.

use qday_ui::fb::{Framebuffer, HEIGHT, WIDTH};
use qday_ui::gallery::Samples;
use qday_ui::keys::{Action, Label, Style};
use qday_ui::screens::{
    PAD_GAP_X, PAD_GAP_Y, PAD_KEY_HEIGHT, PAD_KEY_WIDTH, PAD_LEFT, PAD_TOP, PILL_CONTENT_END,
    PILLS_TOP, Screen, TAB_CONTENT_END, keypad_hit,
};
use qday_ui::theme::{DARK, LIGHT, Theme};

/// The outer margin. The sides are 12 px. The top and bottom follow the fixed positions in the
/// design: the keypad starts at y 8, and text tabs reach y 229.
const LEFT: i32 = 12;
const RIGHT: i32 = 308;
const TOP: i32 = 8;
const BOTTOM: i32 = 230;

fn render(screen: &Screen<'_>, theme: &Theme) -> Framebuffer {
    let mut fb = Framebuffer::new();
    screen.draw(&mut fb, theme);
    fb
}

/// The first non-background pixel in the outer margin.
fn margin_breach(fb: &Framebuffer, theme: &Theme) -> Option<(i32, i32)> {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let outside = !(LEFT..RIGHT).contains(&x) || !(TOP..BOTTOM).contains(&y);
            if outside && fb.pixel(x, y) != theme.bg {
                return Some((x, y));
            }
        }
    }
    None
}

/// Whether a band of rows is empty.
fn rows_clear(fb: &Framebuffer, theme: &Theme, from: i32, to: i32) -> bool {
    (from..to).all(|y| (0..WIDTH).all(|x| fb.pixel(x, y) == theme.bg))
}

fn each_screen(mut check: impl FnMut(&str, &Screen<'_>, &Theme, &Framebuffer)) {
    let samples = Samples::new();
    for (name, screen) in samples.screens() {
        for theme in [&LIGHT, &DARK] {
            let fb = render(&screen, theme);
            check(name, &screen, theme, &fb);
        }
    }
}

#[test]
fn the_margin_check_can_fail() {
    let samples = Samples::new();
    let (_, screen) = samples.screens()[0];
    let mut fb = render(&screen, &LIGHT);
    assert_eq!(margin_breach(&fb, &LIGHT), None);
    fb.rect(3, 100, 1, 1, LIGHT.text);
    assert_eq!(margin_breach(&fb, &LIGHT), Some((3, 100)));
}

#[test]
fn nothing_is_drawn_in_the_outer_margin() {
    each_screen(|name, _, theme, fb| {
        assert_eq!(margin_breach(fb, theme), None, "{name}");
    });
}

#[test]
fn every_screen_fills_the_background_and_draws_something() {
    each_screen(|name, _, theme, fb| {
        assert_eq!(fb.pixel(0, 0), theme.bg, "{name}");
        let drawn = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| fb.pixel(x, y) != theme.bg)
            .count();
        assert!(drawn > 200, "{name} drew {drawn} pixels");
    });
}

#[test]
fn tab_content_stops_above_the_tabs() {
    each_screen(|name, screen, theme, fb| {
        if matches!(screen.keys().style, Style::Tabs(_)) {
            assert!(rows_clear(fb, theme, TAB_CONTENT_END + 1, 219), "{name}");
        }
    });
}

#[test]
fn pill_content_stops_12_px_above_the_pills() {
    each_screen(|name, screen, theme, fb| {
        if screen.keys().style == Style::Pills {
            assert!(
                rows_clear(fb, theme, PILL_CONTENT_END + 1, PILLS_TOP),
                "{name}"
            );
        }
    });
}

#[test]
fn a_screen_with_labels_draws_them_in_the_key_area() {
    each_screen(|name, screen, theme, fb| {
        let bar = screen.keys();
        let labelled = bar.keys.iter().any(|key| key.label != Label::Blank);
        assert_eq!(labelled, bar.style != Style::Hidden, "{name}");
        if labelled {
            let top = if bar.style == Style::Pills {
                PILLS_TOP
            } else {
                219
            };
            assert!(!rows_clear(fb, theme, top, BOTTOM), "{name}");
        }
    });
}

#[test]
fn only_review_screens_can_approve() {
    each_screen(|name, screen, _, _| {
        let approves = (0..4).any(|key| screen.keys().action(key) == Action::Approve);
        let reviewing = name.starts_with("review") || name == "restore-confirm";
        assert_eq!(approves, reviewing, "{name}");
        if reviewing {
            assert_eq!(screen.keys().action(3), Action::Approve);
            assert_eq!(screen.keys().action(2), Action::Nothing);
        }
    });
}

#[test]
fn pure_black_is_never_a_background() {
    each_screen(|name, _, _, fb| {
        assert_ne!(fb.pixel(1, 1), 0, "{name}");
    });
}

#[test]
fn only_a_success_may_use_green_as_its_message() {
    // The refused result must show no green at all.
    let samples = Samples::new();
    let screens = samples.screens();
    let refused = screens
        .iter()
        .find(|(name, _)| *name == "result-refused")
        .expect("the gallery has it");
    let fb = render(&refused.1, &LIGHT);
    let green = (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .any(|(x, y)| fb.pixel(x, y) == LIGHT.success);
    assert!(!green);
}

fn key_corner(column: i32, row: i32) -> (i32, i32) {
    (
        PAD_LEFT + column * (PAD_KEY_WIDTH + PAD_GAP_X),
        PAD_TOP + row * (PAD_KEY_HEIGHT + PAD_GAP_Y),
    )
}

#[test]
fn the_keypad_spans_its_fixed_area() {
    assert_eq!(key_corner(2, 3).0 + PAD_KEY_WIDTH, 308);
    assert_eq!(key_corner(2, 3).1 + PAD_KEY_HEIGHT, 180);
}

#[test]
fn each_digit_key_answers_to_its_own_area() {
    let mut seen = [false; 10];
    for row in 0..4 {
        for column in 0..3 {
            let (x, y) = key_corner(column, row);
            let hits = [
                keypad_hit(x, y),
                keypad_hit(x + PAD_KEY_WIDTH - 1, y),
                keypad_hit(x, y + PAD_KEY_HEIGHT - 1),
                keypad_hit(x + 20, y + 20),
            ];
            assert!(hits.iter().all(|hit| *hit == hits[0]), "{row},{column}");
            if let Some(index) = hits[0] {
                assert!(!seen[index], "digit {index} appears twice");
                seen[index] = true;
            }
        }
    }
    assert_eq!(seen, [true; 10]);
}

#[test]
fn gaps_between_keys_count_as_nothing() {
    let (x, y) = key_corner(0, 0);
    assert_eq!(keypad_hit(x + PAD_KEY_WIDTH, y + 5), None);
    assert_eq!(keypad_hit(x + PAD_KEY_WIDTH + PAD_GAP_X - 1, y + 5), None);
    assert_eq!(keypad_hit(x + 5, y + PAD_KEY_HEIGHT), None);
    assert_eq!(keypad_hit(x + 5, y + PAD_KEY_HEIGHT + PAD_GAP_Y - 1), None);
}

#[test]
fn touches_off_the_keypad_count_as_nothing() {
    assert_eq!(keypad_hit(145, 20), None);
    assert_eq!(keypad_hit(308, 20), None);
    assert_eq!(keypad_hit(200, 7), None);
    assert_eq!(keypad_hit(200, 180), None);
}

#[test]
fn the_keypad_shows_every_digit_on_a_filled_key() {
    let samples = Samples::new();
    let screens = samples.screens();
    let passcode = screens
        .iter()
        .find(|(name, _)| *name == "passcode")
        .expect("it exists");
    let fb = render(&passcode.1, &LIGHT);
    let mut keys = 0;
    for row in 0..4 {
        for column in 0..3 {
            let (x, y) = key_corner(column, row);
            if fb.pixel(x + 25, y + 3) == LIGHT.fill {
                keys += 1;
            }
        }
    }
    assert_eq!(keys, 10);
}
