use std::{fs, path::Path};

use serde_json::{Value, json};

use crate::llm::LlmToolCall;

use super::{
    canonical_or_original, collect_files, matches_pattern, optional_string_arg, parse_object,
    resolve_existing_path, string_arg, text::is_probably_binary, usize_arg,
};

const MAX_SEARCH_FILE_BYTES: usize = 256 * 1024;
const MAX_SEARCH_RESULTS: usize = 50;

pub(super) fn search_files(workspace_root: &Path, call: &LlmToolCall) -> String {
    let workspace_root = canonical_or_original(workspace_root);
    let args = match search_files_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let base = match resolve_existing_path(&workspace_root, &args.path, call) {
        Ok(path) => path,
        Err(result) => return result,
    };
    let mut files = Vec::new();
    collect_files(&workspace_root, &base, &mut files);
    files.sort();
    let (matches, truncated) = search_file_matches(&workspace_root, files, &args);
    search_files_success(call, &args.query, matches, truncated).to_string()
}

struct SearchFilesArgs {
    query: String,
    path: String,
    pattern: Option<String>,
    max_results: usize,
}

fn search_files_args(call: &LlmToolCall) -> Result<SearchFilesArgs, String> {
    let args = parse_object(&call.arguments, call)?;
    Ok(SearchFilesArgs {
        query: string_arg(&args, "query", call)?,
        path: optional_string_arg(&args, "path").unwrap_or_default(),
        pattern: optional_string_arg(&args, "pattern"),
        max_results: usize_arg(&args, "max_results")
            .unwrap_or(MAX_SEARCH_RESULTS)
            .min(MAX_SEARCH_RESULTS),
    })
}

fn search_file_matches(
    workspace_root: &Path,
    files: Vec<String>,
    args: &SearchFilesArgs,
) -> (Vec<Value>, bool) {
    let mut matches = Vec::new();
    for relative in files
        .into_iter()
        .filter(|path| matches_pattern(path, &args.pattern))
    {
        if push_file_matches(workspace_root, &relative, args, &mut matches) {
            return (matches, true);
        }
    }
    (matches, false)
}

fn push_file_matches(
    workspace_root: &Path,
    relative: &str,
    args: &SearchFilesArgs,
    matches: &mut Vec<Value>,
) -> bool {
    let Some(text) = readable_search_text(&workspace_root.join(relative)) else {
        return false;
    };
    for (line_index, line) in text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(&args.query))
    {
        if matches.len() >= args.max_results {
            return true;
        }
        matches.push(json!({
            "path": relative,
            "line_number": line_index + 1,
            "line": line
        }));
    }
    false
}

fn readable_search_text(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if is_probably_binary(&bytes) {
        return None;
    }
    (bytes.len() <= MAX_SEARCH_FILE_BYTES)
        .then(|| String::from_utf8(bytes).ok())
        .flatten()
}

fn search_files_success(
    call: &LlmToolCall,
    query: &str,
    matches: Vec<Value>,
    truncated: bool,
) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "files searched",
        "query": query,
        "matches": matches,
        "truncated": truncated
    })
}
