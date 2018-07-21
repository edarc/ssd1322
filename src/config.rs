//! Defines structs for storing register values of commands in the SSD1322 that are associated with
//! relatively-static configuration.

use command::*;
use interface;

/// The portion of the configuration which will persist inside the `Display` because it shares
/// registers with functions that can be changed after initialization. This allows the rest of the
/// `Config` struct to be thrown away to save RAM after `Display::init` finishes.
pub(crate) struct PersistentConfig {
    com_scan_direction: ComScanDirection,
    com_layout: ComLayout,
}

impl PersistentConfig {
    /// Transmit commands to the display at `iface` necessary to put that display into the
    /// configuration encoded in `self`.
    pub(crate) fn send<DI>(
        &self,
        iface: &mut DI,
        increment_axis: IncrementAxis,
        column_remap: ColumnRemap,
        nibble_remap: NibbleRemap,
    ) -> Result<(), ()>
    where
        DI: interface::DisplayInterface,
    {
        Command::SetRemapping(
            increment_axis,
            column_remap,
            nibble_remap,
            self.com_scan_direction,
            self.com_layout,
        ).send(iface)
    }
}

/// A configuration for the display. Builder methods offer a declarative way to either sent a
/// configuration command at init time, or to leave it at the chip's POR default.
pub struct Config {
    pub(crate) persistent_config: PersistentConfig,
    contrast_current_cmd: Option<Command>,
    phase_lengths_cmd: Option<Command>,
    clock_fosc_divset_cmd: Option<Command>,
    display_enhancements_cmd: Option<Command>,
    second_precharge_period_cmd: Option<Command>,
    precharge_voltage_cmd: Option<Command>,
    com_deselect_voltage_cmd: Option<Command>,
}

impl Config {
    /// Create a new configuration. COM scan direction and COM layout are mandatory because the
    /// display will not function correctly unless they are set, so they must be provided in the
    /// constructor. All other options can be optionally set by calling the provided builder
    /// methods on `Config`.
    pub fn new(com_scan_direction: ComScanDirection, com_layout: ComLayout) -> Self {
        Config {
            persistent_config: PersistentConfig {
                com_scan_direction: com_scan_direction,
                com_layout: com_layout,
            },
            contrast_current_cmd: None,
            phase_lengths_cmd: None,
            clock_fosc_divset_cmd: None,
            display_enhancements_cmd: None,
            second_precharge_period_cmd: None,
            precharge_voltage_cmd: None,
            com_deselect_voltage_cmd: None,
        }
    }

    /// Extend this `Config` to explicitly configure display contrast current. See
    /// `Command::SetContrastCurrent`.
    pub fn contrast_current(self, current: u8) -> Self {
        Self {
            contrast_current_cmd: Some(Command::SetContrastCurrent(current)),
            ..self
        }
    }

    /// Extend this `Config` to explicitly configure OLED drive phase lengths. See
    /// `Command::SetPhaseLengths`.
    pub fn phase_lengths(self, reset: u8, first_precharge: u8) -> Self {
        Self {
            phase_lengths_cmd: Some(Command::SetPhaseLengths(reset, first_precharge)),
            ..self
        }
    }

    /// Extend this `Config` to explicitly configure the display clock frequency and divider. See
    /// `Command::SetClockFoscDivset`.
    pub fn clock_fosc_divset(self, fosc: u8, divset: u8) -> Self {
        Self {
            clock_fosc_divset_cmd: Some(Command::SetClockFoscDivset(fosc, divset)),
            ..self
        }
    }

    /// Extend this `Config` to explicitly configure display enhancement features. See
    /// `Command::SetDisplayEnhancements`.
    pub fn display_enhancements(self, external_vsl: bool, enhanced_low_gs_quality: bool) -> Self {
        Self {
            display_enhancements_cmd: Some(Command::SetDisplayEnhancements(
                external_vsl,
                enhanced_low_gs_quality,
            )),
            ..self
        }
    }

    /// Extend this `Config` to explicitly configure OLED drive second precharge period length. See
    /// `Command::SetSecondPrechargePeriod`.
    pub fn second_precharge_period(self, period: u8) -> Self {
        Self {
            second_precharge_period_cmd: Some(Command::SetSecondPrechargePeriod(period)),
            ..self
        }
    }

    /// Extend this `Config` to explicitly configure OLED drive precharge voltage. See
    /// `Command::SetPreChargeVoltage`.
    pub fn precharge_voltage(self, voltage: u8) -> Self {
        Self {
            precharge_voltage_cmd: Some(Command::SetPreChargeVoltage(voltage)),
            ..self
        }
    }

    /// Extend this `Config` to explicitly configure OLED drive COM deselect voltage. See
    /// `Command::SetComDeselectVoltage`.
    pub fn com_deselect_voltage(self, voltage: u8) -> Self {
        Self {
            com_deselect_voltage_cmd: Some(Command::SetComDeselectVoltage(voltage)),
            ..self
        }
    }

    /// Transmit commands to the display at `iface` necessary to put that display into the
    /// configuration encoded in `self`.
    pub(crate) fn send<DI>(&self, iface: &mut DI) -> Result<(), ()>
    where
        DI: interface::DisplayInterface,
    {
        self.phase_lengths_cmd.map_or(Ok(()), |c| c.send(iface))?;
        self.contrast_current_cmd.map_or(Ok(()), |c| c.send(iface))?;
        self.clock_fosc_divset_cmd
            .map_or(Ok(()), |c| c.send(iface))?;
        self.display_enhancements_cmd
            .map_or(Ok(()), |c| c.send(iface))?;
        self.second_precharge_period_cmd
            .map_or(Ok(()), |c| c.send(iface))?;
        self.precharge_voltage_cmd
            .map_or(Ok(()), |c| c.send(iface))?;
        self.com_deselect_voltage_cmd
            .map_or(Ok(()), |c| c.send(iface))
    }
}
