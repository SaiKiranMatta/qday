//! The one-time part of the scheme: message digits and hash chains.

use crate::adrs::Adrs;
use crate::hash::{Hash, keccak256};

/// Chains per one-time key: 64 message digits and 3 checksum digits.
pub const CHAINS: usize = 67;
/// Digits that come straight from the digest.
const MESSAGE_DIGITS: usize = 64;
/// The largest digit, which is also the last step of a chain.
pub(crate) const MAX_DIGIT: u8 = 15;

/// Splits a digest into its 64 message digits and 3 checksum digits.
#[must_use]
pub fn digits(digest: &Hash) -> [u8; CHAINS] {
    let mut out = [0; CHAINS];
    for (pair, byte) in out[..MESSAGE_DIGITS].chunks_exact_mut(2).zip(digest) {
        pair[0] = byte >> 4;
        pair[1] = byte & 15;
    }
    let checksum: u16 = out[..MESSAGE_DIGITS]
        .iter()
        .map(|&digit| u16::from(MAX_DIGIT - digit))
        .sum();
    let [high, low] = checksum.to_be_bytes();
    out[MESSAGE_DIGITS] = high & 15;
    out[MESSAGE_DIGITS + 1] = low >> 4;
    out[MESSAGE_DIGITS + 2] = low & 15;
    out
}

/// Hashes `value` forward along a chain, from step `from` for `count` steps.
pub(crate) fn advance(
    pub_seed: &Hash,
    leaf: u32,
    chain: u32,
    from: u8,
    count: u8,
    value: &Hash,
) -> Hash {
    let mut value = *value;
    for step in from..from + count {
        value = keccak256(&[pub_seed, &Adrs::chain(leaf, chain, step.into()), &value]);
    }
    value
}

/// Hashes the 67 chain ends into a leaf.
pub(crate) fn leaf_hash(pub_seed: &Hash, leaf: u32, ends: &[Hash; CHAINS]) -> Hash {
    keccak256(&[pub_seed, &Adrs::leaf(leaf), ends.as_flattened()])
}

#[cfg(test)]
mod tests {
    use super::digits;

    #[test]
    fn high_nibble_comes_first() {
        let mut digest = [0; 32];
        digest[0] = 0xAB;
        digest[31] = 0xCD;
        let got = digits(&digest);
        assert_eq!(got[..2], [0xA, 0xB]);
        assert_eq!(got[62..64], [0xC, 0xD]);
    }

    #[test]
    fn all_zero_digest_has_the_largest_checksum() {
        // 64 digits of 0 give C = 64 * 15 = 960 = 0x3C0.
        assert_eq!(digits(&[0; 32])[64..], [3, 12, 0]);
    }

    #[test]
    fn all_ones_digest_has_a_zero_checksum() {
        let got = digits(&[0xFF; 32]);
        assert!(got[..64].iter().all(|&d| d == 15));
        assert_eq!(got[64..], [0, 0, 0]);
    }

    #[test]
    fn checksum_digits_are_big_endian_nibbles() {
        // One digit of 1 in an otherwise all-15 digest gives C = 14.
        let mut digest = [0xFF; 32];
        digest[0] = 0x1F;
        assert_eq!(digits(&digest)[64..], [0, 0, 14]);
    }
}
