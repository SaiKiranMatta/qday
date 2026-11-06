//! XMSS-style one-time signatures over keccak256. See `docs/xmss-spec.md`.

#![no_std]

extern crate alloc;

mod adrs;
mod hash;
mod keys;
mod sign;
mod tree;
mod wots;

pub use adrs::Adrs;
pub use hash::{Hash, Hasher, keccak256};
pub use keys::KeySet;
pub use sign::{PublicKey, SIG_BYTES, SignError, verify};
pub use tree::{HEIGHT, LEAVES, NODES, Tree, node_slot};
pub use wots::{CHAINS, digits};
