//! The command line, as clap derives it.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::color::Rgb;

#[derive(Debug, Parser)]
#[command(
    name = "devctl",
    version,
    about = "Control this machine's RGB lighting and GPU fans",
    long_about = "Control this machine's RGB lighting and GPU fans.\n\n\
        With no command, opens the terminal UI. Every change is saved to the \
        config file, which `devctl on` replays and the fan daemon follows."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Config file [default: ~/.config/devctl/config.toml]
    #[arg(long, value_name = "FILE", global = true)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open the terminal UI (the default)
    Ui,
    /// List the devices found, their regions and their saved lighting
    List,
    /// Set a device's lighting, show it and save it
    Set(SetArgs),
    /// Show the saved lighting on every device, and keep the GPU fans spinning
    On,
    /// Turn every light off, and let the GPU fans stop
    Off,
    /// Show or change how the GPU fans are held
    Fan(FanArgs),
}

#[derive(Debug, Args)]
pub struct SetArgs {
    /// ram, gpu, board, mouse, or all
    pub device: String,
    #[command(subcommand)]
    pub lighting: LightingArgs,
}

#[derive(Debug, Subcommand)]
pub enum LightingArgs {
    /// Turn the device off
    Off,
    /// One color everywhere
    Mono {
        /// RRGGBB, e.g. 8855FF
        color: Rgb,
    },
    /// One color per region; regions not named keep their current color
    Custom {
        /// e.g. wheel=FF0000 logo=00FFFF (see `devctl list` for the regions)
        #[arg(required = true, value_name = "REGION=RRGGBB")]
        regions: Vec<String>,
    },
    /// The GPU's three-color animation
    Tricolor {
        #[arg(num_args = 3, required = true, value_name = "RRGGBB")]
        colors: Vec<Rgb>,
    },
}

#[derive(Debug, Args)]
pub struct FanArgs {
    #[command(subcommand)]
    pub action: Option<FanAction>,

    /// Speed the fans are held at while the GPU is cool, in percent
    #[arg(long, value_name = "PERCENT", value_parser = clap::value_parser!(u32).range(0..=100))]
    pub min: Option<u32>,

    /// Temperature from which the driver's own curve takes over, in °C
    #[arg(long, value_name = "CELSIUS", value_parser = clap::value_parser!(u32).range(30..=90))]
    pub handoff: Option<u32>,

    /// Hold the fans at the minimum (while the lights are on)
    #[arg(long, conflicts_with = "disable")]
    pub enable: bool,

    /// Leave the fans to the driver (zero RPM when cool)
    #[arg(long)]
    pub disable: bool,
}

#[derive(Debug, Subcommand)]
pub enum FanAction {
    /// Run the fan control loop (as root, from devctl-fan.service)
    #[command(hide = true)]
    Daemon,
}
