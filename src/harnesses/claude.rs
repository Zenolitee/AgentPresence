use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::detect::{contains_agent_process, running_process_text};
use crate::format::{format_model_name, format_tokens};
use crate::fsutil::{git_branch_for_path, home_dir, newest_matching_file, read_json_object_file};

use super::{AgentKind, AgentSession};

/// Read ~/.claude/settings.json and extract the configured model display name.
/// Claude Code stores shorthands like "opus[1m]" or "sonnet" — we map these
/// to friendly names ("Opus 4.8", "Sonnet 4.8", etc.).
pub fn claude_configured_model(claude_dir: &Path) -> Option<String> {
    let settings_path = claude_dir.join("settings.json");
    let raw = fs::read_to_string(&settings_path).ok()?;
    let settings: Value = serde_json::from_str(&raw).ok()?;
    let model_str = settings.get("model")?.as_str()?;

    // Strip context-window suffix like "[1m]", "[200k]"
    let base = model_str
        .split('[')
        .next()
        .unwrap_or(model_str)
        .trim()
        .to_ascii_lowercase();

    let display = match base.as_str() {
        // Current Claude model shorthands
        "opus" | "claude-opus-4-8" | "claude-opus-4-8-20250901" => "Opus 4.8",
        "sonnet" | "claude-sonnet-4-8" | "claude-sonnet-4-8-20250901" => "Sonnet 4.8",
        "haiku" | "claude-haiku-3-5" | "claude-haiku-3-5-20241022" => "Haiku 3.5",
        // Legacy shorthands
        "opus-3" | "claude-3-opus-20240229" => "Opus 3",
        "sonnet-3" | "claude-3-sonnet-20240229" => "Sonnet 3",
        "haiku-3" | "claude-3-haiku-20240307" => "Haiku 3",
        // If it looks like a full API model name, reuse format_model_name
        s if s.starts_with("claude-") => return Some(format_model_name(s)),
        // Unknown shorthand — show it as-is but capitalized
        other => {
            let mut c = other.chars();
            match c.next() {
                None => return None,
                Some(f) => {
                    let capitalized: String = f.to_uppercase().collect::<String>() + c.as_str();
                    return Some(capitalized);
                }
            }
        }
    };

    Some(display.to_string())
}

pub fn collect_claude_session() -> io::Result<Option<AgentSession>> {
    let user_home = home_dir()?;
    let claude_dir = user_home.join(".claude");
    let sessions_dir = claude_dir.join("sessions");

    // Find the Claude Code PID from running processes
    let processes = running_process_text()?;
    let lower = processes.to_ascii_lowercase();
    if !contains_agent_process(&lower, &["claude", "@anthropic-ai/claude-code"]) {
        return Ok(None);
    }

    // Find the session file (PID.json)
    let Some(session_entry) = newest_matching_file(&sessions_dir, "", ".json")? else {
        return Ok(None);
    };

    let session_data: Value = read_json_object_file(&session_entry)?;
    let session_id = session_data
        .get("sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let cwd = session_data
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let version = session_data
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let project = cwd
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.trim().is_empty());
    let branch = cwd.as_deref().and_then(git_branch_for_path);

    // Read the project session JSONL for usage data
    let mut model: Option<String> = version;
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;

    if !session_id.is_empty() {
        // Find the project directory from cwd
        if let Some(cwd_path) = cwd.as_ref() {
            let project_key = cwd_path
                .to_str()
                .map(|s| s.replace(':', "").replace('\\', "--").replace('/', "--"))
                .unwrap_or_default();
            let project_dir = claude_dir.join("projects").join(&project_key);
            let session_jsonl = project_dir.join(format!("{session_id}.jsonl"));
            if session_jsonl.exists() {
                let raw = fs::read_to_string(&session_jsonl)?;
                for line in raw.lines() {
                    if let Ok(entry) = serde_json::from_str::<Value>(line) {
                        if entry.get("type").and_then(|v| v.as_str()) == Some("assistant") {
                            if let Some(msg) = entry.get("message") {
                                // Get model from the last assistant message
                                if let Some(m) = msg.get("model").and_then(|v| v.as_str()) {
                                    model = Some(format_model_name(m));
                                }
                                // Accumulate usage
                                if let Some(usage) = msg.get("usage") {
                                    total_input_tokens += usage
                                        .get("input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    total_input_tokens += usage
                                        .get("cache_read_input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    total_output_tokens += usage
                                        .get("output_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Prefer the model from settings.json (what the user configured) over the
    // JSONL model (which is the resolved/actual model from the API response).
    // This matters when using a proxy that remaps models (e.g. Opus → mimo).
    if let Some(configured) = claude_configured_model(&claude_dir) {
        model = Some(configured);
    }

    let total_tokens = total_input_tokens + total_output_tokens;
    let tokens = if total_tokens > 0 {
        Some(format_tokens(total_tokens))
    } else {
        None
    };

    // Estimate cost: ~$3/M input, ~$15/M output for Claude
    let cost = if total_input_tokens > 0 || total_output_tokens > 0 {
        let input_cost = total_input_tokens as f64 * 3.0 / 1_000_000.0;
        let output_cost = total_output_tokens as f64 * 15.0 / 1_000_000.0;
        let total_cost = input_cost + output_cost;
        Some(format!("${:.2}", total_cost))
    } else {
        None
    };

    let metadata = fs::metadata(&session_entry)?;
    let modified = metadata.modified().ok();

    // Active if the process is running (already checked at top of function)

    Ok(Some(AgentSession {
        agent: AgentKind::Claude,
        path: Some(session_entry),
        project,
        branch,
        model,
        surface: Some("Claude Code".to_string()),
        activity: None,
        plan: None,
        tokens,
        cost,
        context: None,
        limits: None,
        active: true,
        started_at: modified,
    }))
}
