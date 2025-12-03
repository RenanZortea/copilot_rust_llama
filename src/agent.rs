use crate::app::AppEvent;
use crate::shell::ShellRequest;
use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use tokio::sync::mpsc;

const OLLAMA_URL: &str = "http://localhost:11434/api/generate";
const MAX_LOOPS: usize = 15;

pub struct AgentConfig {
    pub model: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "qwen3:8b".to_string(), 
        }
    }
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
    done: bool,
    #[serde(default)]
    context: Option<Vec<i64>>,
}

#[derive(Deserialize, Serialize, Debug)]
struct FileOp {
    path: String,
    content: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct ToolAction {
    thought: Option<String>, 
    command: Option<String>,
    write_file: Option<FileOp>,
    path: Option<String>,
    content: Option<String>,
}

struct StreamFilter {
    full_buffer: String,     // Keeps history for JSON parsing
    parse_buffer: String,    // Keeps active text for tag detection
    in_think_block: bool,
}

impl StreamFilter {
    fn new() -> Self {
        Self {
            full_buffer: String::new(),
            parse_buffer: String::new(),
            in_think_block: false,
        }
    }

    fn push(&mut self, chunk: &str) -> Vec<(bool, String)> {
        self.full_buffer.push_str(chunk);
        self.parse_buffer.push_str(chunk);
        
        let mut events = Vec::new();
        
        // Loop to process all complete tags in the buffer
        loop {
            if self.in_think_block {
                // Looking for closing tag </think>
                match self.parse_buffer.find("</think>") {
                    Some(idx) => {
                        // Found it! Emit everything before it as thinking
                        let content = self.parse_buffer[..idx].to_string();
                        if !content.is_empty() {
                            events.push((true, content));
                        }
                        // Remove the tag and everything before it
                        self.parse_buffer = self.parse_buffer[idx + 8..].to_string();
                        self.in_think_block = false;
                    },
                    None => {
                        // No closing tag yet.
                        // We must be careful not to flush a partial closing tag like "</thi"
                        // The max length of the tag is 8 ("</think>").
                        // Keep the last 7 chars in the buffer just in case.
                        let safe_len = self.parse_buffer.len().saturating_sub(7);
                        if safe_len > 0 {
                            let safe_chunk = self.parse_buffer[..safe_len].to_string();
                            events.push((true, safe_chunk));
                            self.parse_buffer = self.parse_buffer[safe_len..].to_string();
                        }
                        break;
                    }
                }
            } else {
                // Looking for opening tag <think>
                match self.parse_buffer.find("<think>") {
                    Some(idx) => {
                        // Found start! Emit everything before it as normal token
                        let content = self.parse_buffer[..idx].to_string();
                        if !content.is_empty() {
                            events.push((false, content));
                        }
                        // Remove the tag and everything before it
                        self.parse_buffer = self.parse_buffer[idx + 7..].to_string();
                        self.in_think_block = true;
                    },
                    None => {
                        // No opening tag yet.
                        // Keep potential partial tag like "<thi"
                        // <think> is 7 chars. Keep last 6.
                        let safe_len = self.parse_buffer.len().saturating_sub(6);
                        if safe_len > 0 {
                            let safe_chunk = self.parse_buffer[..safe_len].to_string();
                            events.push((false, safe_chunk));
                            self.parse_buffer = self.parse_buffer[safe_len..].to_string();
                        }
                        break;
                    }
                }
            }
        }
        
        events
    }
}

fn extract_json_candidate(input: &str) -> Option<String> {
    let starts: Vec<_> = input.match_indices('{').map(|(i, _)| i).collect();
    if starts.is_empty() { return None; }

    for start in starts.into_iter().rev() {
        let mut depth = 0;
        let mut end_idx = None;
        
        for (i, c) in input[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = Some(start + i);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(end) = end_idx {
            let candidate = &input[start..=end];
            if candidate.contains("\"command\"") || candidate.contains("\"write_file\"") || candidate.contains("\"path\"") {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

pub async fn run_agent_loop(
    config: AgentConfig,
    initial_prompt: String,
    tx_app: mpsc::Sender<AppEvent>,
    tx_shell: mpsc::Sender<ShellRequest>,
) -> Result<()> {
    let client = Client::new();
    let mut current_prompt = initial_prompt;
    let mut context: Option<Vec<i64>> = None;
    let mut loop_count = 0;

    let system_prompt = r#"
    You are an AI Agent.
    TOOLS:
    1. "write_file": { "path": "filename", "content": "file content" }
    2. "command": "shell command string"

    INSTRUCTIONS:
    1. First, THINK about the problem using <think>...</think> tags.
    2. Then, output a SINGLE JSON object with your action.
    
    EXAMPLE:
    <think>
    I need to check the current directory.
    </think>
    {
        "command": "ls -la"
    }
    "#;

    loop {
        if loop_count >= MAX_LOOPS {
            tx_app.send(AppEvent::Error("Max loops.".into())).await?;
            break;
        }
        loop_count += 1;

        let request_body = json!({
            "model": config.model,
            "prompt": current_prompt,
            "context": context,
            "system": system_prompt,
            "stream": true,
        });

        let mut stream = client
            .post(OLLAMA_URL)
            .json(&request_body)
            .send()
            .await?
            .bytes_stream();

        let mut filter = StreamFilter::new();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            if let Ok(json_resp) = serde_json::from_slice::<OllamaResponse>(&chunk) {
                let events = filter.push(&json_resp.response);
                for (is_thinking, text) in events {
                    if is_thinking {
                        tx_app.send(AppEvent::Thinking(text)).await?;
                    } else {
                        tx_app.send(AppEvent::Token(text)).await?;
                    }
                }
                if json_resp.done { context = json_resp.context; }
            }
        }

        let json_candidate = extract_json_candidate(&filter.full_buffer).unwrap_or("{}".to_string());
        let raw_value: serde_json::Value = serde_json::from_str(&json_candidate).unwrap_or(json!({}));
        
        let mut final_path = None;
        let mut final_content = None;
        let mut final_cmd = None;

        if let Some(obj) = raw_value.get("write_file").and_then(|v| v.as_object()) {
            if let (Some(p), Some(c)) = (obj.get("path").and_then(|v| v.as_str()), obj.get("content").and_then(|v| v.as_str())) {
                final_path = Some(p.to_string());
                final_content = Some(c.to_string());
            }
        }
        
        if final_path.is_none() {
             if let Some(path_str) = raw_value.get("write_file").and_then(|v| v.as_str()) {
                 if let Some(content_str) = raw_value.get("content").and_then(|v| v.as_str()) {
                     final_path = Some(path_str.to_string());
                     final_content = Some(content_str.to_string());
                 }
             }
        }
        
        if let Some(cmd_str) = raw_value.get("command").and_then(|v| v.as_str()) {
            final_cmd = Some(cmd_str.to_string());
        }

        if let (Some(path), Some(content)) = (final_path, final_content) {
            tx_app.send(AppEvent::CommandStart(format!("Writing {}", path))).await?;
            let target_path = Path::new("workspace").join(&path);
            if let Some(parent) = target_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            match tokio::fs::write(&target_path, &content).await {
                Ok(_) => {
                    tx_app.send(AppEvent::CommandEnd(format!("Wrote {}", path))).await?;
                    current_prompt = format!("System: Written {}.", path);
                }
                Err(e) => {
                    tx_app.send(AppEvent::Error(e.to_string())).await?;
                    current_prompt = format!("System Error: {}", e);
                }
            }
        } else if let Some(cmd) = final_cmd {
            let clean = cmd.trim();
            if clean.is_empty() || clean == "null" { break; }

            tx_app.send(AppEvent::CommandStart(cmd.clone())).await?;
            let (resp_tx, mut resp_rx) = mpsc::channel(100);
            tx_shell.send(ShellRequest::RunCommand { cmd: cmd.clone(), response_tx: resp_tx }).await?;

            let mut output = String::new();
            while let Some(line) = resp_rx.recv().await {
                output.push_str(&line);
                output.push('\n');
            }
            if output.len() > 5000 { output = format!("{}\n[Truncated]", &output[..5000]); }

            tx_app.send(AppEvent::CommandEnd(output.clone())).await?;
            current_prompt = format!("Output:\n{}\nDone?", output);
        } else {
            if filter.full_buffer.trim().is_empty() {
                 tx_app.send(AppEvent::Error("Empty response from agent".to_string())).await?;
                 break;
            }
             break;
        }
    }
    Ok(())
}
