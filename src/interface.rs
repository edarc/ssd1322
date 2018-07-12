pub trait DisplayInterface {
    fn send_command(&mut self, cmd: u8) -> Result<(), ()>;
    fn send_data(&mut self, buf: &[u8]) -> Result<(), ()>;
}

pub mod spi {
    //! The SPI interface supports the "4-wire" interface of the driver, such that each word on the
    //! SPI bus is 8 bits. The "3-wire" mode replaces the D/C GPIO with a 9th bit on each word,
    //! which seems really awkward to implement with embedded_hal SPI.

    use hal;

    use super::DisplayInterface;

    pub struct SpiInterface<SPI, DC> {
        /// The SPI master device connected to the SSD1322.
        spi: SPI,
        /// A GPIO output pin connected to the D/C (data/command) pin of the SSD1322 (the fourth
        /// "wire" of "4-wire" mode).
        dc: DC,
    }

    impl<SPI, DC> SpiInterface<SPI, DC>
    where
        SPI: hal::blocking::spi::Write<u8>,
        DC: hal::digital::OutputPin,
    {
        /// Create a new SPI interface to communicate with the display driver. `spi` is the SPI
        /// master device, and `dc` is the GPIO output pin connected to the D/C pin of the SSD1322.
        pub fn new(spi: SPI, dc: DC) -> Self {
            Self { spi, dc }
        }
    }

    impl<SPI, DC> DisplayInterface for SpiInterface<SPI, DC>
    where
        SPI: hal::blocking::spi::Write<u8>,
        DC: hal::digital::OutputPin,
    {
        fn send_command(&mut self, cmd: u8) -> Result<(), ()> {
            self.dc.set_low();
            self.spi.write(&[cmd]).map_err(|_| ())?;
            self.dc.set_high();
            Ok(())
        }

        fn send_data(&mut self, buf: &[u8]) -> Result<(), ()> {
            self.dc.set_high();
            self.spi.write(&buf).map_err(|_| ())?;
            Ok(())
        }
    }
}

#[cfg(test)]
pub mod test_spy {
    //! An interface for use in unit tests to spy on whatever was sent to it.

    use super::DisplayInterface;

    pub struct TestSpyInterface {
        cmd: Option<u8>,
        data: Vec<u8>,
    }

    impl TestSpyInterface {
        pub fn new() -> Self {
            TestSpyInterface {
                cmd: None,
                data: Vec::new(),
            }
        }
        pub fn check(&self, cmd: u8, data: &[u8]) {
            assert_eq!(self.cmd, Some(cmd));
            assert_eq!(self.data, data);
        }
        pub fn clear(&mut self) {
            self.data.clear()
        }
    }

    impl DisplayInterface for TestSpyInterface {
        fn send_command(&mut self, cmd: u8) -> Result<(), ()> {
            self.cmd = Some(cmd);
            Ok(())
        }
        fn send_data(&mut self, data: &[u8]) -> Result<(), ()> {
            self.data.extend(data.iter().cloned());
            Ok(())
        }
    }
}
