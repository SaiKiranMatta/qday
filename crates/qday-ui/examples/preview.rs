//! Renders every screen in every theme to PNG, into
//! docs/screens/ (one folder per theme, plus a contact sheet each).
//! They come from the same code the firmware runs.
//!
//!     cargo run -p qday-ui --example preview

use std::fs::File;
use std::io::BufWriter;

use qday_ui::canvas::{Canvas, Color, HEIGHT, WIDTH};
use qday_ui::nav::Setting;
use qday_ui::screens::*;
use qday_ui::theme::{self, Theme};
use qday_wallet::settings::Scheme;

const ROOT: [u8; 32] = [
    0x9e, 0x83, 0xa9, 0x01, 0xb7, 0x57, 0xcb, 0xa3, 0x39, 0xa4, 0xd2, 0x22, 0x89, 0xdc, 0x64, 0x3b,
    0x2b, 0x22, 0xc7, 0x94, 0xa8, 0xe5, 0xc9, 0xa3, 0xb9, 0x4f, 0x85, 0x88, 0x2a, 0xf6, 0x01, 0x9c,
];
/// A made-up `keccak256(pk)`, for the ML-DSA screens.
const FINGERPRINT: [u8; 32] = [
    0x4c, 0x1d, 0x7e, 0x20, 0xb9, 0x33, 0x6a, 0x05, 0xe8, 0x1f, 0x52, 0xc4, 0x9d, 0x77, 0x0a, 0x6e,
    0x38, 0xf1, 0x2b, 0xd6, 0x84, 0x19, 0xac, 0x63, 0x5e, 0xc0, 0x27, 0x9b, 0x11, 0xfa, 0x73, 0xd2,
];
const TO: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
const TOKEN: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const DIGITS: [u8; 10] = [7, 2, 9, 4, 0, 6, 1, 8, 3, 5];
const WORDS: [&str; 24] = [
    "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract", "absurd",
    "abuse", "access", "accident", "account", "accuse", "achieve", "acid", "acoustic", "acquire",
    "across", "act", "action", "actor", "actress", "actual",
];

fn render(draw: impl FnOnce(&mut Canvas)) -> Vec<u16> {
    let mut buf = vec![0u16; (WIDTH * HEIGHT) as usize];
    let mut c = Canvas::new(&mut buf);
    draw(&mut c);
    // Exactly what the display receives: RGB565, no dithering.
    let mut out = vec![0u16; (WIDTH * HEIGHT) as usize];
    c.rows_565(0, HEIGHT, &mut out);
    out
}

fn write_png(path: &str, w: usize, h: usize, rgb: &[u8]) {
    let file = BufWriter::new(File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgb).unwrap();
}

/// One screen at 2x, so details are visible on a laptop.
fn save(path: &str, px: &[u16]) {
    let scale = 2;
    let (w, h) = (WIDTH as usize * scale, HEIGHT as usize * scale);
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let c = Color::from_565(px[(y / scale) * WIDTH as usize + x / scale]);
            rgb[(y * w + x) * 3..][..3].copy_from_slice(&[c.r, c.g, c.b]);
        }
    }
    write_png(path, w, h, &rgb);
}

/// All screens at 1x, `cols` per row.
fn sheet(path: &str, cols: usize, tiles: &[Vec<u16>]) {
    let gap = 12usize;
    let rows = tiles.len().div_ceil(cols);
    let (sw, sh) = (WIDTH as usize, HEIGHT as usize);
    let (w, h) = (cols * (sw + gap) + gap, rows * (sh + gap) + gap);
    let mut rgb = vec![0x80u8; w * h * 3];
    for (n, t) in tiles.iter().enumerate() {
        let (ox, oy) = (gap + (n % cols) * (sw + gap), gap + (n / cols) * (sh + gap));
        for y in 0..sh {
            for x in 0..sw {
                let c = Color::from_565(t[y * sw + x]);
                rgb[((oy + y) * w + ox + x) * 3..][..3].copy_from_slice(&[c.r, c.g, c.b]);
            }
        }
    }
    write_png(path, w, h, &rgb);
    eprintln!("wrote {path}");
}

type Draw = Box<dyn Fn(&mut Canvas)>;

fn screens() -> Vec<(&'static str, Draw)> {
    let status = Status {
        tree: 0,
        next_index: 67,
        connected: true,
        scheme: Scheme::Xmss,
    };
    let ml = Status {
        scheme: Scheme::MlDsa44,
        ..status
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
    let latest = || Activity {
        approved: true,
        title: "Send 0.5 ETH",
        detail: "to 0x5aAe...eAed",
        index: Some(66),
    };
    vec![
        (
            "01-wallet",
            Box::new(move |c: &mut Canvas| home(c, &status, &ROOT, false, Some(&latest()), None))
                as Draw,
        ),
        (
            "02-keys",
            Box::new(move |c: &mut Canvas| keys(c, &status, false, None)),
        ),
        (
            "03-activity",
            Box::new(move |c: &mut Canvas| {
                let items = [
                    latest(),
                    Activity {
                        approved: false,
                        title: "Rejected",
                        detail: "You rejected it",
                        index: None,
                    },
                    Activity {
                        approved: true,
                        title: "Call approve",
                        detail: "0xA0b8...eB48",
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
                activity(c, &status, &items, 0, false, None)
            }),
        ),
        (
            "04-activity-opened",
            Box::new(move |c: &mut Canvas| {
                let items = [
                    latest(),
                    Activity {
                        approved: false,
                        title: "Rejected",
                        detail: "You rejected it",
                        index: None,
                    },
                    Activity {
                        approved: true,
                        title: "Call approve",
                        detail: "0xA0b8...eB48",
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
                activity(c, &status, &items, 1, true, None)
            }),
        ),
        (
            "05-device",
            Box::new(move |c: &mut Canvas| {
                device(
                    c,
                    &status,
                    &DeviceInfo {
                        firmware: "qday 0.8",
                        root: &ROOT,
                        counter: 67,
                    },
                    Setting::Theme,
                    false,
                    None,
                )
            }),
        ),
        (
            "06-device-settings",
            Box::new(move |c: &mut Canvas| {
                device(
                    c,
                    &status,
                    &DeviceInfo {
                        firmware: "qday 0.8",
                        root: &ROOT,
                        counter: 67,
                    },
                    Setting::Theme,
                    true,
                    None,
                )
            }),
        ),
        (
            "07-review-send",
            Box::new(move |c: &mut Canvas| review(c, &send(), None)),
        ),
        (
            "08-review-rotation",
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
            "09-review-token",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Token {
                            amount: "1500000",
                            to: TO,
                            token: TOKEN,
                        },
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "10-review-call",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        review: Review::Call {
                            contract: TOKEN,
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
            "11-review-blind",
            Box::new(move |c: &mut Canvas| {
                let review_info = ReviewInfo {
                    review: Review::Blind {
                        description: "Permit2 signature",
                        hash: "0x1a2b3c4d...5e6f70",
                    },
                    fee: "unknown",
                    chain_id: 0,
                    ..send()
                };
                review(c, &review_info, None)
            }),
        ),
        (
            "12-passcode",
            Box::new(move |c: &mut Canvas| {
                let pc = Passcode {
                    title: "Enter passcode",
                    hint: "to sign 0.5 ETH",
                    entered: 3,
                    error: None,
                    digits: &DIGITS,
                    pressed: Some(4),
                };
                passcode(c, &pc, None)
            }),
        ),
        (
            "13-passcode-wrong",
            Box::new(move |c: &mut Canvas| {
                let pc = Passcode {
                    title: "Enter passcode",
                    hint: "to sign 0.5 ETH",
                    entered: 6,
                    error: Some("Wrong code, 9 tries left"),
                    digits: &DIGITS,
                    pressed: None,
                };
                passcode(c, &pc, None)
            }),
        ),
        (
            "14-building",
            Box::new(|c: &mut Canvas| {
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
            "15-signed",
            Box::new(|c: &mut Canvas| result(c, true, "Signed", "Key #0067 sent to your laptop")),
        ),
        (
            "16-rejected",
            Box::new(|c: &mut Canvas| {
                result(
                    c,
                    false,
                    "Rejected",
                    "No signature. Key #0067 is still unused.",
                )
            }),
        ),
        (
            "17-seed",
            Box::new(|c: &mut Canvas| seed_words(c, &WORDS, 0, None)),
        ),
        (
            "17b-seed-check",
            Box::new(|c: &mut Canvas| {
                seed_check(
                    c,
                    0,
                    6,
                    &["abstract", "zoo", "accident", "art"],
                    false,
                    None,
                )
            }),
        ),
        ("18-locked", Box::new(|c: &mut Canvas| locked(c, "4:59", 5))),
        (
            "19-restore",
            Box::new(|c: &mut Canvas| restore_confirm(c, 1, 64, 20, 30, None)),
        ),
        (
            "20-notice",
            Box::new(|c: &mut Canvas| notice(c, "Tree switched", "Now signing with tree 1")),
        ),
        (
            "22-wallet-ml-dsa",
            Box::new(move |c: &mut Canvas| {
                let sent = Activity {
                    index: None,
                    ..latest()
                };
                home(c, &ml, &FINGERPRINT, false, Some(&sent), None)
            }),
        ),
        (
            "23-keys-ml-dsa",
            Box::new(move |c: &mut Canvas| keys_ml(c, &ml, &FINGERPRINT, None)),
        ),
        (
            "24-device-ml-dsa",
            Box::new(move |c: &mut Canvas| {
                device(
                    c,
                    &ml,
                    &DeviceInfo {
                        firmware: "qday 0.8",
                        root: &FINGERPRINT,
                        counter: 67,
                    },
                    Setting::Signing,
                    true,
                    None,
                )
            }),
        ),
        (
            "25-review-ml-dsa",
            Box::new(move |c: &mut Canvas| {
                review(
                    c,
                    &ReviewInfo {
                        fee: "0.0035 ETH",
                        scheme: Scheme::MlDsa44,
                        ..send()
                    },
                    None,
                )
            }),
        ),
        (
            "26-switch-to-ml-dsa",
            Box::new(|c: &mut Canvas| scheme_confirm(c, Scheme::MlDsa44, 24, 30, None)),
        ),
        (
            "27-switch-to-xmss",
            Box::new(|c: &mut Canvas| scheme_confirm(c, Scheme::Xmss, 24, 30, None)),
        ),
        (
            "28-signing",
            Box::new(|c: &mut Canvas| notice(c, "Signing...", "ML-DSA-44")),
        ),
        (
            "29-signed-ml-dsa",
            Box::new(|c: &mut Canvas| result(c, true, "Signed", "ML-DSA-44 signature sent")),
        ),
        (
            "30-switched",
            Box::new(|c: &mut Canvas| result(c, true, "Switched", "Now signing with ML-DSA-44")),
        ),
        (
            "31-activity-ml-dsa",
            Box::new(move |c: &mut Canvas| {
                let items = [Activity {
                    index: None,
                    ..latest()
                }];
                activity(c, &ml, &items, 0, false, None)
            }),
        ),
        (
            "32-share-address",
            Box::new(|c: &mut Canvas| {
                share_confirm(
                    c,
                    &ShareInfo {
                        title: "Stealth address",
                        detail: "Lets anyone pay you privately",
                        gives: "your public address",
                        passcode: false,
                        seconds_left: 24,
                        timeout: 30,
                    },
                    None,
                )
            }),
        ),
        (
            "33-share-view-key",
            Box::new(|c: &mut Canvas| {
                share_confirm(
                    c,
                    &ShareInfo {
                        title: "View key",
                        detail: "Lets the laptop find your payments",
                        gives: "your view key",
                        passcode: true,
                        seconds_left: 24,
                        timeout: 30,
                    },
                    None,
                )
            }),
        ),
        (
            "34-review-stealth",
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
            "21-calibrate",
            Box::new(|c: &mut Canvas| calibrate(c, 1, false, "pen up")),
        ),
    ]
}

fn slug(t: Theme) -> String {
    t.name().to_lowercase().replace(' ', "-")
}

fn main() {
    std::env::set_current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).unwrap();
    let only: Option<String> = std::env::args().nth(1);
    for t in Theme::ALL {
        let name = slug(t);
        if only.as_ref().is_some_and(|o| *o != name) {
            continue;
        }
        theme::set(t);
        let dir = format!("docs/screens/{name}");
        std::fs::create_dir_all(&dir).unwrap();
        let mut tiles = Vec::new();
        for (screen, draw) in screens() {
            let px = render(|c| draw(c));
            save(&format!("{dir}/{screen}.png"), &px);
            tiles.push(px);
        }
        sheet(&format!("docs/screens/{name}.png"), 4, &tiles);
    }
    // The tabs in every theme, side by side.
    let mut tiles = Vec::new();
    for t in Theme::ALL {
        theme::set(t);
        for (screen, draw) in screens() {
            if ["01-wallet", "07-review-send", "12-passcode", "15-signed"].contains(&screen) {
                tiles.push(render(|c| draw(c)));
            }
        }
    }
    sheet("docs/screens/all-themes.png", 4, &tiles);
}
