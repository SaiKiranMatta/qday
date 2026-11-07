//! Sealing: a key held inside the chip, so a copy of the flash gives nothing away.
//!
//! The `qday` partition can't use the chip's flash encryption. The counter and the passcode log
//! work by clearing bits in place, and encrypted flash can't do that: it's written in whole
//! blocks, and its driver erases a whole sector to do it, which would undo every power-cut
//! guarantee. So the two secrets in that partition are protected on their own instead. The seed
//! is stored masked and the passcode is hashed with the chip's key. See `docs/lockdown.md`.

use qday_xmss::Hash;

/// The chip's key couldn't be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SealError;

/// A keyed hash whose key can be used but never read.
pub trait Sealer {
    /// Hashes `message` with the chip's key.
    ///
    /// # Errors
    ///
    /// Fails if the key can't be used. Callers must stop, and must never fall back to storing a
    /// secret unprotected.
    fn keyed_hash(&mut self, message: &[u8]) -> Result<Hash, SealError>;
}
