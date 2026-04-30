use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::fs;

use crate::llm::LlmToolCall;

use super::{AgentToolRunContext, tool_error_json};

mod path;
mod project;
mod refs;
mod search;
mod text;

const MAX_FILE_READ_BYTES: usize = 64 * 1024;
const MAX_DIR_ENTRIES: usize = 500;
const DENIED_ROOTS: &[&str] = &[".git", "target", "node_modules", "outputs", ".budn_staging"];

pub(super) async fn read_file(workspace_root: &Path, call: &LlmToolCall) -> String {
    let workspace_root = canonical_or_original(workspace_root).await;
    let args = match read_file_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let resolved = match resolve_existing_path(&workspace_root, &args.path, call).await {
        Ok(path) => path,
        Err(result) => return result,
    };
    let file = match read_utf8_file(call, &resolved).await {
        Ok(file) => file,
        Err(result) => return result,
    };
    let slice = match text_slice(
        call,
        &file.text,
        file.bytes.len(),
        args.offset,
        args.max_bytes,
    ) {
        Ok(slice) => slice,
        Err(result) => return result,
    };
    read_file_success(call, &args.path, &file.bytes, slice).to_string()
}

struct ReadFileArgs {
    path: String,
    offset: usize,
    max_bytes: usize,
}

struct Utf8File {
    bytes: Vec<u8>,
    text: String,
}

struct TextSlice {
    text: String,
    offset: usize,
    bytes_read: usize,
    truncated: bool,
    file_size: usize,
}

fn read_file_args(call: &LlmToolCall) -> Result<ReadFileArgs, String> {
    let args = parse_object(&call.arguments, call)?;
    Ok(ReadFileArgs {
        path: string_arg(&args, "path", call)?,
        offset: usize_arg(&args, "offset").unwrap_or(0),
        max_bytes: usize_arg(&args, "max_bytes")
            .unwrap_or(MAX_FILE_READ_BYTES)
            .min(MAX_FILE_READ_BYTES),
    })
}

async fn read_utf8_file(call: &LlmToolCall, path: &Path) -> Result<Utf8File, String> {
    let bytes = fs::read(path)
        .await
        .map_err(|error| tool_error_json(call, &format!("读取文件失败: {error}"), "not_found"))?;
    if text::is_probably_binary(&bytes) {
        return Err(tool_error_json(
            call,
            "read_file only supports text files, but the file appears to be binary",
            "invalid_arguments",
        ));
    }
    let text = String::from_utf8(bytes.clone()).map_err(|_| {
        tool_error_json(
            call,
            "read_file only supports UTF-8 text files",
            "invalid_arguments",
        )
    })?;
    Ok(Utf8File { bytes, text })
}

fn text_slice(
    call: &LlmToolCall,
    text: &str,
    file_size: usize,
    offset: usize,
    max_bytes: usize,
) -> Result<TextSlice, String> {
    let start = offset.min(file_size);
    if !text.is_char_boundary(start) {
        return Err(tool_error_json(
            call,
            "offset must be on a UTF-8 character boundary",
            "invalid_arguments",
        ));
    }
    let mut end = start.saturating_add(max_bytes).min(file_size);
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    Ok(TextSlice {
        text: text.get(start..end).unwrap_or("").to_owned(),
        offset: start,
        bytes_read: end.saturating_sub(start),
        truncated: end < file_size,
        file_size,
    })
}

fn read_file_success(call: &LlmToolCall, path: &str, bytes: &[u8], slice: TextSlice) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "file read",
        "path": path,
        "text": slice.text,
        "offset": slice.offset,
        "bytes_read": slice.bytes_read,
        "file_size": slice.file_size,
        "truncated": slice.truncated,
        "hash": sha256_bytes(bytes)
    })
}

pub(super) async fn list_directory(workspace_root: &Path, call: &LlmToolCall) -> String {
    let workspace_root = canonical_or_original(workspace_root).await;
    let args = match list_directory_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let base = match resolve_existing_path(&workspace_root, &args.path, call).await {
        Ok(path) => path,
        Err(result) => return result,
    };
    if !fs::metadata(&base)
        .await
        .is_ok_and(|metadata| metadata.is_dir())
    {
        return tool_error_json(
            call,
            "list_directory path must refer to a directory",
            "invalid_arguments",
        );
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    collect_directory_entries(&workspace_root, &base, &args, &mut entries, &mut truncated).await;
    filter_directory_entries(&mut entries, &args, &mut truncated);
    list_directory_success(call, &args.path, &entries, truncated).to_string()
}

struct ListDirectoryArgs {
    path: String,
    recursive: bool,
    pattern: Option<String>,
    kind: String,
    max_entries: usize,
}

fn list_directory_args(call: &LlmToolCall) -> Result<ListDirectoryArgs, String> {
    let args = parse_object(&call.arguments, call)?;
    Ok(ListDirectoryArgs {
        path: string_arg(&args, "path", call)?,
        recursive: bool_arg(&args, "recursive").unwrap_or(false),
        pattern: optional_string_arg(&args, "pattern"),
        kind: optional_string_arg(&args, "kind").unwrap_or_else(|| "any".into()),
        max_entries: usize_arg(&args, "max_entries")
            .unwrap_or(MAX_DIR_ENTRIES)
            .min(MAX_DIR_ENTRIES),
    })
}

fn filter_directory_entries(
    entries: &mut Vec<DirEntrySummary>,
    args: &ListDirectoryArgs,
    truncated: &mut bool,
) {
    entries.retain(|entry| {
        matches_kind(entry, &args.kind) && matches_pattern(&entry.path, &args.pattern)
    });
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    if entries.len() > args.max_entries {
        entries.truncate(args.max_entries);
        *truncated = true;
    }
}

fn list_directory_success(
    call: &LlmToolCall,
    path: &str,
    entries: &[DirEntrySummary],
    truncated: bool,
) -> Value {
    let entry_values = entries
        .iter()
        .map(|entry| {
            json!({
                "path": entry.path,
                "name": entry.name,
                "kind": entry.kind,
                "size_bytes": entry.size_bytes
            })
        })
        .collect::<Vec<_>>();
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "directory listed",
        "path": path,
        "entries": entry_values,
        "entry_count": entry_values.len(),
        "truncated": truncated
    })
}

pub(super) async fn search_files(workspace_root: &Path, call: &LlmToolCall) -> String {
    search::search_files(workspace_root, call).await
}

pub(super) async fn get_project_context(workspace_root: &Path, call: &LlmToolCall) -> String {
    project::get_project_context(workspace_root, call).await
}

pub(super) fn get_selection(call: &LlmToolCall, context: &AgentToolRunContext) -> String {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "selection returned",
        "selections": context.selections,
        "active_index": context.active_selection_index,
        "context_refs": context.context_refs
    })
    .to_string()
}

pub(super) async fn resolve_ref(
    workspace_root: &Path,
    call: &LlmToolCall,
    context: &AgentToolRunContext,
) -> String {
    let workspace_root = canonical_or_original(workspace_root).await;
    refs::resolve_ref(&workspace_root, call, context).await
}

#[derive(Debug)]
struct DirEntrySummary {
    path: String,
    name: String,
    kind: &'static str,
    size_bytes: Option<u64>,
}

pub(super) fn parse_object(args: &str, call: &LlmToolCall) -> Result<Value, String> {
    serde_json::from_str(args).map_err(|error| {
        tool_error_json(
            call,
            &format!("invalid tool arguments: {error}"),
            "invalid_arguments",
        )
    })
}

pub(super) fn string_arg(args: &Value, key: &str, call: &LlmToolCall) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            tool_error_json(
                call,
                &format!("missing required string argument '{key}'"),
                "invalid_arguments",
            )
        })
}

fn optional_string_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn bool_arg(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

fn usize_arg(args: &Value, key: &str) -> Option<usize> {
    args.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

async fn resolve_existing_path(
    workspace_root: &Path,
    relative: &str,
    call: &LlmToolCall,
) -> Result<PathBuf, String> {
    let relative = path::normalize_workspace_path(relative, call)?;
    let root = first_path_segment(&relative);
    if DENIED_ROOTS.contains(&root) {
        return Err(tool_error_json(
            call,
            &format!("path root '{root}' is denied for this tool"),
            "permission_denied",
        ));
    }
    let root_path = canonical_or_original(workspace_root).await;
    let target = root_path.join(&relative);
    let canonical = match fs::canonicalize(&target).await {
        Ok(path) => path,
        Err(error) => {
            return Err(tool_error_json(
                call,
                &format!("workspace path not found: {error}"),
                "not_found",
            ));
        }
    };
    let canonical_relative = match path::workspace_relative_path(&root_path, &canonical) {
        Some(relative) => relative,
        None => {
            return Err(tool_error_json(
                call,
                "path resolves outside workspace",
                "permission_denied",
            ));
        }
    };
    if is_denied_path(&canonical_relative) {
        let root = first_path_segment(&canonical_relative);
        return Err(tool_error_json(
            call,
            &format!("path root '{root}' is denied for this tool"),
            "permission_denied",
        ));
    }
    Ok(canonical)
}

async fn collect_directory_entries(
    workspace_root: &Path,
    directory: &Path,
    args: &ListDirectoryArgs,
    entries: &mut Vec<DirEntrySummary>,
    truncated: &mut bool,
) {
    let mut directories = vec![directory.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let Ok(mut read_dir) = fs::read_dir(&directory).await else {
            continue;
        };
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if entries.len() > args.max_entries {
                *truncated = true;
                return;
            }
            let path = entry.path();
            let relative = match safe_list_entry_relative_path(workspace_root, &path).await {
                Some(relative) if !is_denied_path(&relative) => relative,
                _ => continue,
            };
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            let is_dir = file_type.is_dir();
            if matches_kind_text(if is_dir { "directory" } else { "file" }, &args.kind)
                && matches_pattern(&relative, &args.pattern)
            {
                if entries.len() >= args.max_entries {
                    *truncated = true;
                    return;
                }
                entries.push(DirEntrySummary {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: relative.clone(),
                    kind: if is_dir { "directory" } else { "file" },
                    size_bytes: if is_dir {
                        None
                    } else {
                        entry.metadata().await.ok().map(|metadata| metadata.len())
                    },
                });
            }
            if args.recursive && is_dir {
                directories.push(path);
            }
        }
    }
}

pub(super) async fn collect_files(
    workspace_root: &Path,
    directory: &Path,
    files: &mut Vec<String>,
) {
    let Ok(directory) = fs::canonicalize(directory).await else {
        return;
    };
    if relative_path(workspace_root, &directory).is_none_or(|relative| is_denied_path(&relative)) {
        return;
    }
    let mut directories = vec![directory];
    while let Some(directory) = directories.pop() {
        let Ok(mut read_dir) = fs::read_dir(&directory).await else {
            continue;
        };
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            let Some(relative) = relative_path(workspace_root, &path) else {
                continue;
            };
            if is_denied_path(&relative) {
                continue;
            }
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if file_type.is_dir() {
                directories.push(path);
            } else if file_type.is_file() {
                files.push(relative);
            }
        }
    }
}

fn matches_kind(entry: &DirEntrySummary, kind: &str) -> bool {
    matches_kind_text(entry.kind, kind)
}

fn matches_kind_text(entry_kind: &str, kind: &str) -> bool {
    kind == "any" || entry_kind == kind
}

fn matches_pattern(path: &str, pattern: &Option<String>) -> bool {
    pattern
        .as_deref()
        .is_none_or(|pattern| path.contains(pattern))
}

fn relative_path(workspace_root: &Path, path: &Path) -> Option<String> {
    path::workspace_relative_path(workspace_root, path)
}

async fn safe_list_entry_relative_path(workspace_root: &Path, path: &Path) -> Option<String> {
    path::safe_existing_relative_path(workspace_root, path).await?;
    relative_path(workspace_root, path)
}

pub(super) fn is_denied_path(relative: &str) -> bool {
    DENIED_ROOTS.contains(&first_path_segment(relative))
}

fn first_path_segment(path: &str) -> &str {
    path.split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
}

async fn canonical_or_original(path: &Path) -> PathBuf {
    fs::canonicalize(path)
        .await
        .unwrap_or_else(|_| path.to_path_buf())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
