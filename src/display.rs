//! The display interface, which uses command module at a slightly higher level. It provides a
//! builder API to configure the display, and methods for writing image data into display regions.

use command::*;
use config::{Config, PersistentConfig};
use interface;

#[derive(Clone, Copy, Debug)]
pub struct PixelCoord(pub u16, pub u8);
#[derive(Clone, Copy, Debug)]
pub struct BufCoord(pub u8, pub u8);

impl From<BufCoord> for PixelCoord {
    fn from(bc: BufCoord) -> Self {
        PixelCoord(bc.0 as u16 * 4, bc.1)
    }
}

/// The basic driver for the display.
pub struct Display<DI>
where
    DI: interface::DisplayInterface,
{
    iface: DI,
    display_size: PixelCoord,
    persistent_config: Option<PersistentConfig>,
}

impl<DI> Display<DI>
where
    DI: interface::DisplayInterface,
{
    /// Construct a new display driver for a display of a particular size.
    pub fn new(iface: DI, display_size: PixelCoord) -> Self {
        Display {
            iface: iface,
            display_size: display_size,
            persistent_config: None,
        }
    }

    /// Initialize the display with a config message.
    pub fn init(&mut self, config: Config) -> Result<(), ()> {
        Command::SetDisplayMode(DisplayMode::BlankDark).send(&mut self.iface)?;
        config.send(&mut self.iface)?;
        self.persistent_config = Some(config.persistent_config);
        Command::SetMuxRatio(self.display_size.1).send(&mut self.iface)?;
        Command::SetDisplayOffset(0).send(&mut self.iface)?;
        Command::SetStartLine(0).send(&mut self.iface)?;
        self.persistent_config.as_ref().unwrap().send(
            &mut self.iface,
            IncrementAxis::Horizontal,
            ColumnRemap::Forward,
            NibbleRemap::Forward,
        )?;
        Command::SetDisplayMode(DisplayMode::Normal).send(&mut self.iface)
    }

    /// Construct a rectangular region onto which to draw image data. The region rectangle is
    /// specified in buffer coordinates, not pixel coordinates, because pixels can only be
    /// addressed by column (groups of 4), not individually.
    pub fn region<'di>(
        &'di mut self,
        upper_left: BufCoord,
        lower_right: BufCoord,
    ) -> Result<Region<'di, DI>, ()> {
        let ul_px: PixelCoord = upper_left.into();
        let lr_px: PixelCoord = lower_right.into();
        if ul_px.0 > self.display_size.0
            || lr_px.0 > self.display_size.0
            || ul_px.1 > self.display_size.1
            || lr_px.1 > self.display_size.1
            || upper_left.0 >= lower_right.0
            || upper_left.1 >= lower_right.1
        {
            return Err(());
        }
        Ok(Region::new(&mut self.iface, upper_left, lower_right))
    }
}

/// A handle to a rectangular region of a display which can be drawn into.
pub struct Region<'di, DI>
where
    DI: 'di + interface::DisplayInterface,
{
    iface: &'di mut DI,
    buf_upper_left: BufCoord,
    buf_cols: u8,
    pixel_cols: u16,
    rows: u8,
}

impl<'di, DI> Region<'di, DI>
where
    DI: 'di + interface::DisplayInterface,
{
    /// Construct a new region. This is intended to be accessible only to the factory method
    /// `Display::region`.
    pub(super) fn new(iface: &'di mut DI, upper_left: BufCoord, lower_right: BufCoord) -> Self {
        let buf_cols = lower_right.0 - upper_left.0;
        Self {
            iface: iface,
            buf_upper_left: upper_left,
            buf_cols: buf_cols,
            pixel_cols: buf_cols as u16 * 4,
            rows: lower_right.1 - upper_left.1,
        }
    }

    /// Draw packed-pixel image data into the region, such that each byte is two 4-bit gray scale
    /// values of horizontally-adjacent pixels. The buffer provided must be pixel_cols*rows/4, or
    /// buf_cols*rows*2 bytes long, or the method will return `Err(())`.
    pub fn draw_packed(&mut self, packed_pixels: &[u8]) -> Result<(), ()> {
        let expect_total_pixels = self.pixel_cols as usize * self.rows as usize;
        if packed_pixels.len() != expect_total_pixels / 4 {
            return Err(());
        }
        Command::SetColumnAddress(
            self.buf_upper_left.0,
            self.buf_upper_left.0 + self.buf_cols - 1,
        ).send(self.iface)?;
        Command::SetRowAddress(self.buf_upper_left.1, self.buf_upper_left.1 + self.rows - 1)
            .send(self.iface)?;
        BufCommand::WriteImageData(packed_pixels).send(self.iface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut disp = Display::new(di.split(), PixelCoord(128, 64));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0xA4, // display blank
            0xCA, [63], // mux ratio 64 lines
            0xA2, [0], // display offset 0
            0xA1, [0], // start line 0
            0xA0, [0b00010100, 0b00010001], // remapping
            0xA6 // display normal
        ));
    }

    #[test]
    fn init_many_options() {
        let di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), PixelCoord(256, 128));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive)
            .contrast_current(160)
            .phase_lengths(5, 5)
            .clock_fosc_divset(7, 0)
            .display_enhancements(true, false)
            .second_precharge_period(4)
            .precharge_voltage(5)
            .com_deselect_voltage(6);
        disp.init(cfg).unwrap();
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0xA4, // display blank
            0xB1, [0x25], // phase lengths
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
            0xA6 // display normal
        ));
    }

    #[test]
    fn region_build() {
        let di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), PixelCoord(128, 64));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();

        assert!(disp.region(BufCoord(3, 10), BufCoord(5, 12)).is_ok());
        assert!(disp.region(BufCoord(0, 0), BufCoord(32, 64)).is_ok());

        assert!(disp.region(BufCoord(5, 10), BufCoord(3, 12)).is_err());
        assert!(disp.region(BufCoord(3, 12), BufCoord(5, 10)).is_err());
        assert!(disp.region(BufCoord(31, 12), BufCoord(33, 10)).is_err());
        assert!(disp.region(BufCoord(3, 63), BufCoord(5, 65)).is_err());
    }

    #[test]
    fn region_draw_packed() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), PixelCoord(128, 64));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.region(BufCoord(3, 10), BufCoord(5, 12)).unwrap();
            region.draw_packed(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [3, 4],
            0x75, [10, 11],
            0x5C, [0xDE, 0xAD, 0xBE, 0xEF]
        ));
        di.clear();
        {
            let mut region = disp.region(BufCoord(3, 10), BufCoord(5, 12)).unwrap();
            assert!(region.draw_packed(&[0xDE, 0xAD, 0xBE]).is_err());
            assert!(region.draw_packed(&[0xDE, 0xAD, 0xBE, 0xEF, 0xAA]).is_err());
        }
    }
}
