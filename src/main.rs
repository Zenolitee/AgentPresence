use rusqlite::Connection;
use serde_json::Value;
use std::env;
use std::ffi::c_void;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowTextW(hWnd: *mut c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
}

const DEFAULT_LARGE_IMAGE: &str = "codex-logo";
const DEFAULT_LARGE_TEXT: &str = "Codex";
const DEFAULT_CLAUDE_LARGE_IMAGE: &str = "claude-logo";
const DEFAULT_CLAUDE_LARGE_TEXT: &str = "Claude Code";
const DEFAULT_OPENCODE_LARGE_IMAGE: &str = "opencode-logo";
const DEFAULT_OPENCODE_LARGE_TEXT: &str = "OpenCode";
const DEFAULT_PI_LARGE_IMAGE: &str = "pi-logo";
const DEFAULT_PI_LARGE_TEXT: &str = "Pi";
const DEFAULT_OPENCODE_CLIENT_ID: &str = "1522861438778212463";
const DEFAULT_PI_CLIENT_ID: &str = "1522861633909821581";
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
    let mut config = Config::load()?;
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--ignore-opencode") {
        config.detect_opencode = false;
    }
    let command = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "run".to_string());

    match command.as_str() {
        "status" => print_status(&config),
        "debug" => print_debug(&config),
        "once" => publish_once(&config),
        "clear" => clear_all_presences(&config),
        "run" => run_loop(&config),
        _ => {
            eprintln!("usage: multi-agent-presence [--ignore-opencode] [run|once|status|debug|clear]");
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
struct Config {
    client_id: String,
    opencode_client_id: String,
    pi_client_id: String,
    large_image: String,
    large_text: String,
    claude_large_image: String,
    claude_large_text: String,
    opencode_large_image: String,
    opencode_large_text: String,
    pi_large_image: String,
    pi_large_text: String,
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
    detect_processes: bool,
    detect_codex: bool,
    detect_pi: bool,
    detect_opencode: bool,
    poll_seconds: u64,
    stale_seconds: u64,
    codex_home: PathBuf,
    pi_home: PathBuf,
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
        let pi_home = env::var("PI_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| string_field(&file_config, "pi_home").map(PathBuf::from))
            .unwrap_or_else(|| user_home.join(".pi"));

        Ok(Self {
            client_id,
            opencode_client_id: string_field(&file_config, "opencode_client_id")
                .unwrap_or_else(|| DEFAULT_OPENCODE_CLIENT_ID.to_string()),
            pi_client_id: string_field(&file_config, "pi_client_id")
                .unwrap_or_else(|| DEFAULT_PI_CLIENT_ID.to_string()),
            large_image: string_field(&file_config, "large_image")
                .unwrap_or_else(|| DEFAULT_LARGE_IMAGE.to_string()),
            large_text: string_field(&file_config, "large_text")
                .unwrap_or_else(|| DEFAULT_LARGE_TEXT.to_string()),
            claude_large_image: string_field(&file_config, "claude_large_image")
                .unwrap_or_else(|| DEFAULT_CLAUDE_LARGE_IMAGE.to_string()),
            claude_large_text: string_field(&file_config, "claude_large_text")
                .unwrap_or_else(|| DEFAULT_CLAUDE_LARGE_TEXT.to_string()),
            opencode_large_image: string_field(&file_config, "opencode_large_image")
                .unwrap_or_else(|| DEFAULT_OPENCODE_LARGE_IMAGE.to_string()),
            opencode_large_text: string_field(&file_config, "opencode_large_text")
                .unwrap_or_else(|| DEFAULT_OPENCODE_LARGE_TEXT.to_string()),
            pi_large_image: string_field(&file_config, "pi_large_image")
                .unwrap_or_else(|| DEFAULT_PI_LARGE_IMAGE.to_string()),
            pi_large_text: string_field(&file_config, "pi_large_text")
                .unwrap_or_else(|| DEFAULT_PI_LARGE_TEXT.to_string()),
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
            detect_processes: bool_field(&file_config, "detect_processes").unwrap_or(true),
            detect_codex: bool_field(&file_config, "detect_codex").unwrap_or(true),
            detect_pi: bool_field(&file_config, "detect_pi").unwrap_or(true),
            detect_opencode: bool_field(&file_config, "detect_opencode").unwrap_or(true),
            poll_seconds: u64_field(&file_config, "poll_seconds").unwrap_or(PRIORITY_POLL_SECONDS),
            stale_seconds: u64_field(&file_config, "stale_seconds")
                .unwrap_or(DEFAULT_STALE_SECONDS),
            codex_home,
            pi_home,
        })
    }
}

fn print_status(config: &Config) -> io::Result<()> {
    println!("codex home: {}", config.codex_home.display());
    println!("sessions: {}", config.codex_home.join("sessions").display());
    println!("pi home: {}", config.pi_home.display());
    println!(
        "pi sessions: {}",
        config.pi_home.join("agent").join("sessions").display()
    );
    println!(
        "client id: {}",
        if config.client_id.is_empty() {
            "missing"
        } else {
            "configured"
        }
    );

    let active_project = get_active_project_from_window();
    if let Some(ref project) = active_project {
        println!("active window project: {project}");
    } else {
        println!("active window project: (none detected)");
    }

    match collect_session(config)? {
        Some(session) => {
            println!("agent: {}", session.agent.display_name());
            println!(
                "discord app id: {}",
                client_id_for_agent(config, session.agent)
            );
            println!(
                "source: {}",
                session
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "process detector".to_string())
            );
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

fn print_debug(_config: &Config) -> io::Result<()> {
    let raw_title = get_foreground_window_title().unwrap_or_else(|| "(null)".to_string());
    println!("raw foreground title: [{raw_title}]");
    match extract_path_from_title(&raw_title) {
        Some(path) => {
            println!("extracted path: {}", path.display());
            println!(
                "file name: {}",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("(none)")
            );
        }
        None => {
            println!("extracted path: (none)");
        }
    }
    Ok(())
}

fn publish_once(config: &Config) -> io::Result<()> {
    let session = collect_session(config)?;
    if let Some(session) = session.as_ref().filter(|session| session.active) {
        let client_id = client_id_for_agent(config, session.agent);
        validate_client_id(client_id)?;
        let _ = clear_other_presences(config, client_id);
        let mut discord = DiscordIpc::connect(client_id)?;
        discord.handshake(client_id)?;
        discord.set_activity(Some(activity_payload(config, Some(session))))?;
    } else {
        clear_all_presences(config)?;
    }
    Ok(())
}

fn run_loop(config: &Config) -> io::Result<()> {
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
        }

        let poll_seconds = if config.priority_presence {
            config.poll_seconds.min(PRIORITY_POLL_SECONDS).max(1)
        } else {
            config.poll_seconds.max(1)
        };
        thread::sleep(Duration::from_secs(poll_seconds));
    }
}

fn validate_client_id(client_id: &str) -> io::Result<()> {
    if client_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing Discord client_id for detected agent; set config.json",
        ));
    }

    Ok(())
}

fn client_id_for_agent<'a>(config: &'a Config, agent: AgentKind) -> &'a str {
    match agent {
        AgentKind::OpenCode => {
            client_id_or_default(&config.opencode_client_id, DEFAULT_OPENCODE_CLIENT_ID)
        }
        AgentKind::Pi => client_id_or_default(&config.pi_client_id, DEFAULT_PI_CLIENT_ID),
        _ => &config.client_id,
    }
}

fn client_id_or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn clear_all_presences(config: &Config) -> io::Result<()> {
    clear_presences(config, None)
}

fn clear_other_presences(config: &Config, keep_client_id: &str) -> io::Result<()> {
    clear_presences(config, Some(keep_client_id))
}

fn clear_presences(config: &Config, keep_client_id: Option<&str>) -> io::Result<()> {
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

fn clear_presence(client_id: &str) -> io::Result<()> {
    let mut discord = DiscordIpc::connect(client_id)?;
    discord.handshake(client_id)?;
    discord.set_activity(None)
}

#[derive(Debug, Clone)]
struct AgentSession {
    agent: AgentKind,
    path: Option<PathBuf>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgentKind {
    Codex,
    Claude,
    OpenCode,
    Pi,
}

impl AgentKind {
    fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::OpenCode => "OpenCode",
            Self::Pi => "Pi",
        }
    }
}

fn collect_session(config: &Config) -> io::Result<Option<AgentSession>> {
    let mut sessions = Vec::new();

    if config.detect_codex {
        if let Some(session) = collect_codex_session(config)? {
            sessions.push(session);
        }
    }

    if config.detect_pi {
        if let Some(session) = collect_pi_session(config)? {
            sessions.push(session);
        }
    }

    if config.detect_opencode {
        if let Some(session) = collect_opencode_session()? {
            sessions.push(session);
        }
    }

    if config.detect_processes {
        if let Some(session) = detect_process_session(config) {
            if let Some(existing) = sessions
                .iter_mut()
                .find(|existing| existing.agent == session.agent)
            {
                existing.active = true;
            } else {
                sessions.push(session);
            }
        }
    }

    Ok(select_best_session(sessions, get_active_project_from_window().as_deref()))
}

fn collect_codex_session(config: &Config) -> io::Result<Option<AgentSession>> {
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

fn codex_activity_is_live(config: &Config) -> bool {
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

fn collect_pi_session(config: &Config) -> io::Result<Option<AgentSession>> {
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

fn pi_activity_is_live(config: &Config) -> bool {
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

fn collect_opencode_session() -> io::Result<Option<AgentSession>> {
    if !opencode_activity_is_live() {
        return Ok(None);
    }

    let app_dir = home_dir()?
        .join("AppData")
        .join("Roaming")
        .join("ai.opencode.desktop");
    let global_path = app_dir.join("opencode.global.dat");
    let global = read_json_object_file(&global_path).unwrap_or(Value::Null);
    let workspace_path = newest_matching_file(&app_dir, "opencode.workspace.", ".dat")?;
    let workspace = workspace_path
        .as_deref()
        .and_then(|path| read_json_object_file(path).ok())
        .unwrap_or(Value::Null);

    let cwd = opencode_project_path(&global);
    let project = cwd
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.trim().is_empty());
    let branch =
        opencode_branch(&workspace).or_else(|| cwd.as_deref().and_then(git_branch_for_path));
    let model = opencode_model(&global, &workspace, cwd.as_deref());

    let db_dir = home_dir()
        .ok()
        .map(|h| h.join(".local").join("share").join("opencode"));
    let started_at = db_dir
        .as_ref()
        .and_then(|dir| most_recent_file_mtime(dir))
        .or_else(|| {
            workspace_path
                .as_ref()
                .and_then(|path| fs::metadata(path).ok())
                .and_then(|metadata| metadata.modified().ok())
        })
        .or_else(|| {
            fs::metadata(&global_path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
        });

    Ok(Some(AgentSession {
        agent: AgentKind::OpenCode,
        path: workspace_path.or(Some(global_path)),
        project,
        branch,
        model,
        surface: Some("OpenCode".to_string()),
        activity: None,
        plan: None,
        tokens: None,
        cost: None,
        context: None,
        limits: None,
        active: true,
        started_at,
    }))
}

fn opencode_activity_is_live() -> bool {
    running_process_text()
        .map(|processes| {
            let lower = processes.to_ascii_lowercase();
            contains_agent_process(&lower, &["opencode", "sst-dev.opencode"])
        })
        .unwrap_or(false)
}

fn read_json_object_file(path: &Path) -> io::Result<Value> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str::<Value>(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn opencode_project_path(global: &Value) -> Option<PathBuf> {
    let server = encoded_json_field(global, "server")?;
    server
        .pointer("/lastProject/local")
        .and_then(Value::as_str)
        .or_else(|| {
            server
                .pointer("/projects/local/0/worktree")
                .and_then(Value::as_str)
        })
        .map(PathBuf::from)
}

fn opencode_branch(workspace: &Value) -> Option<String> {
    encoded_json_field(workspace, "workspace:vcs")?
        .pointer("/value/branch")
        .and_then(Value::as_str)
        .map(|branch| branch.to_string())
        .filter(|branch| !branch.trim().is_empty())
}

fn opencode_model(global: &Value, workspace: &Value, cwd: Option<&Path>) -> Option<String> {
    let selected = encoded_json_field(workspace, "workspace:model-selection")
        .and_then(|value| first_opencode_model(value.get("session")?));
    if selected.is_some() {
        return selected;
    }

    if let Some(model) = opencode_model_from_db(cwd) {
        return Some(model);
    }

    encoded_json_field(global, "model").and_then(|value| {
        let recent = value.get("recent")?.as_array()?.first()?;
        format_provider_model(
            recent.get("providerID").and_then(Value::as_str),
            recent.get("modelID").and_then(Value::as_str),
        )
    })
}

fn opencode_model_from_db(cwd: Option<&Path>) -> Option<String> {
    let dir = home_dir().ok()?.join(".local").join("share").join("opencode");
    let db_path = dir.join("opencode.db");
    let conn = Connection::open(&db_path).ok()?;

    let model_json: String = if let Some(directory) = cwd.and_then(|p| p.to_str()) {
        conn.query_row(
            "SELECT model FROM session WHERE directory = ?1 ORDER BY time_created DESC LIMIT 1",
            [directory],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| {
            conn.query_row(
                "SELECT model FROM session ORDER BY time_created DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default()
        })
    } else {
        conn.query_row(
            "SELECT model FROM session ORDER BY time_created DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default()
    };

    if model_json.is_empty() {
        return None;
    }

    let model_value: Value = serde_json::from_str(&model_json).ok()?;
    format_provider_model(
        model_value.get("providerID").and_then(Value::as_str),
        model_value.get("id").and_then(Value::as_str),
    )
}

fn first_opencode_model(session: &Value) -> Option<String> {
    let object = session.as_object()?;
    for value in object.values() {
        if let Some(model) = value.get("model") {
            if let Some(formatted) = format_provider_model(
                model.get("providerID").and_then(Value::as_str),
                model.get("modelID").and_then(Value::as_str),
            ) {
                return Some(formatted);
            }
        }
    }

    None
}

fn encoded_json_field(value: &Value, key: &str) -> Option<Value> {
    let raw = value.get(key)?.as_str()?;
    serde_json::from_str::<Value>(raw).ok()
}

fn format_provider_model(_provider: Option<&str>, model: Option<&str>) -> Option<String> {
    let model = model?.trim();
    if model.is_empty() {
        return None;
    }

    Some(model.rsplit('/').next().unwrap_or(model).trim().to_string())
}

fn get_foreground_window_title() -> Option<String> {
    #[cfg(windows)]
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn extract_path_from_title(title: &str) -> Option<PathBuf> {
    let trimmed = title.trim();

    // Try the whole title as a path first
    let direct = PathBuf::from(trimmed);
    if direct.is_dir() {
        return Some(direct);
    }

    // Look for a Windows drive-letter path (X:\...) anywhere in the title
    // Handle prefixes like "PS " and suffixes like ">" from PowerShell prompts
    let cleaned = trimmed
        .trim_start_matches("PS ")
        .trim_start_matches("Administrator: ");

    for (i, _) in cleaned.match_indices(|c: char| c.is_ascii_alphabetic()) {
        let rest = &cleaned[i..];
        if rest.len() >= 3 && rest.as_bytes()[1] == b':' && rest.as_bytes()[2] == b'\\' {
            // Find end of path (stop at >, ", space before non-path, etc.)
            let mut end = rest.len();
            for (j, ch) in rest.char_indices().skip(3) {
                if ch == '>' || ch == '"' {
                    end = j;
                    break;
                }
            }
            let candidate = Path::new(&rest[..end]);
            if candidate.is_dir() {
                return Some(candidate.to_path_buf());
            }
        }
    }

    None
}

fn get_active_project_from_window() -> Option<String> {
    let title = get_foreground_window_title()?;
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try to extract a directory path from the title
    if let Some(path) = extract_path_from_title(trimmed) {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let name = name.to_string();
            if !name.trim().is_empty() {
                return Some(name);
            }
        }
    }

    None
}

fn select_best_session(
    mut sessions: Vec<AgentSession>,
    active_project: Option<&str>,
) -> Option<AgentSession> {
    if let Some(active) = active_project {
        if let Some(session) = sessions.iter().find(|s| {
            s.project
                .as_deref()
                .map(|p| p.eq_ignore_ascii_case(active))
                .unwrap_or(false)
        }) {
            return Some(session.clone());
        }
    }

    sessions.sort_by(|left, right| {
        let left_time = left.started_at.unwrap_or(SystemTime::UNIX_EPOCH);
        let right_time = right.started_at.unwrap_or(SystemTime::UNIX_EPOCH);
        right
            .active
            .cmp(&left.active)
            .then_with(|| right.path.is_some().cmp(&left.path.is_some()))
            .then_with(|| right_time.cmp(&left_time))
    });

    sessions.into_iter().next()
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

#[derive(Default)]
struct ParsedPiSession {
    cwd: Option<PathBuf>,
    model: Option<String>,
    activity: Option<String>,
    tokens: Option<String>,
    cost: Option<String>,
    started_at: Option<SystemTime>,
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

fn parse_pi_session_text(raw: &str) -> ParsedPiSession {
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

fn format_pi_model(provider: Option<&str>, model: Option<&str>) -> Option<String> {
    format_provider_model(provider, model)
}

fn pi_activity_from_message(message: &Value) -> Option<String> {
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

fn activity_payload(config: &Config, session: Option<&AgentSession>) -> String {
    let active = session.map(|s| s.active).unwrap_or(false);
    let details = if config.show_activity {
        session
            .and_then(|session| session.activity.as_deref())
            .unwrap_or_else(|| {
                if config.flavor_text {
                    rotating_detail(active)
                } else if active {
                    active_agent_detail(session)
                } else {
                    idle_agent_detail(session)
                }
            })
    } else if config.flavor_text {
        rotating_detail(active)
    } else if active {
        active_agent_detail(session)
    } else {
        idle_agent_detail(session)
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
    if let Some(session) = session {
        if session.path.is_some() {
            if let Some(started_at) = session.started_at {
                if let Ok(duration) = started_at.duration_since(SystemTime::UNIX_EPOCH) {
                    timestamp = format!(r#","timestamps":{{"start":{}}}"#, duration.as_secs());
                }
            }
        }
    }

    let (large_image, large_text) = asset_profile(config, session.map(|session| session.agent));

    format!(
        r#"{{"details":"{}","state":"{}","assets":{{"large_image":"{}","large_text":"{}"}}{}}}"#,
        json_escape(details),
        json_escape(&state),
        json_escape(large_image),
        json_escape(large_text),
        timestamp
    )
}

fn active_agent_detail(session: Option<&AgentSession>) -> &'static str {
    match session.map(|session| session.agent) {
        Some(AgentKind::Claude) => "Using Claude Code",
        Some(AgentKind::OpenCode) => "OpenCode",
        Some(AgentKind::Pi) => "Pi",
        _ => "Using Codex in terminal",
    }
}

fn idle_agent_detail(session: Option<&AgentSession>) -> &'static str {
    match session.map(|session| session.agent) {
        Some(AgentKind::Claude) => "Claude Code idle",
        Some(AgentKind::OpenCode) => "OpenCode",
        Some(AgentKind::Pi) => "Pi idle",
        _ => "Codex idle",
    }
}

fn asset_profile(config: &Config, agent: Option<AgentKind>) -> (&str, &str) {
    match agent {
        Some(AgentKind::Claude) => (&config.claude_large_image, &config.claude_large_text),
        Some(AgentKind::OpenCode) => (&config.opencode_large_image, &config.opencode_large_text),
        Some(AgentKind::Pi) => (&config.pi_large_image, &config.pi_large_text),
        _ => (&config.large_image, &config.large_text),
    }
}

fn detect_process_session(config: &Config) -> Option<AgentSession> {
    let processes = running_process_text().ok()?;
    let agent = detect_agent_from_process_text(&processes, config)?;

    Some(AgentSession {
        agent,
        path: None,
        project: None,
        branch: None,
        model: None,
        surface: Some(agent.display_name().to_string()),
        activity: None,
        plan: None,
        tokens: None,
        cost: None,
        context: None,
        limits: None,
        active: true,
        started_at: Some(SystemTime::now()),
    })
}

fn detect_agent_from_process_text(processes: &str, config: &Config) -> Option<AgentKind> {
    let lower = processes.to_ascii_lowercase();

    if config.detect_opencode
        && contains_agent_process(&lower, &["opencode", "sst-dev.opencode"])
    {
        return Some(AgentKind::OpenCode);
    }

    if contains_agent_process(&lower, &["claude", "@anthropic-ai/claude-code"]) {
        return Some(AgentKind::Claude);
    }

    if config.detect_pi
        && contains_agent_process(
            &lower,
            &["pi.ai", "inflection", "pi desktop", "pi-node", "\\pi.exe"],
        )
    {
        return Some(AgentKind::Pi);
    }

    None
}

fn contains_agent_process(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn running_process_text() -> io::Result<String> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_Process | Select-Object Name,CommandLine | ConvertTo-Json -Compress",
            ])
            .output()?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let fallback = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-Process | Select-Object ProcessName,Path | ConvertTo-Json -Compress",
            ])
            .output()?;

        if fallback.status.success() {
            return Ok(String::from_utf8_lossy(&fallback.stdout).to_string());
        }

        return Ok(format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&fallback.stderr)
        ));
    }

    #[cfg(not(windows))]
    {
        let output = std::process::Command::new("ps")
            .args(["-axo", "comm,args"])
            .output()?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn newest_jsonl(dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    visit_jsonl(dir, &mut newest)?;
    Ok(newest.map(|(path, _)| path))
}

fn newest_matching_file(dir: &Path, prefix: &str, suffix: &str) -> io::Result<Option<PathBuf>> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(prefix) || !name.ends_with(suffix) {
            continue;
        }

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if newest
            .as_ref()
            .map(|(_, current)| modified > *current)
            .unwrap_or(true)
        {
            newest = Some((path, modified));
        }
    }

    Ok(newest.map(|(path, _)| path))
}

fn most_recent_file_mtime(dir: &Path) -> Option<SystemTime> {
    if !dir.exists() {
        return None;
    }
    let mut newest: Option<SystemTime> = None;
    let _ = visit_mtime(dir, &mut newest);
    newest
}

fn visit_mtime(dir: &Path, newest: &mut Option<SystemTime>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            let _ = visit_mtime(&path, newest);
            continue;
        }
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if newest.map(|t| modified > t).unwrap_or(true) {
            *newest = Some(modified);
        }
    }
    Ok(())
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
        ("D:\\discord-rich-presence\\target\\release\\multi-agent-presence.exe", Some(arg)) => {
            format!("multi-agent-presence {arg}")
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

fn format_cost(cost: f64) -> String {
    if cost >= 1.0 {
        format!("${cost:.2}")
    } else {
        format!("${cost:.4}")
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

    fn set_activity(&mut self, activity: Option<String>) -> io::Result<()> {
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

fn parse_iso8601_utc(_value: &str) -> Option<SystemTime> {
    None
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
