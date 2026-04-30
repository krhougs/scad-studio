use app_server_core::{
    AgentCadQueryCodeInput, AgentExecutionScope, AgentTurnInput, cadquery_agent_system_prompt,
    draft_agent_turn, generate_cadquery_code, llm_request_for_cadquery_execute, mode_for_tool_loop,
    rig_backend_decision,
};
use app_server_protocol::{
    AgentMode, CadQueryObjectKind, ChatMessageRecord, ChatRole, SelectionKind, SelectionRef,
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
fn cadquery_agent_system_prompt_covers_runtime_contract() {
    let prompt = cadquery_agent_system_prompt();

    for section in [
        "Role",
        "Core Principles",
        "Modes",
        "File System Contract",
        "Component / Part / Assembly Rules",
        "Ref Handling Rules",
        "Workspace Plan Package Rules",
        "Tool Permission Rules",
        "Experiment Rules",
        "Response Rules",
    ] {
        assert!(prompt.contains(section), "missing section: {section}");
    }
    assert!(prompt.contains("`Agent`"));
    assert!(prompt.contains("`Plan`"));
    assert!(!prompt.contains("Inform / Plan / Execute"));
    assert!(!prompt.contains("confirmed_cadquery"));
    assert!(prompt.contains("source of truth"));
    assert!(prompt.contains("`.py` files are model source code"));
    assert!(prompt.contains("`.md` files are semantic design notes"));
    assert!(prompt.contains("`outputs/` contains derived artifacts only"));
    assert!(prompt.contains("@component"));
    assert!(prompt.contains("@part"));
    assert!(prompt.contains("@assembly"));
    assert!(prompt.contains("@feature"));
    assert!(prompt.contains("@selector"));
    assert!(prompt.contains("@face"));
    assert!(prompt.contains("@edge"));
    assert!(prompt.contains("@vertex"));
    assert!(prompt.contains("Raw face / edge / vertex refs are not long-term truth"));
    assert!(prompt.contains("Do not expose selector refs as MVP protocol selections"));
    assert!(prompt.contains("CadQuery execution completed with warnings"));
    assert!(prompt.contains("diagnostics.traceback"));
    assert!(prompt.contains("byte-for-byte variants"));
    assert!(prompt.contains("Static CadQuery source checks"));
    let tool_rules = prompt
        .split("## 8. Tool Permission Rules")
        .nth(1)
        .expect("tool permission section exists");
    assert!(tool_rules.contains("| `update_chat_summary` semantic tool | Not allowed | Allowed |"));
    assert!(tool_rules.contains("| Static CadQuery source checks | Allowed | Allowed |"));
    assert!(tool_rules.contains("| `cadquery_dry_run` | Not allowed | Allowed through staging |"));
    assert!(tool_rules.contains(
        "| `cadquery_execute` | Not allowed | Allowed through staging and execution scope |"
    ));
    assert!(!prompt.contains("keyword matching"));
}

#[test]
fn cadquery_agent_system_prompt_uses_domain_neutral_feature_examples() {
    let prompt = cadquery_agent_system_prompt();

    for forbidden in [
        "AirPods",
        "airpods",
        "charging_pad",
        "airpods_recess",
        "wireless charging",
        "outer_shell",
        "mounting_boss",
        "alignment_slot",
        "cable_relief_channel",
    ] {
        assert!(
            !prompt.contains(forbidden),
            "system prompt must not contain current acceptance scenario token: {forbidden}"
        );
    }
}

#[test]
fn cadquery_agent_system_prompt_owns_feature_key_naming() {
    let prompt = cadquery_agent_system_prompt();

    assert!(prompt.contains("REFS.features keys are your responsibility"));
    assert!(prompt.contains("tool schemas, warnings, and errors describe structure only"));
    assert!(prompt.contains("do not provide feature names to copy"));
    assert!(prompt.contains("Preserve existing stable feature keys"));
}

#[test]
fn cadquery_execute_llm_request_uses_system_prompt_and_structured_context() {
    let request = llm_request_for_cadquery_execute(AgentCadQueryCodeInput {
        prompt: "replace this screw with a countersunk version".into(),
        history: vec![
            chat_message("msg-1", "initial CAD Plan: preserve head clearance"),
            chat_message("msg-2", ""),
            chat_message("msg-3", "explicit execution scope: edit selected component"),
        ],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        target_display_path: "components/screw.py".into(),
        target_type: CadQueryObjectKind::Component,
        execution_scope: Some(AgentExecutionScope::for_plan(
            "plans/2026042900-screw",
            "plans/2026042900-screw/plan-result.md",
            "components/screw.py",
            CadQueryObjectKind::Component,
            vec!["components/screw.py".into()],
            Vec::new(),
            vec!["outputs/screw.step".into()],
        )),
    });

    assert_eq!(request.system_prompt, cadquery_agent_system_prompt());
    assert!(request.context.contains("Mode: Agent"));
    assert!(request.context.contains("Target path: components/screw.py"));
    assert!(request.context.contains("Target type: component"));
    assert!(request.context.contains("Execution scope"));
    assert!(request.context.contains("plan_ref=plans/2026042900-screw"));
    assert!(
        request
            .context
            .contains("export_targets=outputs/screw.step")
    );
    assert!(request.context.contains("staging"));
    assert!(request.context.contains("Active selection index: 0"));
    assert!(
        request
            .context
            .contains("@instance[full_enclosure/screw_1]")
    );
    assert!(request.context.contains("owner_ref_text=@component[screw]"));
    assert!(
        request
            .context
            .contains("initial CAD Plan: preserve head clearance")
    );
    assert!(
        request
            .context
            .contains("explicit execution scope: edit selected component")
    );
    assert!(!request.context.contains("assembly instance replacement"));
}

#[test]
fn draft_agent_turn_uses_prompt_history_selection_and_plan_ref() {
    let draft = draft_agent_turn(AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "make the lid taller".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        active_selection_index: Some(0),
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    });

    assert!(draft.text.contains("Agent"));
    assert!(draft.text.contains("make the lid taller"));
    assert!(draft.text.contains("previous plan"));
    assert!(draft.text.contains("@face[top_lid:f_0]"));
    assert!(draft.text.contains("Plan ref: none"));
}

#[test]
fn mode_for_tool_loop_preserves_requested_product_mode() {
    assert_eq!(mode_for_tool_loop(AgentMode::Agent), AgentMode::Agent);
    assert_eq!(mode_for_tool_loop(AgentMode::Plan), AgentMode::Plan);
}

#[test]
fn plan_turn_maps_raw_face_selection_to_feature_and_part_target() {
    let draft = draft_agent_turn(AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "add a vent on this face".into(),
        history: vec![chat_message("msg-1", "initial enclosure plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        active_selection_index: Some(0),
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    });

    assert!(draft.text.contains("## CAD Plan"));
    assert!(
        draft
            .text
            .contains("@feature[top_lid.lid_alignment_surface]")
    );
    assert!(draft.text.contains("parts/top_lid.py"));
    assert!(draft.text.contains("part geometry"));
}

#[test]
fn local_agent_backend_refuses_part_codegen_without_llm_backend() {
    let error = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "make a 42 mm taller lid from chat".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![selection("@face[top_lid:f_0]")],
        active_selection_index: Some(0),
        target_display_path: "parts/top_lid.py".into(),
        target_type: CadQueryObjectKind::Part,
        execution_scope: None,
    })
    .expect_err("local fallback must not generate CadQuery geometry");

    assert!(error.message.contains("LLM"));
    assert!(!error.message.contains("make a 42 mm taller lid from chat"));
}

#[test]
fn local_agent_backend_refuses_assembly_codegen_without_llm_backend() {
    let error = generate_cadquery_code(AgentCadQueryCodeInput {
        prompt: "adjust this selected instance".into(),
        history: vec![chat_message("msg-1", "previous plan")],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        target_display_path: "assemblies/full_enclosure.py".into(),
        target_type: CadQueryObjectKind::Assembly,
        execution_scope: None,
    })
    .expect_err("local fallback must not generate assembly geometry");

    assert!(error.message.contains("LLM"));
    assert!(!error.message.contains("cq.Assembly"));
}

#[test]
fn plan_turn_does_not_infer_instance_replacement_from_prompt_words() {
    let draft = draft_agent_turn(AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "replace this screw with a countersunk version".into(),
        history: vec![chat_message("msg-1", "assembly plan")],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    });

    assert!(draft.text.contains("Target: components/screw.py"));
    assert!(draft.text.contains("component geometry"));
    assert!(!draft.text.contains("assemblies/full_enclosure.py"));
    assert!(!draft.text.contains("assembly instance replacement"));
}

#[test]
fn plan_turn_does_not_infer_instance_movement_from_prompt_words() {
    let draft = draft_agent_turn(AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "move this screw 5mm right".into(),
        history: vec![chat_message("msg-1", "assembly plan")],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    });

    assert!(draft.text.contains("Target: components/screw.py"));
    assert!(draft.text.contains("component geometry"));
    assert!(!draft.text.contains("assemblies/full_enclosure.py"));
    assert!(!draft.text.contains("assembly coordination"));
}

#[test]
fn plan_turn_labels_component_body_edit_as_component_geometry() {
    let draft = draft_agent_turn(AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "make this component wider".into(),
        history: vec![chat_message("msg-1", "component plan")],
        selections: vec![component_selection()],
        active_selection_index: Some(0),
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    });

    assert!(draft.text.contains("Target: components/screw.py"));
    assert!(draft.text.contains("component geometry"));
    assert!(!draft.text.contains("component placement"));
}

#[test]
fn plan_turn_labels_instance_body_edit_as_component_geometry() {
    let draft = draft_agent_turn(AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "make this selected instance wider".into(),
        history: vec![chat_message("msg-1", "component plan")],
        selections: vec![instance_selection()],
        active_selection_index: Some(0),
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
    });

    assert!(draft.text.contains("Target: components/screw.py"));
    assert!(draft.text.contains("component geometry"));
    assert!(!draft.text.contains("assembly coordination"));
}

#[test]
fn plan_turn_uses_active_selection_and_keeps_ambiguous_raw_ref() {
    let draft = draft_agent_turn(AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "modify the active face".into(),
        history: vec![chat_message("msg-1", "initial enclosure plan")],
        selections: vec![selection("@face[top_lid:f_0]"), ambiguous_selection()],
        active_selection_index: Some(1),
        plan_ref: None,
        context_refs: Vec::new(),
        execution_scope: None,
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
        run_id: None,
    }
}

fn selection(ref_text: &str) -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Face,
        ref_text: ref_text.into(),
        owner_ref_text: Some("@part[top_lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[top_lid.lid_alignment_surface]".into()),
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
