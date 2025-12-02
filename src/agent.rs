use crate::app::AppEvent;
use crate::shell::ShellSession;
use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;

const OLLAMA_URL: &str = "http://localhost:11434/api/generate";
const MAX_OUTPUT_CHARS: usize = 2000;
const MAX_LOOPS: usize = 15;

pub struct AgentConfig {
    pub model: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "gemma3".to_string(), // Ensure this matches your model
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
struct ToolAction {
    thought: String,
    command: Option<String>,
}

pub async fn run_agent_loop(
    config: AgentConfig,
    initial_prompt: String,
    tx: mpsc::Sender<AppEvent>,
) -> Result<()> {
    let client = Client::new();
    let mut shell = ShellSession::new()?;

    let mut current_prompt = initial_prompt;
    let mut context: Option<Vec<i64>> = None;
    let mut loop_count = 0;

    let system_prompt = r#"
    You are an AI Agent with a persistent shell inside a Docker container.
    You MUST respond using strictly valid JSON format.
    
    Format:
    {
        "thought": "Reasoning...",
        "command": "shell_command_to_run"
    }

    Rules:
    1. If you don't need to run a command, set "command": null.
    2. If the user asks a question, answer in "thought" and set "command": null.
    3. The shell is persistent.
    "#;

    loop {
        if loop_count >= MAX_LOOPS {
            tx.send(AppEvent::Error(
                "Max iterations reached. Stopping loop.".into(),
            ))
            .await?;
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

        let mut full_json_buffer = String::new();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            if let Ok(json_resp) = serde_json::from_slice::<OllamaResponse>(&chunk) {
                tx.send(AppEvent::Token(json_resp.response.clone())).await?;
                full_json_buffer.push_str(&json_resp.response);
                if json_resp.done {
                    context = json_resp.context;
                }
            }
        }

        let action: ToolAction = match serde_json::from_str(&full_json_buffer) {
            Ok(a) => a,
            Err(_) => {
                tx.send(AppEvent::Error("Failed to parse JSON. Stopping.".into()))
                    .await?;
                break;
            }
        };

        if let Some(cmd) = action.command {
            if cmd.trim().is_empty() {
                break;
            }

            tx.send(AppEvent::CommandStart(cmd.clone())).await?;

            // Pass 'tx' here for live streaming!
            let mut output = shell.run_command(&cmd, &tx).await?;

            if output.len() > MAX_OUTPUT_CHARS {
                output = format!("{}\n\n... [Truncated] ...", &output[..MAX_OUTPUT_CHARS]);
            }

            tx.send(AppEvent::CommandEnd(output.clone())).await?;

            current_prompt = format!(
                "Command Output:\n{}\n\nIs the task done? If yes, respond with command: null.",
                output
            );
        } else {
            break;
        }
    }

    Ok(())
}
