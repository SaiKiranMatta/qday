//! With a key in the chip, nothing read from the flash may give up the seed or the passcode.

mod common;

use common::{SimFlash, TestRng};
use qday_wallet::counter::FlashCounter;
use qday_wallet::flash::layout;
use qday_wallet::keystore::{KeyStore, StoreError};
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::{Attempt, Code, Passcode};
use qday_wallet::seal::{SealError, Sealer};
use qday_wallet::settings::Settings;
use qday_wallet::wallet::Wallet;
use qday_xmss::{Hash, KeySet, Tree, keccak256};

const SEED: Hash = [0x42; 32];
const OTHER_SEED: Hash = [0x43; 32];
const CHIP: ChipKey = ChipKey([7; 32]);
const OTHER_CHIP: ChipKey = ChipKey([8; 32]);
const CODE: Code = [1, 2, 3, 4, 5, 6];
const WRONG: Code = [9, 9, 9, 9, 9, 9];
/// Where a slot's header keeps the seed.
const SEED_AT: usize = layout::KEY_SLOTS as usize + 8;

/// Stands in for the chip's HMAC: a keyed hash whose key nothing can read.
#[derive(Clone, Copy)]
struct ChipKey([u8; 32]);

impl Sealer for ChipKey {
    fn keyed_hash(&mut self, message: &[u8]) -> Result<Hash, SealError> {
        Ok(keccak256(&[&self.0, message]))
    }
}

struct BrokenChip;

impl Sealer for BrokenChip {
    fn keyed_hash(&mut self, _message: &[u8]) -> Result<Hash, SealError> {
        Err(SealError)
    }
}

fn tree_of(seed: &Hash) -> Tree {
    Tree::build(&KeySet::derive(seed, 0), |_| {})
}

fn plain(flash: &SimFlash) -> KeyStore<SimFlash> {
    KeyStore::new(flash.clone())
}

fn sealed(flash: &SimFlash, chip: ChipKey) -> KeyStore<SimFlash> {
    KeyStore::new(flash.clone()).sealed_by(Box::new(chip))
}

fn pin(flash: &SimFlash) -> Passcode<SimFlash> {
    Passcode::new(flash.clone(), layout::PASSCODE)
}

fn sealed_pin(flash: &SimFlash, chip: ChipKey) -> Passcode<SimFlash> {
    pin(flash).sealed_by(Box::new(chip))
}

type TestWallet = Wallet<SimFlash, FlashCounter<SimFlash>>;

fn open_sealed(flash: &SimFlash) -> TestWallet {
    Wallet::open(
        sealed(flash, CHIP),
        sealed_pin(flash, CHIP),
        FlashCounter::new(flash.clone(), layout::COUNTER),
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        0,
    )
    .expect("the flash is healthy")
}

fn open_plain(flash: &SimFlash) -> TestWallet {
    Wallet::open(
        plain(flash),
        pin(flash),
        FlashCounter::new(flash.clone(), layout::COUNTER),
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        0,
    )
    .expect("the flash is healthy")
}

#[test]
fn a_sealed_seed_is_not_in_the_flash() {
    let flash = SimFlash::new();
    sealed(&flash, CHIP)
        .write_tree(0, &SEED, 0, 0, &tree_of(&SEED))
        .expect("the flash is healthy");
    assert!(!flash.contains(&SEED));
}

#[test]
fn a_sealed_seed_reads_back_on_its_own_chip() {
    let flash = SimFlash::new();
    let mut keys = sealed(&flash, CHIP);
    keys.write_tree(0, &SEED, 0, 0, &tree_of(&SEED))
        .expect("the flash is healthy");
    assert_eq!(keys.seed(0), Ok(SEED));
}

#[test]
fn another_chip_gets_an_error_and_never_a_wrong_seed() {
    let flash = SimFlash::new();
    sealed(&flash, CHIP)
        .write_tree(0, &SEED, 0, 0, &tree_of(&SEED))
        .expect("the flash is healthy");
    assert_eq!(sealed(&flash, OTHER_CHIP).seed(0), Err(StoreError));
}

#[test]
fn two_seeds_are_not_sealed_with_the_same_mask() {
    let first = SimFlash::new();
    let second = SimFlash::new();
    sealed(&first, CHIP)
        .write_tree(0, &SEED, 0, 0, &tree_of(&SEED))
        .expect("the flash is healthy");
    sealed(&second, CHIP)
        .write_tree(0, &OTHER_SEED, 0, 0, &tree_of(&OTHER_SEED))
        .expect("the flash is healthy");
    let mixed: Vec<u8> = first
        .peek(SEED_AT, 32)
        .iter()
        .zip(second.peek(SEED_AT, 32))
        .map(|(a, b)| a ^ b)
        .collect();
    let seeds_mixed: Vec<u8> = SEED.iter().zip(OTHER_SEED).map(|(a, b)| a ^ b).collect();
    assert_ne!(mixed, seeds_mixed);
}

#[test]
fn a_broken_chip_stores_nothing_rather_than_a_plain_seed() {
    let flash = SimFlash::new();
    let mut keys = KeyStore::new(flash.clone()).sealed_by(Box::new(BrokenChip));
    let result = keys.write_tree(0, &SEED, 0, 0, &tree_of(&SEED));
    assert_eq!(result, Err(StoreError));
    assert!(!flash.contains(&SEED));
}

#[test]
fn with_no_chip_key_the_seed_is_stored_as_before() {
    let flash = SimFlash::new();
    plain(&flash)
        .write_tree(0, &SEED, 0, 0, &tree_of(&SEED))
        .expect("the flash is healthy");
    assert_eq!(flash.peek(SEED_AT, 32), SEED);
    assert_eq!(flash.peek(layout::KEY_SLOTS as usize, 8), b"QDAYKEY1");
}

#[test]
fn a_sealed_store_ignores_a_plain_key() {
    let flash = SimFlash::new();
    let mut keys = plain(&flash);
    keys.write_tree(0, &SEED, 0, 0, &tree_of(&SEED))
        .expect("the flash is healthy");
    keys.activate(0, 1, 0).expect("the flash is healthy");
    let slots = sealed(&flash, CHIP).scan().expect("the flash is healthy");
    assert_eq!(slots.active, None);
    assert_eq!(slots.prepared, None);
}

#[test]
fn a_plain_store_ignores_a_sealed_key() {
    let flash = SimFlash::new();
    let mut keys = sealed(&flash, CHIP);
    keys.write_tree(0, &SEED, 0, 0, &tree_of(&SEED))
        .expect("the flash is healthy");
    keys.activate(0, 1, 0).expect("the flash is healthy");
    let slots = plain(&flash).scan().expect("the flash is healthy");
    assert_eq!(slots.active, None);
}

#[test]
fn a_sealed_passcode_works_and_counts_failures() {
    let flash = SimFlash::new();
    let mut passcode = sealed_pin(&flash, CHIP);
    passcode
        .create(&CODE, &mut TestRng(1))
        .expect("the flash is healthy");
    assert!(matches!(
        passcode.attempt(&CODE, 0),
        Ok(Attempt::Correct(_))
    ));
    assert!(matches!(
        passcode.attempt(&WRONG, 0),
        Ok(Attempt::Wrong { failures: 1, .. })
    ));
}

#[test]
fn a_flash_copy_cant_be_used_to_guess_a_sealed_passcode() {
    let flash = SimFlash::new();
    sealed_pin(&flash, CHIP)
        .create(&CODE, &mut TestRng(1))
        .expect("the flash is healthy");
    let salt = flash.peek(layout::PASSCODE as usize + 16, 16);
    let digits = CODE.map(|digit| b'0' + digit);
    let unkeyed = keccak256(&[b"QDAY-PIN-V1", &salt, &digits]);
    assert!(!flash.contains(&unkeyed));
    assert!(matches!(
        sealed_pin(&flash, OTHER_CHIP).attempt(&CODE, 0),
        Ok(Attempt::Wrong { .. })
    ));
}

#[test]
fn a_plain_passcode_written_into_the_flash_is_not_accepted() {
    let flash = SimFlash::new();
    pin(&flash)
        .create(&CODE, &mut TestRng(1))
        .expect("the flash is healthy");
    assert_eq!(sealed_pin(&flash, CHIP).exists(), Ok(false));
}

#[test]
fn with_no_chip_key_the_passcode_is_stored_as_before() {
    let flash = SimFlash::new();
    pin(&flash)
        .create(&CODE, &mut TestRng(1))
        .expect("the flash is healthy");
    assert_eq!(flash.peek(layout::PASSCODE as usize, 8), b"QDAYPIN1");
}

/// A sealed wallet with a key and a passcode.
fn sealed_wallet() -> SimFlash {
    let flash = SimFlash::new();
    let mut wallet = open_sealed(&flash);
    wallet
        .passcode()
        .create(&CODE, &mut TestRng(1))
        .expect("the flash is healthy");
    wallet
        .create(&mut TestRng(2), |_| {})
        .expect("the flash is healthy");
    flash
}

#[test]
fn a_sealed_wallet_survives_a_restart() {
    let flash = sealed_wallet();
    let wallet = open_sealed(&flash);
    assert!(wallet.is_sealed());
    assert!(wallet.has_key());
}

#[test]
fn erasing_the_passcode_loses_the_keys_instead_of_handing_them_over() {
    let flash = sealed_wallet();
    let erased = vec![0xFF; 0x2000];
    flash.poke(layout::PASSCODE as usize, &erased);
    let wallet = open_sealed(&flash);
    assert!(!wallet.has_key());
    let header = flash.peek(layout::KEY_SLOTS as usize, 8);
    assert_eq!(header, [0xFF; 8]);
}

#[test]
fn a_plain_wallet_keeps_its_keys_while_it_waits_for_a_passcode() {
    let flash = SimFlash::new();
    open_plain(&flash)
        .create(&mut TestRng(2), |_| {})
        .expect("the flash is healthy");
    let wallet = open_plain(&flash);
    assert!(!wallet.is_sealed());
    assert!(wallet.has_key());
}
