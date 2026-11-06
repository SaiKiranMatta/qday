//! Paying a stealth note from an account: the frame carries a `pay` call the device can read back.

use qday_wallet::frame::{Action, FrameTx, MlFrameTx, StealthNote};
use qday_wallet::stealth::CIPHERTEXT_BYTES;
use qday_xmss::keccak256;

const SENDER: [u8; 20] = [0x11; 20];
const POOL: [u8; 20] = [0x90; 20];
const VALUE: u128 = 1_000_000_000_000_000;
/// The `pay` call's bytes before the ciphertext: selector and four words.
const HEAD_BYTES: usize = 132;

fn note() -> StealthNote {
    StealthNote {
        key_hash: [0xAB; 32],
        view_tag: 0x7F,
        ciphertext: vec![0xC1; CIPHERTEXT_BYTES],
    }
}

fn xmss() -> FrameTx {
    FrameTx::stealth_pay(SENDER, 3, 9, POOL, VALUE, note())
}

fn ml() -> MlFrameTx {
    MlFrameTx::stealth_pay(SENDER, 3, POOL, VALUE, note())
}

/// The third frame's call data.
fn call(tx: &FrameTx) -> Vec<u8> {
    tx.frame_views().pop().expect("three frames").data
}

/// An unsigned XMSS transaction whose third frame carries `data`, with the pool's frame limits.
fn with_data(data: Vec<u8>) -> FrameTx {
    let mut tx = xmss();
    let view = tx.frame_views().pop().expect("three frames");
    tx.action = Action::Other {
        target: Some(POOL),
        value: VALUE,
        data,
        execution: view.execution,
        state: view.state,
    };
    tx
}

#[test]
fn the_call_is_what_solidity_encodes() {
    let data = call(&xmss());
    let selector = keccak256(&[b"pay(bytes32,bytes1,bytes)"]);
    assert_eq!(data[..4], selector[..4]);
    assert_eq!(data[4..36], [0xAB; 32]);
    let mut tag = [0; 32];
    tag[0] = 0x7F;
    assert_eq!(data[36..68], tag);
    assert_eq!(u64::from_be_bytes(data[92..100].try_into().unwrap()), 0x60);
    assert_eq!(u64::from_be_bytes(data[124..132].try_into().unwrap()), 1120);
    assert_eq!(data[HEAD_BYTES..], [0xC1; CIPHERTEXT_BYTES]);
    assert_eq!(data.len(), HEAD_BYTES + CIPHERTEXT_BYTES);
}

#[test]
fn an_xmss_payment_survives_the_round_trip() {
    let tx = xmss();
    let got = FrameTx::decode_unsigned(&tx.encode(&[])).expect("it decodes");
    assert_eq!(got, tx);
    assert!(matches!(got.action, Action::StealthPay { .. }));
}

#[test]
fn an_ml_payment_survives_the_round_trip() {
    let tx = ml();
    assert_eq!(
        MlFrameTx::decode_unsigned(&tx.encode(&[])).expect("it decodes"),
        tx
    );
}

#[test]
fn an_ml_transfer_has_no_note() {
    let tx = MlFrameTx::transfer(SENDER, 3, POOL, VALUE);
    let got = MlFrameTx::decode_unsigned(&tx.encode(&[])).expect("it decodes");
    assert_eq!(got.note, None);
}

#[test]
fn the_fee_covers_both_schemes() {
    assert!(xmss().max_fee_wei() > FrameTx::transfer(SENDER, 3, 9, POOL, VALUE).max_fee_wei());
    assert!(ml().max_fee_wei() > MlFrameTx::transfer(SENDER, 3, POOL, VALUE).max_fee_wei());
}

/// Each change makes the call something other than a payment, so the device must not call it one.
#[test]
fn a_call_that_is_nearly_a_payment_is_not_one() {
    let good = call(&xmss());
    let changes: [(usize, u8); 5] = [
        (0, 0xFF),   // selector
        (37, 0x01),  // the view tag's padding
        (99, 0x40),  // the offset
        (131, 0x10), // the length
        (68, 0x01),  // the offset's high bytes
    ];
    for (at, byte) in changes {
        let mut data = good.clone();
        data[at] = byte;
        let raw = with_data(data).encode(&[]);
        let got = FrameTx::decode_unsigned(&raw).expect("an unknown call still decodes");
        assert!(
            matches!(got.action, Action::Other { .. }),
            "changing byte {at} kept it a payment"
        );
    }
}

#[test]
fn a_short_or_long_call_is_not_a_payment() {
    let good = call(&xmss());
    for data in [good[..good.len() - 1].to_vec(), [&good[..], &[0]].concat()] {
        let got = FrameTx::decode_unsigned(&with_data(data).encode(&[])).expect("it decodes");
        assert!(matches!(got.action, Action::Other { .. }));
    }
}

#[test]
fn other_frame_limits_are_not_a_payment() {
    let mut tx = with_data(call(&xmss()));
    let Action::Other { execution, .. } = &mut tx.action else {
        panic!("built as an unknown call");
    };
    *execution += 1;
    let got = FrameTx::decode_unsigned(&tx.encode(&[])).expect("it decodes");
    assert!(matches!(got.action, Action::Other { .. }));
}

#[test]
fn the_ml_decoder_refuses_anything_else_in_its_second_frame() {
    let tx = ml();
    let mut raw = tx.encode(&[]);
    let at = raw
        .windows(4)
        .position(|window| window == &call(&xmss())[..4])
        .expect("the call is in the bytes");
    raw[at] ^= 0xFF;
    assert!(MlFrameTx::decode_unsigned(&raw).is_err());
}
