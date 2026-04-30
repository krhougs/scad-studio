use serde_json::{Value, json};

use super::{
    nullable_string_schema, object_schema, string_array_schema, string_schema, success_schema,
    value_object,
};

pub fn cadquery_analyze_source_input_schema() -> Value {
    object_schema(
        json!({
            "target_path": string_schema("Workspace-relative CadQuery source path."),
            "include_paired_doc": {"type": "boolean"},
            "include_dependencies": {"type": "boolean"}
        }),
        &["target_path"],
    )
}

pub fn cadquery_check_source_input_schema() -> Value {
    cadquery_source_input_schema(&["target_path", "target_type", "code"])
}

pub fn cadquery_dry_run_input_schema() -> Value {
    object_schema(
        cadquery_source_properties(json!({
            "params_json": string_schema("JSON parameter object encoded as a string."),
            "selection_context": string_array_schema(),
            "preview_quality": {"type": "string", "enum": ["fast", "final"]}
        })),
        &["target_path", "target_type", "code"],
    )
}

pub fn cadquery_execute_input_schema() -> Value {
    object_schema(
        cadquery_source_properties(json!({
            "params_json": string_schema("JSON parameter object encoded as a string."),
            "export_formats": export_formats_schema(),
            "export_targets": export_targets_schema(),
            "plan_ref": string_schema("Saved plan path."),
            "reason": string_schema("Short execution reason.")
        })),
        &[
            "target_path",
            "target_type",
            "code",
            "export_formats",
            "export_targets",
        ],
    )
}

pub fn cadquery_get_result_input_schema() -> Value {
    object_schema(
        json!({
            "result_id": string_schema("CadQuery result id."),
            "include_feature_map": {"type": "boolean"},
            "include_exports": {"type": "boolean"}
        }),
        &["result_id"],
    )
}

pub fn cadquery_resolve_selection_input_schema() -> Value {
    object_schema(
        json!({
            "result_id": string_schema("CadQuery result id."),
            "selection_ref": string_schema("Visible selection ref.")
        }),
        &["result_id", "selection_ref"],
    )
}

pub fn cadquery_analyze_source_success_schema() -> Value {
    success_schema(
        json!({
            "target_path": string_schema("Workspace-relative CadQuery source path."),
            "target_type": string_schema("part, component or assembly."),
            "has_build_function": {"type": "boolean"},
            "has_refs": {"type": "boolean"},
            "paired_doc_path": nullable_string_schema(),
            "local_dependencies": string_array_schema(),
            "ref_keys": string_array_schema(),
            "warnings": string_array_schema()
        }),
        &[
            "target_path",
            "target_type",
            "has_build_function",
            "has_refs",
            "warnings",
        ],
    )
}

pub fn cadquery_check_source_success_schema() -> Value {
    success_schema(
        json!({
            "contract": contract_schema(),
            "warnings": string_array_schema()
        }),
        &["contract", "warnings"],
    )
}

pub fn cadquery_run_success_schema() -> Value {
    success_schema(
        json!({
            "result_id": string_schema("Temporary result id."),
            "build_id": string_schema("Build hash."),
            "root_object_kind": string_schema("part, component or assembly."),
            "summary": run_summary_schema(),
            "warnings": string_array_schema()
        }),
        &[
            "result_id",
            "build_id",
            "root_object_kind",
            "summary",
            "warnings",
        ],
    )
}

pub fn cadquery_execute_success_schema() -> Value {
    success_schema(
        json!({
            "result_id": string_schema("Committed result id."),
            "build_id": string_schema("Build hash."),
            "committed_files": string_array_schema(),
            "exports": string_array_schema(),
            "summary": run_summary_schema(),
            "warnings": string_array_schema()
        }),
        &[
            "result_id",
            "build_id",
            "committed_files",
            "exports",
            "summary",
        ],
    )
}

pub fn cadquery_get_result_success_schema() -> Value {
    success_schema(
        json!({
            "result_id": string_schema("CadQuery result id."),
            "build_id": string_schema("Build hash."),
            "root_ref_text": string_schema("Root visible ref."),
            "root_object_kind": string_schema("part, component or assembly."),
            "parts": {"type": "array", "items": part_summary_schema()},
            "exports": string_array_schema()
        }),
        &[
            "result_id",
            "build_id",
            "root_ref_text",
            "root_object_kind",
            "parts",
        ],
    )
}

fn cadquery_source_input_schema(required: &[&str]) -> Value {
    object_schema(cadquery_source_properties(json!({})), required)
}

fn cadquery_source_properties(extra: Value) -> Value {
    let mut properties = value_object(extra);
    properties.insert(
        "target_path".into(),
        string_schema("Workspace-relative CadQuery source path."),
    );
    properties.insert(
        "target_type".into(),
        json!({"type": "string", "enum": ["part", "component", "assembly"]}),
    );
    properties.insert(
        "code".into(),
        string_schema(
            "Complete CadQuery Python source. Must include module-level MODEL_DESCRIPTION, MODEL_DETAILS with purpose, key_dimensions, intended_use, assumptions, interaction_notes, and manufacturing_or_placement_constraints, REFS with type part|component|assembly and non-empty features named from the actual model semantics, and def build(params=None): ... returning the model.",
        ),
    );
    Value::Object(properties)
}

fn export_formats_schema() -> Value {
    json!({
        "type": "array",
        "items": {"type": "string", "enum": ["step", "stl", "3mf"]},
        "description": "Derived artifact formats to export. cadquery_execute must include step so .py source and .step output stay synchronized."
    })
}

fn export_targets_schema() -> Value {
    json!({
        "type": "array",
        "items": {"type": "string"},
        "description": "Workspace-relative derived artifact paths under outputs/. cadquery_execute must include the matching outputs/<target-stem>.step path."
    })
}

fn contract_schema() -> Value {
    object_schema(
        json!({
            "target_type_matches": {"type": "boolean"},
            "has_build_function": {"type": "boolean"},
            "has_refs": {"type": "boolean"},
            "has_model_description": {"type": "boolean"},
            "unsafe_calls": string_array_schema(),
            "invalid_imports": string_array_schema()
        }),
        &[
            "target_type_matches",
            "has_build_function",
            "has_refs",
            "has_model_description",
            "unsafe_calls",
            "invalid_imports",
        ],
    )
}

fn run_summary_schema() -> Value {
    object_schema(
        json!({
            "part_count": {"type": "integer"},
            "face_count": {"type": "integer"},
            "edge_count": {"type": "integer"},
            "vertex_count": {"type": "integer"},
            "features": string_array_schema()
        }),
        &["part_count", "face_count", "edge_count", "vertex_count"],
    )
}

fn part_summary_schema() -> Value {
    object_schema(
        json!({
            "ref_text": string_schema("Visible object ref."),
            "object_kind": string_schema("part, component or assembly."),
            "instance_path": nullable_string_schema(),
            "features": string_array_schema(),
            "face_count": {"type": "integer"},
            "edge_count": {"type": "integer"},
            "vertex_count": {"type": "integer"}
        }),
        &["ref_text", "object_kind", "features"],
    )
}
