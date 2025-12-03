use crate::app::AppEvent;
use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

const CONTAINER_NAME: &str = "ollama_dev_env";

// Request types for our persistent shell
pub enum ShellRequest {
    // A command meant to generate a response (e.g., from the Agent)
    RunCommand {
        cmd: String,
        response_tx: mpsc::Sender<String>, // Channel to stream output back to the caller
    },
    // A raw input from the user (e.g., typing 'ls' in terminal tab)
    UserInput(String),
}

pub struct ShellSession {
    process: Child,
    stdin: Option<ChildStdin>,
    reader: Arc<Mutex<BufReader<ChildStdout>>>,
    delimiter: String,
}

impl ShellSession {
    fn new_internal() -> Result<Self> {
        let mut process = Command::new("docker")
            .args(["exec", "-i", CONTAINER_NAME, "bash", "-l"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdin"))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout"))?;

        let delimiter = "__END_OF_CMD__".to_string();

        Ok(Self {
            process,
            stdin: Some(stdin),
            reader: Arc::new(Mutex::new(BufReader::new(stdout))),
            delimiter,
        })
    }

    // This is the main loop for the Shell Actor
    pub async fn run_actor(
        mut rx_request: mpsc::Receiver<ShellRequest>,
        tx_app_event: mpsc::Sender<AppEvent>,
    ) {
        let mut session = match Self::new_internal() {
            Ok(s) => s,
            Err(e) => {
                let _ = tx_app_event
                    .send(AppEvent::Error(format!("Failed to start shell: {}", e)))
                    .await;
                return;
            }
        };

        let mut current_responder: Option<mpsc::Sender<String>> = None;

        loop {
            tokio::select! {
                // 1. Handle Incoming Requests (Commands)
                Some(req) = rx_request.recv() => {
                    let cmd_str = match req {
                        ShellRequest::RunCommand { cmd, response_tx } => {
                            current_responder = Some(response_tx);
                            cmd
                        },
                        ShellRequest::UserInput(input) => {
                            current_responder = None; // User input has no specific responder channel
                            input
                        }
                    };

                    if let Some(stdin) = session.stdin.as_mut() {
                        // We wrap the command to echo the delimiter so we know when it ends
                        let full_cmd = format!("{{ {}; }} 2>&1; echo {}\n", cmd_str, session.delimiter);
                        if let Err(e) = stdin.write_all(full_cmd.as_bytes()).await {
                            let _ = tx_app_event.send(AppEvent::Error(format!("Stdin error: {}", e))).await;
                        }
                        let _ = stdin.flush().await;
                    }
                }

                // 2. Handle Outgoing Output (Stream Output)
                // We use a separate async reader loop logic or just poll the reader here.
                // Since we need to read continuously, let's just do a read_line here.
                // NOTE: In a real actor, we might split reader/writer, but for simplicity:
                result = read_next_line(&session.reader) => {
                    match result {
                        Ok(Some(line)) => {
                            // Check for delimiter
                            if line.contains(&session.delimiter) {
                                // Signal end of command to responder if exists
                                current_responder = None;
                            } else {
                                let clean_line = line.trim_end().to_string();

                                // 1. Always send to UI Terminal
                                let _ = tx_app_event.send(AppEvent::TerminalLine(clean_line.clone())).await;

                                // 2. If Agent is listening, send to Agent
                                if let Some(tx) = &current_responder {
                                    let _ = tx.send(clean_line).await;
                                }
                            }
                        }
                        Ok(None) => break, // EOF
                        Err(_) => break,
                    }
                }
            }
        }
    }
}

async fn read_next_line(reader: &Arc<Mutex<BufReader<ChildStdout>>>) -> Result<Option<String>> {
    let mut reader = reader.lock().await;
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await?;
    if bytes == 0 {
        return Ok(None);
    }
    Ok(Some(line))
}
