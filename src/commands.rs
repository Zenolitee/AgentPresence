use std::io;

use crate::config::Config;
use crate::discord::{
    clear_all_presences, clear_other_presences, client_id_for_agent, validate_client_id, DiscordIpc,
};
use crate::harnesses::collect_session;
use crate::presence::activity_payload;
use crate::window::{
    extract_path_from_title, get_active_project_from_window, get_foreground_window_title,
};

pub fn print_status(config: &Config) -> io::Result<()> {
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

pub fn print_debug(_config: &Config) -> io::Result<()> {
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

pub fn publish_once(config: &Config) -> io::Result<()> {
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
