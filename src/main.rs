//! Everything here is dispatch and exit codes. The work lives in the library.

use std::path::Path;
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Parser;
use devctl::cli::{Cli, Command, FanAction, FanArgs, LightingArgs, SetArgs};
use devctl::config::{self, Config, Lighting};
use devctl::device::{self, Device};
use devctl::error::ParseError;
use devctl::{custom_base, describe, fan, tui};

/// The request itself was wrong: unknown device, region or color.
const EXIT_REJECTED: u8 = 2;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            if error.downcast_ref::<ParseError>().is_some() {
                ExitCode::from(EXIT_REJECTED)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let path = cli.config.unwrap_or_else(config::default_path);
    match cli.command {
        None | Some(Command::Ui) => tui::run(&path),
        Some(Command::List) => list(&path),
        Some(Command::Set(args)) => set(&path, args),
        Some(Command::On) => on(&path),
        Some(Command::Off) => off(&path),
        Some(Command::Fan(args)) => fan_cmd(&path, args),
    }
}

fn detect() -> Vec<Box<dyn Device>> {
    let detected = device::detect();
    for problem in &detected.problems {
        eprintln!("warning: {problem}");
    }
    detected.devices
}

fn list(path: &Path) -> anyhow::Result<()> {
    let config = config::load(path)?;
    let devices = detect();
    for device in &devices {
        let saved = config
            .lights
            .get(device.id())
            .map(describe)
            .unwrap_or_else(|| "not set".into());
        println!("{:<6} {}", device.id(), device.name());
        println!("       regions: {}", device.regions().join(", "));
        println!("       saved:   {saved}");
    }
    for id in device::IDS {
        if !devices.iter().any(|d| d.id() == id) {
            println!("{id:<6} not found");
        }
    }
    let gpu = &config.fans.gpu;
    let state = if gpu.enabled {
        "held"
    } else {
        "left to the driver"
    };
    println!("fans   GPU {state}: {}% below {}°C", gpu.min, gpu.handoff);
    println!("power  {}", if config.on { "on" } else { "off" });
    Ok(())
}

/// The devices `id` names (`all` names every one found).
fn pick<'a>(
    devices: &'a mut [Box<dyn Device>],
    id: &str,
) -> anyhow::Result<Vec<&'a mut Box<dyn Device>>> {
    if id == "all" {
        return Ok(devices.iter_mut().collect());
    }
    if !device::IDS.contains(&id) {
        return Err(ParseError(format!(
            "there is no device `{id}` (the devices are {}, or all)",
            device::IDS.join(", ")
        ))
        .into());
    }
    match devices.iter_mut().find(|d| d.id() == id) {
        Some(device) => Ok(vec![device]),
        None => Err(anyhow!("{id} was not found on this machine")),
    }
}

fn lighting_for(
    args: &LightingArgs,
    device: &dyn Device,
    config: &Config,
) -> anyhow::Result<Lighting> {
    Ok(match args {
        LightingArgs::Off => Lighting::Off,
        LightingArgs::Mono { color } => Lighting::Mono { color: *color },
        LightingArgs::Tricolor { colors } => Lighting::Tricolor {
            colors: [colors[0], colors[1], colors[2]],
        },
        LightingArgs::Custom { regions } => {
            let mut map = custom_base(config.lights.get(device.id()), device.regions());
            for pair in regions {
                let Some((region, color)) = pair.split_once('=') else {
                    return Err(ParseError(format!("`{pair}` is not REGION=RRGGBB")).into());
                };
                map.insert(region.to_string(), color.parse()?);
            }
            Lighting::Custom { regions: map }
        }
    })
}

fn set(path: &Path, args: SetArgs) -> anyhow::Result<()> {
    let mut config = config::load(path)?;
    if args.device == "all"
        && matches!(
            args.lighting,
            LightingArgs::Custom { .. } | LightingArgs::Tricolor { .. }
        )
    {
        return Err(ParseError(
            "custom and tricolor name one device's regions; give a device, not all".into(),
        )
        .into());
    }
    let mut devices = detect();
    let mut failed = Vec::new();
    for device in pick(&mut devices, &args.device)? {
        let lighting = lighting_for(&args.lighting, device.as_ref(), &config)?;
        device::check(device.as_ref(), &lighting)?;
        match device::apply(device.as_mut(), &lighting) {
            Ok(()) => println!("{}: {}", device.id(), describe(&lighting)),
            Err(e) => failed.push(format!("{e:#}")),
        }
        config.lights.insert(device.id().to_string(), lighting);
    }
    config.on = true;
    config::save(path, &config)?;
    report(failed)
}

fn on(path: &Path) -> anyhow::Result<()> {
    let mut config = config::load(path)?;
    let (done, failed) = devctl::power_on(&mut detect(), &mut config);
    done.iter().for_each(|line| println!("{line}"));
    config::save(path, &config)?;
    report(failed)
}

fn off(path: &Path) -> anyhow::Result<()> {
    let mut config = config::load(path)?;
    let (done, failed) = devctl::power_off(&mut detect(), &mut config);
    done.iter().for_each(|line| println!("{line}"));
    config::save(path, &config)?;
    report(failed)
}

fn report(failed: Vec<String>) -> anyhow::Result<()> {
    if failed.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(failed.join("\n")))
    }
}

fn fan_cmd(path: &Path, args: FanArgs) -> anyhow::Result<()> {
    if let Some(FanAction::Daemon) = args.action {
        return fan::daemon(path);
    }
    let mut config = config::load(path)?;
    let before = config.clone();
    let gpu = &mut config.fans.gpu;
    if let Some(min) = args.min {
        gpu.min = min;
    }
    if let Some(handoff) = args.handoff {
        gpu.handoff = handoff;
    }
    if args.enable {
        gpu.enabled = true;
    }
    if args.disable {
        gpu.enabled = false;
    }
    if config != before {
        config::save(path, &config)?;
    }
    let gpu = &config.fans.gpu;
    let state = if gpu.enabled {
        "held"
    } else {
        "left to the driver"
    };
    println!(
        "GPU fans {state}: {}% below {}°C (back below {}°C)",
        gpu.min,
        gpu.handoff,
        gpu.handoff.saturating_sub(fan::HYSTERESIS)
    );
    if let Ok(nvml) = nvml_wrapper::Nvml::init()
        && let Ok(status) = fan::status(&nvml)
    {
        let speeds: Vec<String> = status.speeds.iter().map(|s| format!("{s}%")).collect();
        println!("now: {}°C, fans {}", status.temperature, speeds.join(" "));
    }
    Ok(())
}
