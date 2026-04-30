use std::path::{Path, PathBuf};

use crate::agent::tools::tool_error_json;
use crate::llm::LlmToolCall;
use tokio::fs;

use super::is_denied_path;

pub(super) fn normalize_workspace_path(
    relative: &str,
    call: &LlmToolCall,
) -> Result<String, String> {
    let cleaned = relative.replace('\\', "/");
    if cleaned.starts_with('/') || cleaned.contains(':') {
        return Err(tool_error_json(
            call,
            "path must be workspace-relative",
            "permission_denied",
        ));
    }
    let cleaned = cleaned.trim_matches('/');
    if cleaned.split('/').any(|segment| segment == "..") {
        return Err(tool_error_json(
            call,
            "path must not contain '..'",
            "permission_denied",
        ));
    }
    Ok(cleaned
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

pub(super) fn workspace_relative_path(workspace_root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(workspace_root)
        .ok()?
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .map(|segments| segments.join("/"))
}

pub(super) async fn safe_existing_relative_path(
    workspace_root: &Path,
    path: &Path,
) -> Option<String> {
    let root = fs::canonicalize(workspace_root)
        .await
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let canonical = fs::canonicalize(path).await.ok()?;
    let relative = workspace_relative_path(&root, &canonical)?;
    (!is_denied_path(&relative)).then_some(relative)
}

pub(super) async fn safe_file_path(workspace_root: &Path, relative: &str) -> Option<PathBuf> {
    let root = fs::canonicalize(workspace_root)
        .await
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let path = fs::canonicalize(root.join(relative)).await.ok()?;
    let canonical_relative = workspace_relative_path(&root, &path)?;
    let is_file = fs::metadata(&path)
        .await
        .is_ok_and(|metadata| metadata.is_file());
    (!is_denied_path(&canonical_relative) && is_file).then_some(path)
}
