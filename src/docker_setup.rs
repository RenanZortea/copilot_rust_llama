use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CONTAINER_NAME: &str = "ollama_dev_env";

// Helper to get the workspace path from ENV or default
pub fn get_workspace_path() -> PathBuf {
    match env::var("LLM_AGENT_WORKSPACE") {
        Ok(p) => PathBuf::from(p),
        Err(_) => PathBuf::from("./workspace"),
    }
}

pub fn ensure_docker_env() -> Result<()> {
    let workspace_path = get_workspace_path();

    // 1. Create the workspace directory locally if it doesn't exist
    if !workspace_path.exists() {
        fs::create_dir_all(&workspace_path)?;
        println!("Created local workspace directory at {:?}", workspace_path);
    }

    // We need the absolute path for Docker volume mounting
    let abs_workspace = fs::canonicalize(&workspace_path)?;

    // 2. Check if container is already running
    let status = Command::new("docker")
        .args([
            "ps",
            "--filter",
            &format!("name={}", CONTAINER_NAME),
            "--format",
            "{{.Names}}",
        ])
        .output()?;

    let output = String::from_utf8_lossy(&status.stdout);
    let is_running = output.trim() == CONTAINER_NAME;

    if !is_running {
        // 3. Clean up any stopped container with the same name
        let _ = Command::new("docker")
            .args(["rm", "-f", CONTAINER_NAME])
            .output();

        println!("Starting Docker Sandbox mapped to: {:?}", abs_workspace);

        // 4. Run the container
        // We use the absolute path resolved above
        let status = Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(CONTAINER_NAME)
            .arg("-v")
            .arg(format!("{}:/workspace", abs_workspace.to_string_lossy()))
            .arg("-w")
            .arg("/workspace")
            .arg("ubuntu:latest")
            .args(["tail", "-f", "/dev/null"])
            .status()?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to start Docker container. Is Docker running?"
            ));
        }
        println!("Docker Sandbox started successfully!");
    }

    // 5. Check if Rust/Cargo is installed
    // We check via 'bash -l -c' to ensure we load the path if it was just installed
    let cargo_check = Command::new("docker")
        .args([
            "exec",
            CONTAINER_NAME,
            "bash",
            "-l",
            "-c",
            "cargo --version",
        ])
        .output();

    let needs_install = match cargo_check {
        Ok(out) => !out.status.success(),
        Err(_) => true,
    };

    if needs_install {
        println!("Installing Basic Tools + Rust inside Docker... (This runs once)");

        // This command installs:
        // 1. curl, git, vim, wget, nano
        // 2. build-essential (gcc/cc) -> CRITICAL for 'cargo run' to link binaries
        // 3. Rust (via rustup)
        let install_cmd = "apt-get update && \
                           apt-get install -y curl git vim nano wget build-essential && \
                           curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y";

        let setup = Command::new("docker")
            .args(["exec", CONTAINER_NAME, "bash", "-c", install_cmd])
            .status()?;

        if !setup.success() {
            eprintln!("Warning: Failed to install tools inside Docker.");
        } else {
            println!("Tools installed successfully.");
        }
    } else {
        println!("Docker environment is ready (Rust is installed).");
    }

    Ok(())
}
