//! The chip's security fuses, as the device reports them.

use qday_wallet::lockdown::{Check, Fuses, HMAC_UP, Level};
use qday_wallet::protocol::{
    Request, Response, parse_request, parse_response, request_line, response_line,
};

fn locked() -> Fuses {
    Fuses {
        crypt_count: 0b111,
        secure_boot: true,
        pad_jtag_off: true,
        usb_jtag_off: true,
        soft_jtag: 0b111,
        direct_boot_off: true,
        manual_encrypt_off: true,
        download_icache_off: true,
        download_dcache_off: true,
        secure_download: true,
        key_purposes: [4, 9, HMAC_UP, 0, 0, 0],
    }
}

#[test]
fn a_new_chip_is_open() {
    assert_eq!(Fuses::default().level(), Level::Open);
    assert_eq!(Fuses::default().missing().count(), Check::ALL.len());
}

#[test]
fn a_fully_burned_chip_is_locked() {
    assert_eq!(locked().level(), Level::Locked);
    assert_eq!(locked().missing().count(), 0);
}

#[test]
fn any_one_gap_leaves_the_chip_partly_locked() {
    let gaps: [fn(&mut Fuses); 10] = [
        |f| f.crypt_count = 0,
        |f| f.secure_boot = false,
        |f| f.pad_jtag_off = false,
        |f| f.usb_jtag_off = false,
        |f| f.soft_jtag = 0,
        |f| f.direct_boot_off = false,
        |f| f.manual_encrypt_off = false,
        |f| f.download_icache_off = false,
        |f| f.secure_download = false,
        |f| f.key_purposes = [4, 9, 0, 0, 0, 0],
    ];
    for gap in gaps {
        let mut fuses = locked();
        gap(&mut fuses);
        assert_eq!(fuses.level(), Level::Partial);
        assert_eq!(fuses.missing().count(), 1);
    }
}

#[test]
fn flash_encryption_is_on_only_for_an_odd_count_of_bits() {
    for (count, on) in [(0, false), (1, true), (0b11, false), (0b111, true)] {
        let fuses = Fuses {
            crypt_count: count,
            ..Fuses::default()
        };
        assert_eq!(fuses.flash_encrypted(), on);
    }
}

#[test]
fn release_mode_needs_every_count_bit_so_it_cant_be_turned_off() {
    let fuses = Fuses {
        crypt_count: 1,
        ..locked()
    };
    assert_eq!(
        fuses.missing().collect::<Vec<_>>(),
        [Check::FlashEncryption]
    );
}

#[test]
fn soft_jtag_is_off_only_for_an_odd_count_of_bits() {
    let fuses = Fuses {
        soft_jtag: 0b11,
        ..locked()
    };
    assert_eq!(fuses.missing().collect::<Vec<_>>(), [Check::SoftJtag]);
}

#[test]
fn the_seal_key_is_found_in_any_block() {
    assert_eq!(Fuses::default().seal_key(), None);
    for block in 0..6_u8 {
        let mut fuses = Fuses::default();
        fuses.key_purposes[usize::from(block)] = HMAC_UP;
        assert_eq!(fuses.seal_key(), Some(block));
    }
}

#[test]
fn fuses_survive_the_wire() {
    let fuses = locked();
    assert_eq!(Fuses::from_bytes(&fuses.to_bytes()), fuses);
    let some = Fuses {
        crypt_count: 1,
        usb_jtag_off: true,
        key_purposes: [1, 2, 3, 4, 5, 6],
        ..Fuses::default()
    };
    assert_eq!(Fuses::from_bytes(&some.to_bytes()), some);
}

#[test]
fn every_check_has_a_name() {
    for check in Check::ALL {
        assert!(!check.name().is_empty());
    }
}

#[test]
fn the_lockdown_request_matches_the_protocol() {
    assert_eq!(request_line(&Request::Lockdown), "QDAY1 LOCKDOWN\n");
    assert_eq!(parse_request("QDAY1 LOCKDOWN"), Some(Ok(Request::Lockdown)));
}

#[test]
fn the_lockdown_response_round_trips() {
    for (fuses, sealed) in [(locked(), true), (Fuses::default(), false)] {
        let response = Response::Lockdown { fuses, sealed };
        let line = response_line(&response);
        assert_eq!(parse_response(line.trim_end()), Some(response));
    }
}

#[test]
fn a_new_chip_reports_all_zeros() {
    let response = Response::Lockdown {
        fuses: Fuses::default(),
        sealed: false,
    };
    assert_eq!(
        response_line(&response),
        "QDAY1 LOCKDOWN 0x000000000000000000 0\n"
    );
}

#[test]
fn bad_lockdown_responses_are_refused() {
    for line in [
        "QDAY1 LOCKDOWN",
        "QDAY1 LOCKDOWN 0x00 0",
        "QDAY1 LOCKDOWN 0x000000000000000000",
        "QDAY1 LOCKDOWN 0x000000000000000000 2",
        "QDAY1 LOCKDOWN 0x000000000000000000 0 extra",
    ] {
        assert_eq!(parse_response(line), None, "{line}");
    }
}
