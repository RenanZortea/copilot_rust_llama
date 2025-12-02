use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

const CONTAINER_NAME: &str = "ollama_dev_env";
const WORKSPACE_DIR: &str = "./workspace";

pub fn ensure_docker_env() -> Result<()> {
    // 1. Create the workspace directory locally if it doesn't exist
    if !Path::new(WORKSPACE_DIR).exists() {
        fs::create_dir(WORKSPACE_DIR)?;
        println!("Created local workspace directory at {}", WORKSPACE_DIR);
    }

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

    if output.trim() == CONTAINER_NAME {
        // Already running
        return Ok(());
    }

    // 3. Clean up any stopped container with the same name
    let _ = Command::new("docker")
        .args(["rm", "-f", CONTAINER_NAME])
        .output();

    println!("Starting Docker Sandbox...");

    // 4. Run the container
    let current_dir = std::env::current_dir()?;
    let abs_workspace = current_dir.join("workspace");

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

    // 5. NEW: Install basic tools (curl, git, etc.) immediately after start
    println!("Installing basic tools (curl, git, vim)... this may take a moment.");
    let setup = Command::new("docker")
        .args([
            "exec",
            CONTAINER_NAME,
            "bash",
            "-c",
            "apt-get update && apt-get install -y curl git vim nano wget",
        ])
        .status()?;

    if !setup.success() {
        eprintln!("Warning: Failed to install basic tools.");
    }

    println!("Docker Sandbox started successfully!");
    Ok(())
}
