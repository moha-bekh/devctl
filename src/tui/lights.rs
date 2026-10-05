//! The lights tab: the devices on the left, the selected one's mode and
//! colors on the right, and a palette to paint the selected slot.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Block;

use super::app::View;
use super::state::State;
use super::widgets::{DIM, Lines, Region, button_style, swatch};
use crate::color::Rgb;
use crate::config::{DEFAULT_TRICOLOR, Lighting};
use crate::custom_base;

const DEVICE: usize = 0;
const MODE: usize = 100;
const SLOT: usize = 200;
const PALETTE: usize = 400;
const DIMMER: usize = 500;
const BRIGHTER: usize = 501;
const HEX: usize = 502;

const PALETTE_COLORS: [Rgb; 16] = [
    Rgb(0xFF, 0x00, 0x00),
    Rgb(0xFF, 0x60, 0x00),
    Rgb(0xFF, 0xC0, 0x00),
    Rgb(0x00, 0xFF, 0x00),
    Rgb(0x00, 0xFF, 0x80),
    Rgb(0x00, 0xFF, 0xFF),
    Rgb(0x00, 0x80, 0xFF),
    Rgb(0x00, 0x00, 0xFF),
    Rgb(0x44, 0x2A, 0x80),
    Rgb(0x88, 0x55, 0xFF),
    Rgb(0x80, 0x00, 0xFF),
    Rgb(0xFF, 0x00, 0xFF),
    Rgb(0xFF, 0x00, 0x80),
    Rgb(0xFF, 0xFF, 0xFF),
    Rgb(0x80, 0x80, 0x80),
    Rgb::BLACK,
];
const BRIGHTER_BY: f32 = 1.15;

#[derive(Default)]
struct Lights {
    device: usize,
    slot: usize,
    /// The hex color being typed, while typing.
    hex: Option<String>,
    regions: Vec<Region>,
}

pub fn view() -> Box<dyn View> {
    Box::<Lights>::default()
}

/// The modes `device` offers, in display order.
fn modes(state: &State, device: usize) -> Vec<&'static str> {
    let device = &state.devices[device];
    Lighting::MODES
        .into_iter()
        .filter(|mode| match *mode {
            "custom" => device.regions().len() > 1,
            "tricolor" => device.supports_tricolor(),
            _ => true,
        })
        .collect()
}

/// The colors the current mode lets the user paint, with their labels.
fn slots(state: &State, device: usize) -> Vec<(String, Rgb)> {
    match state.lighting(device) {
        None | Some(Lighting::Off) => Vec::new(),
        Some(Lighting::Mono { color }) => vec![("color".into(), *color)],
        Some(Lighting::Custom { regions }) => state.devices[device]
            .regions()
            .iter()
            .map(|r| (r.clone(), regions.get(r).copied().unwrap_or_default()))
            .collect(),
        Some(Lighting::Tricolor { colors }) => colors
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("color {}", i + 1), *c))
            .collect(),
    }
}

/// The color a device shows first, to carry over when switching mode.
fn main_color(lighting: Option<&Lighting>) -> Rgb {
    let color = match lighting {
        Some(Lighting::Mono { color }) => *color,
        Some(Lighting::Custom { regions }) => regions
            .values()
            .copied()
            .find(|c| !c.is_black())
            .unwrap_or_default(),
        Some(Lighting::Tricolor { colors }) => colors[0],
        _ => Rgb::BLACK,
    };
    if color.is_black() { Rgb::WHITE } else { color }
}

impl Lights {
    fn set_mode(&mut self, state: &mut State, mode: &str) {
        let current = state.lighting(self.device);
        if current.map(Lighting::mode) == Some(mode) {
            return;
        }
        let regions = state.devices[self.device].regions().to_vec();
        let lighting = match mode {
            "off" => Lighting::Off,
            "mono" => Lighting::Mono {
                color: main_color(current),
            },
            "custom" => Lighting::Custom {
                regions: custom_base(current, &regions),
            },
            _ => Lighting::Tricolor {
                colors: DEFAULT_TRICOLOR,
            },
        };
        self.slot = 0;
        state.set_lighting(self.device, lighting);
    }

    fn next_mode(&mut self, state: &mut State) {
        let modes = modes(state, self.device);
        let current = state
            .lighting(self.device)
            .map(Lighting::mode)
            .unwrap_or("off");
        let at = modes.iter().position(|m| *m == current).unwrap_or(0);
        self.set_mode(state, modes[(at + 1) % modes.len()]);
    }

    /// Paints the selected slot with `f(its current color)`.
    fn paint(&mut self, state: &mut State, f: impl Fn(Rgb) -> Rgb) {
        let Some(lighting) = state.lighting(self.device).cloned() else {
            return;
        };
        let regions = state.devices[self.device].regions().to_vec();
        let lighting = match lighting {
            Lighting::Off => return,
            Lighting::Mono { color } => Lighting::Mono { color: f(color) },
            Lighting::Custom { regions: mut map } => {
                let Some(name) = regions.get(self.slot) else {
                    return;
                };
                let color = map.get(name).copied().unwrap_or_default();
                map.insert(name.clone(), f(color));
                Lighting::Custom { regions: map }
            }
            Lighting::Tricolor { mut colors } => {
                let Some(color) = colors.get_mut(self.slot) else {
                    return;
                };
                *color = f(*color);
                Lighting::Tricolor { colors }
            }
        };
        state.set_lighting(self.device, lighting);
    }

    fn select_device(&mut self, state: &State, device: usize) {
        if device < state.devices.len() {
            self.device = device;
            self.slot = 0;
        }
    }
}

impl View for Lights {
    fn title(&self) -> &'static str {
        "Lights"
    }

    fn draw(&mut self, frame: &mut Frame<'_>, area: Rect, state: &State) {
        self.regions.clear();
        let [left, right] =
            Layout::horizontal([Constraint::Length(30), Constraint::Min(0)]).areas(area);

        let block = Block::bordered().title(" Devices ");
        let inner = block.inner(left);
        frame.render_widget(block, left);
        let mut list = Lines::new(inner);
        for (i, device) in state.devices.iter().enumerate() {
            let selected = i == self.device;
            let lighting = state.lighting(i);
            let label = format!("{} {:<6}", if selected { "▶" } else { " " }, device.id());
            let style = if selected {
                Style::new().add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            };
            list.button(label, style, DEVICE + i);
            let colors = slots(state, i);
            for (_, color) in colors.iter().take(4) {
                list.span(swatch(*color, 2), Some(DEVICE + i));
            }
            if colors.is_empty() {
                list.text(lighting.map(Lighting::mode).unwrap_or("not set"), DIM);
            }
            list.end();
        }
        if state.devices.is_empty() {
            list.text(" no device found", DIM).end();
        }
        self.regions.extend(list.render(frame));

        let Some(device) = state.devices.get(self.device) else {
            return;
        };
        let block = Block::bordered().title(format!(" {} ", device.name()));
        let inner = block.inner(right);
        frame.render_widget(block, right);
        let mut out = Lines::new(inner);

        let current = state.lighting(self.device).map(Lighting::mode);
        out.text(" Mode   ", DIM);
        for (i, mode) in modes(state, self.device).into_iter().enumerate() {
            out.button(
                format!(" {mode} "),
                button_style(current == Some(mode)),
                MODE + i,
            )
            .text(" ", Style::new());
        }
        out.end().end();

        let slots = slots(state, self.device);
        if slots.is_empty() {
            let why = if current.is_none() {
                "nothing saved yet: pick a mode"
            } else {
                "off"
            };
            out.text(format!(" {why}"), DIM).end();
        }
        // Keep the selected slot in view when there are more than fit.
        // The palette below takes 4 lines.
        let room = out.room().saturating_sub(4).max(1);
        let first = self
            .slot
            .saturating_sub(room - 1)
            .min(slots.len().saturating_sub(room));
        for (i, (label, color)) in slots.iter().enumerate().skip(first).take(room) {
            let selected = i == self.slot;
            let style = if selected {
                Style::new().add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            };
            out.button(
                format!(" {} ", if selected { "▶" } else { " " }),
                style,
                SLOT + i,
            )
            .span(swatch(*color, 6), Some(SLOT + i))
            .button(format!("  {label:<16}"), style, SLOT + i)
            .text(format!("#{color}"), DIM)
            .end();
        }

        if !slots.is_empty() {
            out.end().text(" Color  ", DIM);
            for (i, color) in PALETTE_COLORS.iter().enumerate() {
                out.span(swatch(*color, 2), Some(PALETTE + i))
                    .text(" ", Style::new());
            }
            out.end().end().text("        ", Style::new());
            out.button(" − dimmer ", button_style(false), DIMMER)
                .text(" ", Style::new());
            out.button(" + brighter ", button_style(false), BRIGHTER)
                .text(" ", Style::new());
            match &self.hex {
                Some(text) => out.button(format!(" #{text:_<6} "), button_style(true), HEX),
                None => out.button(" #hex ", button_style(false), HEX),
            };
            out.end();
        }
        self.regions.extend(out.render(frame));
    }

    fn key(&mut self, key: KeyEvent, state: &mut State) {
        if let Some(text) = &mut self.hex {
            match key.code {
                KeyCode::Char(c) if c.is_ascii_hexdigit() && text.len() < 6 => {
                    text.push(c.to_ascii_uppercase())
                }
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Enter => {
                    if let Ok(color) = text.parse::<Rgb>() {
                        self.hex = None;
                        self.paint(state, |_| color);
                    }
                }
                KeyCode::Esc => self.hex = None,
                _ => {}
            }
            return;
        }
        let count = slots(state, self.device).len();
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                let n = state.devices.len();
                if n > 0 {
                    self.select_device(state, (self.device + n - 1) % n);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let n = state.devices.len();
                if n > 0 {
                    self.select_device(state, (self.device + 1) % n);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.slot = self.slot.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.slot = (self.slot + 1).min(count.saturating_sub(1))
            }
            KeyCode::Char('m') => self.next_mode(state),
            KeyCode::Char('+') | KeyCode::Char('=') => self.paint(state, |c| c.scale(BRIGHTER_BY)),
            KeyCode::Char('-') => self.paint(state, |c| c.scale(1.0 / BRIGHTER_BY)),
            KeyCode::Char('#') | KeyCode::Char('e') if count > 0 => self.hex = Some(String::new()),
            KeyCode::Char(c @ '1'..='9') => {
                if let Some(color) = PALETTE_COLORS.get(c as usize - '1' as usize) {
                    let color = *color;
                    self.paint(state, |_| color);
                }
            }
            _ => {}
        }
    }

    fn keys(&self) -> &'static [(&'static str, &'static str)] {
        if self.hex.is_some() {
            &[("Enter", "apply"), ("Esc", "cancel")]
        } else {
            &[
                ("←→", "device"),
                ("↑↓", "slot"),
                ("m", "mode"),
                ("1-9", "palette"),
                ("+/-", "brightness"),
                ("e", "hex"),
            ]
        }
    }

    fn regions(&self) -> &[Region] {
        &self.regions
    }

    fn takes_text(&self) -> bool {
        self.hex.is_some()
    }

    fn click(&mut self, id: usize, state: &mut State) {
        match id {
            DEVICE..MODE => self.select_device(state, id - DEVICE),
            MODE..SLOT => {
                if let Some(mode) = modes(state, self.device).get(id - MODE) {
                    self.set_mode(state, mode);
                }
            }
            SLOT..PALETTE => self.slot = id - SLOT,
            PALETTE..DIMMER => {
                let color = PALETTE_COLORS[id - PALETTE];
                self.paint(state, |_| color);
            }
            DIMMER => self.paint(state, |c| c.scale(1.0 / BRIGHTER_BY)),
            BRIGHTER => self.paint(state, |c| c.scale(BRIGHTER_BY)),
            HEX => {
                self.hex = if self.hex.is_some() {
                    None
                } else {
                    Some(String::new())
                }
            }
            _ => {}
        }
    }
}
