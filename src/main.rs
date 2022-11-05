#![deny(unsafe_code)]
#![no_main]
#![no_std]

// Halt on panic
use panic_halt as _; // panic handler

use defmt_rtt as _;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI1])]
mod app {
    use embedded_graphics::{
        mono_font::{ascii::FONT_6X12, MonoTextStyleBuilder},
        pixelcolor::BinaryColor,
        prelude::*,
        text::Text,
    };
    use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};
    use stm32f4xx_hal::pac::IWDG;
    use stm32f4xx_hal::rcc::{Clocks, Rcc};
    use stm32f4xx_hal::{
        gpio::{PB8, PB9},
        i2c::{I2c, I2c1},
        pac,
        prelude::*,
        timer::MonoTimerUs,
        watchdog::IndependentWatchdog,
    };

    #[monotonic(binds = TIM2, default = true)]
    type MicrosecMono = MonoTimerUs<pac::TIM2>;

    type I2C1 = I2c1<(PB8, PB9)>;
    type Display =
        Ssd1306<I2CInterface<I2C1>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        watchdog: IndependentWatchdog,
        display: Display,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let rcc = ctx.device.RCC.constrain();
        let clocks = setup_clocks(rcc);
        let mono = ctx.device.TIM2.monotonic_us(&clocks);

        let watchdog = setup_watchdog(ctx.device.IWDG);

        // set up I2C
        let gpiob = ctx.device.GPIOB.split();
        let i2c = I2c::new(ctx.device.I2C1, (gpiob.pb8, gpiob.pb9), 400.kHz(), &clocks);

        // set up the display
        let display = setup_display(i2c);

        defmt::info!("init done!");

        // test draw on display
        show_hello::spawn().ok();

        (
            Shared {},
            Local { watchdog, display },
            init::Monotonics(mono),
        )
    }

    /// Set up the clocks of the microcontroller
    fn setup_clocks(rcc: Rcc) -> Clocks {
        rcc.cfgr.sysclk(84.MHz()).freeze()
    }

    /// Set up the independent watchdog and start the period task to feed it
    fn setup_watchdog(iwdg: IWDG) -> IndependentWatchdog {
        let mut watchdog = IndependentWatchdog::new(iwdg);
        watchdog.start(500u32.millis());
        watchdog.feed();
        periodic::spawn().ok();
        defmt::trace!("watchdog set up");
        watchdog
    }

    /// Setup the SSD1306 display
    fn setup_display(i2c: I2C1) -> Display {
        let interface = I2CDisplayInterface::new_alternate_address(i2c); // our display runs on 0x3D, not 0x3C
        let mut disp = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        disp.init().unwrap();
        disp.flush().unwrap();
        disp
    }

    /// Feed the watchdog to avoid hardware reset.
    #[task(priority=1, local=[watchdog])]
    fn periodic(cx: periodic::Context) {
        defmt::trace!("feeding the watchdog!");
        cx.local.watchdog.feed();
        periodic::spawn_after(100.millis()).ok();
    }

    /// Display Hello Message
    #[task(priority=1, local=[display])]
    fn show_hello(ctx: show_hello::Context) {
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X12)
            .text_color(BinaryColor::On)
            .build();

        Text::new("Hello World!", Point::new(15, 15), text_style)
            .draw(ctx.local.display)
            .unwrap();
        ctx.local.display.flush().unwrap();
    }
}
