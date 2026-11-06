//! Stealth payments, which anyone can make to a published address with no contact first. X-Wing
//! gives sender and recipient a shared secret, and the secret turns the recipient's published key
//! into a one-time key that only the recipient can sign for.
//!
//! The view seed finds payments and can't spend them, so it can live on a laptop. Spending needs the
//! master secret too, which stays on the device. Hiding who a payment is for rests on ML-KEM's
//! ciphertexts not revealing their recipient, which is an assumption, not a proven property.

use alloc::vec::Vec;

use qday_mldsa::Signer;
use qday_mldsa::blob::key_part;
use qday_mldsa::stealth::{
    Master, OneTimeSigner, PUBLIC_T_BYTES, PublicT, SECRET_BYTES, derive_public,
};
use qday_xmss::{Hash, keccak256};
use x_wing::{
    Decapsulate, DecapsulationKey, Decapsulator, ENCAPSULATION_KEY_SIZE, EncapsulationKey,
    KeyExport,
};

use crate::frame::Address;

/// Length of a published address: the X-Wing key, then the published `t`.
pub const META_ADDRESS_BYTES: usize = ENCAPSULATION_KEY_SIZE + PUBLIC_T_BYTES;
/// Length of an X-Wing ciphertext.
pub const CIPHERTEXT_BYTES: usize = 1120;

const VIEW_TAG: Hash = *b"QDAY-STEALTH-VIEW-V0\0\0\0\0\0\0\0\0\0\0\0\0";
const SPEND_TAG: Hash = *b"QDAY-STEALTH-SPEND-V0\0\0\0\0\0\0\0\0\0\0\0";
const SECRET_TAG: Hash = *b"QDAY-STEALTH-SECRET-V0\0\0\0\0\0\0\0\0\0\0";
const VIEW_TAG_TAG: Hash = *b"QDAY-STEALTH-VIEW-TAG-V0\0\0\0\0\0\0\0\0";

/// A published address.
pub type MetaAddress = [u8; META_ADDRESS_BYTES];
/// An X-Wing ciphertext.
pub type Ciphertext = [u8; CIPHERTEXT_BYTES];

/// What a sender posts with a payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Announcement {
    /// The ciphertext.
    pub ciphertext: Ciphertext,
    /// The hash of the payment's key, which the note is held under.
    pub key_hash: Hash,
    /// A byte that lets a scanner skip most other people's payments.
    pub view_tag: u8,
}

/// What an owner learns from opening an announcement.
pub struct Found {
    /// The payment's secret.
    pub secret: [u8; SECRET_BYTES],
    /// The hash of the payment's key.
    pub key_hash: Hash,
    /// The view tag.
    pub view_tag: u8,
}

/// A request to spend a stealth note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StealthSpend {
    /// The payment's ciphertext.
    pub ciphertext: Ciphertext,
    /// The hash of the payment's key.
    pub key_hash: Hash,
    /// The pool contract.
    pub pool: Address,
    /// Who gets the note.
    pub to: Address,
    /// What whoever submits the spend gets out of the note, in wei.
    pub fee: u128,
}

/// The seed that finds payments and can't spend them.
#[must_use]
pub fn view_seed_for(seed: &Hash, account: u32) -> Hash {
    keccak256(&[&VIEW_TAG, seed, &account.to_be_bytes()])
}

fn master_for(seed: &Hash, account: u32) -> Master {
    Master::from_seed(&keccak256(&[&SPEND_TAG, seed, &account.to_be_bytes()]))
}

/// The address to publish.
#[must_use]
pub fn meta_address_for(seed: &Hash, account: u32) -> MetaAddress {
    let view = DecapsulationKey::from(view_seed_for(seed, account));
    let mut meta = [0; META_ADDRESS_BYTES];
    meta[..ENCAPSULATION_KEY_SIZE].copy_from_slice(&view.encapsulation_key().to_bytes());
    meta[ENCAPSULATION_KEY_SIZE..].copy_from_slice(&master_for(seed, account).public());
    meta
}

fn published_t(meta: &MetaAddress) -> Option<&PublicT> {
    meta[ENCAPSULATION_KEY_SIZE..].try_into().ok()
}

/// What both sides derive from a payment. It binds the whole ciphertext and receiving key, which
/// X-Wing's own combiner doesn't.
fn payment_secret(shared: &[u8], ciphertext: &[u8], key: &[u8]) -> [u8; SECRET_BYTES] {
    let mut secret = [0; SECRET_BYTES];
    for (half, part) in (0_u8..).zip(secret.chunks_exact_mut(32)) {
        part.copy_from_slice(&keccak256(&[&SECRET_TAG, &[half], shared, ciphertext, key]));
    }
    secret
}

fn view_tag_of(secret: &[u8; SECRET_BYTES]) -> u8 {
    keccak256(&[&VIEW_TAG_TAG, secret])[0]
}

/// The key a spend gives the pool, and its hash, for a payment's secret.
fn key_of(meta: &MetaAddress, secret: &[u8; SECRET_BYTES]) -> Option<Vec<u8>> {
    Some(key_part(&derive_public(published_t(meta)?, secret)?))
}

/// Makes a payment to an address, with 64 fresh random bytes. `None` if the address is malformed.
#[must_use]
pub fn pay_to(meta: &MetaAddress, random: &[u8; 64]) -> Option<Announcement> {
    let receiving = &meta[..ENCAPSULATION_KEY_SIZE];
    let key = EncapsulationKey::try_from(receiving).ok()?;
    let (ciphertext, shared) = key.encapsulate_deterministic(random.into());
    let secret = payment_secret(&shared, &ciphertext, receiving);
    let key_hash = keccak256(&[&key_of(meta, &secret)?]);
    Some(Announcement {
        ciphertext: ciphertext.as_slice().try_into().ok()?,
        key_hash,
        view_tag: view_tag_of(&secret),
    })
}

fn secret_for(view: &Hash, ciphertext: &Ciphertext) -> Option<[u8; SECRET_BYTES]> {
    let key = DecapsulationKey::from(*view);
    let shared = key.decapsulate_slice(ciphertext).ok()?;
    Some(payment_secret(
        &shared,
        ciphertext,
        &key.encapsulation_key().to_bytes(),
    ))
}

/// Opens an announcement with a view seed. A ciphertext for someone else gives an unrelated secret
/// rather than an error, so the owner compares the key hash. `None` if the address is malformed.
#[must_use]
pub fn open(view: &Hash, meta: &MetaAddress, ciphertext: &Ciphertext) -> Option<Found> {
    let secret = secret_for(view, ciphertext)?;
    Some(Found {
        secret,
        key_hash: keccak256(&[&key_of(meta, &secret)?]),
        view_tag: view_tag_of(&secret),
    })
}

/// The key a spend gives the pool.
#[must_use]
pub fn key_for(view: &Hash, meta: &MetaAddress, ciphertext: &Ciphertext) -> Option<Vec<u8>> {
    key_of(meta, &secret_for(view, ciphertext)?)
}

/// The signer for a payment, if the ciphertext really is for `key_hash`.
#[must_use]
pub fn signer_for(
    seed: &Hash,
    account: u32,
    ciphertext: &Ciphertext,
    key_hash: &Hash,
) -> Option<OneTimeSigner> {
    let secret = secret_for(&view_seed_for(seed, account), ciphertext)?;
    let signer = master_for(seed, account).one_time(&secret);
    (keccak256(&[&key_part(&signer.public_key())]) == *key_hash).then_some(signer)
}
