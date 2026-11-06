//! Stealth payments: anyone can pay a published address, and only its owner can find or spend them.

mod common;

use std::sync::OnceLock;

use common::{SimFlash, TestRng};
use qday_mldsa::blob::key_part;
use qday_mldsa::stealth::derive_public;
use qday_mldsa::verify;
use qday_wallet::counter::FlashCounter;
use qday_wallet::flash::layout;
use qday_wallet::keystore::KeyStore;
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::{Attempt, Code, Passcode, Proof};
use qday_wallet::settings::{Scheme, Settings};
use qday_wallet::stealth::{
    Announcement, CIPHERTEXT_BYTES, META_ADDRESS_BYTES, MetaAddress, StealthSpend, finds, key_for,
    meta_address_for, open as open_payment, pay_to, spend_hash, view_seed_for,
};
use qday_wallet::wallet::{MAIN_ACCOUNT, Wallet, WalletError};
use qday_xmss::{Hash, keccak256};

const CODE: Code = [1, 2, 3, 4, 5, 6];
const POOL: [u8; 20] = [0x90; 20];
const TO: [u8; 20] = [0xBE; 20];
const FEE: u128 = 1_000;
const RANDOM: [u8; 64] = [0x55; 64];

type TestWallet = Wallet<SimFlash, FlashCounter<SimFlash>>;

fn open(flash: &SimFlash) -> TestWallet {
    Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        FlashCounter::new(flash.clone(), layout::COUNTER),
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        0,
    )
    .expect("the flash is healthy")
}

fn base_image() -> &'static Vec<u8> {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let flash = SimFlash::new();
        let mut wallet = open(&flash);
        wallet
            .create(&mut TestRng(7), |_| {})
            .expect("the flash is healthy");
        wallet
            .passcode()
            .create(&CODE, &mut TestRng(8))
            .expect("the flash is healthy");
        flash.image()
    })
}

fn fresh() -> SimFlash {
    let flash = SimFlash::new();
    flash.poke(0, base_image());
    flash
}

fn proof(wallet: &mut TestWallet) -> Proof {
    match wallet
        .enter_passcode(&CODE, 0)
        .expect("the flash is healthy")
    {
        Attempt::Correct(proof) => proof,
        other => panic!("the code is right: {other:?}"),
    }
}

fn address(wallet: &mut TestWallet) -> MetaAddress {
    wallet.meta_address(MAIN_ACCOUNT).expect("it derives")
}

fn view(wallet: &mut TestWallet) -> Hash {
    let proof = proof(wallet);
    wallet.view_seed(MAIN_ACCOUNT, proof).expect("it derives")
}

fn pay(meta: &MetaAddress, random: &[u8; 64]) -> Announcement {
    pay_to(meta, random).expect("the address is well formed")
}

fn spend(announcement: &Announcement) -> StealthSpend {
    StealthSpend {
        ciphertext: announcement.ciphertext,
        key_hash: announcement.key_hash,
        pool: POOL,
        to: TO,
        fee: FEE,
    }
}

fn sign(wallet: &mut TestWallet, spend: &StealthSpend) -> Result<Vec<u8>, WalletError> {
    let proof = proof(wallet);
    wallet
        .sign_stealth(spend, MAIN_ACCOUNT, proof, &mut TestRng(9))
        .map(|signature| signature.to_vec())
}

#[test]
fn an_address_is_the_documented_size() {
    let flash = fresh();
    assert_eq!(address(&mut open(&flash)).len(), META_ADDRESS_BYTES);
    assert_eq!(META_ADDRESS_BYTES, 4160);
}

#[test]
fn asking_for_an_address_writes_nothing() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let before = flash.operations();
    address(&mut wallet);
    assert_eq!(flash.operations(), before);
}

#[test]
fn an_address_is_stable_and_differs_by_account() {
    let flash = fresh();
    let mut wallet = open(&flash);
    assert_eq!(address(&mut wallet), address(&mut wallet));
    assert_ne!(
        wallet.meta_address(0).expect("it derives"),
        wallet.meta_address(1).expect("it derives")
    );
}

#[test]
fn every_payment_gets_its_own_key_and_ciphertext() {
    let flash = fresh();
    let meta = address(&mut open(&flash));
    let (first, second) = (pay(&meta, &[1; 64]), pay(&meta, &[2; 64]));
    assert_ne!(first.key_hash, second.key_hash);
    assert_ne!(first.ciphertext, second.ciphertext);
    assert_eq!(first.ciphertext.len(), CIPHERTEXT_BYTES);
}

#[test]
fn the_owner_finds_a_payment_with_only_the_view_seed() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let view = view(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let found = open_payment(&view, &meta, &announcement.ciphertext).expect("the address is good");
    assert_eq!(found.key_hash, announcement.key_hash);
    assert_eq!(found.view_tag, announcement.view_tag);
}

#[test]
fn someone_else_doesnt_find_it() {
    let (mine, theirs) = (fresh(), SimFlash::new());
    let mut wallet = open(&mine);
    let meta = address(&mut wallet);
    let announcement = pay(&meta, &RANDOM);

    let mut other = open(&theirs);
    other
        .create(&mut TestRng(100), |_| {})
        .expect("the flash is healthy");
    other
        .passcode()
        .create(&CODE, &mut TestRng(101))
        .expect("the flash is healthy");
    let their_meta = address(&mut other);
    let their_view = view(&mut other);
    let found = open_payment(&their_view, &their_meta, &announcement.ciphertext).expect("it opens");
    assert_ne!(found.key_hash, announcement.key_hash);
}

#[test]
fn a_changed_ciphertext_doesnt_match() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let view = view(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    for at in [10, 1100] {
        let mut changed = announcement.ciphertext;
        changed[at] ^= 1;
        let found = open_payment(&view, &meta, &changed).expect("it opens");
        assert_ne!(found.key_hash, announcement.key_hash, "byte {at}");
    }
}

#[test]
fn a_malformed_address_is_refused() {
    let flash = fresh();
    let mut meta = address(&mut open(&flash));
    meta[META_ADDRESS_BYTES - 1] = 0xFF;
    meta[META_ADDRESS_BYTES - 2] = 0xFF;
    meta[META_ADDRESS_BYTES - 3] = 0xFF;
    assert!(pay_to(&meta, &RANDOM).is_none());
}

#[test]
fn the_key_for_a_payment_hashes_to_what_was_announced() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let view = view(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let key = key_for(&view, &meta, &announcement.ciphertext).expect("it derives");
    assert_eq!(keccak256(&[&key]), announcement.key_hash);
}

#[test]
fn a_spend_is_signed_for_the_real_key() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let view = view(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let signature = sign(&mut wallet, &spend(&announcement)).expect("it signs");

    let found = open_payment(&view, &meta, &announcement.ciphertext).expect("it opens");
    let t: &[u8; 2944] = meta[1216..].try_into().expect("the sizes match");
    let public = derive_public(t, &found.secret).expect("the address is good");
    assert_eq!(keccak256(&[&key_part(&public)]), announcement.key_hash);
    let message = spend_hash(&POOL, &announcement.key_hash, &TO, FEE);
    let signature: [u8; 2420] = signature.try_into().expect("whole");
    assert!(verify(&public, &message, &signature));
}

#[test]
fn a_ciphertext_that_isnt_for_this_key_hash_is_refused() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let mut wrong = spend(&announcement);
    wrong.key_hash = [0x77; 32];
    assert_eq!(sign(&mut wallet, &wrong), Err(WalletError::BadRequest));
}

#[test]
fn a_zero_recipient_or_pool_is_refused() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let mut zero = spend(&announcement);
    zero.to = [0; 20];
    assert_eq!(sign(&mut wallet, &zero), Err(WalletError::BadRequest));
    let mut zero = spend(&announcement);
    zero.pool = [0; 20];
    assert_eq!(sign(&mut wallet, &zero), Err(WalletError::BadRequest));
}

#[test]
fn signing_uses_fresh_randomness() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let spend = spend(&announcement);
    let first_proof = proof(&mut wallet);
    let first = wallet.sign_stealth(&spend, MAIN_ACCOUNT, first_proof, &mut TestRng(1));
    let second_proof = proof(&mut wallet);
    let second = wallet.sign_stealth(&spend, MAIN_ACCOUNT, second_proof, &mut TestRng(2));
    assert_ne!(first.expect("it signs"), second.expect("it signs"));
}

/// Pinned, so a change to any tag or to what's bound can't pass unnoticed.
#[test]
fn the_derivation_is_pinned() {
    let seed = [0x42; 32];
    let meta = meta_address_for(&seed, 0);
    let announcement = pay(&meta, &RANDOM);
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .fold(String::new(), |out, byte| format!("{out}{byte:02x}"))
    };
    assert_eq!(
        hex(&keccak256(&[&meta])),
        "a0b66f6381d976d4fed69390b2af6e331ed6062f73a38d54155e929089d58a8d"
    );
    assert_eq!(
        hex(&view_seed_for(&seed, 0)),
        "40ac6871ae3f65be259acd1d2885a5b98afca7685da2769a409beb557806677f"
    );
    assert_eq!(
        hex(&announcement.key_hash),
        "edbdc47b4446abdc6d3277bbef91ccea335b8d81f1cb4c6c273694b91ac173bb"
    );
    assert_eq!(announcement.view_tag, 253);
}

#[test]
fn a_stealth_key_can_sign_again() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    assert!(sign(&mut wallet, &spend(&announcement)).is_ok());
    let mut other = spend(&announcement);
    other.fee = FEE + 1;
    assert!(sign(&mut wallet, &other).is_ok());
}

#[test]
fn spending_writes_nothing() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let proof = proof(&mut wallet);
    let before = flash.operations();
    wallet
        .sign_stealth(&spend(&announcement), MAIN_ACCOUNT, proof, &mut TestRng(9))
        .expect("it signs");
    assert_eq!(flash.operations(), before);
}

#[test]
fn it_works_in_either_scheme() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let announcement = pay(&meta, &RANDOM);
    let proof_now = proof(&mut wallet);
    wallet
        .set_scheme(Scheme::MlDsa44, proof_now)
        .expect("it switches");
    assert_eq!(address(&mut wallet), meta);
    assert!(sign(&mut wallet, &spend(&announcement)).is_ok());
}

#[test]
fn the_view_seed_isnt_the_master_seed() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let proof_now = proof(&mut wallet);
    let seed = wallet.seed(proof_now).expect("it reads");
    assert_ne!(view(&mut wallet), seed);
    assert_eq!(view(&mut wallet), view_seed_for(&seed, MAIN_ACCOUNT));
}

#[test]
fn restoring_the_same_seed_gives_the_same_address() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let meta = address(&mut wallet);
    let proof_now = proof(&mut wallet);
    let seed = wallet.seed(proof_now).expect("it reads");

    let other = SimFlash::new();
    let mut restored = open(&other);
    restored
        .create(&mut TestRng(100), |_| {})
        .expect("the flash is healthy");
    restored
        .passcode()
        .create(&CODE, &mut TestRng(101))
        .expect("the flash is healthy");
    let proof_now = proof(&mut restored);
    restored
        .restore(&seed, 0, 0, proof_now, |_| {})
        .expect("it restores");
    assert_eq!(address(&mut restored), meta);
}

#[test]
fn the_owner_finds_an_announcement() {
    let seed = [0x5E; 32];
    let meta = meta_address_for(&seed, 0);
    let paid = pay_to(&meta, &RANDOM).expect("the address is good");
    assert_eq!(finds(&view_seed_for(&seed, 0), &meta, &paid), Some(true));
}

#[test]
fn someone_else_finds_nothing() {
    let meta = meta_address_for(&[0x5E; 32], 0);
    let paid = pay_to(&meta, &RANDOM).expect("the address is good");
    let other = [0x77; 32];
    let found = finds(
        &view_seed_for(&other, 0),
        &meta_address_for(&other, 0),
        &paid,
    );
    assert_eq!(found, Some(false));
}

#[test]
fn a_wrong_tag_or_key_hash_is_not_found() {
    let seed = [0x5E; 32];
    let view = view_seed_for(&seed, 0);
    let meta = meta_address_for(&seed, 0);
    let paid = pay_to(&meta, &RANDOM).expect("the address is good");
    let mut wrong_tag = paid;
    wrong_tag.view_tag ^= 1;
    assert_eq!(finds(&view, &meta, &wrong_tag), Some(false));
    let mut wrong_hash = paid;
    wrong_hash.key_hash[0] ^= 1;
    assert_eq!(finds(&view, &meta, &wrong_hash), Some(false));
}
