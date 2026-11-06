//! The blob must match the reference `prepare.py` byte for byte.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use qday_mldsa::blob::{
    BLOB_BYTES, BlobError, check_blob, is_degenerate, prepare, prepare_checked,
};
use qday_mldsa::{KeySeed, PUBLIC_KEY_BYTES, PublicKey, Signer, StandardSigner};
use qday_xmss::{Hash, keccak256};

fn seed() -> Hash {
    core::array::from_fn(|i| if i % 2 == 0 { 0x5e } else { 0xed })
}

fn key(account: u32) -> PublicKey {
    StandardSigner::from_key_seed(&KeySeed::derive(&seed(), account)).public_key()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
        out
    })
}

/// What the reference program writes for a key.
fn reference(public: &[u8]) -> String {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/mldsa/prepare/prepare.py"
    );
    assert!(Path::new(script).exists(), "prepare.py is vendored");
    let mut child = Command::new("python3")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("python3 runs");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(hex(public).as_bytes())
        .expect("the key is written");
    let output = child.wait_with_output().expect("prepare.py finishes");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("the output is text")
}

#[test]
fn the_blob_matches_prepare_py_for_derived_keys() {
    for account in [0, 1, 7] {
        let public = key(account);
        assert_eq!(
            hex(&prepare(&public)),
            reference(&public),
            "account {account}"
        );
    }
}

#[test]
fn the_blob_matches_prepare_py_for_the_key_made_on_the_board() {
    let public = StandardSigner::from_key_seed(&KeySeed::from_bytes([0x51; 32])).public_key();
    assert_eq!(hex(&prepare(&public)), reference(&public));
}

#[test]
fn the_blob_matches_prepare_py_for_extreme_keys() {
    for fill in [0x00, 0xFF, 0xAA] {
        let public = [fill; PUBLIC_KEY_BYTES];
        assert_eq!(hex(&prepare(&public)), reference(&public), "fill {fill:#x}");
    }
}

#[test]
fn a_blob_has_the_right_shape() {
    let blob = prepare(&key(0));
    assert_eq!(blob.len(), BLOB_BYTES);
    assert_eq!(blob[0], 0x00, "EIP-3541 needs this prefix");
}

#[test]
fn the_blob_is_pinned_by_its_hash() {
    // From prepare.py, so this still guards the port on a machine without Python.
    let want = reference(&key(0));
    let bytes: Vec<u8> = (0..want.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&want[at..at + 2], 16).expect("the output is hex"))
        .collect();
    assert_eq!(keccak256(&[&prepare(&key(0))]), keccak256(&[&bytes]));
}

/// Encodes 1,024 ten-bit coefficients after a 32-byte rho.
fn key_with_coefficients(coefficient: impl Fn(usize) -> u16) -> PublicKey {
    let mut public = [0; PUBLIC_KEY_BYTES];
    public[..32].fill(0x42);
    for index in 0..1024 {
        let value = coefficient(index);
        for bit in 0..10 {
            if value >> bit & 1 == 1 {
                let position = index * 10 + bit;
                public[32 + position / 8] |= 1 << (position % 8);
            }
        }
    }
    public
}

#[test]
fn a_zero_key_is_degenerate() {
    assert!(is_degenerate(&key_with_coefficients(|_| 0)));
}

#[test]
fn the_maximal_key_is_degenerate() {
    assert!(is_degenerate(&key_with_coefficients(|_| 1023)));
    assert!(is_degenerate(&[0xFF; PUBLIC_KEY_BYTES]));
}

#[test]
fn near_zero_keys_are_degenerate() {
    let values = [0, 1, 2];
    assert!(is_degenerate(&key_with_coefficients(
        |index| values[index % 3]
    )));
}

#[test]
fn the_floor_is_at_half_the_coefficients() {
    // A coefficient of 512 has a large centred lift, and 0 has none.
    let at = |large: usize| key_with_coefficients(|index| if index < large { 512 } else { 0 });
    assert!(is_degenerate(&at(511)));
    assert!(!is_degenerate(&at(512)));
}

#[test]
fn real_keys_are_not_degenerate() {
    for account in 0..5 {
        assert!(!is_degenerate(&key(account)));
    }
}

#[test]
fn prepare_checked_gives_the_blob_for_a_real_key() {
    assert_eq!(prepare_checked(&key(0)), Ok(prepare(&key(0))));
}

#[test]
fn a_key_of_the_wrong_length_is_refused_not_truncated() {
    let public = key(0);
    let mut longer = public.to_vec();
    longer.push(0);
    assert_eq!(prepare_checked(&longer), Err(BlobError::WrongLength));
    assert_eq!(
        prepare_checked(&public[..1311]),
        Err(BlobError::WrongLength)
    );
    assert_eq!(prepare_checked(&[]), Err(BlobError::WrongLength));
}

#[test]
fn a_degenerate_key_is_refused() {
    let degenerate = key_with_coefficients(|_| 0);
    assert_eq!(prepare_checked(&degenerate), Err(BlobError::Degenerate));
}

#[test]
fn the_right_blob_passes_the_check() {
    let public = key(0);
    assert_eq!(check_blob(&public, &prepare(&public)), Ok(()));
}

#[test]
fn any_changed_byte_fails_the_check() {
    let public = key(0);
    let blob = prepare(&public);
    for offset in [0, 1, 64, 65, 4160, 4161, 20_544] {
        let mut changed = blob.clone();
        changed[offset] ^= 1;
        assert_eq!(
            check_blob(&public, &changed),
            Err(BlobError::Mismatch),
            "byte {offset}"
        );
    }
}

#[test]
fn another_keys_blob_fails_the_check() {
    assert_eq!(
        check_blob(&key(0), &prepare(&key(1))),
        Err(BlobError::Mismatch)
    );
}

#[test]
fn a_short_blob_fails_the_check() {
    let public = key(0);
    let blob = prepare(&public);
    assert_eq!(
        check_blob(&public, &blob[..BLOB_BYTES - 1]),
        Err(BlobError::Mismatch)
    );
}

#[test]
fn the_check_refuses_a_degenerate_key_even_with_its_honest_blob() {
    let degenerate = key_with_coefficients(|_| 0);
    assert_eq!(
        check_blob(&degenerate, &prepare(&degenerate)),
        Err(BlobError::Degenerate)
    );
}
