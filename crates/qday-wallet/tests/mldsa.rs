//! ML-DSA mode: it must leave the counter and the key slots alone, and XMSS must be unaffected.

mod common;

use std::sync::OnceLock;

use common::{SimFlash, TestRng};
use qday_mldsa::{KeySeed, PublicKey, Signer, StandardSigner, verify};
use qday_wallet::counter::{FlashCounter, OneWayCounter};
use qday_wallet::flash::layout;
use qday_wallet::frame::{FrameTx, MlFrameTx};
use qday_wallet::keystore::{KeyStore, StoreError};
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::{Attempt, Code, Passcode, Proof};
use qday_wallet::settings::{Scheme, Settings};
use qday_wallet::wallet::{MAIN_ACCOUNT, MlReview, Pending, Wallet, WalletError};
use qday_xmss::Hash;

const CODE: Code = [1, 2, 3, 4, 5, 6];
const WRONG: Code = [9, 9, 9, 9, 9, 9];
const SENDER: [u8; 20] = [0x11; 20];
const TO: [u8; 20] = [0x22; 20];
const LATER: u64 = 1 << 40;
/// The counter and both key slots.
const UNTOUCHED: usize = layout::MLDSA_PUBLIC as usize;

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

fn switch(wallet: &mut TestWallet, scheme: Scheme) {
    let proof = proof(wallet);
    wallet
        .set_scheme(scheme, proof)
        .expect("the flash is healthy");
}

/// A wallet flash already in ML-DSA mode.
fn ml_flash() -> SimFlash {
    let flash = fresh();
    switch(&mut open(&flash), Scheme::MlDsa44);
    flash
}

fn seed_of(wallet: &mut TestWallet) -> Hash {
    let proof = proof(wallet);
    wallet.seed(proof).expect("the flash is healthy")
}

fn derived_key(seed: &Hash, account: u32) -> PublicKey {
    StandardSigner::from_key_seed(&KeySeed::derive(seed, account)).public_key()
}

fn ml_raw() -> Vec<u8> {
    MlFrameTx::transfer(SENDER, 1, TO, 1000).encode(&[])
}

fn xmss_raw(index: u32) -> Vec<u8> {
    FrameTx::transfer(SENDER, 1, index, TO, 1000).encode(&[])
}

fn ml_review(wallet: &mut TestWallet) -> MlReview {
    match wallet.check_transaction(&ml_raw()) {
        Ok(Pending::MlDsa(review)) => review,
        other => panic!("expected an ML-DSA review, got {other:?}"),
    }
}

fn counter(flash: &SimFlash) -> u32 {
    FlashCounter::new(flash.clone(), layout::COUNTER)
        .value()
        .expect("it reads")
}

#[test]
fn the_layout_keeps_the_reserved_area_free() {
    assert_eq!(layout::MLDSA_PUBLIC, 0x24000);
    assert_eq!(
        layout::KEY_SLOTS + 2 * layout::KEY_SLOT_SIZE,
        layout::MLDSA_PUBLIC
    );
}

#[test]
fn a_current_device_stays_on_xmss() {
    assert_eq!(open(&fresh()).scheme(), Scheme::Xmss);
}

#[test]
fn a_settings_record_from_before_this_change_reads_as_xmss() {
    let flash = SimFlash::new();
    Settings::new(flash.clone())
        .set_theme(2)
        .expect("the flash is healthy");
    let mut settings = Settings::new(flash.clone());
    assert_eq!(settings.scheme(), Ok(Scheme::Xmss));
    assert_eq!(settings.theme(), Ok(2));
}

#[test]
fn the_scheme_and_the_theme_do_not_disturb_each_other() {
    let flash = SimFlash::new();
    let mut settings = Settings::new(flash.clone());
    settings.set_theme(3).expect("the flash is healthy");
    settings
        .set_scheme(Scheme::MlDsa44)
        .expect("the flash is healthy");
    assert_eq!(settings.theme(), Ok(3));
    settings.set_theme(4).expect("the flash is healthy");
    assert_eq!(settings.scheme(), Ok(Scheme::MlDsa44));
    assert_eq!(settings.theme(), Ok(4));
}

#[test]
fn the_scheme_survives_a_restart() {
    assert_eq!(open(&ml_flash()).scheme(), Scheme::MlDsa44);
}

#[test]
fn a_cut_while_saving_the_scheme_leaves_a_valid_scheme() {
    for budget in 0..3 {
        let flash = SimFlash::new();
        flash.cut_after(budget);
        let _ = Settings::new(flash.clone()).set_scheme(Scheme::MlDsa44);
        flash.power_on();
        assert!(Settings::new(flash.clone()).scheme().is_ok());
    }
}

#[test]
fn switching_stores_the_key_from_the_same_seed() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let seed = seed_of(&mut wallet);
    switch(&mut wallet, Scheme::MlDsa44);
    assert_eq!(
        wallet.ml_public_key(MAIN_ACCOUNT),
        Ok(derived_key(&seed, MAIN_ACCOUNT))
    );
}

#[test]
fn the_public_key_needs_no_passcode_after_a_restart() {
    let flash = ml_flash();
    let mut restarted = open(&flash);
    assert!(restarted.ml_public_key(MAIN_ACCOUNT).is_ok());
}

#[test]
fn the_key_is_at_its_fixed_place_in_flash() {
    let flash = ml_flash();
    let key = open(&flash)
        .ml_public_key(MAIN_ACCOUNT)
        .expect("it's stored");
    let stored = flash.peek(layout::MLDSA_PUBLIC as usize, 0x1000);
    assert!(stored.windows(key.len()).any(|window| window == key));
}

#[test]
fn before_switching_there_is_no_stored_key() {
    assert_eq!(
        open(&fresh()).ml_public_key(MAIN_ACCOUNT),
        Err(WalletError::NothingPending)
    );
}

#[test]
fn another_account_has_no_stored_key() {
    assert_eq!(
        open(&ml_flash()).ml_public_key(1),
        Err(WalletError::NothingPending)
    );
}

#[test]
fn a_damaged_key_reads_as_missing() {
    let flash = ml_flash();
    flash.poke(layout::MLDSA_PUBLIC as usize + 100, &[0x55]);
    assert_eq!(
        open(&flash).ml_public_key(MAIN_ACCOUNT),
        Err(WalletError::NothingPending)
    );
}

#[test]
fn the_key_store_round_trips_and_wipes() {
    let flash = SimFlash::new();
    let mut store = MlKeyStore::new(flash.clone());
    let key = [7; 1312];
    assert_eq!(store.load(0), Ok(None));
    store.store(0, &key).expect("the flash is healthy");
    assert_eq!(MlKeyStore::new(flash.clone()).load(0), Ok(Some(key)));
    store.wipe().expect("the flash is healthy");
    assert_eq!(store.load(0), Ok(None));
    assert!(!flash.contains(&key[..64]));
}

#[test]
fn the_key_store_reports_a_dead_flash() {
    let flash = SimFlash::new();
    let mut store = MlKeyStore::new(flash.clone());
    flash.cut_after(0);
    assert_eq!(store.store(0, &[7; 1312]), Err(StoreError));
    assert_eq!(store.load(0), Err(StoreError));
}

#[test]
fn a_cut_while_switching_keeps_a_working_scheme() {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = fresh();
        let mut wallet = open(&flash);
        let proof = proof(&mut wallet);
        let before = flash.operations();
        flash.cut_after(budget);
        let result = wallet.set_scheme(Scheme::MlDsa44, proof);
        let was_cut = flash.was_cut();
        flash.power_on();

        let mut restarted = open(&flash);
        if !was_cut {
            assert_eq!(result, Ok(()));
            assert!(flash.operations() > before);
            assert_eq!(restarted.scheme(), Scheme::MlDsa44);
            break;
        }
        cuts += 1;
        assert!(result.is_err());
        if restarted.scheme() == Scheme::MlDsa44 {
            assert!(
                restarted.ml_public_key(MAIN_ACCOUNT).is_ok(),
                "a key must be there"
            );
        }
    }
    assert!(cuts > 1);
}

#[test]
fn a_signature_verifies_against_the_stored_key() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let key = wallet.ml_public_key(MAIN_ACCOUNT).expect("it's stored");
    let review = ml_review(&mut wallet);
    let proof = proof(&mut wallet);
    let signature = wallet
        .ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(3))
        .expect("signing works");
    assert_eq!(review.message, review.tx.sig_hash());
    assert!(verify(&key, &review.message, &signature));
}

#[test]
fn each_signature_uses_fresh_randomness() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let review = ml_review(&mut wallet);
    let mut rng = TestRng(3);
    let first_proof = proof(&mut wallet);
    let first = wallet
        .ml_sign(&review, MAIN_ACCOUNT, first_proof, &mut rng)
        .expect("it signs");
    let second_proof = proof(&mut wallet);
    let second = wallet
        .ml_sign(&review, MAIN_ACCOUNT, second_proof, &mut rng)
        .expect("it signs");
    assert_ne!(first, second);
}

#[test]
fn the_device_shows_what_the_transaction_really_does() {
    let flash = ml_flash();
    let review = ml_review(&mut open(&flash));
    assert_eq!((review.tx.to, review.tx.value), (TO, 1000));
}

#[test]
fn signing_leaves_the_counter_and_key_slots_untouched() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let before = flash.peek(0, UNTOUCHED);
    for _ in 0..3 {
        let review = ml_review(&mut wallet);
        let proof = proof(&mut wallet);
        wallet
            .ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(3))
            .expect("it signs");
    }
    assert_eq!(flash.peek(0, UNTOUCHED), before);
    assert_eq!(counter(&flash), 0);
}

#[test]
fn the_secret_key_is_never_stored() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let seed = seed_of(&mut wallet);
    let review = ml_review(&mut wallet);
    let proof = proof(&mut wallet);
    wallet
        .ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(3))
        .expect("it signs");
    let key_seed = KeySeed::derive(&seed, MAIN_ACCOUNT);
    assert!(!flash.contains(key_seed.expose()));
}

#[test]
fn a_signature_needs_a_stored_key_that_matches_the_seed() {
    let flash = ml_flash();
    MlKeyStore::new(flash.clone())
        .store(MAIN_ACCOUNT, &[7; 1312])
        .expect("it stores");
    let mut wallet = open(&flash);
    let review = ml_review(&mut wallet);
    let proof = proof(&mut wallet);
    let got = wallet.ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(3));
    assert_eq!(got, Err(WalletError::StorageFailure));
}

#[test]
fn a_missing_stored_key_is_rebuilt_when_signing() {
    let flash = ml_flash();
    MlKeyStore::new(flash.clone()).wipe().expect("it erases");
    let mut wallet = open(&flash);
    let review = ml_review(&mut wallet);
    let proof = proof(&mut wallet);
    let signature = wallet
        .ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(3))
        .expect("it signs");
    let key = wallet
        .ml_public_key(MAIN_ACCOUNT)
        .expect("it's stored again");
    assert!(verify(&key, &review.message, &signature));
}

#[test]
fn accounts_sign_with_their_own_keys() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let seed = seed_of(&mut wallet);
    let review = ml_review(&mut wallet);
    let proof = proof(&mut wallet);
    let signature = wallet
        .ml_sign(&review, 1, proof, &mut TestRng(3))
        .expect("it signs");
    assert!(verify(&derived_key(&seed, 1), &review.message, &signature));
    assert!(!verify(&derived_key(&seed, 0), &review.message, &signature));
}

#[test]
fn an_xmss_transaction_in_ml_dsa_mode_is_the_wrong_scheme() {
    let mut wallet = open(&ml_flash());
    assert_eq!(
        wallet.check_transaction(&xmss_raw(0)),
        Err(WalletError::WrongScheme)
    );
}

#[test]
fn an_ml_dsa_transaction_in_xmss_mode_is_the_wrong_scheme() {
    let mut wallet = open(&fresh());
    assert_eq!(
        wallet.check_transaction(&ml_raw()),
        Err(WalletError::WrongScheme)
    );
}

#[test]
fn garbage_is_a_bad_request_in_both_modes() {
    assert_eq!(
        open(&fresh()).check_transaction(&[1, 2]),
        Err(WalletError::BadRequest)
    );
    assert_eq!(
        open(&ml_flash()).check_transaction(&[1, 2]),
        Err(WalletError::BadRequest)
    );
}

#[test]
fn xmss_transactions_still_route_to_xmss() {
    let mut wallet = open(&fresh());
    assert!(matches!(
        wallet.check_transaction(&xmss_raw(0)),
        Ok(Pending::Xmss(_))
    ));
}

#[test]
fn xmss_rules_still_apply_through_the_router() {
    assert_eq!(
        open(&fresh()).check_transaction(&xmss_raw(1)),
        Err(WalletError::BadRequest)
    );
}

#[test]
fn prepare_and_activate_are_the_wrong_scheme_in_ml_dsa_mode() {
    let mut wallet = open(&ml_flash());
    assert_eq!(wallet.prepare(|_| {}), Err(WalletError::WrongScheme));
    assert_eq!(wallet.activate(1), Err(WalletError::WrongScheme));
}

#[test]
fn rotation_is_never_prompted_in_ml_dsa_mode() {
    let flash = ml_flash();
    let mut counter = FlashCounter::new(flash.clone(), layout::COUNTER);
    for _ in 0..950 {
        counter.advance().expect("it advances");
    }
    assert_eq!(open(&flash).should_prepare(), Ok(false));
}

#[test]
fn xmss_signing_is_refused_in_ml_dsa_mode() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let review = wallet
        .review(&xmss_raw(0))
        .expect("the transaction is fine");
    switch(&mut wallet, Scheme::MlDsa44);
    let proof = proof(&mut wallet);
    assert_eq!(wallet.sign(&review, proof), Err(WalletError::WrongScheme));
    assert_eq!(counter(&flash), 0);
}

#[test]
fn ml_dsa_signing_is_refused_in_xmss_mode() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let review = ml_review(&mut wallet);
    switch(&mut wallet, Scheme::Xmss);
    let proof = proof(&mut wallet);
    let got = wallet.ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(3));
    assert_eq!(got, Err(WalletError::WrongScheme));
}

#[test]
fn switching_back_continues_xmss_at_the_same_key() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let review = wallet
        .review(&xmss_raw(0))
        .expect("the transaction is fine");
    let first = proof(&mut wallet);
    wallet.sign(&review, first).expect("it signs");

    switch(&mut wallet, Scheme::MlDsa44);
    let ml = ml_review(&mut wallet);
    let second = proof(&mut wallet);
    wallet
        .ml_sign(&ml, MAIN_ACCOUNT, second, &mut TestRng(3))
        .expect("it signs");

    switch(&mut wallet, Scheme::Xmss);
    assert_eq!(wallet.next_index(), Ok(1));
    assert!(wallet.check_transaction(&xmss_raw(1)).is_ok());
}

#[test]
fn the_stored_key_stays_after_switching_back() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let key = wallet.ml_public_key(MAIN_ACCOUNT).expect("it's stored");
    switch(&mut wallet, Scheme::Xmss);
    assert_eq!(wallet.ml_public_key(MAIN_ACCOUNT), Ok(key));
}

#[test]
fn restoring_replaces_the_stored_key() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let new_seed = [0x77; 32];
    let proof = proof(&mut wallet);
    wallet
        .restore(&new_seed, 0, 0, proof, |_| {})
        .expect("it restores");
    let mut restarted = open(&flash);
    assert_eq!(restarted.scheme(), Scheme::MlDsa44);
    assert_eq!(
        restarted.ml_public_key(MAIN_ACCOUNT),
        Ok(derived_key(&new_seed, MAIN_ACCOUNT))
    );
}

#[test]
fn restoring_in_xmss_mode_clears_a_stale_key() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    switch(&mut wallet, Scheme::Xmss);
    let proof = proof(&mut wallet);
    wallet
        .restore(&[0x77; 32], 0, 0, proof, |_| {})
        .expect("it restores");
    assert_eq!(
        open(&flash).ml_public_key(MAIN_ACCOUNT),
        Err(WalletError::NothingPending)
    );
}

#[test]
fn a_restored_device_signs_for_the_restored_seed() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let new_seed = [0x77; 32];
    let restore_proof = proof(&mut wallet);
    wallet
        .restore(&new_seed, 0, 0, restore_proof, |_| {})
        .expect("it restores");
    let mut restarted = open(&flash);
    let review = ml_review(&mut restarted);
    let proof = proof(&mut restarted);
    let signature = restarted
        .ml_sign(&review, MAIN_ACCOUNT, proof, &mut TestRng(3))
        .expect("it signs");
    assert!(verify(
        &derived_key(&new_seed, 0),
        &review.message,
        &signature
    ));
}

#[test]
fn a_cut_while_restoring_never_leaves_a_stale_key_in_place() {
    let flash = ml_flash();
    let old_key = open(&flash)
        .ml_public_key(MAIN_ACCOUNT)
        .expect("it's stored");
    let image = flash.image();
    let new_seed = [0x77; 32];
    let mut cuts = 0;
    for budget in (0..).step_by(5) {
        let flash = SimFlash::new();
        flash.poke(0, &image);
        let mut wallet = open(&flash);
        let proof = proof(&mut wallet);
        flash.cut_after(budget);
        let result = wallet.restore(&new_seed, 0, 0, proof, |_| {});
        let was_cut = flash.was_cut();
        flash.power_on();

        let mut restarted = open(&flash);
        let seed = seed_of(&mut restarted);
        if let Ok(key) = restarted.ml_public_key(MAIN_ACCOUNT) {
            assert_eq!(key, derived_key(&seed, MAIN_ACCOUNT), "budget {budget}");
        }
        if !was_cut {
            assert_eq!(result, Ok(()));
            assert_eq!(seed, new_seed);
            break;
        }
        cuts += 1;
        let _ = old_key;
    }
    assert!(cuts > 3);
}

#[test]
fn the_final_erase_clears_the_key_and_the_scheme() {
    let flash = ml_flash();
    let mut wallet = open(&flash);
    let key = wallet.ml_public_key(MAIN_ACCOUNT).expect("it's stored");
    for attempt in 1..=10_u64 {
        wallet
            .enter_passcode(&WRONG, LATER * attempt)
            .expect("the flash is healthy");
    }
    assert!(!wallet.has_key());
    assert!(!flash.contains(&key[..64]));
    assert_eq!(wallet.scheme(), Scheme::Xmss);
    assert_eq!(open(&flash).scheme(), Scheme::Xmss);
}

#[test]
fn a_cut_during_the_final_erase_still_clears_the_key() {
    let mut cuts = 0;
    for budget in (0..60).step_by(3) {
        let flash = ml_flash();
        let mut wallet = open(&flash);
        let key = wallet.ml_public_key(MAIN_ACCOUNT).expect("it's stored");
        for attempt in 1..=9_u64 {
            wallet
                .enter_passcode(&WRONG, LATER * attempt)
                .expect("the flash is healthy");
        }
        flash.cut_after(budget);
        let _ = wallet.enter_passcode(&WRONG, LATER * 20);
        let was_cut = flash.was_cut();
        flash.power_on();
        if !was_cut {
            continue;
        }
        cuts += 1;
        let failures = Passcode::new(flash.clone(), layout::PASSCODE).failures();
        let restarted = open(&flash);
        if failures == Ok(10) || !restarted.has_key() {
            assert!(!restarted.has_key());
            assert!(!flash.contains(&key[..64]), "budget {budget}");
        }
    }
    assert!(cuts > 5);
}
