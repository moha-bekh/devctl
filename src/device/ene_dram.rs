//! ENE RAM sticks (`AUDA0-E6K5-0101`) on the chipset's SMBus.
//!
//! OpenRGB's writes to these arrive corrupted ("Static" turns them off,
//! "Direct" is never latched), so every register is written one byte at a
//! time and read back. Each stick is one region, all its LEDs in one color.

use std::io;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, anyhow};

use super::Device;
use super::i2c::Bus;
use crate::color::Rgb;

const BUS: &str = "SMBus PIIX4 adapter port 0";
/// Where sticks are moved to, in order. Sticks all answer on 0x77 after a
/// power loss, until each is given its own address.
const ADDRESSES: [u8; 8] = [0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77];
const UNMAPPED: u8 = 0x77;
/// The order the sticks sit in, as seen from the front of the case. The
/// remap hands out addresses by slot, and slots are not wired in order:
/// 0x72 sits between 0x70 and 0x71.
const PHYSICAL_ORDER: [u8; 8] = [0x70, 0x72, 0x71, 0x73, 0x74, 0x75, 0x76, 0x77];

const REG_MICRON_CHECK: u16 = 0x1030;
const REG_CONFIG_TABLE: u16 = 0x1C00;
const CONFIG_LED_COUNT: u16 = 0x02;
const REG_DIRECT: u16 = 0x8020;
const REG_APPLY: u16 = 0x80A0;
const REG_SLOT_INDEX: u16 = 0x80F8;
const REG_I2C_ADDRESS: u16 = 0x80F9;
/// 3 bytes per LED, in R, B, G order.
const REG_COLORS_DIRECT_V2: u16 = 0x8100;
const TRIES: usize = 3;

struct Stick {
    addr: u8,
    leds: u8,
}

pub struct EneDram {
    bus: Bus,
    sticks: Vec<Stick>,
    regions: Vec<String>,
}

pub fn detect() -> anyhow::Result<Option<EneDram>> {
    let Ok(bus) = Bus::find(BUS) else {
        return Ok(None);
    };
    remap(&bus).context("cannot give the RAM sticks their addresses")?;
    let mut sticks = Vec::new();
    for addr in PHYSICAL_ORDER {
        if is_ene(&bus, addr) {
            let leds = read(&bus, addr, REG_CONFIG_TABLE + CONFIG_LED_COUNT)?;
            if (1..=32).contains(&leds) {
                sticks.push(Stick { addr, leds });
            }
        }
    }
    if sticks.is_empty() {
        return Ok(None);
    }
    let regions = (1..=sticks.len()).map(|i| format!("stick{i}")).collect();
    Ok(Some(EneDram {
        bus,
        sticks,
        regions,
    }))
}

/// Gives each stick still on 0x77 the next free address, as OpenRGB does.
fn remap(bus: &Bus) -> io::Result<()> {
    let mut next = 0;
    for slot in 0..8 {
        if bus.read_byte(UNMAPPED).is_err() {
            break;
        }
        while next < ADDRESSES.len() && bus.read_byte(ADDRESSES[next]).is_ok() {
            next += 1;
        }
        if next == ADDRESSES.len() {
            break;
        }
        write_unchecked(bus, UNMAPPED, REG_SLOT_INDEX, slot)?;
        write_unchecked(bus, UNMAPPED, REG_I2C_ADDRESS, ADDRESSES[next] << 1)?;
    }
    Ok(())
}

/// ENE controllers echo their offset in registers 0xA0-0xAF; Micron sticks
/// mimic that but are a different controller.
fn is_ene(bus: &Bus, addr: u8) -> bool {
    if bus.read_byte(addr).is_err() && bus.read_byte_data(addr, 0).is_err() {
        return false;
    }
    if !(0xA0..0xB0).all(|i| bus.read_byte_data(addr, i).ok() == Some(i - 0xA0)) {
        return false;
    }
    let micron: Vec<u8> = (0..6)
        .filter_map(|i| read(bus, addr, REG_MICRON_CHECK + i).ok())
        .collect();
    micron != b"Micron"
}

/// The register address goes out byte-swapped.
fn select(bus: &Bus, addr: u8, reg: u16) -> io::Result<()> {
    bus.write_word_data(addr, 0x00, reg.swap_bytes())
}

fn read(bus: &Bus, addr: u8, reg: u16) -> io::Result<u8> {
    select(bus, addr, reg)?;
    bus.read_byte_data(addr, 0x81)
}

fn write_unchecked(bus: &Bus, addr: u8, reg: u16, value: u8) -> io::Result<()> {
    select(bus, addr, reg)?;
    bus.write_byte_data(addr, 0x01, value)
}

fn write(bus: &Bus, addr: u8, reg: u16, value: u8) -> anyhow::Result<()> {
    for _ in 0..TRIES {
        write_unchecked(bus, addr, reg, value)?;
        if read(bus, addr, reg)? == value {
            return Ok(());
        }
        sleep(Duration::from_millis(10));
    }
    Err(anyhow!(
        "RAM stick 0x{addr:02X}: register 0x{reg:04X} did not take 0x{value:02X}"
    ))
}

impl Device for EneDram {
    fn id(&self) -> &'static str {
        "ram"
    }

    fn name(&self) -> String {
        format!("ENE DRAM ({} sticks)", self.sticks.len())
    }

    fn regions(&self) -> &[String] {
        &self.regions
    }

    fn set_colors(&mut self, colors: &[Rgb]) -> anyhow::Result<()> {
        for (stick, color) in self.sticks.iter().zip(colors) {
            write(&self.bus, stick.addr, REG_DIRECT, 1)?;
            for led in 0..stick.leds as u16 {
                let base = REG_COLORS_DIRECT_V2 + 3 * led;
                write(&self.bus, stick.addr, base, color.0)?;
                write(&self.bus, stick.addr, base + 1, color.2)?;
                write(&self.bus, stick.addr, base + 2, color.1)?;
            }
            // The apply register clears itself once the colors are latched.
            write_unchecked(&self.bus, stick.addr, REG_APPLY, 1)?;
        }
        Ok(())
    }
}
