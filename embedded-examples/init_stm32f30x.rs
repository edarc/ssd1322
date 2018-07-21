//! Full example code for setting up an SSD1322 display. This runs on an STM32F303RE, using a
//! Newhaven Displays NHD-3.12-25664UCY2 connected to SPI1, PA8 for C/S, and PA9 for /RESET.

#![deny(unsafe_code)]
#![no_main]
#![no_std]

extern crate cortex_m;
extern crate embedded_hal as hal_api;
extern crate stm32f30x;
extern crate stm32f30x_hal as hal;
#[macro_use]
extern crate cortex_m_rt;
extern crate panic_abort;
extern crate ssd1322;

use core::iter;
use cortex_m::asm;
use cortex_m_rt::ExceptionFrame;
use hal::prelude::*;
use hal::spi;
use ssd1322 as oled;

entry!(main);

exception!(*, default_handler);
exception!(HardFault, hard_fault);

fn hard_fault(_ef: &ExceptionFrame) -> ! {
    asm::bkpt();
    loop {}
}

fn default_handler(_irqn: i16) {
    loop {}
}

fn main() -> ! {
    // Get peripherals and set up RCC.
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32f30x::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut delay = hal::delay::Delay::new(cp.SYST, clocks);

    // Get GPIO A where the display is connected.
    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);

    // Set up SPI1, which is Alternate Function 5 for GPIOs PA5,6,7.
    let disp_sck = gpioa.pa5.into_af5(&mut gpioa.moder, &mut gpioa.afrl);
    let disp_miso = gpioa.pa6.into_af5(&mut gpioa.moder, &mut gpioa.afrl);
    let disp_mosi = gpioa.pa7.into_af5(&mut gpioa.moder, &mut gpioa.afrl);

    let disp_spi = spi::Spi::spi1(
        dp.SPI1,
        (disp_sck, disp_miso, disp_mosi),
        hal_api::spi::Mode {
            polarity: hal_api::spi::Polarity::IdleLow,
            phase: hal_api::spi::Phase::CaptureOnFirstTransition,
        },
        8.mhz(),
        clocks,
        &mut rcc.apb2,
    );

    // PA8 will be the D/C push-pull output for the 4th wire.
    let disp_dc = gpioa
        .pa8
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

    // PA9 is the display's /RESET pin. The ssd1322 library does not control this pin; we will
    // assert reset separately.
    let mut disp_rst = gpioa
        .pa9
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

    // Create the SpiInterface and Display.
    let mut disp = oled::Display::new(
        oled::SpiInterface::new(disp_spi, disp_dc),
        oled::PixelCoord(256, 64),
        oled::PixelCoord(112, 0),
    );

    // Assert the display's /RESET for 10ms.
    disp_rst.set_low();
    delay.delay_ms(10_u16);
    disp_rst.set_high();

    // Initialize the display. These parameters are taken from the Newhaven datasheet for the
    // NHD-3.12-25664UCY2.
    disp.init(
        oled::Config::new(
            oled::ComScanDirection::RowZeroLast,
            oled::ComLayout::DualProgressive,
        ).clock_fosc_divset(9, 1)
            .display_enhancements(true, true)
            .contrast_current(159)
            .phase_lengths(5, 14)
            .precharge_voltage(31)
            .second_precharge_period(8)
            .com_deselect_voltage(7),
    ).unwrap();

    // Get a region covering the entire display area, and clear it by writing all zeros.
    {
        let mut region = disp
            .region(oled::PixelCoord(0, 0), oled::PixelCoord(256, 128))
            .unwrap();
        region.draw_packed(iter::repeat(0)).unwrap();
    }

    loop {
        asm::wfi();
    }
}
