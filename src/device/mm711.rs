//! Cooler Master MM711 mouse: two LEDs (scroll wheel, logo) on its vendor
//! HID interface (USB interface 1). Protocol from OpenRGB's CMMM711Controller.

use std::fs::File;

use anyhow::Context;

use super::{Device, hidraw};
use crate::color::Rgb;

const VENDOR: u16 = 0x2516;
const PRODUCT: u16 = 0x0101;
const INTERFACE: u8 = 1;
/// Report id 0 plus 64 bytes.
const PACKET: usize = 65;
const REPLY_TIMEOUT_MS: i32 = 250;

const MODE_CUSTOM: u8 = 0xB0;
const MODE_OFF: u8 = 0xFE;
const SPEED_NORMAL: u8 = 0x38;
const BRIGHTNESS_MAX: u8 = 0xFF;

pub struct Mm711 {
    file: File,
    regions: Vec<String>,
}

pub fn detect() -> anyhow::Result<Option<Mm711>> {
    let Ok((file, _)) = hidraw::open(VENDOR, PRODUCT, Some(INTERFACE)) else {
        return Ok(None);
    };
    let mouse = Mm711 {
        file,
        regions: vec!["wheel".into(), "logo".into()],
    };
    mouse.command(&[0x41, 0x80]).context("mouse: init failed")?;
    Ok(Some(mouse))
}

impl Mm711 {
    /// Sends one command and waits for the mouse's answer.
    fn command(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let mut packet = [0; PACKET];
        packet[1..=bytes.len()].copy_from_slice(bytes);
        hidraw::write(&self.file, &packet)?;
        hidraw::read_timeout(&self.file, &mut packet, REPLY_TIMEOUT_MS)?;
        Ok(())
    }

    fn set_mode(&self, mode: u8) -> anyhow::Result<()> {
        let update = [
            0x51,
            0x2B,
            0,
            0,
            mode,
            SPEED_NORMAL,
            0x20,
            0xFF,
            0xFF,
            BRIGHTNESS_MAX,
            0,
            0,
            0,
        ];
        self.command(&update)?;
        self.command(&[0x51, 0x28, 0, 0, mode])
    }
}

impl Device for Mm711 {
    fn id(&self) -> &'static str {
        "mouse"
    }

    fn name(&self) -> String {
        "Cooler Master MM711".into()
    }

    fn regions(&self) -> &[String] {
        &self.regions
    }

    fn set_colors(&mut self, colors: &[Rgb]) -> anyhow::Result<()> {
        if colors.iter().all(|c| c.is_black()) {
            return self.set_mode(MODE_OFF).context("mouse: cannot turn off");
        }
        let wheel = colors.first().copied().unwrap_or_default();
        let logo = colors.get(1).copied().unwrap_or_default();
        self.set_mode(MODE_CUSTOM)
            .context("mouse: cannot switch to custom colors")?;
        self.command(&[
            0x51, 0xA8, 0, 0, wheel.0, wheel.1, wheel.2, logo.0, logo.1, logo.2,
        ])
        .context("mouse: cannot set colors")
    }
}
