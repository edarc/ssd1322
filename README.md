# SSD1322 OLED display driver

*Work in progress!*

Driver for the SSD1322 OLED display for use with
[embedded-hal](https://crates.io/crates/embedded-hal).

Initial release will focus on 4-wire SPI interface, as well as a solution to
avoid buffering the entire display RAM in the host. At the chip's maximum
supported resolution of 480x128x4bpp, a full host-side buffer would consume a
colossal (for a μC) 30kiB of RAM.

## Acknowledgements

[jamwaffles/ssd1306](https://github.com/jamwaffles/ssd1306) for internal design
inspiration.

## License

Licensed under either of

- Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
