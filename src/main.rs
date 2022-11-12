#![deny(unsafe_code)]
#![no_main]
#![no_std]

// Halt on panic
use panic_halt as _; // panic handler

use defmt_rtt as _;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI1, EXTI2])]
mod app {
    use stm32f4xx_hal::pac::IWDG;
    use stm32f4xx_hal::rcc::{Clocks, Rcc};
    use stm32f4xx_hal::{
        gpio::{PB8, PB9},
        i2c::{I2c, I2c1},
        pac,
        prelude::*,
        timer::{MonoTimerUs, SysDelay},
        watchdog::IndependentWatchdog,
    };
    use vl53l1;

    #[monotonic(binds = TIM2, default = true)]
    type MicrosecMono = MonoTimerUs<pac::TIM2>;

    type I2C1 = I2c1<(PB8, PB9)>;

    pub struct TOFSensor {
        device: vl53l1::Device,
        i2c: I2C1,
    }

    #[shared]
    struct Shared {
        delay: SysDelay,
    }

    #[local]
    struct Local {
        watchdog: IndependentWatchdog,
        tof_sensor: TOFSensor,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let rcc = ctx.device.RCC.constrain();
        let clocks = setup_clocks(rcc);
        let mono = ctx.device.TIM2.monotonic_us(&clocks);

        let mut delay = ctx.core.SYST.delay(&clocks);

        let watchdog = setup_watchdog(ctx.device.IWDG);

        // set up I2C
        let gpiob = ctx.device.GPIOB.split();
        let i2c = I2c::new(ctx.device.I2C1, (gpiob.pb8, gpiob.pb9), 400.kHz(), &clocks);

        // set up the TOF sensor
        let tof_sensor = setup_tof(i2c, &mut delay);

        defmt::info!("init done!");

        poll_tof::spawn().ok();

        (
            Shared { delay },
            Local {
                watchdog,
                tof_sensor,
            },
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

    /// Set up the TOF Sensor
    fn setup_tof<D>(mut i2c: I2C1, delay: &mut D) -> TOFSensor
    where
        D: vl53l1::Delay,
    {
        let mut vl53l1_dev = vl53l1::Device::default();
        defmt::info!("Software reset...");
        vl53l1::software_reset(&mut vl53l1_dev, &mut i2c, delay).unwrap();
        defmt::info!("  Complete");

        defmt::info!("Data init...");
        vl53l1::data_init(&mut vl53l1_dev, &mut i2c).unwrap();
        defmt::info!("  Complete");

        defmt::info!("Static init...");
        vl53l1::static_init(&mut vl53l1_dev).unwrap();
        defmt::info!("  Complete");

        defmt::info!("Setting region of interest...");
        let roi = vl53l1::UserRoi {
            bot_right_x: 10,
            bot_right_y: 6,
            top_left_x: 6,
            top_left_y: 10,
        };
        vl53l1::set_user_roi(&mut vl53l1_dev, roi).unwrap();
        defmt::info!("  Complete");

        defmt::info!("Setting timing budget and inter-measurement period...");
        vl53l1::set_measurement_timing_budget_micro_seconds(&mut vl53l1_dev, 50_000).unwrap();
        vl53l1::set_inter_measurement_period_milli_seconds(&mut vl53l1_dev, 60).unwrap();

        defmt::info!("Start measurement...");
        vl53l1::start_measurement(&mut vl53l1_dev, &mut i2c).unwrap();
        defmt::info!("  Complete");

        defmt::info!("Wait measurement data ready...");
        vl53l1::wait_measurement_data_ready(&mut vl53l1_dev, &mut i2c, delay).unwrap();
        defmt::info!("  Ready");

        TOFSensor {
            device: vl53l1_dev,
            i2c,
        }
    }

    //#[task(binds=EXTI0, shared=[delay], local=[tof_sensor])]
    //fn tof_interrupt_triggered(mut ctx: tof_interrupt_triggered::Context) {
    #[task(priority=1, shared=[delay], local=[tof_sensor])]
    fn poll_tof(mut ctx: poll_tof::Context) {
        let vl53l1_dev = &mut ctx.local.tof_sensor.device;
        let i2c = &mut ctx.local.tof_sensor.i2c;
        ctx.shared.delay.lock(|delay| {
            defmt::info!("Wait measurement data ready...");
            vl53l1::wait_measurement_data_ready(vl53l1_dev, i2c, delay).unwrap();
            defmt::info!("  Ready");

            defmt::info!("Get ranging measurement data...");
            let rmd = vl53l1::get_ranging_measurement_data(vl53l1_dev, i2c).unwrap();
            vl53l1::clear_interrupt_and_start_measurement(vl53l1_dev, i2c, delay).unwrap();
            defmt::info!("  {:#?} mm", rmd.range_milli_meter);
        });

        poll_tof::spawn_after(1.secs()).ok();
    }

    /// Feed the watchdog to avoid hardware reset.
    #[task(priority=5, local=[watchdog])]
    fn periodic(ctx: periodic::Context) {
        defmt::trace!("feeding the watchdog!");
        ctx.local.watchdog.feed();

        periodic::spawn_after(100.millis()).ok();
    }
}
