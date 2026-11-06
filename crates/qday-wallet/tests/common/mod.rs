//! A simulated NOR flash that can lose power part way through any operation.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use qday_wallet::flash::{Flash, FlashError, SECTOR};

/// Size of the `qday` partition.
pub const PARTITION: usize = 0x40000;

struct State {
    bytes: Vec<u8>,
    /// Writes and erases allowed before the power goes.
    budget: Option<u32>,
    dead: bool,
    operations: u32,
    erases: u32,
}

/// Clones share the same flash, as handles to one chip do.
#[derive(Clone)]
pub struct SimFlash(Rc<RefCell<State>>);

impl SimFlash {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(State {
            bytes: vec![0xFF; PARTITION],
            budget: None,
            dead: false,
            operations: 0,
            erases: 0,
        })))
    }

    /// Cuts the power on the operation after `budget` more have finished.
    pub fn cut_after(&self, budget: u32) {
        self.0.borrow_mut().budget = Some(budget);
    }

    /// Restores power, keeping whatever reached the flash.
    pub fn power_on(&self) {
        let mut state = self.0.borrow_mut();
        state.budget = None;
        state.dead = false;
    }

    pub fn was_cut(&self) -> bool {
        self.0.borrow().dead
    }

    /// Writes and erases so far.
    pub fn operations(&self) -> u32 {
        self.0.borrow().operations
    }

    pub fn erases(&self) -> u32 {
        self.0.borrow().erases
    }

    /// A full copy, which is what an attacker with a flash reader would keep.
    pub fn image(&self) -> Vec<u8> {
        self.0.borrow().bytes.clone()
    }

    /// Whether `needle` appears anywhere in the flash.
    pub fn contains(&self, needle: &[u8]) -> bool {
        self.0
            .borrow()
            .bytes
            .windows(needle.len())
            .any(|window| window == needle)
    }

    /// Overwrites flash directly, as a fault or an attacker would.
    pub fn poke(&self, offset: usize, data: &[u8]) {
        self.0.borrow_mut().bytes[offset..offset + data.len()].copy_from_slice(data);
    }

    pub fn peek(&self, offset: usize, length: usize) -> Vec<u8> {
        self.0.borrow().bytes[offset..offset + length].to_vec()
    }

    /// Returns false, and tears the operation, once the power has gone.
    fn spend(state: &mut State) -> bool {
        if state.dead {
            return false;
        }
        state.operations += 1;
        match &mut state.budget {
            Some(0) => {
                state.dead = true;
                false
            }
            Some(left) => {
                *left -= 1;
                true
            }
            None => true,
        }
    }
}

impl Flash for SimFlash {
    fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), FlashError> {
        let state = self.0.borrow();
        if state.dead {
            return Err(FlashError);
        }
        let start = offset as usize;
        buffer.copy_from_slice(&state.bytes[start..start + buffer.len()]);
        Ok(())
    }

    fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), FlashError> {
        let mut state = self.0.borrow_mut();
        let was_dead = state.dead;
        let whole = Self::spend(&mut state);
        if was_dead {
            return Err(FlashError);
        }
        // A torn write lands its first half, and only some bits of the next byte.
        let landed = if whole { data.len() } else { data.len() / 2 };
        let start = offset as usize;
        for (slot, byte) in state.bytes[start..].iter_mut().zip(&data[..landed]) {
            *slot &= byte;
        }
        if !whole && landed < data.len() {
            state.bytes[start + landed] &= data[landed] | 0xF0;
        }
        if whole { Ok(()) } else { Err(FlashError) }
    }

    fn erase(&mut self, offset: u32) -> Result<(), FlashError> {
        assert_eq!(offset % SECTOR, 0, "erases are sector aligned");
        let mut state = self.0.borrow_mut();
        let was_dead = state.dead;
        let whole = Self::spend(&mut state);
        if was_dead {
            return Err(FlashError);
        }
        state.erases += 1;
        // A torn erase clears only the front of the sector.
        let cleared = if whole {
            SECTOR as usize
        } else {
            SECTOR as usize / 2
        };
        let start = offset as usize;
        state.bytes[start..start + cleared].fill(0xFF);
        if whole { Ok(()) } else { Err(FlashError) }
    }
}

/// Hands out predictable "random" bytes, from `SplitMix64`.
pub struct TestRng(pub u64);

impl qday_wallet::rng::Rng for TestRng {
    fn fill(&mut self, buffer: &mut [u8]) {
        for slot in buffer {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut mixed = self.0;
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *slot = (mixed ^ (mixed >> 31)).to_be_bytes()[0];
        }
    }
}
