use crate::app::{App, AppMode, MessageRole};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};
use textwrap::wrap;

const THROBBER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let width = area.width.max(3) - 2;

    let mut input_height = 0;
    for line in app.input_buffer.lines() {
        let line_len = line.len() as u16;
        if line_len == 0 {
            input_height += 1;
        } else {
            input_height += (line_len + width - 1) / width;
        }
    }
    if input_height == 0 {
        input_height = 1;
    }
    let constrained_height = (input_height).min(10).max(1) + 2;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(constrained_height as u16),
        ])
        .split(area);

    draw_tabs(f, app, chunks[0]);

    match app.mode {
        AppMode::Chat => draw_chat(f, app, chunks[1]),
        AppMode::Terminal => draw_terminal(f, app, chunks[1]),
    }

    draw_input(f, app, chunks[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let spinner = if app.is_processing {
        THROBBER[app.spinner_frame % THROBBER.len()]
    } else {
        " "
    };
    let chat_title = format!("{} Agent ", spinner);
    let tabs = Tabs::new(vec![chat_title.as_str(), " Terminal "])
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .select(match app.mode {
            AppMode::Chat => 0,
            AppMode::Terminal => 1,
        });
    f.render_widget(tabs, area);
}

fn parse_markdown_line(line: &str, in_code_block: bool, base_style: Style) -> (Line<'static>, bool) {
    if line.trim().starts_with("```") {
        return (
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::DarkGray),
            )),
            !in_code_block,
        );
    }
    let mut spans = vec![];
    
    // If we are in a code block, use Green. Otherwise use the base style passed in (Gray for User/Thinking, Yellow for AI)
    let style = if in_code_block {
        Style::default().fg(Color::Green)
    } else if line.starts_with("# ") {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        base_style
    };

    if !in_code_block {
        let parts: Vec<&str> = line.split("**").collect();
        for (i, part) in parts.iter().enumerate() {
            let s = if i % 2 == 1 {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            };
            spans.push(Span::styled(part.to_string(), s));
        }
    } else {
        spans.push(Span::styled(line.to_string(), style));
    }
    (Line::from(spans), in_code_block)
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    let max_width = (area.width.saturating_sub(4)) as usize;
    let mut all_lines = vec![];

    for (i, msg) in app.messages.iter().enumerate() {
        let (header, h_style, content_style) = match msg.role {
            MessageRole::User => (
                " USER ",
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Gray),
            ),
            MessageRole::Assistant => (
                " AI ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            ),
            MessageRole::Thinking => (
                " THINK ",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            ),
            MessageRole::System => (
                " SYS ",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::DarkGray),
            ),
            MessageRole::Error => (
                " ERR ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Red),
            ),
        };

        all_lines.push(Line::from(Span::styled(header, h_style)));

        let content = msg.content.clone();
        let mut in_code_block = false;
        let show_cursor = i == app.messages.len() - 1
            && app.is_processing
            && matches!(msg.role, MessageRole::Assistant | MessageRole::Thinking);

        for line in content.lines() {
            let wrapped = wrap(line, max_width);
            if wrapped.is_empty() {
                let (l, next_state) = parse_markdown_line("", in_code_block, content_style);
                in_code_block = next_state;
                all_lines.push(l);
            }
            for w_line in wrapped {
                let (l, next_state) = parse_markdown_line(&w_line, in_code_block, content_style);
                in_code_block = next_state;
                all_lines.push(l);
            }
        }

        if show_cursor && (app.spinner_frame / 5) % 2 == 0 {
            all_lines.push(Line::from(Span::styled(
                " ▋",
                Style::default().fg(Color::Yellow),
            )));
        }
        all_lines.push(Line::from(""));
    }

    let total_height = all_lines.len() as u16;
    let view_height = area.height.saturating_sub(2);

    let scroll_y = if app.chat_stick_to_bottom {
        if total_height > view_height {
            total_height - view_height
        } else {
            0
        }
    } else {
        app.chat_scroll
    };

    let p = Paragraph::new(all_lines)
        .block(Block::default().borders(Borders::NONE))
        .scroll((scroll_y, 0));

    f.render_widget(p, area);
}

fn draw_terminal(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .terminal_lines
        .iter()
        .map(|l| {
            ListItem::new(Line::from(Span::styled(
                l,
                Style::default().fg(Color::DarkGray),
            )))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::LEFT).title(" Shell "));
    let mut state = app.term_scroll.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.is_processing {
        " Processing... (Esc to STOP) "
    } else {
        match app.mode {
            AppMode::Chat => " Message Agent (Alt+Enter for newline) ",
            AppMode::Terminal => " Manual Terminal Command ",
        }
    };

    let border_style = if app.is_processing {
        Style::default().fg(Color::Red)
    } else {
        match app.mode {
            AppMode::Chat => Style::default().fg(Color::Cyan),
            AppMode::Terminal => Style::default().fg(Color::Green),
        }
    };

    let p = Paragraph::new(app.input_buffer.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}
