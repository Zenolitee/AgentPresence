use serde_json::Value;

pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M tok", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K tok", tokens as f64 / 1_000.0)
    } else {
        format!("{tokens} tok")
    }
}

pub fn format_provider_model(_provider: Option<&str>, model: Option<&str>) -> Option<String> {
    let model = model?.trim();
    if model.is_empty() {
        return None;
    }

    Some(model.rsplit('/').next().unwrap_or(model).trim().to_string())
}

pub fn format_model_name(model: &str) -> String {
    let trimmed = model.trim();
    let fast = trimmed.ends_with("-fast");
    let base = trimmed.strip_suffix("-fast").unwrap_or(trimmed);

    // Known model name mappings
    let display = match base {
        // Claude models
        "claude-opus-4-8" | "claude-opus-4-8-20250901" => "Opus 4.8",
        "claude-sonnet-4-8" | "claude-sonnet-4-8-20250901" => "Sonnet 4.8",
        "claude-haiku-3-5" | "claude-haiku-3-5-20241022" => "Haiku 3.5",
        "claude-3-5-sonnet-20241022" => "Sonnet 3.5",
        "claude-3-5-haiku-20241022" => "Haiku 3.5",
        "claude-3-opus-20240229" => "Opus 3",
        "claude-3-sonnet-20240229" => "Sonnet 3",
        "claude-3-haiku-20240307" => "Haiku 3",
        // OpenAI models
        s if s.starts_with("gpt-4o") => "GPT-4o",
        s if s.starts_with("gpt-4-turbo") => "GPT-4 Turbo",
        s if s.starts_with("gpt-4") => "GPT-4",
        s if s.starts_with("gpt-3.5") => "GPT-3.5",
        s if s.starts_with("o1") => "o1",
        s if s.starts_with("o3") => "o3",
        s if s.starts_with("codex") && s.contains("mini") => "Codex Mini",
        s if s.starts_with("codex") => "Codex",
        // Google models
        s if s.starts_with("gemini-2") => "Gemini 2",
        s if s.starts_with("gemini-1") => "Gemini 1",
        // Anthropic other
        s if s.starts_with("claude-") => {
            // Generic fallback: strip prefix and format
            let rest = s.strip_prefix("claude-").unwrap_or(s);
            let parts: Vec<&str> = rest.split('-').collect();
            let name = parts
                .first()
                .map(|s| {
                    let mut c = s.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .unwrap_or_default();
            let version: String = parts[1..].join(".");
            if version.is_empty() {
                return if fast { format!("Fast {name}") } else { name };
            }
            return if fast {
                format!("Fast {name} {version}")
            } else {
                format!("{name} {version}")
            };
        }
        // OpenCode/other Go models - strip provider prefix
        s if s.contains('/') => {
            let parts: Vec<&str> = s.split('/').collect();
            if let Some(model_part) = parts.last() {
                model_part
            } else {
                s
            }
        }
        other => other,
    };

    if fast {
        format!("Fast {display}")
    } else {
        display.to_string()
    }
}

#[derive(Clone, Copy)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub total_tokens: u64,
}

pub fn parse_token_usage(value: &Value) -> TokenUsage {
    TokenUsage {
        input_tokens: value
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: value
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    }
}

pub fn surface_from_meta(payload: &Value) -> String {
    match (
        payload.get("source").and_then(Value::as_str),
        payload.get("originator").and_then(Value::as_str),
    ) {
        (Some("cli"), _) | (_, Some("codex-tui")) => "Codex CLI".to_string(),
        (Some("vscode"), _) => "Codex VS Code".to_string(),
        (Some("codex-app"), _) => "Codex App".to_string(),
        (Some(source), _) => format!("Codex {}", source),
        _ => "Codex".to_string(),
    }
}

pub fn activity_from_event(value: &Value) -> Option<String> {
    let payload = value.get("payload")?;

    if value.get("type").and_then(Value::as_str) == Some("response_item") {
        let name = payload.get("name").and_then(Value::as_str)?;
        return Some(match name {
            "shell_command" => {
                command_activity(payload).unwrap_or_else(|| "Running terminal command".to_string())
            }
            "apply_patch" => "Applying patch".to_string(),
            other => format!("Using {other}"),
        });
    }

    if value.get("type").and_then(Value::as_str) == Some("event_msg") {
        return match payload.get("type").and_then(Value::as_str) {
            Some("web_search_end") => Some("Searching the web".to_string()),
            Some("patch_apply_end") => Some("Applied patch".to_string()),
            _ => None,
        };
    }

    None
}

pub fn command_activity(payload: &Value) -> Option<String> {
    let arguments = payload.get("arguments").and_then(Value::as_str)?;
    let parsed = serde_json::from_str::<Value>(arguments).ok()?;
    let command = parsed.get("command").and_then(Value::as_str)?.trim();
    if command.is_empty() {
        return None;
    }

    Some(format!("Running command {}", compact_command(command)))
}

pub fn compact_command(command: &str) -> String {
    let first_line = command.lines().next().unwrap_or(command).trim();
    let mut parts = first_line.split_whitespace();
    let head = parts.next().unwrap_or("");
    let second = parts.next();

    let compact = match (head, second) {
        ("cargo", Some(arg)) => format!("cargo {arg}"),
        ("npm", Some(arg)) => format!("npm {arg}"),
        ("pnpm", Some(arg)) => format!("pnpm {arg}"),
        ("yarn", Some(arg)) => format!("yarn {arg}"),
        ("git", Some(arg)) => format!("git {arg}"),
        ("python", Some(arg)) => format!("python {arg}"),
        ("python3", Some(arg)) => format!("python3 {arg}"),
        ("D:\\discord-rich-presence\\target\\release\\multi-agent-presence.exe", Some(arg)) => {
            format!("multi-agent-presence {arg}")
        }
        _ => first_line.to_string(),
    };

    truncate_chars(&compact, 48)
}

pub fn format_plan(plan_type: &str) -> String {
    match plan_type.to_ascii_lowercase().as_str() {
        "plus" => "Plus ($20/month)".to_string(),
        "pro" => "Pro ($200/month)".to_string(),
        "team" => "Team".to_string(),
        other => title_case(other),
    }
}

pub fn format_limits(rate_limits: Option<&Value>) -> Option<String> {
    let rate_limits = rate_limits?;
    let primary = rate_limits
        .pointer("/primary/used_percent")
        .and_then(Value::as_f64)?;
    let secondary = rate_limits
        .pointer("/secondary/used_percent")
        .and_then(Value::as_f64)?;
    Some(format!("5h {:.0}% | 7d {:.0}%", primary, secondary))
}

pub fn format_cost(cost: f64) -> String {
    format!("${cost:.2}")
}

pub fn estimate_cost(_model: &str, _usage: TokenUsage) -> Option<String> {
    None
}

pub fn fit_discord_state(parts: &[String]) -> String {
    let mut state = String::new();
    for part in parts {
        let candidate = if state.is_empty() {
            part.clone()
        } else {
            format!("{state} | {part}")
        };

        if candidate.chars().count() <= 128 {
            state = candidate;
        }
    }

    if state.is_empty() {
        "Local session monitor".to_string()
    } else {
        state
    }
}

pub fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }

    value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>()
        + "..."
}

pub fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => value.to_string(),
    }
}
