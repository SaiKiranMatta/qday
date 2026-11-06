//! The published vectors, and the rules around them.

use qday_mldsa::{
    KeySeed, PUBLIC_KEY_BYTES, PublicKey, SIGNATURE_BYTES, SignError, Signature, Signer,
    StandardSigner, verify,
};
use qday_xmss::{Hash, keccak256};

const MESSAGE: &str = "0x988d78a1953235fde7cc1bcfda1b7c100d57527e1bb78ce4f94983bb8fb72804";
const RND: Hash = [0x22; 32];

fn hash(text: &str) -> Hash {
    let digits = text.strip_prefix("0x").expect("vectors start with 0x");
    assert_eq!(digits.len(), 64);
    let mut out = [0; 32];
    for (slot, pair) in out.iter_mut().zip(digits.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).expect("vectors are ASCII");
        *slot = u8::from_str_radix(pair, 16).expect("vectors are hex");
    }
    out
}

fn seed() -> Hash {
    core::array::from_fn(|i| if i % 2 == 0 { 0x5e } else { 0xed })
}

fn signer(account: u32) -> StandardSigner {
    StandardSigner::from_key_seed(&KeySeed::derive(&seed(), account))
}

fn check_account(account: u32, xi: &str, public: &str, signature: &str) {
    let key_seed = KeySeed::derive(&seed(), account);
    assert_eq!(key_seed.expose(), &hash(xi), "xi");
    let signer = StandardSigner::from_key_seed(&key_seed);
    assert_eq!(
        keccak256(&[&signer.public_key()]),
        hash(public),
        "public key"
    );
    let sig = signer.sign(&hash(MESSAGE), &RND).expect("signing works");
    assert_eq!(keccak256(&[&sig]), hash(signature), "signature");
}

#[test]
fn account_0_matches_the_vector() {
    check_account(
        0,
        "0xa04212f02a399b51ee3f04bf5d798f3b83a84e008697ecb8b4a3d7bb4426e3c6",
        "0xedf67db9472cefc65f57450fba3765b9cd7fbc125897415248ac1ddb006cfdca",
        "0x080e42ae14d538444424db1f130bdd98a8b1fbe7f9d49bf9148e8167be447756",
    );
}

#[test]
fn account_1_matches_the_vector() {
    check_account(
        1,
        "0x8dbda43ce51163b385121310c99c18a10ea9df08e21e5007ebab5e911c7ada65",
        "0x2b28b612b6d2a21ffae7723845de3a5ee110adf955d54e0e8e647e91db89d74a",
        "0x37db9f9afcce24958233408333e86b744318cb8515952eb93643274294bee732",
    );
}

#[test]
fn the_key_made_on_the_board_matches() {
    let signer = StandardSigner::from_key_seed(&KeySeed::from_bytes([0x51; 32]));
    let want = hash("0x1fad3c567bc45c812fa99f7fff68126cbf1e5f29ed514b975db69a037a3e4cd2");
    assert_eq!(keccak256(&[&signer.public_key()]), want);
}

#[test]
fn the_key_seed_follows_the_derivation() {
    let mut tag = [0; 32];
    tag[..17].copy_from_slice(b"QDAY-MLDSA-KEY-V1");
    let want = keccak256(&[&tag, &seed(), &7_u32.to_be_bytes()]);
    assert_eq!(KeySeed::derive(&seed(), 7).expose(), &want);
}

#[test]
fn sizes_match_ml_dsa_44() {
    assert_eq!(PUBLIC_KEY_BYTES, 1312);
    assert_eq!(SIGNATURE_BYTES, 2420);
}

#[test]
fn a_signature_verifies() {
    let signer = signer(0);
    let message = hash(MESSAGE);
    let sig = signer.sign(&message, &RND).expect("signing works");
    assert!(verify(&signer.public_key(), &message, &sig));
}

#[test]
fn fresh_randomness_gives_a_different_valid_signature() {
    let signer = signer(0);
    let message = hash(MESSAGE);
    let first = signer.sign(&message, &[1; 32]).expect("signing works");
    let second = signer.sign(&message, &[2; 32]).expect("signing works");
    assert_ne!(first, second);
    assert!(verify(&signer.public_key(), &message, &first));
    assert!(verify(&signer.public_key(), &message, &second));
}

#[test]
fn another_message_is_refused() {
    let signer = signer(0);
    let sig = signer.sign(&hash(MESSAGE), &RND).expect("signing works");
    assert!(!verify(&signer.public_key(), &[0; 32], &sig));
}

#[test]
fn another_account_is_refused() {
    let sig = signer(0).sign(&hash(MESSAGE), &RND).expect("signing works");
    assert!(!verify(&signer(1).public_key(), &hash(MESSAGE), &sig));
}

#[test]
fn changed_signature_bytes_are_refused() {
    let signer = signer(0);
    let message = hash(MESSAGE);
    let public = signer.public_key();
    let sig = signer.sign(&message, &RND).expect("signing works");
    for offset in (0..SIGNATURE_BYTES).step_by(61) {
        let mut tampered = sig;
        tampered[offset] ^= 0x01;
        assert!(!verify(&public, &message, &tampered), "byte {offset}");
    }
}

#[test]
fn a_changed_public_key_is_refused() {
    let signer = signer(0);
    let message = hash(MESSAGE);
    let sig = signer.sign(&message, &RND).expect("signing works");
    let mut public = signer.public_key();
    public[100] ^= 0x01;
    assert!(!verify(&public, &message, &sig));
}

#[test]
fn nonsense_never_crashes() {
    let message = hash(MESSAGE);
    assert!(!verify(
        &[0; PUBLIC_KEY_BYTES],
        &message,
        &[0; SIGNATURE_BYTES]
    ));
    assert!(!verify(
        &[0xFF; PUBLIC_KEY_BYTES],
        &message,
        &[0xFF; SIGNATURE_BYTES]
    ));
}

/// A signer whose output can be made wrong, as a fault would.
struct FaultySigner {
    inner: StandardSigner,
    flip: bool,
    give_up: bool,
}

impl Signer for FaultySigner {
    fn public_key(&self) -> PublicKey {
        self.inner.public_key()
    }

    fn sign_unchecked(&self, message: &Hash, rnd: &Hash) -> Result<Signature, SignError> {
        if self.give_up {
            return Err(SignError::Exhausted);
        }
        let mut sig = self.inner.sign_unchecked(message, rnd)?;
        if self.flip {
            sig[50] ^= 0x01;
        }
        Ok(sig)
    }
}

#[test]
fn a_faulty_signature_is_held_back() {
    let faulty = FaultySigner {
        inner: signer(0),
        flip: true,
        give_up: false,
    };
    assert_eq!(
        faulty.sign(&hash(MESSAGE), &RND),
        Err(SignError::FaultDetected)
    );
}

#[test]
fn a_custom_signer_that_works_passes_the_guard() {
    let honest = FaultySigner {
        inner: signer(0),
        flip: false,
        give_up: false,
    };
    assert!(honest.sign(&hash(MESSAGE), &RND).is_ok());
}

#[test]
fn giving_up_is_an_error_not_a_crash() {
    let stuck = FaultySigner {
        inner: signer(0),
        flip: false,
        give_up: true,
    };
    assert_eq!(stuck.sign(&hash(MESSAGE), &RND), Err(SignError::Exhausted));
}

#[test]
fn no_other_crate_depends_on_the_signing_library() {
    let crates = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    for entry in std::fs::read_dir(crates).expect("the crates folder exists") {
        let path = entry.expect("the entry reads").path();
        if path.file_name().is_some_and(|name| name == "qday-mldsa") {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).unwrap_or_default();
        assert!(!text.contains("fips204"), "{}", manifest.display());
    }
    let firmware = concat!(env!("CARGO_MANIFEST_DIR"), "/../../firmware/Cargo.toml");
    let text = std::fs::read_to_string(firmware).expect("the firmware manifest exists");
    assert!(!text.contains("fips204"));
}
