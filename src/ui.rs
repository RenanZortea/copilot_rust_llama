use crate::app::{App, AppMode, MessageRole};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};
use textwrap::wrap;

pub fn draw(f: &mut Frame, app: &App) {
    // 1. Vertical Layout (Header/Tabs -> Content -> Input)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(1),    // Main Content
            Constraint::Length(3), // Input
        ])
        .split(f.size());

    draw_tabs(f, app, chunks[0]);

    match app.mode {
        AppMode::Chat => draw_chat(f, app, chunks[1]),
        AppMode::Terminal => draw_terminal(f, app, chunks[1]),
    }

    draw_input(f, app, chunks[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![" Chat ", " Terminal "];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .select(match app.mode {
            AppMode::Chat => 0,
            AppMode::Terminal => 1,
        });
    f.render_widget(tabs, area);
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    // Minimal aesthetic: No borders around the list itself, just content
    let max_width = (area.width.saturating_sub(2)) as usize;

    let items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|msg| {
            let (role_str, style) = match msg.role {
                MessageRole::User => (
                    "YOU",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Assistant => (
                    "AI",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::System => ("SYS", Style::default().fg(Color::DarkGray)),
                MessageRole::Error => ("ERR", Style::default().fg(Color::Red)),
            };

            let header = Line::from(Span::styled(role_str, style));

            // Wrap content
            let wrapped_lines = wrap(&msg.content, max_width);
            let mut content_lines = vec![header];

            for line in wrapped_lines {
                // Slight indent for text
                content_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::raw(line.to_string()),
                ]));
            }
            content_lines.push(Line::from("")); // Spacing

            ListItem::new(content_lines)
        })
        .collect();

    let chat_list = List::new(items).block(Block::default().borders(Borders::NONE)); // Clean look

    let mut state = app.chat_scroll.clone();
    f.render_stateful_widget(chat_list, area, &mut state);
}

fn draw_terminal(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .terminal_lines
        .iter()
        .map(|line| {
            ListItem::new(Line::from(Span::styled(
                line,
                Style::default().fg(Color::Green), // Retro terminal green
            )))
        })
        .collect();

    let term_list = List::new(items)
        .block(Block::default().padding(ratatui::widgets::Padding::new(1, 1, 0, 0)));

    let mut state = app.term_scroll.clone();
    f.render_stateful_widget(term_list, area, &mut state);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let input_style = if app.is_processing {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .borders(Borders::TOP)
        .title(if app.is_processing {
            " Processing... "
        } else {
            " Message "
        })
        .title_style(Style::default().fg(Color::DarkGray));

    let scroll_offset = if app.input_buffer.len() > area.width as usize {
        app.input_buffer.len() - area.width as usize + 5
    } else {
        0
    };

    let text_slice = if app.input_buffer.len() > scroll_offset {
        &app.input_buffer[scroll_offset..]
    } else {
        &app.input_buffer
    };

    let p = Paragraph::new(text_slice).style(input_style).block(block);
    f.render_widget(p, area);
}
