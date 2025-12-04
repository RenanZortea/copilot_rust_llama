use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub model: String,
    pub workspace_path: PathBuf,
    pub ollama_url: String,
    // --- New Config ---
    #[serde(default = "default_voice_url")]
    pub voice_server_url: String,
    #[serde(default)]
    pub voice_enabled: bool,
}

fn default_voice_url() -> String {
    "http://127.0.0.1:5000/tts".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: "qwen2.5-coder:latest".to_string(),
            workspace_path: PathBuf::from("./workspace"),
            ollama_url: "http://localhost:11434/api/chat".to_string(),
            voice_server_url: default_voice_url(),
            voice_enabled: false, // Off by default
        }
    }
}

impl Config {
    fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .join("agerus");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(config_dir.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config at {:?}", config_path))?;

            let config: Config =
                toml::from_str(&content).with_context(|| "Failed to parse config.toml")?;

            return Ok(config);
        }

        Ok(Config::default())
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        Ok(())
    }
}
