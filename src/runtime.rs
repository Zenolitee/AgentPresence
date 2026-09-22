use std::io;
use std::thread;
use std::time::Duration;

use crate::config::{Config, PRIORITY_POLL_SECONDS};
use crate::detect::clear_first_seen_cache;
use crate::discord::{
    clear_all_presences, clear_other_presences, client_id_for_agent, validate_client_id, DiscordIpc,
};
use crate::harnesses::collect_session;
use crate::presence::activity_payload;
use crate::tui;

pub fn run_loop(config: &Config) -> io::Result<()> {
    let mut current_client_id = String::new();
    let mut discord: Option<DiscordIpc> = None;

    loop {
        let session = collect_session(config)?;
        if let Some(session) = session.as_ref().filter(|session| session.active) {
            let client_id = client_id_for_agent(config, session.agent);
            validate_client_id(client_id)?;

            if current_client_id != client_id {
                if let Some(ref mut connection) = discord {
                    let _ = connection.set_activity(None);
                }
                let _ = clear_other_presences(config, client_id);
                let mut next = DiscordIpc::connect(client_id)?;
                next.handshake(client_id)?;
                discord = Some(next);
                current_client_id.clear();
                current_client_id.push_str(client_id);
            }

            if let Some(ref mut connection) = discord {
                if let Err(error) =
                    connection.set_activity(Some(activity_payload(config, Some(session))))
                {
                    eprintln!("discord publish failed: {error}; reconnecting");
                    let mut next = DiscordIpc::connect(client_id)?;
                    next.handshake(client_id)?;
                    *connection = next;
                    current_client_id.clear();
                    current_client_id.push_str(client_id);
                }
            }
        } else {
            if let Some(ref mut connection) = discord {
                if let Err(error) = connection.set_activity(None) {
                    eprintln!("discord clear failed: {error}; clearing all presences");
                    let _ = clear_all_presences(config);
                } else {
                    let _ = clear_all_presences(config);
                }
            } else {
                let _ = clear_all_presences(config);
            }

            discord = None;
            current_client_id.clear();
            clear_first_seen_cache();
        }

        let poll_seconds = if config.priority_presence {
            config.poll_seconds.min(PRIORITY_POLL_SECONDS).max(1)
        } else {
            config.poll_seconds.max(1)
        };
        thread::sleep(Duration::from_secs(poll_seconds));
    }
}

pub fn run_tui_loop(config: Config) -> io::Result<()> {
    let state = std::sync::Arc::new(parking_lot::Mutex::new(tui::AppState {
        config: config.clone(),
        current_session: None,
        discord_connected: false,
        running: true,
    }));

    let state_clone = state.clone();
    let handle = thread::spawn(move || -> io::Result<()> {
        let mut current_client_id = String::new();
        let mut discord: Option<DiscordIpc> = None;

        loop {
            // Check if TUI quit
            {
                let guard = state_clone.lock();
                if !guard.running {
                    break;
                }
            }

            // Read config from shared state (picks up toggle changes). The
            // lock is released immediately: never hold it across the slow
            // multi-process scan or Discord pipe I/O, or the TUI's snapshot
            // blocks behind us and quit appears to hang.
            let config = state_clone.lock().config.clone();
            let session = collect_session(&config)?;

            if let Some(session) = session.as_ref().filter(|session| session.active) {
                let client_id = client_id_for_agent(&config, session.agent).to_string();
                validate_client_id(&client_id)?;

                if current_client_id != client_id {
                    if let Some(ref mut connection) = discord {
                        let _ = connection.set_activity(None);
                    }
                    let _ = clear_other_presences(&config, &client_id);
                    let mut next = DiscordIpc::connect(&client_id)?;
                    next.handshake(&client_id)?;
                    discord = Some(next);
                    current_client_id.clear();
                    current_client_id.push_str(&client_id);
                }

                if let Some(ref mut connection) = discord {
                    let payload = activity_payload(&config, Some(session));
                    if let Err(error) = connection.set_activity(Some(payload)) {
                        eprintln!("discord publish failed: {error}; reconnecting");
                        let mut next = DiscordIpc::connect(&client_id)?;
                        next.handshake(&client_id)?;
                        *connection = next;
                        current_client_id.clear();
                        current_client_id.push_str(&client_id);
                    }
                }

                // Update shared state with current session
                {
                    let mut guard = state_clone.lock();
                    guard.current_session = Some(session.clone());
                    guard.discord_connected = true;
                }
            } else {
                if let Some(ref mut connection) = discord {
                    if let Err(error) = connection.set_activity(None) {
                        eprintln!("discord clear failed: {error}; clearing all presences");
                    }
                }
                let _ = clear_all_presences(&config);

                discord = None;
                current_client_id.clear();
                clear_first_seen_cache();

                // Update shared state
                {
                    let mut guard = state_clone.lock();
                    guard.current_session = None;
                    guard.discord_connected = false;
                }
            }

            let poll_seconds = if config.priority_presence {
                config.poll_seconds.min(PRIORITY_POLL_SECONDS).max(1)
            } else {
                config.poll_seconds.max(1)
            };
            thread::sleep(Duration::from_secs(poll_seconds));
        }

        Ok(())
    });

    // Run TUI on main thread
    let result = tui::run_tui(config, state.clone());

    // Signal presence loop to stop
    {
        let mut guard = state.lock();
        guard.running = false;
    }

    let _ = handle.join();
    result
}
