//! The derived ML-DSA public key, kept in flash so showing it needs no passcode. It's public, so
//! keeping it is safe. The secret key is never stored.

use qday_mldsa::{PUBLIC_KEY_BYTES, PublicKey};
use qday_xmss::keccak256;

use crate::flash::{Flash, layout};
use crate::keystore::StoreError;

const MAGIC: &[u8; 8] = b"QDMLPUB1";
const ACCOUNT_AT: usize = 8;
const KEY_AT: usize = 12;
const CHECK_AT: usize = KEY_AT + PUBLIC_KEY_BYTES;
const RECORD: usize = CHECK_AT + 32;

/// Reads and writes the stored public key.
pub struct MlKeyStore<F> {
    flash: F,
}

impl<F: Flash> MlKeyStore<F> {
    /// A store on `flash`.
    pub fn new(flash: F) -> Self {
        Self { flash }
    }

    /// The stored key for `account`, if there is a whole one.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn load(&mut self, account: u32) -> Result<Option<PublicKey>, StoreError> {
        let mut record = [0; RECORD];
        self.flash
            .read(layout::MLDSA_PUBLIC, &mut record)
            .map_err(|_| StoreError)?;
        let whole = record.starts_with(MAGIC)
            && keccak256(&[&record[..CHECK_AT]])[..] == record[CHECK_AT..]
            && record[ACCOUNT_AT..KEY_AT] == account.to_be_bytes();
        if !whole {
            return Ok(None);
        }
        let mut key = [0; PUBLIC_KEY_BYTES];
        key.copy_from_slice(&record[KEY_AT..CHECK_AT]);
        Ok(Some(key))
    }

    /// Stores a key, replacing any other.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be written.
    pub fn store(&mut self, account: u32, key: &PublicKey) -> Result<(), StoreError> {
        let mut record = [0; RECORD];
        record[..ACCOUNT_AT].copy_from_slice(MAGIC);
        record[ACCOUNT_AT..KEY_AT].copy_from_slice(&account.to_be_bytes());
        record[KEY_AT..CHECK_AT].copy_from_slice(key);
        let check = keccak256(&[&record[..CHECK_AT]]);
        record[CHECK_AT..].copy_from_slice(&check);
        self.wipe()?;
        self.flash
            .write(layout::MLDSA_PUBLIC, &record)
            .map_err(|_| StoreError)
    }

    /// Erases the stored key.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be erased.
    pub fn wipe(&mut self) -> Result<(), StoreError> {
        self.flash
            .erase(layout::MLDSA_PUBLIC)
            .map_err(|_| StoreError)
    }
}
