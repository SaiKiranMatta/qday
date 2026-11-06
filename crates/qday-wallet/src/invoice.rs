//! Payments to one-time hash keys. Each invoice is a fresh key from its own number, so nothing
//! public links two of them, and it's spent whole, once, through the pool contract.

use qday_xmss::{Hash, KeySet, keccak256};

use crate::flash::{Flash, SECTOR, layout};
use crate::frame::{Address, CHAIN_ID};
use crate::keystore::StoreError;

/// Keeps invoice keys apart from every account's trees, whatever their numbers.
const INVOICE_TAG: Hash = *b"QDAY-INVOICE-KEY-V1\0\0\0\0\0\0\0\0\0\0\0\0\0";

const WORD: usize = 32;
const LEAF_AT: usize = 0;
const MESSAGE_AT: usize = WORD;
const CHECK_AT: usize = 2 * WORD;
const CHECK_BYTES: usize = 4;
const RECORD: usize = CHECK_AT + CHECK_BYTES;
const STRIDE: u32 = 68;
const _: () = assert!(STRIDE as usize == RECORD);
const RECORDS_PER_SECTOR: u32 = 60;
const ERASED: u8 = 0xFF;

/// What a sender needs to pay an invoice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invoice {
    /// The invoice's number, which only the recipient needs.
    pub number: u32,
    /// The key, which is what the pool's note is held under.
    pub leaf: Hash,
    /// The key's public seed.
    pub pub_seed: Hash,
}

/// A request to spend an invoice's note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spend {
    /// The invoice.
    pub number: u32,
    /// The pool contract.
    pub pool: Address,
    /// Who gets the note.
    pub to: Address,
    /// What whoever submits the spend gets out of the note, in wei.
    pub fee: u128,
}

/// The key set for an invoice.
#[must_use]
pub fn keys_for(seed: &Hash, account: u32, number: u32) -> KeySet {
    let invoices = keccak256(&[&INVOICE_TAG, seed, &account.to_be_bytes()]);
    KeySet::derive(&invoices, number)
}

fn word_of_address(address: &Address) -> Hash {
    let mut word = [0; WORD];
    word[WORD - address.len()..].copy_from_slice(address);
    word
}

fn word_of_number(value: u128) -> Hash {
    let mut word = [0; WORD];
    word[WORD - 16..].copy_from_slice(&value.to_be_bytes());
    word
}

/// What a spend signs, as the pool's `spendHash` computes it.
#[must_use]
pub fn spend_hash(pool: &Address, leaf: &Hash, to: &Address, fee: u128) -> Hash {
    keccak256(&[
        &word_of_number(u128::from(CHAIN_ID)),
        &word_of_address(pool),
        leaf,
        &word_of_address(to),
        &word_of_number(fee),
    ])
}

/// A record of which message each spent key signed, because a key must never sign two.
pub struct SpendLog<F> {
    flash: F,
}

/// What the log says about a key and a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// The key may sign this message, and now never another.
    Recorded,
    /// The key has signed a different message.
    Used,
    /// There's no room left to record it.
    Full,
}

impl<F: Flash> SpendLog<F> {
    /// A log on `flash`.
    pub fn new(flash: F) -> Self {
        Self { flash }
    }

    /// Where record `index` lives.
    fn offset(index: u32) -> u32 {
        let sector = index / RECORDS_PER_SECTOR;
        let slot = index % RECORDS_PER_SECTOR;
        layout::SPENT_INVOICES + sector * SECTOR + slot * STRIDE
    }

    /// Records that `leaf` signs `message`, unless it has already signed another. A damaged record
    /// takes up its place and counts for nothing, because nothing was signed after it.
    ///
    /// # Errors
    ///
    /// Fails if the record can't be stored, so nothing may be signed.
    pub fn record(&mut self, leaf: &Hash, message: &Hash) -> Result<Entry, StoreError> {
        let mut free = None;
        for index in 0..RECORDS_PER_SECTOR * layout::SPENT_INVOICE_SECTORS {
            let mut record = [0; RECORD];
            self.flash
                .read(Self::offset(index), &mut record)
                .map_err(|_| StoreError)?;
            if record.iter().all(|&byte| byte == ERASED) {
                free = Some(index);
                break;
            }
            let whole = check_of(&record)[..] == record[CHECK_AT..];
            if whole && record[LEAF_AT..MESSAGE_AT] == *leaf {
                return Ok(if record[MESSAGE_AT..CHECK_AT] == *message {
                    Entry::Recorded
                } else {
                    Entry::Used
                });
            }
        }
        let Some(index) = free else {
            return Ok(Entry::Full);
        };

        let mut record = [0; RECORD];
        record[LEAF_AT..MESSAGE_AT].copy_from_slice(leaf);
        record[MESSAGE_AT..CHECK_AT].copy_from_slice(message);
        let check = check_of(&record);
        record[CHECK_AT..].copy_from_slice(&check);
        self.flash
            .write(Self::offset(index), &record)
            .map_err(|_| StoreError)?;

        // A signature may only follow a record that's really there.
        let mut stored = [0; RECORD];
        self.flash
            .read(Self::offset(index), &mut stored)
            .map_err(|_| StoreError)?;
        if stored == record {
            Ok(Entry::Recorded)
        } else {
            Err(StoreError)
        }
    }

    /// Erases the log.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be erased.
    pub fn wipe(&mut self) -> Result<(), StoreError> {
        for sector in 0..layout::SPENT_INVOICE_SECTORS {
            self.flash
                .erase(layout::SPENT_INVOICES + sector * SECTOR)
                .map_err(|_| StoreError)?;
        }
        Ok(())
    }
}

fn check_of(record: &[u8; RECORD]) -> [u8; CHECK_BYTES] {
    let hash = keccak256(&[&record[..CHECK_AT]]);
    let mut check = [0; CHECK_BYTES];
    check.copy_from_slice(&hash[..CHECK_BYTES]);
    check
}

#[cfg(test)]
mod tests {
    use super::{keys_for, spend_hash};
    use crate::hex;

    #[test]
    fn the_spend_hash_matches_the_contract_encoding() {
        let mut pool = [0; 20];
        pool[19] = 0xAA;
        let mut to = [0; 20];
        to[19] = 0xBB;
        assert_eq!(
            hex::encode(&spend_hash(&pool, &[0x11; 32], &to, 1000)),
            "0x9a5987d1c61e040b708221e73aadc74ec92e26227e911a8e11aca4d1fa9a0432"
        );
    }

    #[test]
    fn invoices_differ_by_number() {
        let seed = [1; 32];
        assert_ne!(keys_for(&seed, 0, 1).leaf(0), keys_for(&seed, 0, 2).leaf(0));
    }

    #[test]
    fn invoices_differ_by_account() {
        let seed = [1; 32];
        assert_ne!(keys_for(&seed, 0, 1).leaf(0), keys_for(&seed, 1, 1).leaf(0));
    }

    #[test]
    fn invoices_differ_by_seed() {
        assert_ne!(
            keys_for(&[1; 32], 0, 1).leaf(0),
            keys_for(&[2; 32], 0, 1).leaf(0)
        );
    }

    #[test]
    fn an_invoice_key_is_not_an_account_key() {
        let seed = [1; 32];
        for number in 0..3 {
            let account = qday_xmss::KeySet::derive(&seed, number);
            assert_ne!(keys_for(&seed, 0, number).leaf(0), account.leaf(0));
        }
    }
}
