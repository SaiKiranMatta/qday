//! The passcode, with failure counts that a power cut can't dodge.

use alloc::boxed::Box;

use qday_xmss::{Hash, keccak256};

use crate::flash::{Flash, SECTOR};
use crate::keystore::StoreError;
use crate::rng::Rng;
use crate::seal::Sealer;

/// Digits in a passcode.
pub const CODE_DIGITS: usize = 6;
/// The failure that erases the wallet.
pub const MAX_FAILURES: u32 = 10;

/// A passcode: each digit is 0 to 9.
pub type Code = [u8; CODE_DIGITS];

const MAGIC: &[u8; 8] = b"QDAYPIN1";
/// A record whose hash needs the chip's key, so a copy of the flash can't be tested against
/// guesses and a record for a known code can't be written in from outside.
const SEALED_MAGIC: &[u8; 8] = b"QDAYPIN2";
const DOMAIN: &[u8] = b"QDAY-PIN-V1";
const SEALED_DOMAIN: &[u8] = b"QDAY-PIN-V2";
const SALT_BYTES: usize = 16;
/// Bytes of a record covered by its check.
const RECORD_BODY: usize = 64;
/// A whole record: the body, then the keccak256 of the body.
const RECORD: usize = RECORD_BODY + 32;
/// Marks start here in a sector, each entry being a FAIL byte then an OK byte.
const MARKS_AT: usize = 128;
const MARK_ENTRIES: usize = (SECTOR as usize - MARKS_AT) / 2;
const BLANK: u8 = 0xFF;
/// The only value that counts as OK, so a torn mark stays a failure.
const OK_MARK: u8 = 0;
const FAIL_MARK: u8 = 0;

const MINUTE_MS: u64 = 60_000;
/// Waits after failures 3 to 9.
const WAITS_MS: [u64; 7] = [
    MINUTE_MS / 2,
    MINUTE_MS,
    2 * MINUTE_MS,
    5 * MINUTE_MS,
    15 * MINUTE_MS,
    30 * MINUTE_MS,
    60 * MINUTE_MS,
];

/// The current record and what its marks add up to.
struct Loaded {
    sector: u32,
    sequence: u32,
    salt: [u8; SALT_BYTES],
    hash: Hash,
    failures: u32,
    /// The first unused mark entry, if the sector has one.
    next_entry: Option<usize>,
}

fn same_in_constant_time(left: &Hash, right: &Hash) -> bool {
    let difference = left.iter().zip(right).fold(0, |all, (a, b)| all | (a ^ b));
    difference == 0
}

fn record_bytes(
    magic: [u8; 8],
    sequence: u32,
    failures: u32,
    salt: &[u8; SALT_BYTES],
    hash: &Hash,
) -> [u8; RECORD] {
    let mut bytes = [0; RECORD];
    bytes[..8].copy_from_slice(&magic);
    bytes[8..12].copy_from_slice(&sequence.to_be_bytes());
    bytes[12..16].copy_from_slice(&failures.to_be_bytes());
    bytes[16..32].copy_from_slice(salt);
    bytes[32..RECORD_BODY].copy_from_slice(hash);
    let check = keccak256(&[&bytes[..RECORD_BODY]]);
    bytes[RECORD_BODY..].copy_from_slice(&check);
    bytes
}

/// Proof that the passcode was just entered. Each one allows one protected action.
#[derive(Debug, PartialEq, Eq)]
pub struct Proof(());

impl Proof {
    pub(crate) fn new() -> Self {
        Self(())
    }
}

/// The result of entering a passcode.
#[derive(Debug, PartialEq, Eq)]
pub enum Attempt {
    /// The code was right.
    Correct(Proof),
    /// The code was wrong.
    Wrong {
        /// Failures in a row, including this one.
        failures: u32,
        /// Tries left before the wallet erases itself.
        tries_left: u32,
        /// How long entry is now locked for.
        wait_ms: u64,
    },
    /// Entry is locked, and the code was not checked.
    Locked {
        /// How long is left.
        remaining_ms: u64,
    },
    /// That was the last try. The caller must erase the wallet.
    Exhausted,
}

/// How long entry locks after `failures` failures in a row.
#[must_use]
pub fn wait_after(failures: u32) -> u64 {
    match failures {
        0..=2 => 0,
        3..=9 => WAITS_MS[failures as usize - 3],
        _ => WAITS_MS[WAITS_MS.len() - 1],
    }
}

/// Shuffles the digits 0 to 9 for a keypad, with Fisher-Yates.
pub fn shuffled_digits(rng: &mut impl Rng) -> [u8; 10] {
    let mut digits = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    for last in (1_u32..10).rev() {
        let other = rng.below(last + 1);
        digits.swap(last as usize, other as usize);
    }
    digits
}

/// The stored passcode and its failure log.
pub struct Passcode<F> {
    flash: F,
    base: u32,
    locked_until: u64,
    sealer: Option<Box<dyn Sealer>>,
}

impl<F: Flash> Passcode<F> {
    /// A passcode store whose two sectors start at `base`.
    pub fn new(flash: F, base: u32) -> Self {
        Self {
            flash,
            base,
            locked_until: 0,
            sealer: None,
        }
    }

    /// Hashes passcodes with the chip's key. A plain record then doesn't count as a passcode.
    #[must_use]
    pub fn sealed_by(mut self, sealer: Box<dyn Sealer>) -> Self {
        self.sealer = Some(sealer);
        self
    }

    fn magic(&self) -> [u8; 8] {
        if self.sealer.is_some() {
            *SEALED_MAGIC
        } else {
            *MAGIC
        }
    }

    fn hash_code(&mut self, salt: &[u8; SALT_BYTES], code: Code) -> Result<Hash, StoreError> {
        let digits = code.map(|digit| b'0' + digit);
        let Some(sealer) = &mut self.sealer else {
            return Ok(keccak256(&[DOMAIN, salt, &digits]));
        };
        let mut message = [0; SEALED_DOMAIN.len() + SALT_BYTES + CODE_DIGITS];
        let (domain, rest) = message.split_at_mut(SEALED_DOMAIN.len());
        let (salt_part, digit_part) = rest.split_at_mut(SALT_BYTES);
        domain.copy_from_slice(SEALED_DOMAIN);
        salt_part.copy_from_slice(salt);
        digit_part.copy_from_slice(&digits);
        sealer.keyed_hash(&message).map_err(|_| StoreError)
    }

    fn fresh_hash(
        &mut self,
        code: Code,
        rng: &mut impl Rng,
    ) -> Result<([u8; SALT_BYTES], Hash), StoreError> {
        let mut salt = [0; SALT_BYTES];
        rng.fill(&mut salt);
        let hash = self.hash_code(&salt, code)?;
        Ok((salt, hash))
    }

    /// Whether a passcode has been created.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn exists(&mut self) -> Result<bool, StoreError> {
        Ok(self.load()?.is_some())
    }

    /// Failures in a row so far.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn failures(&mut self) -> Result<u32, StoreError> {
        Ok(self.load()?.map_or(0, |loaded| loaded.failures))
    }

    /// Restarts any wait after a restart, because time isn't kept across one.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn on_boot(&mut self, now_ms: u64) -> Result<(), StoreError> {
        self.locked_until = now_ms + wait_after(self.failures()?);
        Ok(())
    }

    /// How long entry stays locked.
    #[must_use]
    pub fn locked_for(&self, now_ms: u64) -> u64 {
        self.locked_until.saturating_sub(now_ms)
    }

    /// Creates the first passcode.
    ///
    /// # Errors
    ///
    /// Fails if one exists already, or if it can't be stored.
    pub fn create(&mut self, code: &Code, rng: &mut impl Rng) -> Result<(), StoreError> {
        if self.exists()? {
            return Err(StoreError);
        }
        let (salt, hash) = self.fresh_hash(*code, rng)?;
        self.erase(0)?;
        self.write_record(0, 1, 0, &salt, &hash)
    }

    /// Replaces the passcode. Needs proof of the old one.
    ///
    /// # Errors
    ///
    /// Fails if the new one can't be stored.
    pub fn change(
        &mut self,
        _proof: Proof,
        code: &Code,
        rng: &mut impl Rng,
    ) -> Result<(), StoreError> {
        let loaded = self.load()?.ok_or(StoreError)?;
        let (salt, hash) = self.fresh_hash(*code, rng)?;
        self.move_to_other_sector(&loaded, 0, &salt, &hash)
    }

    /// Checks a code. The attempt is counted as a failure before the comparison.
    ///
    /// # Errors
    ///
    /// Fails, without comparing, if the attempt can't be recorded.
    pub fn attempt(&mut self, code: &Code, now_ms: u64) -> Result<Attempt, StoreError> {
        let remaining_ms = self.locked_for(now_ms);
        if remaining_ms > 0 {
            return Ok(Attempt::Locked { remaining_ms });
        }
        let mut loaded = self.load()?.ok_or(StoreError)?;
        if loaded.failures >= MAX_FAILURES {
            return Ok(Attempt::Exhausted);
        }
        if loaded.next_entry.is_none() {
            let (salt, hash) = (loaded.salt, loaded.hash);
            self.move_to_other_sector(&loaded, loaded.failures, &salt, &hash)?;
            loaded = self.load()?.ok_or(StoreError)?;
        }
        let entry = loaded.next_entry.ok_or(StoreError)?;
        let mark_at = self.mark_at(loaded.sector, entry)?;

        // The failure is on record before anyone can learn whether the code was right.
        self.write_mark(mark_at, FAIL_MARK)?;

        let entered = self.hash_code(&loaded.salt, *code)?;
        if same_in_constant_time(&entered, &loaded.hash) {
            self.write_mark(mark_at + 1, OK_MARK)?;
            self.locked_until = 0;
            return Ok(Attempt::Correct(Proof::new()));
        }

        let failures = loaded.failures + 1;
        if failures >= MAX_FAILURES {
            return Ok(Attempt::Exhausted);
        }
        let wait_ms = wait_after(failures);
        self.locked_until = now_ms + wait_ms;
        Ok(Attempt::Wrong {
            failures,
            tries_left: MAX_FAILURES - failures,
            wait_ms,
        })
    }

    /// Erases the passcode and its failure log. The older sector goes first, so a cut can't bring
    /// back a stale record with a lower count.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be erased.
    pub fn wipe(&mut self) -> Result<(), StoreError> {
        let current = self.load()?.map_or(0, |loaded| loaded.sector);
        self.erase(1 - current)?;
        self.erase(current)?;
        self.locked_until = 0;
        Ok(())
    }

    fn sector_at(&self, sector: u32) -> u32 {
        self.base + sector * SECTOR
    }

    fn mark_at(&self, sector: u32, entry: usize) -> Result<u32, StoreError> {
        let offset = u32::try_from(MARKS_AT + entry * 2).map_err(|_| StoreError)?;
        Ok(self.sector_at(sector) + offset)
    }

    fn erase(&mut self, sector: u32) -> Result<(), StoreError> {
        let offset = self.sector_at(sector);
        self.flash.erase(offset).map_err(|_| StoreError)
    }

    /// Writes one mark and proves it landed.
    fn write_mark(&mut self, offset: u32, mark: u8) -> Result<(), StoreError> {
        self.flash.write(offset, &[mark]).map_err(|_| StoreError)?;
        let mut stored = [BLANK];
        self.flash
            .read(offset, &mut stored)
            .map_err(|_| StoreError)?;
        if stored[0] == mark {
            Ok(())
        } else {
            Err(StoreError)
        }
    }

    fn write_record(
        &mut self,
        sector: u32,
        sequence: u32,
        failures: u32,
        salt: &[u8; SALT_BYTES],
        hash: &Hash,
    ) -> Result<(), StoreError> {
        let offset = self.sector_at(sector);
        let bytes = record_bytes(self.magic(), sequence, failures, salt, hash);
        self.flash.write(offset, &bytes).map_err(|_| StoreError)
    }

    /// Starts a fresh log in the other sector. The old one stays valid until the new record is
    /// whole, so a cut can't lose the code or the count.
    fn move_to_other_sector(
        &mut self,
        loaded: &Loaded,
        failures: u32,
        salt: &[u8; SALT_BYTES],
        hash: &Hash,
    ) -> Result<(), StoreError> {
        let other = 1 - loaded.sector;
        self.erase(other)?;
        self.write_record(other, loaded.sequence + 1, failures, salt, hash)?;
        self.erase(loaded.sector)
    }

    fn load(&mut self) -> Result<Option<Loaded>, StoreError> {
        let mut newest: Option<(u32, [u8; RECORD])> = None;
        for sector in 0..2 {
            let mut bytes = [0; RECORD];
            let offset = self.sector_at(sector);
            self.flash
                .read(offset, &mut bytes)
                .map_err(|_| StoreError)?;
            let valid = bytes.starts_with(&self.magic())
                && keccak256(&[&bytes[..RECORD_BODY]])[..] == bytes[RECORD_BODY..];
            if valid && newest.is_none_or(|(_, best)| sequence_of(&bytes) > sequence_of(&best)) {
                newest = Some((sector, bytes));
            }
        }
        let Some((sector, bytes)) = newest else {
            return Ok(None);
        };
        let mut failures = u32::from_be_bytes(word(&bytes, 12));
        let mut next_entry = 0;

        let mut marks = [BLANK; SECTOR as usize - MARKS_AT];
        let offset = self.mark_at(sector, 0)?;
        self.flash
            .read(offset, &mut marks)
            .map_err(|_| StoreError)?;
        for (entry, pair) in marks.chunks_exact(2).enumerate() {
            if pair == [BLANK, BLANK] {
                continue;
            }
            next_entry = entry + 1;
            if pair[0] != BLANK {
                failures += 1;
            }
            if pair[1] == OK_MARK {
                failures = 0;
            }
        }

        let mut salt = [0; SALT_BYTES];
        salt.copy_from_slice(&bytes[16..32]);
        let mut hash = [0; 32];
        hash.copy_from_slice(&bytes[32..RECORD_BODY]);
        Ok(Some(Loaded {
            sector,
            sequence: sequence_of(&bytes),
            salt,
            hash,
            failures,
            next_entry: (next_entry < MARK_ENTRIES).then_some(next_entry),
        }))
    }
}

fn word(bytes: &[u8; RECORD], at: usize) -> [u8; 4] {
    [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
}

fn sequence_of(bytes: &[u8; RECORD]) -> u32 {
    u32::from_be_bytes(word(bytes, 8))
}
