use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// --- Theme Configuration ---
const COLOR_HEADER: Color = Color::Rgb(88, 166, 255); // Cyan/Blue
const COLOR_CODE_BG: Color = Color::Rgb(30, 30, 30); // Dark Gray for blocks
const COLOR_CODE_FG: Color = Color::Rgb(255, 123, 114); // Red/Pink
const COLOR_BOLD: Color = Color::White;
const COLOR_LIST_MARKER: Color = Color::Rgb(63, 185, 80); // Green

pub fn render_markdown(text: &str, width: usize, base_style: Style) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut current_width = 0;

    // Parser options
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(text, options);

    // State machine for styling
    let mut style_stack = vec![base_style];
    let mut in_code_block = false;
    let mut list_depth = 0;

    // Helper function (defined locally to avoid closure capture issues)
    fn force_newline(
        lines: &mut Vec<Line<'static>>,
        current_line: &mut Vec<Span<'static>>,
        current_width: &mut usize,
    ) {
        if !current_line.is_empty() {
            lines.push(Line::from(current_line.clone()));
            current_line.clear();
            *current_width = 0;
        }
    }

    // Helper function for pushing words
    fn push_word(
        lines: &mut Vec<Line<'static>>,
        current_line: &mut Vec<Span<'static>>,
        current_width: &mut usize,
        width: usize,
        word: String,
        style: Style,
    ) {
        let len = word.chars().count();

        // If word fits or line is empty
        if *current_width + len <= width || *current_width == 0 {
            current_line.push(Span::styled(word, style));
            *current_width += len;
        } else {
            // Wrap to new line
            lines.push(Line::from(current_line.clone()));
            current_line.clear();

            let trimmed = word.trim_start();
            if !trimmed.is_empty() {
                current_line.push(Span::styled(trimmed.to_string(), style));
                *current_width = trimmed.chars().count();
            } else {
                *current_width = 0;
            }
        }
    }

    for event in parser {
        match event {
            Event::Start(tag) => {
                let new_style = match tag {
                    Tag::Heading { level, .. } => {
                        force_newline(&mut lines, &mut current_line, &mut current_width);
                        // Add spacing before header
                        if !lines.is_empty() {
                            lines.push(Line::from(""));
                        }

                        let s = Style::default()
                            .fg(COLOR_HEADER)
                            .add_modifier(Modifier::BOLD);
                        match level {
                            pulldown_cmark::HeadingLevel::H1 => {
                                s.add_modifier(Modifier::UNDERLINED)
                            }
                            _ => s,
                        }
                    }
                    Tag::Paragraph => {
                        force_newline(&mut lines, &mut current_line, &mut current_width);
                        *style_stack.last().unwrap()
                    }
                    Tag::CodeBlock(kind) => {
                        force_newline(&mut lines, &mut current_line, &mut current_width);
                        in_code_block = true;

                        // Optional: Add language label
                        if let CodeBlockKind::Fenced(lang) = kind {
                            if !lang.is_empty() {
                                lines.push(Line::from(Span::styled(
                                    format!("```{}", lang),
                                    Style::default().fg(Color::DarkGray),
                                )));
                            }
                        }

                        Style::default().fg(COLOR_CODE_FG).bg(COLOR_CODE_BG)
                    }
                    Tag::List(_) => {
                        force_newline(&mut lines, &mut current_line, &mut current_width);
                        list_depth += 1;
                        *style_stack.last().unwrap()
                    }
                    Tag::Item => {
                        force_newline(&mut lines, &mut current_line, &mut current_width);
                        // Add indentation and bullet
                        let indent = "  ".repeat(list_depth - 1);
                        current_line.push(Span::raw(indent));
                        current_line
                            .push(Span::styled("• ", Style::default().fg(COLOR_LIST_MARKER)));
                        current_width += (list_depth - 1) * 2 + 2;
                        *style_stack.last().unwrap()
                    }
                    Tag::Emphasis => style_stack.last().unwrap().add_modifier(Modifier::ITALIC),
                    Tag::Strong => style_stack
                        .last()
                        .unwrap()
                        .fg(COLOR_BOLD)
                        .add_modifier(Modifier::BOLD),
                    Tag::Strikethrough => style_stack
                        .last()
                        .unwrap()
                        .add_modifier(Modifier::CROSSED_OUT),
                    Tag::Link { .. } => style_stack
                        .last()
                        .unwrap()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                    _ => *style_stack.last().unwrap(),
                };
                style_stack.push(new_style);
            }
            Event::End(tag) => {
                style_stack.pop();
                match tag {
                    TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::Item => {
                        force_newline(&mut lines, &mut current_line, &mut current_width);
                    }
                    TagEnd::CodeBlock => {
                        force_newline(&mut lines, &mut current_line, &mut current_width);
                        in_code_block = false;
                    }
                    TagEnd::List(_) => {
                        list_depth -= 1;
                        if list_depth == 0 {
                            force_newline(&mut lines, &mut current_line, &mut current_width);
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                let style = *style_stack.last().unwrap();

                if in_code_block {
                    // For code blocks, we don't wrap words typically, we just dump the line
                    // But here we get chunks of text.
                    let parts: Vec<&str> = text.split('\n').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if i > 0 {
                            force_newline(&mut lines, &mut current_line, &mut current_width);
                        }
                        current_line.push(Span::styled(part.to_string(), style));
                        current_width += part.chars().count();
                    }
                } else {
                    // Standard reflow wrapping
                    let words = text.split_inclusive(char::is_whitespace);
                    for word in words {
                        push_word(
                            &mut lines,
                            &mut current_line,
                            &mut current_width,
                            width,
                            word.to_string(),
                            style,
                        );
                    }
                }
            }
            Event::Code(text) => {
                // Inline code
                let style = style_stack
                    .last()
                    .unwrap()
                    .fg(COLOR_CODE_FG)
                    .bg(COLOR_CODE_BG);
                push_word(
                    &mut lines,
                    &mut current_line,
                    &mut current_width,
                    width,
                    format!(" {} ", text),
                    style,
                );
            }
            Event::SoftBreak => {
                push_word(
                    &mut lines,
                    &mut current_line,
                    &mut current_width,
                    width,
                    " ".to_string(),
                    *style_stack.last().unwrap(),
                );
            }
            Event::HardBreak => {
                force_newline(&mut lines, &mut current_line, &mut current_width);
            }
            _ => {}
        }
    }

    // Flush remainder
    force_newline(&mut lines, &mut current_line, &mut current_width);

    lines
}
