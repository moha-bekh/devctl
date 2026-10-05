//! MSI Mystic Light 185-byte motherboard controller (MPG X570 Gaming Pro
//! Carbon WiFi), driven through its HID feature report 0x52.
//!
//! OpenRGB's "Static" alternates between the color and black from one call
//! to the next, so the report is read, patched and written back here, then
//! read again to check. Nothing is saved to the board's flash.

use std::fs::File;

use anyhow::{Context, bail};

use super::{Device, hidraw};
use crate::color::Rgb;

const VENDOR: u16 = 0x1462;
const PRODUCT: u16 = 0x7B93;
const REPORT_ID: u8 = 0x52;
const REPORT_SIZE: usize = 185;
const MODE_DISABLE: u8 = 0;
const MODE_STATIC: u8 = 1;
/// Brightness lives in bits 2-6 of the flags byte.
const BRIGHTNESS_MAX: u8 = 10 << 2;
const SAVE_DATA: usize = REPORT_SIZE - 1;

/// Region names and their offsets in the report. Each zone starts with mode,
/// R, G, B, flags, then a second color.
const ZONES: [(&str, usize); 18] = [
    ("jrgb1", 1),
    ("jpipe1", 11),
    ("jpipe2", 21),
    ("jrainbow1", 31),
    ("jrainbow2", 42),
    ("jcorsair", CORSAIR),
    ("jcorsair-outer", 64),
    ("onboard1", 74),
    ("onboard2", 84),
    ("onboard3", 94),
    ("onboard4", 104),
    ("onboard5", 114),
    ("onboard6", 124),
    ("onboard7", 134),
    ("onboard8", 144),
    ("onboard9", 154),
    ("onboard10", 164),
    ("onboard11", 174),
];
/// JCORSAIR only shares the mode/color/flags prefix: the rest holds fan
/// settings and a byte the board changes on every write.
const CORSAIR: usize = 53;

pub struct MysticLight {
    file: File,
    regions: Vec<String>,
}

pub fn detect() -> anyhow::Result<Option<MysticLight>> {
    let Ok((file, _)) = hidraw::open(VENDOR, PRODUCT, None) else {
        return Ok(None);
    };
    let board = MysticLight {
        file,
        regions: ZONES.iter().map(|(name, _)| name.to_string()).collect(),
    };
    board
        .report()
        .context("motherboard: cannot read its feature report")?;
    Ok(Some(board))
}

impl MysticLight {
    fn report(&self) -> std::io::Result<[u8; REPORT_SIZE]> {
        let mut report = [0; REPORT_SIZE];
        report[0] = REPORT_ID;
        hidraw::get_feature(&self.file, &mut report)?;
        Ok(report)
    }
}

impl Device for MysticLight {
    fn id(&self) -> &'static str {
        "board"
    }

    fn name(&self) -> String {
        "MSI MPG X570 Gaming Pro Carbon".into()
    }

    fn regions(&self) -> &[String] {
        &self.regions
    }

    fn set_colors(&mut self, colors: &[Rgb]) -> anyhow::Result<()> {
        let mut report = self
            .report()
            .context("motherboard: cannot read its feature report")?;
        for ((_, at), color) in ZONES.iter().zip(colors) {
            let at = *at;
            let Rgb(r, g, b) = *color;
            report[at] = if color.is_black() {
                MODE_DISABLE
            } else {
                MODE_STATIC
            };
            report[at + 1..at + 4].copy_from_slice(&[r, g, b]);
            report[at + 4] = (report[at + 4] & 0x83) | BRIGHTNESS_MAX;
            if at != CORSAIR {
                report[at + 5..at + 8].copy_from_slice(&[r, g, b]);
            }
        }
        report[SAVE_DATA] = 0;
        hidraw::set_feature(&self.file, &report)
            .context("motherboard: cannot write its feature report")?;

        let check = self
            .report()
            .context("motherboard: cannot read its feature report back")?;
        let bad: Vec<&str> = ZONES
            .iter()
            .filter(|(_, at)| check[*at..*at + 5] != report[*at..*at + 5])
            .map(|(name, _)| *name)
            .collect();
        if !bad.is_empty() {
            bail!("motherboard: {} did not take the new color", bad.join(", "));
        }
        Ok(())
    }
}
