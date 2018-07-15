//! The display interface, which uses command module at a slightly higher level. It provides a
//! builder API to configure the display, and methods for writing image data into display regions.

use command::*;
use config::{Config, PersistentConfig};
use interface;

#[derive(Clone, Copy, Debug)]
pub struct PixelCoord(pub i16, pub i16);

/// The basic driver for the display.
pub struct Display<DI>
where
    DI: interface::DisplayInterface,
{
    iface: DI,
    display_size: PixelCoord,
    display_offset: PixelCoord,
    persistent_config: Option<PersistentConfig>,
}

impl<DI> Display<DI>
where
    DI: interface::DisplayInterface,
{
    /// Construct a new display driver for a display of a particular size.
    pub fn new(iface: DI, display_size: PixelCoord, display_offset: PixelCoord) -> Self {
        if false
            || display_size.0 > NUM_PIXEL_COLS as i16
            || display_size.1 > NUM_PIXEL_ROWS as i16
            || display_offset.0 + display_size.0 > NUM_PIXEL_COLS as i16
            || display_offset.1 + display_size.1 > NUM_PIXEL_ROWS as i16
            || display_size.0.mod_euc(4) != 0
            || display_offset.0.mod_euc(4) != 0
        {
            panic!("Display size or column offset not supported by SSD1322.");
        }
        Display {
            iface: iface,
            display_size: display_size,
            display_offset: display_offset,
            persistent_config: None,
        }
    }

    /// Initialize the display with a config message.
    pub fn init(&mut self, config: Config) -> Result<(), ()> {
        self.sleep(true)?;
        Command::SetDisplayMode(DisplayMode::BlankDark).send(&mut self.iface)?;
        config.send(&mut self.iface)?;
        self.persistent_config = Some(config.persistent_config);
        Command::SetMuxRatio(self.display_size.1 as u8).send(&mut self.iface)?;
        Command::SetDisplayOffset(self.display_offset.1 as u8).send(&mut self.iface)?;
        Command::SetStartLine(0).send(&mut self.iface)?;
        self.persistent_config.as_ref().unwrap().send(
            &mut self.iface,
            IncrementAxis::Horizontal,
            ColumnRemap::Forward,
            NibbleRemap::Forward,
        )?;
        self.sleep(false)?;
        Command::SetDisplayMode(DisplayMode::Normal).send(&mut self.iface)
    }

    /// Control sleep mode.
    pub fn sleep(&mut self, enabled: bool) -> Result<(), ()> {
        Command::SetSleepMode(enabled).send(&mut self.iface)
    }

    /// Control the master contrast.
    pub fn contrast(&mut self, contrast: u8) -> Result<(), ()> {
        Command::SetMasterContrast(contrast).send(&mut self.iface)
    }

    /// Set the vertical pan.
    pub fn vertical_pan(&mut self, offset: u8) -> Result<(), ()> {
        Command::SetStartLine(offset).send(&mut self.iface)
    }

    /// Construct a rectangular region onto which to draw image data. The region start and end
    /// horizontal coordinates must be divisible by 4, because pixels can only be addressed by
    /// column (groups of 4), not individually. The region rectangle must also be within the
    /// viewable area of the display buffer, where the viewable area includes all 128 rows to
    /// support vertical panning.
    pub fn region<'di>(
        &'di mut self,
        upper_left: PixelCoord,
        lower_right: PixelCoord,
    ) -> Result<Region<'di, DI>, ()> {
        // The row fields are bounds-checked against the chip's maximum supported row rather than
        // the display size, because the display supports vertical scrolling by adding an offset to
        // the memory address that corresponds to row 0 (`SetStartLine` command). This feature
        // makes it possible to "pan" displays with fewer rows up and down over the entire 128
        // buffer rows. So, allow users to draw in that area even if it's currently hidden.
        //
        // The chip does not have any such panning support for buffer column addresses outside of
        // the display's viewable area, so even though the chip allows data to be written there, it
        // is probbly an error because it can never be read back and can never be visible on the
        // display. So, check column values against the display size and do not allow drawing
        // outside them.
        if false
            || upper_left.0 > self.display_size.0
            || lower_right.0 > self.display_size.0
            || upper_left.1 > NUM_PIXEL_ROWS as i16
            || lower_right.1 > NUM_PIXEL_ROWS as i16
            || upper_left.0 >= lower_right.0
            || upper_left.1 >= lower_right.1
            || upper_left.0.mod_euc(4) != 0
            || lower_right.0.mod_euc(4) != 0
        {
            return Err(());
        }

        // The column offset only is added to the pixel coordinates of the region. The row offset
        // is handled by the display driver itself using the `SetDisplayOffset` command.
        let ul = PixelCoord(upper_left.0 + self.display_offset.0, upper_left.1);
        let lr = PixelCoord(lower_right.0 + self.display_offset.0, lower_right.1);
        Ok(Region::new(&mut self.iface, ul, lr))
    }
}

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

        // Paint the region using constant memory by allocating a chunk buffer and alternately
        // filling it from the iterator and writing it to the display.
        let region_total_bytes = self.pixel_cols as usize * self.rows as usize / 2;
        let mut total_written = 0;
        let mut buf = [0u8; 32];

        loop {
            // For each of the buffer slots, fill it with a byte from the iterator, tracking how
            // many slots are filled and how many bytes have been copied in total for the entire
            // method call.
            let mut chunk_len = 0;
            for slot in buf.iter_mut() {
                // Break early if we have copied enough bytes to exactly fill the region.
                if total_written >= region_total_bytes {
                    break;
                }

                // Break early if the iterator runs out of bytes.
                if let Some(pixels) = iter.next() {
                    chunk_len += 1;
                    total_written += 1;
                    *slot = pixels;
                }
            }

            // Write the 32-byte chunk to the display buffer.
            // TODO: use non-blocking HAL to let interface write while next buffer fills?
            self.iface.send_data(&buf[..chunk_len])?;

            // We are done if the inner loop exited early, because that would mean either the
            // region has been painted in full or the iterator is exhausted.
            if chunk_len != buf.len() {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PixelCoord as Px, *};
    use interface::test_spy::{Sent, TestSpyInterface};

    macro_rules! send {
        ([$($d:tt),*]) => {Sent::Data(vec![$($d,)*])};
        ($c:tt) => {Sent::Cmd($c)};
    }
    macro_rules! sends {
        ($($e:tt),*) => {&[$(send!($e),)*]};
    }

    #[test]
    fn init_defaults() {
        let di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0xAE, // sleep enable
            0xA4, // display blank
            0xCA, [63], // mux ratio 64 lines
            0xA2, [0], // display offset 0
            0xA1, [0], // start line 0
            0xA0, [0b00010100, 0b00010001], // remapping
            0xAF, // sleep disable
            0xA6 // display normal
        ));
    }

    #[test]
    fn init_many_options() {
        let di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(256, 128), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive)
            .contrast_current(160)
            .phase_lengths(5, 14)
            .clock_fosc_divset(7, 0)
            .display_enhancements(true, false)
            .second_precharge_period(4)
            .precharge_voltage(5)
            .com_deselect_voltage(6);
        disp.init(cfg).unwrap();
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0xAE, // sleep enable
            0xA4, // display blank
            0xB1, [0xE2], // phase lengths
            0xC1, [160], // contrast current
            0xB3, [0x70], // clock
            0xB4, [0b10100000, 0b10110101], // display enhancements
            0xB6, [4], // second precharge
            0xBB, [5], // precharge voltage
            0xBE, [6], // com deselect voltage
            0xCA, [127], // mux ratio 128 lines
            0xA2, [0], // display offset 0
            0xA1, [0], // start line 0
            0xA0, [0b00010100, 0b00010001], // remapping
            0xAF, // sleep disable
            0xA6 // display normal
        ));
    }

    #[test]
    fn init_row_offset() {
        let di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 32));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0xAE, // sleep enable
            0xA4, // display blank
            0xCA, [63], // mux ratio 64 lines
            0xA2, [32], // display offset 32
            0xA1, [0], // start line 0
            0xA0, [0b00010100, 0b00010001], // remapping
            0xAF, // sleep disable
            0xA6 // display normal
        ));
    }

    #[test]
    fn region_build() {
        let di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();

        // In range, correctly ordered, and columns in 4s.
        assert!(disp.region(Px(12, 10), Px(20, 12)).is_ok());
        assert!(disp.region(Px(0, 0), Px(128, 64)).is_ok());

        // Columns not in 4s.
        assert!(disp.region(Px(12, 10), Px(21, 12)).is_err());
        assert!(disp.region(Px(13, 10), Px(20, 12)).is_err());

        // Incorrectly ordered.
        assert!(disp.region(Px(20, 10), Px(12, 12)).is_err());
        assert!(disp.region(Px(12, 12), Px(20, 10)).is_err());

        // Column out of range.
        assert!(disp.region(Px(124, 4), Px(132, 6)).is_err());
        // Row out of display range but not buffer range: not an error! The display can "pan"
        // vertically to see this region.
        assert!(disp.region(Px(12, 60), Px(20, 128)).is_ok());
        // Row out of buffer range: error.
        assert!(disp.region(Px(4, 60), Px(20, 130)).is_err());
    }

    #[test]
    fn region_draw_packed() {
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
    fn region_draw_packed_end_at_region_filled() {
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
    fn region_draw_packed_end_at_iterator_exhausted() {
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
    fn region_draw_packed_multiple_chunks() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.region(Px(0, 10), Px(68, 11)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cycle().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [0, 16],
            0x75, [10, 10],
            0x5C, [0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF,
                   0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF,
                   0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF],
                  [0xDE, 0xAD]
        ));
        di.clear();
    }

    #[test]
    fn region_draw_packed_display_column_offset() {
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
