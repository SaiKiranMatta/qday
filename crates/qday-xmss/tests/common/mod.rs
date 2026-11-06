//! Shared values from the published test vectors.

use std::sync::OnceLock;

use qday_xmss::{Hash, KeySet, Tree};

/// The vector seed: `0x5eed` repeated 16 times.
pub const SEED: Hash = {
    let mut seed = [0; 32];
    let mut i = 0;
    while i < 32 {
        seed[i] = 0x5e;
        seed[i + 1] = 0xed;
        i += 2;
    }
    seed
};

/// Parses a 0x-prefixed 32-byte value.
pub fn hash(text: &str) -> Hash {
    let digits = text.strip_prefix("0x").expect("vectors start with 0x");
    assert_eq!(digits.len(), 64, "vectors are 32 bytes");
    let mut out = [0; 32];
    for (slot, pair) in out.iter_mut().zip(digits.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).expect("vectors are ASCII");
        *slot = u8::from_str_radix(pair, 16).expect("vectors are hex");
    }
    out
}

/// Tree 0 of the vector seed, built once for the whole test binary.
pub fn tree0() -> &'static Tree {
    static TREE: OnceLock<Tree> = OnceLock::new();
    TREE.get_or_init(|| Tree::build(&KeySet::derive(&SEED, 0), |_| {}))
}
