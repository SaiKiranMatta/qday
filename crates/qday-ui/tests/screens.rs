use std::sync::{Mutex, MutexGuard, PoisonError};

use qday_ui::canvas::{Canvas, HEIGHT, WIDTH};
use qday_ui::nav::Setting;
use qday_ui::screens::*;
use qday_ui::theme::{self, Family, Theme};
use qday_wallet::settings::Scheme;

const ROOT: [u8; 32] = [
    0x9e, 0x83, 0xa9, 0x01, 0xb7, 0x57, 0xcb, 0xa3, 0x39, 0xa4, 0xd2, 0x22, 0x89, 0xdc, 0x64, 0x3b,
    0x2b, 0x22, 0xc7, 0x94, 0xa8, 0xe5, 0xc9, 0xa3, 0xb9, 0x4f, 0x85, 0x88, 0x2a, 0xf6, 0x01, 0x9c,
];
const TO: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
const DIGITS: [u8; 10] = [7, 2, 9, 4, 0, 6, 1, 8, 3, 5];
const WORDS: [&str; 24] = ["abandon"; 24];

#[test]
fn schemes_have_their_names_on_screen() {
    assert_eq!(scheme_label(Scheme::Xmss), "XMSS");
    assert_eq!(scheme_label(Scheme::MlDsa44), "ML-DSA-44");
}

#[test]
fn the_other_scheme_is_the_one_a_switch_goes_to() {
    assert_eq!(other_scheme(Scheme::Xmss), Scheme::MlDsa44);
    assert_eq!(other_scheme(Scheme::MlDsa44), Scheme::Xmss);
}

#[test]
fn key_numbers_split_into_greyed_zeros_and_the_number() {
    let parts = |i| {
        let (a, b) = key_parts(i);
        (a.as_str().to_string(), b.as_str().to_string())
    };
    assert_eq!(parts(67), ("#00".into(), "67".into()));
    assert_eq!(parts(0), ("#000".into(), "0".into()), "always one digit");
    assert_eq!(parts(1023), ("#".into(), "1023".into()));
}

#[test]
fn hashes_shorten_to_the_parts_people_compare() {
    assert_eq!(short_hex(&ROOT).as_str(), "0x9e83...019c");
    let (head, tail) = hex_parts(&ROOT);
    assert_eq!((head.as_str(), tail.as_str()), ("0x9e83", "019c"));
}

#[test]
fn addresses_split_into_ten_groups_of_four() {
    let g = address_groups(TO).unwrap();
    assert_eq!(g[0], "5aAe");
    assert_eq!(g[9], "eAed");
    assert_eq!(address_groups("0x1234"), None);
}

type Draw = Box<dyn Fn(&mut Canvas)>;

fn every_screen() -> Vec<(&'static str, Draw)> {
    let status = Status {
        tree: 0,
        next_index: 67,
        connected: true,
        scheme: Scheme::Xmss,
    };
    let send = || ReviewInfo {
        review: Review::Send {
            amount: "0.5",
            unit: "ETH",
            to: TO,
        },
        fee: "0.0026 ETH",
        chain_id: 31337,
        scheme: Scheme::Xmss,
        signature: None,
        index: 67,
        rotation: false,
        seconds_left: 24,
        timeout: 30,
    };
    let items = [
        Activity {
            approved: true,
            title: "Send 0.5 ETH",
            detail: "to 0x5aAe...eAed",
            index: Some(66),
        },
        Activity {
            approved: false,
            title: "Rejected",
            detail: "You rejected it",
            index: None,
        },
        Activity {
            approved: true,
            title: "Call transfer",
            detail: "0x1f98...F984",
            index: Some(65),
        },
        Activity {
            approved: true,
            title: "Send 1.2 ETH",
            detail: "to 0xd8dA...6045",
            index: Some(64),
        },
        Activity {
            approved: true,
            title: "Send 2 ETH",
            detail: "to 0xd8dA...6045",
            index: Some(63),
        },
    ];
    let latest = Activity {
        approved: true,
        title: "Send 0.5 ETH",
        detail: "",
        index: Some(66),
    };
    let ml = Status {
        scheme: Scheme::MlDsa44,
        ..status
    };
    let ml_latest = Activity {
        approved: true,
        title: "Sent 0.5 ETH",
        detail: "to 0x5aAe...eAed",
        index: None,
    };
    vec![
        (
            "home",
            Box::new(move |c: &mut Canvas| home(c, &status, &ROOT, false, Some(&latest), None))
                as Draw,
        ),
        (
            "home offline",
            Box::new(move |c: &mut Canvas| {
                home(
                    c,
                    &Status {
                        connected: false,
                        ..status
                    },
                    &ROOT,
                    true,
                    None,
                    Some(0),
                )
            }),
        ),
        (
            "keys",
            Box::new(move |c: &mut Canvas| keys(c, &status, true, None)),
        ),
        (
            "activity",
            Box::new(move |c: &mut Canvas| activity(c, &status, &items, 1, true, Some(2))),
        ),
        (
            "activity empty",
            Box::new(move |c: &mut Canvas| activity(c, &status, &[], 0, false, None)),
        ),
        (
            "device",
            Box::new(move |c: &mut Canvas| {
                let info = DeviceInfo {
                    firmware: "qday 0.8",
                    root: &ROOT,
                    counter: 67,
                };
                device(c, &status, &info, Setting::Backup, true, None)
            }),
        ),
        (
            "review send",
            Box::new(move |c: &mut Canvas| review(c, &send(), Some(0))),
        ),
        (
            "review rotation",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        rotation: true,
                        index: 900,
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "review token",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Token {
                            amount: "1500000",
                            to: TO,
                            token: TO,
                        },
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "review call",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Call {
                            contract: TO,
                            value: "0 ETH",
                            function: "approve",
                        },
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "review batch",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Batch { calls: 3 },
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "review unknown",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Unknown {
                            selector: "0x12345678",
                        },
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "review blind",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Blind {
                            description: "Permit2 signature",
                            hash: "0x1a2b3c4d...5e6f70",
                        },
                        fee: "unknown",
                        chain_id: 0,
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "home ml-dsa",
            Box::new(move |c: &mut Canvas| home(c, &ml, &ROOT, false, Some(&ml_latest), None)),
        ),
        (
            "home ml-dsa offline",
            Box::new(move |c: &mut Canvas| {
                home(
                    c,
                    &Status {
                        connected: false,
                        ..ml
                    },
                    &ROOT,
                    false,
                    None,
                    Some(0),
                )
            }),
        ),
        (
            "keys ml-dsa",
            Box::new(move |c: &mut Canvas| keys_ml(c, &ml, &ROOT, Some(1))),
        ),
        (
            "device ml-dsa",
            Box::new(move |c: &mut Canvas| {
                let info = DeviceInfo {
                    firmware: "qday 0.8",
                    root: &ROOT,
                    counter: 67,
                };
                device(c, &ml, &info, Setting::Signing, true, None)
            }),
        ),
        (
            "device closed",
            Box::new(move |c: &mut Canvas| {
                let info = DeviceInfo {
                    firmware: "qday 0.8",
                    root: &ROOT,
                    counter: 67,
                };
                device(c, &status, &info, Setting::Theme, false, Some(3))
            }),
        ),
        (
            "review ml-dsa",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        scheme: Scheme::MlDsa44,
                        ..send()
                    },
                    Some(3),
                )
            }),
        ),
        (
            "review ml-dsa long amount",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Send {
                            amount: "1234567.123456789012",
                            unit: "ETH",
                            to: TO,
                        },
                        fee: "0.0035 ETH",
                        scheme: Scheme::MlDsa44,
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "switch to ml-dsa",
            Box::new(|c: &mut Canvas| scheme_confirm(c, Scheme::MlDsa44, 20, 30, Some(3))),
        ),
        (
            "switch to xmss",
            Box::new(|c: &mut Canvas| scheme_confirm(c, Scheme::Xmss, 20, 30, None)),
        ),
        (
            "share an invoice",
            Box::new(|c: &mut Canvas| {
                share_confirm(
                    c,
                    &ShareInfo {
                        title: "Invoice #1000000",
                        detail: "A fresh key to be paid on",
                        gives: "one public key",
                        passcode: false,
                        seconds_left: 20,
                        timeout: 30,
                    },
                    Some(3),
                )
            }),
        ),
        (
            "share the view key",
            Box::new(|c: &mut Canvas| {
                share_confirm(
                    c,
                    &ShareInfo {
                        title: "View key",
                        detail: "Lets the laptop find your payments",
                        gives: "your view key",
                        passcode: true,
                        seconds_left: 20,
                        timeout: 30,
                    },
                    None,
                )
            }),
        ),
        (
            "review stealth",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Send {
                            amount: "Whole note",
                            unit: "",
                            to: TO,
                        },
                        fee: "0.0003 ETH",
                        signature: Some("One-time ML-DSA"),
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "review invoice",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Send {
                            amount: "All of #1000000",
                            unit: "",
                            to: TO,
                        },
                        fee: "0.0003 ETH",
                        signature: Some("One-time hash"),
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "signing",
            Box::new(|c: &mut Canvas| notice(c, "Signing...", "ML-DSA-44")),
        ),
        (
            "signed ml-dsa",
            Box::new(|c: &mut Canvas| result(c, true, "Signed", "ML-DSA-44 signature sent")),
        ),
        (
            "rejected ml-dsa",
            Box::new(|c: &mut Canvas| result(c, false, "Rejected", "No signature was made.")),
        ),
        (
            "switched",
            Box::new(|c: &mut Canvas| result(c, true, "Switched", "Now signing with ML-DSA-44")),
        ),
        (
            "passcode",
            Box::new(move |c: &mut Canvas| {
                passcode(
                    c,
                    &Passcode {
                        title: "Enter passcode",
                        hint: "to sign 0.5 ETH",
                        entered: 3,
                        error: None,
                        digits: &DIGITS,
                        pressed: Some(4),
                    },
                    None,
                )
            }),
        ),
        (
            "passcode wrong",
            Box::new(move |c: &mut Canvas| {
                passcode(
                    c,
                    &Passcode {
                        title: "Enter passcode",
                        hint: "to sign 0.5 ETH",
                        entered: 6,
                        error: Some("Wrong code, 9 tries left"),
                        digits: &DIGITS,
                        pressed: None,
                    },
                    Some(3),
                )
            }),
        ),
        (
            "progress",
            Box::new(move |c: &mut Canvas| {
                progress(
                    c,
                    "Setting up",
                    &[
                        "Building your 1,024 keys.",
                        "Once per key tree,",
                        "about 3 minutes.",
                    ],
                    431,
                    1024,
                )
            }),
        ),
        (
            "signed",
            Box::new(move |c: &mut Canvas| {
                result(c, true, "Signed", "Key #0067 sent to your laptop")
            }),
        ),
        (
            "rejected",
            Box::new(move |c: &mut Canvas| {
                result(
                    c,
                    false,
                    "Rejected",
                    "No signature. Key #0067 is still unused.",
                )
            }),
        ),
        (
            "seed",
            Box::new(move |c: &mut Canvas| seed_words(c, &WORDS, 1, None)),
        ),
        (
            "locked",
            Box::new(move |c: &mut Canvas| locked(c, "4:59", 5)),
        ),
        (
            "restore",
            Box::new(move |c: &mut Canvas| restore_confirm(c, 1, 64, 20, 30, None)),
        ),
        (
            "notice",
            Box::new(move |c: &mut Canvas| notice(c, "Tree switched", "Now signing with tree 1")),
        ),
        (
            "seed check",
            Box::new(|c: &mut Canvas| {
                seed_check(
                    c,
                    0,
                    6,
                    &["abstract", "zoo", "accident", "art"],
                    false,
                    Some(2),
                )
            }),
        ),
        (
            "seed check wrong",
            Box::new(|c: &mut Canvas| {
                seed_check(c, 2, 23, &["legal", "winner", "thank", "year"], true, None)
            }),
        ),
        (
            "calibrate 1",
            Box::new(|c: &mut Canvas| calibrate(c, 0, false, "pen up")),
        ),
        (
            "calibrate 2",
            Box::new(|c: &mut Canvas| calibrate(c, 1, true, "pen down  raw 1234, 2345")),
        ),
        (
            "calibrate 3",
            Box::new(|c: &mut Canvas| calibrate(c, 2, false, "")),
        ),
    ]
}

/// Taken by tests that change the theme.
fn theme_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

fn drawn(draw: impl FnOnce(&mut Canvas)) -> Vec<u16> {
    let mut buf = vec![0u16; (WIDTH * HEIGHT) as usize];
    draw(&mut Canvas::new(&mut buf));
    buf
}

/// The scheme has to show on these screens in every theme.
#[test]
fn the_scheme_changes_what_every_theme_shows() {
    let _theme = theme_lock();
    let info = DeviceInfo {
        firmware: "qday 0.8",
        root: &ROOT,
        counter: 67,
    };
    let review_info = |scheme| ReviewInfo {
        review: Review::Send {
            amount: "0.5",
            unit: "ETH",
            to: TO,
        },
        fee: "0.0035 ETH",
        chain_id: 31337,
        scheme,
        signature: None,
        index: 0,
        rotation: false,
        seconds_left: 24,
        timeout: 30,
    };
    for t in Theme::ALL {
        theme::set(t);
        for scheme in [Scheme::Xmss, Scheme::MlDsa44] {
            let status = Status {
                tree: 0,
                next_index: 0,
                connected: true,
                scheme,
            };
            let other = Status {
                scheme: other_scheme(scheme),
                ..status
            };
            assert!(
                drawn(|c| home(c, &status, &ROOT, false, None, None))
                    != drawn(|c| home(c, &other, &ROOT, false, None, None)),
                "{t:?} home"
            );
            assert!(
                drawn(|c| device(c, &status, &info, Setting::Theme, false, None))
                    != drawn(|c| device(c, &other, &info, Setting::Theme, false, None)),
                "{t:?} device"
            );
            let one_time = ReviewInfo {
                signature: Some("One-time ML-DSA"),
                ..review_info(scheme)
            };
            assert!(
                drawn(|c| review(c, &one_time, None))
                    != drawn(|c| review(c, &review_info(scheme), None)),
                "{t:?} one-time review"
            );
            assert!(
                drawn(|c| review(c, &review_info(scheme), None))
                    != drawn(|c| review(c, &review_info(other_scheme(scheme)), None)),
                "{t:?} review"
            );
        }
    }
    theme::set(Theme::StandardLight);
}

/// Every screen in every theme renders, and (outside the terminal family,
/// whose title bar is full-bleed) leaves the outer frame empty: the margin
/// rule.
#[test]
fn every_screen_renders_in_every_theme_inside_the_margins() {
    let _theme = theme_lock();
    let mut buf = vec![0u16; (WIDTH * HEIGHT) as usize];
    for t in Theme::ALL {
        theme::set(t);
        for (name, draw) in every_screen() {
            let mut c = Canvas::new(&mut buf);
            draw(&mut c);
            if t.family() == Family::Terminal {
                continue;
            }
            let bg = c.get(0, 0);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let frame = !(8..WIDTH - 8).contains(&x) || !(4..HEIGHT - 6).contains(&y);
                    if frame {
                        assert_eq!(
                            c.get(x, y),
                            bg,
                            "{t:?} {name}: drawn in the margin at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
    theme::set(Theme::StandardLight);
}
