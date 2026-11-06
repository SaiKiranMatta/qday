//! A one-time key on its own, with no tree, for payments to a fresh key.

use qday_xmss::{KeySet, ONE_TIME_SIG_BYTES, keccak256, verify_one_time};

fn keys(number: u32) -> KeySet {
    KeySet::derive(&[0x51; 32], number)
}

const MESSAGE: [u8; 32] = [0x11; 32];

#[test]
fn a_signature_checks_against_its_leaf() {
    let keys = keys(1);
    let signature = keys.sign_one_time(&MESSAGE);
    assert!(verify_one_time(
        &keys.leaf(0),
        keys.pub_seed(),
        &MESSAGE,
        &signature
    ));
}

#[test]
fn a_signature_is_the_documented_size() {
    assert_eq!(ONE_TIME_SIG_BYTES, 2176);
}

#[test]
fn another_message_is_refused() {
    let keys = keys(2);
    let signature = keys.sign_one_time(&MESSAGE);
    assert!(!verify_one_time(
        &keys.leaf(0),
        keys.pub_seed(),
        &[0x22; 32],
        &signature
    ));
}

#[test]
fn another_leaf_is_refused() {
    let (first, second) = (keys(3), keys(4));
    let signature = first.sign_one_time(&MESSAGE);
    assert!(!verify_one_time(
        &second.leaf(0),
        first.pub_seed(),
        &MESSAGE,
        &signature
    ));
}

#[test]
fn another_public_seed_is_refused() {
    let (first, second) = (keys(5), keys(6));
    let signature = first.sign_one_time(&MESSAGE);
    assert!(!verify_one_time(
        &first.leaf(0),
        second.pub_seed(),
        &MESSAGE,
        &signature
    ));
}

#[test]
fn every_part_of_a_signature_matters() {
    let keys = keys(7);
    let good = keys.sign_one_time(&MESSAGE);
    for at in [0, 31, 32, 1000, ONE_TIME_SIG_BYTES - 1] {
        let mut bad = good;
        bad[at] ^= 1;
        assert!(
            !verify_one_time(&keys.leaf(0), keys.pub_seed(), &MESSAGE, &bad),
            "byte {at}"
        );
    }
}

#[test]
fn the_wrong_length_is_refused() {
    let keys = keys(8);
    let signature = keys.sign_one_time(&MESSAGE);
    for length in [0, ONE_TIME_SIG_BYTES - 1] {
        assert!(!verify_one_time(
            &keys.leaf(0),
            keys.pub_seed(),
            &MESSAGE,
            &signature[..length]
        ));
    }
    let mut longer = signature.to_vec();
    longer.push(0);
    assert!(!verify_one_time(
        &keys.leaf(0),
        keys.pub_seed(),
        &MESSAGE,
        &longer
    ));
}

#[test]
fn the_same_message_signs_the_same() {
    let keys = keys(9);
    assert_eq!(keys.sign_one_time(&MESSAGE), keys.sign_one_time(&MESSAGE));
}

#[test]
fn keys_from_different_numbers_share_nothing_public() {
    let (first, second) = (keys(10), keys(11));
    assert_ne!(first.leaf(0), second.leaf(0));
    assert_ne!(first.pub_seed(), second.pub_seed());
}

/// Pinned, so a change to the format can't pass unnoticed by the contract.
#[test]
fn the_format_is_pinned() {
    let keys = keys(12);
    let signature = keys.sign_one_time(&MESSAGE);
    let hash = keccak256(&[&signature]);
    assert_eq!(
        hash.iter()
            .fold(String::new(), |out, byte| format!("{out}{byte:02x}")),
        "ac8be5fee79b59877fbe916fecb2054c97ff394c0c3c31f2311029ee682971bb"
    );
}
