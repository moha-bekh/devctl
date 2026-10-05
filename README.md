# devctl

One tool for this machine's lit devices and GPU fans, speaking each device's
protocol directly (no OpenRGB).

| id      | device                               | regions                               | modes                       |
|---------|--------------------------------------|---------------------------------------|-----------------------------|
| `ram`   | 4 ENE DRAM sticks (SMBus)            | `stick1`-`stick4`, front to back      | off, mono, custom           |
| `gpu`   | Gigabyte AORUS RTX 2080 Ti (i2c)     | `card`                                | off, mono, tricolor         |
| `board` | MSI MPG X570 Gaming Pro Carbon (HID) | `jrgb1`, `jrainbow1`, `onboard1`, ... | off, mono, custom           |
| `mouse` | Cooler Master MM711 (HID)            | `wheel`, `logo`                       | off, mono, custom           |

## Usage

```
devctl                                  # terminal UI (keys and mouse)
devctl list                             # devices, regions, saved lighting
devctl set ram mono 8855FF
devctl set mouse custom wheel=FF0000 logo=00FFFF
devctl set gpu tricolor 8855FF 000000 00FFFF
devctl set all off
devctl off                              # everything off, GPU fans may stop
devctl on                               # saved lighting back, fans held again
devctl fan --min 30 --handoff 60        # GPU fans: 30% below 60°C
devctl fan --disable                    # GPU fans left to the driver
```

Every change is saved to `~/.config/devctl/config.toml`.

## Terminal UI

`devctl` with no command opens a full-terminal UI with two tabs. Everything
can be clicked, and everything can also be done from the keyboard.

- **Lights**: the devices on the left, the selected device's mode and colors
  on the right, then a palette, dimmer/brighter buttons and hex input.
- **Fans**: the live GPU temperature and fan speeds, whether the daemon is
  running, and the minimum speed and hand-off temperature.

| key        | does                                       |
|------------|--------------------------------------------|
| `Tab`      | next tab                                   |
| `←` `→`    | previous/next device (Fans: change value)  |
| `↑` `↓`    | previous/next color slot or setting        |
| `m`        | next mode                                  |
| `1`-`9`    | paint the slot with a palette color        |
| `+` `-`    | brighter/dimmer                            |
| `e`        | type a hex color (`Enter` applies)         |
| `o`        | everything on/off                          |
| `q`        | quit                                       |

## GPU fans

The GPU's RGB is dark while its fans are stopped, and the card stops them
when it is cool (zero RPM). Below the hand-off temperature, the daemon holds
the fans at the minimum speed. From the hand-off up, the driver's own curve
cools the card. The daemon takes the fans back 8°C below the hand-off. After
`devctl off`, the fans go back to the driver and may stop.

## Adding a device

Write `src/device/<name>.rs` with a `detect()` that finds the device (i2c
adapter name, or USB ids for HID) and a type implementing `Device`: an id, its
region names, and `set_colors` (one color per region, black meaning off).
Then add it to `device::detect()` and to `device::IDS`.

## Install

```
cargo install --path .
sudo cp contrib/devctl-fan.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now devctl-fan
cp contrib/devctl.service ~/.config/systemd/user/ && systemctl --user enable devctl
```

Access to `/dev/i2c-*` and `/dev/hidraw*` comes from OpenRGB's udev rules
(`uaccess`). Setting fan speeds needs root, hence the system service, which
follows the user's config file.
