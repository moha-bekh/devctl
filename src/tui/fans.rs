//! The fans tab: whether the GPU fans are held at a minimum, at which speed
//! and up to which temperature. The root daemon applies what is saved here.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Block;

use super::app::View;
use super::state::State;
use super::widgets::{DIM, Lines, Region, button_style};
use crate::fan::HYSTERESIS;

const ROWS: usize = 3;
const ROW: usize = 0;
const DOWN: usize = 10;
const UP: usize = 20;
const MIN_STEP: u32 = 5;

#[derive(Default)]
struct Fans {
    row: usize,
    regions: Vec<Region>,
}

pub fn view() -> Box<dyn View> {
    Box::<Fans>::default()
}

impl Fans {
    /// Moves the value on `row` one step up or down (or flips the switch).
    fn adjust(&mut self, state: &mut State, row: usize, up: bool) {
        let gpu = &mut state.config.fans.gpu;
        match row {
            0 => gpu.enabled = !gpu.enabled,
            1 => {
                gpu.min = if up {
                    (gpu.min + MIN_STEP).min(100)
                } else {
                    gpu.min.saturating_sub(MIN_STEP)
                }
            }
            _ => {
                gpu.handoff = if up {
                    (gpu.handoff + 1).min(90)
                } else {
                    gpu.handoff.saturating_sub(1).max(30)
                }
            }
        }
        let gpu = &state.config.fans.gpu;
        let summary = format!(
            "GPU fans {}: {}% below {}°C",
            if gpu.enabled {
                "held"
            } else {
                "left to the driver"
            },
            gpu.min,
            gpu.handoff
        );
        state.message = Some((summary, false));
        state.save();
    }
}

impl View for Fans {
    fn title(&self) -> &'static str {
        "Fans"
    }

    fn draw(&mut self, frame: &mut Frame<'_>, area: Rect, state: &State) {
        let block = Block::bordered().title(" GPU fans ");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let mut out = Lines::new(inner);
        let gpu = &state.config.fans.gpu;

        match &state.gpu {
            Some(status) => {
                let speeds: Vec<String> = status.speeds.iter().map(|s| format!("{s}%")).collect();
                out.text(
                    format!(
                        " Now      {}°C, fans {}",
                        status.temperature,
                        speeds.join(" ")
                    ),
                    Style::new(),
                )
            }
            None => out.text(" Now      NVML unavailable", DIM),
        };
        out.end();
        match state.daemon {
            Some(true) => out.text(
                " Daemon   devctl-fan.service is running",
                Style::new().fg(Color::Green),
            ),
            _ => out.text(
                " Daemon   devctl-fan.service is not running: nothing below is applied",
                Style::new().fg(Color::Yellow),
            ),
        };
        out.end().end();

        let rows = [
            (
                format!(
                    "[{}] hold the fans while the lights are on",
                    if gpu.enabled { "x" } else { " " }
                ),
                None,
            ),
            (
                format!("minimum  {:>3}%", gpu.min),
                Some("speed while the GPU is cool"),
            ),
            (
                format!("hand-off {:>3}°C", gpu.handoff),
                Some("from here up, the driver's own curve"),
            ),
        ];
        for (i, (label, help)) in rows.iter().enumerate() {
            let selected = i == self.row;
            let style = if selected {
                Style::new().add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            };
            out.button(
                format!(" {} ", if selected { "▶" } else { " " }),
                style,
                ROW + i,
            );
            if i > 0 {
                out.button(" − ", button_style(false), DOWN + i)
                    .text(" ", Style::new());
            }
            out.button(label.clone(), style, ROW + i);
            if i > 0 {
                out.text(" ", Style::new())
                    .button(" + ", button_style(false), UP + i);
            }
            if let Some(help) = help {
                out.text(format!("   {help}"), DIM);
            }
            out.end();
        }
        out.end().text(
            format!(
                " Back to the minimum below {}°C. With the lights off (o), the fans may stop.",
                gpu.handoff.saturating_sub(HYSTERESIS)
            ),
            DIM,
        );
        self.regions = out.render(frame);
    }

    fn key(&mut self, key: KeyEvent, state: &mut State) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.row = self.row.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.row = (self.row + 1).min(ROWS - 1),
            KeyCode::Left | KeyCode::Char('-') | KeyCode::Char('h') => {
                self.adjust(state, self.row, false)
            }
            KeyCode::Right | KeyCode::Char('+') | KeyCode::Char('l') => {
                self.adjust(state, self.row, true)
            }
            KeyCode::Char(' ') | KeyCode::Enter if self.row == 0 => self.adjust(state, 0, true),
            _ => {}
        }
    }

    fn keys(&self) -> &'static [(&'static str, &'static str)] {
        &[("↑↓", "setting"), ("←→", "change"), ("space", "toggle")]
    }

    fn regions(&self) -> &[Region] {
        &self.regions
    }

    fn click(&mut self, id: usize, state: &mut State) {
        match id {
            ROW..DOWN => {
                self.row = id - ROW;
                if self.row == 0 {
                    self.adjust(state, 0, true);
                }
            }
            DOWN..UP => self.adjust(state, id - DOWN, false),
            _ => self.adjust(state, id - UP, true),
        }
    }
}
