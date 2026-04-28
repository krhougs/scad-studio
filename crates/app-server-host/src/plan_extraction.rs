use std::path::Path;

use app_server_protocol::{
    CadQueryObjectKind, PathHandle, ProtocolError, ProtocolErrorCode, SelectionUpdateRequest,
    WorkspaceId,
};

pub struct ExtractedPlan {
    pub target_path: String,
    pub target_type: CadQueryObjectKind,
    pub affected_paths: Vec<String>,
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
    if name.is_empty() { None } else { Some(name.to_owned()) }
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
    let stem = last
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or("model");
    PathHandle::new(
        WorkspaceId::new("workspace"),
        vec!["outputs".to_string(), format!("{stem}.step")],
    )
    .unwrap_or_else(|_| target.clone())
}
