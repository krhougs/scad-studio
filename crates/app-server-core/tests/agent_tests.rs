use app_server_core::{
    AgentCadQueryCodeInput, AgentTurnInput, draft_agent_turn, generate_cadquery_code,
    rig_backend_decision,
};
use app_server_protocol::{
    AgentOperationLevel, ChatMessageRecord, ChatRole, SelectionKind, SelectionRef,
};

#[test]
fn rig_backend_decision_records_current_compatible_version() {
    let decision = rig_backend_decision();

    assert_eq!(decision.crate_name, "rig-core");
    assert_eq!(decision.evaluated_version, "0.35.0");
    assert!(decision.selected);
    assert!(decision.rationale.contains("tool"));
    assert!(decision.rationale.contains("stream"));
}

#[test]
fn draft_agent_turn_uses_prompt_history_selection_and_execute_target() {
    let draft = draft_agent_turn(AgentTurnInput {
        operation: AgentOperationLevel::Execute,
        prompt: "make the lid taller".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        confirmed_target_path: Some("parts/top_lid.py".into()),
    });

    assert!(draft.text.contains("Execute"));
    assert!(draft.text.contains("make the lid taller"));
    assert!(draft.text.contains("previous plan"));
    assert!(draft.text.contains("@face[top_lid:f_0]"));
    assert!(draft.text.contains("parts/top_lid.py"));
}

#[test]
fn local_agent_backend_generates_cadquery_code_instead_of_echoing_prompt() {
    let generated = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "make a 42 mm taller lid from chat".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        target_display_path: "parts/top_lid.py".into(),
    })
    .expect("generate cadquery code");

    assert!(generated.code.contains("import cadquery as cq"));
    assert!(generated.code.contains("def build(params=None):"));
    assert!(generated.code.contains("height\", 42.000"));
    assert!(generated.code.contains(".tag(\"top_lid\")"));
    assert!(!generated.code.contains("make a 42 mm taller lid from chat"));
    assert!(generated.response_text.contains("parts/top_lid.py"));
}

fn chat_message(id: &str, content: &str) -> ChatMessageRecord {
    ChatMessageRecord {
        message_id: id.into(),
        ts_ms: 1,
        role: ChatRole::User,
        content: content.into(),
        related_files: Vec::new(),
        tool_call_id: None,
        tool_calls: Vec::new(),
        tool_result: None,
        mesh_result: None,
    }
}

fn selection(ref_text: &str) -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Face,
        ref_text: ref_text.into(),
        owner_ref_text: Some("@part[top_lid]".into()),
        owner_object_kind: Some(app_server_protocol::CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[top_lid.top_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}
