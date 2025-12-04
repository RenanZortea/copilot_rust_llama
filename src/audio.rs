use anyhow::Result;
use reqwest::Client;
use std::io::Cursor;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde_json::json;

pub struct AudioPlayer {
    client: Client,
    endpoint: String,
    enabled: bool,
}

impl AudioPlayer {
    pub fn new(endpoint: String, enabled: bool) -> Self {
        Self {
            client: Client::new(),
            endpoint,
            enabled,
        }
    }

    pub async fn play_text(&self, text: &str) -> Result<()> {
        if !self.enabled || text.trim().is_empty() {
            return Ok(());
        }

        // Clone for the async move block
        let client = self.client.clone();
        let endpoint = self.endpoint.clone();
        let text = text.to_string();

        // Spawn logic to avoid blocking the main thread
        tokio::spawn(async move {
            // 1. Fetch Audio from Python Server
            let res = match client.post(&endpoint).json(&json!({ "text": text })).send().await {
                Ok(r) => r,
                Err(_) => return, // Fail silently if server is down
            };

            if !res.status().is_success() {
                return;
            }

            let audio_bytes = match res.bytes().await {
                Ok(b) => b,
                Err(_) => return,
            };

            let bytes_vec = audio_bytes.to_vec();

            // 2. Play Audio in a blocking thread (Rodio is CPU/IO intensive)
            tokio::task::spawn_blocking(move || {
                // Try to initialize audio device
                // This might fail if ALSA headers are missing or no audio device is present
                match OutputStream::try_default() {
                    Ok((_stream, stream_handle)) => {
                        if let Ok(sink) = Sink::try_new(&stream_handle) {
                            let cursor = Cursor::new(bytes_vec);
                            if let Ok(source) = Decoder::new(cursor) {
                                sink.append(source);
                                sink.sleep_until_end();
                            }
                        }
                    }
                    Err(_) => {
                        // Failed to find audio device (e.g. headless server)
                        // Silent fail
                    }
                }
            });
        });

        Ok(())
    }
}
