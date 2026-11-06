//! Signing and verifying, including the published signature vector.

mod common;

use common::{SEED, hash, tree0};
use qday_xmss::{KeySet, LEAVES, PublicKey, SIG_BYTES, SignError, Tree, keccak256, verify};

const SIGNATURE_HASH: &str = "0x8937def506577209cdaaff87612f54af2e417d5502132267f358b5daa32ece1f";
const MESSAGE: [u8; 32] = [0x11; 32];

fn public_key() -> PublicKey {
    PublicKey {
        root: tree0().root(),
        pub_seed: *KeySet::derive(&SEED, 0).pub_seed(),
    }
}

fn sign(index: u32, message: &[u8; 32]) -> [u8; SIG_BYTES] {
    KeySet::derive(&SEED, 0)
        .sign(&tree0().root(), index, message, &tree0().auth_path(index))
        .expect("the index is in range")
}

#[test]
fn signature_matches_the_vector() {
    let signature = sign(5, &MESSAGE);
    assert_eq!(signature.len(), 2500);
    assert_eq!(keccak256(&[&signature]), hash(SIGNATURE_HASH));
}

#[test]
fn encoding_starts_with_the_index_and_ends_with_the_auth_path() {
    let signature = sign(0x0102, &MESSAGE);
    assert_eq!(signature[..4], [0, 0, 1, 2]);
    let path = tree0().auth_path(0x0102);
    assert_eq!(signature[SIG_BYTES - 320..], *path.as_flattened());
}

#[test]
fn first_last_and_middle_keys_all_verify() {
    for index in [0, 1, 5, 512, 1007, 1008, LEAVES - 1] {
        assert!(
            verify(&public_key(), &MESSAGE, &sign(index, &MESSAGE)),
            "key {index}"
        );
    }
}

#[test]
fn another_message_is_refused() {
    assert!(!verify(&public_key(), &[0x12; 32], &sign(5, &MESSAGE)));
}

#[test]
fn changing_any_byte_of_a_signature_is_refused() {
    let key = public_key();
    let signature = sign(5, &MESSAGE);
    for offset in 0..SIG_BYTES {
        let mut tampered = signature;
        tampered[offset] ^= 0x01;
        assert!(!verify(&key, &MESSAGE, &tampered), "byte {offset}");
    }
}

#[test]
fn another_root_is_refused() {
    let mut key = public_key();
    key.root[31] ^= 1;
    assert!(!verify(&key, &MESSAGE, &sign(5, &MESSAGE)));
}

#[test]
fn another_pub_seed_is_refused() {
    let mut key = public_key();
    key.pub_seed[0] ^= 1;
    assert!(!verify(&key, &MESSAGE, &sign(5, &MESSAGE)));
}

#[test]
fn a_signature_from_another_tree_is_refused() {
    let keys = KeySet::derive(&SEED, 1);
    let tree = Tree::build(&keys, |_| {});
    let signature = keys
        .sign(&tree.root(), 5, &MESSAGE, &tree.auth_path(5))
        .expect("the index is in range");
    assert!(!verify(&public_key(), &MESSAGE, &signature));
}

#[test]
fn wrong_lengths_are_refused() {
    let key = public_key();
    let signature = sign(5, &MESSAGE);
    assert!(!verify(&key, &MESSAGE, &[]));
    assert!(!verify(&key, &MESSAGE, &signature[..SIG_BYTES - 1]));
    let mut longer = signature.to_vec();
    longer.push(0);
    assert!(!verify(&key, &MESSAGE, &longer));
}

#[test]
fn an_index_past_the_tree_is_refused_when_verifying() {
    let key = public_key();
    for index in [LEAVES, LEAVES + 5, u32::MAX] {
        let mut signature = sign(5, &MESSAGE);
        signature[..4].copy_from_slice(&index.to_be_bytes());
        assert!(!verify(&key, &MESSAGE, &signature), "index {index}");
    }
}

#[test]
fn an_index_past_the_tree_is_refused_when_signing() {
    let keys = KeySet::derive(&SEED, 0);
    let auth = tree0().auth_path(0);
    let result = keys.sign(&tree0().root(), LEAVES, &MESSAGE, &auth);
    assert_eq!(result, Err(SignError::IndexOutOfRange));
}

#[test]
fn signing_is_deterministic() {
    assert_eq!(sign(5, &MESSAGE), sign(5, &MESSAGE));
}
