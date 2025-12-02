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

pub struct AgentConfig {
    pub model: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "gemma3".to_string(), // Or llama3, mistral
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

// The structure we expect the AI to output
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
    let mut shell = ShellSession::new()?; // Start persistent shell

    let mut current_prompt = initial_prompt;
    let mut context: Option<Vec<i64>> = None;

    // We force JSON mode in the system prompt
    let system_prompt = r#"
    You are an AI Agent with a persistent shell.
    You MUST respond using strictly valid JSON format.
    
    Format:
    {
        "thought": "Reasoning about what to do...",
        "command": "shell_command_to_run"
    }

    If you don't need to run a command, set "command" to null.
    The shell state is persistent (cd works).
    "#;

    // Loop for multi-turn Agent actions
    loop {
        let request_body = json!({
            "model": config.model,
            "prompt": current_prompt,
            "context": context,
            "system": system_prompt,
            "stream": true,
            "format": "json" // Force Ollama to enforce JSON
        });

        let mut stream = client
            .post(OLLAMA_URL)
            .json(&request_body)
            .send()
            .await?
            .bytes_stream();

        let mut full_json_buffer = String::new();

        // 1. Stream the raw JSON to the UI (so user sees it typing)
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

        // 2. Parse the accumulated JSON
        let action: ToolAction = match serde_json::from_str(&full_json_buffer) {
            Ok(a) => a,
            Err(_) => {
                // Fallback: sometimes models write text before JSON or mess up.
                // For now, we report error and stop.
                tx.send(AppEvent::Error("Failed to parse AI JSON response.".into()))
                    .await?;
                break;
            }
        };

        // 3. Execute Command if present
        if let Some(cmd) = action.command {
            if cmd.trim().is_empty() {
                break;
            }

            tx.send(AppEvent::CommandStart(cmd.clone())).await?;

            // Run in Persistent Shell
            let mut output = shell.run_command(&cmd).await?;

            // 4. Truncate Output
            if output.len() > MAX_OUTPUT_CHARS {
                output = format!(
                    "{}\n\n... [Output Truncated: showing first {} chars] ...",
                    &output[..MAX_OUTPUT_CHARS],
                    MAX_OUTPUT_CHARS
                );
            }

            tx.send(AppEvent::CommandEnd(output.clone())).await?;

            // Feed output back to AI
            current_prompt = format!("Command Output:\n{}", output);
        } else {
            // No command means the AI is done talking
            break;
        }
    }

    Ok(())
}
