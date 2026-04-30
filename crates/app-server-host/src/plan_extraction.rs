use std::path::Path;

use app_server_core::{AgentExecutionScope, ParsedPlanPackage};
use app_server_protocol::{
    AgentCadQueryConfirmation, CadQueryObjectKind, ChatMessageRecord, PathHandle, ProtocolError,
    ProtocolErrorCode, SelectionUpdateRequest, WorkspaceId,
};

pub struct ExtractedPlan {
    pub target_path: String,
    pub target_type: CadQueryObjectKind,
    pub affected_paths: Vec<String>,
    pub description: String,
}

pub struct SavedCadPlan {
    pub plan_ref: String,
    pub target_path: String,
    pub target_type: CadQueryObjectKind,
    pub affected_paths: Vec<String>,
    pub new_paths: Vec<String>,
    pub export_targets: Vec<String>,
    pub description: String,
}

pub fn extract_plan_proposal(
    response_text: &str,
    selection: &SelectionUpdateRequest,
) -> Option<ExtractedPlan> {
    if let Some(plan) = extract_plan_from_json_block(response_text) {
        return Some(plan);
    }
    extract_plan_from_selection(response_text, selection)
}

pub fn latest_saved_cad_plan(messages: &[ChatMessageRecord], run_id: &str) -> Option<SavedCadPlan> {
    messages
        .iter()
        .rev()
        .filter_map(|message| saved_plan_from_message(message, run_id))
        .next()
}

pub fn validate_saved_plan_confirmation(
    confirmation: &AgentCadQueryConfirmation,
    plan: &SavedCadPlan,
) -> Result<(), &'static str> {
    let Some(plan_ref) = &confirmation.plan_ref else {
        return Err("Agent confirmation 缺少已保存 CAD Plan 的 plan_ref");
    };
    if plan_ref.display_path() != plan.plan_ref {
        return Err("Agent confirmation plan_ref 与已保存 CAD Plan 不一致");
    }
    if confirmation.request.target_path.display_path() != plan.target_path {
        return Err("Agent confirmation target_path 与已保存 CAD Plan 不一致");
    }
    if confirmation.request.target_type != plan.target_type {
        return Err("Agent confirmation target_type 与已保存 CAD Plan 不一致");
    }
    if !same_paths(&confirmation.affected_files, &plan.affected_paths)
        || !same_paths(&confirmation.new_files, &plan.new_paths)
        || !same_paths(&confirmation.export_targets, &plan.export_targets)
    {
        return Err("Agent execution scope 与已保存 CAD Plan 不一致");
    }
    Ok(())
}

pub async fn parse_plan_package(
    workspace_root: &Path,
    plan_ref: &PathHandle,
) -> Result<ParsedPlanPackage, ProtocolError> {
    app_server_core::parse_plan_package(workspace_root, &plan_ref.display_path())
        .await
        .map_err(|error| ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, error.message))
}

pub async fn execution_scope_from_plan_ref(
    workspace_root: &Path,
    plan_ref: &PathHandle,
) -> Result<AgentExecutionScope, ProtocolError> {
    parse_plan_package(workspace_root, plan_ref).await
        .map(|plan| AgentExecutionScope::from_plan_package(&plan))
}

fn saved_plan_from_message(message: &ChatMessageRecord, run_id: &str) -> Option<SavedCadPlan> {
    let result = message.tool_result.as_ref()?;
    if result.tool_name != "save_cad_plan" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&result.result_json).ok()?;
    if value.get("status")?.as_str()? != "ok" || value.get("run_id")?.as_str()? != run_id {
        return None;
    }
    let target_path = string_field(&value, "target_path")?;
    let target_type = string_field(&value, "target_type")
        .and_then(|value| target_type_from_label(&value))
        .unwrap_or_else(|| target_type_for_path(&target_path));
    Some(SavedCadPlan {
        plan_ref: string_field(&value, "plan_ref")?,
        target_type,
        affected_paths: string_array_field(&value, "affected_files")
            .filter(|paths| !paths.is_empty())
            .unwrap_or_else(|| vec![target_path.clone()]),
        new_paths: string_array_field(&value, "new_files").unwrap_or_default(),
        export_targets: string_array_field(&value, "export_targets").unwrap_or_default(),
        description: string_field(&value, "summary").unwrap_or_default(),
        target_path,
    })
}

fn target_type_from_label(value: &str) -> Option<CadQueryObjectKind> {
    match value {
        "assembly" => Some(CadQueryObjectKind::Assembly),
        "component" => Some(CadQueryObjectKind::Component),
        "part" => Some(CadQueryObjectKind::Part),
        _ => None,
    }
}

fn same_paths(handles: &[PathHandle], paths: &[String]) -> bool {
    let mut left = handles
        .iter()
        .map(PathHandle::display_path)
        .collect::<Vec<_>>();
    let mut right = paths.to_vec();
    left.sort();
    right.sort();
    left == right
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

fn string_array_field(value: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    Some(
        value
            .get(key)?
            .as_array()?
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
    )
}

fn target_type_for_path(path: &str) -> CadQueryObjectKind {
    if path.starts_with("assemblies/") {
        CadQueryObjectKind::Assembly
    } else if path.starts_with("components/") {
        CadQueryObjectKind::Component
    } else {
        CadQueryObjectKind::Part
    }
}

pub fn extract_plan_from_json_block(response_text: &str) -> Option<ExtractedPlan> {
    let start = response_text.find("```json")?;
    let json_start = start + "```json".len();
    let end = response_text[json_start..].find("```")?;
    let json_str = response_text[json_start..json_start + end].trim();
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let target_path = value.get("target_path")?.as_str()?.to_owned();
    let target_type = match value.get("target_type").and_then(|v| v.as_str()) {
        Some("assembly") => CadQueryObjectKind::Assembly,
        Some("component") => CadQueryObjectKind::Component,
        _ => CadQueryObjectKind::Part,
    };
    let description = value
        .get("description")
        .or_else(|| value.get("change_description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let affected_paths = value
        .get("affected_files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Some(ExtractedPlan {
        target_path,
        target_type,
        affected_paths,
        description,
    })
}

pub fn extract_plan_from_selection(
    response_text: &str,
    selection: &SelectionUpdateRequest,
) -> Option<ExtractedPlan> {
    let has_modify_intent = ["modify", "change", "add", "create", "fillet", "chamfer"]
        .iter()
        .any(|kw| response_text.to_lowercase().contains(kw));
    if !has_modify_intent {
        return None;
    }
    let active_sel = selection
        .active_index
        .and_then(|idx| selection.selections.get(idx as usize))
        .or_else(|| selection.selections.last())?;
    let owner_kind = active_sel.owner_object_kind?;
    let owner_ref = active_sel.owner_ref_text.as_deref()?;
    let name = extract_object_name(owner_ref)?;
    let (target_path, target_type) = match owner_kind {
        CadQueryObjectKind::Assembly => (format!("assemblies/{name}.py"), owner_kind),
        CadQueryObjectKind::Component => (format!("components/{name}.py"), owner_kind),
        CadQueryObjectKind::Part => (format!("parts/{name}.py"), owner_kind),
    };
    Some(ExtractedPlan {
        target_path: target_path.clone(),
        target_type,
        affected_paths: vec![target_path],
        description: String::new(),
    })
}

pub fn extract_object_name(ref_text: &str) -> Option<String> {
    let start = ref_text.find('[')?;
    let end = ref_text.find(']')?;
    let name = ref_text[start + 1..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_owned())
    }
}

pub fn plan_target_handle(
    _workspace_root: &Path,
    relative_path: &str,
) -> Result<PathHandle, ProtocolError> {
    let segments: Vec<&str> = relative_path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidPathHandle,
            "empty plan target path",
        ));
    }
    PathHandle::new(
        WorkspaceId::new("workspace"),
        segments.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
    .map_err(|err| ProtocolError::new(ProtocolErrorCode::InvalidPathHandle, err.to_string()))
}

pub fn export_handle_for(target: &PathHandle) -> PathHandle {
    let segments = target.path_segments();
    let last = segments.last().map(|s| s.as_str()).unwrap_or("model.py");
    let stem = last.rsplit_once('.').map(|(s, _)| s).unwrap_or("model");
    PathHandle::new(
        WorkspaceId::new("workspace"),
        vec!["outputs".to_string(), format!("{stem}.step")],
    )
    .unwrap_or_else(|_| target.clone())
}
