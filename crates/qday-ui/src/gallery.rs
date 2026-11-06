//! One sample of every screen, for the previews and the layout tests.

use alloc::vec::Vec;

use qday_wallet::activity::{Activity, Entry, Summary};
use qday_wallet::frame::{Action, FrameTx};
use qday_wallet::settings::ThemeChoice;
use qday_wallet::tokens;

use crate::keys::Tab;
use crate::screens::{Home, Screen};

/// Sample data that the sample screens borrow.
pub struct Samples {
    /// Home data.
    pub home: Home,
    /// Home data with no laptop and no history.
    pub idle: Home,
    /// A full activity log.
    pub activity: Activity,
    /// An empty activity log.
    pub empty: Activity,
    /// A transfer.
    pub transfer: FrameTx,
    /// A very large, very exact transfer.
    pub exact: FrameTx,
    /// The largest amount a transaction can hold.
    pub huge: FrameTx,
    /// A rotation.
    pub rotation: FrameTx,
    /// A call the device doesn't recognise.
    pub unknown: FrameTx,
    /// A transfer of a token the device knows.
    pub token: FrameTx,
    /// A transfer of a token the device doesn't know.
    pub unknown_token: FrameTx,
    /// A shuffled keypad.
    pub digits: [u8; 10],
    /// Backup words.
    pub words: [&'static str; 24],
}

impl Samples {
    /// Builds the samples.
    #[must_use]
    pub fn new() -> Self {
        let key: [u8; 32] =
            core::array::from_fn(|i| [0xbf, 0xef, 0xd6, 0xec, 0xcd, 0x30, 0xb7, 0xf3][i % 8]);
        let to = [
            0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5,
            0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xC8,
        ];
        let latest = Entry {
            signed: true,
            index: 66,
            summary: Summary::Transfer {
                to,
                value: 1_000_000_000_000_000,
            },
        };
        let home = Home {
            key,
            usb: true,
            next_index: 67,
            tree: 0,
            counter: 67,
            version: "0.1.0",
            latest: Some(latest),
            theme: ThemeChoice::Light,
            next_ready: false,
        };
        let mut activity = Activity::default();
        for index in 47..67 {
            let summary = match index % 5 {
                0 => Summary::Other,
                2 => Summary::TokenTransfer {
                    token: tokens::KNOWN[0].address,
                    to,
                    amount: u128::from(index) * 1_000_000,
                },
                1 => Summary::Rotation,
                _ => Summary::Transfer {
                    to,
                    value: u128::from(index) * 10_000_000_000_000_000,
                },
            };
            activity.push(Entry {
                signed: index % 4 != 0,
                index,
                summary,
            });
        }
        let sender = [0x11; 20];
        let mut unknown = FrameTx::transfer(sender, 7, 67, to, 0);
        unknown.action = Action::Other {
            target: Some(to),
            value: 25_000_000_000_000_000,
            data: alloc::vec![0xa9, 0x05, 0x9c, 0xbb, 0, 0, 0, 1],
            execution: 50_000,
            state: 200_000,
        };
        Self {
            idle: Home {
                usb: false,
                latest: None,
                next_index: 0,
                counter: 0,
                ..home
            },
            home,
            activity,
            empty: Activity::default(),
            transfer: FrameTx::transfer(sender, 7, 67, to, 1_000_000_000_000_000),
            exact: FrameTx::transfer(sender, 7, 67, to, 1_234_567_890_123_456_789),
            huge: FrameTx::transfer(sender, 7, 67, to, u128::MAX),
            rotation: FrameTx::rotation(sender, 7, 1008, [0x18; 32], [0x8c; 32]),
            unknown,
            token: FrameTx::token_transfer(
                sender,
                7,
                67,
                tokens::KNOWN[0].address,
                to,
                125_500_000,
            ),
            unknown_token: FrameTx::token_transfer(sender, 7, 67, [0x5c; 20], to, 125_500_000),
            digits: [7, 2, 9, 0, 5, 1, 8, 3, 6, 4],
            words: [
                "abandon", "language", "wonder", "orbit", "feature", "clever", "mountain",
                "rescue", "thunder", "uniform", "velvet", "whisper", "actress", "bracket",
                "custom", "dolphin", "exhaust", "fragile", "glimpse", "harvest", "insect",
                "journey", "kingdom", "lecture",
            ],
        }
    }

    /// Every screen, with a name for its preview file.
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "this is a flat list of samples")]
    pub fn screens(&self) -> Vec<(&'static str, Screen<'_>)> {
        alloc::vec![
            (
                "wallet",
                Screen::Tab(Tab::Wallet, &self.home, &self.activity)
            ),
            (
                "wallet-idle",
                Screen::Tab(Tab::Wallet, &self.idle, &self.empty)
            ),
            ("keys", Screen::Tab(Tab::Keys, &self.home, &self.activity)),
            (
                "activity",
                Screen::Tab(Tab::Activity, &self.home, &self.activity)
            ),
            (
                "activity-empty",
                Screen::Tab(Tab::Activity, &self.idle, &self.empty)
            ),
            (
                "device",
                Screen::Tab(Tab::Device, &self.home, &self.activity)
            ),
            ("activity-opened", Screen::ActivityList(&self.activity, 4)),
            ("device-opened", Screen::DeviceMenu(&self.home, 0)),
            ("device-opened-seed", Screen::DeviceMenu(&self.home, 2)),
            ("review-transfer", Screen::Review(&self.transfer, 25)),
            ("review-exact", Screen::Review(&self.exact, 18)),
            ("review-huge", Screen::Review(&self.huge, 9)),
            ("review-rotation", Screen::Review(&self.rotation, 30)),
            ("review-warning", Screen::Review(&self.unknown, 12)),
            ("review-token", Screen::Review(&self.token, 25)),
            (
                "review-token-unknown",
                Screen::Review(&self.unknown_token, 12)
            ),
            (
                "restore-confirm",
                Screen::RestoreConfirm {
                    tree: 2,
                    first_index: 164,
                    seconds: 22
                }
            ),
            (
                "passcode",
                Screen::Passcode {
                    title: "Enter passcode",
                    hint: "To sign this",
                    entered: 3,
                    digits: &self.digits,
                }
            ),
            (
                "progress",
                Screen::Progress {
                    title: "Building keys",
                    done: 412,
                    total: 1024
                }
            ),
            (
                "result-signed",
                Screen::Result {
                    success: true,
                    title: "Signed",
                    detail: "Key #0067 used"
                }
            ),
            (
                "result-refused",
                Screen::Result {
                    success: false,
                    title: "Refused",
                    detail: "Nothing was signed"
                }
            ),
            (
                "seed-page-1",
                Screen::SeedWords {
                    words: &self.words,
                    page: 0
                }
            ),
            (
                "seed-page-2",
                Screen::SeedWords {
                    words: &self.words,
                    page: 1
                }
            ),
            (
                "backup-check",
                Screen::BackupCheck {
                    number: 1,
                    position: 6,
                    choices: ["mountain", "clever", "rescue", "orbit"],
                }
            ),
            (
                "locked",
                Screen::Locked {
                    wait_ms: 300_000,
                    tries_left: 4
                }
            ),
            (
                "notice",
                Screen::Notice {
                    title: "Rotation needed",
                    first: "Only reserved keys are left.",
                    second: "Rotate to the next tree to keep signing.",
                }
            ),
            ("calibrate-1", Screen::Calibrate { step: 0 }),
            ("calibrate-2", Screen::Calibrate { step: 1 }),
            ("calibrate-3", Screen::Calibrate { step: 2 }),
        ]
    }
}

impl Default for Samples {
    fn default() -> Self {
        Self::new()
    }
}
