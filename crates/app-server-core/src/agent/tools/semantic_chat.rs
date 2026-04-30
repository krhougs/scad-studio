use std::path::Path;

use app_server_protocol::{ChatSessionId, PathHandle};
use serde_json::{Value, json};

use crate::{
    chat::{ChatStore, ChatSummaryUpdate},
    llm::LlmToolCall,
};

use super::{
    AgentToolRunContext,
    semantic::{
        first_segment, non_empty_string_arg, normalize_workspace_path, optional_string_array,
        parse_object, path_handle,
    },
    tool_error_json,
};

const CHAT_RELATED_FILE_ROOTS: &[&str] = &[
    "components",
    "parts",
    "assemblies",
    "plans",
    "refs",
    "docs",
    "outputs",
];
const CHAT_RELATED_FILE_DENIED_ROOTS: &[&str] =
    &[".git", "target", "node_modules", "chats", ".budn_staging"];

pub(super) fn update_chat_summary(
    workspace_root: &Path,
    call: &LlmToolCall,
    context: &AgentToolRunContext,
) -> String {
    let session_id = match context.session_id.as_ref() {
        Some(session_id) => session_id,
        None => {
            return tool_error_json(
                call,
                "update_chat_summary requires an active Chat session",
                "invalid_arguments",
            );
        }
    };
    let args = match chat_summary_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let update = ChatSummaryUpdate {
        summary: args.summary,
        goal: args.goal,
        related_files: args.related_files,
        open_questions: args.open_questions,
    };
    match ChatStore::new(workspace_root.to_path_buf()).update_summary(session_id, update) {
        Ok(ack) => chat_summary_success(call, session_id, &ack.message_id).to_string(),
        Err(error) => tool_error_json(call, &error.message, "invalid_arguments"),
    }
}

struct ChatSummaryArgs {
    summary: String,
    goal: String,
    related_files: Vec<PathHandle>,
    open_questions: Vec<String>,
}

fn chat_summary_args(call: &LlmToolCall) -> Result<ChatSummaryArgs, String> {
    let value = parse_object(call)?;
    let related_files = optional_string_array(&value, "related_files", call)?
        .into_iter()
        .map(|path| related_path_handle(&path, call))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ChatSummaryArgs {
        summary: non_empty_string_arg(&value, "summary", call)?,
        goal: non_empty_string_arg(&value, "goal", call)?,
        related_files,
        open_questions: optional_string_array(&value, "open_questions", call)?,
    })
}

fn related_path_handle(path: &str, call: &LlmToolCall) -> Result<PathHandle, String> {
    let normalized = normalize_workspace_path(path, call)?;
    let root = first_segment(&normalized);
    if CHAT_RELATED_FILE_DENIED_ROOTS.contains(&root) {
        return Err(tool_error_json(
            call,
            &format!("path root '{root}' is denied for this tool"),
            "permission_denied",
        ));
    }
    if !CHAT_RELATED_FILE_ROOTS.contains(&root) {
        return Err(tool_error_json(
            call,
            &format!("path root '{root}' is not allowed for this tool"),
            "permission_denied",
        ));
    }
    path_handle(&normalized, call)
}

fn chat_summary_success(call: &LlmToolCall, session_id: &ChatSessionId, message_id: &str) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "Chat summary updated",
        "session_id": session_id.0,
        "message_id": message_id,
        "updated_fields": ["summary", "goal", "related_files", "open_questions"]
    })
}
