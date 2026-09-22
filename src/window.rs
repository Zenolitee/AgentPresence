#[cfg(windows)]
use std::ffi::c_void;
use std::path::{Path, PathBuf};

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowTextW(hWnd: *mut c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
}

pub fn get_foreground_window_title() -> Option<String> {
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

pub fn extract_path_from_title(title: &str) -> Option<PathBuf> {
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

pub fn get_active_project_from_window() -> Option<String> {
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
