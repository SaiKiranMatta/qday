//! The ATECC608A driver and the counter built on it, against a model of the chip.
//!
//! The packets and CRCs are checked against values worked out from Microchip's CryptoAuthLib. The
//! model is written from the same source, so these tests can't prove the real chip agrees.

mod common;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::OnceLock;

use common::{SimFlash, TestRng};
use qday_wallet::atecc::{
    Atecc, AteccBus, AteccError, BusError, COUNTER_MAX, ChipCounter, Locks, WAKE_DELAY_US, crc16,
};
use qday_wallet::counter::{
    CounterError, FlashCounter, MAX_CATCH_UP, OneWayCounter, PairedCounter,
};
use qday_wallet::flash::layout;
use qday_wallet::frame::FrameTx;
use qday_wallet::keystore::KeyStore;
use qday_wallet::mlkey::MlKeyStore;
use qday_wallet::passcode::{Attempt, Code, Passcode};
use qday_wallet::settings::Settings;
use qday_wallet::wallet::{Wallet, WalletError};

const INFO: [u8; 8] = [0x03, 0x07, 0x30, 0x00, 0x00, 0x00, 0x03, 0x5D];
const READ_LOCKS: [u8; 8] = [0x03, 0x07, 0x02, 0x00, 0x15, 0x00, 0x17, 0x5D];
const READ_COUNTER_1: [u8; 8] = [0x03, 0x07, 0x24, 0x00, 0x01, 0x00, 0x05, 0x7D];
const INCREMENT_COUNTER_1: [u8; 8] = [0x03, 0x07, 0x24, 0x01, 0x01, 0x00, 0x06, 0xF7];

const SLEEP: u8 = 0x01;
const COMMAND: u8 = 0x03;
const UNLOCKED: u8 = 0x55;
const LOCKED: u8 = 0x00;
const EXECUTION_ERROR: u8 = 0x0F;
const CRC_ERROR: u8 = 0xFF;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Power {
    Asleep,
    Pulsed { waited_us: u32 },
    Awake,
}

struct Chip {
    present: bool,
    power: Power,
    reply: VecDeque<u8>,
    /// Reads refused while a command runs.
    busy_reads: u32,
    next_busy_reads: u32,
    counters: [u32; 2],
    config_lock: u8,
    data_lock: u8,
    commands: Vec<Vec<u8>>,
    /// Sent instead of the next real reply.
    forced_reply: Option<Vec<u8>>,
    /// The command runs, but its reply never arrives.
    lose_reply: bool,
    /// Flips a bit in the next reply, after its CRC is worked out.
    corrupt_reply: bool,
    /// Refuses the Counter command, as a chip might while its config is unlocked.
    refuse_counter: bool,
}

/// What's on the far side of the I2C wires.
#[derive(Clone)]
struct FakeChip(Rc<RefCell<Chip>>);

impl FakeChip {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(Chip {
            present: true,
            power: Power::Asleep,
            reply: VecDeque::new(),
            busy_reads: 0,
            next_busy_reads: 3,
            counters: [0; 2],
            config_lock: LOCKED,
            data_lock: UNLOCKED,
            commands: Vec::new(),
            forced_reply: None,
            lose_reply: false,
            corrupt_reply: false,
            refuse_counter: false,
        })))
    }

    fn with<T>(&self, change: impl FnOnce(&mut Chip) -> T) -> T {
        change(&mut self.0.borrow_mut())
    }

    fn counter(&self, id: usize) -> u32 {
        self.with(|chip| chip.counters[id])
    }

    fn driver(&self) -> Atecc<Self> {
        Atecc::new(self.clone())
    }

    fn chip_counter(&self) -> ChipCounter<Self> {
        ChipCounter::new(self.driver())
    }
}

impl Chip {
    fn run(&mut self, packet: &[u8]) {
        self.commands.push(packet.to_vec());
        let body = &packet[1..];
        let (fields, crc) = body.split_at(body.len() - 2);
        let result = if crc16(fields) != [crc[0], crc[1]] {
            Err(CRC_ERROR)
        } else {
            self.execute(
                fields[1],
                fields[2],
                u16::from_le_bytes([fields[3], fields[4]]),
            )
        };
        let mut reply = match result {
            Ok(data) => frame(&data),
            Err(status) => frame(&[status]),
        };
        if let Some(forced) = self.forced_reply.take() {
            reply = forced;
        }
        if self.corrupt_reply {
            self.corrupt_reply = false;
            reply[1] ^= 1;
        }
        self.busy_reads = self.next_busy_reads;
        if self.lose_reply {
            self.lose_reply = false;
        } else {
            self.reply = reply.into();
        }
    }

    fn execute(&mut self, opcode: u8, param1: u8, param2: u16) -> Result<[u8; 4], u8> {
        match (opcode, param1, param2) {
            (0x30, 0, 0) => Ok([0x00, 0x00, 0x60, 0x02]),
            (0x02, 0, 0x15) => Ok([0, 0, self.data_lock, self.config_lock]),
            (0x24, mode @ (0 | 1), id @ (0 | 1)) => {
                if self.refuse_counter {
                    return Err(EXECUTION_ERROR);
                }
                let counter = &mut self.counters[usize::from(id)];
                if mode == 1 {
                    if *counter >= COUNTER_MAX {
                        return Err(EXECUTION_ERROR);
                    }
                    *counter += 1;
                }
                Ok(counter.to_le_bytes())
            }
            _ => Err(0x03),
        }
    }
}

/// Adds the count byte and the CRC.
fn frame(data: &[u8]) -> Vec<u8> {
    let count = u8::try_from(data.len() + 3).expect("replies are short");
    let mut packet = vec![count];
    packet.extend_from_slice(data);
    let crc = crc16(&packet);
    packet.extend_from_slice(&crc);
    packet
}

impl AteccBus for FakeChip {
    fn wake_pulse(&mut self) {
        self.with(|chip| {
            if chip.present && chip.power == Power::Asleep {
                chip.power = Power::Pulsed { waited_us: 0 };
            }
        });
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), BusError> {
        self.with(|chip| {
            if !chip.present || !matches!(chip.power, Power::Awake) {
                return Err(BusError);
            }
            match bytes.first() {
                Some(&SLEEP) => {
                    chip.power = Power::Asleep;
                    chip.reply.clear();
                }
                Some(&COMMAND) => chip.run(bytes),
                _ => return Err(BusError),
            }
            Ok(())
        })
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<(), BusError> {
        self.with(|chip| {
            if !chip.present {
                return Err(BusError);
            }
            if let Power::Pulsed { waited_us } = chip.power {
                if waited_us < WAKE_DELAY_US {
                    return Err(BusError);
                }
                chip.power = Power::Awake;
                chip.reply = vec![0x04, 0x11, 0x33, 0x43].into();
            }
            if chip.power != Power::Awake {
                return Err(BusError);
            }
            if chip.busy_reads > 0 {
                chip.busy_reads -= 1;
                return Err(BusError);
            }
            if chip.reply.len() < buffer.len() {
                return Err(BusError);
            }
            for byte in buffer {
                *byte = chip.reply.pop_front().ok_or(BusError)?;
            }
            Ok(())
        })
    }

    fn wait_us(&mut self, us: u32) {
        self.with(|chip| {
            if let Power::Pulsed { waited_us } = &mut chip.power {
                *waited_us += us;
            }
        });
    }
}

#[test]
fn the_crc_matches_the_chips_own_wake_reply() {
    assert_eq!(crc16(&[0x04, 0x11]), [0x33, 0x43]);
}

#[test]
fn commands_go_out_byte_for_byte() {
    let chip = FakeChip::new();
    let mut driver = chip.driver();
    driver.revision().expect("the chip answers");
    driver.locks().expect("the chip answers");
    driver.counter(1).expect("the chip answers");
    driver.increment(1).expect("the chip answers");
    assert_eq!(
        chip.with(|chip| chip.commands.clone()),
        [
            INFO.to_vec(),
            READ_LOCKS.to_vec(),
            READ_COUNTER_1.to_vec(),
            INCREMENT_COUNTER_1.to_vec(),
        ],
    );
}

#[test]
fn the_revision_comes_back_as_sent() {
    assert_eq!(
        FakeChip::new().driver().revision(),
        Ok([0x00, 0x00, 0x60, 0x02])
    );
}

#[test]
fn the_lock_bytes_are_read_separately() {
    let chip = FakeChip::new();
    assert_eq!(
        chip.driver().locks(),
        Ok(Locks {
            config: true,
            data: false,
        })
    );
    chip.with(|chip| {
        chip.config_lock = UNLOCKED;
        chip.data_lock = LOCKED;
    });
    assert_eq!(
        chip.driver().locks(),
        Ok(Locks {
            config: false,
            data: true,
        })
    );
}

#[test]
fn reading_a_counter_leaves_it_alone() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.counters = [0, 77]);
    assert_eq!(chip.driver().counter(1), Ok(77));
    assert_eq!(chip.counter(1), 77);
}

#[test]
fn incrementing_gives_the_new_value() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.counters = [5, 77]);
    assert_eq!(chip.driver().increment(1), Ok(78));
    assert_eq!(chip.counter(1), 78);
    assert_eq!(chip.counter(0), 5);
}

#[test]
fn the_chip_is_left_asleep_after_every_command() {
    let chip = FakeChip::new();
    let mut driver = chip.driver();
    driver.counter(1).expect("the chip answers");
    assert_eq!(chip.with(|chip| chip.power), Power::Asleep);
    driver.increment(1).expect("the chip answers");
    assert_eq!(chip.with(|chip| chip.power), Power::Asleep);
}

#[test]
fn the_chip_is_left_asleep_after_a_refusal() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.refuse_counter = true);
    chip.driver().counter(1).expect_err("the chip refused");
    assert_eq!(chip.with(|chip| chip.power), Power::Asleep);
}

#[test]
fn a_chip_left_awake_is_put_to_sleep_and_woken_again() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.power = Power::Awake);
    assert_eq!(chip.driver().counter(1), Ok(0));
}

#[test]
fn a_missing_chip_is_reported() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.present = false);
    assert_eq!(chip.driver().counter(1), Err(AteccError::NoWake));
}

#[test]
fn a_slow_reply_is_waited_for() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.next_busy_reads = 30);
    assert_eq!(chip.driver().increment(1), Ok(1));
}

#[test]
fn a_reply_that_never_comes_is_an_error() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.lose_reply = true);
    assert_eq!(chip.driver().counter(1), Err(AteccError::NoReply));
}

#[test]
fn a_damaged_reply_is_rejected() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.corrupt_reply = true);
    assert_eq!(chip.driver().counter(1), Err(AteccError::BadCrc));
}

#[test]
fn a_status_from_the_chip_is_reported() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.refuse_counter = true);
    assert_eq!(
        chip.driver().counter(1),
        Err(AteccError::Status(EXECUTION_ERROR))
    );
}

#[test]
fn a_reply_of_the_wrong_length_is_rejected() {
    for count in [0, 3, 5, 6, 8, 35, 255] {
        let chip = FakeChip::new();
        chip.with(|chip| chip.forced_reply = Some(vec![count; 40]));
        assert_eq!(
            chip.driver().counter(1),
            Err(AteccError::BadLength),
            "{count}"
        );
    }
}

#[test]
fn a_counter_past_the_chips_limit_is_rejected() {
    let chip = FakeChip::new();
    let too_big = (COUNTER_MAX + 1).to_le_bytes();
    chip.with(|chip| chip.forced_reply = Some(frame(&too_big)));
    assert_eq!(chip.driver().counter(1), Err(AteccError::OutOfRange));
}

#[test]
fn the_probe_changes_nothing_on_the_chip() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.counters = [3, 9]);
    let probe = chip.driver().probe().expect("the chip answers");
    assert_eq!(probe.revision, [0x00, 0x00, 0x60, 0x02]);
    assert_eq!(probe.counter, Ok(9));
    assert_eq!(chip.with(|chip| chip.counters), [3, 9]);
    assert_eq!(
        chip.with(|chip| chip.commands.clone()),
        [INFO.to_vec(), READ_LOCKS.to_vec(), READ_COUNTER_1.to_vec(),],
    );
}

#[test]
fn the_probe_still_reports_a_chip_that_refuses_its_counter() {
    let chip = FakeChip::new();
    chip.with(|chip| {
        chip.refuse_counter = true;
        chip.config_lock = UNLOCKED;
    });
    let probe = chip.driver().probe().expect("the chip answers");
    assert!(!probe.locks.config);
    assert_eq!(probe.counter, Err(AteccError::Status(EXECUTION_ERROR)));
}

#[test]
fn the_probe_fails_with_no_chip() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.present = false);
    assert!(chip.driver().probe().is_err());
}

#[test]
fn the_chip_counter_returns_the_value_it_replaced() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.counters = [0, 41]);
    let mut counter = chip.chip_counter();
    assert_eq!(counter.value(), Ok(41));
    assert_eq!(counter.advance(), Ok(41));
    assert_eq!(counter.value(), Ok(42));
}

#[test]
fn the_chip_counter_stops_at_the_chips_limit() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.counters = [0, COUNTER_MAX]);
    assert_eq!(chip.chip_counter().advance(), Err(CounterError));
    assert_eq!(chip.counter(1), COUNTER_MAX);
}

#[test]
fn a_lost_reply_can_only_skip_a_value() {
    let chip = FakeChip::new();
    let mut counter = chip.chip_counter();
    chip.with(|chip| chip.lose_reply = true);
    assert_eq!(counter.advance(), Err(CounterError));
    assert_eq!(counter.value(), Ok(1));
    assert_eq!(counter.advance(), Ok(1));
}

#[test]
fn the_chip_counter_fails_with_no_chip() {
    let chip = FakeChip::new();
    chip.with(|chip| chip.present = false);
    let mut counter = chip.chip_counter();
    assert_eq!(counter.value(), Err(CounterError));
    assert_eq!(counter.advance(), Err(CounterError));
}

/// A counter in memory, shared so a test can look at it afterwards.
#[derive(Clone)]
struct Plain(Rc<RefCell<(u32, bool)>>);

impl Plain {
    fn at(value: u32) -> Self {
        Self(Rc::new(RefCell::new((value, false))))
    }

    fn get(&self) -> u32 {
        self.0.borrow().0
    }

    fn fail(&self) {
        self.0.borrow_mut().1 = true;
    }
}

impl OneWayCounter for Plain {
    fn value(&mut self) -> Result<u32, CounterError> {
        let (value, failing) = *self.0.borrow();
        if failing {
            Err(CounterError)
        } else {
            Ok(value)
        }
    }

    fn advance(&mut self) -> Result<u32, CounterError> {
        let value = self.value()?;
        self.0.borrow_mut().0 += 1;
        Ok(value)
    }
}

#[test]
fn a_pair_reads_as_the_larger_value() {
    assert_eq!(
        PairedCounter::new(Plain::at(3), Plain::at(9)).value(),
        Ok(9)
    );
    assert_eq!(
        PairedCounter::new(Plain::at(9), Plain::at(3)).value(),
        Ok(9)
    );
}

#[test]
fn a_pair_moves_both_counters() {
    let (a, b) = (Plain::at(4), Plain::at(4));
    let mut pair = PairedCounter::new(a.clone(), b.clone());
    assert_eq!(pair.advance(), Ok(4));
    assert_eq!((a.get(), b.get()), (5, 5));
}

#[test]
fn a_pair_brings_the_one_behind_up_to_the_other() {
    for (first, second) in [(2, 40), (40, 2)] {
        let (a, b) = (Plain::at(first), Plain::at(second));
        let mut pair = PairedCounter::new(a.clone(), b.clone());
        assert_eq!(pair.advance(), Ok(40));
        assert_eq!((a.get(), b.get()), (41, 41));
    }
}

#[test]
fn a_pair_refuses_a_gap_too_large_to_be_real() {
    let (a, b) = (Plain::at(0), Plain::at(MAX_CATCH_UP + 1));
    let mut pair = PairedCounter::new(a.clone(), b.clone());
    assert_eq!(pair.advance(), Err(CounterError));
    assert_eq!((a.get(), b.get()), (0, MAX_CATCH_UP + 1));
}

#[test]
fn a_pair_allows_the_largest_gap_it_can_close() {
    let (a, b) = (Plain::at(1), Plain::at(MAX_CATCH_UP));
    let mut pair = PairedCounter::new(a.clone(), b.clone());
    assert_eq!(pair.advance(), Ok(MAX_CATCH_UP));
    assert_eq!((a.get(), b.get()), (MAX_CATCH_UP + 1, MAX_CATCH_UP + 1));
}

#[test]
fn a_pair_never_falls_back_to_the_counter_that_still_works() {
    for failing in [0, 1] {
        let (a, b) = (Plain::at(4), Plain::at(4));
        [&a, &b][failing].fail();
        let mut pair = PairedCounter::new(a, b);
        assert_eq!(pair.value(), Err(CounterError));
        assert_eq!(pair.advance(), Err(CounterError));
    }
}

#[test]
fn a_pair_cut_halfway_never_goes_back() {
    let (a, b) = (Plain::at(4), Plain::at(4));
    let mut pair = PairedCounter::new(a.clone(), Stops(b.clone()));
    assert_eq!(pair.advance(), Err(CounterError));
    assert_eq!(a.get(), 5);
    assert_eq!(pair.value(), Ok(5));
}

/// Reads fine, but won't move.
struct Stops(Plain);

impl OneWayCounter for Stops {
    fn value(&mut self) -> Result<u32, CounterError> {
        self.0.value()
    }

    fn advance(&mut self) -> Result<u32, CounterError> {
        Err(CounterError)
    }
}

const CODE: Code = [1, 2, 3, 4, 5, 6];

type ChipPair = PairedCounter<ChipCounter<FakeChip>, FlashCounter<SimFlash>>;

fn open_with<C: OneWayCounter>(flash: &SimFlash, counter: C) -> Wallet<SimFlash, C> {
    Wallet::open(
        KeyStore::new(flash.clone()),
        Passcode::new(flash.clone(), layout::PASSCODE),
        counter,
        Settings::new(flash.clone()),
        MlKeyStore::new(flash.clone()),
        0,
    )
    .expect("the flash is healthy")
}

fn paired(flash: &SimFlash, chip: &FakeChip) -> ChipPair {
    PairedCounter::new(
        chip.chip_counter(),
        FlashCounter::new(flash.clone(), layout::COUNTER),
    )
}

/// One wallet with a key and a passcode, built once because trees take time.
fn wallet_image() -> &'static Vec<u8> {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let flash = SimFlash::new();
        let mut wallet = open_with(&flash, FlashCounter::new(flash.clone(), layout::COUNTER));
        wallet
            .create(&mut TestRng(7), |_| {})
            .expect("the flash is healthy");
        wallet
            .passcode()
            .create(&CODE, &mut TestRng(8))
            .expect("the flash is healthy");
        flash.image()
    })
}

fn flash_with_wallet() -> SimFlash {
    let flash = SimFlash::new();
    flash.poke(0, wallet_image());
    flash
}

/// Signs a transfer and returns the key index it used.
fn sign<C: OneWayCounter>(wallet: &mut Wallet<SimFlash, C>) -> Result<u32, WalletError> {
    let index = wallet.next_index()?;
    let tx = FrameTx::transfer([0x11; 20], 1, index, [0x22; 20], 1000).encode(&[]);
    let review = wallet.review(&tx).expect("the transaction is fine");
    let Ok(Attempt::Correct(proof)) = wallet.enter_passcode(&CODE, 0) else {
        panic!("the code is right");
    };
    wallet.sign(&review, proof).map(|_| index)
}

#[test]
fn restoring_old_flash_rewinds_a_flash_counter() {
    let flash = flash_with_wallet();
    let before = flash.image();
    let mut wallet = open_with(&flash, FlashCounter::new(flash.clone(), layout::COUNTER));
    assert_eq!(sign(&mut wallet), Ok(0));
    flash.poke(0, &before);
    let mut wallet = open_with(&flash, FlashCounter::new(flash.clone(), layout::COUNTER));
    assert_eq!(wallet.next_index(), Ok(0));
}

#[test]
fn restoring_old_flash_doesnt_rewind_the_chip() {
    let flash = flash_with_wallet();
    let chip = FakeChip::new();
    let before = flash.image();
    let mut wallet = open_with(&flash, paired(&flash, &chip));
    assert_eq!(sign(&mut wallet), Ok(0));
    assert_eq!(sign(&mut wallet), Ok(1));

    flash.poke(0, &before);
    let mut wallet = open_with(&flash, paired(&flash, &chip));
    assert_eq!(wallet.next_index(), Ok(2));
    assert_eq!(sign(&mut wallet), Ok(2));
}

#[test]
fn swapping_in_a_fresh_chip_doesnt_rewind_the_flash() {
    let flash = flash_with_wallet();
    let mut wallet = open_with(&flash, paired(&flash, &FakeChip::new()));
    assert_eq!(sign(&mut wallet), Ok(0));
    assert_eq!(sign(&mut wallet), Ok(1));

    let mut wallet = open_with(&flash, paired(&flash, &FakeChip::new()));
    assert_eq!(wallet.next_index(), Ok(2));
}

#[test]
fn a_wallet_used_before_the_chip_keeps_its_place() {
    let flash = flash_with_wallet();
    let mut wallet = open_with(&flash, FlashCounter::new(flash.clone(), layout::COUNTER));
    assert_eq!(sign(&mut wallet), Ok(0));
    assert_eq!(sign(&mut wallet), Ok(1));

    let chip = FakeChip::new();
    let mut wallet = open_with(&flash, paired(&flash, &chip));
    assert_eq!(sign(&mut wallet), Ok(2));
    assert_eq!(chip.counter(1), 3);
}

#[test]
fn turning_the_chip_off_again_keeps_the_place_too() {
    let flash = flash_with_wallet();
    let chip = FakeChip::new();
    chip.with(|chip| chip.counters = [0, 5]);
    let mut wallet = open_with(&flash, paired(&flash, &chip));
    assert_eq!(sign(&mut wallet), Ok(5));

    let mut wallet = open_with(&flash, FlashCounter::new(flash.clone(), layout::COUNTER));
    assert_eq!(wallet.next_index(), Ok(6));
}

#[test]
fn nothing_is_signed_without_the_chip() {
    let flash = flash_with_wallet();
    let chip = FakeChip::new();
    let mut wallet = open_with(&flash, paired(&flash, &chip));
    chip.with(|chip| chip.present = false);
    assert_eq!(sign(&mut wallet), Err(WalletError::CounterFailure));
}
