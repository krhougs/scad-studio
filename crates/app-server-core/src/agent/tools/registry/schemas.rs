mod cadquery;

pub use cadquery::*;
use serde_json::{Map, Value, json};

pub fn empty_input_schema() -> Value {
    object_schema(json!({}), &[])
}

pub fn read_file_input_schema() -> Value {
    object_schema(
        json!({
            "path": string_schema("Workspace-relative file path."),
            "offset": {"type": "integer", "minimum": 0},
            "max_bytes": {"type": "integer", "minimum": 1, "maximum": 65536}
        }),
        &["path"],
    )
}

pub fn list_directory_input_schema() -> Value {
    object_schema(
        json!({
            "path": string_schema("Workspace-relative directory path."),
            "recursive": {"type": "boolean"},
            "max_entries": {"type": "integer", "minimum": 1, "maximum": 500},
            "pattern": string_schema("Optional substring file filter."),
            "kind": {"type": "string", "enum": ["any", "file", "directory"]}
        }),
        &["path"],
    )
}

pub fn search_files_input_schema() -> Value {
    object_schema(
        json!({
            "query": string_schema("Text query."),
            "path": string_schema("Optional workspace-relative search root."),
            "pattern": string_schema("Optional substring file name filter."),
            "max_results": {"type": "integer", "minimum": 1, "maximum": 50}
        }),
        &["query"],
    )
}

pub fn resolve_ref_input_schema() -> Value {
    object_schema(
        json!({"ref_text": string_schema("Visible MVP Ref text.")}),
        &["ref_text"],
    )
}

pub fn save_cad_plan_input_schema() -> Value {
    object_schema(
        json!({
            "title": string_schema("Plan title."),
            "target_ref": string_schema("Primary visible target ref."),
            "resolved_target": string_schema("Workspace target path."),
            "affected_files": string_array_schema(),
            "new_files": string_array_schema(),
            "export_targets": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Runner output paths: outputs/{resolved_target stem}.step, .stl, or .3mf."
            },
            "strategy": string_schema("CadQuery strategy."),
            "risks": string_array_schema(),
            "acceptance": string_array_schema(),
            "execution_boundary": string_schema("Confirmed execution boundary.")
        }),
        &[
            "title",
            "target_ref",
            "resolved_target",
            "affected_files",
            "export_targets",
            "strategy",
            "execution_boundary",
        ],
    )
}

pub fn update_chat_summary_input_schema() -> Value {
    object_schema(
        json!({
            "summary": string_schema("Conversation summary."),
            "goal": string_schema("Current goal."),
            "related_files": string_array_schema(),
            "open_questions": string_array_schema()
        }),
        &["summary", "goal"],
    )
}

pub fn write_file_input_schema() -> Value {
    object_schema(
        json!({
            "path": string_schema("Workspace-relative target path."),
            "contents": string_schema("Complete text contents."),
            "expected_hash": string_schema("Optional expected existing content hash.")
        }),
        &["path", "contents"],
    )
}

pub fn patch_file_input_schema() -> Value {
    object_schema(
        json!({
            "path": string_schema("Workspace-relative target path."),
            "expected_hash": string_schema("Expected existing content hash."),
            "search": string_schema("Exact text to replace."),
            "replace": string_schema("Replacement text.")
        }),
        &["path", "expected_hash", "search", "replace"],
    )
}

pub fn copy_file_input_schema() -> Value {
    object_schema(
        json!({
            "source_path": string_schema("Workspace-relative source path."),
            "target_path": string_schema("Workspace-relative target path."),
            "expected_source_hash": string_schema("Optional expected source hash.")
        }),
        &["source_path", "target_path"],
    )
}

pub fn read_file_success_schema() -> Value {
    success_schema(
        json!({
            "path": string_schema("Workspace-relative file path."),
            "text": string_schema("Returned UTF-8 text."),
            "offset": {"type": "integer"},
            "bytes_read": {"type": "integer"},
            "file_size": {"type": "integer"},
            "truncated": {"type": "boolean"},
            "hash": string_schema("Stable content hash.")
        }),
        &[
            "path",
            "text",
            "offset",
            "bytes_read",
            "file_size",
            "truncated",
            "hash",
        ],
    )
}

pub fn list_directory_success_schema() -> Value {
    success_schema(
        json!({
            "path": string_schema("Workspace-relative directory path."),
            "entries": {"type": "array", "items": {"type": "object"}},
            "entry_count": {"type": "integer"},
            "truncated": {"type": "boolean"}
        }),
        &["path", "entries", "entry_count", "truncated"],
    )
}

pub fn search_files_success_schema() -> Value {
    success_schema(
        json!({
            "query": string_schema("Search query."),
            "matches": {"type": "array", "items": {"type": "object"}},
            "truncated": {"type": "boolean"}
        }),
        &["query", "matches", "truncated"],
    )
}

pub fn project_context_success_schema() -> Value {
    success_schema(
        json!({
            "objects": {"type": "array", "items": {"type": "object"}},
            "plans": {"type": "array", "items": {"type": "object"}},
            "chats": {"type": "array", "items": {"type": "object"}},
            "warnings": string_array_schema()
        }),
        &["objects", "plans", "chats", "warnings"],
    )
}

pub fn selection_success_schema() -> Value {
    success_schema(
        json!({
            "selections": {"type": "array", "items": {"type": "object"}},
            "active_index": {"type": ["integer", "null"]},
            "context_refs": string_array_schema()
        }),
        &["selections", "active_index", "context_refs"],
    )
}

pub fn resolve_ref_success_schema() -> Value {
    success_schema(
        json!({
            "owner_ref_text": nullable_string_schema(),
            "owner_path": nullable_string_schema(),
            "owner_doc_path": nullable_string_schema(),
            "raw_ref_text": nullable_string_schema(),
            "candidate_feature_ref": nullable_string_schema(),
            "stable_ref": nullable_string_schema(),
            "ambiguous": {"type": "boolean"},
            "risks": string_array_schema()
        }),
        &[
            "owner_ref_text",
            "owner_path",
            "owner_doc_path",
            "raw_ref_text",
            "stable_ref",
            "ambiguous",
            "risks",
        ],
    )
}

pub fn save_cad_plan_success_schema() -> Value {
    success_schema(
        json!({
            "plan_ref": string_schema("Plan path under plans/."),
            "display_path": string_schema("User-visible path."),
            "hash": string_schema("Stable content hash."),
            "summary": string_schema("Short plan summary."),
            "target_ref": string_schema("Visible target ref."),
            "target_path": string_schema("Resolved CadQuery target path."),
            "affected_files": string_array_schema(),
            "new_files": string_array_schema(),
            "export_targets": string_array_schema(),
            "execution_boundary": string_schema("Confirmed execution boundary."),
            "run_id": nullable_string_schema()
        }),
        &[
            "plan_ref",
            "display_path",
            "hash",
            "summary",
            "target_ref",
            "target_path",
            "affected_files",
            "new_files",
            "export_targets",
            "execution_boundary",
            "run_id",
        ],
    )
}

pub fn update_chat_summary_success_schema() -> Value {
    success_schema(
        json!({
            "session_id": string_schema("Chat session id."),
            "message_id": string_schema("Meta message id."),
            "updated_fields": string_array_schema()
        }),
        &["session_id", "message_id", "updated_fields"],
    )
}

pub fn file_write_success_schema() -> Value {
    success_schema(
        json!({
            "path": string_schema("Workspace-relative written path."),
            "hash": string_schema("Stable content hash."),
            "created": {"type": "boolean"},
            "conflict": {"type": "boolean"}
        }),
        &["path", "hash", "created", "conflict"],
    )
}

pub fn tool_error_schema() -> Value {
    object_schema(
        json!({
            "status": {"type": "string", "const": "error"},
            "tool_call_id": string_schema("LLM tool call id."),
            "tool": string_schema("Tool name."),
            "message": string_schema("Human-readable failure."),
            "error_type": {
                "type": "string",
                "enum": [
                    "permission_denied",
                    "unsupported_tool",
                    "invalid_arguments",
                    "not_found",
                    "file_conflict",
                    "cancelled",
                    "python_import_error",
                    "cadquery_build_error",
                    "topology_mapping_error",
                    "export_error",
                    "timeout"
                ]
            },
            "retry_allowed": {"type": "boolean"},
            "diagnostics": {"type": "object"}
        }),
        &[
            "status",
            "tool_call_id",
            "tool",
            "message",
            "error_type",
            "retry_allowed",
        ],
    )
}

pub(super) fn success_schema(properties: Value, required: &[&str]) -> Value {
    let mut properties = value_object(properties);
    properties.insert("status".into(), json!({"type": "string", "const": "ok"}));
    properties.insert("tool".into(), string_schema("Tool name."));
    properties.insert("message".into(), string_schema("Human-readable summary."));
    let mut required_fields = vec!["status", "tool"];
    required_fields.extend_from_slice(required);
    object_schema(Value::Object(properties), &required_fields)
}

pub(super) fn object_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

pub(super) fn string_schema(description: &'static str) -> Value {
    json!({"type": "string", "description": description})
}

pub(super) fn nullable_string_schema() -> Value {
    json!({"type": ["string", "null"]})
}

pub(super) fn string_array_schema() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

pub(super) fn value_object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}
