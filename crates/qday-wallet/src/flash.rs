//! The flash the wallet stores everything in, and where each thing lives.

use core::cell::RefCell;

/// Size of the smallest area that can be erased.
pub const SECTOR: u32 = 0x1000;

/// Offsets inside the `qday` partition.
pub mod layout {
    /// The one-way counter's log: two sectors.
    pub const COUNTER: u32 = 0x0;
    /// The first key slot.
    pub const KEY_SLOTS: u32 = 0x2000;
    /// Size of each of the two key slots.
    pub const KEY_SLOT_SIZE: u32 = 0x11000;
    /// Touch calibration.
    pub const CALIBRATION: u32 = 0x3B000;
    /// The passcode: two sectors.
    pub const PASSCODE: u32 = 0x3D000;
    /// Settings.
    pub const SETTINGS: u32 = 0x3F000;
}

/// A flash operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashError;

/// NOR flash: erasing sets a sector to all ones, and writing can only clear bits.
pub trait Flash {
    /// Reads `buffer.len()` bytes from `offset`.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), FlashError>;

    /// Writes `data` at `offset`.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be written.
    fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError>;

    /// Erases the sector that starts at `offset`.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be erased.
    fn erase(&mut self, offset: u32) -> Result<(), FlashError>;
}

/// Flash that can only be written in aligned 4-byte words, as the chip's raw writes are.
pub trait WordFlash {
    /// Reads `buffer.len()` bytes from `offset`, with no alignment needed.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), FlashError>;

    /// Writes whole words. `offset` and `data.len()` must be multiples of 4.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be written.
    fn write_words(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError>;

    /// Erases the sector that starts at `offset`.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be erased.
    fn erase(&mut self, offset: u32) -> Result<(), FlashError>;
}

/// Bytes in a flash word.
const WORD: u32 = 4;

/// Writes any bytes through word-only flash. Partial words are padded with `0xFF`, which leaves
/// the neighbouring bytes alone because a NOR write can only clear bits.
pub struct Aligned<W>(pub W);

impl<W: WordFlash> Aligned<W> {
    /// Writes the part of one word that `data` covers.
    fn write_partial(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError> {
        let word_start = offset - offset % WORD;
        let mut word = [0xFF; WORD as usize];
        let start = (offset % WORD) as usize;
        word[start..start + data.len()].copy_from_slice(data);
        self.0.write_words(word_start, &word)
    }
}

impl<W: WordFlash> Flash for Aligned<W> {
    fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), FlashError> {
        self.0.read(offset, buffer)
    }

    fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError> {
        if data.is_empty() {
            return Ok(());
        }
        let mut offset = offset;
        let mut rest = data;

        let misaligned = offset % WORD;
        if misaligned != 0 {
            let head = rest.len().min((WORD - misaligned) as usize);
            let (first, after) = rest.split_at(head);
            self.write_partial(offset, first)?;
            offset += u32::try_from(head).map_err(|_| FlashError)?;
            rest = after;
        }

        let whole = rest.len() - rest.len() % WORD as usize;
        let (words, tail) = rest.split_at(whole);
        if !words.is_empty() {
            self.0.write_words(offset, words)?;
            offset += u32::try_from(whole).map_err(|_| FlashError)?;
        }

        if !tail.is_empty() {
            self.write_partial(offset, tail)?;
        }
        Ok(())
    }

    fn erase(&mut self, offset: u32) -> Result<(), FlashError> {
        self.0.erase(offset)
    }
}

/// Lets several stores share one flash on a single thread.
impl<F: Flash> Flash for &RefCell<F> {
    fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), FlashError> {
        self.borrow_mut().read(offset, buffer)
    }

    fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError> {
        self.borrow_mut().write(offset, data)
    }

    fn erase(&mut self, offset: u32) -> Result<(), FlashError> {
        self.borrow_mut().erase(offset)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{Aligned, Flash, FlashError, WordFlash};

    /// Refuses anything a real chip would refuse, and can only clear bits.
    struct StrictFlash {
        bytes: Vec<u8>,
        writes: usize,
    }

    impl WordFlash for StrictFlash {
        fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), FlashError> {
            let start = offset as usize;
            buffer.copy_from_slice(&self.bytes[start..start + buffer.len()]);
            Ok(())
        }

        fn write_words(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError> {
            assert_eq!(offset % 4, 0, "the offset must be aligned");
            assert_eq!(data.len() % 4, 0, "the length must be whole words");
            self.writes += 1;
            for (slot, byte) in self.bytes[offset as usize..].iter_mut().zip(data) {
                *slot &= byte;
            }
            Ok(())
        }

        fn erase(&mut self, offset: u32) -> Result<(), FlashError> {
            self.bytes[offset as usize..offset as usize + 4096].fill(0xFF);
            Ok(())
        }
    }

    fn flash() -> Aligned<StrictFlash> {
        Aligned(StrictFlash {
            bytes: vec![0xFF; 8192],
            writes: 0,
        })
    }

    fn stored(flash: &mut Aligned<StrictFlash>, offset: u32, length: usize) -> Vec<u8> {
        let mut bytes = vec![0; length];
        flash.read(offset, &mut bytes).expect("the read works");
        bytes
    }

    #[test]
    fn a_single_byte_lands_alone() {
        let mut flash = flash();
        flash.write(129, &[0x00]).expect("the write works");
        assert_eq!(stored(&mut flash, 128, 4), [0xFF, 0x00, 0xFF, 0xFF]);
    }

    #[test]
    fn neighbouring_marks_do_not_disturb_each_other() {
        let mut flash = flash();
        flash.write(128, &[0x00]).expect("the write works");
        flash.write(129, &[0xAB]).expect("the write works");
        flash.write(130, &[0x12]).expect("the write works");
        assert_eq!(stored(&mut flash, 128, 4), [0x00, 0xAB, 0x12, 0xFF]);
    }

    #[test]
    fn any_offset_and_length_round_trips() {
        let data: Vec<u8> = (0..41).map(|value| value * 3).collect();
        for offset in 0..9_u32 {
            for length in 0..data.len() {
                let mut flash = flash();
                flash
                    .write(offset, &data[..length])
                    .expect("the write works");
                assert_eq!(stored(&mut flash, offset, length), data[..length]);
                if offset > 0 {
                    assert_eq!(stored(&mut flash, offset - 1, 1), [0xFF]);
                }
                let after = offset + u32::try_from(length).expect("the length is small");
                assert_eq!(stored(&mut flash, after, 1), [0xFF]);
            }
        }
    }

    #[test]
    fn whole_words_go_out_in_one_write() {
        let mut flash = flash();
        flash.write(0, &[1; 4096]).expect("the write works");
        assert_eq!(flash.0.writes, 1);
    }

    #[test]
    fn empty_writes_do_nothing() {
        let mut flash = flash();
        flash.write(5, &[]).expect("the write works");
        assert_eq!(flash.0.writes, 0);
    }

    #[test]
    fn erase_passes_through() {
        let mut flash = flash();
        flash.write(0, &[0; 8]).expect("the write works");
        flash.erase(0).expect("the erase works");
        assert_eq!(stored(&mut flash, 0, 8), [0xFF; 8]);
    }
}
