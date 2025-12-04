use super::theme::*;
use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize}, // <--- Added Stylize
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    // Center the input bar with some margin
    let centered = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(5),
            Constraint::Percentage(90),
            Constraint::Percentage(5),
        ])
        .split(area)[1];

    let block = Block::default()
        .bg(BG_INPUT)
        .padding(Padding::new(2, 2, 1, 1)); // Internal padding

    f.render_widget(block, centered);

    // Draw text inside
    let inner = centered.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });

    // If input is empty, show placeholder
    let text = if app.input_buffer.is_empty() {
        Line::from(Span::styled(
            "Ask Agerus anything...",
            Style::default().fg(FG_SECONDARY),
        ))
    } else {
        Line::from(vec![
            Span::styled(&app.input_buffer, Style::default().fg(FG_PRIMARY)),
            Span::styled("█", Style::default().fg(ACCENT_ORANGE)), // Cursor
        ])
    };

    f.render_widget(Paragraph::new(text), inner);

    // Decoration line
    let decoration_area = Rect {
        x: centered.x,
        y: centered.y + 1,
        width: 1,
        height: 2,
    };
    f.render_widget(Block::default().bg(ACCENT_BLUE), decoration_area);
}
