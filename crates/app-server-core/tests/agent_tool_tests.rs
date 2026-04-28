use app_server_core::llm::{LlmError, LlmMessage, LlmResponse, LlmToolCall, LlmToolDefinition};
use app_server_core::{ToolExecutor, WorkspaceToolExecutor, agent_tool_definitions, run_tool_loop};
use std::sync::Mutex;

#[test]
fn agent_tool_definitions_returns_expected_tools() {
    let defs = agent_tool_definitions();
    assert_eq!(defs.len(), 2);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"list_directory"));
}

#[test]
fn agent_tool_definitions_have_valid_parameters() {
    let defs = agent_tool_definitions();
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
    let result = executor.execute(&call);
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
    let result = executor.execute(&call);
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
    let result = executor.execute(&call);
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
    let result = executor.execute(&call);
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
    let result = executor.execute(&call);
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
    let result = executor.execute(&call);
    assert!(result.starts_with("Unknown tool:"));
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
    let result = executor.execute(&call);
    assert!(result.starts_with("Error parsing"));
}

struct MockProvider {
    responses: Mutex<Vec<LlmResponse>>,
}

impl MockProvider {
    fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
        }
    }
}

impl app_server_core::llm::LlmProvider for MockProvider {
    fn stream_chat(
        &self,
        _messages: Vec<LlmMessage>,
        _tools: &[LlmToolDefinition],
        _on_token: &dyn Fn(&str) -> bool,
    ) -> Result<LlmResponse, LlmError> {
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
    fn execute(&self, call: &LlmToolCall) -> String {
        format!("echo: {} {}", call.function_name, call.arguments)
    }
}

#[test]
fn run_tool_loop_returns_text_when_no_tool_calls() {
    let provider = MockProvider::new(vec![LlmResponse {
        content: "Hello!".into(),
        tool_calls: Vec::new(),
    }]);
    let result = run_tool_loop(
        vec![LlmMessage::new("user", "hi")],
        &[],
        &provider,
        &EchoExecutor,
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
    let result = run_tool_loop(
        vec![LlmMessage::new("user", "what's in a.py?")],
        &agent_tool_definitions(),
        &provider,
        &EchoExecutor,
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
    let result = run_tool_loop(
        vec![LlmMessage::new("user", "explore")],
        &agent_tool_definitions(),
        &provider,
        &EchoExecutor,
        &|_| true,
    );
    assert_eq!(result.unwrap().content, "done");
}
