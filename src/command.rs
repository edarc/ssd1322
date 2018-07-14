//! The command set for the SSD1322.
//!
//! Note 1: The display RAM of the SSD1322 is arranged in 128 rows and 120 columns, where each
//! column is 4 adjacent pixels (segments) in the row for a total max resolution of 128x480. Each
//! pixel is 4 bits/16 levels of intensity, so each column also refers to two adjacent bytes. Thus,
//! anywhere there is a "column" address, these refer to horizontal groups of 2 bytes driving 4
//! pixels.

use interface::DisplayInterface;

pub const NUM_PIXEL_COLS: u16 = 480;
pub const NUM_PIXEL_ROWS: u8 = 128;
pub const NUM_BUF_COLS: u8 = (NUM_PIXEL_COLS / 4) as u8;
pub const PIXEL_COL_MAX: u16 = NUM_PIXEL_COLS - 1;
pub const PIXEL_ROW_MAX: u8 = NUM_PIXEL_ROWS - 1;
pub const BUF_COL_MAX: u8 = NUM_BUF_COLS - 1;

/// The address increment orientation when writing image data.
#[derive(Clone, Copy)]
pub enum IncrementAxis {
    /// The column address will increment as image data is written, writing pairs of bytes
    /// (horizontal groups of 4 pixels) from left to right in the range set by `SetColumnAddress`
    /// command, and then top to bottom in the range set by `SetRowAddress` command.
    Horizontal,
    /// The row address will increment as image data is written, writing pairs of bytes
    /// (*horizontal* groups of 4 pixels) from top to bottom in the range set by `SetRowAddress`
    /// command, and then left to right in the range set by `SetColumnAddress` command.
    Vertical,
}

/// Setting of column address remapping.
#[derive(Clone, Copy)]
pub enum ColumnRemap {
    /// Column addresses 0->119 map to segments 0,1,2,3->476,477,478,479.
    Forward,
    /// Column addresses 0->119 map to segments 476,477,478,479->0,1,2,3. Note that the pixels
    /// within each column number in the same order; `NibbleRemap` controls the order of mapping
    /// pixels to nibbles within each column.
    Reverse,
}

/// Setting of data nibble remapping.
#[derive(Clone, Copy)]
pub enum NibbleRemap {
    /// 2-byte sequence 0xABCD maps (in L->R order) to pixels 3,2,1,0.
    Reverse,
    /// 2-byte sequence 0xABCD maps (in L->R order) to pixels 0,1,2,3.
    Forward,
}

/// Setting of the COM line scanning of rows. Changing this setting will flip the image vertically.
#[derive(Clone, Copy)]
pub enum ComScanDirection {
    /// COM lines scan row addresses top to bottom, so that row address 0 is the first row of the
    /// display.
    RowZeroFirst,
    /// COM lines scan row addresses bottom to top, so that row address 0 is the last row of the
    /// display.
    RowZeroLast,
}

/// Setting the layout of the COM lines to the display rows. This setting is dictated by how the
/// display module itself wires the OLED matrix to the driver chip, and changing it to anything
/// other than the correct setting for your module will yield a corrupted image. See the display
/// module datasheet for the correct value to use.
#[derive(Clone, Copy)]
pub enum ComLayout {
    /// COM lines are connected to display rows in a progressive arrangement, so that COM lines
    /// 0->127 map to display rows 0->127.
    Progressive,
    /// COM lines are connected to display rows in an interlaced arrangement, so that COM lines
    /// 0->63 map to *even* display rows 0->126, and COM lines 64->127 map to *odd* display rows
    /// 1->127.
    Interlaced,
    /// COM lines are connected to display rows in a dual-COM progressive arrangement, so that COM
    /// lines 0->63 map to display rows 0->63 for half of the columns, and COM lines 64->127 map to
    /// display rows 0->63 for the other half. The maximum displayable image size for this
    /// configuration is halved to 480x64 because each display row uses two COM lines.
    DualProgressive,
}

/// Setting of the display mode.
#[derive(Clone, Copy)]
pub enum DisplayMode {
    /// The display is blanked with all pixels turned OFF (to grayscale level 0).
    BlankDark,
    /// The display is blanked with all pixels turned ON (to grayscale level 15).
    BlankBright,
    /// The display operates normally, showing the image in the display RAM.
    Normal,
    /// The display operates with inverse brightness, showing the image in the display RAM with the
    /// grayscale levels inverted (level 0->15, 1->14, ..., 15->0).
    Inverse,
}

#[derive(Clone, Copy)]
pub enum Command {
    /// Enable the gray scale gamma table (see `BufCommand::SetGrayScaleTable`).
    EnableGrayScaleTable,
    /// Set the column start and end address range when writing to the display RAM. The column
    /// address pointer is reset to the start column address such that `WriteImageData` will begin
    /// writing there. Range is 0-119. (Note 1)
    SetColumnAddress(u8, u8),
    /// Set the row start and end address range when writing to the display RAM. The row address
    /// pointer is reset to the start row address such that `WriteImageData` will begin writing
    /// there. Range is 0-127.
    SetRowAddress(u8, u8),
    /// Set the direction of display address increment, column address remapping, data nibble
    /// remapping, COM scan direction, and COM line layout. See documentation for each enum for
    /// details.
    SetRemapping(
        IncrementAxis,
        ColumnRemap,
        NibbleRemap,
        ComScanDirection,
        ComLayout,
    ),
    /// Set the display start line. Setting this to e.g. 40 will cause the first row of pixels on
    /// the display to display row 40 or the display RAM, and rows 0-39 of the display RAM will be
    /// wrapped to the bottom, "rolling" the displayed image upwards.  This transformation is
    /// applied *before* the MUX ratio setting, meaning if the MUX ratio is set to 90, display rows
    /// 0->89 will always be active, and the "rolled" image will be rendered within those display
    /// rows. Range is 0-127.
    SetStartLine(u8),
    /// Set the display COM line offset. This has a similar effect to `SetStartLine`, rolling the
    /// displayed image upwards as the values increase, except that it is applied *after* the MUX
    /// ratio setting. This means both the image *and* the display rows seleced by the MUX ratio
    /// setting will be rolled upwards. Range is 0-127.
    SetDisplayOffset(u8),
    /// Set the display operating mode. See enum for details.
    SetDisplayMode(DisplayMode),
    /// Enable partial display mode. This selects a range of rows in the display area which will be
    /// active, while all others remain inactive. Range is 0-127, where start must be <= end.
    EnablePartialDisplay(u8, u8),
    /// Disable partial display mode.
    DisablePartialDisplay,
    /// Control sleep mode.
    SetSleepMode(bool),
    /// Set the refresh phase lengths. The first phase (reset) can be set from 5-31 DCLKs, and the
    /// second (first pre-charge) can be set from 3-15 DCLKs. The display module datasheet should
    /// have appropriate values.
    SetPhaseLengths(u8, u8),
    /// Set the oscillator frequency Fosc and the display clock divider. The relationship between
    /// the frequency settings 0-15 and the actual Fosc value is not documented, except that higher
    /// values increase the frequency. The divider DIVSET is a value n from 0-10, where DCLK is
    /// produced by dividing Fosc by 2^n. The resulting DCLK rate indirectly determines the refresh
    /// rate of the display (the exact rate depends on the MUX ratio and some other things).
    SetClockFoscDivset(u8, u8),
    /// Enable or disable display enhancements "external VSL" and "Enhanced low GS display
    /// quality".
    SetDisplayEnhancements(bool, bool),
    /// Set the second pre-charge period. Range 0-15 DCLKs.
    SetSecondPrechargePeriod(u8),
    /// Set the gray scale gamma table to the factory default.
    SetDefaultGrayScaleTable,
    /// Set the pre-charge voltage level, from 0.2*Vcc to 0.6*Vcc. Range 0-31.
    SetPreChargeVoltage(u8),
    /// Set the COM deselect voltage level, from 0.72*Vcc to 0.86*Vcc. Range 0-7.
    SetComDeselectVoltage(u8),
    /// Set the contrast current. Range 0-255.
    SetContrastCurrent(u8),
    /// Set the master contrast control, uniformly reducing all grayscale levels by 0-15
    /// sixteenths. Range 0 (maximum dimming) to 15 (normal contrast).
    SetMasterContrast(u8),
    /// Set the MUX ratio, which controls the number of COM lines that are active and thus the
    /// number of display pixel rows which are active. Which COM lines are active is controlled by
    /// `SetDisplayOffset`, and how the COM lines map to display RAM row addresses is controlled by
    /// `SetStartLine`. Range 16-128.
    SetMuxRatio(u8),
    /// Set whether the command lock is enabled or disabled. Enabling the command lock blocks all
    /// commands except `SetCommandLock`.
    SetCommandLock(bool),
}

pub enum BufCommand<'buf> {
    /// Set the gray scale gamma table. Each byte 0-14 can range from 0-180 and sets the pixel
    /// drive pulse width in DCLKs. Bytes 0->14 adjust the gamma setting for grayscale levels
    /// 1->15; grayscale level 0 cannot be modified. The gamma settings must monotonically
    /// increase.
    SetGrayScaleTable(&'buf [u8]),
    /// Write image data into display RAM. The image data will be written to the display RAM in the
    /// order specified by `SetRemapping` `IncrementAxis` setting. The data, once written, will be
    /// mapped onto the display pixels in a manner determined by `SetRemapping` `ColumnRemap`,
    /// `NibbleRemap`, `ComScanDirection`, and `ComLayout` settings.
    WriteImageData(&'buf [u8]),
}

macro_rules! ok_command {
    ($buf:ident, $cmd:expr,[]) => {
        Ok(($cmd, &$buf[..0]))
    };
    ($buf:ident, $cmd:expr,[$arg0:expr]) => {{
        $buf[0] = $arg0;
        Ok(($cmd, &$buf[..1]))
    }};
    ($buf:ident, $cmd:expr,[$arg0:expr, $arg1:expr]) => {{
        $buf[0] = $arg0;
        $buf[1] = $arg1;
        Ok(($cmd, &$buf[..2]))
    }};
}

impl Command {
    pub fn send<DI>(self, iface: &mut DI) -> Result<(), ()>
    where
        DI: DisplayInterface,
    {
        let mut arg_buf = [0u8; 2];
        let (cmd, data) = match self {
            Command::EnableGrayScaleTable => ok_command!(arg_buf, 0x00, []),
            Command::SetColumnAddress(start, end) => match (start, end) {
                (0...BUF_COL_MAX, 0...BUF_COL_MAX) => ok_command!(arg_buf, 0x15, [start, end]),
                _ => Err(()),
            },
            Command::SetRowAddress(start, end) => match (start, end) {
                (0...PIXEL_ROW_MAX, 0...PIXEL_ROW_MAX) => ok_command!(arg_buf, 0x75, [start, end]),
                _ => Err(()),
            },
            Command::SetRemapping(
                increment_axis,
                column_remap,
                nibble_remap,
                com_scan_direction,
                com_layout,
            ) => {
                let ia = match increment_axis {
                    IncrementAxis::Horizontal => 0x00,
                    IncrementAxis::Vertical => 0x01,
                };
                let cr = match column_remap {
                    ColumnRemap::Forward => 0x00,
                    ColumnRemap::Reverse => 0x02,
                };
                let nr = match nibble_remap {
                    NibbleRemap::Reverse => 0x00,
                    NibbleRemap::Forward => 0x04,
                };
                let csd = match com_scan_direction {
                    ComScanDirection::RowZeroFirst => 0x00,
                    ComScanDirection::RowZeroLast => 0x10,
                };
                let (interlace, dual_com) = match com_layout {
                    ComLayout::Progressive => (0x00, 0x01),
                    ComLayout::Interlaced => (0x20, 0x01),
                    ComLayout::DualProgressive => (0x00, 0x11),
                };
                ok_command!(arg_buf, 0xA0, [ia | cr | nr | csd | interlace, dual_com])
            }
            Command::SetStartLine(line) => match line {
                0...PIXEL_ROW_MAX => ok_command!(arg_buf, 0xA1, [line]),
                _ => Err(()),
            },
            Command::SetDisplayOffset(line) => match line {
                0...PIXEL_ROW_MAX => ok_command!(arg_buf, 0xA2, [line]),
                _ => Err(()),
            },
            Command::SetDisplayMode(mode) => ok_command!(
                arg_buf,
                match mode {
                    DisplayMode::BlankDark => 0xA4,
                    DisplayMode::BlankBright => 0xA5,
                    DisplayMode::Normal => 0xA6,
                    DisplayMode::Inverse => 0xA7,
                },
                []
            ),
            Command::EnablePartialDisplay(start, end) => match (start, end) {
                (0...PIXEL_ROW_MAX, 0...PIXEL_ROW_MAX) if start <= end => {
                    ok_command!(arg_buf, 0xA8, [start, end])
                }
                _ => Err(()),
            },
            Command::DisablePartialDisplay => ok_command!(arg_buf, 0xA9, []),
            Command::SetSleepMode(ena) => ok_command!(
                arg_buf,
                match ena {
                    true => 0xAE,
                    false => 0xAF,
                },
                []
            ),
            Command::SetPhaseLengths(phase_1, phase_2) => match (phase_1, phase_2) {
                (5...31, 3...15) => {
                    let p1 = (phase_1 - 1) >> 1;
                    let p2 = 0xF0 & (phase_2 << 4);
                    ok_command!(arg_buf, 0xB1, [p1 | p2])
                }
                _ => Err(()),
            },
            Command::SetClockFoscDivset(fosc, divset) => match (fosc, divset) {
                (0...15, 0...10) => ok_command!(arg_buf, 0xB3, [fosc << 4 | divset]),
                _ => Err(()),
            },
            Command::SetDisplayEnhancements(ena_external_vsl, ena_enahnced_low_gs_quality) => {
                let vsl = match ena_external_vsl {
                    true => 0xA0,
                    false => 0xA2,
                };
                let gs = match ena_enahnced_low_gs_quality {
                    true => 0xFD,
                    false => 0xB5,
                };
                ok_command!(arg_buf, 0xB4, [vsl, gs])
            }
            Command::SetSecondPrechargePeriod(period) => match period {
                0...15 => ok_command!(arg_buf, 0xB6, [period]),
                _ => Err(()),
            },
            Command::SetDefaultGrayScaleTable => ok_command!(arg_buf, 0xB9, []),
            Command::SetPreChargeVoltage(voltage) => match voltage {
                0...31 => ok_command!(arg_buf, 0xBB, [voltage]),
                _ => Err(()),
            },
            Command::SetComDeselectVoltage(voltage) => match voltage {
                0...7 => ok_command!(arg_buf, 0xBE, [voltage]),
                _ => Err(()),
            },
            Command::SetContrastCurrent(current) => ok_command!(arg_buf, 0xC1, [current]),
            Command::SetMasterContrast(contrast) => match contrast {
                0...15 => ok_command!(arg_buf, 0xC7, [contrast]),
                _ => Err(()),
            },
            Command::SetMuxRatio(ratio) => match ratio {
                16...NUM_PIXEL_ROWS => ok_command!(arg_buf, 0xCA, [ratio - 1]),
                _ => Err(()),
            },
            Command::SetCommandLock(ena) => {
                let e = match ena {
                    true => 0x16,
                    false => 0x12,
                };
                ok_command!(arg_buf, 0xFD, [e])
            }
        }?;
        iface.send_command(cmd)?;
        if data.len() == 0 {
            Ok(())
        } else {
            iface.send_data(data)
        }
    }
}

impl<'a> BufCommand<'a> {
    pub fn send<DI>(self, iface: &mut DI) -> Result<(), ()>
    where
        DI: DisplayInterface,
    {
        let (cmd, data) = match self {
            BufCommand::SetGrayScaleTable(table) => {
                // Each element must be greater than the previous one, and all must be
                // between 0 and 180.
                let ok = table.len() == 15
                    && table[1..]
                        .iter()
                        .fold((true, 0), |(ok_so_far, prev), cur| {
                            (ok_so_far && prev < *cur && *cur <= 180, *cur)
                        })
                        .0 && table[0] <= table[1];
                if ok {
                    Ok((0xB8, table))
                } else {
                    Err(())
                }
            }
            BufCommand::WriteImageData(buf) => Ok((0x5C, buf)),
        }?;
        iface.send_command(cmd)?;
        if data.len() == 0 {
            Ok(())
        } else {
            iface.send_data(data)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use interface::test_spy::TestSpyInterface;
    use std::vec::Vec;

    #[test]
    fn set_column_address() {
        let mut di = TestSpyInterface::new();
        Command::SetColumnAddress(23, 42).send(&mut di).unwrap();
        di.check(0x15, &[23, 42]);
        assert_eq!(Command::SetColumnAddress(120, 42).send(&mut di), Err(()));
        assert_eq!(Command::SetColumnAddress(23, 255).send(&mut di), Err(()));
    }

    #[test]
    fn set_row_address() {
        let mut di = TestSpyInterface::new();
        Command::SetRowAddress(23, 42).send(&mut di).unwrap();
        di.check(0x75, &[23, 42]);
        assert_eq!(Command::SetRowAddress(128, 42).send(&mut di), Err(()));
        assert_eq!(Command::SetRowAddress(23, 255).send(&mut di), Err(()));
    }

    #[test]
    fn set_remapping() {
        let mut di = TestSpyInterface::new();
        Command::SetRemapping(
            IncrementAxis::Horizontal,
            ColumnRemap::Forward,
            NibbleRemap::Reverse,
            ComScanDirection::RowZeroFirst,
            ComLayout::Progressive,
        ).send(&mut di)
            .unwrap();
        di.check(0xA0, &[0x00, 0x01]);

        di.clear();
        Command::SetRemapping(
            IncrementAxis::Vertical,
            ColumnRemap::Reverse,
            NibbleRemap::Forward,
            ComScanDirection::RowZeroLast,
            ComLayout::Interlaced,
        ).send(&mut di)
            .unwrap();
        di.check(0xA0, &[0x37, 0x01]);

        di.clear();
        Command::SetRemapping(
            IncrementAxis::Horizontal,
            ColumnRemap::Forward,
            NibbleRemap::Forward,
            ComScanDirection::RowZeroLast,
            ComLayout::DualProgressive,
        ).send(&mut di)
            .unwrap();
        di.check(0xA0, &[0x14, 0x11]);
    }

    #[test]
    fn write_image_data() {
        let mut di = TestSpyInterface::new();
        let image_buf = (0..24).collect::<Vec<u8>>();
        BufCommand::WriteImageData(&image_buf[..])
            .send(&mut di)
            .unwrap();
        di.check(0x5C, &(0..24u8).collect::<Vec<_>>()[..]);
    }

    #[test]
    fn set_start_line() {
        let mut di = TestSpyInterface::new();
        Command::SetStartLine(23).send(&mut di).unwrap();
        di.check(0xA1, &[23]);
        assert_eq!(Command::SetStartLine(128).send(&mut di), Err(()));
    }

    #[test]
    fn set_display_offset() {
        let mut di = TestSpyInterface::new();
        Command::SetDisplayOffset(23).send(&mut di).unwrap();
        di.check(0xA2, &[23]);
        assert_eq!(Command::SetDisplayOffset(128).send(&mut di), Err(()));
    }

    #[test]
    fn set_display_mode() {
        let mut di = TestSpyInterface::new();
        Command::SetDisplayMode(DisplayMode::BlankDark)
            .send(&mut di)
            .unwrap();
        di.check(0xA4, &[]);
        di.clear();
        Command::SetDisplayMode(DisplayMode::BlankBright)
            .send(&mut di)
            .unwrap();
        di.check(0xA5, &[]);
        di.clear();
        Command::SetDisplayMode(DisplayMode::Normal)
            .send(&mut di)
            .unwrap();
        di.check(0xA6, &[]);
        di.clear();
        Command::SetDisplayMode(DisplayMode::Inverse)
            .send(&mut di)
            .unwrap();
        di.check(0xA7, &[]);
    }

    #[test]
    fn enable_partial_display() {
        let mut di = TestSpyInterface::new();
        Command::EnablePartialDisplay(23, 42).send(&mut di).unwrap();
        di.check(0xA8, &[23, 42]);
        assert_eq!(
            Command::EnablePartialDisplay(23, 128).send(&mut di),
            Err(())
        );
        assert_eq!(
            Command::EnablePartialDisplay(128, 129).send(&mut di),
            Err(())
        );
        assert_eq!(Command::EnablePartialDisplay(42, 23).send(&mut di), Err(()));
    }

    #[test]
    fn sleep_mode() {
        let mut di = TestSpyInterface::new();
        Command::SetSleepMode(true).send(&mut di).unwrap();
        di.check(0xAE, &[]);
        di.clear();
        Command::SetSleepMode(false).send(&mut di).unwrap();
        di.check(0xAF, &[]);
    }

    #[test]
    fn set_phase_lengths() {
        let mut di = TestSpyInterface::new();
        Command::SetPhaseLengths(5, 3).send(&mut di).unwrap();
        di.check(0xB1, &[0x32]);
        di.clear();
        Command::SetPhaseLengths(5, 14).send(&mut di).unwrap();
        di.check(0xB1, &[0xE2]);
        di.clear();
        Command::SetPhaseLengths(7, 3).send(&mut di).unwrap();
        di.check(0xB1, &[0x33]);
        di.clear();
        Command::SetPhaseLengths(31, 15).send(&mut di).unwrap();
        di.check(0xB1, &[0xFF]);
        assert_eq!(Command::SetPhaseLengths(4, 3).send(&mut di), Err(()));
        assert_eq!(Command::SetPhaseLengths(32, 3).send(&mut di), Err(()));
        assert_eq!(Command::SetPhaseLengths(5, 2).send(&mut di), Err(()));
        assert_eq!(Command::SetPhaseLengths(5, 16).send(&mut di), Err(()));
    }

    #[test]
    fn set_clock_fosc_divset() {
        let mut di = TestSpyInterface::new();
        Command::SetClockFoscDivset(0, 0).send(&mut di).unwrap();
        di.check(0xB3, &[0x00]);
        di.clear();
        Command::SetClockFoscDivset(15, 10).send(&mut di).unwrap();
        di.check(0xB3, &[0xFA]);
        assert_eq!(Command::SetClockFoscDivset(0, 11).send(&mut di), Err(()));
        assert_eq!(Command::SetClockFoscDivset(16, 0).send(&mut di), Err(()));
    }

    #[test]
    fn set_display_enhancements() {
        let mut di = TestSpyInterface::new();
        Command::SetDisplayEnhancements(false, false)
            .send(&mut di)
            .unwrap();
        di.check(0xB4, &[0b10100010, 0b10110101]);
        di.clear();
        Command::SetDisplayEnhancements(true, false)
            .send(&mut di)
            .unwrap();
        di.check(0xB4, &[0b10100000, 0b10110101]);
        di.clear();
        Command::SetDisplayEnhancements(true, true)
            .send(&mut di)
            .unwrap();
        di.check(0xB4, &[0b10100000, 0b11111101]);
    }

    #[test]
    fn set_second_precharge_period() {
        let mut di = TestSpyInterface::new();
        Command::SetSecondPrechargePeriod(0).send(&mut di).unwrap();
        di.check(0xB6, &[0]);
        di.clear();
        Command::SetSecondPrechargePeriod(15).send(&mut di).unwrap();
        di.check(0xB6, &[15]);
        di.clear();
        assert_eq!(Command::SetSecondPrechargePeriod(16).send(&mut di), Err(()));
    }

    #[test]
    fn set_gray_scale_table() {
        let mut di = TestSpyInterface::new();
        BufCommand::SetGrayScaleTable(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14])
            .send(&mut di)
            .unwrap();
        di.check(0xB8, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
        di.clear();
        BufCommand::SetGrayScaleTable(&[
            166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180,
        ]).send(&mut di)
            .unwrap();
        di.check(
            0xB8,
            &[
                166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180,
            ],
        );
        di.clear();
        // Out of range
        assert_eq!(
            BufCommand::SetGrayScaleTable(&[
                166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 181,
            ]).send(&mut di),
            Err(())
        );
        // Non-increasing
        assert_eq!(
            BufCommand::SetGrayScaleTable(&[0, 1, 2, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14])
                .send(&mut di),
            Err(())
        );
        // Too many values
        assert_eq!(
            BufCommand::SetGrayScaleTable(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
                .send(&mut di),
            Err(())
        );
        // Too few values
        assert_eq!(
            BufCommand::SetGrayScaleTable(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13])
                .send(&mut di),
            Err(())
        );
    }

    #[test]
    fn set_pre_charge_voltage() {
        let mut di = TestSpyInterface::new();
        Command::SetPreChargeVoltage(17).send(&mut di).unwrap();
        di.check(0xBB, &[17]);
        assert_eq!(Command::SetPreChargeVoltage(32).send(&mut di), Err(()));
    }

    #[test]
    fn set_com_deselect_voltage() {
        let mut di = TestSpyInterface::new();
        Command::SetComDeselectVoltage(3).send(&mut di).unwrap();
        di.check(0xBE, &[3]);
        assert_eq!(Command::SetComDeselectVoltage(8).send(&mut di), Err(()));
    }

    #[test]
    fn set_master_contrasat() {
        let mut di = TestSpyInterface::new();
        Command::SetMasterContrast(3).send(&mut di).unwrap();
        di.check(0xC7, &[3]);
        assert_eq!(Command::SetMasterContrast(16).send(&mut di), Err(()));
    }

    #[test]
    fn set_mux_ratio() {
        let mut di = TestSpyInterface::new();
        Command::SetMuxRatio(128).send(&mut di).unwrap();
        di.check(0xCA, &[127]);
        di.clear();
        Command::SetMuxRatio(16).send(&mut di).unwrap();
        di.check(0xCA, &[15]);
        assert_eq!(Command::SetMuxRatio(15).send(&mut di), Err(()));
        assert_eq!(Command::SetMuxRatio(129).send(&mut di), Err(()));
    }

    #[test]
    fn set_command_lock() {
        let mut di = TestSpyInterface::new();
        Command::SetCommandLock(true).send(&mut di).unwrap();
        di.check(0xFD, &[0b00010110]);
        di.clear();
        Command::SetCommandLock(false).send(&mut di).unwrap();
        di.check(0xFD, &[0b00010010]);
    }
}
