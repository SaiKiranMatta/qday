//! Everything the device decides, kept free of hardware so it can be tested on a laptop.

#![no_std]

extern crate alloc;

pub mod backup;
pub mod frame;
pub mod hex;
pub mod rlp;
