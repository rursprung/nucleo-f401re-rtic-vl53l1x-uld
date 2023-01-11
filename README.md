# VL53L1X (TOF) Using Rust on NUCLEO-F401RE ARM32 Board
[![CI](https://github.com/rursprung/nucleo-f401re-rtic-vl53l1x-uld/actions/workflows/CI.yml/badge.svg)](https://github.com/rursprung/nucleo-f401re-rtic-vl53l1x-uld/actions/workflows/CI.yml)

This example showcases how the [`vl53l1x-uld`](https://crates.io/crates/vl53l1x-uld) crate for the [VL53L1X](https://www.st.com/en/imaging-and-photonics-solutions/vl53l1x.html) TOF can be used on an STM32F4 chip.

The example logs messages using [`defmt`](https://defmt.ferrous-systems.com/).

The example has been tested on a [ST Nucleo-F401RE](https://www.st.com/en/evaluation-tools/nucleo-f401re.html) development
board but should work on any STM32F4xx family microcontroller as long as the TOF is connected via I2C1 on pins `PB8` (SCL) and `PB9` (SDA)
and the interrupt is connected on `PA0`, or the code is adapted accordingly.

## Prerequisites
1. [Install Rust](https://www.rust-lang.org/tools/install)
1. Optional: ensure that the rust toolchain is up-to-date: `rustup update`
1. Install [`probe-run`](https://crates.io/crates/probe-run): `cargo install probe-run`
1. Install [`flip-link`](https://crates.io/crates/flip-link): `cargo install flip-link`
    * Note: `flip-link` is not strictly necessary for this example (it doesn't need
      stack protection), however it can be considered best practices to include it.
1. Install the cross-compile target: `rustup target add thumbv7em-none-eabihf`
1. Install the STLink drivers

## Build & Download to Board
1. Connect the board via USB
1. Optional: change your targeted platform in `Cargo.toml` and `.cargo/config` (it defaults to STM32F401RE)
1. Run `cargo run`
1. Enjoy your running program :)
