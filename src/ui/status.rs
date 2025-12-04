use super::theme::*;
use crate::app::{App, AppMode};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let workspace_name = app
        .config
        .workspace_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let mode_str = match app.mode {
        AppMode::Chat => "CHAT",
        AppMode::Terminal => "TERM",
        AppMode::ModelSelector => "MENU",
    };

    let spinner = if app.is_processing {
        SPINNER[app.spinner_frame % SPINNER.len()]
    } else {
        " "
    };

    let left_text = vec![
        Span::styled(
            format!(" agerus v0.1.0 "),
            Style::default().fg(FG_SECONDARY).bg(Color::Rgb(20, 20, 20)),
        ),
        Span::styled(
            format!(" {} ", workspace_name),
            Style::default().fg(FG_PRIMARY),
        ),
        Span::styled(
            format!(" {} ", mode_str),
            Style::default()
                .fg(BG_MAIN)
                .bg(ACCENT_BLUE)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let right_text = vec![
        Span::styled(format!(" {} ", spinner), Style::default().fg(ACCENT_ORANGE)),
        Span::styled(" tab: switch view ", Style::default().fg(FG_SECONDARY)),
        Span::styled(" ctrl+p: model ", Style::default().fg(FG_SECONDARY)),
    ];

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Min(0)])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(left_text)).alignment(Alignment::Left),
        layout[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(right_text)).alignment(Alignment::Right),
        layout[1],
    );
}
