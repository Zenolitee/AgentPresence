use std::time::SystemTime;

use crate::config::{display_settings_for, Config};
use crate::discord::json_escape;
use crate::format::fit_discord_state;
use crate::harnesses::{AgentKind, AgentSession};

pub const ACTIVE_DETAILS: [&str; 54] = [
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
    "Feeding Hermes tokens",
    "Letting Hermes cook",
    "Trusting Hermes implicitly",
    "Hermes herding cats",
];
pub const IDLE_DETAILS: [&str; 4] = [
    "Codex idle",
    "Waiting in terminal",
    "Standing by",
    "Session quiet",
];

pub fn activity_payload(config: &Config, session: Option<&AgentSession>) -> String {
    let active = session.map(|s| s.active).unwrap_or(false);
    let agent = session.map(|s| s.agent).unwrap_or(AgentKind::Codex);
    let ds = display_settings_for(config, agent);

    let details = if ds.show_activity {
        session
            .and_then(|session| session.activity.as_deref())
            .unwrap_or_else(|| {
                if ds.flavor_text {
                    rotating_detail(active)
                } else if active {
                    active_agent_detail(session)
                } else {
                    idle_agent_detail(session)
                }
            })
    } else if ds.flavor_text {
        rotating_detail(active)
    } else if active {
        active_agent_detail(session)
    } else {
        idle_agent_detail(session)
    };

    let mut state_parts = Vec::new();
    if let Some(session) = session {
        if ds.show_project {
            if let Some(project) = &session.project {
                state_parts.push(project.clone());
            }
        }

        if ds.show_surface {
            if let Some(surface) = &session.surface {
                state_parts.push(surface.clone());
            }
        }

        if ds.show_branch {
            if let Some(branch) = &session.branch {
                state_parts.push(branch.clone());
            }
        }

        if ds.show_model {
            if let Some(model) = &session.model {
                state_parts.push(model.clone());
            }
        }

        if ds.show_plan {
            if let Some(plan) = &session.plan {
                state_parts.push(plan.clone());
            }
        }

        if ds.show_tokens {
            if let Some(tokens) = &session.tokens {
                state_parts.push(tokens.clone());
            }
        }

        if ds.show_cost {
            if let Some(cost) = &session.cost {
                state_parts.push(cost.clone());
            }
        }

        if ds.show_context {
            if let Some(context) = &session.context {
                state_parts.push(context.clone());
            }
        }

        if ds.show_limits {
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

pub fn active_agent_detail(session: Option<&AgentSession>) -> &'static str {
    match session.map(|session| session.agent) {
        Some(AgentKind::Claude) => "Using Claude Code",
        Some(AgentKind::OpenCode) => "OpenCode",
        Some(AgentKind::Pi) => "Pi",
        Some(AgentKind::OhMyPi) => "Oh My Pi",
        Some(AgentKind::Hermes) => "Using Hermes",
        _ => "Using Codex in terminal",
    }
}

pub fn idle_agent_detail(session: Option<&AgentSession>) -> &'static str {
    match session.map(|session| session.agent) {
        Some(AgentKind::Claude) => "Claude Code idle",
        Some(AgentKind::OpenCode) => "OpenCode",
        Some(AgentKind::Pi) => "Pi idle",
        Some(AgentKind::OhMyPi) => "Oh My Pi idle",
        Some(AgentKind::Hermes) => "Hermes idle",
        _ => "Codex idle",
    }
}

pub fn asset_profile(config: &Config, agent: Option<AgentKind>) -> (&str, &str) {
    match agent {
        Some(AgentKind::Claude) => (&config.claude_large_image, &config.claude_large_text),
        Some(AgentKind::OpenCode) => (&config.opencode_large_image, &config.opencode_large_text),
        Some(AgentKind::Pi) => (&config.pi_large_image, &config.pi_large_text),
        Some(AgentKind::OhMyPi) => (&config.omp_large_image, &config.omp_large_text),
        Some(AgentKind::Hermes) => (&config.hermes_large_image, &config.hermes_large_text),
        _ => (&config.large_image, &config.large_text),
    }
}

pub fn rotating_detail(active: bool) -> &'static str {
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
