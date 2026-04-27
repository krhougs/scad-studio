use app_server_core::{
    AgentCadQueryCodeInput, AgentTurnInput, draft_agent_turn, generate_cadquery_code,
    rig_backend_decision,
};
use app_server_protocol::{
    AgentOperationLevel, CadQueryObjectKind, ChatMessageRecord, ChatRole, SelectionKind,
    SelectionRef,
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
        active_selection_index: Some(0),
        confirmed_target_path: Some("parts/top_lid.py".into()),
    });

    assert!(draft.text.contains("Execute"));
    assert!(draft.text.contains("make the lid taller"));
    assert!(draft.text.contains("previous plan"));
    assert!(draft.text.contains("@face[top_lid:f_0]"));
    assert!(draft.text.contains("parts/top_lid.py"));
}

#[test]
fn plan_turn_maps_raw_face_selection_to_feature_and_part_target() {
    let draft = draft_agent_turn(AgentTurnInput {
        operation: AgentOperationLevel::Plan,
        prompt: "add a vent on this face".into(),
        history: vec![chat_message("msg-1", "initial enclosure plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        active_selection_index: Some(0),
        confirmed_target_path: None,
    });

    assert!(draft.text.contains("## CAD Plan"));
    assert!(draft.text.contains("@feature[top_lid.top_surface]"));
    assert!(draft.text.contains("parts/top_lid.py"));
    assert!(draft.text.contains("part geometry"));
}

#[test]
fn local_agent_backend_generates_cadquery_code_instead_of_echoing_prompt() {
    let generated = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "make a 42 mm taller lid from chat".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        active_selection_index: Some(0),
        target_display_path: "parts/top_lid.py".into(),
        target_type: CadQueryObjectKind::Part,
    })
    .expect("generate cadquery code");

    assert!(generated.code.contains("import cadquery as cq"));
    assert!(generated.code.contains("def build(params=None):"));
    assert!(generated.code.contains("height\", 42.000"));
    assert!(generated.code.contains(".tag(\"top_lid\")"));
    assert!(!generated.code.contains("make a 42 mm taller lid from chat"));
    assert!(generated.response_text.contains("parts/top_lid.py"));
}

#[test]
fn local_agent_backend_names_selection_modification_target() {
    let generated = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "open a slot on the selected face".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        active_selection_index: Some(0),
        target_display_path: "parts/top_lid.py".into(),
        target_type: CadQueryObjectKind::Part,
    })
    .expect("generate cadquery code");

    assert!(
        generated
            .response_text
            .contains("@feature[top_lid.top_surface]")
    );
    assert!(generated.response_text.contains("parts/top_lid.py"));
    assert!(generated.response_text.contains("part geometry"));
    assert!(generated.code.contains("SELECTION_REF"));
    assert!(generated.code.contains("cutThruAll"));
}

#[test]
fn local_agent_backend_uses_selected_feature_selector_for_face_cut() {
    let generated = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "open a slot on the selected face".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![bottom_face_selection()],
        active_selection_index: Some(0),
        target_display_path: "parts/top_lid.py".into(),
        target_type: CadQueryObjectKind::Part,
    })
    .expect("generate cadquery code");

    assert!(generated.code.contains("faces(\"<Z\")"));
    assert!(!generated.code.contains("faces(\">Z\").workplane().rect"));
}

#[test]
fn local_agent_backend_does_not_modify_raw_face_without_feature_mapping() {
    let generated = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "open a slot on the selected face".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![raw_face_selection()],
        active_selection_index: Some(0),
        target_display_path: "parts/top_lid.py".into(),
        target_type: CadQueryObjectKind::Part,
    })
    .expect("generate cadquery code");

    assert!(generated.response_text.contains("@face[top_lid:f_9]"));
    assert!(generated.code.contains("SELECTION_REF"));
    assert!(!generated.code.contains(".workplane().rect"));
    assert!(!generated.code.contains("cutThruAll"));
}

#[test]
fn plan_turn_declares_instance_replacement_multi_file_scope() {
    let draft = draft_agent_turn(AgentTurnInput {
        operation: AgentOperationLevel::Plan,
        prompt: "replace this screw with a countersunk version".into(),
        history: vec![chat_message("msg-1", "assembly plan")],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        confirmed_target_path: None,
    });

    assert!(draft.text.contains("components/screw.py"));
    assert!(draft.text.contains("assemblies/full_enclosure.py"));
    assert!(draft.text.contains("assembly instance replacement"));
    assert!(draft.text.contains("Target: assemblies/full_enclosure.py"));
}

#[test]
fn local_agent_backend_generates_assembly_code_for_instance_move() {
    let generated = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "move this screw 5mm right".into(),
        history: vec![chat_message("msg-1", "assembly plan")],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        target_display_path: "assemblies/full_enclosure.py".into(),
        target_type: CadQueryObjectKind::Assembly,
    })
    .expect("generate assembly cadquery code");

    assert!(generated.code.contains("cq.Assembly"));
    assert!(!generated.code.contains("return cq.Workplane(\"XY\").box"));
    assert!(generated.response_text.contains("assembly coordination"));
}

#[test]
fn plan_turn_labels_component_body_edit_as_component_geometry() {
    let draft = draft_agent_turn(AgentTurnInput {
        operation: AgentOperationLevel::Plan,
        prompt: "make this component wider".into(),
        history: vec![chat_message("msg-1", "component plan")],
        selections: vec![component_selection()],
        active_selection_index: Some(0),
        confirmed_target_path: None,
    });

    assert!(draft.text.contains("Target: components/screw.py"));
    assert!(draft.text.contains("component geometry"));
    assert!(!draft.text.contains("component placement"));
}

#[test]
fn plan_turn_labels_instance_body_edit_as_component_geometry() {
    let draft = draft_agent_turn(AgentTurnInput {
        operation: AgentOperationLevel::Plan,
        prompt: "make this selected instance wider".into(),
        history: vec![chat_message("msg-1", "component plan")],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        confirmed_target_path: None,
    });

    assert!(draft.text.contains("Target: components/screw.py"));
    assert!(draft.text.contains("component geometry"));
    assert!(!draft.text.contains("assembly coordination"));
}

#[test]
fn plan_turn_uses_active_selection_and_keeps_ambiguous_raw_ref() {
    let draft = draft_agent_turn(AgentTurnInput {
        operation: AgentOperationLevel::Plan,
        prompt: "modify the active face".into(),
        history: vec![chat_message("msg-1", "initial enclosure plan")],
        selections: vec![selection("@face[top_lid:f_0]"), ambiguous_selection()],
        active_selection_index: Some(1),
        confirmed_target_path: None,
    });

    assert!(draft.text.contains("@face[top_lid:f_1]"));
    assert!(!draft.text.contains("@feature[top_lid.ambiguous_surface]"));
    assert!(draft.text.contains("ambiguous selection"));
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
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[top_lid.top_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}

fn instance_selection() -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Instance,
        ref_text: "@instance[full_enclosure/screw_1]".into(),
        owner_ref_text: Some("@component[screw]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Component),
        instance_path: Some("full_enclosure/screw_1".into()),
        candidate_feature_ref: None,
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}

fn component_selection() -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Component,
        ref_text: "@component[screw]".into(),
        owner_ref_text: None,
        owner_object_kind: None,
        instance_path: None,
        candidate_feature_ref: None,
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}

fn ambiguous_selection() -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[top_lid:f_1]".into(),
        owner_ref_text: Some("@part[top_lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[top_lid.ambiguous_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: true,
    }
}

fn raw_face_selection() -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[top_lid:f_9]".into(),
        owner_ref_text: Some("@part[top_lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: None,
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}

fn bottom_face_selection() -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[top_lid:f_2]".into(),
        owner_ref_text: Some("@part[top_lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[top_lid.bottom_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}
