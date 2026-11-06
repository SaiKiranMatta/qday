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
