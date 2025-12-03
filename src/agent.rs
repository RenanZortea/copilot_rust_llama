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
            model: "gemma3".to_string(),
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
    thought: String,
    command: Option<String>,
    write_file: Option<FileOp>,
}

struct StreamFilter {
    full_buffer: String,
    last_thought_len: usize,
}

impl StreamFilter {
    fn new() -> Self {
        Self {
            full_buffer: String::new(),
            last_thought_len: 0,
        }
    }

    fn push(&mut self, chunk: &str) -> Option<String> {
        self.full_buffer.push_str(chunk);
        if let Some(start_idx) = self.full_buffer.find("\"thought\": \"") {
            let content_start = start_idx + 12;
            let mut current_len = 0;
            let mut escape = false;
            let chars: Vec<char> = self.full_buffer[content_start..].chars().collect();

            for c in chars {
                if escape {
                    escape = false;
                    current_len += 1;
                    continue;
                }
                if c == '\\' {
                    escape = true;
                    current_len += 1;
                    continue;
                }
                if c == '"' {
                    break;
                }
                current_len += 1;
            }

            if current_len > self.last_thought_len {
                let new_part = &self.full_buffer
                    [content_start + self.last_thought_len..content_start + current_len];
                self.last_thought_len = current_len;
                return Some(new_part.replace("\\n", "\n").replace("\\\"", "\""));
            }
        }
        None
    }
}

// --- NEW HELPER: Extract JSON from Markdown/Text ---
fn clean_json(input: &str) -> Option<&str> {
    let start = input.find('{')?;
    let end = input.rfind('}')?;
    if start > end {
        return None;
    }
    Some(&input[start..=end])
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
    You are an AI Agent with a persistent shell inside a Docker container.
    The current working directory is /workspace.
    
    TOOLS:
    1. "write_file": Create/Overwrite files. NO 'cat >'.
    2. "command": Run shell commands. 'cat' allowed for reading.

    Format (JSON):
    {
        "thought": "Reasoning...",
        "command": "ls -la",
        "write_file": null
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
            "format": "json"
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
                if let Some(clean_text) = filter.push(&json_resp.response) {
                    tx_app.send(AppEvent::Token(clean_text)).await?;
                }
                if json_resp.done {
                    context = json_resp.context;
                }
            }
        }

        // --- FIXED PARSING LOGIC ---
        let json_candidate = clean_json(&filter.full_buffer).unwrap_or("{}");

        let action: ToolAction = match serde_json::from_str(json_candidate) {
            Ok(a) => a,
            Err(e) => {
                // If it fails, log the raw output to help debug, then stop
                let err_msg = format!("JSON Error: {}. Raw: {}", e, filter.full_buffer);
                tx_app.send(AppEvent::Error(err_msg)).await?;
                break;
            }
        };

        if let Some(file_op) = action.write_file {
            tx_app
                .send(AppEvent::CommandStart(format!("Writing {}", file_op.path)))
                .await?;
            let target_path = Path::new("workspace").join(&file_op.path);
            if let Some(parent) = target_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            match tokio::fs::write(&target_path, &file_op.content).await {
                Ok(_) => {
                    tx_app
                        .send(AppEvent::CommandEnd(format!("Wrote {}", file_op.path)))
                        .await?;
                    current_prompt = format!("System: Written {}.", file_op.path);
                }
                Err(e) => {
                    tx_app.send(AppEvent::Error(e.to_string())).await?;
                    current_prompt = format!("System Error: {}", e);
                }
            }
        } else if let Some(cmd) = action.command {
            let clean = cmd.trim();
            if clean.is_empty() || clean == "null" {
                break;
            }

            tx_app.send(AppEvent::CommandStart(cmd.clone())).await?;

            let (resp_tx, mut resp_rx) = mpsc::channel(100);

            tx_shell
                .send(ShellRequest::RunCommand {
                    cmd: cmd.clone(),
                    response_tx: resp_tx,
                })
                .await?;

            let mut output = String::new();
            while let Some(line) = resp_rx.recv().await {
                output.push_str(&line);
                output.push('\n');
            }

            if output.len() > 5000 {
                output = format!("{}\n[Truncated]", &output[..5000]);
            }

            tx_app.send(AppEvent::CommandEnd(output.clone())).await?;
            current_prompt = format!("Output:\n{}\nDone?", output);
        } else {
            break;
        }
    }
    Ok(())
}
