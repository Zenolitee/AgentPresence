use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::Value;

use crate::detect::{contains_agent_process, running_process_text};
use crate::format::format_provider_model;
use crate::fsutil::{
    git_branch_for_path, home_dir, most_recent_file_mtime, newest_matching_file,
    read_json_object_file,
};

use super::{AgentKind, AgentSession};

pub fn collect_opencode_session() -> io::Result<Option<AgentSession>> {
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

pub fn opencode_activity_is_live() -> bool {
    running_process_text()
        .map(|processes| {
            let lower = processes.to_ascii_lowercase();
            contains_agent_process(&lower, &["opencode", "sst-dev.opencode"])
        })
        .unwrap_or(false)
}

pub fn opencode_project_path(global: &Value) -> Option<PathBuf> {
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

pub fn opencode_branch(workspace: &Value) -> Option<String> {
    encoded_json_field(workspace, "workspace:vcs")?
        .pointer("/value/branch")
        .and_then(Value::as_str)
        .map(|branch| branch.to_string())
        .filter(|branch| !branch.trim().is_empty())
}

pub fn opencode_model(global: &Value, workspace: &Value, cwd: Option<&Path>) -> Option<String> {
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

pub fn opencode_model_from_db(cwd: Option<&Path>) -> Option<String> {
    let dir = home_dir()
        .ok()?
        .join(".local")
        .join("share")
        .join("opencode");
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

pub fn first_opencode_model(session: &Value) -> Option<String> {
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

pub fn encoded_json_field(value: &Value, key: &str) -> Option<Value> {
    let raw = value.get(key)?.as_str()?;
    serde_json::from_str::<Value>(raw).ok()
}
