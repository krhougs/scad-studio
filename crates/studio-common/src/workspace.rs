use std::path::{Path, PathBuf};

const MAX_RECENT_WORKSPACES: usize = 10;

pub fn workspace_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

pub fn remember_workspace(recent: &[PathBuf], path: &Path) -> Vec<PathBuf> {
    let mut next = Vec::with_capacity(MAX_RECENT_WORKSPACES);
    next.push(path.to_path_buf());
    next.extend(recent.iter().filter(|item| item.as_path() != path).cloned());
    next.truncate(MAX_RECENT_WORKSPACES);
    next
}

pub fn sanitize_recent_workspaces(recent: &[PathBuf]) -> Vec<PathBuf> {
    let mut cleaned = Vec::new();
    for path in recent {
        if !path.is_dir() || cleaned.contains(path) {
            continue;
        }
        cleaned.push(path.clone());
        if cleaned.len() == MAX_RECENT_WORKSPACES {
            break;
        }
    }
    cleaned
}
