use app_server_host::{
    execution_scope_from_plan_ref, export_handle_for, extract_object_name,
    extract_plan_from_json_block, extract_plan_from_selection, extract_plan_proposal,
    latest_saved_cad_plan, parse_plan_package, validate_saved_plan_confirmation,
};
use app_server_protocol::{
    AgentCadQueryConfirmation, CadQueryExecuteRequest, CadQueryExportFormat, CadQueryObjectKind,
    ChatMessageRecord, ChatRole, ChatToolResultRecord, PathHandle, SelectionKind, SelectionRef,
    SelectionUpdateRequest, WorkspaceId,
};

#[test]
fn json_block_extracts_plan_with_all_fields() {
    let response = r#"Here is the plan:
```json
{
  "target_path": "parts/lid.py",
  "target_type": "part",
  "description": "Add a vent hole",
  "affected_files": ["parts/lid.py", "parts/base.py"]
}
```
"#;
    let plan = extract_plan_from_json_block(response).expect("should parse");
    assert_eq!(plan.target_path, "parts/lid.py");
    assert_eq!(plan.target_type, CadQueryObjectKind::Part);
    assert_eq!(plan.description, "Add a vent hole");
    assert_eq!(plan.affected_paths, vec!["parts/lid.py", "parts/base.py"]);
}

#[test]
fn json_block_defaults_to_part_for_unknown_type() {
    let response = r#"```json
{"target_path": "parts/box.py", "description": "resize"}
```"#;
    let plan = extract_plan_from_json_block(response).expect("should parse");
    assert_eq!(plan.target_type, CadQueryObjectKind::Part);
}

#[test]
fn json_block_returns_none_for_invalid_json() {
    assert!(extract_plan_from_json_block("```json\nnot json\n```").is_none());
}

#[test]
fn json_block_returns_none_without_target_path() {
    let response = r#"```json
{"description": "missing target"}
```"#;
    assert!(extract_plan_from_json_block(response).is_none());
}

#[test]
fn json_block_returns_none_when_no_json_fence() {
    assert!(extract_plan_from_json_block("just some text").is_none());
}

#[test]
fn selection_extracts_plan_when_modify_intent_and_active_selection() {
    let selection = selection_with_face();
    let plan = extract_plan_from_selection(
        "I will modify the top surface to add ventilation",
        &selection,
    )
    .expect("should extract");
    assert_eq!(plan.target_path, "parts/top_lid.py");
    assert_eq!(plan.target_type, CadQueryObjectKind::Part);
}

#[test]
fn selection_returns_none_without_modify_intent() {
    let selection = selection_with_face();
    assert!(extract_plan_from_selection("this looks good", &selection).is_none());
}

#[test]
fn selection_returns_none_without_selections() {
    let empty = SelectionUpdateRequest {
        selections: Vec::new(),
        active_index: None,
    };
    assert!(extract_plan_from_selection("modify this", &empty).is_none());
}

#[test]
fn extract_plan_proposal_prefers_json_over_selection() {
    let response = r#"Plan:
```json
{"target_path": "parts/from_json.py", "description": "from json"}
```
I will modify the part."#;
    let selection = selection_with_face();
    let plan = extract_plan_proposal(response, &selection).expect("should extract");
    assert_eq!(plan.target_path, "parts/from_json.py");
}

#[test]
fn latest_saved_cad_plan_extracts_plan_ref_and_confirm_scope() {
    let messages = vec![
        tool_result_message(
            "old",
            r#"{"status":"ok","tool":"save_cad_plan","run_id":"run-old","plan_ref":"plans/old.md","target_path":"parts/old.py"}"#,
        ),
        tool_result_message(
            "new",
            r#"{
                "status":"ok",
                "tool":"save_cad_plan",
                "run_id":"run-2",
                "plan_id":"2026042900-add-lid-vents",
                "plan_ref":"plans/2026042900-add-lid-vents",
                "request_path":"plans/2026042900-add-lid-vents/request.md",
                "plan_path":"plans/2026042900-add-lid-vents/plan.md",
                "result_path":"plans/2026042900-add-lid-vents/plan-result.md",
                "target_ref":"@part[top_lid]",
                "target_path":"parts/top_lid.py",
                "target_type":"part",
                "affected_files":["parts/top_lid.py"],
                "new_files":[],
                "export_targets":["outputs/top_lid.step"],
                "summary":"Cut three rounded vent slots into the top face.",
                "plan_status":"planned"
            }"#,
        ),
    ];

    let saved = latest_saved_cad_plan(&messages, "run-2").expect("saved plan should parse");

    assert_eq!(saved.plan_ref, "plans/2026042900-add-lid-vents");
    assert_eq!(saved.target_path, "parts/top_lid.py");
    assert_eq!(saved.target_type, CadQueryObjectKind::Part);
    assert_eq!(saved.affected_paths, vec!["parts/top_lid.py"]);
    assert!(saved.new_paths.is_empty());
    assert_eq!(saved.export_targets, vec!["outputs/top_lid.step"]);
    assert_eq!(
        saved.description,
        "Cut three rounded vent slots into the top face."
    );
}

#[test]
fn parse_plan_package_extracts_execution_scope() {
    let dir = tempfile::tempdir().unwrap();
    write_plan_package(
        dir.path(),
        "2026042900-add-lid-vents",
        r#"---
plan_id: 2026042900-add-lid-vents
mode: plan
target_path: parts/top_lid.py
target_type: part
affected_files:
  - parts/top_lid.py
new_files: []
export_targets:
  - outputs/top_lid.step
status: planned
created_at: 2026-04-29T14:00:00+08:00
source_chat_session: chat-1
---

# CAD Plan: Add lid vents
"#,
    );

    let parsed = parse_plan_package(
        dir.path(),
        &path_handle(["plans", "2026042900-add-lid-vents"]),
    )
    .expect("plan package should parse");

    assert_eq!(parsed.plan_id, "2026042900-add-lid-vents");
    assert_eq!(parsed.plan_ref, "plans/2026042900-add-lid-vents");
    assert_eq!(parsed.target_path, "parts/top_lid.py");
    assert_eq!(parsed.target_type, CadQueryObjectKind::Part);
    assert_eq!(parsed.affected_files, vec!["parts/top_lid.py"]);
    assert!(parsed.new_files.is_empty());
    assert_eq!(parsed.export_targets, vec!["outputs/top_lid.step"]);
    assert_eq!(
        parsed.result_path,
        "plans/2026042900-add-lid-vents/plan-result.md"
    );
}

#[test]
fn execution_scope_from_plan_ref_uses_parsed_plan_package() {
    let dir = tempfile::tempdir().unwrap();
    write_plan_package(
        dir.path(),
        "2026042900-add-lid-vents",
        r#"---
plan_id: 2026042900-add-lid-vents
mode: plan
target_path: parts/top_lid.py
target_type: part
affected_files:
  - parts/top_lid.py
new_files:
  - docs/top_lid.md
export_targets:
  - outputs/top_lid.step
status: planned
created_at: 2026-04-29T14:00:00+08:00
source_chat_session: chat-1
---

# CAD Plan: Add lid vents
"#,
    );

    let scope = execution_scope_from_plan_ref(
        dir.path(),
        &path_handle(["plans", "2026042900-add-lid-vents"]),
    )
    .expect("plan package should become execution scope");

    assert_eq!(
        scope.plan_ref.as_deref(),
        Some("plans/2026042900-add-lid-vents")
    );
    assert_eq!(
        scope.plan_result_path.as_deref(),
        Some("plans/2026042900-add-lid-vents/plan-result.md")
    );
    assert_eq!(scope.target_path.as_deref(), Some("parts/top_lid.py"));
    assert_eq!(scope.target_type, Some(CadQueryObjectKind::Part));
    assert_eq!(scope.affected_files, vec!["parts/top_lid.py"]);
    assert_eq!(scope.new_files, vec!["docs/top_lid.md"]);
    assert_eq!(scope.export_targets, vec!["outputs/top_lid.step"]);
}

#[test]
fn parse_plan_package_returns_normalized_execution_scope() {
    let dir = tempfile::tempdir().unwrap();
    write_plan_package(
        dir.path(),
        "2026042900-normalized-scope",
        r#"---
plan_id: 2026042900-normalized-scope
mode: plan
target_path: ./parts//top_lid.py
target_type: part
affected_files:
  - parts/./top_lid.py
new_files: []
export_targets:
  - outputs//top_lid.step
status: planned
created_at: 2026-04-29T14:00:00+08:00
source_chat_session: chat-1
---

# CAD Plan: Normalized scope
"#,
    );

    let parsed = parse_plan_package(
        dir.path(),
        &path_handle(["plans", "2026042900-normalized-scope"]),
    )
    .expect("plan package should parse");

    assert_eq!(parsed.target_path, "parts/top_lid.py");
    assert_eq!(parsed.affected_files, vec!["parts/top_lid.py"]);
    assert_eq!(parsed.export_targets, vec!["outputs/top_lid.step"]);
}

#[test]
fn parse_plan_package_rejects_missing_required_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans/2026042900-missing-result")).unwrap();
    std::fs::write(
        dir.path()
            .join("plans/2026042900-missing-result/request.md"),
        "# Request\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("plans/2026042900-missing-result/plan.md"),
        "---\nplan_id: 2026042900-missing-result\n---\n",
    )
    .unwrap();

    let error = parse_plan_package(
        dir.path(),
        &path_handle(["plans", "2026042900-missing-result"]),
    )
    .unwrap_err();

    assert_eq!(
        error.code,
        app_server_protocol::ProtocolErrorCode::InvalidPathHandle
    );
    assert!(error.message.contains("plan-result.md"));
}

#[cfg(unix)]
#[test]
fn parse_plan_package_rejects_symlinked_plans_root() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write_plan_package(
        outside.path(),
        "2026042900-external-plan",
        r#"---
plan_id: 2026042900-external-plan
mode: plan
target_path: parts/top_lid.py
target_type: part
affected_files:
  - parts/top_lid.py
new_files: []
export_targets:
  - outputs/top_lid.step
status: planned
created_at: 2026-04-29T14:00:00+08:00
source_chat_session: chat-1
---

# CAD Plan: External plan
"#,
    );
    std::os::unix::fs::symlink(outside.path().join("plans"), dir.path().join("plans")).unwrap();

    let error = parse_plan_package(
        dir.path(),
        &path_handle(["plans", "2026042900-external-plan"]),
    )
    .unwrap_err();

    assert_eq!(
        error.code,
        app_server_protocol::ProtocolErrorCode::InvalidPathHandle
    );
    assert!(error.message.contains("symlink"));
}

#[test]
fn parse_plan_package_rejects_workspace_escape_and_bad_exports() {
    let dir = tempfile::tempdir().unwrap();
    write_plan_package(
        dir.path(),
        "2026042900-bad-scope",
        r#"---
plan_id: 2026042900-bad-scope
mode: plan
target_path: ../parts/top_lid.py
target_type: part
affected_files:
  - parts/top_lid.py
new_files: []
export_targets:
  - outputs/top_lid.obj
status: planned
created_at: 2026-04-29T14:00:00+08:00
source_chat_session: chat-1
---

# CAD Plan: Bad scope
"#,
    );

    let error = parse_plan_package(dir.path(), &path_handle(["plans", "2026042900-bad-scope"]))
        .unwrap_err();

    assert_eq!(
        error.code,
        app_server_protocol::ProtocolErrorCode::InvalidPathHandle
    );
    assert!(error.message.contains("target_path"));
}

#[test]
fn latest_saved_cad_plan_ignores_failed_or_wrong_run_results() {
    let messages = vec![
        tool_result_message(
            "wrong-run",
            r#"{"status":"ok","tool":"save_cad_plan","run_id":"run-1","plan_ref":"plans/old.md","target_path":"parts/old.py"}"#,
        ),
        tool_result_message(
            "failed",
            r#"{"status":"error","tool":"save_cad_plan","run_id":"run-2","plan_ref":"plans/bad.md","target_path":"parts/bad.py"}"#,
        ),
    ];

    assert!(latest_saved_cad_plan(&messages, "run-2").is_none());
}

#[test]
fn saved_plan_confirmation_requires_same_plan_ref_and_scope() {
    let plan = saved_plan();
    let confirmation = plan_confirmation(Some(path_handle(["plans", "2026042900-add-lid-vents"])));

    assert!(validate_saved_plan_confirmation(&confirmation, &plan).is_ok());
}

#[test]
fn saved_plan_confirmation_rejects_missing_or_mismatched_plan_ref() {
    let plan = saved_plan();
    let missing_ref = plan_confirmation(None);
    let wrong_ref = plan_confirmation(Some(path_handle(["plans", "2026042900-other"])));

    assert!(validate_saved_plan_confirmation(&missing_ref, &plan).is_err());
    assert!(validate_saved_plan_confirmation(&wrong_ref, &plan).is_err());
}

#[test]
fn saved_plan_confirmation_rejects_scope_mismatch() {
    let plan = saved_plan();
    let mut confirmation =
        plan_confirmation(Some(path_handle(["plans", "2026042900-add-lid-vents"])));
    confirmation.export_targets = vec![path_handle(["outputs", "other.step"])];

    let error = validate_saved_plan_confirmation(&confirmation, &plan).unwrap_err();
    assert!(error.contains("execution scope"));
}

#[test]
fn extract_object_name_from_part_ref() {
    assert_eq!(
        extract_object_name("@part[top_lid]"),
        Some("top_lid".into())
    );
}

#[test]
fn extract_object_name_from_component_ref() {
    assert_eq!(
        extract_object_name("@component[pcb_main]"),
        Some("pcb_main".into())
    );
}

#[test]
fn extract_object_name_returns_none_for_empty() {
    assert!(extract_object_name("@part[]").is_none());
    assert!(extract_object_name("no brackets").is_none());
}

#[test]
fn export_handle_replaces_extension_with_step() {
    let target = PathHandle::new(
        WorkspaceId::new("workspace"),
        vec!["parts".to_string(), "lid.py".to_string()],
    )
    .unwrap();
    let export = export_handle_for(&target);
    assert_eq!(export.path_segments(), &["outputs", "lid.step"]);
}

#[test]
fn export_handle_uses_model_for_unknown_extension() {
    let target = PathHandle::new(
        WorkspaceId::new("workspace"),
        vec!["outputs".to_string(), "model".to_string()],
    )
    .unwrap();
    let export = export_handle_for(&target);
    assert_eq!(export.path_segments(), &["outputs", "model.step"]);
}

fn tool_result_message(tool_call_id: &str, result_json: &str) -> ChatMessageRecord {
    ChatMessageRecord {
        message_id: format!("msg-{tool_call_id}"),
        ts_ms: 1,
        role: ChatRole::Tool,
        content: "agent tool completed".into(),
        related_files: Vec::new(),
        tool_call_id: Some(tool_call_id.into()),
        tool_calls: Vec::new(),
        tool_result: Some(ChatToolResultRecord {
            tool_call_id: tool_call_id.into(),
            tool_name: "save_cad_plan".into(),
            result_json: result_json.into(),
        }),
        mesh_result: None,
        run_id: None,
    }
}

fn saved_plan() -> app_server_host::plan_extraction::SavedCadPlan {
    app_server_host::plan_extraction::SavedCadPlan {
        plan_ref: "plans/2026042900-add-lid-vents".into(),
        target_path: "parts/top_lid.py".into(),
        target_type: CadQueryObjectKind::Part,
        affected_paths: vec!["parts/top_lid.py".into()],
        new_paths: Vec::new(),
        export_targets: vec!["outputs/top_lid.step".into()],
        description: "Add vents".into(),
    }
}

fn plan_confirmation(plan_ref: Option<PathHandle>) -> AgentCadQueryConfirmation {
    AgentCadQueryConfirmation {
        request: CadQueryExecuteRequest {
            target_path: path_handle(["parts", "top_lid.py"]),
            target_type: CadQueryObjectKind::Part,
            code: String::new(),
            export_formats: vec![CadQueryExportFormat::Step],
            params_json: "{}".into(),
        },
        plan_ref,
        affected_files: vec![path_handle(["parts", "top_lid.py"])],
        new_files: Vec::new(),
        export_targets: vec![path_handle(["outputs", "top_lid.step"])],
    }
}

fn write_plan_package(root: &std::path::Path, plan_id: &str, plan_markdown: &str) {
    let plan_dir = root.join("plans").join(plan_id);
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::write(plan_dir.join("request.md"), "# Request\n").unwrap();
    std::fs::write(plan_dir.join("plan.md"), plan_markdown).unwrap();
    std::fs::write(plan_dir.join("plan-result.md"), "status: pending\n").unwrap();
}

fn path_handle<const N: usize>(segments: [&str; N]) -> PathHandle {
    PathHandle::new(WorkspaceId::new("workspace"), segments).expect("path handle")
}

fn selection_with_face() -> SelectionUpdateRequest {
    SelectionUpdateRequest {
        selections: vec![SelectionRef {
            kind: SelectionKind::Face,
            ref_text: "@face[top_lid:f_0]".into(),
            owner_ref_text: Some("@part[top_lid]".into()),
            owner_object_kind: Some(CadQueryObjectKind::Part),
            instance_path: None,
            candidate_feature_ref: Some("@feature[top_lid.top_surface]".into()),
            build_id: Some("sha256:build".into()),
            result_id: Some("cq_1".into()),
            ambiguous: false,
        }],
        active_index: Some(0),
    }
}
