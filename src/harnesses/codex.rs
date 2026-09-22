use std::fs;
use std::io;
use std::path::PathBuf;

use serde_json::Value;

use crate::config::Config;
use crate::detect::{contains_agent_process, running_process_text};
use crate::format::{
    activity_from_event, estimate_cost, format_limits, format_model_name, format_plan,
    format_tokens, parse_token_usage, surface_from_meta, TokenUsage,
};
use crate::fsutil::{git_branch_for_path, newest_jsonl, read_head, read_tail, MAX_TAIL_BYTES};

use super::{AgentKind, AgentSession};

pub fn collect_codex_session(config: &Config) -> io::Result<Option<AgentSession>> {
    if !codex_activity_is_live(config) {
        return Ok(None);
    }

    let sessions_dir = config.codex_home.join("sessions");
    let Some(path) = newest_jsonl(&sessions_dir)? else {
        return Ok(None);
    };

    let metadata = fs::metadata(&path)?;

    let head = read_head(&path, 64 * 1024)?;
    let tail = read_tail(&path, MAX_TAIL_BYTES)?;
    let parsed = parse_session_text(&format!("{head}\n{tail}"));
    let cwd = parsed.cwd;
    let project = cwd
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.trim().is_empty());
    let branch = cwd.as_deref().and_then(git_branch_for_path);

    Ok(Some(AgentSession {
        agent: AgentKind::Codex,
        path: Some(path),
        project,
        branch,
        model: parsed.model,
        surface: parsed.surface,
        activity: parsed.activity,
        plan: parsed.plan,
        tokens: parsed.tokens,
        cost: parsed.cost,
        context: parsed.context,
        limits: parsed.limits,
        active: true,
        started_at: metadata.modified().ok(),
    }))
}

pub fn codex_activity_is_live(config: &Config) -> bool {
    if !config.detect_processes {
        return true;
    }

    running_process_text()
        .map(|processes| {
            let lower = processes.to_ascii_lowercase();
            contains_agent_process(
                &lower,
                &[
                    "@openai/codex",
                    "openai\\codex",
                    "openai/codex",
                    "codex.exe",
                    "codex.cmd",
                    "codex ",
                ],
            )
        })
        .unwrap_or(true)
}

#[derive(Default)]
pub struct ParsedSession {
    pub cwd: Option<PathBuf>,
    pub model: Option<String>,
    pub surface: Option<String>,
    pub activity: Option<String>,
    pub plan: Option<String>,
    pub tokens: Option<String>,
    pub cost: Option<String>,
    pub context: Option<String>,
    pub limits: Option<String>,
}

pub fn parse_session_text(raw: &str) -> ParsedSession {
    let mut parsed = ParsedSession::default();
    let mut total_usage: Option<TokenUsage> = None;
    let mut last_usage: Option<TokenUsage> = None;
    let mut model_context_window: Option<u64> = None;

    for line in raw.lines().rev() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if parsed.cwd.is_none() {
            parsed.cwd = value
                .get("payload")
                .and_then(|payload| payload.get("cwd"))
                .and_then(Value::as_str)
                .map(PathBuf::from);
        }

        if parsed.model.is_none() {
            parsed.model = value
                .get("payload")
                .and_then(|payload| payload.get("model"))
                .and_then(Value::as_str)
                .map(format_model_name);
        }

        if parsed.surface.is_none()
            && value.get("type").and_then(Value::as_str) == Some("session_meta")
        {
            parsed.surface = value.get("payload").map(surface_from_meta);
        }

        if parsed.activity.is_none() {
            parsed.activity = activity_from_event(&value);
        }

        if value
            .get("payload")
            .and_then(|payload| payload.get("type"))
            .and_then(Value::as_str)
            == Some("token_count")
        {
            if parsed.plan.is_none() {
                parsed.plan = value
                    .pointer("/payload/rate_limits/plan_type")
                    .and_then(Value::as_str)
                    .map(format_plan);
            }

            if parsed.limits.is_none() {
                parsed.limits = format_limits(
                    value
                        .get("payload")
                        .and_then(|payload| payload.get("rate_limits")),
                );
            }

            if total_usage.is_none() {
                total_usage = value
                    .pointer("/payload/info/total_token_usage")
                    .map(parse_token_usage);
            }

            if last_usage.is_none() {
                last_usage = value
                    .pointer("/payload/info/last_token_usage")
                    .map(parse_token_usage);
            }

            if model_context_window.is_none() {
                model_context_window = value
                    .pointer("/payload/info/model_context_window")
                    .and_then(Value::as_u64);
            }
        }
    }

    if let Some(usage) = total_usage {
        parsed.tokens = Some(format_tokens(usage.total_tokens));
        parsed.cost = parsed
            .model
            .as_deref()
            .and_then(|model| estimate_cost(model, usage));
    }

    if let (Some(usage), Some(window)) = (last_usage, model_context_window) {
        if window > 0 {
            let percent = ((usage.input_tokens as f64 / window as f64) * 100.0).round() as u64;
            parsed.context = Some(format!("Ctx {}% used", percent.min(100)));
        }
    }

    parsed
}
