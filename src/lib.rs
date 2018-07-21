//! Driver library for the Solomon Systech SSD1322 dot matrix OLED display driver.

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(euclidean_division)]

#[cfg(feature = "std")]
extern crate core;

extern crate embedded_hal as hal;
#[macro_use]
extern crate itertools;
extern crate nb;

pub mod command;
pub mod config;
pub mod display;
pub mod interface;

// Re-exports for primary API.
pub use command::{consts, ComLayout, ComScanDirection};
pub use config::Config;
pub use display::{Display, PixelCoord};
pub use interface::spi::SpiInterface;
