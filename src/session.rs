use crate::app::ChatMessage;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new() -> Self {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("agerus");
        path.push("sessions");

        // Ensure directory exists
        if let Err(e) = fs::create_dir_all(&path) {
            eprintln!("Warning: Failed to create session directory: {}", e);
        }

        Self { sessions_dir: path }
    }

    pub fn save_session(&self, name: &str, messages: &Vec<ChatMessage>) -> Result<String> {
        let path = self.sessions_dir.join(format!("{}.json", name));
        let json = serde_json::to_string_pretty(messages)?;
        fs::write(&path, json)?;
        Ok(format!("Saved to {:?}", path))
    }

    pub fn load_session(&self, name: &str) -> Result<Vec<ChatMessage>> {
        let path = self.sessions_dir.join(format!("{}.json", name));
        if !path.exists() {
            return Err(anyhow!("Session file not found: {:?}", path));
        }
        let content = fs::read_to_string(path)?;
        let messages: Vec<ChatMessage> = serde_json::from_str(&content)?;
        Ok(messages)
    }

    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();
        if self.sessions_dir.exists() {
            for entry in fs::read_dir(&self.sessions_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        sessions.push(stem.to_string());
                    }
                }
            }
        }
        sessions.sort();
        Ok(sessions)
    }
}
