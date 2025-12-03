mod agent;
mod app;
mod docker_setup;
mod shell;
mod ui;

use anyhow::Result;
use app::{App, AppEvent};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use shell::{ShellRequest, ShellSession};
use std::{io, time::Duration};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    docker_setup::ensure_docker_env()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx_app_event, mut rx_app_event) = mpsc::channel::<AppEvent>(100);

    // --- SHELL ACTOR SETUP ---
    let (tx_shell, rx_shell) = mpsc::channel::<ShellRequest>(100);
    let tx_shell_evt = tx_app_event.clone();

    // Spawn the persistent shell in the background
    tokio::spawn(async move {
        ShellSession::run_actor(rx_shell, tx_shell_evt).await;
    });
    // -------------------------

    let (tx_key_event, mut rx_key_event) = mpsc::unbounded_channel();
    std::thread::spawn(move || loop {
        if event::poll(Duration::from_millis(50)).expect("Poll failed") {
            if let Ok(evt) = event::read() {
                if tx_key_event.send(evt).is_err() {
                    break;
                }
            }
        }
    });

    let tx_tick = tx_app_event.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(80));
        loop {
            interval.tick().await;
            if tx_tick.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    let mut app = App::new(tx_app_event.clone(), tx_shell);

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        tokio::select! {
            Some(event) = rx_app_event.recv() => app.handle_internal_event(event),
            Some(event) = rx_key_event.recv() => {
                match event {
                    Event::Key(key) => {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                            _ => app.handle_key_event(key),
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
