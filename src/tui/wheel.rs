//! A color wheel popup: hue around the circle, saturation outward from the
//! center, and a bar for the value (lightness). Drawn with half blocks, so
//! each cell holds two pixels and the circle comes out round.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Clear};

use super::widgets::{DIM, Lines, Region, button_style, swatch};
use crate::color::Rgb;

/// Region ids: one per wheel cell, one per value bar row, then the buttons.
pub const WHEEL: usize = 10_000;
pub const VALUE: usize = 20_000;
pub const KEEP: usize = 30_000;
pub const CANCEL: usize = 30_001;

const INFO_WIDTH: u16 = 27;
const HUE_STEP: f32 = 10.0;
const STEP: f32 = 0.05;

/// What a key or click did to the wheel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// The color changed; show it.
    Changed,
    /// Close, keeping the current color.
    Keep,
    /// Close, going back to the color the wheel opened with.
    Cancel,
}

pub struct Wheel {
    h: f32,
    s: f32,
    v: f32,
    pub original: Rgb,
    /// In pixels, as last drawn: clicks are decoded against it.
    radius: i32,
}

impl Wheel {
    pub fn new(color: Rgb) -> Wheel {
        let (h, s, v) = color.to_hsv();
        // Off (black) opens at full value, or the wheel would be all black.
        let v = if v == 0.0 { 1.0 } else { v };
        Wheel {
            h,
            s,
            v,
            original: color,
            radius: 10,
        }
    }

    pub fn color(&self) -> Rgb {
        Rgb::from_hsv(self.h, self.s, self.v)
    }

    fn rows(&self) -> i32 {
        self.radius + 1
    }

    fn diameter(&self) -> i32 {
        2 * self.radius + 1
    }

    /// The color of the wheel pixel at (`x`, `y`) from its center, if inside.
    fn pixel(&self, x: f32, y: f32) -> Option<Rgb> {
        let distance = (x * x + y * y).sqrt() / self.radius as f32;
        (distance <= 1.0).then(|| Rgb::from_hsv((-y).atan2(x).to_degrees(), distance, self.v))
    }

    pub fn draw(&mut self, frame: &mut Frame<'_>, area: Rect) -> Vec<Region> {
        let by_height = area.height as i32 - 3;
        let by_width = (area.width as i32 - INFO_WIDTH as i32 - 9) / 2;
        self.radius = by_height.min(by_width).clamp(4, 14);
        let (diameter, rows) = (self.diameter(), self.rows());

        let width = (diameter as u16 + 5 + INFO_WIDTH + 2).min(area.width);
        let height = (rows.max(9) as u16 + 2).min(area.height);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);
        let block = Block::bordered().title(" Color wheel ");
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let mut regions = Vec::new();

        let rgb = |c: Rgb| Color::Rgb(c.0, c.1, c.2);
        let buffer = frame.buffer_mut();
        for row in 0..rows {
            for column in 0..diameter {
                let (x, y) = (inner.x + column as u16, inner.y + row as u16);
                if x >= inner.right() || y >= inner.bottom() {
                    continue;
                }
                let px = (column - self.radius) as f32;
                let top = self.pixel(px, (2 * row - self.radius) as f32);
                let bottom = self.pixel(px, (2 * row + 1 - self.radius) as f32);
                let cell = &mut buffer[(x, y)];
                match (top, bottom) {
                    (Some(t), Some(b)) => cell.set_symbol("▀").set_fg(rgb(t)).set_bg(rgb(b)),
                    (Some(t), None) => cell.set_symbol("▀").set_fg(rgb(t)).set_bg(Color::Reset),
                    (None, Some(b)) => cell.set_symbol("▄").set_fg(rgb(b)).set_bg(Color::Reset),
                    (None, None) => continue,
                };
                let rect = Rect::new(x, y, 1, 1);
                regions.push(Region {
                    rect,
                    id: WHEEL + (row * diameter + column) as usize,
                });
            }
        }

        // The cursor, on the cell nearest the current hue and saturation.
        let angle = self.h.to_radians();
        let reach = self.s * self.radius as f32;
        let column = (reach * angle.cos()).round() as i32 + self.radius;
        let row = ((-reach * angle.sin()).round() as i32 + self.radius) / 2;
        let (x, y) = (inner.x + column as u16, inner.y + row as u16);
        if x < inner.right() && y < inner.bottom() {
            let mark = if self.v > 0.5 {
                Color::Black
            } else {
                Color::White
            };
            buffer[(x, y)]
                .set_symbol("●")
                .set_fg(mark)
                .set_bg(rgb(self.color()));
        }

        // The value bar: the same hue and saturation, from full to black.
        let bar = inner.x + diameter as u16 + 2;
        let marked = ((1.0 - self.v) * (rows - 1) as f32).round() as i32;
        for row in 0..rows {
            let y = inner.y + row as u16;
            if bar + 2 >= inner.right() || y >= inner.bottom() {
                break;
            }
            let value = 1.0 - row as f32 / (rows - 1) as f32;
            let color = rgb(Rgb::from_hsv(self.h, self.s, value));
            buffer[(bar, y)].set_symbol("█").set_fg(color);
            buffer[(bar + 1, y)].set_symbol("█").set_fg(color);
            if row == marked {
                buffer[(bar + 2, y)].set_symbol("◀").set_style(Style::new());
            }
            regions.push(Region {
                rect: Rect::new(bar, y, 2, 1),
                id: VALUE + row as usize,
            });
        }

        let info = Rect::new(
            bar + 4,
            inner.y,
            inner.right().saturating_sub(bar + 4),
            inner.height,
        );
        let color = self.color();
        let mut out = Lines::new(info);
        out.span(swatch(color, 10), None).end();
        out.span(swatch(color, 10), None).end();
        out.text(format!("#{color}"), Style::new()).end();
        out.text(
            format!(
                "H {:>3.0}°  S {:>3.0}%  V {:>3.0}%",
                self.h,
                self.s * 100.0,
                self.v * 100.0
            ),
            DIM,
        )
        .end()
        .end();
        out.button(" keep ", button_style(true), KEEP)
            .text(" ", Style::new());
        out.button(" cancel ", button_style(false), CANCEL)
            .end()
            .end();
        out.text("click the wheel or the bar", DIM).end();
        regions.extend(out.render(frame));
        regions
    }

    pub fn click(&mut self, id: usize) -> Outcome {
        match id {
            WHEEL..VALUE => {
                let index = (id - WHEEL) as i32;
                let (row, column) = (index / self.diameter(), index % self.diameter());
                let x = (column - self.radius) as f32;
                let y = 2.0 * row as f32 + 0.5 - self.radius as f32;
                self.h = (-y).atan2(x).to_degrees().rem_euclid(360.0);
                self.s = ((x * x + y * y).sqrt() / self.radius as f32).min(1.0);
                Outcome::Changed
            }
            VALUE..KEEP => {
                let row = (id - VALUE) as f32;
                self.v = 1.0 - row / (self.rows() - 1) as f32;
                Outcome::Changed
            }
            KEEP => Outcome::Keep,
            CANCEL => Outcome::Cancel,
            _ => Outcome::None,
        }
    }

    pub fn key(&mut self, code: KeyCode) -> Outcome {
        match code {
            KeyCode::Left | KeyCode::Char('h') => self.h = (self.h - HUE_STEP).rem_euclid(360.0),
            KeyCode::Right | KeyCode::Char('l') => self.h = (self.h + HUE_STEP).rem_euclid(360.0),
            KeyCode::Up | KeyCode::Char('k') => self.s = (self.s + STEP).min(1.0),
            KeyCode::Down | KeyCode::Char('j') => self.s = (self.s - STEP).max(0.0),
            KeyCode::Char('+') | KeyCode::Char('=') => self.v = (self.v + STEP).min(1.0),
            KeyCode::Char('-') => self.v = (self.v - STEP).max(0.0),
            KeyCode::Enter => return Outcome::Keep,
            KeyCode::Esc => return Outcome::Cancel,
            _ => return Outcome::None,
        }
        Outcome::Changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_opens_on_the_color_it_was_given() {
        let wheel = Wheel::new(Rgb(0x88, 0x55, 0xFF));
        assert_eq!(wheel.color(), Rgb(0x88, 0x55, 0xFF));
    }

    #[test]
    fn a_click_on_the_rim_to_the_right_is_pure_red() {
        let mut wheel = Wheel::new(Rgb::WHITE);
        let (row, column) = (wheel.radius / 2, wheel.diameter() - 1);
        assert_eq!(
            wheel.click(WHEEL + (row * wheel.diameter() + column) as usize),
            Outcome::Changed
        );
        let Rgb(r, g, b) = wheel.color();
        assert!(r == 255 && g < 40 && b < 40, "{:?}", wheel.color());
    }

    #[test]
    fn the_bottom_of_the_value_bar_is_black() {
        let mut wheel = Wheel::new(Rgb(0, 0xFF, 0));
        wheel.click(VALUE + (wheel.rows() - 1) as usize);
        assert_eq!(wheel.color(), Rgb::BLACK);
    }
}
