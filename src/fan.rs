//! The GPU fans, through NVML. Reading works as any user; setting a speed
//! needs root, so it is done by `devctl fan daemon`, a system service that
//! follows the config file the user edits.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;

use crate::config::{self, GpuFan};

/// Degrees below `handoff` before the minimum is forced again, so the fans
/// don't switch back and forth around the threshold.
pub const HYSTERESIS: u32 = 8;
const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct GpuStatus {
    pub temperature: u32,
    /// Percent, one per fan.
    pub speeds: Vec<u32>,
}

pub fn status(nvml: &Nvml) -> anyhow::Result<GpuStatus> {
    let device = nvml.device_by_index(0)?;
    let temperature = device.temperature(TemperatureSensor::Gpu)?;
    let speeds = (0..device.num_fans()?)
        .map(|i| device.fan_speed(i))
        .collect::<Result<_, _>>()?;
    Ok(GpuStatus {
        temperature,
        speeds,
    })
}

/// Who controls the fans, as decided by [`decide`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The driver's own curve (zero RPM when cool).
    Driver,
    /// Held at this percentage.
    Forced(u32),
}

/// The next fan control, from the current one and the GPU temperature.
/// Forced to `min` below `handoff`, the driver's curve from `handoff` up; once
/// handed to the driver, it is taken back only below `handoff - HYSTERESIS`.
pub fn decide(
    current: Control,
    active: bool,
    fan: &GpuFan,
    temperature: u32,
    started: bool,
) -> Control {
    if !active {
        return Control::Driver;
    }
    match current {
        Control::Forced(_) if temperature >= fan.handoff => Control::Driver,
        Control::Forced(_) => Control::Forced(fan.min),
        Control::Driver => {
            // On start, take over as soon as the GPU is below the hand-off.
            let threshold = if started {
                fan.handoff
            } else {
                fan.handoff.saturating_sub(HYSTERESIS)
            };
            if temperature < threshold {
                Control::Forced(fan.min)
            } else {
                Control::Driver
            }
        }
    }
}

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

/// Runs until SIGTERM/SIGINT, re-reading `config_path` when it changes. Always
/// leaves the fans on the driver's curve.
pub fn daemon(config_path: &Path) -> anyhow::Result<()> {
    // SAFETY: the handler only stores to an atomic.
    unsafe {
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
    }
    let nvml = Nvml::init().context("cannot load NVML (libnvidia-ml.so)")?;
    let mut device = nvml.device_by_index(0)?;
    let fans = device.num_fans()?;

    let mut config = config::load(config_path)?;
    let mut modified = mtime(config_path);
    let mut control = Control::Driver;
    let mut started = true;
    let result = (|| -> anyhow::Result<()> {
        while !STOP.load(Ordering::Relaxed) {
            if mtime(config_path) != modified {
                modified = mtime(config_path);
                match config::load(config_path) {
                    Ok(new) => config = new,
                    Err(e) => eprintln!("keeping the previous config: {e:#}"),
                }
            }
            let temperature = device.temperature(TemperatureSensor::Gpu)?;
            let active = config.on && config.fans.gpu.enabled;
            let next = decide(control, active, &config.fans.gpu, temperature, started);
            started = false;
            if next != control {
                match next {
                    Control::Forced(speed) => {
                        for fan in 0..fans {
                            device
                                .set_fan_speed(fan, speed)
                                .context("cannot set the fan speed (needs root)")?;
                        }
                        println!("{temperature}°C: fans held at {speed}%");
                    }
                    Control::Driver => {
                        for fan in 0..fans {
                            device.set_default_fan_speed(fan)?;
                        }
                        println!("{temperature}°C: fans back on the driver's curve");
                    }
                }
                control = next;
            }
            for _ in 0..INTERVAL.as_millis() / 100 {
                if STOP.load(Ordering::Relaxed) {
                    break;
                }
                sleep(Duration::from_millis(100));
            }
        }
        Ok(())
    })();
    for fan in 0..fans {
        device.set_default_fan_speed(fan)?;
    }
    println!("fans back on the driver's curve");
    result
}

fn mtime(path: &Path) -> Option<SystemTime> {
    path.metadata().and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAN: GpuFan = GpuFan {
        enabled: true,
        min: 30,
        handoff: 60,
    };

    #[test]
    fn a_cool_gpu_has_its_fans_held_at_the_minimum() {
        assert_eq!(
            decide(Control::Driver, true, &FAN, 35, true),
            Control::Forced(30)
        );
    }

    #[test]
    fn a_hot_gpu_is_left_to_the_driver_and_taken_back_below_the_hysteresis() {
        let hot = decide(Control::Forced(30), true, &FAN, 60, false);
        assert_eq!(hot, Control::Driver);
        assert_eq!(decide(hot, true, &FAN, 55, false), Control::Driver);
        assert_eq!(decide(hot, true, &FAN, 51, false), Control::Forced(30));
    }

    #[test]
    fn a_new_minimum_is_followed_while_forced() {
        let fan = GpuFan { min: 40, ..FAN };
        assert_eq!(
            decide(Control::Forced(30), true, &fan, 35, false),
            Control::Forced(40)
        );
    }

    #[test]
    fn turned_off_means_the_driver_s_curve() {
        assert_eq!(
            decide(Control::Forced(30), false, &FAN, 35, false),
            Control::Driver
        );
    }
}
