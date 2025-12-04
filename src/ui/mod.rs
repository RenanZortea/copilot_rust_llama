pub mod chat;
pub mod input;
pub mod model_selector;
pub mod splash;
pub mod status;
pub mod terminal;
pub mod theme;

use crate::app::{App, AppMode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Stylize, // <--- Added this import
    widgets::Block,
    Frame,
};
use theme::BG_MAIN;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // 1. Main Background
    f.render_widget(Block::default().bg(BG_MAIN), area);

    // 2. Vertical Layout: [ Content (Flex), Input (4), Status (1) ]
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Chat or Terminal or Splash
            Constraint::Length(4), // Input Bar (Height 4 for padding)
            Constraint::Length(1), // Status Footer
        ])
        .split(area);

    let content_area = vertical[0];
    let input_area = vertical[1];
    let status_area = vertical[2];

    // 3. Render Content Area
    if app.messages.len() <= 1 && app.mode == AppMode::Chat {
        splash::draw(f, content_area);
    } else {
        match app.mode {
            AppMode::Chat | AppMode::ModelSelector => chat::draw(f, app, content_area),
            AppMode::Terminal => terminal::draw(f, app, content_area),
        }
    }

    // 4. Render Input Bar
    input::draw(f, app, input_area);

    // 5. Render Status Bar
    status::draw(f, app, status_area);

    // 6. Overlays
    if app.mode == AppMode::ModelSelector {
        model_selector::draw(f, app, area);
    }
}
