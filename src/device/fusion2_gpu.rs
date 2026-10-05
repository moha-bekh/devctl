//! Gigabyte RGB Fusion 2 GPU controller (AORUS RTX 2080 Ti Xtreme), at 0x50
//! on the card's first i2c adapter.
//!
//! OpenRGB lists five zones, but every visible LED (front, plate, fan rings)
//! follows the fifth; the other four light nothing. So the card is a single
//! region here.

use std::fs;

use anyhow::Context;

use super::Device;
use super::i2c::Bus;
use crate::color::Rgb;

const BUS: &str = "NVIDIA i2c adapter 1 at";
const ADDR: u8 = 0x50;
const GIGABYTE_SUBSYSTEM_VENDOR: &str = "0x1458";
/// OpenRGB's "GPU zone 5".
const ZONE: u8 = 4;
const ZONES: u8 = 5;

const REG_COLOR: u8 = 0x40;
const REG_MODE: u8 = 0x88;
/// Four banks of two colors per zone, from 0xB0.
const REG_BANKS: u8 = 0xB0;
const MODE_STATIC: u8 = 0x01;
/// "Tricolor" in OpenRGB, "Dazzle" in Gigabyte's software. Only the first 3
/// of the 8 color slots are used.
const MODE_TRICOLOR: u8 = 0x0B;
const SPEED_NORMAL: u8 = 0x02;
const BRIGHTNESS_MAX: u8 = 0x63;
/// OpenRGB sends 0x08 for Tricolor; 0x02 gives the animation picked by the
/// user. What the byte means is unknown.
const TRICOLOR_FLAG: u8 = 0x02;

pub struct Fusion2Gpu {
    bus: Bus,
    regions: Vec<String>,
}

pub fn detect() -> anyhow::Result<Option<Fusion2Gpu>> {
    if !gigabyte_gpu_present() {
        return Ok(None);
    }
    let Ok(bus) = Bus::find(BUS) else {
        return Ok(None);
    };
    Ok(Some(Fusion2Gpu {
        bus,
        regions: vec!["card".into()],
    }))
}

fn gigabyte_gpu_present() -> bool {
    let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") else {
        return false;
    };
    entries.flatten().any(|entry| {
        let read = |name| fs::read_to_string(entry.path().join(name)).unwrap_or_default();
        read("vendor").trim() == "0x10de"
            && read("class").starts_with("0x03")
            && read("subsystem_vendor").trim() == GIGABYTE_SUBSYSTEM_VENDOR
    })
}

impl Fusion2Gpu {
    fn send(&self, bytes: [u8; 8]) -> anyhow::Result<()> {
        self.bus
            .write(ADDR, &bytes)
            .with_context(|| format!("GPU: write to {} failed", self.bus.path.display()))
    }
}

impl Device for Fusion2Gpu {
    fn id(&self) -> &'static str {
        "gpu"
    }

    fn name(&self) -> String {
        "Gigabyte AORUS RTX 2080 Ti".into()
    }

    fn regions(&self) -> &[String] {
        &self.regions
    }

    fn supports_tricolor(&self) -> bool {
        true
    }

    fn set_colors(&mut self, colors: &[Rgb]) -> anyhow::Result<()> {
        let Rgb(r, g, b) = colors.first().copied().unwrap_or_default();
        self.send([
            REG_MODE,
            MODE_STATIC,
            SPEED_NORMAL,
            BRIGHTNESS_MAX,
            0,
            ZONE + 1,
            0,
            0,
        ])?;
        self.send([REG_COLOR, r, g, b, ZONE + 1, 0, 0, 0])
    }

    fn set_tricolor(&mut self, colors: [Rgb; 3]) -> anyhow::Result<()> {
        // The card ignores colors written to zone 5's banks alone: they only
        // take when every zone's banks are written, before the mode.
        let slot = |i: usize| colors[i % 3];
        for zone in 0..ZONES {
            for bank in 0..4 {
                let Rgb(r1, g1, b1) = slot(2 * bank);
                let Rgb(r2, g2, b2) = slot(2 * bank + 1);
                self.send([
                    REG_BANKS + zone * 4 + bank as u8,
                    MODE_TRICOLOR,
                    r1,
                    g1,
                    b1,
                    r2,
                    g2,
                    b2,
                ])?;
            }
        }
        self.send([
            REG_MODE,
            MODE_TRICOLOR,
            SPEED_NORMAL,
            BRIGHTNESS_MAX,
            TRICOLOR_FLAG,
            ZONE + 1,
            0,
            0,
        ])
    }
}
