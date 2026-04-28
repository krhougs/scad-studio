use app_server_host::{
    export_handle_for, extract_object_name, extract_plan_from_json_block,
    extract_plan_from_selection, extract_plan_proposal,
};
use app_server_protocol::{
    CadQueryObjectKind, PathHandle, SelectionKind, SelectionRef, SelectionUpdateRequest,
    WorkspaceId,
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
