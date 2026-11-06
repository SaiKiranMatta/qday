//! Signing and verifying.

use crate::hash::{Hash, keccak256};
use crate::keys::KeySet;
use crate::tree::{HEIGHT, LEAVES, parent};
use crate::wots::{CHAINS, MAX_DIGIT, advance, digits, leaf_hash};

/// Length of an encoded signature.
pub const SIG_BYTES: usize = 2500;

/// Where each part of the encoding starts.
const R_AT: usize = 4;
const CHAINS_AT: usize = R_AT + 32;
const AUTH_AT: usize = CHAINS_AT + CHAINS * 32;

/// A tree's public key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicKey {
    /// The tree root.
    pub root: Hash,
    /// The public seed.
    pub pub_seed: Hash,
}

/// Why a signature could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignError {
    /// The index is not a key in this tree.
    IndexOutOfRange,
}

impl KeySet {
    /// Signs `message` with one-time key `index`. The caller owns index safety: this never picks
    /// or remembers an index, and signing twice with one index breaks the scheme.
    ///
    /// # Errors
    ///
    /// Returns [`SignError::IndexOutOfRange`] if `index` is not below `LEAVES`.
    pub fn sign(
        &self,
        root: &Hash,
        index: u32,
        message: &Hash,
        auth: &[Hash; HEIGHT],
    ) -> Result<[u8; SIG_BYTES], SignError> {
        if index >= LEAVES {
            return Err(SignError::IndexOutOfRange);
        }
        let wide_index = u256(index);
        let r = keccak256(&[&self.prf_key, &wide_index]);
        let digest = message_digest(&r, root, &wide_index, message);

        let mut out = [0; SIG_BYTES];
        out[..R_AT].copy_from_slice(&index.to_be_bytes());
        out[R_AT..CHAINS_AT].copy_from_slice(&r);
        let slots = out[CHAINS_AT..AUTH_AT].chunks_exact_mut(32);
        for ((chain, slot), digit) in (0..).zip(slots).zip(digits(&digest)) {
            let start = self.chain_start(index, chain);
            let value = advance(self.pub_seed(), index, chain, 0, digit, &start);
            slot.copy_from_slice(&value);
        }
        out[AUTH_AT..].copy_from_slice(auth.as_flattened());
        Ok(out)
    }
}

/// Checks a signature. Anything malformed is simply not valid.
#[must_use]
pub fn verify(key: &PublicKey, message: &Hash, signature: &[u8]) -> bool {
    let Ok(signature) = <&[u8; SIG_BYTES]>::try_from(signature) else {
        return false;
    };
    let index = u32::from_be_bytes([signature[0], signature[1], signature[2], signature[3]]);
    if index >= LEAVES {
        return false;
    }
    let r = hash_at(signature, R_AT);
    let digest = message_digest(&r, &key.root, &u256(index), message);

    let mut ends = [[0; 32]; CHAINS];
    for (chain, (end, digit)) in (0..).zip(ends.iter_mut().zip(digits(&digest))) {
        let value = hash_at(signature, CHAINS_AT + chain as usize * 32);
        *end = advance(
            &key.pub_seed,
            index,
            chain,
            digit,
            MAX_DIGIT - digit,
            &value,
        );
    }

    let mut node = leaf_hash(&key.pub_seed, index, &ends);
    for (level, height) in (0_u32..).zip(0..HEIGHT) {
        let sibling = hash_at(signature, AUTH_AT + height * 32);
        let up = index >> (level + 1);
        node = if (index >> level) & 1 == 0 {
            parent(&key.pub_seed, level + 1, up, &node, &sibling)
        } else {
            parent(&key.pub_seed, level + 1, up, &sibling, &node)
        };
    }
    node == key.root
}

/// The digest that is actually signed, bound to this tree and this key.
fn message_digest(r: &Hash, root: &Hash, wide_index: &Hash, message: &Hash) -> Hash {
    keccak256(&[r, root, wide_index, message])
}

/// A key index as a 32-byte big-endian integer.
fn u256(index: u32) -> Hash {
    let mut out = [0; 32];
    out[28..].copy_from_slice(&index.to_be_bytes());
    out
}

/// Copies the 32 bytes at `offset`.
fn hash_at(signature: &[u8; SIG_BYTES], offset: usize) -> Hash {
    let mut out = [0; 32];
    out.copy_from_slice(&signature[offset..offset + 32]);
    out
}
