use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

const CONTAINER_NAME: &str = "ollama_dev_env";

pub struct ShellSession {
    process: Child,
    stdin: Option<ChildStdin>,
    reader: Arc<Mutex<BufReader<ChildStdout>>>,
    delimiter: String,
}

impl ShellSession {
    pub fn new() -> Result<Self> {
        // INSTEAD of running "bash", we run "docker exec -i ..."
        // -i: Keep STDIN open so we can pipe commands to it
        let mut process = Command::new("docker")
            .args(["exec", "-i", CONTAINER_NAME, "bash"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()) // Merge stderr into stdout
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

    pub async fn run_command(&mut self, cmd: &str) -> Result<String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("Shell stdin is closed"))?;

        // We echo the delimiter to know when the command is done
        let full_cmd = format!("{}; echo {}\n", cmd, self.delimiter);

        stdin.write_all(full_cmd.as_bytes()).await?;
        stdin.flush().await?;

        let mut output = String::new();
        let mut reader = self.reader.lock().await;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).await?;
            if bytes == 0 {
                break;
            }

            if line.trim().contains(&self.delimiter) {
                break;
            }

            output.push_str(&line);
        }

        Ok(output.trim().to_string())
    }
}
