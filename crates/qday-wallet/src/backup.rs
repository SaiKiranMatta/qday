//! The seed as 24 BIP39 words. The seed is the words' entropy.

use bip39::{Language, Mnemonic};
use qday_xmss::Hash;

/// Words in a backup.
pub const WORDS: usize = 24;

/// The 24 words for a seed.
///
/// # Panics
///
/// Never: 32 bytes is always valid BIP39 entropy.
#[must_use]
pub fn seed_to_words(seed: &Hash) -> [&'static str; WORDS] {
    let mnemonic = Mnemonic::from_entropy(seed).expect("32 bytes is valid entropy");
    let mut out = [""; WORDS];
    for (slot, word) in out.iter_mut().zip(mnemonic.words()) {
        *slot = word;
    }
    out
}

/// The seed for 24 words separated by spaces. Returns `None` unless they are valid.
#[must_use]
pub fn words_to_seed(words: &str) -> Option<Hash> {
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, words).ok()?;
    if mnemonic.word_count() != WORDS {
        return None;
    }
    let (entropy, length) = mnemonic.to_entropy_array();
    Hash::try_from(&entropy[..length]).ok()
}

#[cfg(test)]
mod tests {
    use super::{WORDS, seed_to_words, words_to_seed};

    const ZERO_WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon \
        abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
        abandon abandon abandon abandon art";
    const SEVENS_WORDS: &str = "legal winner thank year wave sausage worth useful legal winner \
        thank year wave sausage worth useful legal winner thank year wave sausage worth title";

    #[test]
    fn the_standard_vectors_give_their_words() {
        assert_eq!(seed_to_words(&[0; 32]).join(" "), ZERO_WORDS);
        assert_eq!(seed_to_words(&[0x7f; 32]).join(" "), SEVENS_WORDS);
    }

    #[test]
    fn the_standard_vectors_give_their_seeds() {
        assert_eq!(words_to_seed(ZERO_WORDS), Some([0; 32]));
        assert_eq!(words_to_seed(SEVENS_WORDS), Some([0x7f; 32]));
    }

    #[test]
    fn any_seed_survives_the_round_trip() {
        let seed = core::array::from_fn(|i| u8::try_from(i * 7).expect("the value fits"));
        assert_eq!(words_to_seed(&seed_to_words(&seed).join(" ")), Some(seed));
    }

    #[test]
    fn a_bad_checksum_is_refused() {
        let words = ["abandon"; WORDS].join(" ");
        assert_eq!(words_to_seed(&words), None);
    }

    #[test]
    fn twelve_words_are_refused() {
        let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon \
            abandon abandon about";
        assert_eq!(words_to_seed(words), None);
    }

    #[test]
    fn an_unknown_word_is_refused() {
        assert_eq!(words_to_seed(&ZERO_WORDS.replace("art", "qday")), None);
    }
}
