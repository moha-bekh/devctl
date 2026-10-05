//! Building blocks shared by the views: lines whose spans can be clicked,
//! and color swatches.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::color::Rgb;

/// A clickable area, as last drawn, and the id its view gave it.
#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub rect: Rect,
    pub id: usize,
}

impl Region {
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.rect.x <= column
            && column < self.rect.right()
            && self.rect.y <= row
            && row < self.rect.bottom()
    }
}

/// Lines of text drawn into `area`, recording where each clickable span
/// lands so a click can be traced back to it.
pub struct Lines {
    area: Rect,
    lines: Vec<Line<'static>>,
    current: Vec<Span<'static>>,
    x: u16,
    regions: Vec<Region>,
}

impl Lines {
    pub fn new(area: Rect) -> Lines {
        Lines {
            area,
            lines: Vec::new(),
            current: Vec::new(),
            x: 0,
            regions: Vec::new(),
        }
    }

    pub fn text(&mut self, text: impl Into<String>, style: Style) -> &mut Lines {
        self.push(text.into(), style, None)
    }

    pub fn button(&mut self, text: impl Into<String>, style: Style, id: usize) -> &mut Lines {
        self.push(text.into(), style, Some(id))
    }

    fn push(&mut self, text: String, style: Style, id: Option<usize>) -> &mut Lines {
        self.span(Span::styled(text, style), id)
    }

    /// Any span, clickable when given an id.
    pub fn span(&mut self, span: Span<'static>, id: Option<usize>) -> &mut Lines {
        let width = span.content.chars().count() as u16;
        let y = self.area.y + self.lines.len() as u16;
        if let Some(id) = id
            && y < self.area.bottom()
            && self.x < self.area.width
        {
            let rect = Rect::new(
                self.area.x + self.x,
                y,
                width.min(self.area.width - self.x),
                1,
            );
            self.regions.push(Region { rect, id });
        }
        self.x += width;
        self.current.push(span);
        self
    }

    pub fn end(&mut self) -> &mut Lines {
        self.lines
            .push(Line::from(std::mem::take(&mut self.current)));
        self.x = 0;
        self
    }

    /// Lines still free below what was written.
    pub fn room(&self) -> usize {
        (self.area.height as usize).saturating_sub(self.lines.len())
    }

    pub fn render(mut self, frame: &mut Frame<'_>) -> Vec<Region> {
        if !self.current.is_empty() {
            self.end();
        }
        frame.render_widget(Paragraph::new(self.lines), self.area);
        self.regions
    }
}

/// A block of `color`; off (black) shows as a dim pattern instead of nothing.
pub fn swatch(color: Rgb, width: usize) -> Span<'static> {
    if color.is_black() {
        Span::styled("░".repeat(width), Style::new().fg(Color::DarkGray))
    } else {
        Span::styled(
            "█".repeat(width),
            Style::new().fg(Color::Rgb(color.0, color.1, color.2)),
        )
    }
}

pub fn button_style(active: bool) -> Style {
    if active {
        Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::new()
    }
}

pub const KEY: Style = Style::new().add_modifier(Modifier::BOLD);
pub const DIM: Style = Style::new().fg(Color::DarkGray);
