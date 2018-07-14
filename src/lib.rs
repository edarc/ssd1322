//! Driver library for the Solomon Systech SSD1322 dot matrix OLED display driver.

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(euclidean_division)]

#[cfg(feature = "std")]
extern crate core;

extern crate embedded_hal as hal;
#[macro_use]
extern crate itertools;

pub mod command;
pub mod config;
pub mod display;
pub mod interface;
