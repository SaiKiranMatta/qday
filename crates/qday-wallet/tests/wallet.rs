//! The rules for what gets signed, tested end to end with real signatures.

mod common;

use std::sync::OnceLock;

use common::{SimFlash, TestRng};
use qday_wallet::counter::{CounterError, FlashCounter, OneWayCounter};
use qday_wallet::flash::layout;
use qday_wallet::frame::{Action, FrameTx};
use qday_wallet::keystore::KeyStore;
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::{Attempt, Code, Passcode, Proof};
use qday_wallet::settings::Settings;
use qday_wallet::wallet::{RESTORE_SKIP, Review, Wallet, WalletError};
use qday_xmss::{Hash, KeySet, PublicKey, Tree, verify};

const CODE: Code = [1, 2, 3, 4, 5, 6];
const WRONG: Code = [9, 9, 9, 9, 9, 9];
const SENDER: [u8; 20] = [0x11; 20];
const TO: [u8; 20] = [0x22; 20];
const LATER: u64 = 1 << 40;

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

/// One wallet image with a key and a passcode, built once because trees take time.
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

fn transfer(index: u32) -> Vec<u8> {
    FrameTx::transfer(SENDER, 1, index, TO, 1000).encode(&[])
}

fn sign_next(wallet: &mut TestWallet) -> (u32, [u8; 2500], Hash) {
    let index = wallet.next_index().expect("the flash is healthy");
    let review = wallet
        .review(&transfer(index))
        .expect("the transaction is fine");
    let proof = proof(wallet);
    let signature = wallet.sign(&review, proof).expect("the flash is healthy");
    (index, signature, review.message)
}

/// Moves the counter so the next key is `index`.
fn jump_to(flash: &SimFlash, index: u32) {
    let mut wallet = open(flash);
    let next = wallet.next_index().expect("the flash is healthy");
    let mut counter = FlashCounter::new(flash.clone(), layout::COUNTER);
    for _ in next..index {
        counter.advance().expect("the flash is healthy");
    }
}

#[test]
fn a_new_wallet_has_no_key() {
    assert!(!open(&SimFlash::new()).has_key());
}

#[test]
fn creating_a_wallet_reports_every_leaf() {
    let flash = SimFlash::new();
    let mut wallet = open(&flash);
    let mut last = 0;
    wallet
        .create(&mut TestRng(1), |done| last = done)
        .expect("the flash is healthy");
    assert_eq!(last, 1024);
    assert!(wallet.has_key());
    assert_eq!(wallet.next_index(), Ok(0));
}

#[test]
fn a_second_create_is_refused() {
    let mut wallet = open(&fresh());
    assert_eq!(
        wallet.create(&mut TestRng(1), |_| {}),
        Err(WalletError::StorageFailure)
    );
}

#[test]
fn a_cut_while_creating_leaves_nothing_or_a_whole_wallet() {
    let mut cuts = 0;
    for budget in (0..).step_by(3) {
        let flash = SimFlash::new();
        let mut wallet = open(&flash);
        flash.cut_after(budget);
        let result = wallet.create(&mut TestRng(1), |_| {});
        let was_cut = flash.was_cut();
        flash.power_on();

        let mut restarted = open(&flash);
        if !was_cut {
            assert_eq!(result, Ok(()));
            assert!(restarted.has_key());
            break;
        }
        cuts += 1;
        if restarted.has_key() {
            assert_eq!(restarted.next_index(), Ok(0));
        } else {
            restarted
                .create(&mut TestRng(2), |_| {})
                .expect("the flash is back");
        }
    }
    assert!(cuts > 5);
}

#[test]
fn the_key_survives_a_restart() {
    let flash = fresh();
    let before = open(&flash).active();
    assert!(before.is_some());
    assert_eq!(open(&flash).active(), before);
}

#[test]
fn a_signature_verifies_against_the_public_key() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let key = wallet.active().expect("a key is active").key;
    let (index, signature, message) = sign_next(&mut wallet);
    assert_eq!(index, 0);
    assert!(verify(&key, &message, &signature));
}

#[test]
fn each_signature_uses_the_next_key() {
    let flash = fresh();
    let mut wallet = open(&flash);
    for expected in 0..4 {
        let (index, signature, _) = sign_next(&mut wallet);
        assert_eq!(index, expected);
        assert_eq!(signature[..4], expected.to_be_bytes());
    }
    assert_eq!(open(&flash).next_index(), Ok(4));
}

#[test]
fn a_review_does_not_move_the_counter() {
    let flash = fresh();
    let mut wallet = open(&flash);
    wallet
        .review(&transfer(0))
        .expect("the transaction is fine");
    assert_eq!(wallet.next_index(), Ok(0));
}

#[test]
fn a_rejected_transaction_leaves_the_key_unused() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let review = wallet.review(&transfer(0));
    assert!(review.is_ok());
    drop(review);
    assert_eq!(open(&flash).next_index(), Ok(0));
}

#[test]
fn garbage_is_a_bad_request() {
    assert_eq!(
        open(&fresh()).review(&[1, 2, 3]),
        Err(WalletError::BadRequest)
    );
}

#[test]
fn a_transaction_for_another_index_is_a_bad_request() {
    let mut wallet = open(&fresh());
    assert_eq!(wallet.review(&transfer(1)), Err(WalletError::BadRequest));
}

#[test]
fn the_device_reports_what_the_transaction_really_does() {
    let review = open(&fresh())
        .review(&transfer(0))
        .expect("the transaction is fine");
    assert_eq!(
        review.tx.action,
        Action::Transfer {
            to: TO,
            value: 1000
        }
    );
    assert_eq!(review.message, review.tx.sig_hash());
}

#[test]
fn a_stale_review_cannot_be_signed_after_the_counter_moved() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let review = wallet
        .review(&transfer(0))
        .expect("the transaction is fine");
    sign_next(&mut wallet);
    let proof = proof(&mut wallet);
    assert_eq!(wallet.sign(&review, proof), Err(WalletError::BadRequest));
    assert_eq!(wallet.next_index(), Ok(1));
}

#[test]
fn the_counter_is_stored_before_the_signature_comes_back() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let (_, _, _) = sign_next(&mut wallet);
    assert_eq!(
        FlashCounter::new(flash.clone(), layout::COUNTER).value(),
        Ok(1)
    );
}

#[test]
fn a_power_cut_while_signing_never_reuses_a_key() {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = fresh();
        let mut wallet = open(&flash);
        let review = wallet
            .review(&transfer(0))
            .expect("the transaction is fine");
        let proof = proof(&mut wallet);
        flash.cut_after(budget);
        let result = wallet.sign(&review, proof);
        let was_cut = flash.was_cut();
        flash.power_on();

        let next = open(&flash).next_index().expect("the flash is back");
        if !was_cut {
            assert!(result.is_ok());
            assert_eq!(next, 1);
            break;
        }
        cuts += 1;
        assert!(result.is_err(), "a cut must not produce a signature");
        assert!(next <= 1);
    }
    assert!(cuts > 0);
}

/// A counter that refuses to move.
struct StuckCounter;

impl OneWayCounter for StuckCounter {
    fn value(&mut self) -> Result<u32, CounterError> {
        Ok(0)
    }

    fn advance(&mut self) -> Result<u32, CounterError> {
        Err(CounterError)
    }
}

#[test]
fn nothing_is_signed_if_the_counter_fails() {
    let flash = fresh();
    let mut wallet = Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        StuckCounter,
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        0,
    )
    .expect("the flash is healthy");
    let review = wallet
        .review(&transfer(0))
        .expect("the transaction is fine");
    let Ok(Attempt::Correct(proof)) = wallet.enter_passcode(&CODE, 0) else {
        panic!("the code is right");
    };
    assert_eq!(
        wallet.sign(&review, proof),
        Err(WalletError::CounterFailure)
    );
}

/// A counter that reports a move it didn't make.
struct JumpyCounter;

impl OneWayCounter for JumpyCounter {
    fn value(&mut self) -> Result<u32, CounterError> {
        Ok(0)
    }

    fn advance(&mut self) -> Result<u32, CounterError> {
        Ok(5)
    }
}

#[test]
fn nothing_is_signed_if_the_counter_gives_the_wrong_index() {
    let flash = fresh();
    let mut wallet = Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        JumpyCounter,
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        0,
    )
    .expect("the flash is healthy");
    let review = wallet
        .review(&transfer(0))
        .expect("the transaction is fine");
    let Ok(Attempt::Correct(proof)) = wallet.enter_passcode(&CODE, 0) else {
        panic!("the code is right");
    };
    assert_eq!(
        wallet.sign(&review, proof),
        Err(WalletError::CounterFailure)
    );
}

#[test]
fn key_1007_still_signs_a_transfer() {
    let flash = fresh();
    jump_to(&flash, 1007);
    let (index, _, _) = sign_next(&mut open(&flash));
    assert_eq!(index, 1007);
}

#[test]
fn a_transfer_on_a_reserved_key_needs_a_rotation() {
    let flash = fresh();
    jump_to(&flash, 1008);
    assert_eq!(
        open(&flash).review(&transfer(1008)),
        Err(WalletError::RotationNeeded)
    );
}

#[test]
fn an_unknown_call_on_a_reserved_key_needs_a_rotation() {
    let flash = fresh();
    jump_to(&flash, 1008);
    let mut tx = FrameTx::transfer(SENDER, 1, 1008, TO, 0);
    tx.action = Action::Other {
        target: Some(TO),
        value: 0,
        data: vec![1, 2, 3, 4],
        execution: 50_000,
        state: 200_000,
    };
    assert_eq!(
        open(&flash).review(&tx.encode(&[])),
        Err(WalletError::RotationNeeded)
    );
}

#[test]
fn no_keys_left_is_exhausted() {
    let flash = fresh();
    jump_to(&flash, 1024);
    assert_eq!(
        open(&flash).review(&transfer(1024)),
        Err(WalletError::KeysExhausted)
    );
}

#[test]
fn the_next_tree_is_built_from_key_900() {
    let flash = fresh();
    jump_to(&flash, 899);
    assert_eq!(open(&flash).should_prepare(), Ok(false));
    jump_to(&flash, 900);
    assert_eq!(open(&flash).should_prepare(), Ok(true));
}

#[test]
fn a_prepared_tree_stops_the_prompt() {
    let flash = fresh();
    jump_to(&flash, 900);
    let mut wallet = open(&flash);
    wallet.prepare(|_| {}).expect("the flash is healthy");
    assert_eq!(wallet.should_prepare(), Ok(false));
}

#[test]
fn prepare_builds_tree_1_from_the_same_seed() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let key = wallet.prepare(|_| {}).expect("the flash is healthy");
    let proof = proof(&mut wallet);
    let seed = wallet.seed(proof).expect("the flash is healthy");
    let keys = KeySet::derive(&seed, 1);
    let tree = Tree::build(&keys, |_| {});
    assert_eq!(
        key,
        PublicKey {
            root: tree.root(),
            pub_seed: *keys.pub_seed()
        }
    );
    assert_eq!(wallet.prepared().map(|prepared| prepared.tree), Some(1));
}

#[test]
fn preparing_twice_does_not_rebuild() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let first = wallet.prepare(|_| {}).expect("the flash is healthy");
    let before = flash.operations();
    let second = wallet.prepare(|_| {}).expect("the flash is healthy");
    assert_eq!(first, second);
    assert_eq!(flash.operations(), before);
}

fn rotation(wallet: &mut TestWallet, index: u32, key: &PublicKey) -> Vec<u8> {
    let _ = wallet;
    FrameTx::rotation(SENDER, 1, index, key.root, key.pub_seed).encode(&[])
}

#[test]
fn a_rotation_to_the_prepared_key_signs_the_rotation_message() {
    let flash = fresh();
    jump_to(&flash, 1008);
    let mut wallet = open(&flash);
    let active = wallet.active().expect("a key is active").key;
    let next = wallet.prepare(|_| {}).expect("the flash is healthy");
    let raw = rotation(&mut wallet, 1008, &next);
    let review = wallet.review(&raw).expect("the rotation is fine");
    assert_ne!(review.message, review.tx.sig_hash());
    assert_eq!(review.message, review.tx.signing_message());
    let proof = proof(&mut wallet);
    let signature = wallet.sign(&review, proof).expect("the flash is healthy");
    assert!(verify(&active, &review.message, &signature));
}

#[test]
fn a_rotation_works_before_the_reserved_keys() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let next = wallet.prepare(|_| {}).expect("the flash is healthy");
    let raw = rotation(&mut wallet, 0, &next);
    assert!(wallet.review(&raw).is_ok());
}

#[test]
fn a_rotation_with_nothing_prepared_is_nothing_pending() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let key = PublicKey {
        root: [1; 32],
        pub_seed: [2; 32],
    };
    let raw = rotation(&mut wallet, 0, &key);
    assert_eq!(wallet.review(&raw), Err(WalletError::NothingPending));
}

#[test]
fn a_rotation_to_any_other_key_is_nothing_pending() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let mut key = wallet.prepare(|_| {}).expect("the flash is healthy");
    key.root[0] ^= 1;
    let raw = rotation(&mut wallet, 0, &key);
    assert_eq!(wallet.review(&raw), Err(WalletError::NothingPending));
}

#[test]
fn transfers_keep_signing_the_plain_hash_while_a_tree_is_prepared() {
    let flash = fresh();
    let mut wallet = open(&flash);
    wallet.prepare(|_| {}).expect("the flash is healthy");
    let review = wallet
        .review(&transfer(0))
        .expect("the transaction is fine");
    assert_eq!(review.message, review.tx.sig_hash());
}

#[test]
fn activate_needs_a_prepared_tree() {
    assert_eq!(open(&fresh()).activate(1), Err(WalletError::NothingPending));
}

#[test]
fn activate_needs_the_right_tree_number() {
    let flash = fresh();
    let mut wallet = open(&flash);
    wallet.prepare(|_| {}).expect("the flash is healthy");
    assert_eq!(wallet.activate(2), Err(WalletError::NothingPending));
}

#[test]
fn activating_starts_the_new_tree_at_key_0() {
    let flash = fresh();
    jump_to(&flash, 1009);
    let mut wallet = open(&flash);
    let next = wallet.prepare(|_| {}).expect("the flash is healthy");
    wallet.activate(1).expect("the flash is healthy");

    let mut restarted = open(&flash);
    let active = restarted.active().expect("a key is active");
    assert_eq!((active.tree, active.key), (1, next));
    assert_eq!(restarted.next_index(), Ok(0));
    let (index, signature, message) = sign_next(&mut restarted);
    assert_eq!(index, 0);
    assert!(verify(&next, &message, &signature));
}

#[test]
fn the_counter_keeps_rising_across_trees() {
    let flash = fresh();
    jump_to(&flash, 10);
    let mut wallet = open(&flash);
    wallet.prepare(|_| {}).expect("the flash is healthy");
    wallet.activate(1).expect("the flash is healthy");
    assert_eq!(
        FlashCounter::new(flash.clone(), layout::COUNTER).value(),
        Ok(10)
    );
    sign_next(&mut open(&flash));
    assert_eq!(
        FlashCounter::new(flash.clone(), layout::COUNTER).value(),
        Ok(11)
    );
}

#[test]
fn a_cut_while_activating_leaves_a_working_key_and_no_reuse() {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = fresh();
        jump_to(&flash, 3);
        let mut wallet = open(&flash);
        wallet.prepare(|_| {}).expect("the flash is healthy");
        flash.cut_after(budget);
        let result = wallet.activate(1);
        let was_cut = flash.was_cut();
        flash.power_on();

        let mut restarted = open(&flash);
        let tree = restarted.active().expect("a key is active").tree;
        let next = restarted.next_index().expect("the flash is back");
        if !was_cut {
            assert_eq!(result, Ok(()));
            assert_eq!((tree, next), (1, 0));
            break;
        }
        cuts += 1;
        assert!(
            (tree, next) == (0, 3) || (tree, next) == (1, 0),
            "tree {tree} key {next}"
        );
    }
    assert!(cuts > 0);
}

#[test]
fn restore_skips_past_the_chain_index() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let seed = [0x77; 32];
    let proof = proof(&mut wallet);
    wallet
        .restore(&seed, 2, 100, proof, |_| {})
        .expect("the flash is healthy");

    let mut restarted = open(&flash);
    let active = restarted.active().expect("a key is active");
    assert_eq!(active.tree, 2);
    assert_eq!(restarted.next_index(), Ok(100 + RESTORE_SKIP));

    let keys = KeySet::derive(&seed, 2);
    let tree = Tree::build(&keys, |_| {});
    assert_eq!(active.key.root, tree.root());
    let (index, signature, message) = sign_next(&mut restarted);
    assert_eq!(index, 164);
    assert!(verify(&active.key, &message, &signature));
}

#[test]
fn restore_works_on_a_used_device_without_lowering_the_counter() {
    let flash = fresh();
    jump_to(&flash, 50);
    let mut wallet = open(&flash);
    let proof = proof(&mut wallet);
    wallet
        .restore(&[0x77; 32], 0, 0, proof, |_| {})
        .expect("the flash is healthy");
    assert_eq!(open(&flash).next_index(), Ok(64));
    assert_eq!(
        FlashCounter::new(flash.clone(), layout::COUNTER).value(),
        Ok(50)
    );
}

#[test]
fn restore_past_the_end_of_the_tree_is_exhausted() {
    let mut wallet = open(&fresh());
    let proof = proof(&mut wallet);
    let got = wallet.restore(&[0x77; 32], 0, 1024 - RESTORE_SKIP, proof, |_| {});
    assert_eq!(got, Err(WalletError::KeysExhausted));
}

#[test]
fn a_cut_while_restoring_keeps_the_old_key_or_the_new_one() {
    let mut cuts = 0;
    let old = open(&fresh()).active().expect("a key is active").key;
    for budget in (0..).step_by(5) {
        let flash = fresh();
        let mut wallet = open(&flash);
        let proof = proof(&mut wallet);
        flash.cut_after(budget);
        let result = wallet.restore(&[0x77; 32], 0, 0, proof, |_| {});
        let was_cut = flash.was_cut();
        flash.power_on();
        let active = open(&flash).active().expect("a key must stay active");
        if !was_cut {
            assert_eq!(result, Ok(()));
            assert_ne!(active.key, old);
            break;
        }
        cuts += 1;
        assert!(active.key == old || active.first_index == RESTORE_SKIP);
    }
    assert!(cuts > 0);
}

#[test]
fn the_seed_reads_back_for_a_backup() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let proof = proof(&mut wallet);
    let seed = wallet.seed(proof).expect("the flash is healthy");
    assert_ne!(seed, [0; 32]);
    assert!(flash.contains(&seed));
}

#[test]
fn the_tenth_failure_erases_the_keys_and_the_passcode() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let proof_seed = {
        let proof = proof(&mut wallet);
        wallet.seed(proof).expect("the flash is healthy")
    };
    for attempt in 1..=10_u64 {
        wallet
            .enter_passcode(&WRONG, LATER * attempt)
            .expect("the flash is healthy");
    }
    assert!(!wallet.has_key());
    assert!(!flash.contains(&proof_seed));
    assert_eq!(wallet.passcode().exists(), Ok(false));
}

#[test]
fn the_counter_survives_an_erase() {
    let flash = fresh();
    jump_to(&flash, 5);
    let mut wallet = open(&flash);
    for attempt in 1..=10_u64 {
        wallet
            .enter_passcode(&WRONG, LATER * attempt)
            .expect("the flash is healthy");
    }
    assert_eq!(
        FlashCounter::new(flash.clone(), layout::COUNTER).value(),
        Ok(5)
    );
}

#[test]
fn a_cut_during_the_final_erase_finishes_on_the_next_start() {
    let mut cuts = 0;
    for budget in (0..60).step_by(3) {
        let flash = fresh();
        let mut wallet = open(&flash);
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
        let mut restarted = open(&flash);
        if failures == Ok(10) || !restarted.has_key() {
            assert!(
                !restarted.has_key(),
                "budget {budget}: keys left after the last failure"
            );
            assert_eq!(restarted.passcode().exists(), Ok(false));
        }
    }
    assert!(cuts > 5);
}

#[test]
fn a_review_type_is_not_enough_to_sign_without_proof() {
    // This is a compile-time rule: `sign` takes a `Proof`, which only a correct code produces.
    fn takes_proof(_: fn(&mut TestWallet, &Review, Proof) -> Result<[u8; 2500], WalletError>) {}
    takes_proof(TestWallet::sign);
}
