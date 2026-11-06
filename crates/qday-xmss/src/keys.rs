//! The per-tree secrets, and the one-time keys they generate.

use crate::adrs::Adrs;
use crate::hash::{Hash, keccak256};
use crate::wots::{CHAINS, MAX_DIGIT, advance, leaf_hash};

/// The three values derived from a seed and a tree number. Deliberately not `Debug`.
pub struct KeySet {
    sk_seed: Hash,
    pub_seed: Hash,
    pub(crate) prf_key: Hash,
}

impl KeySet {
    /// Derives tree number `tree` from a seed.
    #[must_use]
    pub fn derive(seed: &Hash, tree: u32) -> Self {
        let tree = tree.to_be_bytes();
        let part = |tag: u8| keccak256(&[&[tag], seed, &tree]);
        Self {
            sk_seed: part(0),
            pub_seed: part(1),
            prf_key: part(2),
        }
    }

    /// The public seed, which forms half of the public key.
    #[must_use]
    pub fn pub_seed(&self) -> &Hash {
        &self.pub_seed
    }

    /// Computes the leaf of one-time key `leaf`.
    #[must_use]
    pub fn leaf(&self, leaf: u32) -> Hash {
        let mut ends = [[0; 32]; CHAINS];
        for (chain, end) in (0..).zip(&mut ends) {
            let start = self.chain_start(leaf, chain);
            *end = advance(&self.pub_seed, leaf, chain, 0, MAX_DIGIT, &start);
        }
        leaf_hash(&self.pub_seed, leaf, &ends)
    }

    /// The secret value at step 0 of a chain.
    pub(crate) fn chain_start(&self, leaf: u32, chain: u32) -> Hash {
        keccak256(&[&self.sk_seed, &Adrs::secret(leaf, chain)])
    }
}
