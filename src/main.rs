#![deny(unsafe_code)]
#![no_main]
#![no_std]

// Halt on panic
//use panic_halt as _; // panic handler
use panic_probe as _;

use defmt_rtt as _;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI1, EXTI2])]
mod app {
    use stm32f4xx_hal::pac::IWDG;
    use stm32f4xx_hal::rcc::{Clocks, Rcc};
    use stm32f4xx_hal::{
        gpio::{Edge, Input, PA0, PB8, PB9},
        i2c::{I2c, I2c1},
        pac,
        prelude::*,
        timer::MonoTimerUs,
        watchdog::IndependentWatchdog,
    };
    use vl53l1x_uld::{IOVoltage, Polarity, VL53L1X};

    #[monotonic(binds = TIM2, default = true)]
    type MicrosecMono = MonoTimerUs<pac::TIM2>;

    type I2C1 = I2c1<(PB8, PB9)>;
    type TOFSensor = VL53L1X<I2C1>;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        watchdog: IndependentWatchdog,
        tof_sensor: TOFSensor,
        tof_data_interrupt: PA0<Input>,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let mut syscfg = ctx.device.SYSCFG.constrain();

        let rcc = ctx.device.RCC.constrain();
        let clocks = setup_clocks(rcc);
        let mono = ctx.device.TIM2.monotonic_us(&clocks);

        let watchdog = setup_watchdog(ctx.device.IWDG);

        // set up I2C
        let gpiob = ctx.device.GPIOB.split();
        let i2c = I2c::new(ctx.device.I2C1, (gpiob.pb8, gpiob.pb9), 400.kHz(), &clocks);

        let gpioa = ctx.device.GPIOA.split();
        let mut tof_data_interrupt = gpioa.pa0.into_pull_down_input();
        tof_data_interrupt.make_interrupt_source(&mut syscfg);
        tof_data_interrupt.enable_interrupt(&mut ctx.device.EXTI);
        tof_data_interrupt.trigger_on_edge(&mut ctx.device.EXTI, Edge::Falling);

        // set up the TOF sensor
        let tof_sensor = setup_tof(i2c);

        defmt::info!("init done!");

        (
            Shared {},
            Local {
                watchdog,
                tof_sensor,
                tof_data_interrupt,
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
    fn setup_tof(i2c: I2C1) -> TOFSensor {
        let mut dev = VL53L1X::new(i2c, vl53l1x_uld::DEFAULT_ADDRESS);
        dev.init(IOVoltage::Volt2_8).expect("");
        dev.set_interrupt_polarity(Polarity::ActiveHigh).expect("");
        dev.start_ranging().expect("");

        dev
    }

    /// Triggers every time the TOF has data (= new range measurement) available to be consumed.
    #[task(binds=EXTI0, local=[tof_sensor, tof_data_interrupt])]
    fn tof_interrupt_triggered(mut ctx: tof_interrupt_triggered::Context) {
        ctx.local.tof_data_interrupt.clear_interrupt_pending_bit();

        let vl53l1x_dev = &mut ctx.local.tof_sensor;
        if let Ok(distance) = vl53l1x_dev.get_distance() {
            defmt::info!("Received range: {}mm", distance);
        }
        vl53l1x_dev.clear_interrupt().ok();
    }

    /// Feed the watchdog to avoid hardware reset.
    #[task(priority=1, local=[watchdog])]
    fn periodic(ctx: periodic::Context) {
        defmt::trace!("feeding the watchdog!");
        ctx.local.watchdog.feed();

        periodic::spawn_after(200.millis()).ok();
    }
}
