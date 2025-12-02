use crate::app::{App, MessageRole};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use textwrap::wrap;

pub fn draw(f: &mut Frame, app: &App) {
    // Split screen: Top (Chat), Bottom (Input)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Chat history takes available space
            Constraint::Length(3), // Input box fixed height
        ])
        .split(f.size());

    // Calculate available width for text
    // Width - 2 (borders) - 2 (padding/prefixes approx)
    let max_width = (chunks[0].width.saturating_sub(4)) as usize;

    // --- Chat Area ---
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .map(|m| {
            let (prefix, style) = match m.role {
                MessageRole::User => (
                    "User: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Assistant => ("AI: ", Style::default().fg(Color::Green)),
                MessageRole::System => (
                    "Sys: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::ITALIC),
                ),
                MessageRole::Error => ("Err: ", Style::default().fg(Color::Red)),
            };

            let mut lines = vec![];

            // 1. First line contains the Prefix + first part of content
            // We combine them to wrap properly, or just print prefix on line 1
            let full_text = format!("{}{}", prefix, m.content);

            // Wrap the text
            let wrapped_lines = wrap(&m.content, max_width);

            if wrapped_lines.is_empty() {
                // Handle empty message (processing...)
                lines.push(Line::from(Span::styled(prefix, style)));
            } else {
                for (i, line_str) in wrapped_lines.iter().enumerate() {
                    if i == 0 {
                        // First line has the prefix
                        lines.push(Line::from(vec![
                            Span::styled(prefix, style),
                            Span::raw(line_str.to_string()),
                        ]));
                    } else {
                        // Subsequent lines are indented slightly or just raw
                        lines.push(Line::from(Span::raw(line_str.to_string())));
                    }
                }
            }

            // Add spacing after message
            lines.push(Line::from(""));

            ListItem::new(lines)
        })
        .collect();

    let chat_list = List::new(messages).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Ollama Terminal "),
    );

    // Automatically scroll to bottom
    // We count the number of items (messages), not lines, so this works for the List widget
    let msg_count = app.messages.len();
    let mut state = app.list_state.clone();
    if msg_count > 0 {
        state.select(Some(msg_count - 1));
    }

    f.render_stateful_widget(chat_list, chunks[0], &mut state);

    // --- Input Area ---
    let input_style = if app.is_processing {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Input (Esc to Quit) ")
        .border_style(if app.is_processing {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Cyan)
        });

    // We manually scroll the input if it gets too long
    // A simple way is to take the last N characters that fit
    let input_width = (chunks[1].width.saturating_sub(2)) as usize;
    let input_content = &app.input_buffer;
    let display_input = if input_content.len() > input_width {
        &input_content[input_content.len() - input_width..]
    } else {
        input_content
    };

    let input_text = Paragraph::new(display_input)
        .style(input_style)
        .block(input_block);

    f.render_widget(input_text, chunks[1]);
}
