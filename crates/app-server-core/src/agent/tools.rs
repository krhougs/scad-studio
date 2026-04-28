use std::path::PathBuf;

use crate::llm::{LlmError, LlmMessage, LlmProvider, LlmResponse, LlmToolCall, LlmToolDefinition};

const MAX_TOOL_ROUNDS: usize = 10;
const MAX_FILE_READ_BYTES: usize = 64 * 1024;
const MAX_DIR_ENTRIES: usize = 500;

pub fn agent_tool_definitions() -> Vec<LlmToolDefinition> {
    vec![
        LlmToolDefinition {
            name: "read_file".into(),
            description: "Read the content of a file in the workspace. Use this to examine CadQuery source files (.py), design notes (.md), or other project files.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Workspace-relative file path, e.g. 'parts/top_lid.py'"
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        LlmToolDefinition {
            name: "list_directory".into(),
            description: "List files and subdirectories in a workspace directory. Returns one entry per line; directories end with '/'.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Workspace-relative directory path, e.g. 'parts' or '' for root"
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
    ]
}

pub trait ToolExecutor: Send + Sync {
    fn execute(&self, call: &LlmToolCall) -> String;
}

pub struct WorkspaceToolExecutor {
    workspace_root: PathBuf,
}

impl WorkspaceToolExecutor {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    fn resolve_safe(&self, relative: &str) -> Result<PathBuf, String> {
        let cleaned = relative.replace('\\', "/");
        let cleaned = cleaned.trim_matches('/');
        if cleaned.split('/').any(|seg| seg == "..") {
            return Err("path must not contain '..'".into());
        }
        let resolved = self.workspace_root.join(cleaned);
        // For existing paths, canonicalize to resolve symlinks and verify containment
        if let Ok(canonical) = resolved.canonicalize() {
            let ws_canonical = self
                .workspace_root
                .canonicalize()
                .unwrap_or_else(|_| self.workspace_root.clone());
            if !canonical.starts_with(&ws_canonical) {
                return Err("path resolves outside workspace".into());
            }
            return Ok(canonical);
        }
        // Non-existent paths: component check + starts_with is sufficient
        // (read/list will report "file not found" error)
        if !resolved.starts_with(&self.workspace_root) {
            return Err("path is outside workspace".into());
        }
        Ok(resolved)
    }

    fn read_file(&self, args: &str) -> String {
        let path = match parse_path_arg(args) {
            Ok(p) => p,
            Err(e) => return e,
        };
        let resolved = match self.resolve_safe(&path) {
            Ok(p) => p,
            Err(e) => return format!("Error: {e}"),
        };
        match std::fs::read(&resolved) {
            Ok(bytes) if bytes.len() > MAX_FILE_READ_BYTES => {
                let truncated = String::from_utf8_lossy(&bytes[..MAX_FILE_READ_BYTES]);
                format!("{truncated}\n\n[truncated at {MAX_FILE_READ_BYTES} bytes]")
            }
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => format!("Error reading file: {e}"),
        }
    }

    fn list_directory(&self, args: &str) -> String {
        let path = match parse_path_arg(args) {
            Ok(p) => p,
            Err(e) => return e,
        };
        let resolved = match self.resolve_safe(&path) {
            Ok(p) => p,
            Err(e) => return format!("Error: {e}"),
        };
        match std::fs::read_dir(&resolved) {
            Ok(entries) => {
                let mut items: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .take(MAX_DIR_ENTRIES + 1)
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        if is_dir {
                            format!("{name}/")
                        } else {
                            name
                        }
                    })
                    .collect();
                items.sort();
                let truncated = items.len() > MAX_DIR_ENTRIES;
                if truncated {
                    items.truncate(MAX_DIR_ENTRIES);
                }
                if items.is_empty() {
                    "(empty directory)".into()
                } else if truncated {
                    format!("{}\n\n[truncated at {MAX_DIR_ENTRIES} entries]", items.join("\n"))
                } else {
                    items.join("\n")
                }
            }
            Err(e) => format!("Error listing directory: {e}"),
        }
    }
}

impl ToolExecutor for WorkspaceToolExecutor {
    fn execute(&self, call: &LlmToolCall) -> String {
        match call.function_name.as_str() {
            "read_file" => self.read_file(&call.arguments),
            "list_directory" => self.list_directory(&call.arguments),
            other => format!("Unknown tool: {other}"),
        }
    }
}

pub fn run_tool_loop(
    initial_messages: Vec<LlmMessage>,
    tools: &[LlmToolDefinition],
    provider: &dyn LlmProvider,
    executor: &dyn ToolExecutor,
    on_token: &dyn Fn(&str) -> bool,
) -> Result<LlmResponse, LlmError> {
    let mut messages = initial_messages;
    let mut last_content = String::new();
    for _ in 0..MAX_TOOL_ROUNDS {
        let response = provider.stream_chat(messages.clone(), tools, on_token)?;
        if !response.has_tool_calls() {
            return Ok(response);
        }
        last_content = response.content.clone();
        messages.push(LlmMessage::assistant_with_tool_calls(
            response.content.clone(),
            response.tool_calls.clone(),
        ));
        for call in &response.tool_calls {
            let result = executor.execute(call);
            messages.push(LlmMessage::tool_result(call.id.clone(), result));
        }
    }
    // Return accumulated content rather than failing — the LLM may have said something useful
    Ok(LlmResponse {
        content: if last_content.is_empty() {
            "Agent reached maximum tool call rounds.".into()
        } else {
            last_content
        },
        tool_calls: Vec::new(),
    })
}

fn parse_path_arg(json_args: &str) -> Result<String, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(json_args).map_err(|e| format!("Error parsing arguments: {e}"))?;
    parsed
        .get("path")
        .and_then(|p| p.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| "Error: missing 'path' argument".into())
}

