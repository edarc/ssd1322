//! The main API to the display driver. It provides a builder API to configure the display, and
//! methods for obtaining `Region` instances which can be used to write image data to the display.

// This has to be here in order to be usable by mods declared afterwards.
#[cfg(test)]
#[macro_use]
pub mod testing {
    macro_rules! send {
        ([$($d:tt),*]) => {Sent::Data(vec![$($d,)*])};
        ($c:tt) => {Sent::Cmd($c)};
    }
    macro_rules! sends {
        ($($e:tt),*) => {&[$(send!($e),)*]};
    }
}

pub mod overscanned_region;
pub mod region;

use command::consts::*;
use command::*;
use config::{Config, PersistentConfig};
use display::overscanned_region::OverscannedRegion;
use display::region::Region;
use interface;

/// A pixel coordinate pair of `column` and `row`. `column` must be in the range [0,
/// `consts::PIXEL_COL_MAX`], and `row` must be in the range [0, `consts::PIXEL_ROW_MAX`].
#[derive(Clone, Copy, Debug)]
pub struct PixelCoord(pub i16, pub i16);

/// A driver for an SSD1322 display.
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
    /// Construct a new display driver for a display with viewable dimensions `display_size`, which
    /// is connected to the interface `iface`.
    ///
    /// Some display modules with resolution lower than the maximum supported by the chip will
    /// connect column driver or COM lines starting in the middle rather than from 0 for mechanical
    /// PCB layout reasons.
    ///
    /// For such modules, `display_offset` allows automatically removing these offsets when drawing
    /// image data to the display. It describes the number of pixels of offset the display column
    /// numbering has relative to the driver and COM line numbering: `display_offset.0` indicates
    /// the driver line column which corresponds to pixel column 0 of the display, and
    /// `display_offset.1` indicates which COM line corresponds to pixel row 0 of the display.
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
    ///
    /// This uses the `Command::SetStartLine` feature to shift the display RAM row addresses
    /// relative to the active set of COM lines, allowing any display-height-sized window of the
    /// entire 128 rows of display RAM to be made visible.
    pub fn vertical_pan(&mut self, offset: u8) -> Result<(), ()> {
        Command::SetStartLine(offset).send(&mut self.iface)
    }

    /// Construct a rectangular region onto which to draw image data.
    ///
    /// The region start and end horizontal coordinates must be divisible by 4, because pixels can
    /// only be addressed by column address (groups of 4), not individually. The region rectangle
    /// must also be within the viewable area of the display buffer, where the viewable area
    /// includes all 128 rows to support vertical panning.
    ///
    /// Regions are intended to be short-lived, and mutably borrow the display so clashing writes
    /// are prevented.
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
        // is probably an error because it can never be read back and can never be visible on the
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

    /// Construct a rectangular region onto which to draw image data which silently discards
    /// overscan.
    ///
    /// The region start and end horizontal coordinates must be divisible by 4, because pixels can
    /// only be addressed by column (groups of 4), not individually. An overscanned region
    /// rectangle *need not* lie within the viewable area of the display buffer, as it will
    /// automatically crop non-viewable pixels to alleviate its user from worrying about boundary
    /// conditions.
    ///
    /// Regions are intended to be short-lived, and mutably borrow the display so clashing writes
    /// are prevented.
    pub fn overscanned_region<'di>(
        &'di mut self,
        upper_left: PixelCoord,
        lower_right: PixelCoord,
    ) -> Result<OverscannedRegion<'di, DI>, ()> {
        if false
            || upper_left.0 >= lower_right.0
            || upper_left.1 >= lower_right.1
            || upper_left.0.mod_euc(4) != 0
            || lower_right.0.mod_euc(4) != 0
        {
            return Err(());
        }

        Ok(OverscannedRegion::new(
            &mut self.iface,
            upper_left,
            lower_right,
            self.display_size.0,
            self.display_offset.0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{PixelCoord as Px, *};
    use interface::test_spy::{Sent, TestSpyInterface};

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
    fn overscanned_region_build() {
        let di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();

        // Correctly ordered, and columns in 4s.
        assert!(disp.overscanned_region(Px(12, 10), Px(20, 12)).is_ok());
        assert!(disp.overscanned_region(Px(0, 0), Px(128, 64)).is_ok());

        // Columns not in 4s.
        assert!(disp.overscanned_region(Px(12, 10), Px(21, 12)).is_err());
        assert!(disp.overscanned_region(Px(13, 10), Px(20, 12)).is_err());

        // Incorrectly ordered.
        assert!(disp.overscanned_region(Px(20, 10), Px(12, 12)).is_err());
        assert!(disp.overscanned_region(Px(12, 12), Px(20, 10)).is_err());

        // Partially out of range.
        assert!(disp.overscanned_region(Px(-8, 4), Px(12, 6)).is_ok());
        assert!(disp.overscanned_region(Px(4, -5), Px(20, 20)).is_ok());
        assert!(disp.overscanned_region(Px(124, 4), Px(132, 6)).is_ok());
        assert!(disp.overscanned_region(Px(4, 60), Px(20, 130)).is_ok());

        // Entirely out of range.
        assert!(disp.overscanned_region(Px(-16, 130), Px(-4, 160)).is_ok());
        assert!(disp.overscanned_region(Px(128, -16), Px(132, -4)).is_ok());
    }
}
