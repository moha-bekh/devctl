//! The full-terminal UI: one tab per concern (lights, fans), every control
//! reachable by key and by click. Built like omniscient's view: a `View`
//! trait, regions recorded while drawing, and a frame that routes clicks to
//! the region under the mouse.

mod app;
mod fans;
mod lights;
mod state;
mod widgets;

use std::io::stdout;
use std::path::Path;

use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

pub fn run(path: &Path) -> anyhow::Result<()> {
    let state = state::State::load(path)?;
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture)?;
    let result = app::App::new(state).run(&mut terminal);
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
