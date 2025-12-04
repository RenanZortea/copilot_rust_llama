use super::theme::*;
use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize}, // <--- Added Stylize
    widgets::{Block, Borders, Clear, List, ListItem},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Select Model ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_BLUE))
        .bg(BG_MAIN);

    let area = centered_rect(50, 40, area);
    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    let items: Vec<ListItem> = app
        .available_models
        .iter()
        .map(|m| {
            let is_current = *m == app.config.model;
            let style = if is_current {
                Style::default()
                    .fg(ACCENT_ORANGE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG_PRIMARY)
            };

            let prefix = if is_current { "● " } else { "  " };
            ListItem::new(format!("{}{}", prefix, m)).style(style)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::Rgb(20, 20, 20)))
        .highlight_symbol(" ");

    let mut state = app.model_list_state.clone();
    f.render_stateful_widget(list, inner, &mut state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
