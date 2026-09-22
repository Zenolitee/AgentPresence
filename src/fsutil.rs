use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

pub const MAX_TAIL_BYTES: u64 = 256 * 1024;

pub fn read_json_object_file(path: &Path) -> io::Result<Value> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str::<Value>(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn newest_jsonl(dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    visit_jsonl(dir, &mut newest)?;
    Ok(newest.map(|(path, _)| path))
}

pub fn newest_matching_file(dir: &Path, prefix: &str, suffix: &str) -> io::Result<Option<PathBuf>> {
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

pub fn most_recent_file_mtime(dir: &Path) -> Option<SystemTime> {
    if !dir.exists() {
        return None;
    }
    let mut newest: Option<SystemTime> = None;
    let _ = visit_mtime(dir, &mut newest);
    newest
}

pub fn visit_mtime(dir: &Path, newest: &mut Option<SystemTime>) -> io::Result<()> {
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

pub fn visit_jsonl(dir: &Path, newest: &mut Option<(PathBuf, SystemTime)>) -> io::Result<()> {
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

pub fn read_tail(path: &Path, max_bytes: u64) -> io::Result<String> {
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

pub fn read_head(path: &Path, max_bytes: u64) -> io::Result<String> {
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

pub fn git_branch_for_path(path: &Path) -> Option<String> {
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

pub fn branch_from_git_dir(git_dir: &Path) -> Option<String> {
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

pub fn home_dir() -> io::Result<PathBuf> {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))
}

pub fn parse_iso8601_utc(_value: &str) -> Option<SystemTime> {
    None
}
