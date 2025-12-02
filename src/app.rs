use crate::agent::{run_agent_loop, AgentConfig};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

// Messages to display in the UI
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

// Events passed from the Async Agent back to the UI
pub enum AppEvent {
    Token(String),        // A chunk of text from AI
    AgentFinished,        // AI is done thinking
    CommandStart(String), // AI is running a command
    CommandEnd(String),   // Command finished
    Error(String),
}

pub struct App {
    pub input_buffer: String,
    pub messages: Vec<ChatMessage>,
    pub is_processing: bool,
    pub list_state: ListState,
    pub event_tx: mpsc::Sender<AppEvent>, // Channel to send events to self
}

impl App {
    pub fn new(event_tx: mpsc::Sender<AppEvent>) -> Self {
        Self {
            input_buffer: String::new(),
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: "Welcome to Ollama Terminal. Ask me to do something!".into(),
            }],
            is_processing: false,
            list_state: ListState::default(),
            event_tx,
        }
    }

    /// Handle keyboard inputs
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if self.is_processing {
            return; // Lock input while thinking
        }

        match key.code {
            KeyCode::Char(c) => self.input_buffer.push(c),
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => self.submit_message(),
            _ => {}
        }
    }

    /// Handle events coming from the Agent thread
    pub fn handle_internal_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Token(token) => {
                // Append token to the last message (assuming it's the assistant)
                if let Some(last) = self.messages.last_mut() {
                    last.content.push_str(&token);
                }
            }
            AppEvent::CommandStart(cmd) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!(">> Executing: {}", cmd),
                });
            }
            AppEvent::CommandEnd(output) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!(">> Result:\n{}", output),
                });
                // Prepare a new empty message for the AI's follow-up explanation
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
                    content: err,
                });
                self.is_processing = false;
            }
        }
    }

    fn submit_message(&mut self) {
        if self.input_buffer.trim().is_empty() {
            return;
        }

        let user_text = self.input_buffer.clone();
        self.input_buffer.clear();
        self.is_processing = true;

        // Add user message to UI
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: user_text.clone(),
        });

        // Add an empty Assistant message (placeholder for streaming)
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
        });

        // Spawn the Agent Logic in the background
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let config = AgentConfig::default();
            if let Err(e) = run_agent_loop(config, user_text, tx.clone()).await {
                let _ = tx.send(AppEvent::Error(e.to_string())).await;
            }
            let _ = tx.send(AppEvent::AgentFinished).await;
        });
    }
}
