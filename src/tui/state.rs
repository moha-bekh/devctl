//! What the views look at and change: the devices, the saved config, and
//! the GPU's live temperature. Every change is shown on the hardware and
//! saved at once.

use std::path::{Path, PathBuf};
use std::process::Command;

use nvml_wrapper::Nvml;

use crate::config::{self, Config, Lighting};
use crate::device::{self, Device};
use crate::fan::{self, GpuStatus};
use crate::{describe, power_off, power_on};

pub struct State {
    pub devices: Vec<Box<dyn Device>>,
    pub config: Config,
    path: PathBuf,
    nvml: Option<Nvml>,
    pub gpu: Option<GpuStatus>,
    /// Whether devctl-fan.service runs; `None` when systemctl can't say.
    pub daemon: Option<bool>,
    /// The last outcome, and whether it was a failure.
    pub message: Option<(String, bool)>,
}

impl State {
    pub fn load(path: &Path) -> anyhow::Result<State> {
        let detected = device::detect();
        let message = (!detected.problems.is_empty()).then(|| (detected.problems.join("; "), true));
        let mut state = State {
            devices: detected.devices,
            config: config::load(path)?,
            path: path.to_path_buf(),
            nvml: Nvml::init().ok(),
            gpu: None,
            daemon: None,
            message,
        };
        state.refresh();
        Ok(state)
    }

    /// Re-reads the live GPU numbers and the daemon's state.
    pub fn refresh(&mut self) {
        self.gpu = self.nvml.as_ref().and_then(|nvml| fan::status(nvml).ok());
        self.daemon = Command::new("systemctl")
            .args(["is-active", "--quiet", "devctl-fan.service"])
            .status()
            .ok()
            .map(|status| status.success());
    }

    pub fn lighting(&self, device: usize) -> Option<&Lighting> {
        self.config.lights.get(self.devices[device].id())
    }

    pub fn set_lighting(&mut self, index: usize, lighting: Lighting) {
        let device = &mut self.devices[index];
        let outcome = device::apply(device.as_mut(), &lighting);
        let id = device.id();
        self.message = Some(match outcome {
            Ok(()) => (format!("{id}: {}", describe(&lighting)), false),
            Err(e) => (format!("{e:#}"), true),
        });
        self.config.lights.insert(id.to_string(), lighting);
        self.config.on = true;
        self.save();
    }

    pub fn toggle_power(&mut self) {
        let (done, failed) = if self.config.on {
            power_off(&mut self.devices, &mut self.config)
        } else {
            power_on(&mut self.devices, &mut self.config)
        };
        self.message = Some(if failed.is_empty() {
            (
                format!(
                    "{}: {}",
                    if self.config.on { "on" } else { "off" },
                    done.join(", ")
                ),
                false,
            )
        } else {
            (failed.join("; "), true)
        });
        self.save();
    }

    pub fn save(&mut self) {
        if let Err(e) = config::save(&self.path, &self.config) {
            self.message = Some((format!("{e:#}"), true));
        }
    }
}

#[cfg(test)]
impl State {
    /// A state over `devices`, with no GPU, saving into a scratch file.
    pub fn fake(devices: Vec<Box<dyn Device>>, config: Config) -> State {
        let path = std::env::temp_dir().join(format!("devctl-test-{}.toml", std::process::id()));
        State {
            devices,
            config,
            path,
            nvml: None,
            gpu: None,
            daemon: Some(true),
            message: None,
        }
    }
}
