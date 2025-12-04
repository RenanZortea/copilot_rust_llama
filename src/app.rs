use crate::agent::run_agent_loop;
use crate::config::Config;
use crate::mcp::McpRequest;
use crate::session::SessionManager;
use crate::shell::ShellRequest;
use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum AppMode {
    Chat,
    Terminal,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Error,
    Thinking,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

pub enum AppEvent {
    Token(String),
    Thinking(String),
    AgentFinished,
    CommandStart(String),
    CommandEnd(String),
    TerminalLine(String),
    Error(String),
    Tick,
}

pub struct App {
    pub mode: AppMode,
    pub input_buffer: String,
    pub messages: Vec<ChatMessage>,

    // Session State
    pub current_session: String,
    pub session_manager: SessionManager,

    // UI State
    pub chat_scroll: u16,
    pub chat_stick_to_bottom: bool,
    pub terminal_lines: Vec<String>,
    pub term_scroll: ListState,
    pub spinner_frame: usize,

    // Async State
    pub is_processing: bool,
    pub agent_task: Option<JoinHandle<()>>,

    // Channels
    pub event_tx: mpsc::Sender<AppEvent>,
    pub shell_tx: mpsc::Sender<ShellRequest>,
    pub mcp_tx: mpsc::Sender<McpRequest>,
    pub config: Config,
}

impl App {
    pub fn new(
        event_tx: mpsc::Sender<AppEvent>,
        shell_tx: mpsc::Sender<ShellRequest>,
        mcp_tx: mpsc::Sender<McpRequest>,
        config: Config,
    ) -> Self {
        let session_manager = SessionManager::new();
        // Generate a default session name
        let current_session = format!("chat_{}", Local::now().format("%Y-%m-%d_%H-%M"));

        Self {
            mode: AppMode::Chat,
            input_buffer: String::new(),
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: format!("Ready. Model: {}", config.model),
            }],
            current_session,
            session_manager,

            chat_scroll: 0,
            chat_stick_to_bottom: true,

            terminal_lines: vec![String::from("--- Shell Connected ---")],
            term_scroll: ListState::default(),

            is_processing: false,
            agent_task: None,
            spinner_frame: 0,

            event_tx,
            shell_tx,
            mcp_tx,
            config,
        }
    }

    // --- Session Actions ---

    pub fn save_current_session(&mut self) {
        match self
            .session_manager
            .save_session(&self.current_session, &self.messages)
        {
            Ok(_) => {
                // Optional: Notify user via system message if explicit save,
                // but usually we want silent auto-save.
            }
            Err(e) => {
                self.add_system_message(format!("Auto-save failed: {}", e), MessageRole::Error);
            }
        }
    }

    pub fn load_session_by_name(&mut self, name: String) {
        match self.session_manager.load_session(&name) {
            Ok(msgs) => {
                self.messages = msgs;
                self.current_session = name;
                self.chat_stick_to_bottom = true;
                self.add_system_message(
                    format!("Session '{}' loaded.", self.current_session),
                    MessageRole::System,
                );
            }
            Err(e) => {
                self.add_system_message(format!("Failed to load: {}", e), MessageRole::Error);
            }
        }
    }

    pub fn start_new_session(&mut self, name_opt: Option<String>) {
        let name = name_opt
            .unwrap_or_else(|| format!("chat_{}", Local::now().format("%Y-%m-%d_%H-%M-%S")));

        self.messages.clear();
        self.current_session = name;
        self.add_system_message(
            format!("New Session: {}", self.current_session),
            MessageRole::System,
        );
        self.save_current_session();
    }

    fn add_system_message(&mut self, content: String, role: MessageRole) {
        self.messages.push(ChatMessage { role, content });
        self.chat_stick_to_bottom = true;
    }

    // --- Inputs & Events ---

    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_down(),
            MouseEventKind::ScrollUp => self.scroll_up(),
            _ => {}
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
            KeyCode::Esc if self.is_processing => self.abort_agent(),
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            KeyCode::PageUp => self.scroll_page(-10),
            KeyCode::PageDown => self.scroll_page(10),
            KeyCode::Char(c) if !self.is_processing => self.input_buffer.push(c),
            KeyCode::Backspace if !self.is_processing => {
                self.input_buffer.pop();
            }
            KeyCode::Enter if !self.is_processing => {
                if key.modifiers.contains(KeyModifiers::ALT) {
                    self.input_buffer.push('\n');
                } else {
                    self.submit_message();
                }
            }
            _ => {}
        }
    }

    fn abort_agent(&mut self) {
        if let Some(task) = self.agent_task.take() {
            task.abort();
        }
        self.is_processing = false;
        self.add_system_message("🛑 Cancelled by user.".into(), MessageRole::System);
        self.save_current_session();
    }

    pub fn handle_internal_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => {
                if self.is_processing {
                    self.spinner_frame = self.spinner_frame.wrapping_add(1);
                }
            }
            AppEvent::Token(t) => self.append_message_content(t, MessageRole::Assistant),
            AppEvent::Thinking(t) => self.append_message_content(t, MessageRole::Thinking),
            AppEvent::CommandStart(c) => {
                self.add_system_message(format!("🛠️ {}", c), MessageRole::System)
            }
            AppEvent::CommandEnd(o) => {
                let s = if o.len() > 200 {
                    format!("Output ({} bytes) sent to terminal.", o.len())
                } else {
                    o
                };
                self.add_system_message(s, MessageRole::System);
            }
            AppEvent::TerminalLine(l) => {
                let was_at_bottom = self
                    .term_scroll
                    .selected()
                    .map_or(true, |s| s >= self.terminal_lines.len().saturating_sub(1));
                self.terminal_lines.push(l);
                if was_at_bottom {
                    self.term_scroll
                        .select(Some(self.terminal_lines.len().saturating_sub(1)));
                }
            }
            AppEvent::AgentFinished => {
                self.is_processing = false;
                self.agent_task = None;
                self.save_current_session(); // Auto-save on answer
            }
            AppEvent::Error(e) => {
                self.add_system_message(e, MessageRole::Error);
                self.is_processing = false;
                self.agent_task = None;
                self.save_current_session();
            }
        }
    }

    fn append_message_content(&mut self, content: String, role: MessageRole) {
        let start_new = if let Some(last) = self.messages.last() {
            // Use Discriminant check or simple pattern match to see if roles differ
            // Since PartialEq is derived, we can compare specific enum variants if they have no data,
            // but these variants don't match exactly because they are same type.
            // We need to check if the *type* of the last role matches the new role.
            match (&last.role, &role) {
                (MessageRole::Assistant, MessageRole::Assistant) => false,
                (MessageRole::Thinking, MessageRole::Thinking) => false,
                _ => true,
            }
        } else {
            true
        };

        if start_new {
            self.messages.push(ChatMessage { role, content });
        } else {
            if let Some(last) = self.messages.last_mut() {
                last.content.push_str(&content);
            }
        }
        self.chat_stick_to_bottom = true;
    }

    fn submit_message(&mut self) {
        if self.input_buffer.trim().is_empty() {
            return;
        }
        let text = self.input_buffer.clone();
        self.input_buffer.clear();

        // --- Slash Commands ---
        if text.starts_with('/') {
            let parts: Vec<&str> = text.split_whitespace().collect();
            match parts[0] {
                "/new" => {
                    let arg = parts.get(1).map(|&s| s.to_string());
                    self.start_new_session(arg);
                    return;
                }
                "/save" => {
                    self.save_current_session();
                    self.add_system_message(
                        format!("Saved '{}'", self.current_session),
                        MessageRole::System,
                    );
                    return;
                }
                "/load" => {
                    if let Some(name) = parts.get(1) {
                        self.load_session_by_name(name.to_string());
                    } else {
                        self.add_system_message(
                            "Usage: /load <session_name>".into(),
                            MessageRole::Error,
                        );
                    }
                    return;
                }
                "/list" => {
                    match self.session_manager.list_sessions() {
                        Ok(list) => {
                            let content = format!("Available Sessions:\n- {}", list.join("\n- "));
                            self.add_system_message(content, MessageRole::System);
                        }
                        Err(e) => self
                            .add_system_message(format!("List failed: {}", e), MessageRole::Error),
                    }
                    return;
                }
                "/reset" => {
                    self.messages.clear();
                    self.add_system_message("Context reset.".into(), MessageRole::System);
                    return;
                }
                _ => {} // Treat as normal message
            }
        }

        match self.mode {
            AppMode::Chat => {
                self.is_processing = true;
                self.add_system_message(text.clone(), MessageRole::User); // Adds message + scrolls
                self.save_current_session(); // Save user input

                let tx = self.event_tx.clone();
                let mcp = self.mcp_tx.clone();
                let history = self.messages.clone();
                let config = self.config.clone();

                let handle = tokio::spawn(async move {
                    if let Err(e) = run_agent_loop(config, history, tx.clone(), mcp).await {
                        let _ = tx.send(AppEvent::Error(e.to_string())).await;
                    }
                    let _ = tx.send(AppEvent::AgentFinished).await;
                });
                self.agent_task = Some(handle);
            }
            AppMode::Terminal => {
                let shell = self.shell_tx.clone();
                tokio::spawn(async move {
                    let _ = shell.send(ShellRequest::UserInput(text)).await;
                });
            }
        }
    }

    fn scroll_up(&mut self) {
        if let AppMode::Chat = self.mode {
            self.chat_stick_to_bottom = false;
            self.chat_scroll = self.chat_scroll.saturating_sub(1);
        } else {
            let i = self.term_scroll.selected().unwrap_or(0) as i32;
            self.term_scroll.select(Some((i - 1).max(0) as usize));
        }
    }
    fn scroll_down(&mut self) {
        if let AppMode::Chat = self.mode {
            self.chat_stick_to_bottom = false;
            self.chat_scroll = self.chat_scroll.saturating_add(1);
        } else {
            let i = self.term_scroll.selected().unwrap_or(0) as i32;
            self.term_scroll.select(Some((i + 1).max(0) as usize));
        }
    }
    fn scroll_page(&mut self, amt: i16) {
        if let AppMode::Chat = self.mode {
            self.chat_stick_to_bottom = false;
            if amt < 0 {
                self.chat_scroll = self.chat_scroll.saturating_sub(amt.abs() as u16);
            } else {
                self.chat_scroll = self.chat_scroll.saturating_add(amt.abs() as u16);
            }
        } else {
            let i = self.term_scroll.selected().unwrap_or(0) as i32;
            self.term_scroll
                .select(Some((i + amt as i32).max(0) as usize));
        }
    }
}
