#![deny(unsafe_code)]
#![no_main]
#![no_std]

// Halt on panic
use panic_halt as _; // panic handler

use defmt_rtt as _;


#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI1])]
mod app {
    use stm32f4xx_hal::{
        pac,
        prelude::*,
        timer::MonoTimerUs,
        watchdog::IndependentWatchdog,
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

        defmt::info!("init done!");

        (
            Shared {},
            Local {
                watchdog,
            },
            init::Monotonics(mono),
        )
    }

    // Feed the watchdog to avoid hardware reset.
    #[task(priority=1, local=[watchdog])]
    fn periodic(cx: periodic::Context) {
        defmt::trace!("feeding the watchdog!");
        cx.local.watchdog.feed();
        periodic::spawn_after(100.millis()).ok();
    }
}
