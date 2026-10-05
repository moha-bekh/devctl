//! The lit devices of this machine, behind one trait. Each module finds its
//! device itself (bus or USB ids) and speaks its protocol directly; nothing
//! goes through OpenRGB.

mod ene_dram;
mod fusion2_gpu;
mod hidraw;
mod i2c;
mod mm711;
mod mystic_light;

use anyhow::bail;

use crate::color::Rgb;
use crate::config::Lighting;
use crate::error::ParseError;

pub trait Device {
    /// Short name used on the command line and as the config key.
    fn id(&self) -> &'static str;
    fn name(&self) -> String;
    /// The parts that can each take their own color, in a stable order.
    fn regions(&self) -> &[String];
    fn supports_tricolor(&self) -> bool {
        false
    }
    /// One color per region, in `regions()` order. Black turns a region off.
    fn set_colors(&mut self, colors: &[Rgb]) -> anyhow::Result<()>;
    fn set_tricolor(&mut self, _colors: [Rgb; 3]) -> anyhow::Result<()> {
        bail!("{} has no tricolor effect", self.id())
    }
}

/// Every device id, in display order.
pub const IDS: [&str; 4] = ["ram", "gpu", "board", "mouse"];

/// The devices that answered, and why the others could not be used. A
/// device that is simply absent is neither.
pub struct Detected {
    pub devices: Vec<Box<dyn Device>>,
    pub problems: Vec<String>,
}

pub fn detect() -> Detected {
    let mut detected = Detected {
        devices: Vec::new(),
        problems: Vec::new(),
    };
    let mut add = |found: anyhow::Result<Option<Box<dyn Device>>>| match found {
        Ok(Some(device)) => detected.devices.push(device),
        Ok(None) => {}
        Err(e) => detected.problems.push(format!("{e:#}")),
    };
    add(ene_dram::detect().map(|d| d.map(|d| Box::new(d) as _)));
    add(fusion2_gpu::detect().map(|d| d.map(|d| Box::new(d) as _)));
    add(mystic_light::detect().map(|d| d.map(|d| Box::new(d) as _)));
    add(mm711::detect().map(|d| d.map(|d| Box::new(d) as _)));
    detected
}

/// Shows `lighting` on `device`, refusing what the device cannot show.
pub fn apply(device: &mut dyn Device, lighting: &Lighting) -> anyhow::Result<()> {
    check(device, lighting)?;
    match lighting {
        Lighting::Tricolor { colors } => device.set_tricolor(*colors),
        other => {
            let colors = other.colors_for(device.regions());
            device.set_colors(&colors)
        }
    }
}

/// Refuses a lighting that names a region `device` lacks, or an effect it
/// does not have.
pub fn check(device: &dyn Device, lighting: &Lighting) -> Result<(), ParseError> {
    match lighting {
        Lighting::Tricolor { .. } if !device.supports_tricolor() => Err(ParseError(format!(
            "{} has no tricolor effect (only the gpu has one)",
            device.id()
        ))),
        Lighting::Custom { regions } => {
            for name in regions.keys() {
                if !device.regions().contains(name) {
                    return Err(ParseError(format!(
                        "{} has no region `{name}` (its regions are {})",
                        device.id(),
                        device.regions().join(", ")
                    )));
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
