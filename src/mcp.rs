use crate::shell::ShellRequest;
use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};

// --- MCP Protocol Definitions ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug)]
pub enum McpRequest {
    ListTools(oneshot::Sender<Vec<ToolDefinition>>),
    CallTool {
        name: String,
        arguments: serde_json::Value,
        response_tx: oneshot::Sender<Result<String>>,
    },
}

// --- The Server Actor ---

pub struct McpServer {
    shell_tx: mpsc::Sender<ShellRequest>,
    http_client: reqwest::Client,
    workspace_root: PathBuf, // New field to store the configured workspace
}

impl McpServer {
    pub async fn start(shell_tx: mpsc::Sender<ShellRequest>) -> mpsc::Sender<McpRequest> {
        let (tx, mut rx) = mpsc::channel(32);
        
        // Determine workspace root once at startup
        let workspace_root = match env::var("LLM_AGENT_WORKSPACE") {
            Ok(p) => PathBuf::from(p),
            Err(_) => PathBuf::from("./workspace"), // Fallback relative path
        };

        let mut server = Self { 
            shell_tx,
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            workspace_root,
        };

        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                server.handle_request(req).await;
            }
        });

        tx
    }

    async fn handle_request(&mut self, req: McpRequest) {
        match req {
            McpRequest::ListTools(resp_tx) => {
                let tools = vec![
                    ToolDefinition {
                        name: "run_command".into(),
                        description: "Run a shell command in the Docker container.".into(),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "command": { "type": "string", "description": "Command to run" }
                            },
                            "required": ["command"]
                        }),
                    },
                    ToolDefinition {
                        name: "write_file".into(),
                        description: "Write content to a file in the workspace.".into(),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Relative path" },
                                "content": { "type": "string", "description": "Content" }
                            },
                            "required": ["path", "content"]
                        }),
                    },
                    ToolDefinition {
                        name: "read_file".into(),
                        description: "Read a file.".into(),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Path" }
                            },
                            "required": ["path"]
                        }),
                    },
                    ToolDefinition {
                        name: "list_files".into(),
                        description: "List files in a directory.".into(),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Directory path" }
                            }
                        }),
                    },
                    ToolDefinition {
                        name: "fetch_url".into(),
                        description: "Fetch and read a URL (web browsing).".into(),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL" }
                            },
                            "required": ["url"]
                        }),
                    },
                    ToolDefinition {
                        name: "web_search".into(),
                        description: "Search the web (DuckDuckGo). Returns title and URL.".into(),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Search query" }
                            },
                            "required": ["query"]
                        }),
                    },
                ];
                let _ = resp_tx.send(tools);
            }
            McpRequest::CallTool { name, arguments, response_tx } => {
                let result = self.execute_tool(name, arguments).await;
                let _ = response_tx.send(result);
            }
        }
    }

    async fn execute_tool(&self, name: String, args: serde_json::Value) -> Result<String> {
        match name.as_str() {
            "run_command" => {
                let cmd = args.get("command").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'command'"))?;
                let (tx, mut rx) = mpsc::channel(100);
                self.shell_tx.send(ShellRequest::RunCommand { cmd: cmd.to_string(), response_tx: tx }).await?;
                let mut output = String::new();
                while let Some(chunk) = rx.recv().await { output.push_str(&chunk); output.push('\n'); }
                if output.len() > 5000 { output = format!("{}\n...[Output Truncated]", &output[..5000]); }
                Ok(output)
            }
            "write_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let content = args.get("content").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing content"))?;
                
                // Join with the configured workspace root
                let target = self.workspace_root.join(path);
                
                if let Some(p) = target.parent() { tokio::fs::create_dir_all(p).await?; }
                tokio::fs::write(&target, content).await?;
                Ok(format!("Successfully wrote to {}", path))
            }
            "read_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                
                // Join with the configured workspace root
                let target = self.workspace_root.join(path);
                
                if !target.exists() { return Ok(format!("File not found: {}", path)); }
                let content = tokio::fs::read_to_string(target).await?;
                if content.lines().count() > 300 {
                   let preview: String = content.lines().take(300).collect::<Vec<_>>().join("\n");
                   Ok(format!("{}\n... [File too long, first 300 lines shown]", preview))
                } else { Ok(content) }
            }
            "list_files" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                
                // Join with the configured workspace root
                let target = self.workspace_root.join(path_str);
                
                // Safety check: Ensure target exists
                if !target.exists() {
                     return Ok(format!("Directory not found: {}", path_str));
                }

                let mut entries = tokio::fs::read_dir(target).await?;
                let mut list = Vec::new();
                while let Some(entry) = entries.next_entry().await? {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let meta = entry.metadata().await?;
                    let type_str = if meta.is_dir() { "DIR" } else { "FILE" };
                    list.push(format!("[{}] {}", type_str, name));
                }
                Ok(list.join("\n"))
            }
            "fetch_url" => {
                let url = args.get("url").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing url"))?;
                let resp = self.http_client.get(url).send().await?.text().await?;
                
                let re_script = Regex::new(r"(?si)<script.*?>.*?</script>").unwrap();
                let re_style = Regex::new(r"(?si)<style.*?>.*?</style>").unwrap();
                let re_tags = Regex::new(r"<[^>]*>").unwrap();
                let re_whitespace = Regex::new(r"\s+").unwrap();
                let no_script = re_script.replace_all(&resp, "");
                let no_style = re_style.replace_all(&no_script, "");
                let clean_tags = re_tags.replace_all(&no_style, " ");
                let clean_text = re_whitespace.replace_all(&clean_tags, " ");
                let text = clean_text.trim().to_string();
                if text.len() > 8000 { Ok(format!("{}\n...[Webpage truncated]", &text[..8000])) } else { Ok(text) }
            }
            "web_search" => {
                let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing query"))?;
                
                let params = [("q", query)];
                let resp = self.http_client.post("https://html.duckduckgo.com/html/")
                    .form(&params)
                    .send().await?
                    .text().await?;

                let document = Html::parse_document(&resp);
                let link_selector = Selector::parse(".result__a").unwrap();
                let mut results = Vec::new();

                for element in document.select(&link_selector).take(10) {
                    let title = element.text().collect::<Vec<_>>().join(" ");
                    if let Some(href) = element.value().attr("href") {
                        if href.starts_with("http") {
                             results.push(format!("Title: {}\nURL: {}\n", title.trim(), href));
                        }
                    }
                }

                if results.is_empty() {
                    Ok("No results found.".to_string())
                } else {
                    Ok(results.join("\n---\n"))
                }
            }
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }
    }
}
