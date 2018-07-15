//! Region abstraction for drawing into rectangular regions of the display.

use command::{BufCommand, Command};
use display::PixelCoord;
use interface;
use nb;

/// A handle to a rectangular region of a display which can be drawn into.
pub struct Region<'di, DI>
where
    DI: 'di + interface::DisplayInterface,
{
    iface: &'di mut DI,
    top: u8,
    rows: u8,
    buf_left: u8,
    buf_cols: u8,
    pixel_cols: u16,
}

impl<'di, DI> Region<'di, DI>
where
    DI: 'di + interface::DisplayInterface,
{
    /// Construct a new region. This is only called by the factory method `Display::region`, which
    /// checks that the region coordinates are within the viewable area and pre-compensates the
    /// column coordinates for the display column offset.
    pub(super) fn new(iface: &'di mut DI, upper_left: PixelCoord, lower_right: PixelCoord) -> Self {
        let pixel_cols = lower_right.0 - upper_left.0;
        Self {
            iface: iface,
            top: upper_left.1 as u8,
            rows: (lower_right.1 - upper_left.1) as u8,
            buf_left: (upper_left.0 / 4) as u8,
            buf_cols: (pixel_cols / 4) as u8,
            pixel_cols: pixel_cols as u16,
        }
    }

    /// Draw packed-pixel image data into the region, such that each byte is two 4-bit gray scale
    /// values of horizontally-adjacent pixels. Pixels are drawn left-to-right and top-to-bottom.
    pub fn draw_packed<I>(&mut self, mut iter: I) -> Result<(), ()>
    where
        I: Iterator<Item = u8>,
    {
        // Set the row and column address registers and put the display in write mode.
        Command::SetColumnAddress(self.buf_left, self.buf_left + self.buf_cols - 1)
            .send(self.iface)?;
        Command::SetRowAddress(self.top, self.top + self.rows - 1).send(self.iface)?;
        BufCommand::WriteImageData(&[]).send(self.iface)?;

        // Paint the region using asynchronous writes so that iter.next() may run concurrently with
        // the SPI write cycle for a small throughput win.
        let region_total_bytes = self.pixel_cols as usize * self.rows as usize / 2;
        let mut total_written = 0;
        let mut next_byte: u8;

        loop {
            // Break early if we have copied enough bytes to exactly fill the region.
            if total_written >= region_total_bytes {
                break;
            }

            // Break early if the iterator runs out of bytes.
            match iter.next() {
                Some(pixels) => {
                    total_written += 1;
                    next_byte = pixels;
                }
                None => break,
            }

            // Write the byte to the interface FIFO. If the FIFO is full then poll it until the
            // send succeeds before continuing the outer loop to consume the next byte from the
            // iterator.
            loop {
                match self.iface.send_data_async(next_byte) {
                    Ok(()) => break,
                    Err(nb::Error::WouldBlock) => {}
                    Err(nb::Error::Other(())) => return Err(()),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use command::{ComLayout, ComScanDirection};
    use config::Config;
    use display::{Display, PixelCoord as Px};
    use interface::test_spy::{Sent, TestSpyInterface};

    #[test]
    fn draw_packed() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.region(Px(12, 10), Px(16, 12)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [3, 3],
            0x75, [10, 11],
            0x5C, [0xDE, 0xAD, 0xBE, 0xEF]
        ));
    }

    #[test]
    fn draw_packed_end_at_region_filled() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.region(Px(12, 10), Px(16, 12)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF, 0xAA].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [3, 3],
            0x75, [10, 11],
            0x5C, [0xDE, 0xAD, 0xBE, 0xEF]
        ));
        di.clear();
    }

    #[test]
    fn draw_packed_end_at_iterator_exhausted() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.region(Px(12, 10), Px(16, 12)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [3, 3],
            0x75, [10, 11],
            0x5C, [0xDE, 0xAD, 0xBE]
        ));
        di.clear();
    }

    #[test]
    fn draw_packed_display_column_offset() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(64, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.region(Px(0, 10), Px(4, 12)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [16, 16],
            0x75, [10, 11],
            0x5C, [0xDE, 0xAD, 0xBE, 0xEF]
        ));
        di.clear();
    }
}
