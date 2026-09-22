pub mod claude;
pub mod codex;
pub mod hermes;
pub mod omp;
pub mod opencode;
pub mod pi;

use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::config::Config;
use crate::detect::detect_process_session;
use crate::window::get_active_project_from_window;

use self::claude::collect_claude_session;
use self::codex::collect_codex_session;
use self::hermes::collect_hermes_session;
use self::omp::collect_omp_session;
use self::opencode::collect_opencode_session;
use self::pi::collect_pi_session;

#[derive(Debug, Clone)]
pub struct AgentSession {
    pub agent: AgentKind,
    pub path: Option<PathBuf>,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub surface: Option<String>,
    pub activity: Option<String>,
    pub plan: Option<String>,
    pub tokens: Option<String>,
    pub cost: Option<String>,
    pub context: Option<String>,
    pub limits: Option<String>,
    pub active: bool,
    pub started_at: Option<SystemTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentKind {
    Codex,
    Claude,
    OpenCode,
    Pi,
    OhMyPi,
    Hermes,
}

impl AgentKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::OpenCode => "OpenCode",
            Self::Pi => "Pi",
            Self::OhMyPi => "Oh My Pi",
            Self::Hermes => "Hermes",
        }
    }
}

pub fn collect_session(config: &Config) -> io::Result<Option<AgentSession>> {
    let mut sessions = Vec::new();

    if config.detect_codex {
        if let Some(session) = collect_codex_session(config)? {
            sessions.push(session);
        }
    }
    if config.detect_claude {
        if let Some(session) = collect_claude_session()? {
            sessions.push(session);
        }
    }

    if config.detect_pi {
        if let Some(session) = collect_pi_session(config)? {
            sessions.push(session);
        }
    }
    if config.detect_omp {
        if let Some(session) = collect_omp_session(config)? {
            sessions.push(session);
        }
    }
    if config.detect_hermes {
        if let Some(session) = collect_hermes_session(config)? {
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

    Ok(select_best_session(
        sessions,
        get_active_project_from_window().as_deref(),
    ))
}

pub fn select_best_session(
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
