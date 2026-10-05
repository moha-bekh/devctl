//! The saved state: what each device should show, and how the GPU fans
//! behave. One TOML file, written by the CLI and the terminal UI, and read by
//! the root fan daemon.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::color::Rgb;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// False after `devctl off`: the lights are dark and the GPU fans may
    /// stop. The lighting below is kept for the next `devctl on`.
    pub on: bool,
    /// Keyed by device id (`ram`, `gpu`, `board`, `mouse`).
    pub lights: BTreeMap<String, Lighting>,
    pub fans: Fans,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            on: true,
            lights: BTreeMap::new(),
            fans: Fans::default(),
        }
    }
}

/// What one device shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Lighting {
    Off,
    /// Every region in one color.
    Mono {
        color: Rgb,
    },
    /// One color per region; a region left out is off.
    Custom {
        regions: BTreeMap<String, Rgb>,
    },
    /// The Gigabyte GPU's built-in three-color animation.
    Tricolor {
        colors: [Rgb; 3],
    },
}

impl Lighting {
    pub const MODES: [&'static str; 4] = ["off", "mono", "custom", "tricolor"];

    pub fn mode(&self) -> &'static str {
        match self {
            Lighting::Off => "off",
            Lighting::Mono { .. } => "mono",
            Lighting::Custom { .. } => "custom",
            Lighting::Tricolor { .. } => "tricolor",
        }
    }

    /// The colors to show for `regions`, one each, in order.
    pub fn colors_for(&self, regions: &[String]) -> Vec<Rgb> {
        match self {
            Lighting::Off | Lighting::Tricolor { .. } => vec![Rgb::BLACK; regions.len()],
            Lighting::Mono { color } => vec![*color; regions.len()],
            Lighting::Custom { regions: map } => regions
                .iter()
                .map(|r| map.get(r).copied().unwrap_or(Rgb::BLACK))
                .collect(),
        }
    }
}

pub const DEFAULT_TRICOLOR: [Rgb; 3] = [Rgb(0x88, 0x55, 0xFF), Rgb::BLACK, Rgb(0x00, 0xFF, 0xFF)];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fans {
    pub gpu: GpuFan,
}

/// The GPU's RGB is dark while its fans are stopped (zero RPM at idle), so
/// below `handoff` °C the daemon holds them at `min` %. From `handoff` up, the
/// driver's own curve cools the card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GpuFan {
    pub enabled: bool,
    pub min: u32,
    pub handoff: u32,
}

impl Default for GpuFan {
    fn default() -> GpuFan {
        GpuFan {
            enabled: true,
            min: 30,
            handoff: 60,
        }
    }
}

/// `$XDG_CONFIG_HOME/devctl/config.toml`, else `~/.config/devctl/config.toml`.
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("devctl").join("config.toml")
}

/// The config at `path`, or the defaults when there is no file yet.
pub fn load(path: &Path) -> anyhow::Result<Config> {
    match fs::read_to_string(path) {
        Ok(text) => {
            toml::from_str(&text).with_context(|| format!("cannot read {}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Writes through a temporary file and a rename, so the fan daemon never
/// reads a half-written config.
pub fn save(path: &Path, config: &Config) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, toml::to_string(config)?)
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_survives_a_round_trip_through_toml() {
        let mut config = Config::default();
        config.lights.insert(
            "gpu".into(),
            Lighting::Tricolor {
                colors: DEFAULT_TRICOLOR,
            },
        );
        config
            .lights
            .insert("ram".into(), Lighting::Mono { color: Rgb::WHITE });
        let regions = BTreeMap::from([("logo".to_string(), Rgb(1, 2, 3))]);
        config
            .lights
            .insert("mouse".into(), Lighting::Custom { regions });
        config.lights.insert("board".into(), Lighting::Off);
        let text = toml::to_string(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
    }

    #[test]
    fn a_custom_region_left_out_is_off() {
        let regions = BTreeMap::from([("logo".to_string(), Rgb::WHITE)]);
        let names = vec!["wheel".to_string(), "logo".to_string()];
        assert_eq!(
            Lighting::Custom { regions }.colors_for(&names),
            vec![Rgb::BLACK, Rgb::WHITE]
        );
    }
}
