use crate::agent::{run_agent_loop, AgentConfig};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

// --- Enums ---

#[derive(Clone, PartialEq)]
pub enum AppMode {
    Chat,
    Terminal,
}

#[derive(Clone)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Error,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

pub enum AppEvent {
    Token(String),
    AgentFinished,
    CommandStart(String),
    CommandEnd(String),
    TerminalLine(String), // New event for real-time streaming
    Error(String),
}

// --- State ---

pub struct App {
    pub mode: AppMode,
    pub input_buffer: String,

    // Chat Tab Data
    pub messages: Vec<ChatMessage>,
    pub chat_scroll: ListState,

    // Terminal Tab Data
    pub terminal_lines: Vec<String>,
    pub term_scroll: ListState,

    pub is_processing: bool,
    pub event_tx: mpsc::Sender<AppEvent>,
}

impl App {
    pub fn new(event_tx: mpsc::Sender<AppEvent>) -> Self {
        Self {
            mode: AppMode::Chat,
            input_buffer: String::new(),
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: "Welcome. Press [Tab] to switch views.".into(),
            }],
            chat_scroll: ListState::default(),
            terminal_lines: vec![String::from("--- Docker Shell Connected ---")],
            term_scroll: ListState::default(),
            is_processing: false,
            event_tx,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                self.mode = match self.mode {
                    AppMode::Chat => AppMode::Terminal,
                    AppMode::Terminal => AppMode::Chat,
                };
            }
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            KeyCode::PageUp => self.scroll_page_up(),
            KeyCode::PageDown => self.scroll_page_down(),
            KeyCode::Char(c) if !self.is_processing => self.input_buffer.push(c),
            KeyCode::Backspace if !self.is_processing => {
                self.input_buffer.pop();
            }
            KeyCode::Enter if !self.is_processing => self.submit_message(),
            _ => {}
        }
    }

    pub fn handle_internal_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Token(token) => {
                if let Some(last) = self.messages.last_mut() {
                    last.content.push_str(&token);
                }
            }
            AppEvent::CommandStart(cmd) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Running: {}", cmd),
                });
                self.add_terminal_log(format!("$ {}", cmd));
            }
            AppEvent::TerminalLine(line) => {
                // Stream line directly to terminal view
                self.add_terminal_log(line);
            }
            AppEvent::CommandEnd(_) => {
                // We don't print the whole block anymore since we streamed it.
                // Just add an empty AI message for the next response.
                self.messages.push(ChatMessage {
                    role: MessageRole::Assistant,
                    content: String::new(),
                });
            }
            AppEvent::AgentFinished => {
                self.is_processing = false;
            }
            AppEvent::Error(err) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::Error,
                    content: err.clone(),
                });
                self.add_terminal_log(format!("[ERROR] {}", err));
                self.is_processing = false;
            }
        }
    }

    fn submit_message(&mut self) {
        if self.input_buffer.trim().is_empty() {
            return;
        }

        let text = self.input_buffer.clone();
        self.input_buffer.clear();
        self.is_processing = true;

        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: text.clone(),
        });

        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
        });

        self.chat_scroll.select(Some(self.messages.len() - 1));

        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let config = AgentConfig::default();
            if let Err(e) = run_agent_loop(config, text, tx.clone()).await {
                let _ = tx.send(AppEvent::Error(e.to_string())).await;
            }
            let _ = tx.send(AppEvent::AgentFinished).await;
        });
    }

    fn add_terminal_log(&mut self, line: String) {
        self.terminal_lines.push(line);
        // Stick to bottom if already at bottom
        let len = self.terminal_lines.len();
        if len > 0 {
            self.term_scroll.select(Some(len - 1));
        }
    }

    // --- Scrolling Logic ---
    fn scroll_up(&mut self) {
        match self.mode {
            AppMode::Chat => Self::scroll_list(&mut self.chat_scroll, &self.messages, -1),
            AppMode::Terminal => Self::scroll_list(&mut self.term_scroll, &self.terminal_lines, -1),
        }
    }
    fn scroll_down(&mut self) {
        match self.mode {
            AppMode::Chat => Self::scroll_list(&mut self.chat_scroll, &self.messages, 1),
            AppMode::Terminal => Self::scroll_list(&mut self.term_scroll, &self.terminal_lines, 1),
        }
    }
    fn scroll_page_up(&mut self) {
        match self.mode {
            AppMode::Chat => Self::scroll_list(&mut self.chat_scroll, &self.messages, -10),
            AppMode::Terminal => {
                Self::scroll_list(&mut self.term_scroll, &self.terminal_lines, -10)
            }
        }
    }
    fn scroll_page_down(&mut self) {
        match self.mode {
            AppMode::Chat => Self::scroll_list(&mut self.chat_scroll, &self.messages, 10),
            AppMode::Terminal => Self::scroll_list(&mut self.term_scroll, &self.terminal_lines, 10),
        }
    }
    fn scroll_list<T>(state: &mut ListState, items: &[T], amount: i32) {
        let i = match state.selected() {
            Some(i) => i as i32 + amount,
            None => 0,
        };
        let clamped = i.clamp(0, items.len().saturating_sub(1) as i32);
        state.select(Some(clamped as usize));
    }
}
