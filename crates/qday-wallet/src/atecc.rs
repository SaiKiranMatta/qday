//! The ATECC608A over I2C, as far as its one-way counters need. Nothing here writes to the chip's
//! configuration or locks it. Written from Microchip's `CryptoAuthLib` and not yet run on a chip.

use crate::counter::{CounterError, OneWayCounter};

/// The chip's I2C address as it leaves the factory.
pub const ADDRESS: u8 = 0x60;
/// The highest value a counter can reach.
pub const COUNTER_MAX: u32 = 2_097_151;
/// How long the chip needs after the wake pulse.
pub const WAKE_DELAY_US: u32 = 1500;
/// The counter the wallet uses. Counter 0 can be tied to limited-use keys, so it's left alone.
pub const COUNTER_ID: u16 = 1;

const WORD_COMMAND: u8 = 0x03;
const WORD_SLEEP: u8 = 0x01;

const OP_READ: u8 = 0x02;
const OP_COUNTER: u8 = 0x24;
const OP_INFO: u8 = 0x30;

const COUNTER_READ: u8 = 0;
const COUNTER_INCREMENT: u8 = 1;
/// The config word holding `LockValue` and `LockConfig`, bytes 84 to 87.
const LOCK_WORD: u16 = 0x15;
const UNLOCKED: u8 = 0x55;

const WOKEN: [u8; 4] = [0x04, 0x11, 0x33, 0x43];
const WAKE_TRIES: u32 = 3;
/// The slowest command here takes 25 ms.
const REPLY_POLLS: u32 = 50;
const REPLY_POLL_US: u32 = 1000;

/// A status reply is the count, one byte and the CRC. A data reply has four bytes.
const STATUS_REPLY: usize = 4;
const DATA_REPLY: usize = 7;

/// The bus refused, which is also how the chip says it's busy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusError;

/// The wires to the chip.
pub trait AteccBus {
    /// Holds SDA low for at least 60 microseconds. Nothing answers this.
    fn wake_pulse(&mut self);

    /// Writes to the chip.
    ///
    /// # Errors
    ///
    /// Fails if the chip doesn't acknowledge.
    fn write(&mut self, bytes: &[u8]) -> Result<(), BusError>;

    /// Fills `buffer` from the chip.
    ///
    /// # Errors
    ///
    /// Fails if the chip doesn't acknowledge.
    fn read(&mut self, buffer: &mut [u8]) -> Result<(), BusError>;

    /// Waits.
    fn wait_us(&mut self, us: u32);
}

/// Why a command gave no usable answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AteccError {
    /// The chip didn't wake, or isn't there.
    NoWake,
    /// The command couldn't be sent.
    Bus,
    /// No reply came in time.
    NoReply,
    /// The reply isn't a length any command here gets.
    BadLength,
    /// The reply was damaged.
    BadCrc,
    /// The chip gave this status instead of data.
    Status(u8),
    /// The chip gave a counter it can't hold.
    OutOfRange,
}

/// Which zones are locked. Locking can't be undone, and nothing here does it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locks {
    /// The config zone.
    pub config: bool,
    /// The data and OTP zones.
    pub data: bool,
}

/// What a read-only look at the chip found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    /// The revision bytes.
    pub revision: [u8; 4],
    /// Which zones are locked.
    pub locks: Locks,
    /// The wallet's counter, or why the chip wouldn't give it.
    pub counter: Result<u32, AteccError>,
}

/// The chip's CRC-16, low byte first.
#[must_use]
pub fn crc16(data: &[u8]) -> [u8; 2] {
    const POLYNOMIAL: u16 = 0x8005;
    let mut crc: u16 = 0;
    for byte in data {
        for bit in 0..8 {
            let data_bit = (byte >> bit) & 1 != 0;
            let crc_bit = crc & 0x8000 != 0;
            crc <<= 1;
            if data_bit != crc_bit {
                crc ^= POLYNOMIAL;
            }
        }
    }
    crc.to_le_bytes()
}

/// The chip. Every command wakes it and puts it back to sleep, so its watchdog never cuts one off.
pub struct Atecc<B> {
    bus: B,
}

impl<B: AteccBus> Atecc<B> {
    /// A chip on `bus`.
    pub fn new(bus: B) -> Self {
        Self { bus }
    }

    /// The revision bytes, which say what the chip is.
    ///
    /// # Errors
    ///
    /// Fails if the chip gives no good answer.
    pub fn revision(&mut self) -> Result<[u8; 4], AteccError> {
        self.run(OP_INFO, 0, 0)
    }

    /// Which zones are locked.
    ///
    /// # Errors
    ///
    /// Fails if the chip gives no good answer.
    pub fn locks(&mut self) -> Result<Locks, AteccError> {
        let [_, _, data, config] = self.run(OP_READ, 0, LOCK_WORD)?;
        Ok(Locks {
            config: config != UNLOCKED,
            data: data != UNLOCKED,
        })
    }

    /// A counter's value.
    ///
    /// # Errors
    ///
    /// Fails if the chip gives no good answer.
    pub fn counter(&mut self, id: u16) -> Result<u32, AteccError> {
        checked(self.run(OP_COUNTER, COUNTER_READ, id)?)
    }

    /// Moves a counter up by one, for good, and returns its new value.
    ///
    /// # Errors
    ///
    /// Fails if the chip gives no good answer. The counter may have moved anyway.
    pub fn increment(&mut self, id: u16) -> Result<u32, AteccError> {
        checked(self.run(OP_COUNTER, COUNTER_INCREMENT, id)?)
    }

    /// Looks at the chip without changing anything on it.
    ///
    /// # Errors
    ///
    /// Fails if the chip doesn't answer at all. A refused counter read is reported in the result.
    pub fn probe(&mut self) -> Result<Probe, AteccError> {
        Ok(Probe {
            revision: self.revision()?,
            locks: self.locks()?,
            counter: self.counter(COUNTER_ID),
        })
    }

    fn run(&mut self, opcode: u8, param1: u8, param2: u16) -> Result<[u8; 4], AteccError> {
        self.wake()?;
        let result = self.exchange(opcode, param1, param2);
        // If this fails, the chip's watchdog puts it to sleep soon anyway.
        let _ = self.bus.write(&[WORD_SLEEP]);
        result
    }

    fn wake(&mut self) -> Result<(), AteccError> {
        for _ in 0..WAKE_TRIES {
            self.bus.wake_pulse();
            self.bus.wait_us(WAKE_DELAY_US);
            let mut reply = [0; WOKEN.len()];
            if self.bus.read(&mut reply).is_ok() && reply == WOKEN {
                return Ok(());
            }
            // A chip already awake ignores the pulse, so it has to sleep first.
            let _ = self.bus.write(&[WORD_SLEEP]);
        }
        Err(AteccError::NoWake)
    }

    fn exchange(&mut self, opcode: u8, param1: u8, param2: u16) -> Result<[u8; 4], AteccError> {
        let [low, high] = param2.to_le_bytes();
        let mut packet = [WORD_COMMAND, 7, opcode, param1, low, high, 0, 0];
        let crc = crc16(&packet[1..6]);
        packet[6..].copy_from_slice(&crc);
        self.bus.write(&packet).map_err(|_| AteccError::Bus)?;
        self.reply()
    }

    fn reply(&mut self) -> Result<[u8; 4], AteccError> {
        let mut reply = [0; DATA_REPLY];
        self.first_byte(&mut reply[0])?;
        let length = usize::from(reply[0]);
        if length != STATUS_REPLY && length != DATA_REPLY {
            return Err(AteccError::BadLength);
        }
        self.bus
            .read(&mut reply[1..length])
            .map_err(|_| AteccError::NoReply)?;
        let (body, crc) = reply[..length].split_at(length - 2);
        if crc16(body) != *crc {
            return Err(AteccError::BadCrc);
        }
        if length == STATUS_REPLY {
            return Err(AteccError::Status(reply[1]));
        }
        Ok([reply[1], reply[2], reply[3], reply[4]])
    }

    /// The chip refuses reads until the command has finished.
    fn first_byte(&mut self, byte: &mut u8) -> Result<(), AteccError> {
        for _ in 0..REPLY_POLLS {
            if self.bus.read(core::slice::from_mut(byte)).is_ok() {
                return Ok(());
            }
            self.bus.wait_us(REPLY_POLL_US);
        }
        Err(AteccError::NoReply)
    }
}

fn checked(bytes: [u8; 4]) -> Result<u32, AteccError> {
    let value = u32::from_le_bytes(bytes);
    if value > COUNTER_MAX {
        return Err(AteccError::OutOfRange);
    }
    Ok(value)
}

/// The chip's counter as a one-way counter. Restoring old flash can't touch it.
pub struct ChipCounter<B> {
    chip: Atecc<B>,
}

impl<B: AteccBus> ChipCounter<B> {
    /// Uses the wallet's counter on `chip`.
    pub fn new(chip: Atecc<B>) -> Self {
        Self { chip }
    }
}

impl<B: AteccBus> OneWayCounter for ChipCounter<B> {
    fn value(&mut self) -> Result<u32, CounterError> {
        self.chip.counter(COUNTER_ID).map_err(|_| CounterError)
    }

    fn advance(&mut self) -> Result<u32, CounterError> {
        let value = self.chip.increment(COUNTER_ID).map_err(|_| CounterError)?;
        value.checked_sub(1).ok_or(CounterError)
    }
}
