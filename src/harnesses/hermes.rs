use std::fs;
use std::io;
use std::time::Duration;

use serde_json::Value;

use crate::config::Config;
use crate::format::{format_cost, format_provider_model, format_tokens};
use crate::fsutil::read_json_object_file;

use super::{AgentKind, AgentSession};

pub fn collect_hermes_session(config: &Config) -> io::Result<Option<AgentSession>> {
    let sessions_path = config.hermes_home.join("sessions").join("sessions.json");
    if !sessions_path.exists() {
        return Ok(None);
    }

    let metadata = fs::metadata(&sessions_path)?;
    let modified = metadata.modified().ok();
    let age = modified
        .and_then(|modified| modified.elapsed().ok())
        .unwrap_or(Duration::MAX);

    let sessions = read_json_object_file(&sessions_path)?;
    let session = sessions.as_object().and_then(|object| {
        object
            .values()
            .filter(|value| value.is_object())
            .max_by(|left, right| {
                hermes_session_updated_at(left).cmp(&hermes_session_updated_at(right))
            })
    });
    let Some(session) = session else {
        return Ok(None);
    };

    let model = session.get("model_override").and_then(|value| {
        format_provider_model(
            value.get("provider").and_then(Value::as_str),
            value.get("model").and_then(Value::as_str),
        )
    });

    let total_tokens = ["input_tokens", "output_tokens", "cache_read_tokens"]
        .iter()
        .filter_map(|key| session.get(*key).and_then(Value::as_u64))
        .sum::<u64>();
    let tokens = if total_tokens > 0 {
        Some(format_tokens(total_tokens))
    } else {
        None
    };

    let cost = session
        .get("estimated_cost_usd")
        .and_then(Value::as_f64)
        .filter(|cost| *cost > 0.0)
        .map(format_cost);

    Ok(Some(AgentSession {
        agent: AgentKind::Hermes,
        path: Some(sessions_path),
        project: None,
        branch: None,
        model,
        surface: Some("Hermes".to_string()),
        activity: None,
        plan: None,
        tokens,
        cost,
        context: None,
        limits: None,
        active: age.as_secs() <= config.stale_seconds,
        started_at: modified,
    }))
}

pub fn hermes_session_updated_at(value: &Value) -> String {
    value
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
