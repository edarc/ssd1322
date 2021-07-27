//! This module provides shims for the `embedded-hal` hardware corresponding to the SSD1322's
//! supported electrical/bus interfaces. It is a shim between `embedded-hal` implementations and
//! the display driver's command layer.

use nb;

/// An interface for the SSD1322 implements this trait, which provides the basic operations for
/// sending pre-encoded commands and data to the chip via the interface.
pub trait DisplayInterface {
    type Error;

    fn send_command(&mut self, cmd: u8) -> Result<(), Self::Error>;
    fn send_data(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
    fn send_data_async(&mut self, word: u8) -> nb::Result<(), Self::Error>;
}

pub mod spi {
    //! The SPI interface supports the "4-wire" interface of the driver, such that each word on the
    //! SPI bus is 8 bits. The "3-wire" mode is not supported, as it replaces the D/C GPIO with a
    //! 9th bit on each SPI word, and `embedded-hal` SPI traits do not currently support
    //! non-byte-aligned SPI word lengths.

    use embedded_hal as hal;

    use super::DisplayInterface;
    use nb;

    /// The union of all errors that may occur on the SPI interface. This consists of variants for
    /// the error types of the D/C GPIO and the SPI bus.
    #[derive(Debug)]
    pub enum SpiInterfaceError<DCE, SPIE> {
        DCError(DCE),
        SPIError(SPIE),
    }

    impl<DCE, SPIE> SpiInterfaceError<DCE, SPIE> {
        fn from_dc(e: DCE) -> Self {
            Self::DCError(e)
        }
        fn from_spi(e: SPIE) -> Self {
            Self::SPIError(e)
        }
    }

    /// A configured `DisplayInterface` for controlling an SSD1322 via 4-wire SPI.
    pub struct SpiInterface<SPI, DC> {
        /// The SPI master device connected to the SSD1322.
        spi: SPI,
        /// A GPIO output pin connected to the D/C (data/command) pin of the SSD1322 (the fourth
        /// "wire" of "4-wire" mode).
        dc: DC,
    }

    impl<SPI, DC> SpiInterface<SPI, DC>
    where
        SPI: hal::spi::FullDuplex<u8>,
        DC: hal::digital::v2::OutputPin,
    {
        /// Create a new SPI interface to communicate with the display driver. `spi` is the SPI
        /// master device, and `dc` is the GPIO output pin connected to the D/C pin of the SSD1322.
        pub fn new(spi: SPI, dc: DC) -> Self {
            Self { spi: spi, dc: dc }
        }
    }

    impl<SPI, DC> DisplayInterface for SpiInterface<SPI, DC>
    where
        SPI: hal::spi::FullDuplex<u8>,
        DC: hal::digital::v2::OutputPin,
    {
        type Error = SpiInterfaceError<
            <DC as hal::digital::v2::OutputPin>::Error,
            <SPI as hal::spi::FullDuplex<u8>>::Error,
        >;

        /// Send a command word to the display's command register. Synchronous.
        fn send_command(&mut self, cmd: u8) -> Result<(), Self::Error> {
            // The SPI device has FIFOs that we must ensure are drained before the bus will
            // quiesce. This must happen before asserting DC for a command.
            while let Ok(_) = self.spi.read() {
                self.dc.set_high().map_err(Self::Error::from_dc)?;
            }
            self.dc.set_low().map_err(Self::Error::from_dc)?;
            let bus_op = nb::block!(self.spi.send(cmd))
                .and_then(|_| nb::block!(self.spi.read()))
                .map_err(Self::Error::from_spi)
                .map(core::mem::drop);
            self.dc.set_high().map_err(Self::Error::from_dc)?;
            bus_op
        }

        /// Send a sequence of data words to the display from a buffer. Synchronous.
        fn send_data(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            for word in buf {
                nb::block!(self.spi.send(word.clone())).map_err(Self::Error::from_spi)?;
                nb::block!(self.spi.read()).map_err(Self::Error::from_spi)?;
            }
            Ok(())
        }

        /// Send a data word to the display asynchronously, using `nb` style non-blocking send. If
        /// the hardware FIFO is full, returns `WouldBlock` which means the word was not accepted
        /// and should be retried later.
        fn send_data_async(&mut self, word: u8) -> nb::Result<(), Self::Error> {
            match self.spi.send(word) {
                Ok(()) => {
                    let _ = self.spi.read();
                    Ok(())
                }
                Err(nb::Error::Other(e)) => Err(nb::Error::Other(Self::Error::from_spi(e))),
                Err(nb::Error::WouldBlock) => Err(nb::Error::WouldBlock),
            }
        }
    }
}

#[cfg(test)]
pub mod test_spy {
    //! An interface for use in unit tests to spy on whatever was sent to it.

    use super::DisplayInterface;
    use nb;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Debug, PartialEq)]
    pub enum Sent {
        Cmd(u8),
        Data(Vec<u8>),
    }

    pub struct TestSpyInterface {
        sent: Rc<RefCell<Vec<Sent>>>,
    }

    impl TestSpyInterface {
        pub fn new() -> Self {
            TestSpyInterface {
                sent: Rc::new(RefCell::new(Vec::new())),
            }
        }
        pub fn split(&self) -> Self {
            Self {
                sent: self.sent.clone(),
            }
        }
        pub fn check(&self, cmd: u8, data: &[u8]) {
            let sent = self.sent.borrow();
            if data.len() == 0 {
                assert_eq!(sent.len(), 1);
            } else {
                assert_eq!(sent.len(), 2);
                assert_eq!(sent[1], Sent::Data(data.to_vec()));
            }
            assert_eq!(sent[0], Sent::Cmd(cmd));
        }
        pub fn check_multi(&self, expect: &[Sent]) {
            assert_eq!(*self.sent.borrow(), expect);
        }
        pub fn clear(&mut self) {
            self.sent.borrow_mut().clear()
        }
    }

    impl DisplayInterface for TestSpyInterface {
        type Error = core::convert::Infallible;

        fn send_command(&mut self, cmd: u8) -> Result<(), Self::Error> {
            self.sent.borrow_mut().push(Sent::Cmd(cmd));
            Ok(())
        }
        fn send_data(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            self.sent.borrow_mut().push(Sent::Data(data.to_vec()));
            Ok(())
        }
        fn send_data_async(&mut self, word: u8) -> nb::Result<(), Self::Error> {
            let mut sent = self.sent.borrow_mut();
            {
                let last_idx = sent.len() - 1;
                match &mut sent[last_idx] {
                    Sent::Cmd(_) => {}
                    Sent::Data(ref mut d) => {
                        d.push(word);
                        return Ok(());
                    }
                };
            }
            sent.push(Sent::Data(vec![word]));
            Ok(())
        }
    }
}
