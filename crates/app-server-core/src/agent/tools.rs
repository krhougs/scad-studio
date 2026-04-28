mod readonly;
mod registry;

use std::path::PathBuf;

use app_server_protocol::{AgentOperationLevel, ChatSessionId, SelectionRef};
use serde_json::json;

use crate::llm::{LlmError, LlmMessage, LlmProvider, LlmResponse, LlmToolCall};
pub use registry::{
    AgentSemanticStore, AgentToolCategory, AgentToolPathPolicy, AgentToolPermission, AgentToolSpec,
    CadQueryModelFilePolicy, OutputPathPolicy, agent_tool_definitions_for_operation,
    agent_tool_permission, agent_tool_specs,
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

    fn contains_confirmed_file(&self, path: &str) -> bool {
        self.affected_files
            .iter()
            .any(|confirmed| confirmed == path)
            || self.new_files.iter().any(|confirmed| confirmed == path)
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
        match call.function_name.as_str() {
            "read_file" => readonly::read_file(&self.workspace_root, call),
            "list_directory" => readonly::list_directory(&self.workspace_root, call),
            "search_files" => readonly::search_files(&self.workspace_root, call),
            "get_project_context" => readonly::get_project_context(&self.workspace_root, call),
            "get_selection" => readonly::get_selection(call, context),
            "resolve_ref" => readonly::resolve_ref(&self.workspace_root, call, context),
            _ => tool_error_json(
                call,
                "tool is registered but not implemented by this executor",
                "unsupported_tool",
            ),
        }
    }
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
            &call.arguments,
            &spec.path_policy,
            context.confirmation_scope.as_ref(),
        )
    {
        return tool_error_json(call, &error.message, error.error_type);
    }
    executor.execute(call, context)
}

struct ToolPolicyError {
    message: String,
    error_type: &'static str,
}

impl ToolPolicyError {
    fn invalid_arguments(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "invalid_arguments",
        }
    }

    fn permission_denied(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "permission_denied",
        }
    }
}

fn validate_tool_path_policy(
    args: &str,
    policy: &AgentToolPathPolicy,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    let parsed: serde_json::Value = serde_json::from_str(args).map_err(|error| {
        ToolPolicyError::invalid_arguments(format!("invalid tool arguments: {error}"))
    })?;
    let paths = collect_workspace_path_args(&parsed);
    for (field, path) in paths {
        let normalized = validate_one_tool_path(&path, policy)?;
        validate_cadquery_model_file_policy(&field, &normalized, policy)?;
        validate_confirmation_file_scope(&field, &normalized, policy, confirmation_scope)?;
    }
    let export_targets = parsed
        .get("export_targets")
        .map(parse_export_targets)
        .transpose()?;
    if export_formats_requested(&parsed)
        && policy.output_paths == OutputPathPolicy::ConfirmationOutputsOnly
        && export_targets.as_ref().is_none_or(Vec::is_empty)
    {
        return Err(ToolPolicyError::permission_denied(
            "export_formats require confirmed export_targets",
        ));
    }
    if let Some(exports) = export_targets {
        for export in exports {
            validate_export_target_scope(&export, policy, confirmation_scope)?;
        }
    }
    Ok(())
}

fn collect_workspace_path_args(parsed: &serde_json::Value) -> Vec<(&'static str, String)> {
    let Some(object) = parsed.as_object() else {
        return Vec::new();
    };
    ["path", "source_path", "target_path"]
        .iter()
        .filter_map(|field| {
            object
                .get(*field)
                .and_then(|value| value.as_str())
                .map(|value| (*field, value.to_owned()))
        })
        .collect()
}

fn validate_one_tool_path(
    path: &str,
    policy: &AgentToolPathPolicy,
) -> Result<String, ToolPolicyError> {
    let cleaned = normalize_workspace_path(path)?;
    let root = first_path_segment(&cleaned);
    if policy.denied_roots.iter().any(|denied| *denied == root) {
        return Err(ToolPolicyError::permission_denied(format!(
            "path root '{root}' is denied for this tool"
        )));
    }
    if !policy.allowed_roots.is_empty()
        && !policy
            .allowed_roots
            .iter()
            .any(|allowed| *allowed == "" || *allowed == root)
    {
        return Err(ToolPolicyError::permission_denied(format!(
            "path root '{root}' is not allowed for this tool"
        )));
    }
    Ok(cleaned)
}

fn validate_cadquery_model_file_policy(
    field: &str,
    path: &str,
    policy: &AgentToolPathPolicy,
) -> Result<(), ToolPolicyError> {
    if !is_cadquery_model_path(path) {
        return Ok(());
    }
    match policy.cadquery_model_file {
        CadQueryModelFilePolicy::ReadOnly | CadQueryModelFilePolicy::CadQueryToolOnly => Ok(()),
        CadQueryModelFilePolicy::Denied => Err(ToolPolicyError::permission_denied(
            "CadQuery model .py files must be modified through CadQuery tools",
        )),
        CadQueryModelFilePolicy::CopyOnly if field == "source_path" => Ok(()),
        CadQueryModelFilePolicy::CopyOnly => Err(ToolPolicyError::permission_denied(
            "CadQuery model .py files cannot be the target of copy_file",
        )),
    }
}

fn validate_confirmation_file_scope(
    field: &str,
    path: &str,
    policy: &AgentToolPathPolicy,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    if !policy.requires_confirmation_scope || field == "source_path" {
        return Ok(());
    }
    let Some(scope) = confirmation_scope else {
        return Err(ToolPolicyError::permission_denied(
            "tool requires confirmed execution scope",
        ));
    };
    if scope.contains_confirmed_file(path) {
        Ok(())
    } else {
        Err(ToolPolicyError::permission_denied(format!(
            "path '{path}' is outside confirmed affected_files / new_files"
        )))
    }
}

fn export_formats_requested(parsed: &serde_json::Value) -> bool {
    parsed
        .get("export_formats")
        .and_then(|value| value.as_array())
        .is_some_and(|formats| !formats.is_empty())
}

fn parse_export_targets(value: &serde_json::Value) -> Result<Vec<String>, ToolPolicyError> {
    let Some(targets) = value.as_array() else {
        return Err(ToolPolicyError::invalid_arguments(
            "export_targets must be an array of workspace-relative strings",
        ));
    };
    targets
        .iter()
        .map(|target| {
            let Some(path) = target.as_str() else {
                return Err(ToolPolicyError::invalid_arguments(
                    "export_targets must be an array of workspace-relative strings",
                ));
            };
            normalize_workspace_path(path)
        })
        .collect()
}

fn validate_export_target_scope(
    path: &str,
    policy: &AgentToolPathPolicy,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    if policy.output_paths == OutputPathPolicy::Denied {
        return Err(ToolPolicyError::permission_denied(
            "export_targets are not allowed for this tool",
        ));
    }
    if policy.output_paths != OutputPathPolicy::ConfirmationOutputsOnly {
        return Ok(());
    }
    if first_path_segment(path) != "outputs" {
        return Err(ToolPolicyError::permission_denied(
            "export target must be under outputs/",
        ));
    }
    let Some(scope) = confirmation_scope else {
        return Err(ToolPolicyError::permission_denied(
            "export target requires confirmed execution scope",
        ));
    };
    if scope.contains_export_target(path) {
        Ok(())
    } else {
        Err(ToolPolicyError::permission_denied(format!(
            "export target '{path}' is outside confirmed export_targets"
        )))
    }
}

fn is_cadquery_model_path(path: &str) -> bool {
    matches!(
        first_path_segment(path),
        "components" | "parts" | "assemblies"
    ) && path.ends_with(".py")
}

fn normalize_workspace_path(path: &str) -> Result<String, ToolPolicyError> {
    let cleaned = path.replace('\\', "/");
    if cleaned.starts_with('/') || cleaned.contains(':') {
        return Err(ToolPolicyError::permission_denied(
            "path must be workspace-relative",
        ));
    }
    let cleaned = cleaned.trim_matches('/');
    if cleaned.split('/').any(|segment| segment == "..") {
        return Err(ToolPolicyError::permission_denied(
            "path must not contain '..'",
        ));
    }
    Ok(cleaned
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

fn normalize_scope_paths(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter_map(|path| normalize_workspace_path(&path).ok())
        .collect()
}

fn first_path_segment(path: &str) -> &str {
    path.split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
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
