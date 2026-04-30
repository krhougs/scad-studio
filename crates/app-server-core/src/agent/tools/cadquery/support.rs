use std::path::Path;

use app_server_protocol::{CadQueryMeshPayload, CadQueryObjectKind, CadQueryPartMesh};
use serde_json::{Value, json};

use crate::llm::LlmToolCall;

use super::super::{CadQueryToolCachedResult, CadQueryToolRunResult};

#[derive(Debug, Clone)]
pub(super) struct SourceContract {
    pub(super) target_type_matches: bool,
    pub(super) has_build_function: bool,
    pub(super) has_refs: bool,
    pub(super) has_model_description: bool,
    pub(super) unsafe_calls: Vec<&'static str>,
    pub(super) invalid_imports: Vec<String>,
}

pub(super) fn analyze_success(
    workspace_root: &Path,
    call: &LlmToolCall,
    target_path: &str,
    include_paired_doc: bool,
    include_dependencies: bool,
    source: &str,
) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery source analyzed",
        "target_path": target_path,
        "target_type": target_type_label(target_type_from_path(target_path)),
        "has_build_function": has_build_function(source),
        "has_refs": has_refs(source),
        "has_model_description": has_model_description(source),
        "paired_doc_path": include_paired_doc.then(|| paired_doc_path(workspace_root, target_path)).flatten(),
        "local_dependencies": if include_dependencies { local_dependencies(source) } else { Vec::new() },
        "ref_keys": feature_keys(source),
        "warnings": analyze_warnings(source)
    })
}

pub(super) fn source_contract(
    path: &str,
    expected_type: CadQueryObjectKind,
    source: &str,
) -> SourceContract {
    SourceContract {
        target_type_matches: declared_type(source).unwrap_or_else(|| target_type_from_path(path))
            == expected_type,
        has_build_function: has_build_function(source),
        has_refs: has_refs(source),
        has_model_description: has_model_description(source),
        unsafe_calls: unsafe_calls(source),
        invalid_imports: invalid_project_imports(source),
    }
}

pub(super) fn contract_json(contract: &SourceContract) -> Value {
    json!({
        "target_type_matches": contract.target_type_matches,
        "has_build_function": contract.has_build_function,
        "has_refs": contract.has_refs,
        "has_model_description": contract.has_model_description,
        "unsafe_calls": contract.unsafe_calls,
        "invalid_imports": contract.invalid_imports
    })
}

pub(super) fn contract_warnings(contract: &SourceContract) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    if !contract.has_build_function {
        warnings.push("missing build function");
    }
    if !contract.has_refs {
        warnings.push(
            "missing REFS.features; add e.g. REFS = {\"type\":\"part\",\"features\":{\"part_body\":{},\"placement_pocket\":{},\"access_notch\":{}}}",
        );
    }
    if !contract.has_model_description {
        warnings.push(
            "missing MODEL_DESCRIPTION / MODEL_DETAILS; add a concise purpose string and structured model details for file-list preview and human review",
        );
    }
    if !contract.target_type_matches {
        warnings.push("target type does not match REFS type");
    }
    if !contract.invalid_imports.is_empty() {
        warnings.push("invalid project-local import");
    }
    if !contract.unsafe_calls.is_empty() {
        warnings.push("unsafe call detected");
    }
    warnings
}

pub(super) fn run_success(
    call: &LlmToolCall,
    result: CadQueryToolRunResult,
    committed: bool,
) -> Value {
    if committed {
        let message = if result.warnings.is_empty() {
            "CadQuery execution completed"
        } else {
            "CadQuery execution completed with warnings"
        };
        json!({
            "status": "ok",
            "tool": call.function_name,
            "message": message,
            "result_id": result.mesh.result_id,
            "build_id": result.mesh.build_id,
            "committed_files": result.committed_files,
            "exports": result.exports,
            "summary": mesh_summary(&result.mesh),
            "warnings": result.warnings
        })
    } else {
        json!({
            "status": "ok",
            "tool": call.function_name,
            "message": "CadQuery dry run completed",
            "result_id": result.mesh.result_id,
            "build_id": result.mesh.build_id,
            "root_object_kind": target_type_label(result.mesh.root_object_kind),
            "summary": mesh_summary(&result.mesh),
            "warnings": result.warnings
        })
    }
}

pub(super) fn result_success(call: &LlmToolCall, result: &CadQueryToolCachedResult) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery result summarized",
        "result_id": result.mesh.result_id,
        "build_id": result.mesh.build_id,
        "root_ref_text": result.mesh.root_ref_text,
        "root_object_kind": target_type_label(result.mesh.root_object_kind),
        "parts": result.mesh.parts.iter().map(part_summary).collect::<Vec<_>>(),
        "exports": result.exports
    })
}

pub(super) fn resolve_selection_success(
    call: &LlmToolCall,
    mesh: &CadQueryMeshPayload,
    ref_text: &str,
) -> Value {
    let resolved = resolve_face_ref(mesh, ref_text).or_else(|| resolve_feature_ref(mesh, ref_text));
    let (owner, feature, stable, ambiguous) = match resolved {
        Some((owner, feature)) => (Some(owner), Some(feature.clone()), Some(feature), false),
        None => (None, None, None, true),
    };
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery selection resolved",
        "raw_ref_text": ref_text,
        "owner_ref_text": owner,
        "owner_path": Value::Null,
        "owner_doc_path": Value::Null,
        "candidate_feature_ref": feature,
        "stable_ref": stable,
        "ambiguous": ambiguous,
        "risks": if ambiguous { vec!["selection could not be mapped to a stable feature"] } else { Vec::new() }
    })
}

pub(super) fn is_model_path(path: &str) -> bool {
    matches!(
        path.split('/').next().unwrap_or(""),
        "components" | "parts" | "assemblies"
    ) && path.ends_with(".py")
}

pub(super) fn target_type_label(kind: CadQueryObjectKind) -> &'static str {
    match kind {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
    }
}

fn target_type_from_path(path: &str) -> CadQueryObjectKind {
    match path.split('/').next().unwrap_or("") {
        "components" => CadQueryObjectKind::Component,
        "assemblies" => CadQueryObjectKind::Assembly,
        _ => CadQueryObjectKind::Part,
    }
}

fn paired_doc_path(root: &Path, source: &str) -> Option<String> {
    let doc = source.strip_suffix(".py")?.to_owned() + ".md";
    root.join(&doc).is_file().then_some(doc)
}

fn has_build_function(source: &str) -> bool {
    source.contains("def build(") || source.contains("def build (")
}

fn has_refs(source: &str) -> bool {
    source.contains("REFS") && source.contains("features")
}

fn has_model_description(source: &str) -> bool {
    source.contains("MODEL_DESCRIPTION") && source.contains("MODEL_DETAILS")
}

fn declared_type(source: &str) -> Option<CadQueryObjectKind> {
    for (needle, kind) in [
        ("\"type\":\"part\"", CadQueryObjectKind::Part),
        ("\"type\": \"part\"", CadQueryObjectKind::Part),
        ("\"type\":\"component\"", CadQueryObjectKind::Component),
        ("\"type\": \"component\"", CadQueryObjectKind::Component),
        ("\"type\":\"assembly\"", CadQueryObjectKind::Assembly),
        ("\"type\": \"assembly\"", CadQueryObjectKind::Assembly),
    ] {
        if source.contains(needle) {
            return Some(kind);
        }
    }
    None
}

fn unsafe_calls(source: &str) -> Vec<&'static str> {
    [
        ("open(", "open"),
        ("open (", "open"),
        ("io.open", "io.open"),
        ("Path(", "Path"),
        ("Path (", "Path"),
        ("write_text(", "write_text"),
        ("write_bytes(", "write_bytes"),
        ("unlink(", "unlink"),
        ("subprocess.", "subprocess"),
        ("os.system", "os.system"),
        ("os.remove", "os.remove"),
        ("os.rename", "os.rename"),
        ("os.replace", "os.replace"),
        ("shutil.rmtree", "shutil.rmtree"),
        ("shutil.move", "shutil.move"),
    ]
    .into_iter()
    .filter_map(|(needle, label)| source.contains(needle).then_some(label))
    .collect()
}

fn invalid_project_imports(source: &str) -> Vec<String> {
    let mut imports = source
        .lines()
        .filter_map(invalid_project_import_line)
        .collect::<Vec<_>>();
    imports.sort();
    imports.dedup();
    imports
}

fn invalid_project_import_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("from .") {
        return Some(trimmed.to_owned());
    }
    if let Some(module) = trimmed
        .strip_prefix("from ")
        .and_then(|rest| rest.split_once(" import ").map(|(module, _)| module))
    {
        return invalid_import_root(module).then(|| trimmed.to_owned());
    }
    let modules = trimmed.strip_prefix("import ")?;
    modules
        .split(',')
        .any(|module| invalid_import_root(module))
        .then(|| trimmed.to_owned())
}

fn invalid_import_root(module: &str) -> bool {
    let root = module
        .trim()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("");
    matches!(
        root,
        "chats" | "plans" | "outputs" | ".budn_staging" | "docs" | "target" | "node_modules"
    )
}

fn analyze_warnings(source: &str) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    if !has_build_function(source) {
        warnings.push("missing build function");
    }
    if !has_refs(source) {
        warnings.push("missing REFS features");
    }
    if !has_model_description(source) {
        warnings.push("missing MODEL_DESCRIPTION / MODEL_DETAILS");
    }
    warnings
}

fn local_dependencies(source: &str) -> Vec<String> {
    let mut deps = source
        .lines()
        .filter_map(local_dependency_line)
        .collect::<Vec<_>>();
    deps.sort();
    deps.dedup();
    deps
}

fn local_dependency_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let module = trimmed
        .strip_prefix("from ")
        .and_then(|rest| rest.split_once(" import ").map(|(module, _)| module))
        .or_else(|| trimmed.strip_prefix("import "))?;
    let root = module.split('.').next()?;
    matches!(root, "components" | "parts" | "assemblies")
        .then(|| format!("{}.py", module.replace('.', "/")))
}

fn feature_keys(source: &str) -> Vec<String> {
    let Some(features_index) = source
        .find("\"features\"")
        .or_else(|| source.find("'features'"))
    else {
        return Vec::new();
    };
    let Some(open_offset) = source[features_index..].find('{') else {
        return Vec::new();
    };
    source[features_index + open_offset + 1..]
        .split(['"', '\''])
        .skip(1)
        .step_by(2)
        .take_while(|key| *key != "selector")
        .filter(|key| !matches!(*key, "type" | "features"))
        .map(str::to_owned)
        .take(20)
        .collect()
}

fn part_summary(part: &CadQueryPartMesh) -> Value {
    json!({
        "ref_text": part.ref_text,
        "object_kind": target_type_label(part.object_kind),
        "instance_path": part.instance_path,
        "features": part.feature_map.iter().map(|item| item.feature.clone()).collect::<Vec<_>>(),
        "face_count": part.faces.len(),
        "edge_count": part.edges.len(),
        "vertex_count": part.vertices.len()
    })
}

fn mesh_summary(mesh: &CadQueryMeshPayload) -> Value {
    json!({
        "part_count": mesh.parts.len(),
        "face_count": mesh.parts.iter().map(|part| part.faces.len()).sum::<usize>(),
        "edge_count": mesh.parts.iter().map(|part| part.edges.len()).sum::<usize>(),
        "vertex_count": mesh.parts.iter().map(|part| part.vertices.len()).sum::<usize>(),
        "features": mesh.parts.iter().flat_map(|part| part.feature_map.iter().map(|feature| feature.feature.clone())).collect::<Vec<_>>()
    })
}

fn resolve_face_ref(mesh: &CadQueryMeshPayload, ref_text: &str) -> Option<(String, String)> {
    let (owner, face_idx) = parse_face_ref(ref_text)?;
    let part = mesh
        .parts
        .iter()
        .find(|part| part_name_matches(part, &owner))?;
    let feature = part
        .feature_map
        .iter()
        .find(|feature| feature.face_indices.contains(&face_idx))?;
    Some((
        part.ref_text.clone(),
        format!("@feature[{owner}.{}]", feature.feature),
    ))
}

fn resolve_feature_ref(mesh: &CadQueryMeshPayload, ref_text: &str) -> Option<(String, String)> {
    let value = ref_text.strip_prefix("@feature[")?.strip_suffix(']')?;
    let (owner, feature_name) = value.split_once('.')?;
    let part = mesh
        .parts
        .iter()
        .find(|part| part_name_matches(part, owner))?;
    part.feature_map
        .iter()
        .any(|feature| feature.feature == feature_name)
        .then(|| (part.ref_text.clone(), ref_text.to_owned()))
}

fn parse_face_ref(ref_text: &str) -> Option<(String, u32)> {
    let value = ref_text.strip_prefix("@face[")?.strip_suffix(']')?;
    let (owner, face) = value.split_once(":f_")?;
    Some((owner.to_owned(), face.parse().ok()?))
}

fn part_name_matches(part: &CadQueryPartMesh, owner: &str) -> bool {
    part.name == owner || part.ref_text == format!("@part[{owner}]")
}
