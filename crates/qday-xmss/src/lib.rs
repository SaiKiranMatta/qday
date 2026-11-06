//! XMSS-style one-time signatures over keccak256. See `docs/xmss-spec.md`.

#![no_std]

extern crate alloc;

mod adrs;
mod hash;
mod keys;
mod tree;
mod wots;

pub use adrs::Adrs;
pub use hash::{Hash, keccak256};
pub use keys::KeySet;
pub use tree::{HEIGHT, LEAVES, NODES, Tree, node_slot};
pub use wots::{CHAINS, digits};
