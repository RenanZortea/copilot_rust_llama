use crate::app::{App, AppMode, MessageRole};
use crate::markdown::render_markdown; // Import the new renderer
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

// --- Theme / Color Palette ---
const BG_MAIN: Color = Color::Rgb(13, 17, 23);
const BG_SIDEBAR: Color = Color::Rgb(22, 27, 34);
const BORDER_COLOR: Color = Color::Rgb(48, 54, 61);
const FG_PRIMARY: Color = Color::Rgb(201, 209, 217);
const FG_SECONDARY: Color = Color::Rgb(139, 148, 158);
const ACCENT_CYAN: Color = Color::Rgb(88, 166, 255);
const ACCENT_GREEN: Color = Color::Rgb(63, 185, 80);
const ACCENT_RED: Color = Color::Rgb(248, 81, 73);
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Block::default().bg(BG_MAIN), area);

    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(1)])
        .split(area);

    draw_sidebar(f, app, main_layout[0]);
    draw_main_content(f, app, main_layout[1]);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let sidebar_block = Block::default()
        .bg(BG_SIDEBAR)
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(BORDER_COLOR));
    f.render_widget(sidebar_block.clone(), area);
    let inner_area = sidebar_block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Length(2), Constraint::Min(1), Constraint::Length(3)])
        .split(inner_area);

    // Header
    let header_text = vec![
        Line::from(vec![
            Span::styled(">_ ", Style::default().fg(ACCENT_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Agerus Agent", Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("model: ", Style::default().fg(FG_SECONDARY)),
            Span::styled(app.config.model.clone(), Style::default().fg(ACCENT_CYAN)),
        ]),
        Line::from(vec![
            Span::styled("cwd:   ", Style::default().fg(FG_SECONDARY)),
            Span::styled(app.config.workspace_path.file_name().unwrap_or_default().to_string_lossy(), Style::default().fg(FG_PRIMARY)),
        ]),
    ];
    f.render_widget(Paragraph::new(header_text).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(BORDER_COLOR)).padding(Padding::new(1, 1, 1, 1))), chunks[0]);

    // Navigation
    let active = Style::default().fg(ACCENT_CYAN).add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(FG_SECONDARY);
    let nav_items = vec![
        ListItem::new(if app.mode == AppMode::Chat { "● Chat" } else { "○ Chat" }).style(if app.mode == AppMode::Chat { active } else { inactive }),
        ListItem::new(if app.mode == AppMode::Terminal { "● Terminal" } else { "○ Terminal" }).style(if app.mode == AppMode::Terminal { active } else { inactive }),
        ListItem::new(""),
        ListItem::new(Line::from(vec![Span::styled("Session:", Style::default().fg(FG_SECONDARY).add_modifier(Modifier::UNDERLINED))])),
        ListItem::new(Line::from(Span::styled(&app.current_session, Style::default().fg(ACCENT_GREEN)))),
        ListItem::new(""),
        ListItem::new(Line::from(vec![Span::styled("Commands:", Style::default().fg(FG_SECONDARY).add_modifier(Modifier::UNDERLINED))])),
        ListItem::new(Line::from(vec![Span::styled("/new", Style::default().fg(FG_PRIMARY)), Span::raw(" - New Chat")])),
        ListItem::new(Line::from(vec![Span::styled("/load", Style::default().fg(FG_PRIMARY)), Span::raw(" - Load Chat")])),
        ListItem::new(Line::from(vec![Span::styled("/list", Style::default().fg(FG_PRIMARY)), Span::raw(" - List Chats")])),
    ];
    f.render_widget(List::new(nav_items).block(Block::default().padding(Padding::horizontal(1))), chunks[2]);

    // Footer
    let (symbol, style) = if app.is_processing {
        (SPINNER[app.spinner_frame % SPINNER.len()], Style::default().fg(ACCENT_CYAN).add_modifier(Modifier::BOLD))
    } else {
        ("●", Style::default().fg(ACCENT_GREEN))
    };
    f.render_widget(Paragraph::new(Line::from(Span::styled(symbol, style))).block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(BORDER_COLOR)).padding(Padding::new(1, 1, 0, 0))), chunks[3]);
}

fn draw_main_content(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(3)])
        .split(area);

    match app.mode {
        AppMode::Chat => draw_chat_view(f, app, chunks[0]),
        AppMode::Terminal => draw_terminal_view(f, app, chunks[0]),
    }

    f.render_widget(Block::default().borders(Borders::TOP).border_style(Style::default().fg(BORDER_COLOR)), chunks[1]);
    draw_input_bar(f, app, chunks[2]);
}

fn draw_chat_view(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().padding(Padding::new(2, 2, 1, 1));
    f.render_widget(block.clone(), area);
    let inner_area = block.inner(area);
    let max_width = inner_area.width as usize;
    let mut lines = vec![];

    for msg in &app.messages {
        match msg.role {
            MessageRole::System => {
                // System messages are simple text
                lines.push(Line::from(Span::styled(format!("  >> {}", msg.content), Style::default().fg(FG_SECONDARY))));
            }
            MessageRole::Thinking => {
                // Render thinking with Markdown too, but styled as Italic/Dim
                lines.push(Line::from(vec![Span::styled("  ⚡ Thinking...", Style::default().fg(FG_SECONDARY).add_modifier(Modifier::ITALIC))]));
                
                // We use a narrower width for thinking blocks to indent them
                let think_width = max_width.saturating_sub(4);
                let base_style = Style::default().fg(FG_SECONDARY).add_modifier(Modifier::ITALIC);
                
                // Render using the new markdown engine
                let rendered = render_markdown(&msg.content, think_width, base_style);
                
                for line in rendered {
                    // Prepend indentation
                    let mut spans = vec![Span::raw("    ")];
                    spans.extend(line.spans);
                    lines.push(Line::from(spans));
                }
            }
            _ => {
                // Header
                let (name, style, color) = match msg.role {
                    MessageRole::User => ("User", Style::default().fg(ACCENT_GREEN).add_modifier(Modifier::BOLD), FG_PRIMARY),
                    MessageRole::Error => ("Error", Style::default().fg(ACCENT_RED).add_modifier(Modifier::BOLD), ACCENT_RED),
                    _ => ("Agerus", Style::default().fg(ACCENT_CYAN).add_modifier(Modifier::BOLD), FG_PRIMARY),
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", name), style),
                    Span::styled(chrono::Local::now().format("%H:%M").to_string(), Style::default().fg(FG_SECONDARY)),
                ]));

                // Content - Render with Markdown!
                if matches!(msg.role, MessageRole::Error) {
                    // Errors usually raw text
                    lines.push(Line::from(Span::styled(&msg.content, Style::default().fg(color))));
                } else {
                    let base_style = Style::default().fg(color);
                    let rendered = render_markdown(&msg.content, max_width, base_style);
                    lines.extend(rendered);
                }
            }
        }
        lines.push(Line::from("")); // Margin
    }

    let scroll = if app.chat_stick_to_bottom {
        (lines.len() as u16).saturating_sub(inner_area.height)
    } else {
        app.chat_scroll
    };
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner_area);
}

fn draw_terminal_view(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.terminal_lines.iter().map(|l| ListItem::new(Line::from(Span::styled(l, Style::default().fg(FG_PRIMARY))))).collect();
    let mut state = app.term_scroll.clone();
    f.render_stateful_widget(List::new(items).block(Block::default().padding(Padding::new(1, 1, 1, 1))), area, &mut state);
}

fn draw_input_bar(f: &mut Frame, app: &App, area: Rect) {
    let (prompt, style) = if app.mode == AppMode::Chat { ("> ", Style::default().fg(ACCENT_CYAN)) } else { ("> ", Style::default().fg(ACCENT_GREEN)) };
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(prompt, style.add_modifier(Modifier::BOLD)),
        Span::styled(&app.input_buffer, Style::default().fg(FG_PRIMARY)),
        Span::styled("▋", Style::default().fg(FG_SECONDARY)),
    ])).block(Block::default().padding(Padding::horizontal(1))), area);
}
