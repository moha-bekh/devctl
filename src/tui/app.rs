//! The frame around the views: header with the power switch and GPU
//! numbers, tabs, the last outcome, and the key hints. Keys and clicks the
//! frame doesn't use go to the current view.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::{DefaultTerminal, Frame};

use super::state::State;
use super::widgets::{DIM, KEY, Lines, Region, button_style};
use super::{fans, lights};

const REFRESH: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(200);
const POWER: usize = 0;
const TAB: usize = 100;

/// One tab of the UI. Every action reachable by click is also a key.
pub trait View {
    fn title(&self) -> &'static str;
    fn draw(&mut self, frame: &mut Frame<'_>, area: Rect, state: &State);
    fn key(&mut self, key: KeyEvent, state: &mut State);
    /// Key hints for the footer: (key, what it does).
    fn keys(&self) -> &'static [(&'static str, &'static str)];
    /// Clickable areas, as last drawn.
    fn regions(&self) -> &[Region];
    fn click(&mut self, id: usize, state: &mut State);
    /// True while the view holds the keyboard (typing, a popup), so the
    /// frame's own keys don't fire.
    fn captures_keys(&self) -> bool {
        false
    }
}

const FRAME_KEYS: &[(&str, &str)] = &[("Tab", "next tab"), ("o", "all on/off"), ("q", "quit")];

pub struct App {
    state: State,
    views: Vec<Box<dyn View>>,
    current: usize,
    regions: Vec<Region>,
    quit: bool,
}

impl App {
    pub fn new(state: State) -> App {
        App {
            state,
            views: vec![lights::view(), fans::view()],
            current: 0,
            regions: Vec::new(),
            quit: false,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let mut refreshed = Instant::now();
        while !self.quit {
            if refreshed.elapsed() >= REFRESH {
                self.state.refresh();
                refreshed = Instant::now();
            }
            terminal.draw(|frame| self.draw(frame))?;
            if event::poll(POLL)? {
                match event::read()? {
                    Event::Key(key) if key.kind != KeyEventKind::Release => self.key(key),
                    Event::Mouse(mouse) => self.mouse(mouse),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let [header, body, status, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        self.regions.clear();

        let mut top = Lines::new(header);
        top.text(" devctl ", Style::new().add_modifier(Modifier::BOLD));
        for (i, view) in self.views.iter().enumerate() {
            top.text(" ", Style::new()).button(
                format!(" {} ", view.title()),
                button_style(i == self.current),
                TAB + i,
            );
        }
        top.text("   ", Style::new());
        let (label, style) = if self.state.config.on {
            (
                "● on ",
                Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            )
        } else {
            (
                "○ off",
                Style::new()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
        };
        top.button(label, style, POWER);
        if let Some(gpu) = &self.state.gpu {
            let speeds: Vec<String> = gpu.speeds.iter().map(|s| format!("{s}%")).collect();
            top.text(
                format!("   GPU {}°C, fans {}", gpu.temperature, speeds.join(" ")),
                DIM,
            );
        }
        self.regions.extend(top.render(frame));

        self.views[self.current].draw(frame, body, &self.state);

        let mut line = Lines::new(status);
        match &self.state.message {
            Some((text, true)) => line.text(format!(" ✗ {text}"), Style::new().fg(Color::Red)),
            Some((text, false)) => line.text(format!(" ✓ {text}"), Style::new().fg(Color::Green)),
            None => line.text("", Style::new()),
        };
        line.render(frame);

        let mut hints = Lines::new(footer);
        for (key, what) in self.views[self.current].keys().iter().chain(FRAME_KEYS) {
            hints
                .span(Span::styled(format!(" {key}"), KEY), None)
                .text(format!(" {what} "), DIM);
        }
        hints.render(frame);
    }

    fn key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        let view = &mut self.views[self.current];
        if view.captures_keys() {
            view.key(key, &mut self.state);
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Tab => self.current = (self.current + 1) % self.views.len(),
            KeyCode::BackTab => {
                self.current = (self.current + self.views.len() - 1) % self.views.len()
            }
            KeyCode::Char('o') => self.state.toggle_power(),
            _ => view.key(key, &mut self.state),
        }
    }

    fn mouse(&mut self, mouse: MouseEvent) {
        let (column, row) = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(region) = self.regions.iter().find(|r| r.contains(column, row)) {
                    match region.id {
                        POWER => self.state.toggle_power(),
                        id => self.current = id - TAB,
                    }
                    return;
                }
                let view = &mut self.views[self.current];
                if let Some(id) = view
                    .regions()
                    .iter()
                    .rev()
                    .find(|r| r.contains(column, row))
                    .map(|r| r.id)
                {
                    view.click(id, &mut self.state);
                }
            }
            MouseEventKind::ScrollUp => {
                self.views[self.current].key(KeyEvent::from(KeyCode::Up), &mut self.state)
            }
            MouseEventKind::ScrollDown => {
                self.views[self.current].key(KeyEvent::from(KeyCode::Down), &mut self.state)
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::color::Rgb;
    use crate::config::{Config, Lighting};
    use crate::device::Device;

    struct Fake {
        regions: Vec<String>,
        shown: std::rc::Rc<std::cell::RefCell<Vec<Rgb>>>,
    }

    impl Device for Fake {
        fn id(&self) -> &'static str {
            "mouse"
        }
        fn name(&self) -> String {
            "Fake mouse".into()
        }
        fn regions(&self) -> &[String] {
            &self.regions
        }
        fn set_colors(&mut self, colors: &[Rgb]) -> anyhow::Result<()> {
            *self.shown.borrow_mut() = colors.to_vec();
            Ok(())
        }
    }

    fn app() -> (App, std::rc::Rc<std::cell::RefCell<Vec<Rgb>>>) {
        let shown = std::rc::Rc::default();
        let device = Fake {
            regions: vec!["wheel".into(), "logo".into()],
            shown: std::rc::Rc::clone(&shown),
        };
        let mut config = Config::default();
        config.lights.insert(
            "mouse".into(),
            Lighting::Mono {
                color: Rgb(0x88, 0x55, 0xFF),
            },
        );
        (App::new(State::fake(vec![Box::new(device)], config)), shown)
    }

    fn render(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(110, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn click(app: &mut App, needle: &str) {
        let screen = render(app);
        let (row, line) = screen
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains(needle))
            .expect(needle);
        let column = line[..line.find(needle).unwrap()].chars().count() as u16;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row: row as u16,
            modifiers: KeyModifiers::NONE,
        });
    }

    #[test]
    fn clicking_custom_then_a_palette_color_paints_one_region() {
        let (mut app, shown) = app();
        click(&mut app, " custom ");
        click(&mut app, "logo");
        // The first palette swatch is red.
        let screen = render(&mut app);
        let row = screen.lines().position(|l| l.contains(" Color ")).unwrap();
        let line = screen.lines().nth(row).unwrap();
        let column = line[..line.find(" Color ").unwrap()].chars().count() as u16 + 8;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row: row as u16,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            *shown.borrow(),
            vec![Rgb(0x88, 0x55, 0xFF), Rgb(0xFF, 0, 0)]
        );
    }

    #[test]
    fn clicking_a_color_opens_the_wheel_and_a_click_on_it_paints() {
        let (mut app, shown) = app();
        // The swatch sits after the selection marker.
        let screen = render(&mut app);
        let (row, line) = screen
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("#8855FF"))
            .unwrap();
        let column = line[..line.find("██████").unwrap()].chars().count() as u16;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row: row as u16,
            modifiers: KeyModifiers::NONE,
        });
        assert!(render(&mut app).contains("Color wheel"));
        // The first wheel cell is the top of the circle: a bright blue-ish
        // hue, not the purple the slot started from.
        let wheel = app.views[0]
            .regions()
            .iter()
            .find(|r| r.id >= super::super::wheel::WHEEL)
            .unwrap()
            .rect;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: wheel.x,
            row: wheel.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_ne!(*shown.borrow(), Vec::<Rgb>::new());
        assert_ne!(shown.borrow()[0], Rgb(0x88, 0x55, 0xFF));
        app.key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(shown.borrow()[0], Rgb(0x88, 0x55, 0xFF));
        assert!(!render(&mut app).contains("Color wheel"));
    }

    #[test]
    fn the_fans_tab_changes_the_minimum_by_click() {
        let (mut app, _) = app();
        click(&mut app, " Fans ");
        let screen = render(&mut app);
        let (row, line) = screen
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("minimum"))
            .unwrap();
        let plus = line.rfind(" + ").unwrap();
        let column = line[..plus].chars().count() as u16 + 1;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row: row as u16,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.state.config.fans.gpu.min, 35);
    }
}
