use app_server_core::llm::{LlmError, LlmMessage, LlmResponse, LlmToolCall, LlmToolDefinition};
use app_server_core::{
    AgentToolConfirmationScope, AgentToolRunContext, NoopToolLoopObserver, ToolExecutor,
    ToolLoopObserver, WorkspaceToolExecutor, agent_tool_definitions_for_operation,
    run_tool_loop_with_registry,
};
use app_server_protocol::{
    AgentOperationLevel, CadQueryObjectKind, ChatSessionId, SelectionKind, SelectionRef,
};
use std::sync::Mutex;

fn tool_context(
    operation: AgentOperationLevel,
    confirmation_scope: Option<AgentToolConfirmationScope>,
) -> AgentToolRunContext {
    let mut context = AgentToolRunContext::new(std::env::temp_dir(), operation);
    context.confirmation_scope = confirmation_scope;
    context
}

#[test]
fn agent_tool_definitions_returns_expected_tools() {
    let defs = agent_tool_definitions_for_operation(AgentOperationLevel::Inform);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"list_directory"));
    assert!(names.contains(&"resolve_ref"));
    assert!(!names.contains(&"write_file"));
}

#[test]
fn agent_tool_definitions_have_valid_parameters() {
    let defs = agent_tool_definitions_for_operation(AgentOperationLevel::Inform);
    for def in &defs {
        assert_eq!(def.parameters["type"], "object");
        assert!(def.parameters["properties"].is_object());
        assert!(def.parameters["required"].is_array());
    }
}

#[test]
fn workspace_tool_executor_read_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.py");
    std::fs::write(&file_path, "import cadquery").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let call = LlmToolCall {
        id: "call_1".into(),
        function_name: "read_file".into(),
        arguments: "{\"path\": \"test.py\"}".into(),
    };
    let result = executor.execute(&call, &tool_context(AgentOperationLevel::Inform, None));
    assert_eq!(result, "import cadquery");
}

#[test]
fn workspace_tool_executor_read_file_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let call = LlmToolCall {
        id: "call_1".into(),
        function_name: "read_file".into(),
        arguments: "{\"path\": \"nonexistent.py\"}".into(),
    };
    let result = executor.execute(&call, &tool_context(AgentOperationLevel::Inform, None));
    assert!(result.starts_with("Error reading file:"));
}

#[test]
fn workspace_tool_executor_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let call = LlmToolCall {
        id: "call_1".into(),
        function_name: "read_file".into(),
        arguments: "{\"path\": \"../etc/passwd\"}".into(),
    };
    let result = executor.execute(&call, &tool_context(AgentOperationLevel::Inform, None));
    assert!(result.contains("must not contain"));
}

#[test]
fn workspace_tool_executor_list_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.py"), "").unwrap();
    std::fs::create_dir(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("b.md"), "").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let call = LlmToolCall {
        id: "call_1".into(),
        function_name: "list_directory".into(),
        arguments: "{\"path\": \"\"}".into(),
    };
    let result = executor.execute(&call, &tool_context(AgentOperationLevel::Inform, None));
    assert!(result.contains("a.py"));
    assert!(result.contains("b.md"));
    assert!(result.contains("parts/"));
}

#[test]
fn workspace_tool_executor_list_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let call = LlmToolCall {
        id: "call_1".into(),
        function_name: "list_directory".into(),
        arguments: "{\"path\": \"\"}".into(),
    };
    let result = executor.execute(&call, &tool_context(AgentOperationLevel::Inform, None));
    assert_eq!(result, "(empty directory)");
}

#[test]
fn workspace_tool_executor_unknown_tool() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let call = LlmToolCall {
        id: "call_1".into(),
        function_name: "delete_everything".into(),
        arguments: "{}".into(),
    };
    let result = executor.execute(&call, &tool_context(AgentOperationLevel::Inform, None));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "unsupported_tool");
}

#[test]
fn workspace_tool_executor_invalid_json_args() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let call = LlmToolCall {
        id: "call_1".into(),
        function_name: "read_file".into(),
        arguments: "not json".into(),
    };
    let result = executor.execute(&call, &tool_context(AgentOperationLevel::Inform, None));
    assert!(result.starts_with("Error parsing"));
}

struct MockProvider {
    responses: Mutex<Vec<LlmResponse>>,
    tool_names_seen: Mutex<Vec<Vec<String>>>,
}

impl MockProvider {
    fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
            tool_names_seen: Mutex::new(Vec::new()),
        }
    }

    fn tool_names_seen(&self) -> Vec<Vec<String>> {
        self.tool_names_seen.lock().unwrap().clone()
    }
}

impl app_server_core::llm::LlmProvider for MockProvider {
    fn stream_chat(
        &self,
        _messages: Vec<LlmMessage>,
        tools: &[LlmToolDefinition],
        _on_token: &dyn Fn(&str) -> bool,
    ) -> Result<LlmResponse, LlmError> {
        self.tool_names_seen.lock().unwrap().push(
            tools
                .iter()
                .map(|definition| definition.name.clone())
                .collect(),
        );
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            Err(LlmError {
                message: "no more mock responses".into(),
            })
        } else {
            Ok(responses.remove(0))
        }
    }
}

struct EchoExecutor;

impl ToolExecutor for EchoExecutor {
    fn execute(&self, call: &LlmToolCall, _context: &AgentToolRunContext) -> String {
        format!("echo: {} {}", call.function_name, call.arguments)
    }
}

struct CountingExecutor {
    calls: Mutex<Vec<String>>,
}

impl CountingExecutor {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl ToolExecutor for CountingExecutor {
    fn execute(&self, call: &LlmToolCall, _context: &AgentToolRunContext) -> String {
        self.calls.lock().unwrap().push(call.function_name.clone());
        format!("counted: {}", call.function_name)
    }
}

#[derive(Default)]
struct ContextRecordingExecutor {
    contexts: Mutex<Vec<AgentToolRunContext>>,
}

impl ContextRecordingExecutor {
    fn contexts(&self) -> Vec<AgentToolRunContext> {
        self.contexts.lock().unwrap().clone()
    }
}

impl ToolExecutor for ContextRecordingExecutor {
    fn execute(&self, _call: &LlmToolCall, context: &AgentToolRunContext) -> String {
        self.contexts.lock().unwrap().push(context.clone());
        "{\"status\":\"ok\"}".into()
    }
}

#[derive(Default)]
struct RecordingObserver {
    starts: Mutex<Vec<String>>,
    results: Mutex<Vec<String>>,
}

impl ToolLoopObserver for RecordingObserver {
    fn tool_start(&self, call: &LlmToolCall) {
        self.starts.lock().unwrap().push(call.function_name.clone());
    }

    fn tool_result(&self, _call: &LlmToolCall, result: &str) {
        self.results.lock().unwrap().push(result.to_owned());
    }
}

#[test]
fn run_tool_loop_returns_text_when_no_tool_calls() {
    let provider = MockProvider::new(vec![LlmResponse {
        content: "Hello!".into(),
        tool_calls: Vec::new(),
    }]);
    let result = run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "hi")],
        tool_context(AgentOperationLevel::Inform, None),
        &provider,
        &EchoExecutor,
        &NoopToolLoopObserver,
        &|_| true,
    );
    assert_eq!(result.unwrap().content, "Hello!");
}

#[test]
fn run_tool_loop_executes_tools_and_continues() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_1".into(),
                function_name: "read_file".into(),
                arguments: "{\"path\": \"a.py\"}".into(),
            }],
        },
        LlmResponse {
            content: "Based on the file, here is my answer.".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let result = run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "what's in a.py?")],
        tool_context(AgentOperationLevel::Inform, None),
        &provider,
        &EchoExecutor,
        &NoopToolLoopObserver,
        &|_| true,
    );
    assert_eq!(
        result.unwrap().content,
        "Based on the file, here is my answer."
    );
}

#[test]
fn run_tool_loop_handles_multiple_tool_rounds() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "c1".into(),
                function_name: "list_directory".into(),
                arguments: "{\"path\": \"\"}".into(),
            }],
        },
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "c2".into(),
                function_name: "read_file".into(),
                arguments: "{\"path\": \"parts/lid.py\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let result = run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "explore")],
        tool_context(AgentOperationLevel::Inform, None),
        &provider,
        &EchoExecutor,
        &NoopToolLoopObserver,
        &|_| true,
    );
    assert_eq!(result.unwrap().content, "done");
}

#[test]
fn registry_tool_loop_filters_tools_for_auto_before_decision() {
    let provider = MockProvider::new(vec![LlmResponse {
        content: "done".into(),
        tool_calls: Vec::new(),
    }]);
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "auto")],
        tool_context(AgentOperationLevel::Auto, None),
        &provider,
        &EchoExecutor,
        &NoopToolLoopObserver,
        &|_| true,
    )
    .unwrap();
    let seen = provider.tool_names_seen();
    let tools = seen.first().expect("provider should see tools");
    assert!(tools.iter().any(|name| name == "read_file"));
    assert!(tools.iter().any(|name| name == "resolve_ref"));
    assert!(!tools.iter().any(|name| name == "save_cad_plan"));
    assert!(!tools.iter().any(|name| name == "write_file"));
    assert!(!tools.iter().any(|name| name == "cadquery_execute"));
}

#[test]
fn registry_tool_loop_denies_unauthorized_tool_without_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_write".into(),
                function_name: "write_file".into(),
                arguments: "{\"path\":\"parts/a.md\",\"contents\":\"x\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    let response = run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "read only")],
        tool_context(AgentOperationLevel::Inform, None),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert_eq!(response.content, "done");
    assert!(executor.calls().is_empty());
    assert_eq!(observer.starts.lock().unwrap().as_slice(), ["write_file"]);
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["tool_call_id"], "call_write");
    assert_eq!(parsed["error_type"], "permission_denied");
}

#[test]
fn registry_tool_loop_enforces_denied_path_roots_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_outputs".into(),
                function_name: "read_file".into(),
                arguments: "{\"path\":\"outputs/model.step\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "read outputs")],
        tool_context(AgentOperationLevel::Inform, None),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert!(executor.calls().is_empty());
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["tool_call_id"], "call_outputs");
    assert_eq!(parsed["error_type"], "permission_denied");
    assert!(parsed["message"].as_str().unwrap().contains("outputs"));
}

#[test]
fn registry_tool_loop_enforces_staging_path_denial_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_staging".into(),
                function_name: "list_directory".into(),
                arguments: "{\"path\":\".budn_staging\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "inspect staging")],
        tool_context(AgentOperationLevel::Inform, None),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert!(executor.calls().is_empty());
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap()
            .contains(".budn_staging")
    );
}

#[test]
fn registry_tool_loop_enforces_confirmed_file_scope_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_write_scope".into(),
                function_name: "write_file".into(),
                arguments: "{\"path\":\"docs/outside.md\",\"contents\":\"x\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/confirmed.md".into()], Vec::new(), Vec::new());
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "write outside scope")],
        tool_context(AgentOperationLevel::Execute, Some(scope)),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert!(executor.calls().is_empty());
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "permission_denied");
    assert!(parsed["message"].as_str().unwrap().contains("outside"));
}

#[test]
fn registry_tool_loop_denies_plain_file_tool_writes_to_cadquery_model_source() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_model_write".into(),
                function_name: "write_file".into(),
                arguments: "{\"path\":\"parts/lid.py\",\"contents\":\"x\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    let scope =
        AgentToolConfirmationScope::new(vec!["parts/lid.py".into()], Vec::new(), Vec::new());
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "write model source")],
        tool_context(AgentOperationLevel::Execute, Some(scope)),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert!(executor.calls().is_empty());
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "permission_denied");
    assert!(parsed["message"].as_str().unwrap().contains("CadQuery"));
}

#[test]
fn registry_tool_loop_enforces_confirmed_export_targets_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_export_scope".into(),
                function_name: "cadquery_execute".into(),
                arguments: concat!(
                    "{\"target_path\":\"parts/lid.py\",",
                    "\"target_type\":\"part\",",
                    "\"code\":\"def build(params=None): pass\",",
                    "\"export_targets\":[\"outputs/unconfirmed.step\"]}"
                )
                .into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    let scope = AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/confirmed.step".into()],
    );
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "execute outside export scope")],
        tool_context(AgentOperationLevel::Execute, Some(scope)),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert!(executor.calls().is_empty());
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "permission_denied");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap()
            .contains("export target")
    );
}

#[test]
fn registry_tool_loop_requires_export_targets_when_export_formats_are_requested() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_missing_exports".into(),
                function_name: "cadquery_execute".into(),
                arguments: concat!(
                    "{\"target_path\":\"parts/lid.py\",",
                    "\"target_type\":\"part\",",
                    "\"code\":\"def build(params=None): pass\",",
                    "\"export_formats\":[\"step\"]}"
                )
                .into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    let scope = AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "execute without export targets")],
        tool_context(AgentOperationLevel::Execute, Some(scope)),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert!(executor.calls().is_empty());
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "permission_denied");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap()
            .contains("export_targets")
    );
}

#[test]
fn registry_tool_loop_rejects_non_string_export_targets() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_bad_export_target_type".into(),
                function_name: "cadquery_execute".into(),
                arguments: concat!(
                    "{\"target_path\":\"parts/lid.py\",",
                    "\"target_type\":\"part\",",
                    "\"code\":\"def build(params=None): pass\",",
                    "\"export_formats\":[\"step\"],",
                    "\"export_targets\":[123]}"
                )
                .into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    let scope = AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "execute with invalid export target")],
        tool_context(AgentOperationLevel::Execute, Some(scope)),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert!(executor.calls().is_empty());
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "invalid_arguments");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap()
            .contains("export_targets")
    );
}

#[test]
fn registry_tool_loop_records_authorized_tool_start_and_result() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_read".into(),
                function_name: "read_file".into(),
                arguments: "{\"path\":\"README.md\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = CountingExecutor::new();
    let observer = RecordingObserver::default();
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "read")],
        tool_context(AgentOperationLevel::Inform, None),
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert_eq!(executor.calls(), vec!["read_file"]);
    assert_eq!(observer.starts.lock().unwrap().as_slice(), ["read_file"]);
    assert_eq!(
        observer.results.lock().unwrap().as_slice(),
        ["counted: read_file"]
    );
}

#[test]
fn registry_tool_loop_passes_unified_context_to_executor() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_read_context".into(),
                function_name: "read_file".into(),
                arguments: "{\"path\":\"README.md\"}".into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = ContextRecordingExecutor::default();
    let context = AgentToolRunContext {
        workspace_root: std::env::temp_dir(),
        session_id: Some(ChatSessionId("agent-tools".into())),
        run_id: Some("run-1".into()),
        operation: AgentOperationLevel::Inform,
        selections: vec![SelectionRef {
            kind: SelectionKind::Part,
            ref_text: "@part[lid]".into(),
            owner_ref_text: Some("@part[lid]".into()),
            owner_object_kind: Some(CadQueryObjectKind::Part),
            instance_path: None,
            candidate_feature_ref: None,
            build_id: Some("build_1".into()),
            result_id: Some("cq_1".into()),
            ambiguous: false,
        }],
        active_selection_index: Some(0),
        context_refs: vec!["@part[lid]".into()],
        confirmation_scope: None,
    };
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "read with context")],
        context.clone(),
        &provider,
        &executor,
        &NoopToolLoopObserver,
        &|_| true,
    )
    .unwrap();

    let contexts = executor.contexts();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0], context);
}
