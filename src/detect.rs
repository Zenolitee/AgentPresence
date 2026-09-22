use std::io;
use std::time::{Duration, Instant, SystemTime};

use parking_lot::Mutex;

use crate::config::Config;
use crate::harnesses::{AgentKind, AgentSession};

/// Cache of first detection time per agent kind.
/// Keyed by agent kind discriminant (0=Codex, 1=Claude, 2=OpenCode, 3=Pi, 4=OhMyPi, 5=Hermes).
pub static FIRST_SEEN: Mutex<Option<[Option<SystemTime>; 6]>> = Mutex::new(None);

/// Shared snapshot of the running-process listing, refreshed at most once per
/// [`CACHE_TTL`]. See `running_process_text`.
static PROCESS_TEXT_CACHE: Mutex<Option<(String, Instant)>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(1);

/// Get or initialize the first-seen time for an agent kind.
/// Returns the cached time if already seen, or records and returns the current time.
pub fn get_or_init_first_seen(agent: AgentKind) -> SystemTime {
    let idx = agent as usize;
    let mut cache = FIRST_SEEN.lock();
    let slots = cache.get_or_insert_with(|| [None; 6]);
    *slots[idx].get_or_insert(SystemTime::now())
}

/// Clear all cached first-seen times (call when no process is detected).
pub fn clear_first_seen_cache() {
    let mut cache = FIRST_SEEN.lock();
    *cache = None;
}

pub fn detect_process_session(config: &Config) -> Option<AgentSession> {
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
        started_at: Some(get_or_init_first_seen(agent)),
    })
}

pub fn detect_agent_from_process_text(processes: &str, config: &Config) -> Option<AgentKind> {
    let lower = processes.to_ascii_lowercase();

    if config.detect_opencode && contains_agent_process(&lower, &["opencode", "sst-dev.opencode"]) {
        return Some(AgentKind::OpenCode);
    }

    if config.detect_claude
        && contains_agent_process(&lower, &["claude", "@anthropic-ai/claude-code"])
    {
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

    if config.detect_omp && contains_agent_process(&lower, &["omp"]) {
        return Some(AgentKind::OhMyPi);
    }

    None
}

pub fn contains_agent_process(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

pub fn running_process_text() -> io::Result<String> {
    // One poll cycle calls this up to six times (per-harness liveness checks
    // plus fallback detection); each call spawns PowerShell, so share one
    // scan per second across callers instead of re-spawning every time.
    {
        let cache = PROCESS_TEXT_CACHE.lock();
        if let Some((text, scanned_at)) = cache.as_ref() {
            if scanned_at.elapsed() < CACHE_TTL {
                return Ok(text.clone());
            }
        }
    }

    let text = fetch_process_text()?;
    *PROCESS_TEXT_CACHE.lock() = Some((text.clone(), Instant::now()));
    Ok(text)
}

fn fetch_process_text() -> io::Result<String> {
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
