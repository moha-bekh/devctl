//! devctl: one tool for this machine's lit devices and GPU fans.

pub mod cli;
pub mod color;
pub mod config;
pub mod device;
pub mod error;
pub mod fan;
pub mod tui;

use std::collections::BTreeMap;

use config::{Config, Lighting};
use device::Device;

/// One line describing a lighting, e.g. `mono 8855FF`.
pub fn describe(lighting: &Lighting) -> String {
    match lighting {
        Lighting::Off => "off".into(),
        Lighting::Mono { color } => format!("mono {color}"),
        Lighting::Custom { regions } => {
            let parts: Vec<String> = regions.iter().map(|(r, c)| format!("{r}={c}")).collect();
            format!("custom {}", parts.join(" "))
        }
        Lighting::Tricolor { colors } => {
            format!("tricolor {} {} {}", colors[0], colors[1], colors[2])
        }
    }
}

/// The per-region colors a device starts from when switching to custom: the
/// saved custom colors, or the mono color everywhere, or white.
pub fn custom_base(saved: Option<&Lighting>, regions: &[String]) -> BTreeMap<String, color::Rgb> {
    match saved {
        Some(Lighting::Custom { regions: map }) => regions
            .iter()
            .map(|r| (r.clone(), map.get(r).copied().unwrap_or_default()))
            .collect(),
        Some(Lighting::Mono { color }) => regions.iter().map(|r| (r.clone(), *color)).collect(),
        Some(Lighting::Tricolor { colors }) => {
            regions.iter().map(|r| (r.clone(), colors[0])).collect()
        }
        _ => regions
            .iter()
            .map(|r| (r.clone(), color::Rgb::WHITE))
            .collect(),
    }
}

/// Shows each device's saved lighting and marks the config on. Devices with
/// nothing saved are left as they are. Returns one line per device, and the
/// failures.
pub fn power_on(
    devices: &mut [Box<dyn Device>],
    config: &mut Config,
) -> (Vec<String>, Vec<String>) {
    let (mut done, mut failed) = (Vec::new(), Vec::new());
    for device in devices {
        match config.lights.get(device.id()) {
            Some(lighting) => match device::apply(device.as_mut(), lighting) {
                Ok(()) => done.push(format!("{}: {}", device.id(), describe(lighting))),
                Err(e) => failed.push(format!("{e:#}")),
            },
            None => done.push(format!("{}: nothing saved, left as is", device.id())),
        }
    }
    config.on = true;
    (done, failed)
}

/// Turns every device off and marks the config off, keeping the saved
/// lighting for the next [`power_on`].
pub fn power_off(
    devices: &mut [Box<dyn Device>],
    config: &mut Config,
) -> (Vec<String>, Vec<String>) {
    let (mut done, mut failed) = (Vec::new(), Vec::new());
    for device in devices {
        match device::apply(device.as_mut(), &Lighting::Off) {
            Ok(()) => done.push(format!("{}: off", device.id())),
            Err(e) => failed.push(format!("{e:#}")),
        }
    }
    config.on = false;
    (done, failed)
}
