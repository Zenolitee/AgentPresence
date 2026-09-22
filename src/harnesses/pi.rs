use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde_json::Value;

use crate::config::Config;
use crate::detect::{contains_agent_process, running_process_text};
use crate::format::{format_cost, format_provider_model, format_tokens};
use crate::fsutil::{
    git_branch_for_path, newest_jsonl, parse_iso8601_utc, read_head, read_tail, MAX_TAIL_BYTES,
};

use super::{AgentKind, AgentSession};

pub fn collect_pi_session(config: &Config) -> io::Result<Option<AgentSession>> {
    if !pi_activity_is_live(config) {
        return Ok(None);
    }

    let sessions_dir = config.pi_home.join("agent").join("sessions");
    let Some(path) = newest_jsonl(&sessions_dir)? else {
        return Ok(None);
    };

    let metadata = fs::metadata(&path)?;
    let modified = metadata.modified().ok();
    let age = modified
        .and_then(|modified| modified.elapsed().ok())
        .unwrap_or(Duration::MAX);

    let tail = read_tail(&path, MAX_TAIL_BYTES)?;
    let raw = if metadata.len() <= MAX_TAIL_BYTES {
        tail
    } else {
        let head = read_head(&path, 64 * 1024)?;
        format!("{head}\n{tail}")
    };
    let parsed = parse_pi_session_text(&raw);
    let cwd = parsed.cwd;
    let project = cwd
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.trim().is_empty());
    let branch = cwd.as_deref().and_then(git_branch_for_path);

    Ok(Some(AgentSession {
        agent: AgentKind::Pi,
        path: Some(path),
        project,
        branch,
        model: parsed.model,
        surface: Some("Pi".to_string()),
        activity: parsed.activity,
        plan: None,
        tokens: parsed.tokens,
        cost: parsed.cost,
        context: None,
        limits: None,
        active: age.as_secs() <= config.stale_seconds,
        started_at: modified,
    }))
}

pub fn pi_activity_is_live(config: &Config) -> bool {
    if !config.detect_processes {
        return true;
    }

    running_process_text()
        .map(|processes| {
            let lower = processes.to_ascii_lowercase();
            contains_agent_process(
                &lower,
                &["pi.ai", "inflection", "pi desktop", "pi-node", "\\pi.exe"],
            )
        })
        .unwrap_or(false)
}

#[derive(Default)]
pub struct ParsedPiSession {
    pub cwd: Option<PathBuf>,
    pub model: Option<String>,
    pub activity: Option<String>,
    pub tokens: Option<String>,
    pub cost: Option<String>,
    pub started_at: Option<SystemTime>,
}

pub fn parse_pi_session_text(raw: &str) -> ParsedPiSession {
    let mut parsed = ParsedPiSession::default();
    let mut total_tokens = 0_u64;
    let mut total_cost = 0_f64;

    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if parsed.cwd.is_none() {
            parsed.cwd = value.get("cwd").and_then(Value::as_str).map(PathBuf::from);
        }

        if parsed.started_at.is_none() {
            parsed.started_at = value
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(parse_iso8601_utc);
        }

        if value.get("type").and_then(Value::as_str) == Some("model_change") {
            parsed.model = format_pi_model(
                value.get("provider").and_then(Value::as_str),
                value.get("modelId").and_then(Value::as_str),
            );
        }

        if let Some(message) = value.get("message") {
            if let Some(model) = format_pi_model(
                message.get("provider").and_then(Value::as_str),
                message.get("model").and_then(Value::as_str),
            ) {
                parsed.model = Some(model);
            }

            if let Some(activity) = pi_activity_from_message(message) {
                parsed.activity = Some(activity);
            }

            if let Some(usage) = message.get("usage") {
                total_tokens = total_tokens.saturating_add(
                    usage
                        .get("totalTokens")
                        .or_else(|| usage.get("total_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                );
                total_cost += usage
                    .pointer("/cost/total")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
            }
        }
    }

    if total_tokens > 0 {
        parsed.tokens = Some(format_tokens(total_tokens));
    }

    if total_cost > 0.0 {
        parsed.cost = Some(format_cost(total_cost));
    }

    parsed
}

pub fn format_pi_model(provider: Option<&str>, model: Option<&str>) -> Option<String> {
    format_provider_model(provider, model)
}

pub fn pi_activity_from_message(message: &Value) -> Option<String> {
    let content = message.get("content")?.as_array()?;

    for item in content.iter().rev() {
        let item_type = item.get("type").and_then(Value::as_str);
        let name = item.get("name").and_then(Value::as_str);

        if item_type == Some("toolCall") {
            return Some(match name {
                Some("bash") | Some("shell") => "Running terminal command".to_string(),
                Some("edit") => "Editing files".to_string(),
                Some("write") => "Writing files".to_string(),
                Some("read") => "Reading files".to_string(),
                Some("Agent") => "Running subagent".to_string(),
                Some(other) => format!("Using {other}"),
                None => "Using tools".to_string(),
            });
        }
    }

    None
}
