use app_server_core::{
    AgentSemanticStore, AgentToolSpec, CadQueryModelFilePolicy, OutputPathPolicy,
    agent_tool_definitions_for_mode, agent_tool_permission, agent_tool_specs,
};
use app_server_protocol::AgentMode;

#[test]
fn registry_tool_set_matches_mvp_contract() {
    let specs = agent_tool_specs();
    let mut actual = specs
        .iter()
        .map(|spec| spec.definition.name.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected_tool_modes()
        .iter()
        .map(|(tool, _)| *tool)
        .collect::<Vec<_>>();
    expected.sort_unstable();
    assert_eq!(actual, expected);

    for spec in agent_tool_specs() {
        assert_eq!(spec.definition.parameters["type"], "object");
        assert!(spec.definition.parameters["properties"].is_object());
        assert!(spec.definition.parameters["required"].is_array());
        assert_eq!(spec.success_schema["type"], "object");
        assert_eq!(spec.error_schema["type"], "object");
        assert_eq!(spec.error_schema["properties"]["status"]["const"], "error");
    }
}

#[test]
fn registry_definitions_are_filtered_by_mode_contract() {
    for (tool, modes) in expected_tool_modes() {
        let spec = spec_by_name(tool);
        assert_eq!(spec.allowed_modes, modes, "mode set for {tool}");
        for mode in all_modes() {
            let names = tool_names(mode);
            assert_eq!(
                names.iter().any(|name| name == tool),
                modes.contains(&mode),
                "tool definition visibility for {tool:?} in {mode:?}"
            );
        }
    }
}

#[test]
fn registry_permission_reports_confirmation_requirements() {
    for (tool, modes) in expected_tool_modes() {
        let spec = spec_by_name(tool);
        for mode in all_modes() {
            let without_confirmation = agent_tool_permission(tool, mode, false);
            let with_confirmation = agent_tool_permission(tool, mode, true);
            let allowed_by_mode = modes.contains(&mode);
            assert_eq!(
                without_confirmation.allowed,
                allowed_by_mode && !spec.requires_confirmation,
                "permission without confirmation for {tool:?} in {mode:?}"
            );
            assert_eq!(
                with_confirmation.allowed, allowed_by_mode,
                "permission with confirmation for {tool:?} in {mode:?}"
            );
            assert_eq!(
                with_confirmation.requires_confirmation, spec.requires_confirmation,
                "confirmation flag for {tool}"
            );
        }
    }
    assert!(!agent_tool_permission("delete_everything", AgentMode::Agent, true).allowed);
}

#[test]
fn registry_declares_path_scope_contracts() {
    let save_plan = spec_by_name("save_cad_plan");
    assert_eq!(
        save_plan.path_policy.semantic_store,
        Some(AgentSemanticStore::CadPlan)
    );
    assert_eq!(save_plan.path_policy.allowed_roots, vec!["plans"]);
    assert_eq!(
        save_plan.path_policy.output_paths,
        OutputPathPolicy::DeclaredOutputsOnly
    );

    let chat_summary = spec_by_name("update_chat_summary");
    assert_eq!(
        chat_summary.path_policy.semantic_store,
        Some(AgentSemanticStore::ChatSummary)
    );
    assert!(chat_summary.path_policy.denied_roots.contains(&"chats"));

    for tool in ["write_file", "patch_file"] {
        let spec = spec_by_name(tool);
        assert!(spec.path_policy.requires_confirmation_scope);
        assert_eq!(
            spec.path_policy.cadquery_model_file,
            CadQueryModelFilePolicy::Denied
        );
        assert!(spec.path_policy.denied_roots.contains(&"outputs"));
        assert!(spec.path_policy.denied_roots.contains(&"chats"));
        assert!(!spec.path_policy.allowed_roots.contains(&"plans"));
    }

    let copy_file = spec_by_name("copy_file");
    assert_eq!(
        copy_file.path_policy.cadquery_model_file,
        CadQueryModelFilePolicy::CopyOnly
    );
    assert!(!copy_file.path_policy.allowed_roots.contains(&"plans"));

    let cadquery_execute = spec_by_name("cadquery_execute");
    assert!(cadquery_execute.path_policy.requires_confirmation_scope);
    assert_eq!(
        cadquery_execute.path_policy.cadquery_model_file,
        CadQueryModelFilePolicy::CadQueryToolOnly
    );
    assert_eq!(
        cadquery_execute.path_policy.output_paths,
        OutputPathPolicy::ConfirmationOutputsOnly
    );

    let dry_run = spec_by_name("cadquery_dry_run");
    assert_eq!(
        dry_run.path_policy.output_paths,
        OutputPathPolicy::TemporaryResultCacheOnly
    );
    assert!(!dry_run.path_policy.requires_confirmation_scope);

    for tool in ["read_file", "list_directory", "cadquery_analyze_source"] {
        assert!(
            spec_by_name(tool)
                .path_policy
                .denied_roots
                .contains(&".budn_staging")
        );
    }
}

#[test]
fn registry_uses_specific_canonical_schemas() {
    assert_required("cadquery_execute", &["target_path", "target_type", "code"]);
    assert_required("cadquery_get_result", &["result_id"]);
    assert_required(
        "cadquery_resolve_selection",
        &["result_id", "selection_ref"],
    );
    assert_required(
        "patch_file",
        &["path", "expected_hash", "search", "replace"],
    );
    assert_required(
        "save_cad_plan",
        &[
            "title",
            "target_ref",
            "resolved_target",
            "affected_files",
            "export_targets",
        ],
    );
    assert_success_required(
        "save_cad_plan",
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
    );
    assert_required("update_chat_summary", &["summary", "goal"]);
    assert_success_required(
        "update_chat_summary",
        &["session_id", "message_id", "updated_fields"],
    );
    assert_success_required("read_file", &["path", "text", "hash", "truncated"]);
    assert_success_required(
        "cadquery_execute",
        &["result_id", "build_id", "committed_files"],
    );
    assert_success_required(
        "cadquery_get_result",
        &["result_id", "root_ref_text", "parts"],
    );

    let error_schema = spec_by_name("cadquery_execute").error_schema;
    assert_schema_required(&error_schema, &["tool_call_id", "error_type"]);
    let error_types = error_schema["properties"]["error_type"]["enum"]
        .as_array()
        .expect("error type enum");
    assert!(
        error_types
            .iter()
            .any(|value| value == "python_import_error")
    );
    assert!(error_types.iter().any(|value| value == "unsupported_tool"));

    let check_success = spec_by_name("cadquery_check_source").success_schema;
    let contract = &check_success["properties"]["contract"];
    assert_schema_required(
        contract,
        &[
            "target_type_matches",
            "has_build_function",
            "has_refs",
            "unsafe_calls",
        ],
    );

    let execute_success = spec_by_name("cadquery_execute").success_schema;
    let summary = &execute_success["properties"]["summary"];
    assert_schema_required(
        summary,
        &["part_count", "face_count", "edge_count", "vertex_count"],
    );
}

fn spec_by_name(tool: &str) -> AgentToolSpec {
    agent_tool_specs()
        .into_iter()
        .find(|spec| spec.definition.name == tool)
        .unwrap_or_else(|| panic!("missing tool spec: {tool}"))
}

fn expected_tool_modes() -> Vec<(&'static str, Vec<AgentMode>)> {
    let readonly = vec![AgentMode::Agent, AgentMode::Plan];
    vec![
        ("read_file", readonly.clone()),
        ("list_directory", readonly.clone()),
        ("search_files", readonly.clone()),
        ("get_project_context", readonly.clone()),
        ("get_selection", readonly.clone()),
        ("resolve_ref", readonly.clone()),
        ("cadquery_analyze_source", readonly.clone()),
        ("cadquery_get_result", readonly.clone()),
        ("cadquery_resolve_selection", readonly),
        ("update_chat_summary", vec![AgentMode::Agent]),
        ("save_cad_plan", vec![AgentMode::Plan]),
        (
            "cadquery_check_source",
            vec![AgentMode::Plan, AgentMode::Agent],
        ),
        ("cadquery_dry_run", vec![AgentMode::Agent]),
        ("write_file", vec![AgentMode::Agent]),
        ("patch_file", vec![AgentMode::Agent]),
        ("copy_file", vec![AgentMode::Agent]),
        ("cadquery_execute", vec![AgentMode::Agent]),
    ]
}

fn all_modes() -> [AgentMode; 2] {
    [AgentMode::Agent, AgentMode::Plan]
}

fn tool_names(mode: AgentMode) -> Vec<String> {
    agent_tool_definitions_for_mode(mode)
        .into_iter()
        .map(|definition| definition.name)
        .collect()
}

fn assert_required(tool: &str, fields: &[&str]) {
    let schema = spec_by_name(tool).definition.parameters;
    assert_schema_required(&schema, fields);
}

fn assert_success_required(tool: &str, fields: &[&str]) {
    let schema = spec_by_name(tool).success_schema;
    assert_schema_required(&schema, fields);
}

fn assert_schema_required(schema: &serde_json::Value, fields: &[&str]) {
    let required = schema["required"].as_array().expect("required array");
    for field in fields {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "schema should require {field}"
        );
    }
}
