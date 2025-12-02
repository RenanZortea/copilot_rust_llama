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
    // -d: Detached (background)
    // -v: Volume mount (Local PWD/workspace -> /workspace)
    // -w: Working directory
    // tail -f /dev/null: Keeps the container running forever so we can exec into it
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
        .arg("ubuntu:latest") // You can switch this to python:3.9 or node:18
        .args(["tail", "-f", "/dev/null"])
        .status()?;

    if !status.success() {
        return Err(anyhow!(
            "Failed to start Docker container. Is Docker running?"
        ));
    }

    println!("Docker Sandbox started successfully!");
    Ok(())
}
