use crate::agent::{run_agent_loop, AgentConfig};
use crate::shell::ShellRequest;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Clone, PartialEq)]
pub enum AppMode {
    Chat,
    Terminal,
}

#[derive(Clone)]
pub enum MessageRole { User, Assistant, System, Error, Thinking }

#[derive(Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

pub enum AppEvent {
    Token(String),
    Thinking(String), // New event for thinking stream
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
    
    pub chat_scroll: u16,
    pub chat_stick_to_bottom: bool,
    
    pub terminal_lines: Vec<String>,
    pub term_scroll: ListState,
    
    pub is_processing: bool,
    pub agent_task: Option<JoinHandle<()>>, 
    
    pub event_tx: mpsc::Sender<AppEvent>,
    pub shell_tx: mpsc::Sender<ShellRequest>, 
    pub spinner_frame: usize,
}

impl App {
    pub fn new(event_tx: mpsc::Sender<AppEvent>, shell_tx: mpsc::Sender<ShellRequest>) -> Self {
        Self {
            mode: AppMode::Chat,
            input_buffer: String::new(),
            messages: vec![ChatMessage { role: MessageRole::System, content: "Ready.".into() }],
            
            chat_scroll: 0,
            chat_stick_to_bottom: true, 
            
            terminal_lines: vec![String::from("--- Shell Connected ---")],
            term_scroll: ListState::default(),
            is_processing: false,
            agent_task: None,
            
            event_tx,
            shell_tx,
            spinner_frame: 0,
        }
    }

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
            KeyCode::Esc if self.is_processing => {
                self.abort_agent();
            }
            
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            KeyCode::PageUp => self.scroll_page(-10),
            KeyCode::PageDown => self.scroll_page(10),
            
            KeyCode::Char(c) if !self.is_processing => self.input_buffer.push(c),
            KeyCode::Backspace if !self.is_processing => { self.input_buffer.pop(); },
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
        self.messages.push(ChatMessage { 
            role: MessageRole::System, 
            content: "🛑 Cancelled by user.".into() 
        });
        self.chat_stick_to_bottom = true;
    }

    pub fn handle_internal_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => if self.is_processing { self.spinner_frame = self.spinner_frame.wrapping_add(1); },
            
            // Handle regular tokens (Answer)
            AppEvent::Token(t) => {
                // If the last message was Thinking, we start a NEW Assistant message
                // If the last message was Assistant, we append.
                let start_new = if let Some(last) = self.messages.last() {
                    !matches!(last.role, MessageRole::Assistant)
                } else {
                    true
                };

                if start_new {
                    self.messages.push(ChatMessage { role: MessageRole::Assistant, content: t });
                } else {
                    if let Some(last) = self.messages.last_mut() {
                        last.content.push_str(&t);
                    }
                }
                self.chat_stick_to_bottom = true;
            }

            // Handle thinking tokens
            AppEvent::Thinking(t) => {
                 let start_new = if let Some(last) = self.messages.last() {
                    !matches!(last.role, MessageRole::Thinking)
                } else {
                    true
                };

                if start_new {
                    self.messages.push(ChatMessage { role: MessageRole::Thinking, content: t });
                } else {
                    if let Some(last) = self.messages.last_mut() {
                        last.content.push_str(&t);
                    }
                }
                self.chat_stick_to_bottom = true;
            }

            AppEvent::CommandStart(c) => {
                self.messages.push(ChatMessage { role: MessageRole::System, content: c });
                self.chat_stick_to_bottom = true;
            },
            AppEvent::TerminalLine(l) => {
                let was_at_bottom = self.term_scroll.selected()
                    .map_or(true, |s| s >= self.terminal_lines.len().saturating_sub(1));

                self.terminal_lines.push(l);
                
                if was_at_bottom {
                    self.term_scroll.select(Some(self.terminal_lines.len().saturating_sub(1)));
                }
            },
            AppEvent::CommandEnd(o) => {
                let s = if o.len() > 100 { format!("Out ({}b) -> Term", o.len()) } else { o };
                self.messages.push(ChatMessage { role: MessageRole::System, content: s });
                self.chat_stick_to_bottom = true;
            },
            AppEvent::AgentFinished => {
                self.is_processing = false;
                self.agent_task = None;
            },
            AppEvent::Error(e) => {
                self.messages.push(ChatMessage { role: MessageRole::Error, content: e });
                self.is_processing = false;
                self.agent_task = None;
                self.chat_stick_to_bottom = true;
            }
        }
    }

    fn submit_message(&mut self) {
        if self.input_buffer.trim().is_empty() { return; }
        let text = self.input_buffer.clone();
        self.input_buffer.clear();

        match self.mode {
            AppMode::Chat => {
                self.is_processing = true;
                self.chat_stick_to_bottom = true; 
                self.messages.push(ChatMessage { role: MessageRole::User, content: text.clone() });
                // We do NOT push an empty Assistant message here anymore, 
                // because the first response might be Thinking.
                
                let tx = self.event_tx.clone();
                let shell = self.shell_tx.clone();
                
                let handle = tokio::spawn(async move {
                    if let Err(e) = run_agent_loop(AgentConfig::default(), text, tx.clone(), shell).await {
                        let _ = tx.send(AppEvent::Error(e.to_string())).await;
                    }
                    let _ = tx.send(AppEvent::AgentFinished).await;
                });
                
                self.agent_task = Some(handle);
            },
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
            self.term_scroll_delta(-1);
        }
    }
    fn scroll_down(&mut self) {
        if let AppMode::Chat = self.mode {
            self.chat_stick_to_bottom = false;
            self.chat_scroll = self.chat_scroll.saturating_add(1);
        } else {
            self.term_scroll_delta(1);
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
            self.term_scroll_delta(amt as i32);
        }
    }
    
    fn term_scroll_delta(&mut self, delta: i32) {
        let i = self.term_scroll.selected().unwrap_or(0) as i32;
        self.term_scroll.select(Some((i + delta).max(0) as usize));
    }
}
