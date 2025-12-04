use super::theme::*;
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, List, ListItem, Padding},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .terminal_lines
        .iter()
        .map(|l| ListItem::new(Line::from(Span::styled(l, Style::default().fg(FG_PRIMARY)))))
        .collect();

    // Using a mutable borrow of app state for scrolling is standard pattern in ratatui
    // We clone the state here because draw takes &App (immutable), but List needs internal mutability for state
    // In a real app we might pass &mut App or keep state separate, but cloning ListState is cheap.
    let mut state = app.term_scroll.clone();

    f.render_stateful_widget(
        List::new(items).block(Block::default().padding(Padding::new(1, 1, 1, 1))),
        area,
        &mut state,
    );
}
