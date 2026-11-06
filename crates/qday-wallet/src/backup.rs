//! The seed as 24 BIP39 words. The seed is the words' entropy.

use bip39::{Language, Mnemonic};
use qday_xmss::Hash;

use crate::rng::Rng;

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

const WORDS_U32: u32 = 24;
const CHOICES_U32: u32 = 4;
const WORD_LIST_U32: u32 = 2048;

/// Questions in a backup check.
pub const QUESTIONS: usize = 3;
/// Choices per question, one for each key.
pub const CHOICES: usize = 4;

/// One question: which word is at this position?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Question {
    /// The position in the backup, from 0.
    pub position: usize,
    /// The four words offered.
    pub choices: [&'static str; CHOICES],
    /// Which choice is right.
    pub answer: usize,
}

/// What a pick led to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Right, and there's another question.
    Next,
    /// Right, and that was the last question.
    Passed,
    /// Wrong: go back to the words.
    Wrong,
}

/// Three questions on different words.
pub struct BackupCheck {
    questions: [Question; QUESTIONS],
    current: usize,
}

impl BackupCheck {
    /// Picks the questions for a seed.
    pub fn new(seed: &Hash, rng: &mut impl Rng) -> Self {
        let words = seed_to_words(seed);
        let positions = pick_positions(rng);
        let questions = positions.map(|position| ask(position, words[position], rng));
        Self {
            questions,
            current: 0,
        }
    }

    /// The question being asked.
    #[must_use]
    pub fn question(&self) -> &Question {
        &self.questions[self.current]
    }

    /// Which question this is, from 0.
    #[must_use]
    pub fn number(&self) -> usize {
        self.current
    }

    /// Answers the current question with choice `pick`.
    pub fn pick(&mut self, pick: usize) -> Progress {
        if pick != self.question().answer {
            return Progress::Wrong;
        }
        if self.current + 1 == QUESTIONS {
            return Progress::Passed;
        }
        self.current += 1;
        Progress::Next
    }
}

/// Picks different positions, with the first steps of a shuffle.
fn pick_positions(rng: &mut impl Rng) -> [usize; QUESTIONS] {
    let mut order: [usize; WORDS] = core::array::from_fn(|position| position);
    let mut picked = [0; QUESTIONS];
    for (taken, slot) in (0_u32..).zip(&mut picked) {
        let other = taken + rng.below(WORDS_U32 - taken);
        order.swap(taken as usize, other as usize);
        *slot = order[taken as usize];
    }
    picked
}

fn ask(position: usize, word: &'static str, rng: &mut impl Rng) -> Question {
    let list = Language::English.word_list();
    let answer = rng.below(CHOICES_U32) as usize;
    let mut choices = [""; CHOICES];
    choices[answer] = word;
    for slot in 0..CHOICES {
        while choices[slot].is_empty() {
            let decoy = list[rng.below(WORD_LIST_U32) as usize];
            if !choices.contains(&decoy) {
                choices[slot] = decoy;
            }
        }
    }
    Question {
        position,
        choices,
        answer,
    }
}

#[cfg(test)]
mod tests {
    use super::{BackupCheck, CHOICES, Progress, QUESTIONS, WORDS, seed_to_words, words_to_seed};
    use crate::rng::Rng;

    /// A small generator for tests.
    struct Steps(u32);

    impl Rng for Steps {
        fn fill(&mut self, buffer: &mut [u8]) {
            for slot in buffer {
                self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *slot = self.0.to_be_bytes()[0];
            }
        }
    }

    const SEED: [u8; 32] = [0x7f; 32];

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

    #[test]
    fn questions_are_on_different_words() {
        for start in 0..50 {
            let mut check = BackupCheck::new(&SEED, &mut Steps(start));
            let mut seen = [false; WORDS];
            for _ in 0..QUESTIONS {
                let position = check.question().position;
                assert!(position < WORDS);
                assert!(!seen[position], "position {position} asked twice");
                seen[position] = true;
                let answer = check.question().answer;
                check.pick(answer);
            }
        }
    }

    #[test]
    fn the_answer_is_the_real_word() {
        let words = seed_to_words(&SEED);
        for start in 0..50 {
            let check = BackupCheck::new(&SEED, &mut Steps(start));
            let question = check.question();
            assert_eq!(question.choices[question.answer], words[question.position]);
        }
    }

    #[test]
    fn the_choices_are_four_different_words() {
        for start in 0..50 {
            let check = BackupCheck::new(&SEED, &mut Steps(start));
            let choices = check.question().choices;
            for (i, word) in choices.iter().enumerate() {
                assert!(!choices[i + 1..].contains(word), "{word} offered twice");
            }
            assert_eq!(choices.len(), CHOICES);
        }
    }

    #[test]
    fn the_answer_moves_between_keys() {
        let mut seen = [false; CHOICES];
        for start in 0..100 {
            seen[BackupCheck::new(&SEED, &mut Steps(start)).question().answer] = true;
        }
        assert_eq!(seen, [true; CHOICES]);
    }

    #[test]
    fn three_right_answers_pass() {
        let mut check = BackupCheck::new(&SEED, &mut Steps(1));
        for expected in [Progress::Next, Progress::Next, Progress::Passed] {
            let answer = check.question().answer;
            assert_eq!(check.pick(answer), expected);
        }
    }

    #[test]
    fn a_wrong_pick_fails() {
        let mut check = BackupCheck::new(&SEED, &mut Steps(1));
        let wrong = (check.question().answer + 1) % CHOICES;
        assert_eq!(check.pick(wrong), Progress::Wrong);
    }

    #[test]
    fn a_wrong_pick_on_the_last_question_still_fails() {
        let mut check = BackupCheck::new(&SEED, &mut Steps(1));
        for _ in 0..2 {
            let answer = check.question().answer;
            check.pick(answer);
        }
        assert_eq!(check.number(), 2);
        let wrong = (check.question().answer + 1) % CHOICES;
        assert_eq!(check.pick(wrong), Progress::Wrong);
    }
}
