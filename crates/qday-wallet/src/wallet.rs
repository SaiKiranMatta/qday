//! The wallet's rules, on top of the counter, key slots and passcode.

use qday_xmss::{Hash, KeySet, LEAVES, PublicKey, SIG_BYTES, Tree};

use crate::counter::OneWayCounter;
use crate::flash::Flash;
use crate::frame::{Action, FrameTx};
use crate::keystore::{ActiveKey, KeyStore, PreparedKey, Slots, StoreError};
use crate::passcode::{Attempt, Code, MAX_FAILURES, Passcode, Proof};
use crate::rng::Rng;

/// From this key on, the device builds the next tree without being asked.
pub const PREPARE_FROM: u32 = 900;
/// From this key on, only rotations can be signed.
pub const RESERVED_FROM: u32 = 1008;
/// Keys a restored device skips past the chain's `nextIndex`.
pub const RESTORE_SKIP: u32 = 64;

/// Why a request failed. Each one has a code on the USB line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletError {
    /// The request is malformed, or isn't a transaction this device will sign.
    BadRequest,
    /// The user refused, or didn't decide in time.
    Rejected,
    /// There are no keys left in this tree.
    KeysExhausted,
    /// The counter can't be trusted, so nothing was signed.
    CounterFailure,
    /// Only a rotation can be signed now.
    RotationNeeded,
    /// No prepared tree matches.
    NothingPending,
    /// Flash failed.
    StorageFailure,
}

impl From<StoreError> for WalletError {
    fn from(_: StoreError) -> Self {
        Self::StorageFailure
    }
}

/// What a transaction really does, as decoded by the device itself. Only `Wallet::review` makes
/// one, so `sign` can't be handed a transaction that skipped its rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    tx: FrameTx,
    message: Hash,
}

impl Review {
    /// The decoded transaction.
    #[must_use]
    pub fn tx(&self) -> &FrameTx {
        &self.tx
    }

    /// The hash the signature will cover.
    #[must_use]
    pub fn message(&self) -> &Hash {
        &self.message
    }
}

/// The wallet.
pub struct Wallet<F, C> {
    keys: KeyStore<F>,
    passcode: Passcode<F>,
    counter: C,
    slots: Slots,
}

impl<F: Flash, C: OneWayCounter> Wallet<F, C> {
    /// Opens the wallet, finishing an erase or a first activation that a power cut interrupted.
    ///
    /// # Errors
    ///
    /// Fails if storage can't be read.
    pub fn open(
        keys: KeyStore<F>,
        passcode: Passcode<F>,
        counter: C,
        now_ms: u64,
    ) -> Result<Self, WalletError> {
        let mut wallet = Self {
            keys,
            passcode,
            counter,
            slots: Slots::default(),
        };
        if wallet.passcode.failures()? >= MAX_FAILURES {
            wallet.erase()?;
        }
        wallet.passcode.on_boot(now_ms)?;
        wallet.slots = wallet.keys.scan()?;
        if let (None, Some(prepared)) = (wallet.slots.active, wallet.slots.prepared) {
            wallet.switch_to(prepared.slot)?;
        }
        Ok(wallet)
    }

    /// Whether a key is ready to use.
    #[must_use]
    pub fn has_key(&self) -> bool {
        self.slots.active.is_some()
    }

    /// Creates a new wallet from a random seed, reporting finished leaves.
    ///
    /// # Errors
    ///
    /// Fails if a wallet exists, or if storage fails.
    pub fn create(
        &mut self,
        rng: &mut impl Rng,
        progress: impl FnMut(u32),
    ) -> Result<(), WalletError> {
        if self.slots.active.is_some() || self.slots.prepared.is_some() {
            return Err(WalletError::StorageFailure);
        }
        let mut seed = [0; 32];
        rng.fill(&mut seed);
        let tree = Tree::build(&KeySet::derive(&seed, 0), progress);
        self.keys.write_tree(0, &seed, 0, 0, &tree)?;
        self.switch_to(0)
    }

    /// The passcode store.
    pub fn passcode(&mut self) -> &mut Passcode<F> {
        &mut self.passcode
    }

    /// How long passcode entry stays locked.
    #[must_use]
    pub fn locked_for(&self, now_ms: u64) -> u64 {
        self.passcode.locked_for(now_ms)
    }

    /// Enters a passcode, erasing the wallet on the last failure.
    ///
    /// # Errors
    ///
    /// Fails if storage fails.
    pub fn enter_passcode(&mut self, code: &Code, now_ms: u64) -> Result<Attempt, WalletError> {
        let attempt = self.passcode.attempt(code, now_ms)?;
        if attempt == Attempt::Exhausted {
            self.erase()?;
        }
        Ok(attempt)
    }

    /// Erases the keys, then the passcode. The failure count outlives the keys, so a power cut
    /// part way through finishes on the next start.
    fn erase(&mut self) -> Result<(), WalletError> {
        self.keys.wipe()?;
        self.slots = Slots::default();
        self.passcode.wipe()?;
        Ok(())
    }

    /// The active key.
    #[must_use]
    pub fn active(&self) -> Option<ActiveKey> {
        self.slots.active
    }

    /// The prepared next key.
    #[must_use]
    pub fn prepared(&self) -> Option<PreparedKey> {
        self.slots.prepared
    }

    /// The next one-time key.
    ///
    /// # Errors
    ///
    /// Fails if there is no key or the counter can't be read.
    pub fn next_index(&mut self) -> Result<u32, WalletError> {
        let active = self.slots.active.ok_or(WalletError::StorageFailure)?;
        let counter = self
            .counter
            .value()
            .map_err(|_| WalletError::CounterFailure)?;
        index_at(&active, counter)
    }

    /// Whether the next tree should be built now without being asked.
    ///
    /// # Errors
    ///
    /// Fails if the counter can't be read.
    pub fn should_prepare(&mut self) -> Result<bool, WalletError> {
        Ok(self.next_key_ready().is_none() && self.next_index()? >= PREPARE_FROM)
    }

    /// The prepared tree, if it is the one that follows the active tree.
    #[must_use]
    pub fn next_key_ready(&self) -> Option<PreparedKey> {
        let active = self.slots.active?;
        self.slots
            .prepared
            .filter(|prepared| prepared.tree == active.tree + 1)
    }

    /// Builds the next tree into the free slot. Does nothing if it's ready.
    ///
    /// # Errors
    ///
    /// Fails if there is no key, or if storage fails.
    pub fn prepare(&mut self, progress: impl FnMut(u32)) -> Result<PublicKey, WalletError> {
        if let Some(prepared) = self.next_key_ready() {
            return Ok(prepared.key);
        }
        let active = self.slots.active.ok_or(WalletError::StorageFailure)?;
        let seed = self.keys.seed(active.slot)?;
        let number = active.tree + 1;
        let tree = Tree::build(&KeySet::derive(&seed, number), progress);
        self.keys
            .write_tree(self.slots.free_slot(), &seed, number, 0, &tree)?;
        self.slots = self.keys.scan()?;
        let prepared = self.next_key_ready().ok_or(WalletError::StorageFailure)?;
        Ok(prepared.key)
    }

    /// Switches to the prepared tree. The caller restarts afterwards.
    ///
    /// # Errors
    ///
    /// Fails with `NothingPending` unless tree `tree` is prepared.
    pub fn activate(&mut self, tree: u32) -> Result<(), WalletError> {
        let prepared = self
            .next_key_ready()
            .filter(|prepared| prepared.tree == tree)
            .ok_or(WalletError::NothingPending)?;
        self.switch_to(prepared.slot)
    }

    /// Makes a slot active from the counter's current value.
    fn switch_to(&mut self, slot: u8) -> Result<(), WalletError> {
        let counter = self
            .counter
            .value()
            .map_err(|_| WalletError::CounterFailure)?;
        let serial = self.slots.active.map_or(1, |active| active.serial + 1);
        self.keys.activate(slot, serial, counter)?;
        self.slots = self.keys.scan()?;
        Ok(())
    }

    /// Decides whether a raw unsigned transaction may be shown for approval. Everything shown
    /// comes from the bytes that will be signed, never from a description.
    ///
    /// # Errors
    ///
    /// Fails with the code the laptop should get.
    pub fn review(&mut self, raw: &[u8]) -> Result<Review, WalletError> {
        let tx = FrameTx::decode_unsigned(raw).map_err(|_| WalletError::BadRequest)?;
        let next = self.next_index()?;
        if next >= LEAVES {
            return Err(WalletError::KeysExhausted);
        }
        if tx.index != next {
            return Err(WalletError::BadRequest);
        }
        match &tx.action {
            Action::Rotate {
                next_root,
                next_pub_seed,
            } => {
                let wanted = PublicKey {
                    root: *next_root,
                    pub_seed: *next_pub_seed,
                };
                if self
                    .next_key_ready()
                    .is_none_or(|prepared| prepared.key != wanted)
                {
                    return Err(WalletError::NothingPending);
                }
            }
            Action::Transfer { .. } | Action::Other { .. } => {
                if next >= RESERVED_FROM {
                    return Err(WalletError::RotationNeeded);
                }
            }
        }
        let message = tx.signing_message();
        Ok(Review { tx, message })
    }

    /// Advances the counter, then signs with the index that gave. Needs a fresh passcode proof.
    ///
    /// # Errors
    ///
    /// Fails, without signing, if the counter doesn't move or the state has changed.
    pub fn sign(&mut self, review: &Review, _proof: Proof) -> Result<[u8; SIG_BYTES], WalletError> {
        let active = self.slots.active.ok_or(WalletError::StorageFailure)?;
        let index = review.tx.index;
        if self.next_index()? != index {
            return Err(WalletError::BadRequest);
        }

        // Read everything first, so a storage failure can't burn a key.
        let seed = self.keys.seed(active.slot)?;
        let auth = self.keys.auth_path(active.slot, index)?;

        let counter = self
            .counter
            .advance()
            .map_err(|_| WalletError::CounterFailure)?;
        if index_at(&active, counter)? != index {
            return Err(WalletError::CounterFailure);
        }

        KeySet::derive(&seed, active.tree)
            .sign(&active.key.root, index, &review.message, &auth)
            .map_err(|_| WalletError::KeysExhausted)
    }

    /// Reads the seed for a backup. Needs a fresh passcode proof.
    ///
    /// # Errors
    ///
    /// Fails if there is no key or storage fails.
    pub fn seed(&mut self, _proof: Proof) -> Result<Hash, WalletError> {
        let active = self.slots.active.ok_or(WalletError::StorageFailure)?;
        Ok(self.keys.seed(active.slot)?)
    }

    /// Replaces the keys with `seed`, skipping past the keys the chain may have seen.
    ///
    /// # Errors
    ///
    /// Fails if the first index would be past the tree, or if storage fails.
    pub fn restore(
        &mut self,
        seed: &Hash,
        tree: u32,
        chain_next_index: u32,
        _proof: Proof,
        progress: impl FnMut(u32),
    ) -> Result<(), WalletError> {
        let first_index = chain_next_index
            .checked_add(RESTORE_SKIP)
            .filter(|&index| index < LEAVES)
            .ok_or(WalletError::KeysExhausted)?;
        let built = Tree::build(&KeySet::derive(seed, tree), progress);
        let slot = self.slots.free_slot();
        self.keys
            .write_tree(slot, seed, tree, first_index, &built)?;
        self.switch_to(slot)
    }
}

/// The key index a counter value stands for.
fn index_at(active: &ActiveKey, counter: u32) -> Result<u32, WalletError> {
    counter
        .checked_sub(active.counter_base)
        .and_then(|used| used.checked_add(active.first_index))
        .ok_or(WalletError::CounterFailure)
}
