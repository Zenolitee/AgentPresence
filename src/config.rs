use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::fsutil::home_dir;
use crate::harnesses::AgentKind;

pub const DEFAULT_LARGE_IMAGE: &str = "codex-logo";
pub const DEFAULT_LARGE_TEXT: &str = "Codex";
pub const DEFAULT_CLAUDE_LARGE_IMAGE: &str = "claudecode-color";
pub const DEFAULT_CLAUDE_LARGE_TEXT: &str = "Claude Code";
pub const DEFAULT_CLAUDE_CLIENT_ID: &str = "1525571660877529218";
pub const DEFAULT_OPENCODE_LARGE_IMAGE: &str = "opencode-logo";
pub const DEFAULT_OPENCODE_LARGE_TEXT: &str = "OpenCode";
pub const DEFAULT_PI_LARGE_IMAGE: &str = "pi-logo";
pub const DEFAULT_PI_LARGE_TEXT: &str = "Pi";
pub const DEFAULT_OPENCODE_CLIENT_ID: &str = "1522861438778212463";
pub const DEFAULT_PI_CLIENT_ID: &str = "1522861633909821581";
pub const DEFAULT_HERMES_LARGE_IMAGE: &str = "hermes-logo";
pub const DEFAULT_HERMES_LARGE_TEXT: &str = "Hermes";
pub const PRIORITY_POLL_SECONDS: u64 = 2;
pub const DEFAULT_STALE_SECONDS: u64 = 180;
pub const DEFAULT_CONFIG_JSON: &str = r#"{
  "client_id": "1522704011491545159",
  "opencode_client_id": "1522861438778212463",
  "pi_client_id": "1522861633909821581",
  "claude_client_id": "1525571660877529218",
  "hermes_client_id": "",

  "large_image": "codex-logo",
  "claude_large_image": "claudecode-color",
  "claude_large_text": "Claude Code",
  "opencode_large_image": "opencode-logo",
  "opencode_large_text": "OpenCode",
  "pi_large_image": "pi-logo",
  "pi_large_text": "Pi",
  "hermes_large_image": "hermes-logo",
  "hermes_large_text": "Hermes",

  "priority_presence": true,

  "detect_processes": true,
  "detect_codex": true,
  "detect_pi": true,
  "detect_opencode": true,
  "detect_omp": true,
  "detect_claude": true,
  "detect_hermes": true,

  "codex_home": null,
  "pi_home": null,
  "omp_home": null,
  "hermes_home": null,
  "poll_seconds": 2,
  "stale_seconds": 180,
  "omp_client_id": "1523945901519929466",
  "omp_large_image": "hero",
  "omp_large_text": "Oh My Pi"
}
"#;

/// Per-agent display settings for Discord presence.
#[derive(Clone, Debug)]
pub struct DisplaySettings {
    pub show_project: bool,
    pub show_model: bool,
    pub show_branch: bool,
    pub show_activity: bool,
    pub show_surface: bool,
    pub show_plan: bool,
    pub show_tokens: bool,
    pub show_cost: bool,
    pub show_context: bool,
    pub show_limits: bool,
    pub flavor_text: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            show_project: true,
            show_model: true,
            show_branch: true,
            show_activity: true,
            show_surface: true,
            show_plan: true,
            show_tokens: true,
            show_cost: true,
            show_context: true,
            show_limits: true,
            flavor_text: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub client_id: String,
    pub opencode_client_id: String,
    pub pi_client_id: String,
    pub claude_client_id: String,
    pub hermes_client_id: String,
    pub large_image: String,
    pub large_text: String,
    pub claude_large_image: String,
    pub claude_large_text: String,
    pub opencode_large_image: String,
    pub opencode_large_text: String,
    pub pi_large_image: String,
    pub pi_large_text: String,
    pub hermes_large_image: String,
    pub hermes_large_text: String,
    pub priority_presence: bool,
    pub detect_processes: bool,
    pub detect_codex: bool,
    pub detect_pi: bool,
    pub detect_opencode: bool,
    pub detect_omp: bool,
    pub detect_claude: bool,
    pub detect_hermes: bool,
    pub poll_seconds: u64,
    pub stale_seconds: u64,
    pub codex_home: PathBuf,
    pub pi_home: PathBuf,
    pub omp_home: PathBuf,
    pub hermes_home: PathBuf,
    pub omp_client_id: String,
    pub omp_large_image: String,
    pub omp_large_text: String,
    pub codex_display: DisplaySettings,
    pub claude_display: DisplaySettings,
    pub opencode_display: DisplaySettings,
    pub pi_display: DisplaySettings,
    pub omp_display: DisplaySettings,
    pub hermes_display: DisplaySettings,
}

impl Config {
    pub fn load() -> io::Result<Self> {
        let user_home = home_dir()?;
        let config_dir = user_home.join(".agent-presence");
        let local_app_data = env::var("LOCALAPPDATA")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home.join("AppData").join("Local"));
        let config_path = config_dir.join("config.json");

        if !config_path.exists() {
            let _ = fs::create_dir_all(&config_dir);
            let _ = fs::write(&config_path, DEFAULT_CONFIG_JSON);
        }

        let file_config = fs::read_to_string(&config_path).unwrap_or_default();

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
            claude_client_id: string_field(&file_config, "claude_client_id")
                .unwrap_or_else(|| DEFAULT_CLAUDE_CLIENT_ID.to_string()),
            hermes_client_id: string_field(&file_config, "hermes_client_id").unwrap_or_default(),
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
            hermes_large_image: string_field(&file_config, "hermes_large_image")
                .unwrap_or_else(|| DEFAULT_HERMES_LARGE_IMAGE.to_string()),
            hermes_large_text: string_field(&file_config, "hermes_large_text")
                .unwrap_or_else(|| DEFAULT_HERMES_LARGE_TEXT.to_string()),
            priority_presence: bool_field(&file_config, "priority_presence").unwrap_or(true),
            detect_processes: bool_field(&file_config, "detect_processes").unwrap_or(true),
            detect_codex: bool_field(&file_config, "detect_codex").unwrap_or(true),
            detect_pi: bool_field(&file_config, "detect_pi").unwrap_or(true),
            detect_opencode: bool_field(&file_config, "detect_opencode").unwrap_or(true),
            detect_omp: bool_field(&file_config, "detect_omp").unwrap_or(true),
            detect_claude: bool_field(&file_config, "detect_claude").unwrap_or(true),
            detect_hermes: bool_field(&file_config, "detect_hermes").unwrap_or(true),
            poll_seconds: u64_field(&file_config, "poll_seconds").unwrap_or(PRIORITY_POLL_SECONDS),
            stale_seconds: u64_field(&file_config, "stale_seconds")
                .unwrap_or(DEFAULT_STALE_SECONDS),
            codex_home,
            pi_home,
            omp_home: env::var("OMP_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| string_field(&file_config, "omp_home").map(PathBuf::from))
                .unwrap_or_else(|| user_home.join(".omp")),
            hermes_home: env::var("HERMES_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| string_field(&file_config, "hermes_home").map(PathBuf::from))
                .unwrap_or_else(|| local_app_data.join("hermes")),
            omp_client_id: string_field(&file_config, "omp_client_id")
                .unwrap_or_else(|| DEFAULT_PI_CLIENT_ID.to_string()),
            omp_large_image: string_field(&file_config, "omp_large_image")
                .unwrap_or_else(|| "hero".to_string()),
            omp_large_text: string_field(&file_config, "omp_large_text")
                .unwrap_or_else(|| "Oh My Pi".to_string()),
            codex_display: load_display_settings(&file_config, "codex"),
            claude_display: load_display_settings(&file_config, "claude"),
            opencode_display: load_display_settings(&file_config, "opencode"),
            pi_display: load_display_settings(&file_config, "pi"),
            omp_display: load_display_settings(&file_config, "omp"),
            hermes_display: load_display_settings(&file_config, "hermes"),
        })
    }
}

pub fn load_display_settings(file_config: &str, agent: &str) -> DisplaySettings {
    let prefix = format!("{agent}_");
    let mut ds = DisplaySettings::default();

    // Parse JSON once
    let parsed: serde_json::Value =
        serde_json::from_str(file_config).unwrap_or(serde_json::Value::Null);

    // Try per-agent settings first, fall back to global
    let get_bool = |key: &str, default: bool| -> bool {
        let agent_key = format!("{prefix}{key}");
        parsed
            .get(&agent_key)
            .and_then(|v| v.as_bool())
            .or_else(|| parsed.get(key).and_then(|v| v.as_bool()))
            .unwrap_or(default)
    };

    ds.show_project = get_bool("show_project", true);
    ds.show_model = get_bool("show_model", true);
    ds.show_branch = get_bool("show_branch", true);
    ds.show_activity = get_bool("show_activity", true);
    ds.show_surface = get_bool("show_surface", true);
    ds.show_plan = get_bool("show_plan", true);
    ds.show_tokens = get_bool("show_tokens", true);
    ds.show_cost = get_bool("show_cost", true);
    ds.show_context = get_bool("show_context", true);
    ds.show_limits = get_bool("show_limits", true);
    ds.flavor_text = get_bool("flavor_text", true);
    ds
}

pub fn display_settings_for<'a>(config: &'a Config, agent: AgentKind) -> &'a DisplaySettings {
    match agent {
        AgentKind::Codex => &config.codex_display,
        AgentKind::Claude => &config.claude_display,
        AgentKind::OpenCode => &config.opencode_display,
        AgentKind::Pi => &config.pi_display,
        AgentKind::OhMyPi => &config.omp_display,
        AgentKind::Hermes => &config.hermes_display,
    }
}

pub fn save_config(config: &Config) -> io::Result<()> {
    let user_home = home_dir()?;
    let config_dir = user_home.join(".agent-presence");
    fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.json");

    let mut map = serde_json::Map::new();

    // Client IDs
    map.insert("client_id".into(), config.client_id.clone().into());
    map.insert(
        "opencode_client_id".into(),
        config.opencode_client_id.clone().into(),
    );
    map.insert("pi_client_id".into(), config.pi_client_id.clone().into());
    map.insert(
        "claude_client_id".into(),
        config.claude_client_id.clone().into(),
    );
    map.insert("omp_client_id".into(), config.omp_client_id.clone().into());

    map.insert(
        "hermes_client_id".into(),
        config.hermes_client_id.clone().into(),
    );
    // Image/text
    map.insert("large_image".into(), config.large_image.clone().into());
    map.insert("large_text".into(), config.large_text.clone().into());
    map.insert(
        "claude_large_image".into(),
        config.claude_large_image.clone().into(),
    );
    map.insert(
        "claude_large_text".into(),
        config.claude_large_text.clone().into(),
    );
    map.insert(
        "opencode_large_image".into(),
        config.opencode_large_image.clone().into(),
    );
    map.insert(
        "opencode_large_text".into(),
        config.opencode_large_text.clone().into(),
    );
    map.insert(
        "pi_large_image".into(),
        config.pi_large_image.clone().into(),
    );
    map.insert("pi_large_text".into(), config.pi_large_text.clone().into());
    map.insert(
        "omp_large_image".into(),
        config.omp_large_image.clone().into(),
    );
    map.insert(
        "omp_large_text".into(),
        config.omp_large_text.clone().into(),
    );

    map.insert(
        "hermes_large_image".into(),
        config.hermes_large_image.clone().into(),
    );
    map.insert(
        "hermes_large_text".into(),
        config.hermes_large_text.clone().into(),
    );
    // General settings
    map.insert("priority_presence".into(), config.priority_presence.into());
    map.insert("detect_processes".into(), config.detect_processes.into());
    map.insert("detect_codex".into(), config.detect_codex.into());
    map.insert("detect_pi".into(), config.detect_pi.into());
    map.insert("detect_opencode".into(), config.detect_opencode.into());
    map.insert("detect_omp".into(), config.detect_omp.into());
    map.insert("detect_claude".into(), config.detect_claude.into());
    map.insert("detect_hermes".into(), config.detect_hermes.into());
    map.insert("poll_seconds".into(), config.poll_seconds.into());
    map.insert("stale_seconds".into(), config.stale_seconds.into());

    // Paths
    if let Some(p) = config.codex_home.to_str() {
        map.insert("codex_home".into(), p.into());
    }
    if let Some(p) = config.pi_home.to_str() {
        map.insert("pi_home".into(), p.into());
    }
    if let Some(p) = config.omp_home.to_str() {
        map.insert("omp_home".into(), p.into());
    }
    if let Some(p) = config.hermes_home.to_str() {
        map.insert("hermes_home".into(), p.into());
    }

    // Per-agent display settings
    let save_ds = |prefix: &str,
                   ds: &DisplaySettings,
                   map: &mut serde_json::Map<String, serde_json::Value>| {
        map.insert(format!("{prefix}_show_project"), ds.show_project.into());
        map.insert(format!("{prefix}_show_model"), ds.show_model.into());
        map.insert(format!("{prefix}_show_branch"), ds.show_branch.into());
        map.insert(format!("{prefix}_show_activity"), ds.show_activity.into());
        map.insert(format!("{prefix}_show_surface"), ds.show_surface.into());
        map.insert(format!("{prefix}_show_plan"), ds.show_plan.into());
        map.insert(format!("{prefix}_show_tokens"), ds.show_tokens.into());
        map.insert(format!("{prefix}_show_cost"), ds.show_cost.into());
        map.insert(format!("{prefix}_show_context"), ds.show_context.into());
        map.insert(format!("{prefix}_show_limits"), ds.show_limits.into());
        map.insert(format!("{prefix}_flavor_text"), ds.flavor_text.into());
    };

    save_ds("codex", &config.codex_display, &mut map);
    save_ds("claude", &config.claude_display, &mut map);
    save_ds("opencode", &config.opencode_display, &mut map);
    save_ds("pi", &config.pi_display, &mut map);
    save_ds("omp", &config.omp_display, &mut map);
    save_ds("hermes", &config.hermes_display, &mut map);

    let value = serde_json::Value::Object(map);
    let pretty = serde_json::to_string_pretty(&value)?;
    fs::write(config_path, pretty)?;
    Ok(())
}

pub fn string_field(value: &str, key: &str) -> Option<String> {
    json_string_field(value, key)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub fn bool_field(value: &str, key: &str) -> Option<bool> {
    let marker = format!("\"{key}\"");
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

pub fn u64_field(value: &str, key: &str) -> Option<u64> {
    let marker = format!("\"{key}\"");
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

pub fn json_string_field(value: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
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
