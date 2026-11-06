//! The one hash function used everywhere.

use sha3::{Digest, Keccak256};

/// A 32-byte hash output.
pub type Hash = [u8; 32];

/// Hashes the concatenation of `parts`.
#[must_use]
pub fn keccak256(parts: &[&[u8]]) -> Hash {
    let mut hasher = Keccak256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// Hashes data that arrives in pieces, for input too large to hold at once.
#[derive(Default)]
pub struct Hasher(Keccak256);

impl Hasher {
    /// Adds a piece.
    pub fn update(&mut self, data: &[u8]) {
        Digest::update(&mut self.0, data);
    }

    /// Finishes the hash.
    #[must_use]
    pub fn finish(self) -> Hash {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::{Hasher, keccak256};

    #[test]
    fn empty_input_gives_the_known_keccak_digest() {
        let expected = [
            0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7,
            0x03, 0xc0, 0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04,
            0x5d, 0x85, 0xa4, 0x70,
        ];
        assert_eq!(keccak256(&[]), expected);
    }

    #[test]
    fn a_hasher_matches_the_one_shot_hash() {
        let mut hasher = Hasher::default();
        hasher.update(b"ab");
        hasher.update(b"");
        hasher.update(b"c");
        assert_eq!(hasher.finish(), keccak256(&[b"abc"]));
    }

    #[test]
    fn parts_hash_the_same_as_their_concatenation() {
        assert_eq!(keccak256(&[b"ab", b"c"]), keccak256(&[b"abc"]));
        assert_ne!(keccak256(&[b"abc"]), [0; 32]);
    }
}
