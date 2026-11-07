//! Everything the device decides, kept free of hardware so it can be tested on a laptop.

#![no_std]

extern crate alloc;

pub mod backup;
pub mod counter;
pub mod flash;
pub mod frame;
pub mod hex;
pub mod input;
pub mod keystore;
pub mod lockdown;
pub mod mlkey;
pub mod passcode;
pub mod protocol;
pub mod rlp;
pub mod rng;
pub mod seal;
pub mod settings;
pub mod stealth;
pub mod submission;
pub mod tokens;
pub mod wallet;
