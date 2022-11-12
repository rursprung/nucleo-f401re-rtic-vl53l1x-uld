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
        gpio::{Edge, PB8, PB9},
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
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let mut syscfg = ctx.device.SYSCFG.constrain();

        let rcc = ctx.device.RCC.constrain();
        let clocks = setup_clocks(rcc);
        let mono = ctx.device.TIM2.monotonic_us(&clocks);

        let mut delay = ctx.core.SYST.delay(&clocks);

        let watchdog = setup_watchdog(ctx.device.IWDG);

        // set up I2C
        let gpiob = ctx.device.GPIOB.split();
        let i2c = I2c::new(ctx.device.I2C1, (gpiob.pb8, gpiob.pb9), 400.kHz(), &clocks);

        let gpioa = ctx.device.GPIOA.split();
        let mut tof_data_interrupt = gpioa.pa0.into_pull_down_input();
        tof_data_interrupt.make_interrupt_source(&mut syscfg);
        tof_data_interrupt.enable_interrupt(&mut ctx.device.EXTI);
        tof_data_interrupt.trigger_on_edge(&mut ctx.device.EXTI, Edge::Rising);

        // set up the TOF sensor
        let tof_sensor = setup_tof(i2c, &mut delay);

        defmt::info!("init done!");

        //poll_tof::spawn().ok();

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
        watchdog.start(1000u32.millis());
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
        vl53l1::software_reset(&mut vl53l1_dev, &mut i2c, delay).unwrap();
        vl53l1::data_init(&mut vl53l1_dev, &mut i2c).unwrap();
        vl53l1::static_init(&mut vl53l1_dev).unwrap();

        // TODO: find numbers which work well for this project (don't trigger too often but do trigger often enough)
        vl53l1::set_measurement_timing_budget_micro_seconds(&mut vl53l1_dev, 50_000).unwrap();
        vl53l1::set_inter_measurement_period_milli_seconds(&mut vl53l1_dev, 60).unwrap();

        vl53l1::start_measurement(&mut vl53l1_dev, &mut i2c).unwrap();

        TOFSensor {
            device: vl53l1_dev,
            i2c,
        }
    }

    /// Triggers every time the TOF has data (= new range measurement) available to be consumed.
    #[task(binds=EXTI0, shared=[delay], local=[tof_sensor])]
    fn tof_interrupt_triggered(mut ctx: tof_interrupt_triggered::Context) {
        let vl53l1_dev = &mut ctx.local.tof_sensor.device;
        let i2c = &mut ctx.local.tof_sensor.i2c;
        ctx.shared.delay.lock(|delay| {
            let rmd = vl53l1::get_ranging_measurement_data(vl53l1_dev, i2c).unwrap();
            vl53l1::clear_interrupt_and_start_measurement(vl53l1_dev, i2c, delay).unwrap();
            match rmd.range_status {
                vl53l1::RangeStatus::RANGE_VALID => {
                    defmt::info!("Received range: {}mm", rmd.range_milli_meter)
                }
                status => defmt::warn!(
                    "Received invalid range status: {:?}",
                    defmt::Debug2Format(&status)
                ),
            }
        });
    }

    /// Feed the watchdog to avoid hardware reset.
    #[task(priority=1, local=[watchdog])]
    fn periodic(ctx: periodic::Context) {
        defmt::trace!("feeding the watchdog!");
        ctx.local.watchdog.feed();

        periodic::spawn_after(200.millis()).ok();
    }
}
