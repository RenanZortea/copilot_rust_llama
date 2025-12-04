use ratatui::style::Color;

// --- OpenCode Theme Palette ---
pub const BG_MAIN: Color = Color::Rgb(9, 9, 9); // Deep Black
pub const BG_INPUT: Color = Color::Rgb(30, 30, 30); // Dark Gray for input
pub const FG_PRIMARY: Color = Color::Rgb(220, 220, 220); // Off-white
pub const FG_SECONDARY: Color = Color::Rgb(100, 100, 100); // Dimmed text
pub const ACCENT_ORANGE: Color = Color::Rgb(255, 158, 100); // Cursor / Highlight
pub const ACCENT_BLUE: Color = Color::Rgb(122, 162, 247); // Mode indicators

pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
