//! Invoices: fresh one-time keys, and the rule that a key never signs two different messages.

mod common;

use std::sync::OnceLock;

use common::{SimFlash, TestRng};
use qday_wallet::counter::FlashCounter;
use qday_wallet::flash::layout;
use qday_wallet::invoice::{Entry, Spend, SpendLog, keys_for, spend_hash};
use qday_wallet::keystore::KeyStore;
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::{Attempt, Code, MAX_FAILURES, Passcode, Proof};
use qday_wallet::settings::{Scheme, Settings};
use qday_wallet::wallet::{MAIN_ACCOUNT, Wallet, WalletError};
use qday_xmss::{Hash, verify_one_time};

const CODE: Code = [1, 2, 3, 4, 5, 6];
const WRONG: Code = [9, 9, 9, 9, 9, 9];
const POOL: [u8; 20] = [0x90; 20];
const TO: [u8; 20] = [0xBE; 20];
const OTHER: [u8; 20] = [0xBA; 20];
const FEE: u128 = 1_000;
const LOG_CAPACITY: u32 = 480;
const LATER: u64 = 1 << 40;

type TestWallet = Wallet<SimFlash, FlashCounter<SimFlash>>;

fn open(flash: &SimFlash) -> TestWallet {
    Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        FlashCounter::new(flash.clone(), layout::COUNTER),
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        SpendLog::new(flash.clone()),
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

fn seed_of(wallet: &mut TestWallet) -> Hash {
    let proof = proof(wallet);
    wallet.seed(proof).expect("the flash is healthy")
}

fn spend(number: u32) -> Spend {
    Spend {
        number,
        pool: POOL,
        to: TO,
        fee: FEE,
    }
}

fn sign(wallet: &mut TestWallet, spend: &Spend) -> Result<Vec<u8>, WalletError> {
    let proof = proof(wallet);
    wallet
        .sign_invoice(spend, MAIN_ACCOUNT, proof)
        .map(|signature| signature.to_vec())
}

#[test]
fn an_invoice_is_the_key_for_its_number() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let seed = seed_of(&mut wallet);
    let invoice = wallet.invoice(MAIN_ACCOUNT, 12).expect("it derives");
    let keys = keys_for(&seed, MAIN_ACCOUNT, 12);
    assert_eq!(invoice.number, 12);
    assert_eq!(invoice.leaf, keys.leaf(0));
    assert_eq!(invoice.pub_seed, *keys.pub_seed());
}

#[test]
fn asking_for_an_invoice_writes_nothing() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let before = flash.operations();
    wallet.invoice(MAIN_ACCOUNT, 12).expect("it derives");
    assert_eq!(flash.operations(), before);
}

#[test]
fn accounts_have_their_own_invoices() {
    let flash = fresh();
    let mut wallet = open(&flash);
    assert_ne!(
        wallet.invoice(0, 12).expect("it derives").leaf,
        wallet.invoice(1, 12).expect("it derives").leaf
    );
}

#[test]
fn a_spend_is_signed_for_the_real_key() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let invoice = wallet.invoice(MAIN_ACCOUNT, 3).expect("it derives");
    let signature = sign(&mut wallet, &spend(3)).expect("it signs");
    let message = spend_hash(&POOL, &invoice.leaf, &TO, FEE);
    assert!(verify_one_time(
        &invoice.leaf,
        &invoice.pub_seed,
        &message,
        &signature
    ));
}

#[test]
fn the_same_spend_can_be_signed_again_with_the_same_result() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let first = sign(&mut wallet, &spend(3)).expect("it signs");
    let second = sign(&mut wallet, &spend(3)).expect("it signs");
    assert_eq!(first, second);
}

#[test]
fn a_key_never_signs_a_different_recipient() {
    let flash = fresh();
    let mut wallet = open(&flash);
    sign(&mut wallet, &spend(3)).expect("it signs");
    let changed = Spend {
        to: OTHER,
        ..spend(3)
    };
    assert_eq!(sign(&mut wallet, &changed), Err(WalletError::InvoiceUsed));
}

#[test]
fn a_key_never_signs_a_different_fee() {
    let flash = fresh();
    let mut wallet = open(&flash);
    sign(&mut wallet, &spend(3)).expect("it signs");
    let changed = Spend {
        fee: FEE + 1,
        ..spend(3)
    };
    assert_eq!(sign(&mut wallet, &changed), Err(WalletError::InvoiceUsed));
}

#[test]
fn a_key_never_signs_for_a_different_pool() {
    let flash = fresh();
    let mut wallet = open(&flash);
    sign(&mut wallet, &spend(3)).expect("it signs");
    let changed = Spend {
        pool: OTHER,
        ..spend(3)
    };
    assert_eq!(sign(&mut wallet, &changed), Err(WalletError::InvoiceUsed));
}

#[test]
fn the_rule_survives_a_restart() {
    let flash = fresh();
    sign(&mut open(&flash), &spend(3)).expect("it signs");
    let changed = Spend {
        to: OTHER,
        ..spend(3)
    };
    assert_eq!(
        sign(&mut open(&flash), &changed),
        Err(WalletError::InvoiceUsed)
    );
}

#[test]
fn other_invoices_are_unaffected() {
    let flash = fresh();
    let mut wallet = open(&flash);
    sign(&mut wallet, &spend(3)).expect("it signs");
    assert!(sign(&mut wallet, &spend(4)).is_ok());
}

#[test]
fn a_zero_recipient_is_refused_before_it_uses_the_key() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let zero = Spend {
        to: [0; 20],
        ..spend(3)
    };
    assert_eq!(sign(&mut wallet, &zero), Err(WalletError::BadRequest));
    assert!(sign(&mut wallet, &spend(3)).is_ok());
}

#[test]
fn a_zero_pool_is_refused_before_it_uses_the_key() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let zero = Spend {
        pool: [0; 20],
        ..spend(3)
    };
    assert_eq!(sign(&mut wallet, &zero), Err(WalletError::BadRequest));
    assert!(sign(&mut wallet, &spend(3)).is_ok());
}

#[test]
fn spending_leaves_the_counter_and_key_slots_alone() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let proof = proof(&mut wallet);
    let before = flash.peek(0, layout::MLDSA_PUBLIC as usize);
    wallet
        .sign_invoice(&spend(3), MAIN_ACCOUNT, proof)
        .expect("it signs");
    assert!(flash.peek(0, layout::MLDSA_PUBLIC as usize) == before);
}

#[test]
fn invoices_work_in_ml_dsa_mode_too() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let proof_now = proof(&mut wallet);
    wallet
        .set_scheme(Scheme::MlDsa44, proof_now)
        .expect("it switches");
    let invoice = wallet.invoice(MAIN_ACCOUNT, 3).expect("it derives");
    let signature = sign(&mut wallet, &spend(3)).expect("it signs");
    let message = spend_hash(&POOL, &invoice.leaf, &TO, FEE);
    assert!(verify_one_time(
        &invoice.leaf,
        &invoice.pub_seed,
        &message,
        &signature
    ));
}

/// The record comes first, so a cut can never leave a signature with no record behind it.
#[test]
fn a_cut_never_lets_a_key_sign_two_messages() {
    let image = {
        let flash = fresh();
        sign(&mut open(&flash), &spend(1)).expect("it signs");
        flash.image()
    };
    let mut cuts = 0;
    for budget in 0.. {
        let flash = SimFlash::new();
        flash.poke(0, &image);
        let mut wallet = open(&flash);
        let proof = proof(&mut wallet);
        flash.cut_after(budget);
        let result = wallet.sign_invoice(&spend(2), MAIN_ACCOUNT, proof);
        let was_cut = flash.was_cut();
        flash.power_on();

        let changed = Spend {
            to: OTHER,
            ..spend(2)
        };
        if result.is_ok() {
            assert_eq!(
                sign(&mut open(&flash), &changed),
                Err(WalletError::InvoiceUsed),
                "budget {budget}"
            );
        }
        if !was_cut {
            assert!(result.is_ok());
            break;
        }
        cuts += 1;
        assert!(result.is_err(), "budget {budget}");
        // Whatever reached the flash, the first request can always be retried.
        assert!(
            sign(&mut open(&flash), &spend(2)).is_ok(),
            "budget {budget}"
        );
    }
    assert!(cuts >= 1);
}

#[test]
fn a_write_that_doesnt_stick_gives_no_signature() {
    let flash = fresh();
    let mut wallet = open(&flash);
    let proof = proof(&mut wallet);
    flash.lose_writes();
    assert!(matches!(
        wallet.sign_invoice(&spend(3), MAIN_ACCOUNT, proof),
        Err(WalletError::StorageFailure)
    ));
}

#[test]
fn a_damaged_record_is_skipped() {
    let flash = fresh();
    let mut wallet = open(&flash);
    sign(&mut wallet, &spend(1)).expect("it signs");
    flash.poke(layout::SPENT_INVOICES as usize + 10, &[0x00]);
    assert!(sign(&mut wallet, &spend(2)).is_ok());
    let changed = Spend {
        to: OTHER,
        ..spend(2)
    };
    assert_eq!(sign(&mut wallet, &changed), Err(WalletError::InvoiceUsed));
}

#[test]
fn a_full_log_refuses_new_keys_but_remembers_old_ones() {
    let flash = fresh();
    let mut log = SpendLog::new(flash.clone());
    for number in 0..LOG_CAPACITY {
        let mut leaf = [0; 32];
        leaf[..4].copy_from_slice(&number.to_be_bytes());
        assert_eq!(log.record(&leaf, &[1; 32]), Ok(Entry::Recorded), "{number}");
    }
    assert_eq!(log.record(&[0xAA; 32], &[1; 32]), Ok(Entry::Full));
    let mut last = [0; 32];
    last[..4].copy_from_slice(&(LOG_CAPACITY - 1).to_be_bytes());
    assert_eq!(log.record(&last, &[1; 32]), Ok(Entry::Recorded));
    assert_eq!(log.record(&last, &[2; 32]), Ok(Entry::Used));
}

#[test]
fn a_full_log_gives_its_own_error() {
    let flash = fresh();
    let mut log = SpendLog::new(flash.clone());
    for number in 0..LOG_CAPACITY {
        let mut leaf = [0; 32];
        leaf[..4].copy_from_slice(&number.to_be_bytes());
        log.record(&leaf, &[1; 32]).expect("it records");
    }
    let mut wallet = open(&flash);
    assert_eq!(sign(&mut wallet, &spend(3)), Err(WalletError::SpendLogFull));
}

#[test]
fn the_log_stays_inside_its_sectors() {
    let flash = fresh();
    let after = (layout::SPENT_INVOICES + layout::SPENT_INVOICE_SECTORS * 0x1000) as usize;
    let before = flash.peek(after, 0x1000);
    let mut log = SpendLog::new(flash.clone());
    for number in 0..LOG_CAPACITY + 5 {
        let mut leaf = [0; 32];
        leaf[..4].copy_from_slice(&number.to_be_bytes());
        log.record(&leaf, &[1; 32]).expect("it records");
    }
    assert!(flash.peek(after, 0x1000) == before);
}

fn lock_out(wallet: &mut TestWallet) {
    for failure in 0..MAX_FAILURES {
        let now = LATER * (u64::from(failure) + 1);
        wallet.enter_passcode(&WRONG, now).expect("it counts");
    }
}

#[test]
fn the_final_erase_clears_the_log() {
    let flash = fresh();
    let mut wallet = open(&flash);
    sign(&mut wallet, &spend(3)).expect("it signs");
    lock_out(&mut wallet);
    assert!(
        flash
            .peek(layout::SPENT_INVOICES as usize, 68)
            .iter()
            .all(|&b| b == 0xFF)
    );
}

#[test]
fn a_cut_during_the_final_erase_still_clears_the_log() {
    let image = {
        let flash = fresh();
        sign(&mut open(&flash), &spend(3)).expect("it signs");
        flash.image()
    };
    for budget in 0..40 {
        let flash = SimFlash::new();
        flash.poke(0, &image);
        let mut wallet = open(&flash);
        flash.cut_after(budget + 30);
        for failure in 0..MAX_FAILURES {
            let now = LATER * (u64::from(failure) + 1);
            if wallet.enter_passcode(&WRONG, now).is_err() {
                break;
            }
        }
        let was_cut = flash.was_cut();
        flash.power_on();
        let failures_left = open(&flash).passcode().exists() == Ok(true);
        if was_cut && failures_left {
            continue;
        }
        assert!(
            flash
                .peek(layout::SPENT_INVOICES as usize, 68)
                .iter()
                .all(|&b| b == 0xFF),
            "budget {budget}"
        );
    }
}
