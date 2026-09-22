#[cfg(not(windows))]
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::config::{
    Config, DEFAULT_CLAUDE_CLIENT_ID, DEFAULT_OPENCODE_CLIENT_ID, DEFAULT_PI_CLIENT_ID,
};
use crate::harnesses::AgentKind;

pub fn validate_client_id(client_id: &str) -> io::Result<()> {
    if client_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing Discord client_id for detected agent; set config.json",
        ));
    }

    Ok(())
}

pub fn client_id_for_agent<'a>(config: &'a Config, agent: AgentKind) -> &'a str {
    match agent {
        AgentKind::Claude => {
            client_id_or_default(&config.claude_client_id, DEFAULT_CLAUDE_CLIENT_ID)
        }
        AgentKind::OpenCode => {
            client_id_or_default(&config.opencode_client_id, DEFAULT_OPENCODE_CLIENT_ID)
        }
        AgentKind::Pi => client_id_or_default(&config.pi_client_id, DEFAULT_PI_CLIENT_ID),
        AgentKind::OhMyPi => client_id_or_default(&config.omp_client_id, DEFAULT_PI_CLIENT_ID),
        AgentKind::Hermes => client_id_or_default(&config.hermes_client_id, &config.client_id),
        _ => &config.client_id,
    }
}

pub fn client_id_or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

pub fn clear_all_presences(config: &Config) -> io::Result<()> {
    clear_presences(config, None)
}

pub fn clear_other_presences(config: &Config, keep_client_id: &str) -> io::Result<()> {
    clear_presences(config, Some(keep_client_id))
}

pub fn clear_presences(config: &Config, keep_client_id: Option<&str>) -> io::Result<()> {
    let mut ids = vec![
        config.client_id.as_str(),
        client_id_or_default(&config.opencode_client_id, DEFAULT_OPENCODE_CLIENT_ID),
        client_id_or_default(&config.pi_client_id, DEFAULT_PI_CLIENT_ID),
    ];

    ids.sort_unstable();
    ids.dedup();

    let mut last_error = None;
    let mut cleared_any = false;

    for client_id in ids {
        if client_id.trim().is_empty() {
            continue;
        }
        if keep_client_id == Some(client_id) {
            continue;
        }

        match clear_presence(client_id) {
            Ok(()) => cleared_any = true,
            Err(error) => last_error = Some(error),
        }
    }

    if cleared_any || last_error.is_none() {
        Ok(())
    } else {
        Err(last_error.unwrap())
    }
}

pub fn clear_presence(client_id: &str) -> io::Result<()> {
    let mut discord = DiscordIpc::connect(client_id)?;
    discord.handshake(client_id)?;
    discord.set_activity(None)
}

pub struct DiscordIpc {
    stream: File,
}

impl DiscordIpc {
    pub fn connect(_client_id: &str) -> io::Result<Self> {
        for path in discord_ipc_paths() {
            if let Ok(stream) = OpenOptions::new().read(true).write(true).open(&path) {
                return Ok(Self { stream });
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Discord IPC pipe not found; start Discord desktop first",
        ))
    }

    pub fn handshake(&mut self, client_id: &str) -> io::Result<()> {
        self.send(
            0,
            &format!(r#"{{"v":1,"client_id":"{}"}}"#, json_escape(client_id)),
        )
    }

    pub fn set_activity(&mut self, activity: Option<String>) -> io::Result<()> {
        let activity = activity.unwrap_or_else(|| "null".to_string());
        self.send(
            1,
            &format!(
                r#"{{"cmd":"SET_ACTIVITY","args":{{"pid":{},"activity":{}}},"nonce":"codex-{}"}}"#,
                std::process::id(),
                activity,
                unix_millis()
            ),
        )
    }

    fn send(&mut self, opcode: u32, payload: &str) -> io::Result<()> {
        let bytes = payload.as_bytes();
        self.stream.write_all(&opcode.to_le_bytes())?;
        self.stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
        self.stream.write_all(bytes)?;
        self.stream.flush()
    }
}

pub fn discord_ipc_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    {
        for index in 0..10 {
            paths.push(PathBuf::from(format!(r"\\.\pipe\discord-ipc-{index}")));
        }
    }

    #[cfg(not(windows))]
    {
        let roots = [
            env::var("XDG_RUNTIME_DIR").ok(),
            env::var("TMPDIR").ok(),
            Some("/tmp".to_string()),
        ];

        for root in roots.into_iter().flatten() {
            for index in 0..10 {
                paths.push(PathBuf::from(format!("{root}/discord-ipc-{index}")));
            }
        }
    }

    paths
}

pub fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn json_escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());

    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch.is_control() => output.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => output.push(ch),
        }
    }

    output
}
