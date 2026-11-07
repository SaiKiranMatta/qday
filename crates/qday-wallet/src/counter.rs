//! The one-way counter that stops a one-time key being used twice.

use crate::flash::{Flash, SECTOR};

/// The counter could not be read or moved. Nothing may be signed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterError;

/// A counter that can only go up. A secure element can replace the flash version behind this.
pub trait OneWayCounter {
    /// The current value.
    ///
    /// # Errors
    ///
    /// Fails if the value can't be read.
    fn value(&mut self) -> Result<u32, CounterError>;

    /// Stores a value one higher and returns the value it replaced.
    ///
    /// # Errors
    ///
    /// Fails if the new value isn't safely stored.
    fn advance(&mut self) -> Result<u32, CounterError>;
}

/// The most one counter of a pair may be moved at once. Well past any real gap, which is a wallet's
/// signatures so far, and it stops a forged flash value from using up the chip's counter.
pub const MAX_CATCH_UP: u32 = 2048;

/// Two counters kept level, so going back needs both to go back. Reads as the larger, and if either
/// fails nothing is signed: there's no falling back to the one that still works.
pub struct PairedCounter<A, B> {
    first: A,
    second: B,
}

impl<A: OneWayCounter, B: OneWayCounter> PairedCounter<A, B> {
    /// Pairs two counters. The first moves first, so a cut between them only skips a value.
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<A: OneWayCounter, B: OneWayCounter> OneWayCounter for PairedCounter<A, B> {
    fn value(&mut self) -> Result<u32, CounterError> {
        Ok(self.first.value()?.max(self.second.value()?))
    }

    fn advance(&mut self) -> Result<u32, CounterError> {
        let (first, second) = (self.first.value()?, self.second.value()?);
        let value = first.max(second);
        let target = value.checked_add(1).ok_or(CounterError)?;
        if target - first.min(second) > MAX_CATCH_UP {
            return Err(CounterError);
        }
        raise(&mut self.first, first, target)?;
        raise(&mut self.second, second, target)?;
        Ok(value)
    }
}

/// Moves a counter from `from` up to `target`.
fn raise(counter: &mut impl OneWayCounter, from: u32, target: u32) -> Result<(), CounterError> {
    for expected in from..target {
        if counter.advance()? != expected {
            return Err(CounterError);
        }
    }
    Ok(())
}

/// Bytes per log entry: the value, then its bitwise inverse.
const ENTRY: usize = 8;
/// Entries that fit in one sector.
const ENTRIES: u32 = SECTOR / 8;
/// Entries read from flash at a time.
const CHUNK: usize = 64;

/// What a scan of the log found.
struct Scan {
    value: u32,
    /// The sector holding the largest entry.
    active: u32,
    /// The first slot in that sector after everything written, if there is one.
    free: Option<u32>,
}

/// An append-only log in two sectors. The value is the largest entry, so an interrupted write can
/// only skip ahead. The sector holding the largest entry is never erased.
pub struct FlashCounter<F> {
    flash: F,
    base: u32,
}

impl<F: Flash> FlashCounter<F> {
    /// A counter whose two sectors start at `base`.
    pub fn new(flash: F, base: u32) -> Self {
        Self { flash, base }
    }

    fn sector_at(&self, sector: u32) -> u32 {
        self.base + sector * SECTOR
    }

    /// Reads the log from flash. Nothing is cached, so a failed write can't leave stale state.
    fn scan(&mut self) -> Result<Scan, CounterError> {
        let mut best: Option<(u32, u32)> = None;
        let mut used = [0; 2];
        for sector in 0..2 {
            let start = self.sector_at(sector);
            let mut buffer = [0; ENTRY * CHUNK];
            for first in (0..ENTRIES).step_by(CHUNK) {
                let offset = start + first * 8;
                self.flash
                    .read(offset, &mut buffer)
                    .map_err(|_| CounterError)?;
                for (slot, entry) in (first..).zip(buffer.chunks_exact(ENTRY)) {
                    if entry.iter().all(|&byte| byte == 0xFF) {
                        continue;
                    }
                    used[sector as usize] = slot + 1;
                    if let Some(value) = decode(entry)
                        && best.is_none_or(|(largest, _)| value > largest)
                    {
                        best = Some((value, sector));
                    }
                }
            }
        }
        let (value, active) = best.unwrap_or((0, 0));
        let next = used[active as usize];
        Ok(Scan {
            value,
            active,
            free: (next < ENTRIES).then_some(next),
        })
    }
}

fn decode(entry: &[u8]) -> Option<u32> {
    let value = u32::from_be_bytes(entry[..4].try_into().ok()?);
    let inverse = u32::from_be_bytes(entry[4..].try_into().ok()?);
    (value == !inverse).then_some(value)
}

fn encode(value: u32) -> [u8; ENTRY] {
    let mut entry = [0; ENTRY];
    entry[..4].copy_from_slice(&value.to_be_bytes());
    entry[4..].copy_from_slice(&(!value).to_be_bytes());
    entry
}

impl<F: Flash> OneWayCounter for FlashCounter<F> {
    fn value(&mut self) -> Result<u32, CounterError> {
        Ok(self.scan()?.value)
    }

    fn advance(&mut self) -> Result<u32, CounterError> {
        let scan = self.scan()?;
        let next = scan.value.checked_add(1).ok_or(CounterError)?;

        let offset = if let Some(slot) = scan.free {
            self.sector_at(scan.active) + slot * 8
        } else {
            let other = self.sector_at(1 - scan.active);
            self.flash.erase(other).map_err(|_| CounterError)?;
            other
        };
        self.flash
            .write(offset, &encode(next))
            .map_err(|_| CounterError)?;

        if self.scan()?.value == next {
            Ok(scan.value)
        } else {
            Err(CounterError)
        }
    }
}
