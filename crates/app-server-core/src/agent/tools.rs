mod file_write;
mod readonly;
mod registry;
mod semantic;
mod semantic_chat;
mod semantic_export;
mod tool_path_policy;

use std::path::PathBuf;

use app_server_protocol::{AgentOperationLevel, ChatSessionId, SelectionRef};
use serde_json::json;

use crate::llm::{LlmError, LlmMessage, LlmProvider, LlmResponse, LlmToolCall};
pub use registry::{
    AgentSemanticStore, AgentToolCategory, AgentToolPathPolicy, AgentToolPermission, AgentToolSpec,
    CadQueryModelFilePolicy, OutputPathPolicy, agent_tool_definitions_for_operation,
    agent_tool_permission, agent_tool_specs,
};
use tool_path_policy::{
    normalize_scope_paths, validate_registry_tool_intent, validate_tool_path_policy,
};

const MAX_TOOL_ROUNDS: usize = 10;
pub trait ToolExecutor: Send + Sync {
    fn execute(&self, call: &LlmToolCall, context: &AgentToolRunContext) -> String;
}

pub trait ToolLoopObserver: Send + Sync {
    fn tool_start(&self, call: &LlmToolCall);
    fn tool_result(&self, call: &LlmToolCall, result: &str);
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentToolConfirmationScope {
    pub affected_files: Vec<String>,
    pub new_files: Vec<String>,
    pub export_targets: Vec<String>,
}

impl AgentToolConfirmationScope {
    pub fn new(
        affected_files: Vec<String>,
        new_files: Vec<String>,
        export_targets: Vec<String>,
    ) -> Self {
        Self {
            affected_files: normalize_scope_paths(affected_files),
            new_files: normalize_scope_paths(new_files),
            export_targets: normalize_scope_paths(export_targets),
        }
    }

    pub(super) fn contains_affected_file(&self, path: &str) -> bool {
        self.affected_files
            .iter()
            .any(|confirmed| confirmed == path)
    }

    pub(super) fn contains_new_file(&self, path: &str) -> bool {
        self.new_files.iter().any(|confirmed| confirmed == path)
    }

    fn contains_export_target(&self, path: &str) -> bool {
        self.export_targets
            .iter()
            .any(|confirmed| confirmed == path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolRunContext {
    pub workspace_root: PathBuf,
    pub session_id: Option<ChatSessionId>,
    pub run_id: Option<String>,
    pub operation: AgentOperationLevel,
    pub selections: Vec<SelectionRef>,
    pub active_selection_index: Option<u32>,
    pub context_refs: Vec<String>,
    pub confirmation_scope: Option<AgentToolConfirmationScope>,
}

impl AgentToolRunContext {
    pub fn new(workspace_root: PathBuf, operation: AgentOperationLevel) -> Self {
        Self {
            workspace_root,
            session_id: None,
            run_id: None,
            operation,
            selections: Vec::new(),
            active_selection_index: None,
            context_refs: Vec::new(),
            confirmation_scope: None,
        }
    }
}

pub struct NoopToolLoopObserver;

impl ToolLoopObserver for NoopToolLoopObserver {
    fn tool_start(&self, _call: &LlmToolCall) {}

    fn tool_result(&self, _call: &LlmToolCall, _result: &str) {}
}

pub struct WorkspaceToolExecutor {
    workspace_root: PathBuf,
}

impl WorkspaceToolExecutor {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl ToolExecutor for WorkspaceToolExecutor {
    fn execute(&self, call: &LlmToolCall, context: &AgentToolRunContext) -> String {
        if let Some(result) = validate_direct_executor_permission(call, context) {
            return result;
        }
        match call.function_name.as_str() {
            "read_file" => readonly::read_file(&self.workspace_root, call),
            "list_directory" => readonly::list_directory(&self.workspace_root, call),
            "search_files" => readonly::search_files(&self.workspace_root, call),
            "get_project_context" => readonly::get_project_context(&self.workspace_root, call),
            "get_selection" => readonly::get_selection(call, context),
            "resolve_ref" => readonly::resolve_ref(&self.workspace_root, call, context),
            "save_cad_plan" => semantic::save_cad_plan(&self.workspace_root, call, context),
            "update_chat_summary" => {
                semantic_chat::update_chat_summary(&self.workspace_root, call, context)
            }
            "write_file" => file_write::write_file(&self.workspace_root, call, context),
            "patch_file" => file_write::patch_file(&self.workspace_root, call, context),
            "copy_file" => file_write::copy_file(&self.workspace_root, call, context),
            _ => tool_error_json(
                call,
                "tool is registered but not implemented by this executor",
                "unsupported_tool",
            ),
        }
    }
}

fn validate_direct_executor_permission(
    call: &LlmToolCall,
    context: &AgentToolRunContext,
) -> Option<String> {
    let spec = agent_tool_specs()
        .into_iter()
        .find(|spec| spec.definition.name == call.function_name)?;
    let permission = agent_tool_permission(
        &call.function_name,
        context.operation,
        context.confirmation_scope.is_some(),
    );
    if !permission.allowed {
        return Some(tool_error_json(
            call,
            permission.reason,
            "permission_denied",
        ));
    }
    validate_tool_path_policy(
        &call.function_name,
        &call.arguments,
        &spec.path_policy,
        context.confirmation_scope.as_ref(),
    )
    .err()
    .map(|error| tool_error_json(call, &error.message, error.error_type))
}

pub fn run_tool_loop_with_registry(
    initial_messages: Vec<LlmMessage>,
    context: AgentToolRunContext,
    provider: &dyn LlmProvider,
    executor: &dyn ToolExecutor,
    observer: &dyn ToolLoopObserver,
    on_token: &dyn Fn(&str) -> bool,
) -> Result<LlmResponse, LlmError> {
    let tools = agent_tool_definitions_for_operation(context.operation);
    let mut messages = initial_messages;
    let mut last_content = String::new();
    for _ in 0..MAX_TOOL_ROUNDS {
        let response = provider.stream_chat(messages.clone(), &tools, on_token)?;
        if !response.has_tool_calls() {
            return Ok(response);
        }
        last_content = response.content.clone();
        messages.push(LlmMessage::assistant_with_tool_calls(
            response.content.clone(),
            response.tool_calls.clone(),
        ));
        for call in &response.tool_calls {
            observer.tool_start(call);
            let result = execute_registry_tool(call, &context, executor);
            observer.tool_result(call, &result);
            messages.push(LlmMessage::tool_result(call.id.clone(), result));
        }
    }
    Ok(LlmResponse {
        content: if last_content.is_empty() {
            "Agent reached maximum tool call rounds.".into()
        } else {
            last_content
        },
        tool_calls: Vec::new(),
    })
}

fn execute_registry_tool(
    call: &LlmToolCall,
    context: &AgentToolRunContext,
    executor: &dyn ToolExecutor,
) -> String {
    let spec = agent_tool_specs()
        .into_iter()
        .find(|spec| spec.definition.name == call.function_name);
    let permission = agent_tool_permission(
        &call.function_name,
        context.operation,
        context.confirmation_scope.is_some(),
    );
    if !permission.allowed {
        return tool_error_json(call, permission.reason, "permission_denied");
    }
    if let Some(spec) = spec
        && let Err(error) = validate_tool_path_policy(
            &call.function_name,
            &call.arguments,
            &spec.path_policy,
            context.confirmation_scope.as_ref(),
        )
    {
        return tool_error_json(call, &error.message, error.error_type);
    }
    if let Err(error) = validate_registry_tool_intent(
        &call.function_name,
        &call.arguments,
        context.confirmation_scope.as_ref(),
    ) {
        return tool_error_json(call, &error.message, error.error_type);
    }
    executor.execute(call, context)
}

fn tool_error_json(call: &LlmToolCall, message: &str, error_type: &str) -> String {
    json!({
        "status": "error",
        "tool_call_id": call.id,
        "tool": call.function_name,
        "message": message,
        "error_type": error_type,
        "retry_allowed": false
    })
    .to_string()
}
