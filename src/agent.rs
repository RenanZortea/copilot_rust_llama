use crate::app::{AppEvent, MessageRole};
use crate::config::Config;
use crate::mcp::{McpRequest, ToolDefinition};
use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};

const MAX_LOOPS: usize = 10;

// --- ROBUST SYSTEM PROMPT ---
const AGENT_SYSTEM_PROMPT: &str = r#"
You are Agerus, an expert software development agent running in a secure Docker sandbox.

# CRITICAL OPERATIONAL RULES:
1. **CONTEXT IS KING**: 
   - You have NO magic knowledge of the user's files.
   - ALWAYS run `list_files` to explore the directory structure first.
   - ALWAYS run `read_file` to see file content before editing.
   - ALWAYS run `consult_documentation` to see the documentation for the language.

2. **SANDBOXED ENVIRONMENT**:
   - You are running inside a Docker container.
   - You can safely run destructive commands (rm, etc) if necessary.
   - You cannot open GUI applications.

3. **THINK BEFORE ACTING**:
   - Before calling a tool, briefly explain your plan.
   - If a tool fails, analyze the error and try a different approach.

4. **FORMATTING**:
   - When writing code, return the full file content if the file is small.
   - For large files, ensure you have read them first to avoid overwriting content blindly.
"#;

// --- Ollama API Structures ---

#[derive(Deserialize, Debug)]
struct ChatResponse {
    message: Option<Message>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Message {
    content: Option<String>,
    thinking: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ToolCall {
    function: ToolFunction,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ToolFunction {
    name: String,
    arguments: serde_json::Value,
}

// --- The Agent Logic ---

pub async fn run_agent_loop(
    config: Config,
    history: Vec<crate::app::ChatMessage>,
    app_tx: mpsc::Sender<AppEvent>,
    mcp_tx: mpsc::Sender<McpRequest>,
) -> Result<()> {
    // 1. Fetch Tools from MCP Server
    let (tx, rx) = oneshot::channel();
    if let Err(e) = mcp_tx.send(McpRequest::ListTools(tx)).await {
        app_tx
            .send(AppEvent::Error(format!("Failed to contact MCP: {}", e)))
            .await?;
        return Ok(());
    }

    let tools: Vec<ToolDefinition> = match rx.await {
        Ok(t) => t,
        Err(_) => {
            app_tx
                .send(AppEvent::Error("MCP Server dropped connection".into()))
                .await?;
            return Ok(());
        }
    };

    let ollama_tools: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema
                }
            })
        })
        .collect();

    // 2. CONSTRUCT MESSAGE HISTORY
    // We start with the forceful system prompt, then append the user's history.
    let mut messages = vec![json!({
        "role": "system",
        "content": AGENT_SYSTEM_PROMPT
    })];

    let history_json: Vec<serde_json::Value> = history
        .iter()
        .map(|msg| {
            let role = match msg.role {
                MessageRole::User => "user",
                // Thinking blocks are internal UI states, usually mapped to assistant for context
                MessageRole::Assistant | MessageRole::Thinking => "assistant",
                // System logs in UI (like "File saved") are mapped to user or system. 
                // Mapping to "user" often helps the model see it as an observation.
                MessageRole::System | MessageRole::Error => "system",
            };
            json!({ "role": role, "content": msg.content })
        })
        .collect();
    
    messages.extend(history_json);

    let client = Client::new();
    let mut loops = 0;

    loop {
        if loops >= MAX_LOOPS {
            break;
        }
        loops += 1;

        // Try with tools first
        let mut body = json!({
            "model": config.model,
            "messages": messages,
            "tools": ollama_tools,
            "stream": true
        });

        let mut res = client.post(&config.ollama_url).json(&body).send().await;

        // --- Fallback Logic ---
        if let Ok(ref response) = res {
            if response.status() == reqwest::StatusCode::BAD_REQUEST {
                app_tx.send(AppEvent::Thinking(format!(
                    "Model '{}' rejected tools. Falling back to text-only mode.", 
                    config.model
                ))).await?;

                body = json!({
                    "model": config.model,
                    "messages": messages,
                    "stream": true
                });
                
                res = client.post(&config.ollama_url).json(&body).send().await;
            }
        }

        match res {
            Err(e) => {
                app_tx
                    .send(AppEvent::Error(format!("Ollama Connection Error: {}", e)))
                    .await?;
                break;
            }
            Ok(response) => {
                if !response.status().is_success() {
                    let text = response.text().await.unwrap_or_default();
                    app_tx
                        .send(AppEvent::Error(format!("Ollama API Error: {}", text)))
                        .await?;
                    break;
                }

                let mut stream = response.bytes_stream();
                let mut buffer = String::new();
                let mut full_content = String::new();
                let mut buffer_tools = Vec::new();
                let mut parsing_thought = false;

                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Err(e) => {
                            app_tx
                                .send(AppEvent::Error(format!("Stream Error: {}", e)))
                                .await?;
                            break;
                        }
                        Ok(chunk) => {
                            if let Ok(s) = std::str::from_utf8(&chunk) {
                                buffer.push_str(s);
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].to_string();
                                    buffer.drain(..=pos);

                                    if line.trim().is_empty() {
                                        continue;
                                    }

                                    match serde_json::from_str::<ChatResponse>(&line) {
                                        Ok(resp) => {
                                            if let Some(err) = resp.error {
                                                app_tx
                                                    .send(AppEvent::Error(format!(
                                                        "Ollama Error: {}",
                                                        err
                                                    )))
                                                    .await?;
                                            }

                                            if let Some(msg) = resp.message {
                                                // Handle native thinking fields
                                                if let Some(think) = msg.thinking {
                                                    if !think.is_empty() {
                                                        app_tx.send(AppEvent::Thinking(think)).await?;
                                                    }
                                                } else if let Some(reason) = msg.reasoning_content {
                                                    if !reason.is_empty() {
                                                        app_tx.send(AppEvent::Thinking(reason)).await?;
                                                    }
                                                }

                                                if let Some(content) = msg.content {
                                                    if !content.is_empty() {
                                                        let mut text = content.clone();
                                                        
                                                        // Parse <think> tags if model outputs them in content
                                                        if text.contains("<think>") {
                                                            parsing_thought = true;
                                                            text = text.replace("<think>", "");
                                                        }

                                                        if text.contains("</think>") {
                                                            parsing_thought = false;
                                                            let parts: Vec<&str> =
                                                                text.split("</think>").collect();
                                                            if let Some(t) = parts.first() {
                                                                if !t.is_empty() {
                                                                    app_tx.send(AppEvent::Thinking(t.to_string())).await?;
                                                                }
                                                            }
                                                            if parts.len() > 1 {
                                                                let c = parts[1];
                                                                if !c.is_empty() {
                                                                    full_content.push_str(c);
                                                                    app_tx.send(AppEvent::Token(c.to_string())).await?;
                                                                }
                                                            }
                                                            continue;
                                                        }

                                                        if parsing_thought {
                                                            app_tx.send(AppEvent::Thinking(text)).await?;
                                                        } else {
                                                            full_content.push_str(&text);
                                                            app_tx.send(AppEvent::Token(text)).await?;
                                                        }
                                                    }
                                                }
                                                if let Some(calls) = msg.tool_calls {
                                                    buffer_tools.extend(calls);
                                                }
                                            }
                                        }
                                        Err(_) => {}
                                    }
                                }
                            }
                        }
                    }
                }

                if buffer_tools.is_empty() {
                    break;
                }

                messages.push(json!({ "role": "assistant", "content": full_content, "tool_calls": buffer_tools }));

                for tool in &buffer_tools {
                    let (tx, rx) = oneshot::channel();
                    app_tx
                        .send(AppEvent::CommandStart(format!(
                            "{}(...)",
                            tool.function.name
                        )))
                        .await?;

                    if let Err(e) = mcp_tx
                        .send(McpRequest::CallTool {
                            name: tool.function.name.clone(),
                            arguments: tool.function.arguments.clone(),
                            response_tx: tx,
                        })
                        .await
                    {
                        app_tx
                            .send(AppEvent::Error(format!("Failed to call tool: {}", e)))
                            .await?;
                        break;
                    }

                    let result = match rx.await {
                        Ok(Ok(out)) => out,
                        Ok(Err(e)) => format!("Tool Execution Error: {}", e),
                        Err(_) => "Tool Execution Panicked".to_string(),
                    };

                    app_tx.send(AppEvent::CommandEnd(result.clone())).await?;
                    messages.push(json!({ "role": "tool", "content": result }));
                }
            }
        }
    }

    Ok(())
}
