use app_server_core::{
    AgentExecutionScope, AgentToolCall, AgentToolRunContext, CadQueryModelContract,
    CadQueryToolCachedResult, CadQueryToolRunRequest, CadQueryToolRunResult, CadQueryToolRuntime,
    CadQueryToolRuntimeError, ChatStore, ToolExecutor, WorkspaceToolExecutor,
    agent_tool_definitions_for_mode,
};
use app_server_protocol::{
    AgentMode, CadQueryFeatureFaces, CadQueryMeshPayload, CadQueryObjectKind, CadQueryPartMesh,
    EdgeGroup, FaceGroup, PathHandle, PreviewUnit, SelectionKind, SelectionRef, VertexPoint,
    WorkspaceId,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime should build")
        .block_on(future)
}

fn tool_context(
    mode: AgentMode,
    execution_scope: Option<AgentExecutionScope>,
) -> AgentToolRunContext {
    let mut context = AgentToolRunContext::new(std::env::temp_dir(), mode);
    context.execution_scope = execution_scope;
    context
}

fn call(name: &str, arguments: &str) -> AgentToolCall {
    AgentToolCall {
        id: format!("call_{name}"),
        function_name: name.into(),
        arguments: arguments.into(),
    }
}

fn tool_json(executor: &WorkspaceToolExecutor, call: &AgentToolCall) -> serde_json::Value {
    let result = block_on(executor.execute(call, &tool_context(AgentMode::Agent, None)));
    serde_json::from_str(&result).expect("tool result should be json")
}

fn tool_json_with_context(
    executor: &WorkspaceToolExecutor,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
) -> serde_json::Value {
    let result = block_on(executor.execute(call, context));
    serde_json::from_str(&result).expect("tool result should be json")
}

fn valid_part_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Contract test model\"\n\
MODEL_DETAILS = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn triple_quoted_model_contract_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"\"\"Contract test model\"\"\"\n\
MODEL_DETAILS = {{\"purpose\":\"\"\"Verify model contract\"\"\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn annotated_model_contract_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION: str = \"Contract test model\"\n\
MODEL_DETAILS: dict[str, str] = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn structured_model_contract_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Structured contract test model\"\n\
MODEL_DETAILS = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":{{\"height\":\"8 mm\",\"width\":\"20 mm\"}},\"intended_use\":\"automated contract validation\",\"assumptions\":[\"no external dependencies\"],\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":[\"print flat\"]}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn parenthesized_model_contract_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = (\n\
    \"Contract test \"\n\
    \"model\"\n\
)\n\
MODEL_DETAILS = {{\n\
    \"purpose\": (\n\
        \"Verify \"\n\
        \"model contract\"\n\
    ),\n\
    \"key_dimensions\":\"unit dimensions\",\n\
    \"intended_use\": (\n\
        \"automated contract \"\n\
        \"validation\"\n\
    ),\n\
    \"assumptions\":\"no external dependencies\",\n\
    \"interaction_notes\":\"select named features\",\n\
    \"manufacturing_or_placement_constraints\": (\n\
        \"print \"\n\
        \"flat\"\n\
    ),\n\
}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn incomplete_model_details_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Incomplete contract test model\"\n\
MODEL_DETAILS = {{\n\
    # \"purpose\": \"Verify model contract\",\n\
    # \"key_dimensions\": \"unit dimensions\",\n\
    # \"intended_use\": \"automated contract validation\",\n\
    # \"assumptions\": \"no external dependencies\",\n\
    # \"interaction_notes\": \"select named features\",\n\
    # \"manufacturing_or_placement_constraints\": \"none\",\n\
}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn scoped_model_details_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Scoped contract test model\"\n\
def details():\n    MODEL_DETAILS = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn empty_model_details_value_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Empty value contract test model\"\n\
MODEL_DETAILS = {{\"purpose\":\"\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn collection_model_details_value_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Collection value contract test model\"\n\
MODEL_DETAILS = {{\"purpose\":{{}},\"key_dimensions\":[],\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn comment_only_collection_model_details_value_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Comment-only collection value contract test model\"\n\
MODEL_DETAILS = {{\n\
    \"purpose\": {{\n\
        # no usable purpose text\n\
    }},\n\
    \"key_dimensions\": [\n\
        # no usable dimension text\n\
    ],\n\
    \"intended_use\":\"automated contract validation\",\n\
    \"assumptions\":\"no external dependencies\",\n\
    \"interaction_notes\":\"select named features\",\n\
    \"manufacturing_or_placement_constraints\":\"none\",\n\
}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn empty_model_description_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"\"\n\
MODEL_DETAILS = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn string_literal_model_details_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"String literal contract test model\"\n\
NOTE = '''\n\
MODEL_DETAILS = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
'''\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn parenthesized_expression_model_contract_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = (\n\
    \"Expression contract test model\"\n\
) + dynamic_description()\n\
MODEL_DETAILS = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn tuple_model_description_source(feature: &str) -> String {
    format!(
        "MODEL_DESCRIPTION = \"Tuple contract test model\", dynamic_description()\n\
MODEL_DETAILS = {{\"purpose\":\"Verify model contract\",\"key_dimensions\":\"unit dimensions\",\"intended_use\":\"automated contract validation\",\"assumptions\":\"no external dependencies\",\"interaction_notes\":\"select named features\",\"manufacturing_or_placement_constraints\":\"none\"}}\n\
REFS = {{\"type\":\"part\",\"features\":{{\"{feature}\":{{}}}}}}\n\
def build(params=None): pass"
    )
}

fn test_path_handle(path: impl IntoIterator<Item = impl Into<String>>) -> PathHandle {
    PathHandle::new(WorkspaceId::new("ws"), path).expect("valid test path")
}

fn test_hash(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn save_plan_call_json(title: &str) -> String {
    format!(
        r#"{{
            "title":"{title}",
            "request":"Add three rounded ventilation slots to the selected top lid.",
            "target_ref":"@part[top_lid]",
            "target_path":"parts/top_lid.py",
            "target_type":"part",
            "affected_files":["parts/top_lid.py"],
            "new_files":[],
            "export_targets":["outputs/top_lid.step"],
            "strategy":"Cut three rounded vent slots into the top face.",
            "risks":["Maintain wall thickness"],
            "acceptance":["STEP export builds"],
            "execution_scope":"Only Agent mode CadQuery execution may modify parts/top_lid.py."
        }}"#
    )
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
                "request":"Add three rounded ventilation slots to the selected top lid.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/top_lid.py"],
                "new_files":[],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"Cut three rounded vent slots into the top face.",
                "risks":["Maintain wall thickness"],
                "acceptance":["STEP export builds"],
                "execution_scope":"Only Agent mode CadQuery execution may modify parts/top_lid.py."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["tool"], "save_cad_plan");
    assert!(
        result["plan_id"]
            .as_str()
            .unwrap()
            .ends_with("-add-lid-vents")
    );
    assert_eq!(result["target_path"], "parts/top_lid.py");
    assert_eq!(result["target_type"], "part");
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
    assert!(plan_ref.starts_with("plans/"));
    assert!(plan_ref.ends_with("-add-lid-vents"));
    assert_eq!(result["request_path"], format!("{plan_ref}/request.md"));
    assert_eq!(result["plan_path"], format!("{plan_ref}/plan.md"));
    assert_eq!(result["result_path"], format!("{plan_ref}/plan-result.md"));
    assert_eq!(result["plan_status"], "planned");

    let request =
        std::fs::read_to_string(dir.path().join(format!("{plan_ref}/request.md"))).unwrap();
    let plan = std::fs::read_to_string(dir.path().join(format!("{plan_ref}/plan.md"))).unwrap();
    let plan_result =
        std::fs::read_to_string(dir.path().join(format!("{plan_ref}/plan-result.md"))).unwrap();
    assert!(request.contains("Add three rounded ventilation slots"));
    assert!(plan.contains("plan_id:"));
    assert!(plan.contains("mode: plan"));
    assert!(plan.contains("target_path: parts/top_lid.py"));
    assert!(plan.contains("target_type: part"));
    assert!(plan.contains("status: planned"));
    assert!(plan.contains("# CAD Plan: Add lid vents"));
    assert!(plan.contains("outputs/top_lid.step"));
    assert!(plan.contains("Only Agent mode CadQuery execution may modify parts/top_lid.py."));
    assert!(plan_result.starts_with("status: pending"));
}

#[test]
fn workspace_tool_executor_save_cad_plan_allocates_next_daily_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let first = tool_json_with_context(
        &executor,
        &call("save_cad_plan", &save_plan_call_json("Add lid vents")),
        &context,
    );
    let second = tool_json_with_context(
        &executor,
        &call("save_cad_plan", &save_plan_call_json("Add lid vents")),
        &context,
    );

    assert_eq!(first["status"], "ok");
    assert_eq!(second["status"], "ok");
    let first_id = first["plan_id"].as_str().unwrap();
    let second_id = second["plan_id"].as_str().unwrap();
    assert!(first_id.ends_with("00-add-lid-vents"));
    assert!(second_id.ends_with("01-add-lid-vents"));
    assert!(
        dir.path()
            .join(second["plan_path"].as_str().unwrap())
            .is_file()
    );
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
                "request":"Unsafe plan request.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["../secret.py"],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"No write should happen.",
                "execution_scope":"Plan only."
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
                "request":"Missing export request.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/top_lid.py"],
                "strategy":"Cut three rounded vent slots.",
                "execution_scope":"Plan only."
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
fn workspace_tool_executor_save_cad_plan_requires_target_in_execution_scope() {
    let dir = tempfile::tempdir().unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "save_cad_plan",
            r#"{
                "title":"Wrong scope",
                "request":"Wrong scope request.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/base.py"],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"Cut three rounded vent slots.",
                "execution_scope":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(result["message"].as_str().unwrap().contains("target_path"));
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
                "request":"Unknown export request.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/top_lid.py"],
                "export_targets":["outputs/top_lid.obj"],
                "strategy":"Cut three rounded vent slots.",
                "execution_scope":"Plan only."
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
                "request":"Add lid vents request.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/top_lid.py"],
                "export_targets":["outputs/custom.step"],
                "strategy":"Cut three rounded vent slots.",
                "execution_scope":"Plan only."
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
                "request":"Add lid vents request.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/top_lid.py"],
                "export_targets":["outputs/top_lid.step"],
                "strategy":"Cut three rounded vent slots.",
                "execution_scope":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_ne!(result["plan_ref"], "plans/add-lid-vents.md");
    assert!(!outside.path().join("escaped.md").exists());
    assert!(
        dir.path()
            .join(result["plan_ref"].as_str().unwrap())
            .is_dir()
    );
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
                "request":"Add lid vents request.",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/top_lid.py"],
                "strategy":"Cut three rounded vent slots.",
                "execution_scope":"Plan only."
            }"#,
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(!dir.path().join("plans").exists());
}

#[test]
fn workspace_tool_executor_write_file_creates_scoped_text_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
fn workspace_tool_executor_write_file_agent_mode_creates_safe_text_without_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);

    let result = tool_json_with_context(
        &executor,
        &call(
            "write_file",
            r##"{"path":"docs/agent-note.md","contents":"# Agent note\n"}"##,
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["created"], true);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("docs/agent-note.md")).unwrap(),
        "# Agent note\n"
    );
}

#[test]
fn workspace_tool_executor_write_file_allows_empty_text_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/empty.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["plans/manual.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
fn workspace_tool_executor_patch_file_rejects_symlink_target_to_unscoped_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/real.md"), "alpha\nbeta\n").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("docs/real.md"),
        dir.path().join("docs/notes.md"),
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
fn workspace_tool_executor_write_file_rejects_existing_file_scoped_as_new_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "old\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(vec!["docs/notes.md".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
fn workspace_tool_executor_patch_file_rejects_existing_file_scoped_as_new_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/notes.md"), "alpha\nbeta\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/notes.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["parts/lid.py".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
fn workspace_tool_executor_copy_file_copies_scoped_text_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/source.md"), "source\n").unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["docs/copy.md".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
fn workspace_tool_executor_copy_file_allows_model_source_to_scoped_new_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "def build(params=None):\n    pass\n",
    )
    .unwrap();
    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let scope =
        AgentExecutionScope::new(Vec::new(), vec!["parts/lid_variant.py".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope =
        AgentExecutionScope::new(vec!["parts/lid_variant.py".into()], Vec::new(), Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
    let scope = AgentExecutionScope::new(Vec::new(), vec!["parts/new.py".into()], Vec::new());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

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
            "REFS = {\"type\":\"part\",\"features\":{\"lid_alignment_surface\":{\"selector\":\"top\"}}}\n",
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
    assert_eq!(
        result["ref_keys"],
        serde_json::json!(["lid_alignment_surface"])
    );
}

#[test]
fn workspace_tool_executor_cadquery_analyze_source_uses_runtime_warning_contract() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        parenthesized_model_contract_source("lid_alignment_surface"),
    )
    .unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")).with_model_contract(true));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime);
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_analyze_source",
            "{\"target_path\":\"parts/lid.py\"}",
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["has_model_description"], true);
    assert!(
        !result["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("MODEL_DESCRIPTION"))
    );
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
                "\"code\":\"REFS = {\\\"type\\\":\\\"component\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
fn workspace_tool_executor_cadquery_check_source_explains_refs_shape() {
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
                "\"code\":\"def build(params=None): pass\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["contract"]["has_refs"], false);
    assert!(
        result["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("REFS ="))
    );
    let warnings_text = result["warnings"].to_string();
    for forbidden in [
        "placement_pocket",
        "access_notch",
        "outer_shell",
        "mounting_boss",
        "alignment_slot",
        "semantic_part_feature_name",
        "semantic_component_feature_name",
        "semantic_assembly_feature_name",
    ] {
        assert!(
            !warnings_text.contains(forbidden),
            "CadQuery contract warning example should be domain-neutral: {forbidden}"
        );
    }
}

#[test]
fn workspace_tool_executor_cadquery_check_source_warns_about_missing_model_description() {
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
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
                "def build(params=None): pass\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "ok");
    assert_eq!(result["contract"]["has_model_description"], false);
    assert!(
        result["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("MODEL_DESCRIPTION"))
    );
    let warnings = result["warnings"].to_string();
    for required in ["purpose", "key_dimensions", "intended_use", "assumptions"] {
        assert!(
            warnings.contains(required),
            "CadQuery model detail warning should mention {required}"
        );
    }
}

#[test]
fn workspace_tool_executor_cadquery_check_source_uses_runtime_model_contract() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")).with_model_contract(true));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime);
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);
    let code = parenthesized_model_contract_source("lid_alignment_surface");
    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": code,
    })
    .to_string();

    let result = tool_json_with_context(&executor, &call("cadquery_check_source", &args), &context);

    assert_eq!(result["status"], "ok");
    assert_eq!(result["contract"]["has_model_description"], true);
}

#[test]
fn workspace_tool_executor_cadquery_execute_explains_missing_refs_shape() {
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
                "\"code\":\"REFS = {\\\"base\\\": \\\"body\\\"}\\n",
                "def build(params=None): pass\"}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("REFS.features")
    );
    assert!(result["message"].as_str().unwrap().contains("\"features\""));
    for forbidden in [
        "placement_pocket",
        "access_notch",
        "outer_shell",
        "mounting_boss",
        "alignment_slot",
        "semantic_part_feature_name",
        "semantic_component_feature_name",
        "semantic_assembly_feature_name",
    ] {
        assert!(
            !result["message"].as_str().unwrap().contains(forbidden),
            "CadQuery execute error example should be domain-neutral: {forbidden}"
        );
    }
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_missing_model_details() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let scope = AgentExecutionScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_grip_surface\\\":{}}}\\n",
                "def build(params=None): pass\",",
                "\"export_formats\":[\"step\"],",
                "\"export_targets\":[\"outputs/lid.step\"]}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("MODEL_DESCRIPTION")
    );
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_incomplete_model_details() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let scope = AgentExecutionScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);
    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": incomplete_model_details_source("lid_grip_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();

    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("MODEL_DETAILS")
    );
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_non_module_or_empty_model_details() {
    let dir = tempfile::tempdir().unwrap();
    let runtime =
        Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")).with_model_contract(false));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let scope = AgentExecutionScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

    for (case, code) in [
        ("scoped", scoped_model_details_source("lid_grip_surface")),
        (
            "empty",
            empty_model_details_value_source("lid_grip_surface"),
        ),
        (
            "collection",
            collection_model_details_value_source("lid_grip_surface"),
        ),
        (
            "comment_only_collection",
            comment_only_collection_model_details_value_source("lid_grip_surface"),
        ),
        (
            "empty_description",
            empty_model_description_source("lid_grip_surface"),
        ),
        (
            "string_literal",
            string_literal_model_details_source("lid_grip_surface"),
        ),
        (
            "parenthesized_expression",
            parenthesized_expression_model_contract_source("lid_grip_surface"),
        ),
        (
            "tuple_description",
            tuple_model_description_source("lid_grip_surface"),
        ),
    ] {
        let args = serde_json::json!({
            "target_path": "parts/lid.py",
            "target_type": "part",
            "code": code,
            "export_formats": ["step"],
            "export_targets": ["outputs/lid.step"],
        })
        .to_string();

        let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

        assert_eq!(result["status"], "error", "{case}");
        assert_eq!(result["error_type"], "invalid_arguments", "{case}");
        assert!(
            result["message"]
                .as_str()
                .unwrap()
                .contains("MODEL_DETAILS")
        );
    }
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_checks_model_contract_before_paired_doc_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(dir.path().join("parts/lid.py"), "old\n").unwrap();
    std::fs::write(dir.path().join("parts/lid.md"), "# Lid\n").unwrap();
    let runtime =
        Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")).with_model_contract(false));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let scope = AgentExecutionScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);
    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_grip_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();

    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "invalid_arguments");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("MODEL_DESCRIPTION")
    );
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_requires_step_export_target() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            &serde_json::json!({
                "target_path": "parts/lid.py",
                "target_type": "part",
                "code": valid_part_source("lid_grip_surface"),
            })
            .to_string(),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("export_formats")
    );
    assert!(
        result["message"]
            .as_str()
            .unwrap()
            .contains("export_targets")
    );
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_non_step_only_exports() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["stl"],
        "export_targets": ["outputs/lid.stl"],
    })
    .to_string();

    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    assert!(result["message"].as_str().unwrap().contains("step"));
    assert!(runtime.execute_requests().is_empty());
}

#[test]
fn workspace_tool_executor_cadquery_execute_accepts_python_model_contract_variants() {
    for (case, code) in [
        (
            "triple_quoted",
            triple_quoted_model_contract_source("lid_alignment_surface"),
        ),
        (
            "annotated",
            annotated_model_contract_source("lid_alignment_surface"),
        ),
        (
            "structured",
            structured_model_contract_source("lid_alignment_surface"),
        ),
        (
            "parenthesized",
            parenthesized_model_contract_source("lid_alignment_surface"),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("parts")).unwrap();
        std::fs::write(dir.path().join("parts/lid.py"), "old\n").unwrap();
        let runtime =
            Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")).with_model_contract(true));
        let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf())
            .with_cadquery_runtime(runtime.clone());
        let scope = AgentExecutionScope::new(
            vec!["parts/lid.py".into()],
            Vec::new(),
            vec!["outputs/lid.step".into()],
        );
        let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
        context.execution_scope = Some(scope);
        let args = serde_json::json!({
            "target_path": "parts/lid.py",
            "target_type": "part",
            "code": code,
            "export_formats": ["step"],
            "export_targets": ["outputs/lid.step"],
        })
        .to_string();

        let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

        assert_eq!(result["status"], "ok", "{case}: {result}");
        assert_eq!(runtime.execute_requests().len(), 1, "{case}");
    }
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
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
    context.execution_scope = Some(AgentExecutionScope::new(
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
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
    context.execution_scope = Some(AgentExecutionScope::new(
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
                "REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
    let scope = AgentExecutionScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();
    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);
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
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
    assert_eq!(
        summary["parts"][0]["features"],
        serde_json::json!(["lid_alignment_surface"])
    );
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_plan_mode() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Plan);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
fn workspace_tool_executor_cadquery_execute_allows_agent_mode_without_plan_scope() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);

    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();
    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

    assert_eq!(result["status"], "ok");
    assert_eq!(result["result_id"], "cq_1");
    assert_eq!(runtime.execute_requests().len(), 1);
}

#[test]
fn workspace_tool_executor_cadquery_execute_updates_plan_result_for_plan_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans/2026042900-lid")).unwrap();
    std::fs::write(
        dir.path().join("plans/2026042900-lid/plan-result.md"),
        "---\nstatus: pending\n---\n",
    )
    .unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.run_id = Some("run-plan-1".into());
    context.execution_scope = Some(AgentExecutionScope::for_plan(
        "plans/2026042900-lid",
        "plans/2026042900-lid/plan-result.md",
        "parts/lid.py",
        CadQueryObjectKind::Part,
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    ));

    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();
    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

    assert_eq!(result["status"], "ok");
    assert_eq!(
        result["plan_result_path"],
        serde_json::json!("plans/2026042900-lid/plan-result.md")
    );
    let plan_result =
        std::fs::read_to_string(dir.path().join("plans/2026042900-lid/plan-result.md")).unwrap();
    assert!(plan_result.contains("status: succeeded"));
    assert!(plan_result.contains("run_id: `run-plan-1`"));
    assert!(plan_result.contains("result_id: `cq_1`"));
    assert!(plan_result.contains("outputs/lid.step"));
}

#[test]
fn workspace_tool_executor_cadquery_execute_records_plan_result_for_scope_failure() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans/2026042900-lid")).unwrap();
    std::fs::write(
        dir.path().join("plans/2026042900-lid/plan-result.md"),
        "---\nstatus: pending\n---\n",
    )
    .unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.run_id = Some("run-plan-1".into());
    context.execution_scope = Some(AgentExecutionScope::for_plan(
        "plans/2026042900-lid",
        "plans/2026042900-lid/plan-result.md",
        "parts/lid.py",
        CadQueryObjectKind::Part,
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    ));

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/other.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
                "def build(params=None): pass\",",
                "\"export_formats\":[\"step\"],",
                "\"export_targets\":[\"outputs/other.step\"]}"
            ),
        ),
        &context,
    );

    assert_eq!(result["status"], "error");
    assert_eq!(result["error_type"], "permission_denied");
    let plan_result =
        std::fs::read_to_string(dir.path().join("plans/2026042900-lid/plan-result.md")).unwrap();
    assert!(plan_result.contains("status: failed"));
    assert!(plan_result.contains("run_id: `run-plan-1`"));
    assert!(plan_result.contains("outside execution"));
    assert!(runtime.execute_requests().is_empty());
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_cadquery_execute_does_not_update_plan_result_through_symlink_parent() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join("2026042900-lid")).unwrap();
    std::fs::write(
        outside.path().join("2026042900-lid/plan-result.md"),
        "---\nstatus: outside\n---\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("plans")).unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime);
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.run_id = Some("run-plan-1".into());
    context.execution_scope = Some(AgentExecutionScope::for_plan(
        "plans/2026042900-lid",
        "plans/2026042900-lid/plan-result.md",
        "parts/lid.py",
        CadQueryObjectKind::Part,
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    ));

    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();
    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

    assert_eq!(result["status"], "ok");
    assert!(result["plan_result_update_warning"].is_string());
    assert_eq!(
        std::fs::read_to_string(outside.path().join("2026042900-lid/plan-result.md")).unwrap(),
        "---\nstatus: outside\n---\n"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn workspace_tool_executor_cadquery_execute_does_not_update_hard_linked_plan_result() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_plan_result = outside.path().join("plan-result.md");
    std::fs::write(&outside_plan_result, "---\nstatus: outside\n---\n").unwrap();
    std::fs::create_dir_all(dir.path().join("plans/2026042900-lid")).unwrap();
    std::fs::hard_link(
        &outside_plan_result,
        dir.path().join("plans/2026042900-lid/plan-result.md"),
    )
    .unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime);
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.run_id = Some("run-plan-1".into());
    context.execution_scope = Some(AgentExecutionScope::for_plan(
        "plans/2026042900-lid",
        "plans/2026042900-lid/plan-result.md",
        "parts/lid.py",
        CadQueryObjectKind::Part,
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    ));

    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();
    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

    assert_eq!(result["status"], "ok");
    assert!(result["plan_result_update_warning"].is_string());
    assert_eq!(
        std::fs::read_to_string(outside_plan_result).unwrap(),
        "---\nstatus: outside\n---\n"
    );
}

#[test]
fn workspace_tool_executor_cadquery_execute_rejects_unmatched_export_target() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(FakeCadQueryRuntime::new(sample_mesh("cq_1")));
    let executor =
        WorkspaceToolExecutor::new(dir.path().to_path_buf()).with_cadquery_runtime(runtime.clone());
    let scope = AgentExecutionScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/other.step".into()],
    );
    let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
    context.execution_scope = Some(scope);

    let result = tool_json_with_context(
        &executor,
        &call(
            "cadquery_execute",
            concat!(
                "{\"target_path\":\"parts/lid.py\",",
                "\"target_type\":\"part\",",
                "\"code\":\"REFS = {\\\"type\\\":\\\"part\\\",\\\"features\\\":{\\\"lid_alignment_surface\\\":{}}}\\n",
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
    context.execution_scope = Some(AgentExecutionScope::new(
        vec!["parts/lid.py".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    ));

    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();
    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

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
    context.execution_scope = Some(AgentExecutionScope::new(
        vec!["parts/lid.py".into(), "parts/lid.md".into()],
        Vec::new(),
        vec!["outputs/lid.step".into()],
    ));

    let args = serde_json::json!({
        "target_path": "parts/lid.py",
        "target_type": "part",
        "code": valid_part_source("lid_alignment_surface"),
        "export_formats": ["step"],
        "export_targets": ["outputs/lid.step"],
    })
    .to_string();
    let result = tool_json_with_context(&executor, &call("cadquery_execute", &args), &context);

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
    assert_eq!(
        result["candidate_feature_ref"],
        "@feature[lid.lid_alignment_surface]"
    );
    assert_eq!(result["stable_ref"], "@feature[lid.lid_alignment_surface]");
    assert_eq!(result["ambiguous"], false);
}

#[test]
fn workspace_tool_executor_direct_call_denies_chat_summary_in_plan_mode() {
    let dir = tempfile::tempdir().unwrap();
    let store = ChatStore::new(dir.path().to_path_buf());
    let created =
        block_on(store.create("agent tools", Some("old goal".into()), Vec::new())).unwrap();
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
        block_on(store.history(&created.session_id, None))
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
    let created =
        block_on(store.create("agent tools", Some("old goal".into()), Vec::new())).unwrap();
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
                "related_files":["parts/top_lid.py","outputs/top_lid.step","plans/add-lid-vents.md"],
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

    let history = block_on(store.history(&created.session_id, None)).unwrap();
    let latest = history.messages.last().unwrap();
    assert_eq!(latest.role, app_server_protocol::ChatRole::Meta);
    assert!(latest.content.contains("\"type\":\"chat_summary\""));
    assert!(latest.content.contains("Discussed vent placement."));
    assert!(latest.tool_calls.is_empty());
    assert!(latest.tool_call_id.is_none());
    assert!(latest.tool_result.is_none());
    assert!(latest.mesh_result.is_none());
    assert_eq!(latest.related_files[0].display_path(), "parts/top_lid.py");
    assert_eq!(
        latest.related_files[1].display_path(),
        "outputs/top_lid.step"
    );

    let sessions = block_on(store.list(false)).unwrap();
    assert_eq!(
        sessions.sessions[0].related_files[0].display_path(),
        "parts/top_lid.py"
    );
    assert_eq!(
        sessions.sessions[0].related_files[1].display_path(),
        "outputs/top_lid.step"
    );
}

#[test]
fn workspace_tool_executor_update_chat_summary_can_clear_related_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = ChatStore::new(dir.path().to_path_buf());
    let initial_related = test_path_handle(["parts", "top_lid.py"]);
    let created = block_on(store.create(
        "agent tools",
        Some("old goal".into()),
        vec![initial_related],
    ))
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
    let sessions = block_on(store.list(false)).unwrap();
    assert!(sessions.sessions[0].related_files.is_empty());
}

#[test]
fn workspace_tool_executor_update_chat_summary_rejects_denied_or_unknown_roots() {
    for related_file in [
        "chats/agent-tools.jsonl",
        ".git/config",
        "target/debug/out.step",
        "node_modules/pkg/index.js",
        ".budn_staging/result.step",
        "tmp/result.step",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = ChatStore::new(dir.path().to_path_buf());
        let created =
            block_on(store.create("agent tools", Some("old goal".into()), Vec::new())).unwrap();
        let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
        let mut context = AgentToolRunContext::new(dir.path().to_path_buf(), AgentMode::Agent);
        context.session_id = Some(created.session_id.clone());
        let args = serde_json::json!({
            "summary": "bad",
            "goal": "bad",
            "related_files": [related_file],
            "open_questions": []
        })
        .to_string();

        let result =
            tool_json_with_context(&executor, &call("update_chat_summary", &args), &context);

        assert_eq!(result["status"], "error", "{related_file}");
        assert_eq!(result["error_type"], "permission_denied", "{related_file}");
        assert_eq!(
            block_on(store.history(&created.session_id, None))
                .unwrap()
                .messages
                .len(),
            1,
            "{related_file}"
        );
    }
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
    std::fs::create_dir_all(dir.path().join("plans/2026042900-add-lid-vents")).unwrap();
    std::fs::write(
        dir.path().join("plans/2026042900-add-lid-vents/request.md"),
        "# Request\n\nAdd lid vents.\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("plans/2026042900-add-lid-vents/plan.md"),
        r#"---
plan_id: 2026042900-add-lid-vents
mode: plan
target_path: parts/lid.py
target_type: part
affected_files:
  - parts/lid.py
new_files: []
export_targets:
  - outputs/lid.step
status: planned
created_at: 2026-04-29T14:00:00+08:00
source_chat_session: chat-1
---

# CAD Plan: Add lid vents
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path()
            .join("plans/2026042900-add-lid-vents/plan-result.md"),
        "status: pending\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("chats/main.jsonl"), "{}\n").unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("get_project_context", "{}"));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["objects"][0]["object_type"], "part");
    assert_eq!(result["objects"][0]["source_path"], "parts/lid.py");
    assert_eq!(result["objects"][0]["paired_doc_path"], "parts/lid.md");
    let plans = result["plans"].as_array().unwrap();
    let package = plans
        .iter()
        .find(|plan| plan["kind"] == "plan_package")
        .unwrap();
    assert_eq!(package["plan_id"], "2026042900-add-lid-vents");
    assert_eq!(package["plan_ref"], "plans/2026042900-add-lid-vents");
    assert_eq!(package["title"], "Add lid vents");
    assert_eq!(package["status"], "planned");
    assert_eq!(package["target_path"], "parts/lid.py");
    assert_eq!(package["target_type"], "part");
    assert!(package["updated_ms"].is_number());
    assert_eq!(
        package["result_path"],
        "plans/2026042900-add-lid-vents/plan-result.md"
    );
    let legacy = plans
        .iter()
        .find(|plan| plan["kind"] == "legacy_plan")
        .unwrap();
    assert_eq!(legacy["path"], "plans/lid-plan.md");
    assert!(legacy["updated_ms"].is_number());
    assert_eq!(result["chats"][0]["path"], "chats/main.jsonl");
}

#[cfg(unix)]
#[test]
fn workspace_tool_executor_get_project_context_does_not_follow_plans_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join("plans/2026042900-external")).unwrap();
    std::fs::write(outside.path().join("plans/legacy.md"), "# leaked\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("plans"), dir.path().join("plans")).unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(&executor, &call("get_project_context", "{}"));

    assert_eq!(result["status"], "ok");
    assert!(result["plans"].as_array().unwrap().is_empty());
    assert!(
        result["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("symlink"))
    );
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
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_1]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.lid_alignment_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];

    let tool_result = block_on(executor.execute(&call("get_selection", "{}"), &context));
    let result: serde_json::Value = serde_json::from_str(&tool_result).unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["active_index"], 0);
    assert_eq!(result["selections"][0]["ref_text"], "@face[lid:f_1]");
    assert_eq!(
        result["selections"][0]["candidate_feature_ref"],
        "@feature[lid.lid_alignment_surface]"
    );
}

#[test]
fn workspace_tool_executor_resolve_ref_maps_object_feature_and_raw_selection() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parts")).unwrap();
    std::fs::write(
        dir.path().join("parts/lid.py"),
        "REFS = {\"features\": {\"lid_alignment_surface\": {\"kind\": \"feature\"}}}\n",
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
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
    );
    assert_eq!(feature["status"], "ok");
    assert_eq!(feature["owner_path"], "parts/lid.py");
    assert_eq!(feature["owner_doc_path"], "parts/lid.md");
    assert_eq!(feature["stable_ref"], "@feature[lid.lid_alignment_surface]");
    assert_eq!(feature["ambiguous"], false);

    let mut context = tool_context(AgentMode::Agent, None);
    context.selections = vec![SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_1]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.lid_alignment_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];
    let tool_result = block_on(executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_1]\"}"),
        &context,
    ));
    let raw: serde_json::Value = serde_json::from_str(&tool_result).unwrap();
    assert_eq!(raw["status"], "ok");
    assert_eq!(raw["raw_ref_text"], "@face[lid:f_1]");
    assert_eq!(raw["owner_ref_text"], "@part[lid]");
    assert_eq!(
        raw["candidate_feature_ref"],
        "@feature[lid.lid_alignment_surface]"
    );
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

    let tool_result = block_on(executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@part[lid]\"}"),
        &context,
    ));
    let result: serde_json::Value = serde_json::from_str(&tool_result).unwrap();
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
        candidate_feature_ref: Some("@feature[lid.lid_alignment_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];

    let tool_result = block_on(executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_missing]\"}"),
        &context,
    ));
    let result: serde_json::Value = serde_json::from_str(&tool_result).unwrap();
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
        "REFS = {\"features\": {\"lid_alignment_surface\": {}}}\n",
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
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
        "REFS = {\"features\": {\"lid_alignment_surface\": {}}}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink("../outputs/lid.py", dir.path().join("parts/lid.py")).unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
        "# lid_alignment_surface appears in a comment only\nREFS = {\"features\": {\"side\": {}}}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
            "{\"ref_text\":\"@feature[../.budn_staging/lid.lid_alignment_surface]\"}",
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
        "note = 'REFS = {\"features\": {\"lid_alignment_surface\": {}}}'\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
        "REFS = '{\"features\": {\"lid_alignment_surface\": {}}}'\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
        "REFS = None  # {\"features\": {\"lid_alignment_surface\": {}}}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
        "REFS = None\nOTHER = {\"features\": {\"lid_alignment_surface\": {}}}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
        "REFS = {\n    # \"features\": {\"lid_alignment_surface\": {}}\n}\n",
    )
    .unwrap();

    let executor = WorkspaceToolExecutor::new(dir.path().to_path_buf());
    let result = tool_json(
        &executor,
        &call(
            "resolve_ref",
            "{\"ref_text\":\"@feature[lid.lid_alignment_surface]\"}",
        ),
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
        "REFS = {\"features\": {\"lid_alignment_surface/../bad\": {}}}\n",
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
        candidate_feature_ref: Some("@feature[lid.lid_alignment_surface/../bad]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }];

    let tool_result = block_on(executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_unsafe]\"}"),
        &context,
    ));
    let result: serde_json::Value = serde_json::from_str(&tool_result).unwrap();
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
        candidate_feature_ref: Some("@feature[lid.lid_alignment_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: true,
    }];

    let tool_result = block_on(executor.execute(
        &call("resolve_ref", "{\"ref_text\":\"@face[lid:f_2]\"}"),
        &context,
    ));
    let result: serde_json::Value = serde_json::from_str(&tool_result).unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["raw_ref_text"], "@face[lid:f_2]");
    assert_eq!(result["stable_ref"], serde_json::Value::Null);
    assert_eq!(result["ambiguous"], true);
    assert!(!result["risks"].as_array().unwrap().is_empty());
}

struct FakeCadQueryRuntime {
    mesh: CadQueryMeshPayload,
    dry_runs: Mutex<Vec<CadQueryToolRunRequest>>,
    executes: Mutex<Vec<CadQueryToolRunRequest>>,
    results: Mutex<HashMap<String, CadQueryToolCachedResult>>,
    model_contract: Option<bool>,
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
            model_contract: None,
        }
    }

    fn with_model_contract(mut self, has_model_description: bool) -> Self {
        self.model_contract = Some(has_model_description);
        self
    }

    fn dry_run_requests(&self) -> Vec<CadQueryToolRunRequest> {
        self.dry_runs.lock().unwrap().clone()
    }

    fn execute_requests(&self) -> Vec<CadQueryToolRunRequest> {
        self.executes.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl CadQueryToolRuntime for FakeCadQueryRuntime {
    async fn model_contract(
        &self,
        _request: &CadQueryToolRunRequest,
    ) -> Option<Result<CadQueryModelContract, CadQueryToolRuntimeError>> {
        self.model_contract.map(|has_model_description| {
            Ok(CadQueryModelContract {
                has_model_description,
            })
        })
    }

    async fn dry_run(
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

    async fn execute(
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
        artifact_relation: None,
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
                features: vec!["lid_alignment_surface".into()],
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
                feature: "lid_alignment_surface".into(),
                face_indices: vec![0],
            }],
        }],
    }
}
