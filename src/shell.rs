use crate::app::AppEvent;
use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, Mutex};

const CONTAINER_NAME: &str = "ollama_dev_env";

pub struct ShellSession {
    process: Child,
    stdin: Option<ChildStdin>,
    reader: Arc<Mutex<BufReader<ChildStdout>>>,
    delimiter: String,
}

impl ShellSession {
    pub fn new() -> Result<Self> {
        let mut process = Command::new("docker")
            .args(["exec", "-i", CONTAINER_NAME, "bash"])
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

    /// Runs a command, streams output to the UI, and returns the full output string.
    pub async fn run_command(&mut self, cmd: &str, tx: &mpsc::Sender<AppEvent>) -> Result<String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("Shell stdin is closed"))?;

        // Redirect stderr to stdout using { ... } 2>&1
        // This ensures we see errors and progress bars (like curl)
        let full_cmd = format!("{{ {}; }} 2>&1; echo {}\n", cmd, self.delimiter);

        stdin.write_all(full_cmd.as_bytes()).await?;
        stdin.flush().await?;

        let mut output_buffer = String::new();
        let mut reader = self.reader.lock().await;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).await?;
            if bytes == 0 {
                break; // EOF
            }

            if line.contains(&self.delimiter) {
                break;
            }

            // Stream to UI
            let trimmed_line = line.trim_end().to_string();
            tx.send(AppEvent::TerminalLine(trimmed_line)).await?;

            output_buffer.push_str(&line);
        }

        Ok(output_buffer.trim().to_string())
    }
}
