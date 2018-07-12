//! Driver library for the Solomon Systech SSD1322 dot matrix OLED display driver.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate core;

extern crate embedded_hal as hal;

pub mod command;
pub mod config;
pub mod display;
pub mod interface;
