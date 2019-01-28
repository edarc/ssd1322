//! Driver library for the Solomon Systech SSD1322 dot matrix OLED display driver.
//!
//! This driver is intended to work on embedded platforms using any implementation of the
//! `embedded-hal` trait library.
//!
//! Because the SSD1322 supports displays as large as 480x128 @ 4bpp, the primary API uses a
//! `Region` abstraction to allow writing a stream of pixel data from an iterator onto a
//! rectangular sub-region of the display area. This avoids the requirement to buffer the entire
//! display RAM in the host, since such a buffer would consume a colossal (for a μC) 30kiB of RAM.
//!
//! To use the driver:
//!
//! - Use your platform's `embedded-hal` implementation to obtain the necessary I/Os where your
//!   SSD1322 display is connected. For example, in 4-wire SPI mode, you will need a configured SPI
//!   master device and one GPIO push-pull output pin device.
//!
//! - Construct a `DisplayInterface`, for example an `SpiInterface`, which will take ownership of
//!   the I/Os you just obtained.
//!
//! - Construct a `Display`, which will take ownership of the `DisplayInterface` along with the
//!   display resolution and offset parameters.
//!
//! - Referring to your display module's datasheet, create a `Config` to set the various parameters
//!   in the chip appropriately for the OLEDs in your display module, and send it to the display
//!   with `Display::init`.
//!
//! - To draw, call `Display::region` or `Display::overscanned_region` to obtain a region instance
//!   for the rectangular area where you want to write image information. Use the `draw_packed` or
//!   `draw` methods of the region to write image data supplied by an iterator. The region is
//!   intended to be short-lived and will mutably borrow the display, so the compiler will prevent
//!   accidental clashing writes.
//!
//! - Other functions of the device, such as sleep mode, vertical pan, and contrast control, are
//!   available via methods on `Display`.
//!
//! Example code is available in the `examples` folder.

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(euclidean_division)]

#[cfg(feature = "std")]
extern crate core;

extern crate embedded_hal as hal;
#[macro_use]
extern crate itertools;
#[macro_use]
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
