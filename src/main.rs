use serde_json::Value;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

const DEFAULT_LARGE_IMAGE: &str = "codex-logo";
const DEFAULT_LARGE_TEXT: &str = "Codex";
const PRIORITY_POLL_SECONDS: u64 = 2;
const DEFAULT_STALE_SECONDS: u64 = 180;
const MAX_TAIL_BYTES: u64 = 256 * 1024;
const ACTIVE_DETAILS: [&str; 50] = [
    "Arguing with TypeScript",
    "Bribing the compiler",
    "Negotiating with bugs",
    "Summoning stack traces",
    "Petting the codebase",
    "Feeding tokens to Codex",
    "Questioning all imports",
    "Gaslighting the linter",
    "Whispering to npm",
    "Teaching bugs manners",
    "Bullying flaky tests",
    "Untangling spaghetti",
    "Interrogating functions",
    "Staring at red text",
    "Making bugs confess",
    "Collecting semicolons",
    "Defusing merge conflicts",
    "Praying to package.json",
    "Wrestling node_modules",
    "Hunting rogue commas",
    "Poking the API",
    "Debugging by vibes",
    "Consulting the rubber duck",
    "Asking the DOM politely",
    "Gaslighting Playwright",
    "Befriending the terminal",
    "Escaping callback hell",
    "Searching for who asked",
    "Patching emotional damage",
    "Cooking questionable diffs",
    "Letting Codex yap",
    "Typing with consequences",
    "Refreshing with hope",
    "Running on caffeine",
    "Fixing one bug, spawning two",
    "Trying not to rm -rf",
    "Sacrificing RAM",
    "Reading ancient code",
    "Looking busy professionally",
    "Summoning a green check",
    "Begging tests to pass",
    "Blaming cache",
    "Deleting evidence",
    "Making localhost suffer",
    "Fighting invisible state",
    "Pushing pixels legally",
    "Explaining code to itself",
    "Putting bugs in timeout",
    "Debugging the forbidden soup",
    "Turning errors into lore",
];
const IDLE_DETAILS: [&str; 4] = [
    "Codex idle",
    "Waiting in terminal",
    "Standing by",
    "Session quiet",
];

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let config = Config::load()?;
    let command = env::args().nth(1).unwrap_or_else(|| "run".to_string());

    match command.as_str() {
        "status" => print_status(&config),
        "once" => publish_once(&config),
        "run" => run_loop(&config),
        _ => {
            eprintln!("usage: codex-discord-presence [run|once|status]");
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
struct Config {
    client_id: String,
    large_image: String,
    large_text: String,
    hide_project: bool,
    hide_model: bool,
    show_branch: bool,
    flavor_text: bool,
    show_activity: bool,
    show_surface: bool,
    show_plan: bool,
    show_tokens: bool,
    show_cost: bool,
    show_context: bool,
    show_limits: bool,
    priority_presence: bool,
    poll_seconds: u64,
    stale_seconds: u64,
    codex_home: PathBuf,
}

impl Config {
    fn load() -> io::Result<Self> {
        let user_home = home_dir()?;
        let config_path = user_home
            .join(".codex-discord-presence")
            .join("config.json");
        let file_config = fs::read_to_string(config_path).unwrap_or_default();

        let client_id = env::var("CODEX_DISCORD_CLIENT_ID")
            .ok()
            .or_else(|| string_field(&file_config, "client_id"))
            .unwrap_or_default();

        let codex_home = env::var("CODEX_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| string_field(&file_config, "codex_home").map(PathBuf::from))
            .unwrap_or_else(|| user_home.join(".codex"));

        Ok(Self {
            client_id,
            large_image: string_field(&file_config, "large_image")
                .unwrap_or_else(|| DEFAULT_LARGE_IMAGE.to_string()),
            large_text: string_field(&file_config, "large_text")
                .unwrap_or_else(|| DEFAULT_LARGE_TEXT.to_string()),
            hide_project: bool_field(&file_config, "hide_project").unwrap_or(false),
            hide_model: bool_field(&file_config, "hide_model").unwrap_or(false),
            show_branch: bool_field(&file_config, "show_branch").unwrap_or(true),
            flavor_text: bool_field(&file_config, "flavor_text").unwrap_or(true),
            show_activity: bool_field(&file_config, "show_activity").unwrap_or(true),
            show_surface: bool_field(&file_config, "show_surface").unwrap_or(true),
            show_plan: bool_field(&file_config, "show_plan").unwrap_or(true),
            show_tokens: bool_field(&file_config, "show_tokens").unwrap_or(true),
            show_cost: bool_field(&file_config, "show_cost").unwrap_or(true),
            show_context: bool_field(&file_config, "show_context").unwrap_or(true),
            show_limits: bool_field(&file_config, "show_limits").unwrap_or(true),
            priority_presence: bool_field(&file_config, "priority_presence").unwrap_or(true),
            poll_seconds: u64_field(&file_config, "poll_seconds").unwrap_or(PRIORITY_POLL_SECONDS),
            stale_seconds: u64_field(&file_config, "stale_seconds")
                .unwrap_or(DEFAULT_STALE_SECONDS),
            codex_home,
        })
    }
}

fn print_status(config: &Config) -> io::Result<()> {
    println!("codex home: {}", config.codex_home.display());
    println!("sessions: {}", config.codex_home.join("sessions").display());
    println!(
        "client id: {}",
        if config.client_id.is_empty() {
            "missing"
        } else {
            "configured"
        }
    );

    match collect_session(config)? {
        Some(session) => {
            println!("latest session: {}", session.path.display());
            println!("active: {}", session.active);
            println!(
                "project: {}",
                session.project.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "model: {}",
                session.model.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "branch: {}",
                session.branch.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "surface: {}",
                session.surface.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "activity: {}",
                session.activity.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "plan: {}",
                session.plan.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "tokens: {}",
                session.tokens.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "context: {}",
                session.context.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "limits: {}",
                session.limits.unwrap_or_else(|| "unknown".to_string())
            );
            println!(
                "cost: {}",
                session.cost.unwrap_or_else(|| "unknown".to_string())
            );
        }
        None => println!("latest session: none"),
    }

    Ok(())
}

fn publish_once(config: &Config) -> io::Result<()> {
    validate_client_id(config)?;
    let session = collect_session(config)?;
    let mut discord = DiscordIpc::connect(&config.client_id)?;
    discord.handshake(&config.client_id)?;
    discord.set_activity(activity_payload(config, session.as_ref()))?;
    Ok(())
}

fn run_loop(config: &Config) -> io::Result<()> {
    validate_client_id(config)?;
    let mut discord = DiscordIpc::connect(&config.client_id)?;
    discord.handshake(&config.client_id)?;

    loop {
        let session = collect_session(config)?;
        if let Err(error) = discord.set_activity(activity_payload(config, session.as_ref())) {
            eprintln!("discord publish failed: {error}; reconnecting");
            discord = DiscordIpc::connect(&config.client_id)?;
            discord.handshake(&config.client_id)?;
        }

        let poll_seconds = if config.priority_presence {
            config.poll_seconds.min(PRIORITY_POLL_SECONDS).max(1)
        } else {
            config.poll_seconds.max(1)
        };
        thread::sleep(Duration::from_secs(poll_seconds));
    }
}

fn validate_client_id(config: &Config) -> io::Result<()> {
    if config.client_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing Discord client_id; set CODEX_DISCORD_CLIENT_ID or config.json",
        ));
    }

    Ok(())
}

#[derive(Debug)]
struct CodexSession {
    path: PathBuf,
    project: Option<String>,
    branch: Option<String>,
    model: Option<String>,
    surface: Option<String>,
    activity: Option<String>,
    plan: Option<String>,
    tokens: Option<String>,
    cost: Option<String>,
    context: Option<String>,
    limits: Option<String>,
    active: bool,
    started_at: Option<SystemTime>,
}

fn collect_session(config: &Config) -> io::Result<Option<CodexSession>> {
    let sessions_dir = config.codex_home.join("sessions");
    let Some(path) = newest_jsonl(&sessions_dir)? else {
        return Ok(None);
    };

    let metadata = fs::metadata(&path)?;
    let age = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .unwrap_or(Duration::MAX);

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

    Ok(Some(CodexSession {
        path,
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
        active: age.as_secs() <= config.stale_seconds,
        started_at: metadata.created().ok().or_else(|| metadata.modified().ok()),
    }))
}

#[derive(Default)]
struct ParsedSession {
    cwd: Option<PathBuf>,
    model: Option<String>,
    surface: Option<String>,
    activity: Option<String>,
    plan: Option<String>,
    tokens: Option<String>,
    cost: Option<String>,
    context: Option<String>,
    limits: Option<String>,
}

fn parse_session_text(raw: &str) -> ParsedSession {
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
                .map(clean_model);
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

fn activity_payload(config: &Config, session: Option<&CodexSession>) -> String {
    let active = session.map(|s| s.active).unwrap_or(false);
    let details = if config.show_activity {
        session
            .and_then(|session| session.activity.as_deref())
            .unwrap_or_else(|| {
                if config.flavor_text {
                    rotating_detail(active)
                } else if active {
                    "Using Codex in terminal"
                } else {
                    "Codex idle"
                }
            })
    } else if config.flavor_text {
        rotating_detail(active)
    } else if active {
        "Using Codex in terminal"
    } else {
        "Codex idle"
    };

    let mut state_parts = Vec::new();
    if let Some(session) = session {
        if !config.hide_project {
            if let Some(project) = &session.project {
                state_parts.push(project.clone());
            }
        }

        if config.show_surface {
            if let Some(surface) = &session.surface {
                state_parts.push(surface.clone());
            }
        }

        if config.show_branch {
            if let Some(branch) = &session.branch {
                state_parts.push(branch.clone());
            }
        }

        if !config.hide_model {
            if let Some(model) = &session.model {
                state_parts.push(model.clone());
            }
        }

        if config.show_plan {
            if let Some(plan) = &session.plan {
                state_parts.push(plan.clone());
            }
        }

        if config.show_tokens {
            if let Some(tokens) = &session.tokens {
                state_parts.push(tokens.clone());
            }
        }

        if config.show_cost {
            if let Some(cost) = &session.cost {
                state_parts.push(cost.clone());
            }
        }

        if config.show_context {
            if let Some(context) = &session.context {
                state_parts.push(context.clone());
            }
        }

        if config.show_limits {
            if let Some(limits) = &session.limits {
                state_parts.push(limits.clone());
            }
        }
    }

    let state = if state_parts.is_empty() {
        "Local session monitor".to_string()
    } else {
        fit_discord_state(&state_parts)
    };

    let mut timestamp = String::new();
    if let Some(started_at) = session.and_then(|s| s.started_at) {
        if let Ok(duration) = started_at.duration_since(SystemTime::UNIX_EPOCH) {
            timestamp = format!(r#","timestamps":{{"start":{}}}"#, duration.as_secs());
        }
    }

    format!(
        r#"{{"details":"{}","state":"{}","assets":{{"large_image":"{}","large_text":"{}"}}{}}}"#,
        json_escape(details),
        json_escape(&state),
        json_escape(&config.large_image),
        json_escape(&config.large_text),
        timestamp
    )
}

fn newest_jsonl(dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    visit_jsonl(dir, &mut newest)?;
    Ok(newest.map(|(path, _)| path))
}

fn visit_jsonl(dir: &Path, newest: &mut Option<(PathBuf, SystemTime)>) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            visit_jsonl(&path, newest)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if newest
            .as_ref()
            .map(|(_, current)| modified > *current)
            .unwrap_or(true)
        {
            *newest = Some((path, modified));
        }
    }

    Ok(())
}

fn read_tail(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if start > 0 {
        if let Some(first_newline) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=first_newline);
        }
    }

    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn read_head(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(max_bytes)
        .read_to_end(&mut bytes)?;

    if let Some(last_newline) = bytes.iter().rposition(|byte| *byte == b'\n') {
        bytes.truncate(last_newline + 1);
    }

    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn clean_model(model: &str) -> String {
    let trimmed = model.trim();
    let fast = trimmed.ends_with("-fast");
    let base = trimmed.strip_suffix("-fast").unwrap_or(trimmed);
    let mut display = base
        .split('-')
        .map(|part| {
            if part.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
                part.to_string()
            } else {
                part.to_ascii_uppercase()
            }
        })
        .collect::<Vec<_>>()
        .join("-");

    if fast {
        display = format!("Fast {}", display);
    }

    display
}

#[derive(Clone, Copy)]
struct TokenUsage {
    input_tokens: u64,
    total_tokens: u64,
}

fn parse_token_usage(value: &Value) -> TokenUsage {
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

fn surface_from_meta(payload: &Value) -> String {
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

fn activity_from_event(value: &Value) -> Option<String> {
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

fn command_activity(payload: &Value) -> Option<String> {
    let arguments = payload.get("arguments").and_then(Value::as_str)?;
    let parsed = serde_json::from_str::<Value>(arguments).ok()?;
    let command = parsed.get("command").and_then(Value::as_str)?.trim();
    if command.is_empty() {
        return None;
    }

    Some(format!("Running command {}", compact_command(command)))
}

fn compact_command(command: &str) -> String {
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
        ("D:\\discord-rich-presence\\target\\release\\codex-discord-presence.exe", Some(arg)) => {
            format!("codex-discord-presence {arg}")
        }
        _ => first_line.to_string(),
    };

    truncate_chars(&compact, 48)
}

fn format_plan(plan_type: &str) -> String {
    match plan_type.to_ascii_lowercase().as_str() {
        "plus" => "Plus ($20/month)".to_string(),
        "pro" => "Pro ($200/month)".to_string(),
        "team" => "Team".to_string(),
        other => title_case(other),
    }
}

fn format_limits(rate_limits: Option<&Value>) -> Option<String> {
    let rate_limits = rate_limits?;
    let primary = rate_limits
        .pointer("/primary/used_percent")
        .and_then(Value::as_f64)?;
    let secondary = rate_limits
        .pointer("/secondary/used_percent")
        .and_then(Value::as_f64)?;
    Some(format!("5h {:.0}% | 7d {:.0}%", primary, secondary))
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M tok", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K tok", tokens as f64 / 1_000.0)
    } else {
        format!("{tokens} tok")
    }
}

fn estimate_cost(_model: &str, _usage: TokenUsage) -> Option<String> {
    None
}

fn fit_discord_state(parts: &[String]) -> String {
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

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }

    value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>()
        + "..."
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => value.to_string(),
    }
}

fn git_branch_for_path(path: &Path) -> Option<String> {
    for current in path.ancestors() {
        let git_path = current.join(".git");
        if git_path.is_dir() {
            return branch_from_git_dir(&git_path);
        }

        if git_path.is_file() {
            let raw = fs::read_to_string(&git_path).ok()?;
            let gitdir = raw.trim().strip_prefix("gitdir:")?.trim();
            let resolved = if Path::new(gitdir).is_absolute() {
                PathBuf::from(gitdir)
            } else {
                current.join(gitdir)
            };
            return branch_from_git_dir(&resolved);
        }
    }

    None
}

fn branch_from_git_dir(git_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let trimmed = head.trim();

    if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
        return branch
            .rsplit('/')
            .next()
            .map(|name| name.to_string())
            .filter(|name| !name.is_empty());
    }

    if trimmed.len() >= 7 {
        return Some(format!("detached {}", &trimmed[..7]));
    }

    None
}

fn rotating_detail(active: bool) -> &'static str {
    let phrases: &[&str] = if active {
        &ACTIVE_DETAILS
    } else {
        &IDLE_DETAILS
    };
    let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let index = ((seconds / 10) as usize) % phrases.len();
    phrases[index]
}

fn home_dir() -> io::Result<PathBuf> {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))
}

fn string_field(value: &str, key: &str) -> Option<String> {
    json_string_field(value, key)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn bool_field(value: &str, key: &str) -> Option<bool> {
    let marker = format!(r#""{key}""#);
    let after_key = value.find(&marker).and_then(|index| {
        value[index + marker.len()..]
            .find(':')
            .map(|offset| index + marker.len() + offset + 1)
    })?;
    let tail = value[after_key..].trim_start();

    if tail.starts_with("true") {
        Some(true)
    } else if tail.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn u64_field(value: &str, key: &str) -> Option<u64> {
    let marker = format!(r#""{key}""#);
    let after_key = value.find(&marker).and_then(|index| {
        value[index + marker.len()..]
            .find(':')
            .map(|offset| index + marker.len() + offset + 1)
    })?;
    let digits: String = value[after_key..]
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();

    digits.parse().ok()
}

struct DiscordIpc {
    stream: File,
}

impl DiscordIpc {
    fn connect(_client_id: &str) -> io::Result<Self> {
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

    fn handshake(&mut self, client_id: &str) -> io::Result<()> {
        self.send(
            0,
            &format!(r#"{{"v":1,"client_id":"{}"}}"#, json_escape(client_id)),
        )
    }

    fn set_activity(&mut self, activity: String) -> io::Result<()> {
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
        self.stream.write_all(&bytes)?;
        self.stream.flush()
    }
}

fn discord_ipc_paths() -> Vec<PathBuf> {
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

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn json_string_field(value: &str, key: &str) -> Option<String> {
    let marker = format!(r#""{key}""#);
    let marker_index = value.find(&marker)?;
    let after_key = &value[marker_index + marker.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let mut chars = after_colon.chars();

    if chars.next()? != '"' {
        return None;
    }

    let mut output = String::new();
    let mut escaped = false;

    for ch in chars {
        if escaped {
            match ch {
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                '/' => output.push('/'),
                'b' => output.push('\u{0008}'),
                'f' => output.push('\u{000c}'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                _ => output.push(ch),
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(output);
        } else {
            output.push(ch);
        }
    }

    None
}

fn json_escape(value: &str) -> String {
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
