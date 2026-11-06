//! What the device shows, kept free of hardware so every screen can be rendered on a laptop.

#![no_std]

extern crate alloc;

pub mod fb;
pub mod fonts;
pub mod format;
pub mod gallery;
pub mod keys;
pub mod screens;
pub mod theme;
