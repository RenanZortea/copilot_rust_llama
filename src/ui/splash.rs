use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span, Text}, // <--- Added Text
    widgets::Paragraph,
    Frame,
};
use super::theme::*;

pub fn draw(f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(8), // Logo
            Constraint::Length(1), // Spacer
            Constraint::Length(10), // Commands
            Constraint::Min(1),
        ])
        .split(area);

    // ASCII Art Logo
    // We use a raw string literal (r#...#) so we don't have to escape backslashes
    let logo_str = r#"
             █████╗  ██████╗ ███████╗██████╗ ██╗   ██╗███████╗
            ██╔══██╗██╔════╝ ██╔════╝██╔══██╗██║   ██║██╔════╝
            ███████║██║  ███╗█████╗  ██████╔╝██║   ██║███████╗
            ██╔══██║██║   ██║██╔══╝  ██╔══██╗██║   ██║╚════██║
            ██║  ██║╚██████╔╝███████╗██║  ██║╚██████╔╝███████║
            ╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝
    "#;

    // Convert string to Text object which Paragraph accepts
    let logo = Paragraph::new(Text::from(logo_str)).alignment(Alignment::Center).style(Style::default().fg(FG_PRIMARY));
    f.render_widget(logo, chunks[1]);

    // Commands List (Centered)
    let commands_text = vec![
        Line::from(vec![
            Span::styled("Commands         ", Style::default().fg(FG_PRIMARY)),
            Span::styled("ctrl+p", Style::default().fg(ACCENT_ORANGE)),
        ]),
        Line::from(vec![
            Span::styled("List sessions    ", Style::default().fg(FG_PRIMARY)),
            Span::styled("ctrl+l", Style::default().fg(ACCENT_ORANGE)),
        ]),
        Line::from(vec![
            Span::styled("Switch view      ", Style::default().fg(FG_PRIMARY)),
            Span::styled("tab   ", Style::default().fg(ACCENT_ORANGE)),
        ]),
        Line::from(vec![
            Span::styled("Exit             ", Style::default().fg(FG_PRIMARY)),
            Span::styled("ctrl+c", Style::default().fg(ACCENT_ORANGE)),
        ]),
    ];

    // Center the commands horizontally
    let cmd_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(30), Constraint::Percentage(35)])
        .split(chunks[3]);

    let commands = Paragraph::new(commands_text).alignment(Alignment::Center);
    f.render_widget(commands, cmd_layout[1]);
}
