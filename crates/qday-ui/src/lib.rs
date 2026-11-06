//! The device UI: a small anti-aliased renderer over an RGB565 framebuffer, the screens and themes,
//! and the app that drives them. It has no hardware dependencies, so every screen can be rendered
//! and checked on the laptop.

#![no_std]

extern crate alloc;

pub mod app;
pub mod canvas;
pub mod fb;
pub mod fonts;
pub mod format;
pub mod nav;
pub mod screens;
pub mod text;
pub mod theme;
pub mod ui;
pub mod widgets;
