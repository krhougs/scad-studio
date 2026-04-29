use app_server_core::llm::{LlmError, LlmMessage, LlmResponse, LlmToolCall, LlmToolDefinition};
use app_server_core::{
    AgentToolConfirmationScope, AgentToolRunContext, CadQueryToolCachedResult,
    CadQueryToolRunRequest, CadQueryToolRunResult, CadQueryToolRuntime, CadQueryToolRuntimeError,
    ChatStore, NoopToolLoopObserver, ToolExecutor, ToolLoopObserver, WorkspaceToolExecutor,
    agent_tool_definitions_for_mode, run_tool_loop_with_registry,
};
use app_server_protocol::{
    AgentMode, CadQueryFeatureFaces, CadQueryMeshPayload, CadQueryObjectKind, CadQueryPartMesh,
    ChatSessionId, EdgeGroup, FaceGroup, PathHandle, PreviewUnit, SelectionKind, SelectionRef,
    VertexPoint, WorkspaceId,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn tool_context(
    mode: AgentMode,
    confirmation_scope: Option<AgentToolConfirmationScope>,
) -> AgentToolRunContext {
    let mut context = AgentToolRunContext::new(std::env::temp_dir(), mode);
    context.confirmation_scope = confirmation_scope;
    context
}

fn call(name: &str, arguments: &str) -> LlmToolCall {
    LlmToolCall {
        id: format!("call_{name}"),
        function_name: name.into(),
        arguments: arguments.into(),
    }
}

fn tool_json(executor: &WorkspaceToolExecutor, call: &LlmToolCall) -> serde_json::Value {
    serde_json::from_str(&executor.execute(call, &tool_context(AgentMode::Agent, None)))
        .expect("tool result should be json")
}

fn tool_json_with_context(
    executor: &WorkspaceToolExecutor,
    call: &LlmToolCall,
    context: &AgentToolRunContext,
) -> serde_json::Value {
    serde_json::from_str(&executor.execute(call, context)).expect("tool result should be json")
}

fn test_path_handle(path: impl IntoIterator<Item = impl Into<String>>) -> PathHandle {
    PathHandle::new(WorkspaceId::new("ws"), path).expect("valid test path")
}

fn test_hash(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

#[test]
fn agent_tool_definitions_returns_expected_tools() {
    let defs = agent_tool_definitions_for_mode(AgentMode::Agent);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"list_directory"));
    assert!(names.contains(&"resolve_ref"));
    assert!(names.contains(&"write_file"));
    assert!(names.contains(&"cadquery_execute"));
}

#[test]
fn agent_tool_definitions_have_valid_parameters() {
    let defs = agent_tool_definitions_for_mode(AgentMode::Agent);
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
    std::fs::write(&file_path, "import cadquery\nbox = 1\n").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "read_file",
            "{\"path\":\"test.py\",\"offset\":7,\"max_bytes\":8}",
        ),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["tool"], "read_file");
    assert_eq!(result["path"], "test.py");
    assert_eq!(result["text"], "cadquery");
    assert_eq!(result["offset"], 7);
    assert_eq!(result["bytes_read"], 8);
    assert_eq!(result["file_size"], 24);
    assert_eq!(result["truncated"], true);
    assert!(result["hash"].as_str().unwrap().starts_with("sha256:"));
}

#[test]
fn workspace_tool_executor_read_file_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("read_file", "{\"path\":\"nonexistent.py\"}"),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "not_found");
}

#[test]
fn workspace_tool_executor_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("read_file", "{\"path\":\"../etc/passwd\"}"),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(result["message"].as_str().unwrap().contains(".."));
}

#[test]
fn workspace_tool_executor_denies_dotted_denied_root_paths() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("outputs")).unwrap();
    std::fs::write(dir.path().join("outputs/model.step"), "solid model").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("read_file", "{\"path\":\"./outputs/model.step\"}"),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(result["message"].as_str().unwrap().contains("outputs"));
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_denies_symlink_to_denied_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("outputs")).unwrap();
    std::fs::write(dir.path().join("outputs/model.step"), "solid model").unwrap();
    std::os::unix::fs::symlink("../outputs/model.step", dir.path().join("parts/model.step"))
        .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("read_file", "{\"path\":\"parts/model.step\"}"),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(result["message"].as_str().unwrap().contains("outputs"));
}

#[test]
fn workspace_tool_executor_read_file_rejects_binary_content() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("mesh.bin"), [0xff, 0xfe]).unwrap();
    std::fs::write(dir.path().join("mesh.txt"), b"solid\0mesh").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("read_file", "{\"path\":\"mesh.bin\"}"));
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(result["message"].as_str().unwrap().contains("UTF-8"));

    let result = tool_json(&executor, &call("read_file", "{\"path\":\"mesh.txt\"}"));
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(result["message"].as_str().unwrap().contains("binary"));
}

#[test]
fn workspace_tool_executor_read_file_clamps_max_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let text = "a".repeat(70 * 1024);
    std::fs::write(dir.path().join("large.txt"), text).unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "read_file",
            "{\"path\":\"large.txt\",\"max_bytes\":1000000}",
        ),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["bytes_read"], 64 * 1024);
    assert_eq!(result["truncated"], true);
}

#[test]
fn workspace_tool_executor_read_file_rejects_non_boundary_offset() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("unicode.txt"), "盖子").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("read_file", "{\"path\":\"unicode.txt\",\"offset\":1}"),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(result["message"].as_str().unwrap().contains("offset"));
}

#[test]
fn workspace_tool_executor_save_cad_plan_writes_structured_markdown_under_plans() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);
    context.run_id = Some("run-42".into());

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Add lid vents",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["parts/top_lid.py"],
                "new_files":["plans/add-lid-vents-notes.md"],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"Cut three rounded vent slots into the top face.",
                "risks":["Maintain wall thickness"],
                "acceptance":["STEP export builds"],
                "execution_boundary":"Only Agent mode CadQuery execution may modify parts/top_lid.py."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["tool"], "save_cad_plan");
    assert_eq!(result["target_ref"], "@part[top_lid]");
    assert_eq!(result["target_path"], "parts/top_lid.py");
    assert_eq!(
        result["affected_files"],
        serde_json::json!(["parts/top_lid.py"])
    );
    assert_eq!(
        result["export_targets"],
        serde_json::json!(["outputs/top_lid.step"])
    );
    assert_eq!(result["run_id"], "run-42");
    assert!(result["hash"].as_str().unwrap().starts_with("sha256:"));

    let plan_ref = result["plan_ref"].as_str().unwrap();
    assert!(plan_ref.starts_with("plans/add-lid-vents"));
    assert!(plan_ref.ends_with(".md"));
    let markdown = std::fs::read_to_string(dir.path().join(plan_ref)).unwrap();
    assert!(markdown.contains("# Add lid vents"));
    assert!(markdown.contains("Target Ref"));
    assert!(markdown.contains("@part[top_lid]"));
    assert!(markdown.contains("parts/top_lid.py"));
    assert!(markdown.contains("outputs/top_lid.step"));
    assert!(markdown.contains("Only Agent mode CadQuery execution may modify parts/top_lid.py."));
}

#[test]
fn workspace_tool_executor_save_cad_plan_rejects_unsafe_scope_paths() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Unsafe plan",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["../secret.py"],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"No write should happen.",
                "execution_boundary":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("plans").exists());
}

#[test]
fn workspace_tool_executor_save_cad_plan_requires_export_targets() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Missing export",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["parts/top_lid.py"],
                "strategy":"Cut three rounded vent slots.",
                "execution_boundary":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("export_targets")
    );
    assert!(!dir.path().join("plans").exists());
}

#[test]
fn workspace_tool_executor_save_cad_plan_requires_target_in_confirmed_scope() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Wrong scope",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["parts/base.py"],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"Cut three rounded vent slots.",
                "execution_boundary":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("resolved_target")
    );
    assert!(!dir.path().join("plans").exists());
}

#[test]
fn workspace_tool_executor_save_cad_plan_rejects_unknown_export_target_extension() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Unknown export",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["parts/top_lid.py"],
                "export_targets":["outputs/top_lid.obj"],
                "strategy":"Cut three rounded vent slots.",
                "execution_boundary":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("export_targets")
    );
    assert!(!dir.path().join("plans").exists());
}

#[test]
fn workspace_tool_executor_save_cad_plan_requires_runner_export_filename() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Add lid vents",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["parts/top_lid.py"],
                "export_targets":["outputs/custom.step"],
                "strategy":"Cut three rounded vent slots.",
                "execution_boundary":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("export_targets")
    );
    assert!(!dir.path().join("plans").exists());
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_save_cad_plan_does_not_write_through_symlink_file() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("plans")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("escaped.md"),
        dir.path().join("plans/add-lid-vents.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Add lid vents",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["parts/top_lid.py"],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"Cut three rounded vent slots.",
                "execution_boundary":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["plan_ref"], "plans/add-lid-vents-2.md");
    assert!(!outside.path().join("escaped.md").exists());
    assert!(dir.path().join("plans/add-lid-vents-2.md").is_file());
}

#[test]
fn workspace_tool_executor_direct_call_denies_save_plan_outside_plan_mode() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Add lid vents",
                "target_ref":"@part[top_lid]",
                "resolved_target":"parts/top_lid.py",
                "affected_files":["parts/top_lid.py"],
                "strategy":"Cut three rounded vent slots.",
                "execution_boundary":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("plans").exists());
}

#[test]
fn workspace_tool_executor_write_file_creates_confirmed_text_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            r##"{"path":"docs/notes.md","contents":"# Notes\n"}"##,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["tool"], "write_file");
    assert_eq!(result["path"], "docs/notes.md");
    assert_eq!(result["created"], true);
    assert_eq!(result["conflict"], false);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "# Notes\n"
    );
}

#[test]
fn workspace_tool_executor_write_file_allows_empty_text_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/empty.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call("write_file", r#"{"path":"docs/empty.md","contents":""}"#),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["created"], true);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/empty.md")).unwrap(),
        ""
    );
}

#[test]
fn workspace_tool_executor_write_file_overwrites_with_matching_hash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "old\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            &format!(
                r#"{{"path":"docs/notes.md","contents":"new\n","expected_hash":"{}"}}"#,
                test_hash("old\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["created"], false);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "new\n"
    );
}

#[test]
fn workspace_tool_executor_write_file_rejects_existing_file_without_hash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "old\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            r#"{"path":"docs/notes.md","contents":"new\n"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "file_conflict");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "old\n"
    );
}

#[test]
fn workspace_tool_executor_write_file_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            r#"{"path":"../escape.md","contents":"escape\n"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("../escape.md").exists());
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_write_file_rejects_symlink_target() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("notes.md"),
        dir.path().join("docs/notes.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            &format!(
                r#"{{"path":"docs/notes.md","contents":"new\n","expected_hash":"{}"}}"#,
                test_hash("")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!outside.path().join("notes.md").exists());
}

#[test]
fn workspace_tool_executor_write_file_rejects_nul_text() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            "{\"path\":\"docs/notes.md\",\"contents\":\"bad\\u0000text\"}",
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(!dir.path().join("docs/notes.md").exists());
}

#[test]
fn workspace_tool_executor_write_file_rejects_plans_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["plans/manual.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            r##"{"path":"plans/manual.md","contents":"# Manual plan\n"}"##,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("plans/manual.md").exists());
}

#[test]
fn workspace_tool_executor_patch_file_replaces_expected_hash_content() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "alpha\nbeta\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"beta\n","replace":"gamma\n"}}"#,
                test_hash("alpha\nbeta\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["created"], false);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "alpha\ngamma\n"
    );
}

#[test]
fn workspace_tool_executor_patch_file_allows_empty_replace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "alpha\nbeta\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"beta\n","replace":""}}"#,
                test_hash("alpha\nbeta\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "alpha\n"
    );
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_patch_file_rejects_hard_link_alias_to_model_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "alpha\nbeta\n").unwrap();
    std::fs::hard_link(
        dir.path().join("parts/lid.py"),
        dir.path().join("docs/notes.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"beta\n","replace":"gamma\n"}}"#,
                test_hash("alpha\nbeta\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("parts/lid.py")).unwrap(),
        "alpha\nbeta\n"
    );
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_patch_file_rejects_symlink_target_to_model_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "alpha\nbeta\n").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("parts/lid.py"),
        dir.path().join("docs/notes.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"beta\n","replace":"gamma\n"}}"#,
                test_hash("alpha\nbeta\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("parts/lid.py")).unwrap(),
        "alpha\nbeta\n"
    );
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_patch_file_rejects_symlink_target_to_chat_log() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("chats")).unwrap();
    std::fs::write(dir.path().join("chats/session.jsonl"), "alpha\nbeta\n").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("chats/session.jsonl"),
        dir.path().join("docs/notes.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"beta\n","replace":"gamma\n"}}"#,
                test_hash("alpha\nbeta\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("chats/session.jsonl")).unwrap(),
        "alpha\nbeta\n"
    );
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_patch_file_rejects_symlink_target_to_unconfirmed_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/real.md"), "alpha\nbeta\n").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("docs/real.md"),
        dir.path().join("docs/notes.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"beta\n","replace":"gamma\n"}}"#,
                test_hash("alpha\nbeta\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/real.md")).unwrap(),
        "alpha\nbeta\n"
    );
}

#[test]
fn workspace_tool_executor_patch_file_rejects_hash_conflict() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "alpha\nbeta\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            r#"{"path":"docs/notes.md","expected_hash":"sha256:bad","search":"beta\n","replace":"gamma\n"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "file_conflict");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "alpha\nbeta\n"
    );
}

#[test]
fn workspace_tool_executor_patch_file_rejects_ambiguous_search_text() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "item\nitem\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"item\n","replace":"done\n"}}"#,
                test_hash("item\nitem\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "file_conflict");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "item\nitem\n"
    );
}

#[test]
fn workspace_tool_executor_write_file_rejects_existing_file_confirmed_as_new_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "old\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            &format!(
                r#"{{"path":"docs/notes.md","contents":"new\n","expected_hash":"{}"}}"#,
                test_hash("old\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "file_conflict");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "old\n"
    );
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_write_file_rejects_hard_link_alias_to_chat_log() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("chats")).unwrap();
    std::fs::write(dir.path().join("chats/session.jsonl"), "old\n").unwrap();
    std::fs::hard_link(
        dir.path().join("chats/session.jsonl"),
        dir.path().join("docs/notes.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            &format!(
                r#"{{"path":"docs/notes.md","contents":"new\n","expected_hash":"{}"}}"#,
                test_hash("old\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("chats/session.jsonl")).unwrap(),
        "old\n"
    );
}

#[test]
fn workspace_tool_executor_patch_file_rejects_existing_file_confirmed_as_new_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "alpha\nbeta\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "patch_file",
            &format!(
                r#"{{"path":"docs/notes.md","expected_hash":"{}","search":"beta\n","replace":"gamma\n"}}"#,
                test_hash("alpha\nbeta\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/notes.md")).unwrap(),
        "alpha\nbeta\n"
    );
}

#[test]
fn workspace_tool_executor_direct_write_file_rejects_cadquery_model_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["parts/lid.py".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            r#"{"path":"parts/lid.py","contents":"def build(params=None): pass\n"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("parts/lid.py").exists());
}

#[test]
fn workspace_tool_executor_copy_file_copies_confirmed_text_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), "source\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            &format!(
                r#"{{"source_path":"docs/source.md","target_path":"docs/copy.md","expected_source_hash":"{}"}}"#,
                test_hash("source\n")
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["path"], "docs/copy.md");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/copy.md")).unwrap(),
        "source\n"
    );
}

#[test]
fn workspace_tool_executor_copy_file_rejects_binary_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), b"bad\0text").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"docs/source.md","target_path":"docs/copy.md"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(!dir.path().join("docs/copy.md").exists());
}

#[test]
fn workspace_tool_executor_copy_file_rejects_existing_target() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), "source\n").unwrap();
    std::fs::write(dir.path().join("docs/copy.md"), "existing\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"docs/source.md","target_path":"docs/copy.md"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "file_conflict");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/copy.md")).unwrap(),
        "existing\n"
    );
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_copy_file_rejects_symlink_target() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), "source\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("copy.md"),
        dir.path().join("docs/copy.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"docs/source.md","target_path":"docs/copy.md"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!outside.path().join("copy.md").exists());
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_copy_file_rejects_hard_link_alias_target() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), "source\n").unwrap();
    std::fs::write(dir.path().join("docs/other.md"), "other\n").unwrap();
    std::fs::hard_link(
        dir.path().join("docs/other.md"),
        dir.path().join("docs/copy.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"docs/source.md","target_path":"docs/copy.md"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/other.md")).unwrap(),
        "other\n"
    );
}

#[test]
fn workspace_tool_executor_copy_file_rejects_source_hash_conflict() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), "source\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"docs/source.md","target_path":"docs/copy.md","expected_source_hash":"sha256:bad"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "file_conflict");
    assert!(!dir.path().join("docs/copy.md").exists());
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_copy_file_rejects_hard_link_alias_source_to_chat_log() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("chats")).unwrap();
    std::fs::write(dir.path().join("chats/session.jsonl"), "secret\n").unwrap();
    std::fs::hard_link(
        dir.path().join("chats/session.jsonl"),
        dir.path().join("docs/source.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"docs/source.md","target_path":"docs/copy.md"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("docs/copy.md").exists());
}

#[test]
fn workspace_tool_executor_copy_file_allows_model_source_to_confirmed_new_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "def build(params=None):\n    pass\n",
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentToolConfirmationScope::new(
        Vec::new(),
        vec!["parts/lid_variant.py".into()],
        Vec::new(),
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"parts/lid.py","target_path":"parts/lid_variant.py"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("parts/lid_variant.py")).unwrap(),
        "def build(params=None):\n    pass\n"
    );
}

#[test]
fn workspace_tool_executor_copy_file_rejects_model_target_not_in_new_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "def build(): pass\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentToolConfirmationScope::new(
        vec!["parts/lid_variant.py".into()],
        Vec::new(),
        Vec::new(),
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"parts/lid.py","target_path":"parts/lid_variant.py"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("parts/lid_variant.py").exists());
}

#[test]
fn workspace_tool_executor_copy_file_rejects_text_source_to_model_target() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), "def build(): pass\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["parts/new.py".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "copy_file",
            r#"{"source_path":"docs/source.md","target_path":"parts/new.py"}"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("parts/new.py").exists());
}

#[test]
fn workspace_tool_executor_cadquery_analyze_source_summarizes_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("components")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        concat!(
            "from components.pcb import build as pcb_build\n",
            "REFS = {\"type\":\"part\",\"features\":{\"top\":{\"selector\":\"top\"}}}\n",
            "def build(params=None):\n",
            "    return pcb_build(params)\n"
        ),
    )
    .unwrap();
    std::fs::write(dir.path().join("parts/lid.md"), "# Lid\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());

    let result = tool_json(
        &executor,
        &call(
            "cadquery_analyze_source",
            "{\"target_path\":\"parts/lid.py\",\"include_paired_doc\":true,\"include_dependencies\":true}",
        ),
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["target_type"], "part");
    assert_eq!(result["has_build_function"], true);
    assert_eq!(result["has_refs"], true);
    assert_eq!(result["paired_doc_path"], "parts/lid.md");
    assert_eq!(
        result["local_dependencies"],
        serde_json::json!(["components/pcb.py"])
    );
    assert_eq!(result["ref_keys"], serde_json::json!(["top"]));
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_cadquery_analyze_source_rejects_symlink_model() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("chats")).unwrap();
    std::fs::write(dir.path().join("chats/session.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("chats/session.jsonl"),
        dir.path().join("parts/lid.py"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());

    let result = tool_json(
        &executor,
        &call(
            "cadquery_analyze_source",
            "{\"target_path\":\"parts/lid.py\"}",
        ),
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
}

#[test]
fn workspace_tool_executor_cadquery_check_source_reports_contract() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_check_source",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"component\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "open('x', 'w')\\n\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["contract"]["has_build_function"], false);
    assert_eq!(result["contract"]["has_refs"], true);
    assert_eq!(result["contract"]["target_type_matches"], false);
    assert_eq!(
        result["contract"]["unsafe_calls"],
        serde_json::json!(["open"])
    );
}

#[test]
fn workspace_tool_executor_cadquery_dry_run_rejects_invalid_params_json() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("dry_cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_dry_run",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\",",
                "\"params_json\":\"{\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(runtime.dry_run_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_dry_run_uses_runtime_without_writing_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "old\n").unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("dry_cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_dry_run",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\",",
                "\"params_json\":\"{}\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["result_id"], "dry_cq_1");
    assert_eq!(result["summary"]["part_count"], 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("parts/lid.py")).unwrap(),
        "old\n"
    );
    assert_eq!(runtime.dry_run_requests().len(), 1);
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_unsafe_source() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        Vec::new(),
    ));

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None):\\n    open('x', 'w')\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_invalid_project_import() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        Vec::new(),
    ));

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"import docs as design_docs, chats.session\\n",
                "REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_allows_single_commit_and_get_result() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "old\n").unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let scope = AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\",",
                "\"export_formats\":[\"step\"],",
                "\"export_targets\":[\"outputs/lid.step\"]}"
            ),
        ),
        &context,
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["result_id"], "cq_1");
    assert_eq!(
        result["committed_files"],
        serde_json::json!(["parts/lid.py"])
    );
    assert_eq!(result["exports"], serde_json::json!(["outputs/lid.step"]));

    let second = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\",",
                "\"export_targets\":[\"outputs/lid.step\"]}"
            ),
        ),
        &context,
    );
    assert_eq!(second["status"], "error");
    assert_eq!(second["error_type"], "permission_denied");
    assert_eq!(runtime.execute_requests().len(), 1);

    let summary = tool_json(
        &executor,
        &call("cadquery_get_result", "{\"result_id\":\"cq_1\"}"),
    );
    assert_eq!(summary["status"], "ok");
    assert_eq!(summary["root_ref_text"], "@part[lid]");
    assert_eq!(summary["parts"][0]["features"], serde_json::json!(["top"]));
}

#[test]
fn workspace_tool_executor_cadquery_execute_requires_confirmed_scope() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_unmatched_export_target() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let scope = AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/other.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\",",
                "\"export_formats\":[\"step\"],",
                "\"export_targets\":[\"outputs/other.step\"]}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_requires_paired_doc_in_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.md"), "# Lid\n").unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        Vec::new(),
    ));

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(runtime.execute_requests().is_empty());
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_cadquery_execute_rejects_hard_linked_paired_doc() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("chats")).unwrap();
    std::fs::write(dir.path().join("chats/session.jsonl"), "{}\n").unwrap();
    std::fs::hard_link(
        dir.path().join("chats/session.jsonl"),
        dir.path().join("parts/lid.md"),
    )
    .unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into(), "parts/lid.md".into()],
        Vec::new(),
        Vec::new(),
    ));

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                "def build(params=None): pass\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_resolve_selection_rejects_selector_ref() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime);

    let result = tool_json(
        &executor,
        &call(
            "cadquery_resolve_selection",
            "{\"result_id\":\"cq_1\",\"selection_ref\":\"@selector[top]\"}",
        ),
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
}

#[test]
fn workspace_tool_executor_cadquery_resolve_selection_maps_feature() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime);

    let result = tool_json(
        &executor,
        &call(
            "cadquery_resolve_selection",
            "{\"result_id\":\"cq_1\",\"selection_ref\":\"@face[lid:f_0]\"}",
        ),
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_ref_text"], "@part[lid]");
    assert_eq!(result["candidate_feature_ref"], "@feature[lid.top]");
    assert_eq!(result["stable_ref"], "@feature[lid.top]");
    assert_eq!(result["ambiguous"], false);
}

#[test]
fn workspace_tool_executor_direct_call_denies_chat_summary_in_plan_mode() {
    let dir = tempfile::tempdir().unwrap();
    let store = ChatStore::new(dir.path().to_path_buf());
    let created = store
        .create("agent tools", Some("old goal".into()), Vec::new())
        .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);
    context.session_id = Some(created.session_id.clone());

    let result = tool_json_with_context(
        &executor,
        &call(
            "update_chat_summary",
            r#"{
                "summary":"bad",
                "goal":"bad",
                "related_files":[],
                "open_questions":[]
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        store
            .history(&created.session_id, None)
            .unwrap()
            .messages
            .len(),
        1
    );
}

#[test]
fn workspace_tool_executor_update_chat_summary_appends_chatstore_meta() {
    let dir = tempfile::tempdir().unwrap();
    let store = ChatStore::new(dir.path().to_path_buf());
    let created = store
        .create("agent tools", Some("old goal".into()), Vec::new())
        .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.session_id = Some(created.session_id.clone());

    let result = tool_json_with_context(
        &executor,
        &call(
            "update_chat_summary",
            r#"{
                "summary":"Discussed vent placement.",
                "goal":"Prepare a CadQuery execution plan.",
                "related_files":["parts/top_lid.py","plans/add-lid-vents.md"],
                "open_questions":["Confirm slot count"]
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["tool"], "update_chat_summary");
    assert_eq!(result["session_id"], created.session_id.0);
    assert_eq!(
        result["updated_fields"],
        serde_json::json!(["summary", "goal", "related_files", "open_questions"])
    );

    let history = store.history(&created.session_id, None).unwrap();
    let latest = history.messages.last().unwrap();
    assert_eq!(latest.role, app_server_protocol::ChatRole::Meta);
    assert!(latest.content.contains("\"type\":\"chat_summary\""));
    assert!(latest.content.contains("Discussed vent placement."));
    assert_eq!(latest.related_files[0].display_path(), "parts/top_lid.py");

    let sessions = store.list(false).unwrap();
    assert_eq!(
        sessions.sessions[0].related_files[0].display_path(),
        "parts/top_lid.py"
    );
}

#[test]
fn workspace_tool_executor_update_chat_summary_can_clear_related_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = ChatStore::new(dir.path().to_path_buf());
    let initial_related = test_path_handle(["parts", "top_lid.py"]);
    let created = store
        .create(
            "agent tools",
            Some("old goal".into()),
            vec![initial_related],
        )
        .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.session_id = Some(created.session_id.clone());

    let result = tool_json_with_context(
        &executor,
        &call(
            "update_chat_summary",
            r#"{
                "summary":"No active file scope.",
                "goal":"Continue discussion.",
                "related_files":[],
                "open_questions":[]
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    let sessions = store.list(false).unwrap();
    assert!(sessions.sessions[0].related_files.is_empty());
}

#[test]
fn workspace_tool_executor_update_chat_summary_rejects_arbitrary_chat_paths() {
    let dir = tempfile::tempdir().unwrap();
    let store = ChatStore::new(dir.path().to_path_buf());
    let created = store
        .create("agent tools", Some("old goal".into()), Vec::new())
        .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.session_id = Some(created.session_id.clone());

    let result = tool_json_with_context(
        &executor,
        &call(
            "update_chat_summary",
            r#"{
                "summary":"bad",
                "goal":"bad",
                "related_files":["chats/agent-tools.jsonl"],
                "open_questions":[]
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert_eq!(
        store
            .history(&created.session_id, None)
            .unwrap()
            .messages
            .len(),
        1
    );
}

#[test]
fn workspace_tool_executor_list_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.py"), "").unwrap();
    std::fs::create_dir(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "").unwrap();
    std::fs::write(dir.path().join("b.md"), "").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "list_directory",
            "{\"path\":\"\",\"recursive\":true,\"pattern\":\".py\",\"kind\":\"file\"}",
        ),
    );
    assert_eq!(result["status"], "ok");
    let entries = result["entries"].as_array().unwrap();
    let paths = entries
        .iter()
        .map(|entry| entry["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(paths, vec!["a.py", "parts/lid.py"]);
    assert_eq!(result["entry_count"], 2);
    assert_eq!(result["truncated"], false);
}

#[test]
fn workspace_tool_executor_list_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("list_directory", "{\"path\":\"\"}"));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["entries"].as_array().unwrap().len(), 0);
    assert_eq!(result["entry_count"], 0);
}

#[test]
fn workspace_tool_executor_list_directory_rejects_file_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.py"), "").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("list_directory", "{\"path\":\"a.py\"}"));
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(result["message"].as_str().unwrap().contains("directory"));
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_list_directory_rejects_symlink_child_to_denied_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("outputs")).unwrap();
    std::fs::write(dir.path().join("outputs/model.step"), "solid model").unwrap();
    std::os::unix::fs::symlink("../outputs/model.step", dir.path().join("parts/model.step"))
        .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("list_directory", "{\"path\":\"parts\"}"));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["entries"].as_array().unwrap().len(), 0);
}

#[test]
fn workspace_tool_executor_list_directory_filters_before_truncation() {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..505 {
        std::fs::write(dir.path().join(format!("file_{index:03}.txt")), "").unwrap();
    }
    std::fs::write(dir.path().join("target.py"), "").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "list_directory",
            "{\"path\":\"\",\"pattern\":\".py\",\"kind\":\"file\",\"max_entries\":500}",
        ),
    );
    assert_eq!(result["status"], "ok");
    let entries = result["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["path"], "target.py");
    assert_eq!(result["truncated"], false);
}

#[test]
fn workspace_tool_executor_list_directory_clamps_max_entries() {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..505 {
        std::fs::write(dir.path().join(format!("file_{index:03}.txt")), "").unwrap();
    }

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("list_directory", "{\"path\":\"\",\"max_entries\":1000}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["entry_count"], 500);
    assert_eq!(result["truncated"], true);
}

#[test]
fn workspace_tool_executor_unknown_tool() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let parsed = tool_json(&executor, &call("delete_everything", "{}"));
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error_type"], "unsupported_tool");
}

#[test]
fn workspace_tool_executor_invalid_json_args() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("read_file", "not json"));
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
}

#[test]
fn workspace_tool_executor_search_files_excludes_outputs_and_returns_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("outputs")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "def build():\n    return lid\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("parts/cache.py"), b"return\0cached").unwrap();
    std::fs::write(dir.path().join("outputs/lid.txt"), "return lid\n").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "search_files",
            "{\"query\":\"return\",\"path\":\"\",\"pattern\":\".py\",\"max_results\":10}",
        ),
    );
    assert_eq!(result["status"], "ok");
    let matches = result["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["path"], "parts/lid.py");
    assert_eq!(matches[0]["line_number"], 2);
}

#[test]
fn workspace_tool_executor_search_files_clamps_max_results() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    for index in 0..55 {
        std::fs::write(
            dir.path().join(format!("parts/file_{index:03}.py")),
            "def build():\n    return hit\n",
        )
        .unwrap();
    }

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "search_files",
            "{\"query\":\"return\",\"path\":\"parts\",\"max_results\":1000}",
        ),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["matches"].as_array().unwrap().len(), 50);
    assert_eq!(result["truncated"], true);
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_search_files_rejects_symlink_child_to_denied_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("outputs")).unwrap();
    std::fs::write(dir.path().join("outputs/model.py"), "return leaked\n").unwrap();
    std::os::unix::fs::symlink("../outputs/model.py", dir.path().join("parts/model.py")).unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "search_files",
            "{\"query\":\"return\",\"path\":\"parts\",\"max_results\":10}",
        ),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["matches"].as_array().unwrap().len(), 0);
}

#[test]
fn workspace_tool_executor_get_project_context_summarizes_cadquery_objects() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("plans")).unwrap();
    std::fs::create_dir_all(dir.path().join("chats")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "def build(): pass\n").unwrap();
    std::fs::write(dir.path().join("parts/lid.md"), "# lid\n").unwrap();
    std::fs::write(dir.path().join("plans/lid-plan.md"), "# plan\n").unwrap();
    std::fs::write(dir.path().join("chats/main.jsonl"), "{}\n").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("get_project_context", "{}"));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["objects"][0]["object_type"], "part");
    assert_eq!(result["objects"][0]["source_path"], "parts/lid.py");
    assert_eq!(result["objects"][0]["paired_doc_path"], "parts/lid.md");
    assert_eq!(result["plans"][0]["path"], "plans/lid-plan.md");
    assert_eq!(result["chats"][0]["path"], "chats/main.jsonl");
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_get_project_context_rejects_symlinked_project_root_to_denied_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("outputs")).unwrap();
    std::fs::write(dir.path().join("outputs/lid.py"), "def build(): pass\n").unwrap();
    std::os::unix::fs::symlink("outputs", dir.path().join("parts")).unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("get_project_context", "{}"));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["objects"].as_array().unwrap().len(), 0);
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_get_project_context_rejects_symlinked_paired_doc_to_denied_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("outputs")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "def build(): pass\n").unwrap();
    std::fs::write(dir.path().join("outputs/lid.md"), "# leaked\n").unwrap();
    std::os::unix::fs::symlink("../outputs/lid.md", dir.path().join("parts/lid.md")).unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("get_project_context", "{}"));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["objects"][0]["source_path"], "parts/lid.py");
    assert_eq!(
        result["objects"][0]["paired_doc_path"],
        serde_json::Value::Null
    );
    assert_eq!(result["objects"][0]["has_paired_doc"], false);
}

#[test]
fn workspace_tool_executor_get_selection_uses_tool_context_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = tool_context(AgentMode::Agent, None);
    context.active_selection_index = Some(0);
    context.context_refs = vec!["@part[lid]".into()];
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_1]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.top]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];

    let result: serde_json::Value =
        serde_json::from_str(&executor.execute(&call("get_selection", "{}"), &context)).unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["active_index"], 0);
    assert_eq!(result["context_refs"][0], "@part[lid]");
    assert_eq!(result["selections"][0]["ref_text"], "@face[lid:f_1]");
    assert_eq!(
        result["selections"][0]["candidate_feature_ref"],
        "@feature[lid.top]"
    );
}

#[test]
fn workspace_tool_executor_resolve_ref_maps_object_feature_and_raw_selection() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = {\"features\": {\"top\": {\"kind\": \"feature\"}}}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("parts/lid.md"), "# lid\n").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let object = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@part[lid]\"}"),
    );
    assert_eq!(object["status"], "ok");
    assert_eq!(object["owner_path"], "parts/lid.py");
    assert_eq!(object["owner_doc_path"], "parts/lid.md");
    assert_eq!(object["stable_ref"], "@part[lid]");

    let feature = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(feature["status"], "ok");
    assert_eq!(feature["owner_path"], "parts/lid.py");
    assert_eq!(feature["owner_doc_path"], "parts/lid.md");
    assert_eq!(feature["stable_ref"], "@feature[lid.top]");
    assert_eq!(feature["ambiguous"], false);

    let mut context = tool_context(AgentMode::Agent, None);
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_1]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.top]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];
    let raw: serde_json::Value = serde_json::from_str(&executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_1]\"}"),
        &context,
    ))
    .unwrap();
    assert_eq!(raw["status"], "ok");
    assert_eq!(raw["raw_ref_text"], "@face[lid:f_1]");
    assert_eq!(raw["owner_ref_text"], "@part[lid]");
    assert_eq!(raw["candidate_feature_ref"], "@feature[lid.top]");
}

#[test]
fn workspace_tool_executor_resolve_ref_prefers_object_mapping_over_selection_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = {\"features\": {}}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("parts/lid.md"), "# lid\n").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = tool_context(AgentMode::Agent, None);
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Part,
        ref_text: "@part[lid]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: None,
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];

    let result: serde_json::Value = serde_json::from_str(&executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@part[lid]\"}"),
        &context,
    ))
    .unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["stable_ref"], "@part[lid]");
    assert_eq!(result["owner_path"], "parts/lid.py");
    assert_eq!(result["owner_doc_path"], "parts/lid.md");
    assert_eq!(result["ambiguous"], false);
}

#[test]
fn workspace_tool_executor_resolve_ref_selection_requires_safe_owner_source() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = tool_context(AgentMode::Agent, None);
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_missing]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.top]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];

    let result: serde_json::Value = serde_json::from_str(&executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_missing]\"}"),
        &context,
    ))
    .unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], serde_json::Value::Null);
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
    assert!(!result["risks"].as_array().unwrap().is_empty());
}

#[test]
fn workspace_tool_executor_resolve_ref_reports_unstable_raw_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@edge[lid:e_1]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["raw_ref_text"], "@edge[lid:e_1]");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
    assert!(!result["risks"].as_array().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_resolve_ref_rejects_symlink_escape_owner_source() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        outside.path().join("lid.py"),
        "REFS = {\"features\": {\"top\": {}}}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("lid.py"),
        dir.path().join("parts/lid.py"),
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], serde_json::Value::Null);
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_resolve_ref_rejects_symlink_denied_root_owner_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::create_dir_all(dir.path().join("outputs")).unwrap();
    std::fs::write(
        dir.path().join("outputs/lid.py"),
        "REFS = {\"features\": {\"top\": {}}}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink("../outputs/lid.py", dir.path().join("parts/lid.py")).unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], serde_json::Value::Null);
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[test]
fn workspace_tool_executor_resolve_ref_requires_refs_feature_entry() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "# top appears in a comment only\nREFS = {\"features\": {\"side\": {}}}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], "parts/lid.py");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
    assert!(!result["risks"].as_array().unwrap().is_empty());
}

#[test]
fn workspace_tool_executor_resolve_ref_rejects_path_like_ref_names() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@part[../outputs/lid]\"}"),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");

    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[../.budn_staging/lid.top]\"}",
        ),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
}

#[test]
fn workspace_tool_executor_resolve_ref_ignores_refs_inside_string_literal() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "note = 'REFS = {\"features\": {\"top\": {}}}'\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], "parts/lid.py");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[test]
fn workspace_tool_executor_resolve_ref_ignores_refs_dict_inside_refs_string_assignment() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = '{\"features\": {\"top\": {}}}'\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], "parts/lid.py");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[test]
fn workspace_tool_executor_resolve_ref_ignores_refs_dict_inside_refs_assignment_comment() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = None  # {\"features\": {\"top\": {}}}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], "parts/lid.py");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[test]
fn workspace_tool_executor_resolve_ref_ignores_refs_dict_after_non_dict_refs_assignment() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = None\nOTHER = {\"features\": {\"top\": {}}}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], "parts/lid.py");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[test]
fn workspace_tool_executor_resolve_ref_ignores_refs_feature_inside_comment() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = {\n    # \"features\": {\"top\": {}}\n}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call("resolve_ref", "{\"ref_text\":\"@feature[lid.top]\"}"),
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["owner_path"], "parts/lid.py");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[test]
fn workspace_tool_executor_resolve_ref_selection_rejects_unsafe_candidate_feature() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = {\"features\": {\"top/../bad\": {}}}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = tool_context(AgentMode::Agent, None);
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_unsafe]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.top/../bad]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];

    let result: serde_json::Value = serde_json::from_str(&executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_unsafe]\"}"),
        &context,
    ))
    .unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
}

#[test]
fn workspace_tool_executor_resolve_ref_keeps_ambiguous_selection_unstable() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let mut context = tool_context(AgentMode::Agent, None);
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_2]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.top]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: true,
    }];

    let result: serde_json::Value = serde_json::from_str(&executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_2]\"}"),
        &context,
    ))
    .unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["raw_ref_text"], "@face[lid:f_2]");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
    assert!(!result["risks"].as_array().unwrap().is_empty());
}

struct MockProvider {
    responses: Mutex<Vec<LlmResponse>>,
    tool_names_seen: Mutex<Vec<Vec<String>>>,
}

struct FakeCadQueryRuntime {
    mesh: CadQueryMeshPayload,
    dry_runs: Mutex<Vec<CadQueryToolRunRequest>>,
    executes: Mutex<Vec<CadQueryToolRunRequest>>,
    results: Mutex<HashMap<String, CadQueryToolCachedResult>>,
}

impl FakeCadQueryRuntime {
    fn new(mesh: CadQueryMeshPayload) -> Self {
        let mut results = HashMap::new();
        results.insert(
            mesh.result_id.clone(),
            CadQueryToolCachedResult {
                mesh: mesh.clone(),
                exports: Vec::new(),
                warnings: Vec::new(),
            },
        );
        Self {
            mesh,
            dry_runs: Mutex::new(Vec::new()),
            executes: Mutex::new(Vec::new()),
            results: Mutex::new(results),
        }
    }

    fn dry_run_requests(&self) -> Vec<CadQueryToolRunRequest> {
        self.dry_runs.lock().unwrap().clone()
    }

    fn execute_requests(&self) -> Vec<CadQueryToolRunRequest> {
        self.executes.lock().unwrap().clone()
    }
}

impl CadQueryToolRuntime for FakeCadQueryRuntime {
    fn dry_run(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        self.dry_runs.lock().unwrap().push(request);
        Ok(CadQueryToolRunResult {
            mesh: self.mesh.clone(),
            committed_files: Vec::new(),
            exports: Vec::new(),
            warnings: Vec::new(),
        })
    }

    fn execute(
        &self,
        request: CadQueryToolRunRequest,
    ) -> Result<CadQueryToolRunResult, CadQueryToolRuntimeError> {
        let committed_files = vec![request.target_path.clone()];
        let exports = request.export_targets.clone();
        self.executes.lock().unwrap().push(request);
        Ok(CadQueryToolRunResult {
            mesh: self.mesh.clone(),
            committed_files,
            exports,
            warnings: Vec::new(),
        })
    }

    fn get_result(&self, result_id: &str) -> Option<CadQueryToolCachedResult> {
        self.results.lock().unwrap().get(result_id).cloned()
    }
}

fn sample_mesh(result_id: &str) -> CadQueryMeshPayload {
    CadQueryMeshPayload {
        result_id: result_id.into(),
        build_id: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        unit: PreviewUnit::Millimeter,
        root_ref_text: "@part[lid]".into(),
        root_object_kind: CadQueryObjectKind::Part,
        parts: vec![CadQueryPartMesh {
            name: "lid".into(),
            object_kind: CadQueryObjectKind::Part,
            ref_text: "@part[lid]".into(),
            instance_path: None,
            transform: None,
            faces: vec![FaceGroup {
                face_idx: 0,
                positions: vec![0.0, 0.0, 0.0],
                normals: vec![0.0, 0.0, 1.0],
                features: vec!["top".into()],
                ambiguous: false,
            }],
            edges: vec![EdgeGroup {
                edge_idx: 0,
                polyline: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                adjacent_faces: vec![0],
            }],
            vertices: vec![VertexPoint {
                vertex_idx: 0,
                position: [0.0, 0.0, 0.0],
                adjacent_edges: vec![0],
            }],
            feature_map: vec![CadQueryFeatureFaces {
                feature: "top".into(),
                face_indices: vec![0],
            }],
        }],
    }
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
        tool_context(AgentMode::Agent, None),
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
        tool_context(AgentMode::Agent, None),
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
        tool_context(AgentMode::Agent, None),
        &provider,
        &EchoExecutor,
        &NoopToolLoopObserver,
        &|_| true,
    );
    assert_eq!(result.unwrap().content, "done");
}

#[test]
fn registry_tool_loop_filters_tools_for_agent_mode() {
    let provider = MockProvider::new(vec![LlmResponse {
        content: "done".into(),
        tool_calls: Vec::new(),
    }]);
    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "agent")],
        tool_context(AgentMode::Agent, None),
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
    assert!(tools.iter().any(|name| name == "write_file"));
    assert!(tools.iter().any(|name| name == "cadquery_execute"));
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
        tool_context(AgentMode::Agent, None),
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
        tool_context(AgentMode::Agent, None),
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
fn registry_tool_loop_enforces_dotted_denied_path_roots_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_outputs".into(),
                function_name: "read_file".into(),
                arguments: "{\"path\":\"./outputs/model.step\"}".into(),
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
        tool_context(AgentMode::Agent, None),
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
        tool_context(AgentMode::Agent, None),
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
        tool_context(AgentMode::Agent, Some(scope)),
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
fn registry_tool_loop_requires_patch_target_in_affected_files_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_patch_new_file".into(),
                function_name: "patch_file".into(),
                arguments: concat!(
                    "{\"path\":\"docs/new.md\",",
                    "\"expected_hash\":\"sha256:abc\",",
                    "\"search\":\"old\",",
                    "\"replace\":\"new\"}"
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
    let scope = AgentToolConfirmationScope::new(Vec::new(), vec!["docs/new.md".into()], Vec::new());

    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "patch new file")],
        tool_context(AgentMode::Agent, Some(scope)),
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
            .contains("affected_files")
    );
}

#[test]
fn registry_tool_loop_requires_copy_target_in_new_files_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_copy_affected".into(),
                function_name: "copy_file".into(),
                arguments: "{\"source_path\":\"docs/source.md\",\"target_path\":\"docs/copy.md\"}"
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
    let scope =
        AgentToolConfirmationScope::new(vec!["docs/copy.md".into()], Vec::new(), Vec::new());

    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "copy into affected")],
        tool_context(AgentMode::Agent, Some(scope)),
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
    assert!(parsed["message"].as_str().unwrap().contains("new_files"));
}

#[test]
fn registry_tool_loop_rejects_text_source_to_model_copy_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_text_to_model".into(),
                function_name: "copy_file".into(),
                arguments: "{\"source_path\":\"docs/source.md\",\"target_path\":\"parts/new.py\"}"
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
    let scope =
        AgentToolConfirmationScope::new(Vec::new(), vec!["parts/new.py".into()], Vec::new());

    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "copy text to model")],
        tool_context(AgentMode::Agent, Some(scope)),
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
fn registry_tool_loop_rejects_write_file_new_file_with_expected_hash_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_new_with_hash".into(),
                function_name: "write_file".into(),
                arguments:
                    "{\"path\":\"docs/new.md\",\"contents\":\"x\",\"expected_hash\":\"sha256:abc\"}"
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
    let scope = AgentToolConfirmationScope::new(Vec::new(), vec!["docs/new.md".into()], Vec::new());

    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "create with hash")],
        tool_context(AgentMode::Agent, Some(scope)),
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
    assert!(parsed["message"].as_str().unwrap().contains("new_files"));
}

#[test]
fn registry_tool_loop_rejects_write_file_affected_without_hash_before_executing() {
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_affected_without_hash".into(),
                function_name: "write_file".into(),
                arguments: "{\"path\":\"docs/existing.md\",\"contents\":\"x\"}".into(),
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
        AgentToolConfirmationScope::new(vec!["docs/existing.md".into()], Vec::new(), Vec::new());

    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "overwrite without hash")],
        tool_context(AgentMode::Agent, Some(scope)),
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
            .contains("expected_hash")
    );
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
        tool_context(AgentMode::Agent, Some(scope)),
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
        tool_context(AgentMode::Agent, Some(scope)),
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
        tool_context(AgentMode::Agent, Some(scope)),
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
        vec![LlmMessage::new(
            "user",
            "execute with invalid export target",
        )],
        tool_context(AgentMode::Agent, Some(scope)),
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
fn registry_tool_loop_executes_scoped_cadquery_tool() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "old\n").unwrap();
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_cadquery_execute".into(),
                function_name: "cadquery_execute".into(),
                arguments: concat!(
                    "{\"target_path\":\"parts/lid.py\",",
                    "\"target_type\":\"part\",",
                    "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"top\\\":{}}}\\n",
                    "def build(params=None): pass\",",
                    "\"export_formats\":[\"step\"],",
                    "\"export_targets\":[\"outputs/lid.step\"]}"
                )
                .into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_loop_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let observer = RecordingObserver::default();
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.confirmation_scope = Some(AgentToolConfirmationScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    ));

    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "execute cadquery")],
        context,
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    assert_eq!(
        observer.starts.lock().unwrap().as_slice(),
        ["cadquery_execute"]
    );
    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result_id"], "cq_loop_1");
    assert_eq!(runtime.execute_requests().len(), 1);
    assert_eq!(
        runtime.execute_requests()[0].export_targets,
        vec!["outputs/lid.step"]
    );
}

#[test]
fn registry_tool_loop_allows_save_cad_plan_declared_export_targets() {
    let dir = tempfile::tempdir().unwrap();
    let provider = MockProvider::new(vec![
        LlmResponse {
            content: String::new(),
            tool_calls: vec![LlmToolCall {
                id: "call_save_plan".into(),
                function_name: "save_cad_plan".into(),
                arguments: concat!(
                    "{\"title\":\"Add lid vents\",",
                    "\"target_ref\":\"@part[top_lid]\",",
                    "\"resolved_target\":\"parts/top_lid.py\",",
                    "\"affected_files\":[\"parts/top_lid.py\"],",
                    "\"export_targets\":[\"outputs/top_lid.step\"],",
                    "\"strategy\":\"Cut three rounded vent slots.\",",
                    "\"execution_boundary\":\"Plan only.\"}"
                )
                .into(),
            }],
        },
        LlmResponse {
            content: "done".into(),
            tool_calls: Vec::new(),
        },
    ]);
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let observer = RecordingObserver::default();
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);
    context.run_id = Some("run-1".into());

    run_tool_loop_with_registry(
        vec![LlmMessage::new("user", "save plan")],
        context,
        &provider,
        &executor,
        &observer,
        &|_| true,
    )
    .unwrap();

    let result = observer.results.lock().unwrap().remove(0);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(
        parsed["export_targets"],
        serde_json::json!(["outputs/top_lid.step"])
    );
    assert!(dir.path().join("plans/add-lid-vents.md").is_file());
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
        tool_context(AgentMode::Agent, None),
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
        mode: AgentMode::Agent,
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
