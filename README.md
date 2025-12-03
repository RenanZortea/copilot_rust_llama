# Copilot Rust Llama

![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)
![Rust](https://img.shields.io/badge/Made_with-Rust-orange.svg)
![Docker](https://img.shields.io/badge/Environment-Docker-blue)
![Ollama](https://img.shields.io/badge/AI-Ollama-white)

A robust, terminal-based LLM agent written in Rust. This tool acts as an intelligent coding companion that runs alongside your terminal, allowing for safe command execution, file manipulation, and web browsing through a sandboxed environment.

It connects to a local [Ollama](https://ollama.com/) instance and utilizes the **Model Context Protocol (MCP)** to perform actions safely inside a Docker container.

## 🚀 Features

- **Interactive TUI**: A rich Terminal User Interface built with [Ratatui](https://github.com/ratatui/ratatui), featuring split views for Chat and raw Terminal output.
- **Sandboxed Execution**: All shell commands run inside an isolated Docker container (`ollama_dev_env`) to ensure host system safety.
- **Model Context Protocol (MCP)**: Implements a tool server that allows the LLM to:
  - `run_command`: Execute shell commands in the sandbox.
  - `read_file` / `write_file`: Manage files in the workspace.
  - `web_search`: Search the web using DuckDuckGo.
  - `fetch_url`: Scrape and read content from websites.
- **Smart Thinking**: Displays "Thinking" blocks for models that support reasoning (like DeepSeek or Qwen).
- **Persistent Shell**: Maintains a persistent bash session, allowing stateful command execution (e.g., `cd` commands persist).

## 🛠️ Prerequisites

Before running the agent, ensure you have the following installed:

1.  **Rust Toolchain**: [Install Rust](https://www.rust-lang.org/tools/install)
2.  **Docker**: Must be installed and running (used for the sandbox environment).
3.  **Ollama**: [Install Ollama](https://ollama.com/) and pull the default model:
    ```bash
    ollama pull qwen3:8b
    ```
    _(Note: You can change the model in `src/agent.rs` if desired)_.

## 📦 Installation & Usage

### Clone the repository

```bash
git clone https://github.com/renanzortea/copilot_rust_llama.git
cd copilot_rust_llama
```

### Run the setup script

This script checks dependencies, creates your workspace, and builds the agent.

```bash
chmod +x setup.sh
./setup.sh
```

Follow the prompts to select your workspace folder.

### Start the agent

The setup script creates a wrapper called `run_agent.sh` containing your configuration.

```bash
./run_agent.sh
```

**Controls:**

- **Chat Mode**: Type your request and press `Enter`. Use `Alt+Enter` for newlines.
- **Switch Views**: Press `Tab` to toggle between the **Agent Chat** and the **Terminal** view.
- **Scroll**: `Up`/`Down` arrows or `PageUp`/`PageDown`.
- **Exit**: `Ctrl+C`.

## 🏗️ Architecture

- **Agent**: The core logic loops through messages, calling Ollama API, and handling tool calls via MCP.
- **MCP Server**: Acts as the bridge between the LLM and the system, exposing tools like `run_command` and `web_search`.
- **Shell Actor**: Manages the `docker exec` process, handling stdin/stdout streams to provide a real-time shell experience.
- **UI**: Renders the application state using Ratatui, handling input and drawing the chat/terminal widgets.

## 📝 Configuration

Currently, configuration is handled via code constants:

- **Model**: Defaults to `qwen3:8b` in `src/agent.rs`.
- **Ollama URL**: Defaults to `http://localhost:11434/api/chat` in `src/agent.rs`.

## ✅ Todo / Roadmap

- [x] **Json Formatting**: Implemented MCP with JSON-based tool calling.
- [x] **Manual Terminal Control**: Added `ShellSession` and Terminal tab.
- [x] **Better UI**: Implemented TUI with `ratatui`.
- [x] **Asynchronous Operations**: Fully async using `tokio`.
- [x] **Security**: Sandboxed execution via Docker.
- [ ] **Config File**: Move model configuration to a `config.toml` or environment variables.
- [ ] **Syntax Highlighting**: Improve code block rendering in the chat UI.
- [ ] **Session History**: Save and load chat history.
- [ ] **Cloud Models**: Switch to cloud models like Gemini, ChatGPT, or Claude.

## 📄 License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE](LICENSE) file for details.
