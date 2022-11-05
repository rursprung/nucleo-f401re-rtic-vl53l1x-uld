#![deny(unsafe_code)]
#![no_main]
#![no_std]

// Halt on panic
use panic_halt as _; // panic handler

use defmt_rtt as _;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI1])]
mod app {
    use embedded_graphics::{
        mono_font::{
            ascii::{FONT_6X12},
            MonoTextStyleBuilder,
        },
        pixelcolor::BinaryColor,
        prelude::*,
        text::Text,
    };
    use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
    use stm32f4xx_hal::rcc::{Clocks, Rcc};
    use stm32f4xx_hal::{
        i2c::I2c, pac, prelude::*, timer::MonoTimerUs, watchdog::IndependentWatchdog,
    };

    #[monotonic(binds = TIM2, default = true)]
    type MicrosecMono = MonoTimerUs<pac::TIM2>;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        watchdog: IndependentWatchdog,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let mut syscfg = ctx.device.SYSCFG.constrain();

        // Set up the system clock. We want to run at 48MHz for this one.
        let rcc = ctx.device.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();

        let mono = ctx.device.TIM2.monotonic_us(&clocks);

        // set up the watchdog
        let mut watchdog = IndependentWatchdog::new(ctx.device.IWDG);
        watchdog.start(500u32.millis());
        watchdog.feed();
        periodic::spawn().ok();

        // set up I2C
        let gpiob = ctx.device.GPIOB.split();
        let i2c = I2c::new(
            ctx.device.I2C1,
            (
                gpiob.pb8.into_alternate().set_open_drain(),
                gpiob.pb9.into_alternate().set_open_drain(),
            ),
            400.kHz(),
            &clocks,
        );

        // set up the display
        let interface = I2CDisplayInterface::new_alternate_address(i2c); // our display runs on 0x3D, not 0x3C
        let mut disp = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        disp.init().unwrap();
        disp.flush().unwrap();

        defmt::info!("init done!");

        // test draw on display
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X12)
            .text_color(BinaryColor::On)
            .build();

        Text::new("Hello World!", Point::new(15, 15), text_style)
            .draw(&mut disp)
            .unwrap();
        disp.flush().unwrap();

        (Shared {}, Local { watchdog }, init::Monotonics(mono))
    }

    // Feed the watchdog to avoid hardware reset.
    #[task(priority=1, local=[watchdog])]
    fn periodic(cx: periodic::Context) {
        defmt::trace!("feeding the watchdog!");
        cx.local.watchdog.feed();
        periodic::spawn_after(100.millis()).ok();
    }
}
