use crate::app::{App, MessageRole};
use crate::markdown::render_markdown;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use super::theme::*;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    // Add some padding so text isn't glued to the edge
    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(1), Constraint::Length(2)])
        .split(area)[1];

    let mut lines = vec![];
    let max_width = area.width as usize;

    for msg in &app.messages {
        // Skip the initial system message in chat view to keep it clean
        if matches!(msg.role, MessageRole::System) && msg.content.starts_with("Ready") {
           continue; 
        }

        match msg.role {
            MessageRole::System => {
                lines.push(Line::from(Span::styled(format!("  >> {}", msg.content), Style::default().fg(FG_SECONDARY))));
            }
            MessageRole::Thinking => {
                lines.push(Line::from(vec![Span::styled("  ⚡ Thinking...", Style::default().fg(FG_SECONDARY).add_modifier(Modifier::ITALIC))]));
                let rendered = render_markdown(&msg.content, max_width - 4, Style::default().fg(FG_SECONDARY).add_modifier(Modifier::ITALIC));
                for line in rendered {
                    let mut spans = vec![Span::raw("    ")];
                    spans.extend(line.spans);
                    lines.push(Line::from(spans));
                }
            }
            _ => {
                let (name, style) = match msg.role {
                    MessageRole::User => ("User", Style::default().fg(ACCENT_BLUE).add_modifier(Modifier::BOLD)),
                    MessageRole::Assistant => ("Agerus", Style::default().fg(ACCENT_ORANGE).add_modifier(Modifier::BOLD)),
                    MessageRole::Error => ("Error", Style::default().fg(Color::Red)),
                    _ => ("System", Style::default().fg(FG_SECONDARY)),
                };
                
                // Header: Name + Time
                lines.push(Line::from(vec![
                    Span::styled(name, style),
                    Span::styled(format!(" {}", chrono::Local::now().format("%H:%M")), Style::default().fg(FG_SECONDARY)),
                ]));

                // Content
                if matches!(msg.role, MessageRole::Error) {
                     lines.push(Line::from(Span::styled(&msg.content, Style::default().fg(Color::Red))));
                } else {
                    let base_style = Style::default().fg(FG_PRIMARY);
                    let rendered = render_markdown(&msg.content, max_width, base_style);
                    lines.extend(rendered);
                }
            }
        }
        lines.push(Line::from("")); // Spacing
    }

    let scroll = if app.chat_stick_to_bottom {
        (lines.len() as u16).saturating_sub(area.height)
    } else {
        app.chat_scroll
    };

    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}
