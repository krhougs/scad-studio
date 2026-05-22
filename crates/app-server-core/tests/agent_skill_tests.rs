use app_server_core::{AgentTurnInput, SkillInjectionContext, active_skill_names, build_turn_context, collect_skill_injections};
use app_server_protocol::AgentMode;

fn make_skill_ctx(mode: AgentMode, cq_tools: bool, last_error: bool) -> SkillInjectionContext {
    SkillInjectionContext {
        mode,
        cadquery_execution_tools_registered: cq_tools,
        last_turn_cadquery_error: last_error,
    }
}

#[test]
fn agent_mode_with_cadquery_tools_injects_all_three_skills() {
    let ctx = make_skill_ctx(AgentMode::Agent, true, false);
    let names = active_skill_names(&ctx);
    assert_eq!(names, vec!["failure-recovery", "engineering-defaults", "structured-brief"]);
}

#[test]
fn plan_mode_injects_no_skills() {
    let ctx = make_skill_ctx(AgentMode::Plan, false, false);
    let names = active_skill_names(&ctx);
    assert!(names.is_empty(), "Plan mode should not inject skills, got: {names:?}");
}

#[test]
fn agent_mode_without_cadquery_tools_injects_no_skills() {
    let ctx = make_skill_ctx(AgentMode::Agent, false, false);
    let names = active_skill_names(&ctx);
    assert!(names.is_empty());
}

#[test]
fn failure_recovery_compact_when_no_previous_error() {
    let ctx = make_skill_ctx(AgentMode::Agent, true, false);
    let text = collect_skill_injections(&ctx);
    assert!(text.contains("[Skill: failure-recovery]"));
    assert!(text.contains("Maximum 2 automatic retries per turn"));
    assert!(!text.contains("The previous turn had a CadQuery execution error"));
}

#[test]
fn failure_recovery_full_when_previous_error() {
    let ctx = make_skill_ctx(AgentMode::Agent, true, true);
    let text = collect_skill_injections(&ctx);
    assert!(text.contains("[Skill: failure-recovery]"));
    assert!(text.contains("The previous turn had a CadQuery execution error"));
    assert!(text.contains("Failure classification and repair procedures"));
}

#[test]
fn engineering_defaults_contains_wall_thickness() {
    let ctx = make_skill_ctx(AgentMode::Agent, true, false);
    let text = collect_skill_injections(&ctx);
    assert!(text.contains("Wall thickness: 2–3 mm"));
    assert!(text.contains("M3=3.4 mm"));
}

#[test]
fn structured_brief_contains_template() {
    let ctx = make_skill_ctx(AgentMode::Agent, true, false);
    let text = collect_skill_injections(&ctx);
    assert!(text.contains("**Brief**"));
    assert!(text.contains("Target dimensions"));
    assert!(text.contains("Do NOT output a brief for"));
}

#[test]
fn build_turn_context_includes_skills_in_agent_mode() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "test".into(),
        history: vec![],
        selections: vec![],
        active_selection_index: None,
        plan_ref: None,
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(context.contains("Active skills:"));
    assert!(context.contains("[Skill: failure-recovery]"));
    assert!(context.contains("[Skill: engineering-defaults]"));
    assert!(context.contains("[Skill: structured-brief]"));
}

#[test]
fn build_turn_context_no_skills_in_plan_mode() {
    let input = AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "test".into(),
        history: vec![],
        selections: vec![],
        active_selection_index: None,
        plan_ref: None,
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(!context.contains("Active skills:"));
    assert!(!context.contains("[Skill:"));
}

#[test]
fn build_turn_context_detects_cadquery_error_in_history() {
    use app_server_protocol::{ChatMessageRecord, ChatRole, ChatToolResultRecord};

    let error_result = ChatToolResultRecord {
        tool_call_id: "call_1".into(),
        tool_name: "cadquery_dry_run".into(),
        result_json: r#"{"status":"error","error_type":"geometry_invalid","message":"empty shape"}"#.into(),
    };
    let tool_msg = ChatMessageRecord {
        message_id: "msg_tool".into(),
        ts_ms: 1000,
        role: ChatRole::Tool,
        content: String::new(),
        related_files: vec![],
        tool_call_id: Some("call_1".into()),
        tool_calls: vec![],
        tool_result: Some(error_result),
        mesh_result: None,
        search_sources: vec![],
        run_id: None,
        agent_id: None,
        turn_id: None,
    };
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "fix it".into(),
        history: vec![tool_msg],
        selections: vec![],
        active_selection_index: None,
        plan_ref: None,
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(context.contains("The previous turn had a CadQuery execution error"));
}
