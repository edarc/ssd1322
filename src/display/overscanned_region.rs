//! Extended region abstraction that allows requesting regions that "overscan" the display, i.e.
//! portions of the region may lie outside the displayable area. Image data written into
//! overscanned regions is silently discarded, to relieve the user from having to consider boundary
//! conditions in code where the region rectangle is dynamically computed.

use itertools::iproduct;

use crate::command::consts::*;
use crate::display::region::{Pack8to4, Region};
use crate::display::PixelCoord;
use crate::interface;

/// A handle to a rectangular region which can be drawn into, but which is permitted to have
/// portions that lie outside the viewable area of the display. Pixels that fall outside the
/// viewable area are automatically dropped. This allows the user to avoid manually handling
/// boundary conditions if they simply want things drawn outside the viewable area to be cropped
/// off automatically.
///
/// The functionality is separated into its own kind of region so that the cost of the cropping
/// logic is not paid when it is known to be unnecessary.
///
/// These are intended to be short-lived, and contain a mutable borrow of the display that issued
/// them so clashing writes are prevented.
pub struct OverscannedRegion<'di, DI>
where
    DI: 'di + interface::DisplayInterface,
{
    viewable_region: Option<Region<'di, DI>>,
    upper_left: PixelCoord,
    lower_right: PixelCoord,
    viewable_pixel_cols: i16,
}

/// Clip a value between some low and high limit.
fn clip<T: PartialOrd>(lo: T, x: T, hi: T) -> T {
    match () {
        _ if x > hi => hi,
        _ if x < lo => lo,
        _ => x,
    }
}

fn in_range<T: PartialOrd>(x: T, lo: T, hi: T) -> bool {
    x >= lo && x < hi
}

impl<'di, DI> OverscannedRegion<'di, DI>
where
    DI: 'di + interface::DisplayInterface,
{
    /// Construct a new region. This is only called by the factory method
    /// `Display::overscanned_region`, which checks the region coordinates are correctly ordered,
    /// and pre-compensates the column coordinates for the display column offset.
    pub(super) fn new(
        iface: &'di mut DI,
        upper_left: PixelCoord,
        lower_right: PixelCoord,
        viewable_pixel_cols: i16,
        pixel_col_offset: i16,
    ) -> Self {
        let viewable_ul = PixelCoord(
            clip(0, upper_left.0, viewable_pixel_cols),
            clip(0, upper_left.1, NUM_PIXEL_ROWS as i16),
        );
        let viewable_lr = PixelCoord(
            clip(0, lower_right.0, viewable_pixel_cols),
            clip(0, lower_right.1, NUM_PIXEL_ROWS as i16),
        );
        let viewable_region = if viewable_ul.0 == viewable_lr.0 || viewable_ul.1 == viewable_lr.1 {
            None
        } else {
            Some(Region::new(
                iface,
                PixelCoord(viewable_ul.0 + pixel_col_offset, viewable_ul.1),
                PixelCoord(viewable_lr.0 + pixel_col_offset, viewable_lr.1),
            ))
        };
        Self {
            viewable_region: viewable_region,
            upper_left: upper_left,
            lower_right: lower_right,
            viewable_pixel_cols: viewable_pixel_cols,
        }
    }

    /// Draw packed-pixel image data into the region, such that each byte is two 4-bit gray scale
    /// values of horizontally-adjacent pixels. Pixels are drawn left-to-right and top-to-bottom.
    /// The sequence of pixels is filtered such that only pixels which intersect the displayable
    /// area are transmitted to the hardware.
    pub fn draw_packed<I>(&mut self, iter: I) -> Result<(), DI::Error>
    where
        I: Iterator<Item = u8>,
    {
        if self.viewable_region.is_none() {
            return Ok(());
        }
        let input_coords = iproduct!(
            self.upper_left.1..self.lower_right.1,
            (self.upper_left.0..self.lower_right.0).step_by(2)
        );
        let input_with_coords = input_coords.zip(iter);
        let viewable_pixel_cols = self.viewable_pixel_cols;
        let only_viewable = input_with_coords
            .filter(|((r, c), _)| {
                in_range(*r, 0, NUM_PIXEL_ROWS as i16) && in_range(*c, 0, viewable_pixel_cols)
            })
            .map(|(_, pixels)| pixels);
        self.viewable_region
            .as_mut()
            .unwrap()
            .draw_packed(only_viewable)
    }

    /// Draw unpacked pixel image data into the region, where each byte independently represents a
    /// single pixel intensity value in the range [0, 15]. Pixels are drawn left-to-right and
    /// top-to-bottom. The sequence of pixels is filtered such that only pixels which intersect the
    /// displayable area are transmitted to the hardware.
    pub fn draw<I>(&mut self, iter: I) -> Result<(), DI::Error>
    where
        I: Iterator<Item = u8>,
    {
        self.draw_packed(Pack8to4(iter))
    }
}

#[cfg(test)]
mod tests {
    use crate::command::{ComLayout, ComScanDirection};
    use crate::config::Config;
    use crate::display::{Display, PixelCoord as Px};
    use crate::interface::test_spy::{Sent, TestSpyInterface};

    #[test]
    fn draw_packed_interior() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(12, 10), Px(16, 12)).unwrap();
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
    fn draw_packed_complete_crop() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(-16, -5), Px(-12, -3)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
        ));
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(16, 132), Px(20, 134)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
        ));
    }

    #[test]
    fn draw_packed_crop_row_edge() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(16, -1), Px(20, 1)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [4, 4],
            0x75, [0, 0],
            0x5C, [0xBE, 0xEF]
        ));
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(16, 127), Px(20, 129)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [4, 4],
            0x75, [127, 127],
            0x5C, [0xDE, 0xAD]
        ));
    }

    #[test]
    fn draw_packed_crop_col_edge() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(-4, 10), Px(4, 12)).unwrap();
            region
                .draw_packed(
                    [0xDE, 0xAD, 0xBE, 0xEF, 0xB0, 0x1D, 0xFA, 0xCE]
                        .iter()
                        .cloned(),
                )
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [0, 0],
            0x75, [10, 11],
            0x5C, [0xBE, 0xEF, 0xFA, 0xCE]
        ));
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(124, 10), Px(132, 12)).unwrap();
            region
                .draw_packed(
                    [0xDE, 0xAD, 0xBE, 0xEF, 0xB0, 0x1D, 0xFA, 0xCE]
                        .iter()
                        .cloned(),
                )
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [31, 31],
            0x75, [10, 11],
            0x5C, [0xDE, 0xAD, 0xB0, 0x1D]
        ));
    }

    #[test]
    fn draw_packed_crop_corner() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(0, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(-4, -1), Px(4, 1)).unwrap();
            region
                .draw_packed(
                    [0xDE, 0xAD, 0xBE, 0xEF, 0xB0, 0x1D, 0xFA, 0xCE]
                        .iter()
                        .cloned(),
                )
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [0, 0],
            0x75, [0, 0],
            0x5C, [0xFA, 0xCE]
        ));
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(124, 127), Px(132, 129)).unwrap();
            region
                .draw_packed(
                    [0xDE, 0xAD, 0xBE, 0xEF, 0xB0, 0x1D, 0xFA, 0xCE]
                        .iter()
                        .cloned(),
                )
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [31, 31],
            0x75, [127, 127],
            0x5C, [0xDE, 0xAD]
        ));
    }

    #[test]
    fn draw_packed_display_column_offset_interior() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(64, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(0, 10), Px(8, 12)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [16, 17],
            0x75, [10, 11],
            0x5C, [0xDE, 0xAD, 0xBE, 0xEF]
        ));
    }

    #[test]
    fn draw_packed_display_column_offset_crop_col() {
        let mut di = TestSpyInterface::new();
        let mut disp = Display::new(di.split(), Px(128, 64), Px(24, 0));
        let cfg = Config::new(ComScanDirection::RowZeroLast, ComLayout::DualProgressive);
        disp.init(cfg).unwrap();
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(-4, 10), Px(4, 11)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [6, 6],
            0x75, [10, 10],
            0x5C, [0xBE, 0xEF]
        ));
        di.clear();
        {
            let mut region = disp.overscanned_region(Px(124, 10), Px(132, 11)).unwrap();
            region
                .draw_packed([0xDE, 0xAD, 0xBE, 0xEF].iter().cloned())
                .unwrap();
        }
        #[cfg_attr(rustfmt, rustfmt_skip)]
        di.check_multi(sends!(
            0x15, [37, 37],
            0x75, [10, 10],
            0x5C, [0xDE, 0xAD]
        ));
    }
}
