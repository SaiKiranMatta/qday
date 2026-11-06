//! One-time ML-DSA keys that a sender can derive and only the recipient can sign for. The real
//! verifier is the judge of every signature here.

use qday_mldsa::blob::{is_degenerate, key_part, prepare};
use qday_mldsa::stealth::{Master, PUBLIC_T_BYTES, derive_public, shared_rho};
use qday_mldsa::{Signer, verify};

const SEED: [u8; 32] = [0x42; 32];
const MESSAGE: [u8; 32] = [0x11; 32];
const RND: [u8; 32] = [0x22; 32];

fn secret(n: u8) -> [u8; 64] {
    [n; 64]
}

#[test]
fn the_published_key_is_the_documented_size() {
    assert_eq!(Master::from_seed(&SEED).public().len(), PUBLIC_T_BYTES);
    assert_eq!(PUBLIC_T_BYTES, 2944);
}

#[test]
fn a_master_comes_from_its_seed() {
    assert_eq!(
        Master::from_seed(&SEED).public(),
        Master::from_seed(&SEED).public()
    );
    assert_ne!(
        Master::from_seed(&SEED).public(),
        Master::from_seed(&[0x43; 32]).public()
    );
}

#[test]
fn sender_and_recipient_reach_the_same_key() {
    let master = Master::from_seed(&SEED);
    for n in 0..5 {
        let sender = derive_public(&master.public(), &secret(n)).expect("the key is well formed");
        assert_eq!(sender, master.one_time(&secret(n)).public_key(), "{n}");
    }
}

#[test]
fn every_one_time_key_shares_the_matrix() {
    let master = Master::from_seed(&SEED);
    for n in 0..3 {
        let key = master.one_time(&secret(n)).public_key();
        assert_eq!(key[..32], shared_rho());
    }
}

#[test]
fn one_time_keys_differ() {
    let master = Master::from_seed(&SEED);
    assert_ne!(
        master.one_time(&secret(1)).public_key(),
        master.one_time(&secret(2)).public_key()
    );
}

#[test]
fn the_standard_verifier_accepts_a_one_time_signature() {
    let signer = Master::from_seed(&SEED).one_time(&secret(1));
    let signature = signer.sign(&MESSAGE, &RND).expect("it signs");
    assert!(verify(&signer.public_key(), &MESSAGE, &signature));
}

#[test]
fn many_signatures_all_verify() {
    let master = Master::from_seed(&SEED);
    for n in 0..12 {
        let signer = master.one_time(&secret(n));
        let message = [n; 32];
        let rnd = [n.wrapping_add(100); 32];
        let signature = signer.sign_unchecked(&message, &rnd).expect("it signs");
        assert!(verify(&signer.public_key(), &message, &signature), "{n}");
    }
}

/// The doubled bound is what stops a signature leaking a wider secret, and the verifier can't see
/// it: a signature from a signer with the normal bound would still verify. So check it directly.
#[test]
fn signatures_stay_inside_the_doubled_bound() {
    const GAMMA1: i64 = 1 << 17;
    const WIDE_BETA: i64 = 156;
    let master = Master::from_seed(&SEED);
    for n in 0..24 {
        let signer = master.one_time(&secret(n));
        let signature = signer.sign_unchecked(&[n; 32], &[n; 32]).expect("it signs");
        for index in 0..1024 {
            let bit = 32 * 8 + index * 18;
            let mut packed = 0_i64;
            for offset in 0..18 {
                let at = bit + offset;
                packed |= i64::from(signature[at / 8] >> (at % 8) & 1) << offset;
            }
            assert!((GAMMA1 - packed).abs() < GAMMA1 - WIDE_BETA, "{n} {index}");
        }
    }
}

#[test]
fn another_message_is_refused() {
    let signer = Master::from_seed(&SEED).one_time(&secret(1));
    let signature = signer.sign(&MESSAGE, &RND).expect("it signs");
    assert!(!verify(&signer.public_key(), &[0x33; 32], &signature));
}

#[test]
fn another_payments_key_is_refused() {
    let master = Master::from_seed(&SEED);
    let signature = master
        .one_time(&secret(1))
        .sign(&MESSAGE, &RND)
        .expect("it signs");
    assert!(!verify(
        &master.one_time(&secret(2)).public_key(),
        &MESSAGE,
        &signature
    ));
}

#[test]
fn another_master_cant_sign_for_the_key() {
    let key = Master::from_seed(&SEED).one_time(&secret(1)).public_key();
    let signature = Master::from_seed(&[0x43; 32])
        .one_time(&secret(1))
        .sign(&MESSAGE, &RND)
        .expect("it signs for its own key");
    assert!(!verify(&key, &MESSAGE, &signature));
}

#[test]
fn signing_is_hedged() {
    let signer = Master::from_seed(&SEED).one_time(&secret(1));
    assert_ne!(
        signer.sign(&MESSAGE, &[1; 32]).expect("it signs"),
        signer.sign(&MESSAGE, &[2; 32]).expect("it signs")
    );
}

#[test]
fn a_one_time_key_isnt_degenerate() {
    let key = Master::from_seed(&SEED).one_time(&secret(1)).public_key();
    assert!(!is_degenerate(&key));
}

#[test]
fn a_malformed_published_key_is_refused() {
    let mut t = Master::from_seed(&SEED).public();
    t[..3].fill(0xFF);
    assert_eq!(derive_public(&t, &secret(1)), None);
}

#[test]
fn the_key_part_is_the_blob_without_the_matrix() {
    let key = Master::from_seed(&SEED).one_time(&secret(1)).public_key();
    assert_eq!(key_part(&key), prepare(&key)[1..4161]);
}
